use crate::store::Store;
use axum::{
	Json, Router,
	extract::{DefaultBodyLimit, Path, Query, State},
	http::{HeaderMap, StatusCode},
	response::{IntoResponse, Response},
	routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx_core::{row::Row, transaction::Transaction};
use sqlx_postgres::Postgres;
use subtle::ConstantTimeEq;
use uuid::Uuid;
use vpnhood_protocol::{ServerCommand, ServerStatus, SessionRequest, SessionUsage, Traffic};

pub fn router(store: Store) -> Router {
	Router::new()
		.route("/healthz", get(|| async { "ok" }))
		.route("/api/hooks/ad/{tenant}", post(reward_receipt))
		.route("/api/agent/configure", post(configure))
		.route("/api/agent/status", post(status))
		.route(
			"/api/agent/sessions",
			post(create_session).get(all_sessions),
		)
		.route("/api/agent/sessions/{id}", get(get_session))
		.route("/api/agent/sessions/{id}/usage", post(add_usage))
		.route("/api/agent/sessions/usages", post(add_usages))
		.route(
			"/api/agent/acme/http01_key_authorization",
			get(acme_authorization),
		)
		.layer(DefaultBodyLimit::max(512 * 1024))
		.with_state(store)
}

struct ApiError(StatusCode);
impl IntoResponse for ApiError {
	fn into_response(self) -> Response {
		(self.0, Json(json!({"error":"İstek tamamlanamadı."}))).into_response()
	}
}
impl From<anyhow::Error> for ApiError {
	fn from(error: anyhow::Error) -> Self {
		tracing::error!(%error, "Erişim yöneticisi isteği başarısız");
		Self(StatusCode::SERVICE_UNAVAILABLE)
	}
}
impl From<sqlx_core::error::Error> for ApiError {
	fn from(error: sqlx_core::error::Error) -> Self {
		anyhow::Error::from(error).into()
	}
}

async fn authenticate<'a>(
	store: &'a Store,
	headers: &HeaderMap,
) -> Result<(Uuid, Uuid, Transaction<'a, Postgres>), ApiError> {
	let credential = headers
		.get("authorization")
		.and_then(|h| h.to_str().ok())
		.and_then(|h| h.strip_prefix("Bearer "))
		.filter(|c| c.len() <= 200)
		.ok_or(ApiError(StatusCode::UNAUTHORIZED))?;
	let parts: Vec<&str> = credential.split('.').collect();
	if parts.len() != 3 {
		return Err(ApiError(StatusCode::UNAUTHORIZED));
	}
	let tenant = Uuid::parse_str(parts[0]).map_err(|_| ApiError(StatusCode::UNAUTHORIZED))?;
	let server = Uuid::parse_str(parts[1]).map_err(|_| ApiError(StatusCode::UNAUTHORIZED))?;
	let mut tx = store.begin(tenant).await?;
	let expected: Option<Vec<u8>> = sqlx_core::query_scalar::query_scalar(
		"SELECT credential_hash FROM servers WHERE tenant_id=$1 AND id=$2",
	)
	.bind(tenant)
	.bind(server)
	.fetch_optional(&mut *tx)
	.await?;
	let hash = Sha256::digest(credential.as_bytes());
	if !expected.is_some_and(|e| bool::from(e.ct_eq(hash.as_slice()))) {
		return Err(ApiError(StatusCode::UNAUTHORIZED));
	}
	Ok((tenant, server, tx))
}
async fn configure(
	State(store): State<Store>,
	headers: HeaderMap,
	Json(info): Json<Value>,
) -> Result<Json<Value>, ApiError> {
	if !info.is_object() {
		return Err(ApiError(StatusCode::BAD_REQUEST));
	}
	let (tenant, server, mut tx) = authenticate(&store, &headers).await?;
	let configuration: Vec<u8> = sqlx_core::query_scalar::query_scalar(
		"UPDATE servers SET info=$3,last_seen=now() WHERE tenant_id=$1 AND id=$2 RETURNING configuration",
	)
	.bind(tenant)
	.bind(server)
	.bind(info)
	.fetch_one(&mut *tx)
	.await?;
	let plaintext = store
		.vault
		.open(&configuration, &format!("{tenant}:{server}:config"))?;
	let mut config: Value = serde_json::from_slice(&plaintext).map_err(anyhow::Error::from)?;
	let interval =
		vpnhood_protocol::config::duration_seconds(&config["updateStatusInterval"]).unwrap_or(15.0);
	let heartbeat = (interval * 3.0).ceil().clamp(90.0, 259200.0) as i32;
	sqlx_core::query::query("UPDATE servers SET heartbeat_seconds=$3 WHERE tenant_id=$1 AND id=$2")
		.bind(tenant)
		.bind(server)
		.bind(heartbeat)
		.execute(&mut *tx)
		.await?;
	if let Some(object) = config.as_object_mut() {
		object.remove("publicServerToken");
	}
	tx.commit().await?;
	Ok(Json(config))
}
async fn status(
	State(store): State<Store>,
	headers: HeaderMap,
	Json(status): Json<ServerStatus>,
) -> Result<Json<ServerCommand>, ApiError> {
	validate_usages(&status.session_usages)?;
	let (tenant, server, mut tx) = authenticate(&store, &headers).await?;
	let details = Value::Object(status.details);
	let code: String = sqlx_core::query_scalar::query_scalar(
		"UPDATE servers SET status=CASE WHEN $3->>'configCode'=status->>'configCode' AND ($3->'endPointStatuses' IS NULL OR $3->'endPointStatuses'='null'::jsonb) THEN ($3-'endPointStatuses')||jsonb_build_object('endPointStatuses',status->'endPointStatuses') ELSE $3 END,last_seen=now() WHERE tenant_id=$1 AND id=$2 RETURNING config_code",
	)
	.bind(tenant)
	.bind(server)
	.bind(&details)
	.fetch_one(&mut *tx)
	.await?;
	// Keep bounded, once-per-minute telemetry without session payloads or credentials.
	let sample = [
		"sessionCount",
		"tcpConnectionCount",
		"udpConnectionCount",
		"cpuUsage",
		"availableMemory",
		"usedMemory",
		"threadCount",
		"tunnelSpeed",
	]
	.into_iter()
	.filter_map(|key| details.get(key).map(|v| (key.to_owned(), v.clone())))
	.collect::<serde_json::Map<_, _>>();
	sqlx_core::query::query("INSERT INTO server_samples(tenant_id,server_id,status) SELECT $1,$2,$3 WHERE NOT EXISTS(SELECT FROM server_samples WHERE tenant_id=$1 AND server_id=$2 AND created_at>now()-interval '1 minute')").bind(tenant).bind(server).bind(Value::Object(sample)).execute(&mut *tx).await?;
	sqlx_core::query::query("DELETE FROM server_samples WHERE tenant_id=$1 AND server_id=$2 AND created_at<now()-interval '31 days'").bind(tenant).bind(server).execute(&mut *tx).await?;
	let ids = status
		.session_usages
		.iter()
		.filter_map(|u| i64::try_from(u.session_id).ok())
		.collect::<Vec<_>>();
	sqlx_core::query::query("SELECT id FROM access_tokens WHERE tenant_id=$1 AND id IN (SELECT token_id FROM sessions WHERE tenant_id=$1 AND server_id=$2 AND (id=ANY($3) OR pending_notify)) ORDER BY id FOR UPDATE").bind(tenant).bind(server).bind(ids).fetch_all(&mut *tx).await?;
	let mut responses = store
		.usages(&mut tx, tenant, server, status.session_usages)
		.await?;
	let pending=sqlx_core::query::query("SELECT id FROM sessions WHERE tenant_id=$1 AND server_id=$2 AND pending_notify ORDER BY id LIMIT 1000").bind(tenant).bind(server).fetch_all(&mut *tx).await?;
	for row in pending {
		let id = row.get::<i64, _>("id") as u64;
		responses.insert(id, store.get(&mut tx, tenant, server, id).await?);
	}
	tx.commit().await?;
	Ok(Json(ServerCommand {
		config_code: code,
		session_responses: responses,
	}))
}
async fn create_session(
	State(store): State<Store>,
	headers: HeaderMap,
	Json(request): Json<SessionRequest>,
) -> Result<Json<vpnhood_protocol::SessionResponse>, ApiError> {
	let (tenant, server, mut tx) = authenticate(&store, &headers).await?;
	let result = store.create(&mut tx, tenant, server, request).await?;
	tx.commit().await?;
	Ok(Json(result))
}
async fn get_session(
	State(store): State<Store>,
	headers: HeaderMap,
	Path(id): Path<u64>,
) -> Result<Json<vpnhood_protocol::SessionResponse>, ApiError> {
	let (tenant, server, mut tx) = authenticate(&store, &headers).await?;
	let result = store.get(&mut tx, tenant, server, id).await?;
	tx.commit().await?;
	Ok(Json(result))
}
async fn all_sessions(
	State(store): State<Store>,
	headers: HeaderMap,
) -> Result<Json<Vec<vpnhood_protocol::SessionResponse>>, ApiError> {
	let (tenant, server, mut tx) = authenticate(&store, &headers).await?;
	let ids = sqlx_core::query::query("SELECT id FROM sessions WHERE tenant_id=$1 AND server_id=$2 AND NOT closed ORDER BY id LIMIT 10000")
		.bind(tenant).bind(server).fetch_all(&mut *tx).await?;
	let ids = ids
		.iter()
		.map(|r| r.get::<i64, _>("id"))
		.collect::<Vec<_>>();
	sqlx_core::query::query("SELECT id FROM access_tokens WHERE tenant_id=$1 AND id IN (SELECT token_id FROM sessions WHERE tenant_id=$1 AND server_id=$2 AND id=ANY($3)) ORDER BY id FOR UPDATE").bind(tenant).bind(server).bind(&ids).fetch_all(&mut *tx).await?;
	let mut responses = Vec::new();
	for id in ids {
		responses.push(store.get(&mut tx, tenant, server, id as u64).await?);
	}
	tx.commit().await?;
	Ok(Json(responses))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageQuery {
	#[serde(default)]
	ad_data: Option<String>,
	#[serde(default)]
	close_session: bool,
}
async fn add_usage(
	State(store): State<Store>,
	headers: HeaderMap,
	Path(id): Path<u64>,
	Query(query): Query<UsageQuery>,
	Json(traffic): Json<Traffic>,
) -> Result<Json<vpnhood_protocol::SessionResponse>, ApiError> {
	let usages = vec![SessionUsage {
		session_id: id,
		sent: traffic.sent,
		received: traffic.received,
		closed: query.close_session,
		ad_data: query.ad_data,
		..Default::default()
	}];
	validate_usages(&usages)?;
	let (tenant, server, mut tx) = authenticate(&store, &headers).await?;
	let mut results = store.usages(&mut tx, tenant, server, usages).await?;
	tx.commit().await?;
	Ok(Json(
		results
			.remove(&id)
			.ok_or(ApiError(StatusCode::INTERNAL_SERVER_ERROR))?,
	))
}
async fn add_usages(
	State(store): State<Store>,
	headers: HeaderMap,
	Json(usages): Json<Vec<SessionUsage>>,
) -> Result<Json<std::collections::BTreeMap<u64, vpnhood_protocol::SessionResponse>>, ApiError> {
	validate_usages(&usages)?;
	let (tenant, server, mut tx) = authenticate(&store, &headers).await?;
	let results = store.usages(&mut tx, tenant, server, usages).await?;
	tx.commit().await?;
	Ok(Json(results))
}
fn validate_usages(usages: &[SessionUsage]) -> Result<(), ApiError> {
	if usages.len() > 1000 || usages.iter().any(|u| u.sent < 0 || u.received < 0) {
		return Err(ApiError(StatusCode::BAD_REQUEST));
	}
	Ok(())
}

#[derive(Deserialize)]
struct AcmeQuery {
	token: String,
}
async fn acme_authorization(
	State(store): State<Store>,
	headers: HeaderMap,
	Query(query): Query<AcmeQuery>,
) -> Result<Json<String>, ApiError> {
	let (tenant, server, mut tx) = authenticate(&store, &headers).await?;
	if query.token.len() > 256 {
		return Err(ApiError(StatusCode::BAD_REQUEST));
	}
	let proof: Option<String> = sqlx_core::query_scalar::query_scalar("SELECT key_authorization FROM acme_challenges WHERE tenant_id=$1 AND server_id=$2 AND token=$3 AND expires_at>now()")
		.bind(tenant).bind(server).bind(query.token).fetch_optional(&mut *tx).await?;
	Ok(Json(proof.ok_or(ApiError(StatusCode::NOT_FOUND))?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewardReceipt {
	session_id: u64,
	ad_data: String,
}
async fn reward_receipt(
	State(store): State<Store>,
	headers: HeaderMap,
	Path(tenant): Path<Uuid>,
	Json(receipt): Json<RewardReceipt>,
) -> Result<StatusCode, ApiError> {
	let credential = headers
		.get("authorization")
		.and_then(|h| h.to_str().ok())
		.and_then(|v| v.strip_prefix("Bearer "))
		.filter(|v| v.len() <= 256)
		.ok_or(ApiError(StatusCode::UNAUTHORIZED))?;
	let mut tx = store.begin(tenant).await?;
	let expected: Option<Vec<u8>> = sqlx_core::query_scalar::query_scalar(
		"SELECT credential_hash FROM ad_hooks WHERE tenant_id=$1",
	)
	.bind(tenant)
	.fetch_optional(&mut *tx)
	.await?;
	if !expected
		.is_some_and(|h| bool::from(h.ct_eq(Sha256::digest(credential.as_bytes()).as_slice())))
	{
		return Err(ApiError(StatusCode::UNAUTHORIZED));
	}
	let id = i64::try_from(receipt.session_id).map_err(|_| ApiError(StatusCode::BAD_REQUEST))?;
	if !valid_ad_data(id, &receipt.ad_data) {
		return Err(ApiError(StatusCode::BAD_REQUEST));
	}
	let exists:bool=sqlx_core::query_scalar::query_scalar("SELECT EXISTS(SELECT FROM sessions WHERE tenant_id=$1 AND id=$2 AND NOT closed AND created_at>now()-interval '1 day')").bind(tenant).bind(id).fetch_one(&mut *tx).await?;
	if !exists {
		return Err(ApiError(StatusCode::NOT_FOUND));
	}
	sqlx_core::query::query("DELETE FROM ad_rewards WHERE tenant_id=$1 AND expires_at<=now()")
		.bind(tenant)
		.execute(&mut *tx)
		.await?;
	sqlx_core::query::query("INSERT INTO ad_rewards(tenant_id,session_id,ad_hash) VALUES($1,$2,$3) ON CONFLICT(tenant_id,ad_hash) DO NOTHING").bind(tenant).bind(id).bind(Sha256::digest(receipt.ad_data.as_bytes()).to_vec()).execute(&mut *tx).await?;
	tx.commit().await?;
	Ok(StatusCode::NO_CONTENT)
}
pub fn valid_ad_data(session: i64, data: &str) -> bool {
	let prefix = format!("sid:{session};ad:");
	data.strip_prefix(&prefix)
		.is_some_and(|v| Uuid::parse_str(v).is_ok())
}
