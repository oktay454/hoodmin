use crate::{policy::AccessPolicy, store::Store};
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx_core::{query::query, row::Row, transaction::Transaction};
use sqlx_postgres::Postgres;
use uuid::Uuid;
use vpnhood_protocol::{ServerToken, SessionErrorCode as Code, SessionRequest, SessionResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerPolicy {
	pub enabled: bool,
	pub premium: bool,
	pub allow_auto_select: bool,
	pub power: f64,
	pub location: String,
	pub tags: Vec<String>,
	pub push_token_to_clients: bool,
	pub ga_measurement_id: Option<String>,
	pub max_normal_sessions: i64,
	pub max_premium_sessions: i64,
}
impl Default for ServerPolicy {
	fn default() -> Self {
		Self {
			enabled: true,
			premium: false,
			allow_auto_select: true,
			power: 0.0,
			location: "*/*".into(),
			tags: vec![],
			push_token_to_clients: false,
			ga_measurement_id: None,
			max_normal_sessions: 0,
			max_premium_sessions: 0,
		}
	}
}
impl ServerPolicy {
	pub fn validate(&self) -> Result<()> {
		ensure!(
			self.ga_measurement_id
				.as_ref()
				.is_none_or(|v| v.starts_with("G-")
					&& v.len() <= 50
					&& v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')),
			"Analytics kimliği geçersiz"
		);
		ensure!(
			self.power.is_finite() && (0.0..=1_000_000.0).contains(&self.power),
			"Power geçersiz"
		);
		ensure!(
			self.location.len() <= 120
				&& self.location.split('/').count() == 2
				&& !self.location.chars().any(char::is_control),
			"Konum CC/Region biçiminde olmalı"
		);
		ensure!(
			self.tags.len() <= 32
				&& self
					.tags
					.iter()
					.all(|s| s.len() <= 80 && !s.chars().any(char::is_control)),
			"Etiketler geçersiz"
		);
		ensure!(
			(0..=1_000_000).contains(&self.max_normal_sessions)
				&& (0..=1_000_000).contains(&self.max_premium_sessions),
			"Oturum sınırı geçersiz"
		);
		Ok(())
	}
}
pub fn location_matches(location: &str, request: Option<&str>) -> bool {
	let desired = request
		.unwrap_or("*/*")
		.split('[')
		.next()
		.unwrap_or("*/*")
		.trim();
	let a = location.split('/').collect::<Vec<_>>();
	let b = desired.split('/').collect::<Vec<_>>();
	(0..2).all(|i| {
		b.get(i).is_none_or(|v| {
			v.is_empty() || *v == "*" || a.get(i).is_some_and(|a| a.eq_ignore_ascii_case(v))
		})
	})
}
/// Redirect only after token proof has been checked. Returned certificates/secrets belong to this tenant.
#[allow(clippy::too_many_arguments)]
pub async fn route(
	store: &Store,
	tx: &mut Transaction<'_, Postgres>,
	tenant: Uuid,
	server: Uuid,
	seed: Uuid,
	farm: Option<Uuid>,
	premium: bool,
	request: &SessionRequest,
) -> Result<Option<SessionResponse>> {
	let rows=query("SELECT id,management,paused,last_seen,heartbeat_seconds,status,info,farm_id FROM servers WHERE tenant_id=$1 AND (id=$2 OR ($3::uuid IS NOT NULL AND farm_id=$3)) ORDER BY id")
		.bind(tenant).bind(server).bind(farm).fetch_all(&mut **tx).await?;
	let mut eligible = Vec::new();
	let explicit = request
		.server_location
		.as_deref()
		.is_some_and(|v| !location_matches("*/*", Some(v)));
	for row in rows {
		let id: Uuid = row.get("id");
		let member = if let Some(farm) = farm {
			row.get::<Option<Uuid>, _>("farm_id") == Some(farm)
		} else {
			id == seed
		};
		let policy: ServerPolicy = serde_json::from_value(row.get("management"))?;
		if !member
			|| row.get::<bool, _>("paused")
			|| !policy.enabled
			|| (policy.premium && !premium)
			|| (!explicit && !policy.allow_auto_select && id != server)
			|| !location_matches(&policy.location, request.server_location.as_deref())
		{
			continue;
		}
		if id != server
			&& row
				.get::<Option<DateTime<Utc>>, _>("last_seen")
				.is_none_or(|t| {
					t < Utc::now()
						- chrono::Duration::seconds(i64::from(
							row.get::<i32, _>("heartbeat_seconds"),
						))
				}) {
			continue;
		}
		let status: Option<Value> = row.get("status");
		if status
			.as_ref()
			.is_some_and(|s| s["configError"].as_str().is_some_and(|s| !s.is_empty()))
		{
			continue;
		}
		let count:i64=sqlx_core::query_scalar::query_scalar("SELECT count(*) FROM sessions WHERE tenant_id=$1 AND server_id=$2 AND premium=$3 AND NOT closed AND (expires_at IS NULL OR expires_at>now()) AND last_seen>now()-interval '48 hours'").bind(tenant).bind(id).bind(premium).fetch_one(&mut **tx).await?;
		let max = if premium {
			policy.max_premium_sessions
		} else {
			policy.max_normal_sessions
		};
		if max > 0 && count >= max {
			continue;
		}
		let info: Option<Value> = row.get("info");
		let power = if policy.power > 0.0 {
			policy.power
		} else {
			info.as_ref()
				.and_then(|v| v["logicalCoreCount"].as_f64())
				.unwrap_or(1.0)
				.max(1.0)
		};
		let load = status
			.as_ref()
			.and_then(|s| s["sessionCount"].as_f64())
			.unwrap_or(count as f64)
			.max(count as f64)
			/ power;
		eligible.push((load, id));
	}
	eligible.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
	if !request.allow_redirect {
		return Ok((!eligible.iter().any(|(_, id)| *id == server))
			.then(|| SessionResponse::error(Code::NoServerAvailable)));
	}
	let Some((_, selected)) = eligible.first() else {
		return Ok(Some(SessionResponse::error(Code::NoServerAvailable)));
	};
	if *selected == server {
		return Ok(None);
	}
	if request.protocol_version < 10 {
		return Ok(Some(SessionResponse::error(Code::UnsupportedClient)));
	}
	let bytes: Vec<u8> = sqlx_core::query_scalar::query_scalar(
		"SELECT configuration FROM servers WHERE tenant_id=$1 AND id=$2",
	)
	.bind(tenant)
	.bind(selected)
	.fetch_one(&mut **tx)
	.await?;
	let config: Value = serde_json::from_slice(
		&store
			.vault
			.open(&bytes, &format!("{tenant}:{selected}:config"))?,
	)?;
	let token: ServerToken = serde_json::from_value(config["publicServerToken"].clone())?;
	Ok(Some(SessionResponse {
		redirect_server_tokens: Some(vec![token]),
		..SessionResponse::error(Code::RedirectHost)
	}))
}

pub fn token_farm(policy: &AccessPolicy, farm: Option<Uuid>) -> Result<Option<Uuid>> {
	ensure!(
		!policy.farm_access || farm.is_some(),
		"Grup erişimi için sunucu bir gruba atanmalı"
	);
	Ok(if policy.farm_access { farm } else { None })
}
pub fn advertisement(config: &Value) -> Value {
	let mut token = config["publicServerToken"].clone();
	if let Some(object) = token.as_object_mut() {
		for key in ["ct", "sec", "ch", "url", "loc"] {
			object.remove(key);
		}
	}
	token
}
pub fn apply_advertisement(config: &mut Value, settings: &Value) -> Result<()> {
	let object = settings
		.as_object()
		.ok_or_else(|| anyhow::anyhow!("Duyuru JSON nesnesi olmalı"))?;
	ensure!(
		object.keys().all(|key| [
			"hname", "hport", "isv", "ep", "ep_st", "urls", "path", "loc2"
		]
		.contains(&key.as_str())),
		"Bilinmeyen duyuru alanı"
	);
	let mut token = config["publicServerToken"].clone();
	for key in ["urls", "path", "loc2"] {
		token.as_object_mut().expect("server token").remove(key);
	}
	for (name, value) in object {
		token[name] = value.clone();
	}
	let parsed: ServerToken = serde_json::from_value(token.clone())?;
	crate::management::normalize_host(&parsed.hname)?;
	ensure!(
		parsed.hport > 0 && parsed.ep_st <= 4,
		"Port veya uç seçim stratejisi geçersiz"
	);
	ensure!(
		parsed.ep.len() <= 256
			&& parsed.ep.iter().all(|s| s
				.parse::<std::net::SocketAddr>()
				.is_ok_and(|e| e.port() > 0)),
		"Genel uçlar IP:port olmalı"
	);
	ensure!(
		parsed.urls.as_ref().is_none_or(|urls| urls.len() <= 16
			&& urls.iter().all(|u| u
				.parse::<axum::http::Uri>()
				.is_ok_and(|v| v.scheme_str() == Some("https")
					&& v.authority().is_some_and(|a| !a.as_str().contains('@')))
				&& u.len() <= 2048
				&& !u.chars().any(char::is_control))),
		"URL HTTPS olmalı"
	);
	ensure!(
		parsed.path.as_ref().is_none_or(|p| p.starts_with('/')
			&& !p.starts_with("//")
			&& p.len() <= 1024
			&& p.bytes()
				.all(|b| b.is_ascii_alphanumeric() || b"/-_.~%".contains(&b))),
		"Yol geçersiz"
	);
	ensure!(
		parsed.loc2.as_ref().is_none_or(|v| v.len() <= 256
			&& v.iter()
				.all(|s| s.len() <= 256 && !s.chars().any(char::is_control))),
		"Konum duyurusu geçersiz"
	);
	token["ct"] = serde_json::json!(Utc::now());
	config["publicServerToken"] = token;
	Ok(())
}

/// Farm keys use a farm-specific URL; ordinary single-server keys retain their own URL.
pub async fn advertised_token(
	tx: &mut Transaction<'_, Postgres>,
	tenant: Uuid,
	server: Uuid,
	farm: Option<Uuid>,
	config: &Value,
) -> Result<ServerToken> {
	let mut token: ServerToken = serde_json::from_value(config["publicServerToken"].clone())?;
	if let Some(farm) = farm {
		let origin: Option<String> = sqlx_core::query_scalar::query_scalar(
			"SELECT publication_origin FROM farms WHERE tenant_id=$1 AND id=$2",
		)
		.bind(tenant)
		.bind(farm)
		.fetch_one(&mut **tx)
		.await?;
		if let Some(origin) = origin {
			let member: bool = sqlx_core::query_scalar::query_scalar(
				"SELECT EXISTS(SELECT FROM servers WHERE tenant_id=$1 AND id=$2 AND farm_id=$3)",
			)
			.bind(tenant)
			.bind(server)
			.bind(farm)
			.fetch_one(&mut **tx)
			.await?;
			ensure!(
				member,
				"Anahtarın ilk sunucusu artık grubunda değil; anahtarı yeniden oluşturun"
			);
			token.urls = Some(vec![format!("{origin}/f/{tenant}/{farm}/{server}")]);
			token.url = None;
		}
		let urls:Vec<String>=sqlx_core::query_scalar::query_scalar("SELECT publish_url FROM external_publications WHERE tenant_id=$1 AND farm_id=$2 AND seed_id=$3 AND enabled AND last_success IS NOT NULL ORDER BY id").bind(tenant).bind(farm).bind(server).fetch_all(&mut **tx).await?;
		if !urls.is_empty() {
			let current = token.urls.get_or_insert_with(Vec::new);
			for url in urls {
				if !current.contains(&url) {
					current.push(url)
				}
			}
		}
	}
	Ok(token)
}
/// Select a bootstrap endpoint; the authenticated session decision still enforces plan/location/capacity.
pub async fn publication_target(
	tx: &mut Transaction<'_, Postgres>,
	tenant: Uuid,
	farm: Uuid,
) -> Result<Option<Uuid>> {
	let rows=query("SELECT id,paused,management,last_seen,heartbeat_seconds,status FROM servers WHERE tenant_id=$1 AND farm_id=$2 ORDER BY id").bind(tenant).bind(farm).fetch_all(&mut **tx).await?;
	let mut candidates = Vec::new();
	for row in rows {
		let policy: ServerPolicy = serde_json::from_value(row.get("management"))?;
		if row.get::<bool, _>("paused")
			|| !policy.enabled
			|| !policy.allow_auto_select
			|| row
				.get::<Option<DateTime<Utc>>, _>("last_seen")
				.is_none_or(|t| {
					t < Utc::now()
						- chrono::Duration::seconds(i64::from(
							row.get::<i32, _>("heartbeat_seconds"),
						))
				}) {
			continue;
		}
		let status: Option<Value> = row.get("status");
		if status
			.as_ref()
			.is_some_and(|v| v["configError"].as_str().is_some_and(|s| !s.is_empty()))
		{
			continue;
		}
		let load = status
			.as_ref()
			.and_then(|v| v["sessionCount"].as_f64())
			.filter(|v| v.is_finite() && *v >= 0.0)
			.unwrap_or(0.0)
			/ policy.power.max(1.0);
		candidates.push((load, row.get::<Uuid, _>("id")));
	}
	candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
	Ok(candidates.first().map(|v| v.1))
}

pub async fn load_certificates(
	tx: &mut Transaction<'_, Postgres>,
	vault: &crate::crypto::Vault,
	tenant: Uuid,
	farm: Option<Uuid>,
) -> Result<Option<Value>> {
	let Some(farm) = farm else {
		return Ok(None);
	};
	let bytes: Option<Vec<u8>> = sqlx_core::query_scalar::query_scalar(
		"SELECT certificates FROM farms WHERE tenant_id=$1 AND id=$2 FOR SHARE",
	)
	.bind(tenant)
	.bind(farm)
	.fetch_one(&mut **tx)
	.await?;
	bytes
		.map(|b| {
			Ok(serde_json::from_slice(
				&vault.open(&b, &format!("{tenant}:{farm}:certificates"))?,
			)?)
		})
		.transpose()
}
pub fn apply_certificates(config: &mut Value, shared: &Value) -> Result<()> {
	let entries = shared
		.as_array()
		.ok_or_else(|| anyhow::anyhow!("Grup sertifika listesi geçersiz"))?;
	for entry in entries {
		crate::certificates::attach(
			config,
			entry["certificate"].clone(),
			entry["hash"]
				.as_str()
				.ok_or_else(|| anyhow::anyhow!("Sertifika parmak izi eksik"))?,
			entry["default"].as_bool().unwrap_or(false),
		)?;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn location_wildcards_do_not_grant_other_regions() {
		assert!(location_matches("TR/Istanbul", Some("TR/*")));
		assert!(location_matches("TR/Istanbul", None));
		assert!(!location_matches("DE/Frankfurt", Some("TR/*")));
	}
}
