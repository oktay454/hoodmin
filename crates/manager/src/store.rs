use crate::{
	crypto::{Vault, random},
	policy::AccessPolicy,
};
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx_core::{row::Row, transaction::Transaction};
use sqlx_postgres::{PgPool, PgRow, Postgres};
use uuid::Uuid;
use vpnhood_protocol::{
	AccessInfo, AccessUsage, AdRequirement, ConnectPlanId, SessionErrorCode as Code,
	SessionRequest, SessionResponse, SessionSuppressType, SessionUsage, Traffic,
	validate_client_id,
};

#[derive(Clone)]
pub struct Store {
	pub pool: PgPool,
	pub vault: Vault,
}
impl Store {
	pub async fn begin(&self, tenant: Uuid) -> Result<Transaction<'_, Postgres>> {
		let mut tx = self.pool.begin().await?;
		sqlx_core::query::query("SELECT set_config('app.tenant_id',$1,true)")
			.bind(tenant.to_string())
			.execute(&mut *tx)
			.await?;
		Ok(tx)
	}
	pub async fn create(
		&self,
		tx: &mut Transaction<'_, Postgres>,
		tenant: Uuid,
		server: Uuid,
		request: SessionRequest,
	) -> Result<SessionResponse> {
		let paused: bool = sqlx_core::query_scalar::query_scalar(
			"SELECT paused FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
		)
		.bind(tenant)
		.bind(server)
		.fetch_one(&mut **tx)
		.await?;
		if request.client_info.client_id.len() > 256
			|| request
				.host_end_point
				.parse::<std::net::SocketAddr>()
				.is_err()
		{
			return Ok(SessionResponse::error(Code::AccessError));
		}
		let Ok(mut token_id) = Uuid::parse_str(&request.token_id) else {
			return Ok(SessionResponse::error(Code::AccessError));
		};
		let code_token: Option<Uuid> =
			if let Some(code) = request.access_code.as_ref().filter(|s| !s.is_empty()) {
				if code.len() > 256 {
					return Ok(SessionResponse::error(Code::AccessCodeRejected));
				}
				sqlx_core::query_scalar::query_scalar(
					"SELECT token_id FROM access_codes WHERE tenant_id=$1 AND code_hash=$2",
				)
				.bind(tenant)
				.bind(Sha256::digest(code.as_bytes()).to_vec())
				.fetch_optional(&mut **tx)
				.await?
			} else {
				None
			};
		let ids = vec![token_id, code_token.unwrap_or(token_id)];
		sqlx_core::query::query(
			"SELECT id FROM access_tokens WHERE tenant_id=$1 AND id=ANY($2) ORDER BY id FOR UPDATE",
		)
		.bind(tenant)
		.bind(&ids)
		.fetch_all(&mut **tx)
		.await?;
		let token =
			sqlx_core::query::query("SELECT * FROM access_tokens WHERE tenant_id=$1 AND id=$2")
				.bind(tenant)
				.bind(token_id)
				.fetch_optional(&mut **tx)
				.await?;
		let Some(token) = token else {
			return Ok(SessionResponse::error(Code::AccessError));
		};
		let secret = self
			.vault
			.open(token.get("secret"), &format!("{tenant}:{token_id}:token"))?;
		if !validate_client_id(
			&request.client_info.client_id,
			&secret,
			&request.encrypted_client_id,
		) {
			return Ok(SessionResponse::error(Code::AccessError));
		}
		let current_farm: Option<Uuid> = sqlx_core::query_scalar::query_scalar(
			"SELECT farm_id FROM servers WHERE tenant_id=$1 AND id=$2",
		)
		.bind(tenant)
		.bind(server)
		.fetch_one(&mut **tx)
		.await?;
		let seed: Uuid = token.get("server_id");
		let token_farm: Option<Uuid> = token.get("farm_id");
		if seed != server && (token_farm.is_none() || token_farm != current_farm) {
			return Ok(SessionResponse::error(Code::AccessError));
		}
		let mut token = self.roll_cycle(tx, tenant, token_id, token).await?;
		let mut policy: AccessPolicy = serde_json::from_value(token.get("policy"))?;
		let code = token_error(&token, &policy);
		if code != Code::Ok {
			return Ok(SessionResponse::error(code));
		}
		if request.access_code.as_ref().is_some_and(|s| !s.is_empty()) {
			let Some(code_id) = code_token else {
				return Ok(SessionResponse::error(Code::AccessCodeRejected));
			};
			let replacement =
				sqlx_core::query::query("SELECT * FROM access_tokens WHERE tenant_id=$1 AND id=$2")
					.bind(tenant)
					.bind(code_id)
					.fetch_one(&mut **tx)
					.await?;
			let seed: Uuid = replacement.get("server_id");
			let farm: Option<Uuid> = replacement.get("farm_id");
			if seed != server && (farm.is_none() || farm != current_farm) {
				return Ok(SessionResponse::error(Code::AccessCodeRejected));
			}
			token_id = code_id;
			token = self.roll_cycle(tx, tenant, token_id, replacement).await?;
			policy = serde_json::from_value(token.get("policy"))?;
			let code = token_error(&token, &policy);
			if code != Code::Ok {
				return Ok(SessionResponse::error(code));
			}
		}
		sqlx_core::query::query("UPDATE sessions SET closed=true,pending_notify=true,decision='SessionExpired' WHERE tenant_id=$1 AND token_id=$2 AND NOT closed AND (expires_at<=now() OR last_seen<now()-interval '48 hours')").bind(tenant).bind(token_id).execute(&mut **tx).await?;
		let Some((premium, mut seconds, ad_requirement)) = policy.plan(&request.plan_id) else {
			return Ok(SessionResponse::error(Code::PlanRejected));
		};
		let farm: Option<Uuid> = token.get("farm_id");
		let seed: Uuid = token.get("server_id");
		if paused && farm.is_none() {
			return Ok(SessionResponse::error(Code::Maintenance));
		}
		if let Some(response) =
			crate::farms::route(self, tx, tenant, server, seed, farm, premium, &request).await?
		{
			return Ok(response);
		}
		if ad_requirement != AdRequirement::None {
			let available: bool = sqlx_core::query_scalar::query_scalar(
				"SELECT EXISTS(SELECT FROM ad_hooks WHERE tenant_id=$1)",
			)
			.bind(tenant)
			.fetch_one(&mut **tx)
			.await?;
			if !available {
				return Ok(SessionResponse::error(Code::PlanRejected));
			}
		}
		let client_hash = Sha256::digest(request.client_info.client_id.as_bytes()).to_vec();
		if request.plan_id == ConnectPlanId::PremiumByTrial && policy.trial_daily_seconds > 0 {
			let used: i64 = sqlx_core::query_scalar::query_scalar("SELECT ceil(coalesce(sum(greatest(0,extract(epoch FROM least(now(),coalesce(expires_at,now()),last_seen)-created_at))),0))::bigint FROM sessions WHERE tenant_id=$1 AND token_id=$2 AND client_hash=$3 AND metadata->>'planId'='PremiumByTrial' AND created_at >= date_trunc('day',now())")
				.bind(tenant).bind(token_id).bind(&client_hash).fetch_one(&mut **tx).await?;
			if used >= policy.trial_daily_seconds {
				return Ok(SessionResponse::error(Code::DailyLimitExceeded));
			}
			seconds = seconds.min(policy.trial_daily_seconds - used);
		}
		if policy.device_life_span > 0 {
			sqlx_core::query::query("DELETE FROM access_devices WHERE tenant_id=$1 AND token_id=$2 AND last_used_at < now()-make_interval(days=>$3)")
				.bind(tenant).bind(token_id).bind(policy.device_life_span).execute(&mut **tx).await?;
			let devices: i64 = sqlx_core::query_scalar::query_scalar(
				"SELECT count(*) FROM access_devices WHERE tenant_id=$1 AND token_id=$2 AND client_hash<>$3",
			)
			.bind(tenant)
			.bind(token_id)
			.bind(&client_hash)
			.fetch_one(&mut **tx)
			.await?;
			if token.get::<i32, _>("max_clients") > 0
				&& devices >= i64::from(token.get::<i32, _>("max_clients"))
			{
				return Ok(SessionResponse::error(Code::AccessLocked));
			}
		}
		let clients: i64 = sqlx_core::query_scalar::query_scalar("SELECT count(DISTINCT client_hash) FROM sessions WHERE tenant_id=$1 AND token_id=$2 AND NOT closed AND client_hash<>$3")
			.bind(tenant).bind(token_id).bind(&client_hash).fetch_one(&mut **tx).await?;
		let mut suppressed_to = SessionSuppressType::None;
		if token.get::<i32, _>("max_clients") > 0
			&& clients >= i64::from(token.get::<i32, _>("max_clients"))
		{
			if !policy.suppress_other_devices {
				return Ok(SessionResponse::error(Code::AccessLocked));
			}
			sqlx_core::query::query("UPDATE sessions SET closed=true,pending_notify=true,decision='SessionSuppressedBy',suppressed_by='Other' WHERE tenant_id=$1 AND token_id=$2 AND NOT closed AND client_hash IN (SELECT client_hash FROM sessions WHERE tenant_id=$1 AND token_id=$2 AND NOT closed AND client_hash<>$3 GROUP BY client_hash ORDER BY min(last_seen) LIMIT $4)")
				.bind(tenant).bind(token_id).bind(&client_hash).bind(clients-i64::from(token.get::<i32,_>("max_clients"))+1).execute(&mut **tx).await?;
			suppressed_to = SessionSuppressType::Other;
		}
		let previous = sqlx_core::query::query("UPDATE sessions SET closed=true,pending_notify=true,decision='SessionSuppressedBy',suppressed_by='YourSelf' WHERE tenant_id=$1 AND token_id=$2 AND client_hash=$3 AND NOT closed")
			.bind(tenant).bind(token_id).bind(&client_hash).execute(&mut **tx).await?.rows_affected();
		if previous > 0 && suppressed_to == SessionSuppressType::None {
			suppressed_to = SessionSuppressType::YourSelf;
		}
		let session_key = random::<16>();
		let key = self
			.vault
			.seal(&session_key, &format!("{tenant}:{server}:session"))?;
		let country = request
			.client_country
			.filter(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_alphabetic()))
			.map(|c| c.to_uppercase());
		let metadata = serde_json::json!({"clientCountry":country,"extraData":request.extra_data,"clientIp":request.client_ip,"clientVersion":request.client_info.client_version,"userAgent":request.client_info.user_agent,"hostEndPoint":request.host_end_point,"remoteEndPoint":request.remote_end_point,"planId":request.plan_id,"allowAnonymousTracker":request.allow_anonymous_tracker,"userReview":request.user_review});
		let expiration = if ad_requirement == AdRequirement::Rewarded {
			Some(Utc::now() + chrono::Duration::minutes(2))
		} else if seconds > 0 {
			Some(Utc::now() + chrono::Duration::seconds(seconds))
		} else {
			None
		};
		let id: i64 = sqlx_core::query_scalar::query_scalar("INSERT INTO sessions(tenant_id,server_id,token_id,client_hash,session_key,protocol_version,metadata,expires_at,premium,suppressed_to,ad_requirement) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING id")
			.bind(tenant).bind(server).bind(token_id).bind(&client_hash).bind(key).bind(request.protocol_version).bind(&metadata).bind(expiration).bind(premium).bind(serde_json::to_value(suppressed_to)?.as_str().unwrap_or("None")).bind(serde_json::to_value(ad_requirement)?.as_str().unwrap_or("None")).fetch_one(&mut **tx).await?;
		sqlx_core::query::query("UPDATE access_tokens SET first_used_at=coalesce(first_used_at,now()),last_used_at=now(),cycle_started_at=coalesce(cycle_started_at,now()) WHERE tenant_id=$1 AND id=$2")
			.bind(tenant).bind(token_id).execute(&mut **tx).await?;
		sqlx_core::query::query("INSERT INTO access_devices(tenant_id,token_id,client_hash,metadata) VALUES($1,$2,$3,$4) ON CONFLICT(tenant_id,token_id,client_hash) DO UPDATE SET last_used_at=now(),metadata=$4")
			.bind(tenant).bind(token_id).bind(&client_hash).bind(serde_json::json!({"operatingSystem":metadata["userAgent"]})).execute(&mut **tx).await?;
		self.get(tx, tenant, server, id as u64).await
	}
	pub async fn get(
		&self,
		tx: &mut Transaction<'_, Postgres>,
		tenant: Uuid,
		server: Uuid,
		id: u64,
	) -> Result<SessionResponse> {
		let Ok(id_signed) = i64::try_from(id) else {
			return Ok(SessionResponse::error(Code::SessionError));
		};
		let session = sqlx_core::query::query(
			"SELECT * FROM sessions WHERE tenant_id=$1 AND server_id=$2 AND id=$3",
		)
		.bind(tenant)
		.bind(server)
		.bind(id_signed)
		.fetch_optional(&mut **tx)
		.await?;
		let Some(session) = session else {
			return Ok(SessionResponse::error(Code::SessionError));
		};
		let token_id: Uuid = session.get("token_id");
		let token = sqlx_core::query::query(
			"SELECT * FROM access_tokens WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
		)
		.bind(tenant)
		.bind(token_id)
		.fetch_one(&mut **tx)
		.await?;
		let token = self.roll_cycle(tx, tenant, token_id, token).await?;
		let policy: AccessPolicy = serde_json::from_value(token.get("policy"))?;
		let current_farm: Option<Uuid> = sqlx_core::query_scalar::query_scalar(
			"SELECT farm_id FROM servers WHERE tenant_id=$1 AND id=$2",
		)
		.bind(tenant)
		.bind(server)
		.fetch_one(&mut **tx)
		.await?;
		let permitted = match token.get::<Option<Uuid>, _>("farm_id") {
			Some(farm) => current_farm == Some(farm),
			None => token.get::<Uuid, _>("server_id") == server,
		};
		let closed: bool = session.get("closed");
		let code = if !permitted {
			Code::AccessError
		} else if closed {
			let decision: String = session.get("decision");
			if decision == "Ok" {
				Code::SessionClosed
			} else {
				serde_json::from_value(Value::String(decision)).unwrap_or(Code::SessionClosed)
			}
		} else if session
			.get::<Option<DateTime<Utc>>, _>("expires_at")
			.is_some_and(|v| v <= Utc::now())
		{
			Code::SessionExpired
		} else {
			token_error(&token, &policy)
		};
		if code == Code::Ok {
			sqlx_core::query::query(
				"UPDATE sessions SET last_seen=now() WHERE tenant_id=$1 AND id=$2",
			)
			.bind(tenant)
			.bind(id_signed)
			.execute(&mut **tx)
			.await?;
		}
		sqlx_core::query::query("UPDATE access_devices SET last_used_at=now() WHERE tenant_id=$1 AND token_id=$2 AND client_hash=$3").bind(tenant).bind(token_id).bind(session.get::<Vec<u8>,_>("client_hash")).execute(&mut **tx).await?;
		let clients: i64 = sqlx_core::query_scalar::query_scalar("SELECT count(DISTINCT client_hash) FROM sessions WHERE tenant_id=$1 AND token_id=$2 AND NOT closed")
			.bind(tenant).bind(token_id).fetch_one(&mut **tx).await?;
		let metadata: Value = session.get("metadata");
		let expiration = effective_expiry(&token, &policy);
		let session_expiry: Option<DateTime<Utc>> = session.get("expires_at");
		let devices: i64 = sqlx_core::query_scalar::query_scalar(
			"SELECT count(*) FROM access_devices WHERE tenant_id=$1 AND token_id=$2",
		)
		.bind(tenant)
		.bind(token_id)
		.fetch_one(&mut **tx)
		.await?;
		let traffic = Traffic {
			sent: token.get("sent"),
			received: token.get("received"),
		};
		let server_policy: Value = sqlx_core::query_scalar::query_scalar(
			"SELECT management FROM servers WHERE tenant_id=$1 AND id=$2",
		)
		.bind(tenant)
		.bind(server)
		.fetch_one(&mut **tx)
		.await?;
		let server_policy: crate::farms::ServerPolicy = serde_json::from_value(server_policy)?;
		let access_key = if server_policy.push_token_to_clients {
			Some(crate::management::access_key(tx, &self.vault, tenant, token_id).await?)
		} else {
			None
		};
		Ok(SessionResponse {
			error_code: code,
			error_message: None,
			session_id: id,
			session_key: self.vault.open(
				session.get("session_key"),
				&format!("{tenant}:{server}:session"),
			)?,
			protocol_version: session.get("protocol_version"),
			created_time: Some(session.get("created_at")),
			access_usage: Some(AccessUsage {
				is_premium: session.get("premium"),
				can_extend_by_rewarded_ad: policy.rewarded_seconds > 0,
				is_user_review_recommended: false,
				user_review_recommended: 0,
				cycle_traffic: Traffic {
					sent: token.get("cycle_sent"),
					received: token.get("cycle_received"),
				},
				total_traffic: traffic,
				max_traffic: session_quota(&token, &policy),
				expiration_time: expiration.into_iter().chain(session_expiry).min(),
				active_client_count: clients,
			}),
			extra_data: metadata["extraData"].as_str().map(str::to_owned),
			client_country: metadata["clientCountry"].as_str().map(str::to_owned),
			client_public_address: metadata["clientIp"].as_str().and_then(|ip| ip.parse().ok()),
			suppressed_by: serde_json::from_value(Value::String(session.get("suppressed_by")))?,
			suppressed_to: serde_json::from_value(Value::String(session.get("suppressed_to")))?,
			ad_requirement: serde_json::from_value(Value::String(session.get("ad_requirement")))?,
			access_info: Some(AccessInfo {
				is_new: false,
				created_time: token.get("created_at"),
				last_used_time: token
					.get::<Option<DateTime<Utc>>, _>("last_used_at")
					.unwrap_or_else(Utc::now),
				expiration_time: expiration,
				is_premium: policy.is_premium,
				max_cycle_traffic: policy.max_cycle_traffic,
				max_total_traffic: token.get("max_traffic"),
				max_device_count: token.get("max_clients"),
				max_speed_mbps: Some(Traffic {
					sent: policy.max_upload_mbps,
					received: policy.max_download_mbps,
				}),
				device_life_span: policy.device_life_span,
				devices_summary: Some(
					serde_json::json!({"deviceCount":devices,"hasMoreDevices":false,"devices":null}),
				),
			}),
			ga_measurement_id: server_policy
				.ga_measurement_id
				.filter(|_| metadata["allowAnonymousTracker"].as_bool() == Some(true)),
			server_tags: server_policy.tags,
			server_location: Some(server_policy.location),
			access_key,
			..SessionResponse::error(Code::Ok)
		})
	}
	pub async fn usages(
		&self,
		tx: &mut Transaction<'_, Postgres>,
		tenant: Uuid,
		server: Uuid,
		usages: Vec<SessionUsage>,
	) -> Result<std::collections::BTreeMap<u64, SessionResponse>> {
		ensure!(
			usages.len() <= 1000,
			"Bir bildirim en fazla 1000 oturum içerebilir."
		);
		ensure!(
			usages.iter().all(|u| u.sent >= 0 && u.received >= 0),
			"Trafik negatif olamaz."
		);
		let ids: Vec<i64> = usages
			.iter()
			.filter_map(|u| i64::try_from(u.session_id).ok())
			.collect();
		// Bütün token kilitleri sabit sırada alınır; farklı toplu işlemler birbirini kilitlemez.
		sqlx_core::query::query("SELECT id FROM access_tokens WHERE tenant_id=$1 AND id IN (SELECT token_id FROM sessions WHERE tenant_id=$1 AND server_id=$2 AND id=ANY($3)) ORDER BY id FOR UPDATE")
			.bind(tenant).bind(server).bind(ids).fetch_all(&mut **tx).await?;
		let mut responses = std::collections::BTreeMap::new();
		for usage in usages {
			if let Ok(id) = i64::try_from(usage.session_id) {
				let token_id: Option<Uuid> = sqlx_core::query_scalar::query_scalar(
					"SELECT token_id FROM sessions WHERE tenant_id=$1 AND server_id=$2 AND id=$3",
				)
				.bind(tenant)
				.bind(server)
				.bind(id)
				.fetch_optional(&mut **tx)
				.await?;
				if let Some(token) = token_id {
					let row = sqlx_core::query::query(
						"SELECT * FROM access_tokens WHERE tenant_id=$1 AND id=$2",
					)
					.bind(tenant)
					.bind(token)
					.fetch_one(&mut **tx)
					.await?;
					self.roll_cycle(tx, tenant, token, row).await?;
					if let Some(data) = usage.ad_data.as_ref() {
						let receipt: Option<i64> = if crate::agent::valid_ad_data(id, data) {
							sqlx_core::query_scalar::query_scalar("UPDATE ad_rewards SET consumed=true WHERE tenant_id=$1 AND session_id=$2 AND ad_hash=$3 AND NOT consumed AND expires_at>now() RETURNING session_id").bind(tenant).bind(id).bind(Sha256::digest(data.as_bytes()).to_vec()).fetch_optional(&mut **tx).await?
						} else {
							None
						};
						if receipt.is_some() {
							let policy: Value = sqlx_core::query_scalar::query_scalar(
								"SELECT policy FROM access_tokens WHERE tenant_id=$1 AND id=$2",
							)
							.bind(tenant)
							.bind(token)
							.fetch_one(&mut **tx)
							.await?;
							let policy: AccessPolicy = serde_json::from_value(policy)?;
							let expires = if policy.rewarded_seconds > 0 {
								Some(
									Utc::now() + chrono::Duration::seconds(policy.rewarded_seconds),
								)
							} else {
								None
							};
							sqlx_core::query::query("UPDATE sessions SET ad_verified=true,expires_at=$3 WHERE tenant_id=$1 AND id=$2 AND NOT closed").bind(tenant).bind(id).bind(expires).execute(&mut **tx).await?;
						} else {
							let verified: bool = sqlx_core::query_scalar::query_scalar(
								"SELECT ad_verified FROM sessions WHERE tenant_id=$1 AND id=$2",
							)
							.bind(tenant)
							.bind(id)
							.fetch_one(&mut **tx)
							.await?;
							if !verified {
								sqlx_core::query::query("UPDATE sessions SET closed=true,decision='RewardedAdRejected' WHERE tenant_id=$1 AND id=$2").bind(tenant).bind(id).execute(&mut **tx).await?;
							}
						}
					}
					// Raporlar kümülatif sayaç değil fark (delta) içerir; kimliksiz raporlara tekrar önleme garantisi verilemez.
					sqlx_core::query::query(
						"UPDATE access_tokens SET sent=sent+$3,received=received+$4,cycle_sent=cycle_sent+$3,cycle_received=cycle_received+$4,last_used_at=CASE WHEN $3+$4>0 THEN now() ELSE last_used_at END WHERE tenant_id=$1 AND id=$2",
					)
					.bind(tenant)
					.bind(token)
					.bind(usage.sent)
					.bind(usage.received)
					.execute(&mut **tx)
					.await?;
					sqlx_core::query::query(
						"UPDATE sessions SET last_seen=now(),closed=closed OR $3,pending_notify=CASE WHEN $3 THEN false ELSE pending_notify END WHERE tenant_id=$1 AND id=$2",
					)
					.bind(tenant)
					.bind(id)
					.bind(usage.closed || usage.error_code != Code::Ok)
					.execute(&mut **tx)
					.await?;
				}
			}
			responses.insert(
				usage.session_id,
				self.get(tx, tenant, server, usage.session_id).await?,
			);
		}
		Ok(responses)
	}
}
fn effective_expiry(token: &PgRow, policy: &AccessPolicy) -> Option<DateTime<Utc>> {
	let explicit: Option<DateTime<Utc>> = token.get("expires_at");
	let first: Option<DateTime<Utc>> = token.get("first_used_at");
	let lifetime = first
		.filter(|_| policy.lifetime_days > 0)
		.map(|v| v + chrono::Duration::days(policy.lifetime_days as i64));
	explicit.into_iter().chain(lifetime).min()
}
fn session_quota(token: &PgRow, policy: &AccessPolicy) -> i64 {
	let total_limit: i64 = token.get("max_traffic");
	if policy.max_cycle_traffic == 0 && total_limit == 0 {
		return 0;
	}
	if total_limit == 0 {
		return policy.max_cycle_traffic;
	}
	// AccessUsage uses the cycle counter; project remaining total quota onto that counter.
	let total = token
		.get::<i64, _>("sent")
		.saturating_add(token.get("received"));
	let cycle = token
		.get::<i64, _>("cycle_sent")
		.saturating_add(token.get("cycle_received"));
	let remaining = cycle.saturating_add(total_limit.saturating_sub(total).max(0));
	if policy.max_cycle_traffic == 0 {
		remaining
	} else {
		policy.max_cycle_traffic.min(remaining)
	}
}
fn token_error(token: &PgRow, policy: &AccessPolicy) -> Code {
	if token.get::<bool, _>("revoked") {
		return Code::AccessLocked;
	}
	if effective_expiry(token, policy).is_some_and(|e| e <= Utc::now()) {
		return Code::AccessExpired;
	}
	let max = token.get::<i64, _>("max_traffic");
	let total =
		i128::from(token.get::<i64, _>("sent")) + i128::from(token.get::<i64, _>("received"));
	let cycle = i128::from(token.get::<i64, _>("cycle_sent"))
		+ i128::from(token.get::<i64, _>("cycle_received"));
	if (max > 0 && total >= i128::from(max))
		|| (policy.max_cycle_traffic > 0 && cycle >= i128::from(policy.max_cycle_traffic))
	{
		return Code::AccessTrafficOverflow;
	}
	Code::Ok
}
impl Store {
	async fn roll_cycle(
		&self,
		tx: &mut Transaction<'_, Postgres>,
		tenant: Uuid,
		id: Uuid,
		token: PgRow,
	) -> Result<PgRow> {
		let policy: AccessPolicy = serde_json::from_value(token.get("policy"))?;
		if policy.cycle_seconds <= 0 {
			return Ok(token);
		}
		let start: Option<DateTime<Utc>> = token.get("cycle_started_at");
		if start.is_some_and(|s| s + chrono::Duration::seconds(policy.cycle_seconds) <= Utc::now())
		{
			let start = start.expect("due cycle");
			let elapsed = (Utc::now() - start).num_seconds() / policy.cycle_seconds;
			return Ok(sqlx_core::query::query("UPDATE access_tokens SET cycle_sent=0,cycle_received=0,cycle_started_at=$3 WHERE tenant_id=$1 AND id=$2 RETURNING *")
				.bind(tenant).bind(id).bind(start + chrono::Duration::seconds(elapsed * policy.cycle_seconds)).fetch_one(&mut **tx).await?);
		}
		Ok(token)
	}
}
