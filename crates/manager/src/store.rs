use crate::crypto::{Vault, random};
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx_core::{row::Row, transaction::Transaction};
use sqlx_postgres::{PgPool, PgRow, Postgres};
use uuid::Uuid;
use vpnhood_protocol::{
	AccessUsage, SessionErrorCode as Code, SessionRequest, SessionResponse, SessionUsage, Traffic,
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
		if request.client_info.client_id.len() > 256
			|| request
				.host_end_point
				.parse::<std::net::SocketAddr>()
				.is_err()
		{
			return Ok(SessionResponse::error(Code::AccessError));
		}
		let Ok(token_id) = Uuid::parse_str(&request.token_id) else {
			return Ok(SessionResponse::error(Code::AccessError));
		};
		let token = sqlx_core::query::query(
			"SELECT * FROM access_tokens WHERE tenant_id=$1 AND server_id=$2 AND id=$3 FOR UPDATE",
		)
		.bind(tenant)
		.bind(server)
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
		let code = token_error(&token);
		if code != Code::Ok {
			return Ok(SessionResponse::error(code));
		}
		let client_hash = Sha256::digest(request.client_info.client_id.as_bytes()).to_vec();
		let clients: i64 = sqlx_core::query_scalar::query_scalar("SELECT count(DISTINCT client_hash) FROM sessions WHERE tenant_id=$1 AND token_id=$2 AND NOT closed AND client_hash<>$3")
			.bind(tenant).bind(token_id).bind(&client_hash).fetch_one(&mut **tx).await?;
		if clients >= i64::from(token.get::<i32, _>("max_clients")) {
			return Ok(SessionResponse::error(Code::AccessLocked));
		}
		// Aynı cihaz yeniden bağlandığında önceki oturum yeni bir cihaz sayılmaz.
		sqlx_core::query::query("UPDATE sessions SET closed=true WHERE tenant_id=$1 AND token_id=$2 AND client_hash=$3 AND NOT closed")
			.bind(tenant).bind(token_id).bind(&client_hash).execute(&mut **tx).await?;
		// Oturum kimliği uygulama genelinde artar; tel sözleşmesindeki u64 aralığının güvenli alt kümesidir.
		let session_key = random::<16>();
		let key = self
			.vault
			.seal(&session_key, &format!("{tenant}:{server}:session"))?;
		let id: i64 = sqlx_core::query_scalar::query_scalar("INSERT INTO sessions(tenant_id,server_id,token_id,client_hash,session_key,protocol_version) VALUES($1,$2,$3,$4,$5,$6) RETURNING id")
			.bind(tenant).bind(server).bind(token_id).bind(client_hash).bind(key).bind(request.protocol_version).fetch_one(&mut **tx).await?;
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
		let token =
			sqlx_core::query::query("SELECT * FROM access_tokens WHERE tenant_id=$1 AND id=$2")
				.bind(tenant)
				.bind(token_id)
				.fetch_one(&mut **tx)
				.await?;
		let closed: bool = session.get("closed");
		let code = if closed {
			Code::SessionClosed
		} else {
			token_error(&token)
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
		let clients: i64 = sqlx_core::query_scalar::query_scalar("SELECT count(DISTINCT client_hash) FROM sessions WHERE tenant_id=$1 AND token_id=$2 AND NOT closed")
			.bind(tenant).bind(token_id).fetch_one(&mut **tx).await?;
		let traffic = Traffic {
			sent: token.get("sent"),
			received: token.get("received"),
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
				is_premium: true,
				cycle_traffic: traffic.clone(),
				total_traffic: traffic,
				max_traffic: token.get("max_traffic"),
				expiration_time: token.get("expires_at"),
				active_client_count: clients,
			}),
			server_tags: vec![],
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
		sqlx_core::query::query("SELECT id FROM access_tokens WHERE tenant_id=$1 AND server_id=$2 AND id IN (SELECT token_id FROM sessions WHERE tenant_id=$1 AND server_id=$2 AND id=ANY($3)) ORDER BY id FOR UPDATE")
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
					// Raporlar kümülatif sayaç değil fark (delta) içerir; kimliksiz raporlara tekrar önleme garantisi verilemez.
					sqlx_core::query::query(
						"UPDATE access_tokens SET sent=sent+$3,received=received+$4 WHERE tenant_id=$1 AND id=$2",
					)
					.bind(tenant)
					.bind(token)
					.bind(usage.sent)
					.bind(usage.received)
					.execute(&mut **tx)
					.await?;
					sqlx_core::query::query(
						"UPDATE sessions SET last_seen=now(),closed=closed OR $3 WHERE tenant_id=$1 AND id=$2",
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
fn token_error(token: &PgRow) -> Code {
	if token.get::<bool, _>("revoked") {
		return Code::AccessLocked;
	}
	if token
		.get::<Option<DateTime<Utc>>, _>("expires_at")
		.is_some_and(|e| e <= Utc::now())
	{
		return Code::AccessExpired;
	}
	let max = token.get::<i64, _>("max_traffic");
	let total =
		i128::from(token.get::<i64, _>("sent")) + i128::from(token.get::<i64, _>("received"));
	if max > 0 && total >= i128::from(max) {
		return Code::AccessTrafficOverflow;
	}
	Code::Ok
}
