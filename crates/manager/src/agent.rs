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
			get(|| async { StatusCode::NOT_FOUND }),
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
	let code: String = sqlx_core::query_scalar::query_scalar(
		"UPDATE servers SET status=$3,last_seen=now() WHERE tenant_id=$1 AND id=$2 RETURNING config_code",
	)
	.bind(tenant)
	.bind(server)
	.bind(Value::Object(status.details))
	.fetch_one(&mut *tx)
	.await?;
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
	let mut responses = Vec::new();
	for row in ids {
		responses.push(
			store
				.get(&mut tx, tenant, server, row.get::<i64, _>("id") as u64)
				.await?,
		);
	}
	tx.commit().await?;
	Ok(Json(responses))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageQuery {
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
