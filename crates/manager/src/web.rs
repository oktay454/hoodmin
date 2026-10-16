use crate::{
	auth::{self, User},
	i18n, management,
	store::Store,
	views,
};
use axum::{
	Form, Router,
	extract::{DefaultBodyLimit, Path, Query, State},
	http::{HeaderMap, StatusCode, header},
	middleware::{self, Next},
	response::{
		IntoResponse, Redirect, Response, Sse,
		sse::{Event, KeepAlive},
	},
	routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, query_scalar::query_scalar, row::Row};
use std::{convert::Infallible, time::Duration};
use uuid::Uuid;

#[derive(Clone)]
pub struct WebState {
	pub store: Store,
	pub origin: String,
	pub dummy_hash: String,
}
impl WebState {
	pub fn new(store: Store, origin: String) -> anyhow::Result<Self> {
		let origin = origin.trim_end_matches('/').to_owned();
		anyhow::ensure!(origin.starts_with("https://"), "Panel adresi HTTPS olmalı.");
		Ok(Self {
			store,
			origin,
			dummy_hash: auth::hash_password(&auth::nonce())?,
		})
	}
}
pub fn router(state: WebState) -> Router {
	Router::new()
		.route("/", get(|| async { Redirect::to("/panel") }))
		.route("/signin", get(signin).post(login))
		.route("/language", get(i18n::change))
		.route("/theme", get(i18n::change_theme))
		.route("/logout", post(logout))
		.route("/panel", get(dashboard))
		.route("/panel/{page}", get(page))
		.route("/manage/bots", get(bots).post(create_bot))
		.route("/manage/bots/{id}/revoke", post(revoke_bot))
		.route("/manage/farms", post(add_farm))
		.route("/manage/servers", post(add_server))
		.route("/manage/servers/{id}", post(edit_server))
		.route(
			"/manage/servers/{id}/configuration",
			get(server_configuration).post(save_configuration),
		)
		.route(
			"/manage/servers/{id}/credential",
			get(confirm_credential).post(rotate_credential),
		)
		.route("/reports", get(reports))
		.route(
			"/manage/server-profiles",
			get(server_profiles).post(add_server_profile),
		)
		.route("/manage/server-profiles/{id}", post(edit_server_profile))
		.route(
			"/manage/server-profiles/{id}/apply",
			post(apply_server_profile),
		)
		.route("/manage/profiles", get(profiles).post(add_profile))
		.route("/manage/profiles/{id}", post(edit_profile))
		.route(
			"/manage/tokens/{id}/policy",
			get(token_policy).post(save_token_policy),
		)
		.route("/manage/tokens/{id}/key", get(token_key))
		.route("/manage/tokens/{id}/download", get(download_token))
		.route("/manage/tokens/{id}/details", post(save_token_details))
		.route("/s/{tenant}/{server}", get(published_server_token))
		.route("/manage/servers/{id}/publish", post(publish_token))
		.route(
			"/manage/servers/{id}/certificates",
			get(server_certificates).post(upload_certificate),
		)
		.route("/manage/servers/{id}/challenge", post(set_challenge))
		.route(
			"/manage/servers/{id}/routing",
			get(server_routing).post(save_routing),
		)
		.route("/manage/tokens/{id}/access-code", post(create_access_code))
		.route(
			"/manage/integrations",
			get(integrations).post(configure_ad_hook),
		)
		.route(
			"/manage/servers/{id}/installation",
			get(installation).post(start_installation),
		)
		.route("/manage/install-jobs/{id}", get(install_status))
		.route("/manage/tokens", post(add_tokens))
		.route("/manage/tokens/bulk", post(bulk_tokens))
		.route("/manage/sessions/{id}/close", post(close_session))
		.route("/manage/users", post(add_user))
		.route("/manage/users/{id}", post(edit_user))
		.route("/manage/password", post(change_password))
		.route("/manage/tenant", post(update_tenant))
		.route("/export/usage.csv", get(export_csv))
		.route("/events", get(events))
		.route(
			"/assets/app.css",
			get(|| async {
				(
					[(header::CONTENT_TYPE, "text/css; charset=utf-8")],
					include_str!("../assets/app.css"),
				)
			}),
		)
		.route(
			"/assets/app.js",
			get(|| async {
				(
					[(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
					include_str!("../assets/app.js"),
				)
			}),
		)
		.nest("/api/manage", management_api())
		.layer(DefaultBodyLimit::max(512 * 1024))
		.layer(middleware::from_fn(security))
		.layer(middleware::from_fn(i18n::middleware))
		.with_state(state)
}
async fn security(request: axum::extract::Request, next: Next) -> Response {
	let mut response = next.run(request).await;
	for (name, value) in [
		("cache-control", "no-store"),
		("x-content-type-options", "nosniff"),
		("referrer-policy", "same-origin"),
		(
			"content-security-policy",
			"default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
		),
		(
			"permissions-policy",
			"camera=(), microphone=(), geolocation=()",
		),
	] {
		response.headers_mut().insert(
			axum::http::HeaderName::from_static(name),
			axum::http::HeaderValue::from_static(value),
		);
	}
	response
}
pub struct PanelError(pub StatusCode, pub &'static str);
impl IntoResponse for PanelError {
	fn into_response(self) -> Response {
		if self.0 == StatusCode::UNAUTHORIZED {
			return Redirect::to("/signin").into_response();
		}
		(self.0, views::error(self.1)).into_response()
	}
}
impl From<anyhow::Error> for PanelError {
	fn from(e: anyhow::Error) -> Self {
		tracing::error!(%e,"Panel işlemi başarısız");
		Self(
			StatusCode::SERVICE_UNAVAILABLE,
			"İşlem tamamlanamadı. Bilgileri kontrol edip yeniden deneyin.",
		)
	}
}
impl From<sqlx_core::error::Error> for PanelError {
	fn from(e: sqlx_core::error::Error) -> Self {
		anyhow::Error::from(e).into()
	}
}
fn secret_response(user: &User, title: &str, description: &str, value: &str) -> Response {
	if user.bot {
		axum::Json(json!({"value":value})).into_response()
	} else {
		views::secret(user, title, description, value).into_response()
	}
}
fn bad(message: &'static str) -> PanelError {
	PanelError(StatusCode::BAD_REQUEST, message)
}
fn check_origin(state: &WebState, headers: &HeaderMap) -> Result<(), PanelError> {
	if headers.get("origin").and_then(|h| h.to_str().ok()) != Some(state.origin.as_str()) {
		return Err(PanelError(
			StatusCode::FORBIDDEN,
			"İsteğin kaynağı doğrulanamadı.",
		));
	}
	Ok(())
}
#[derive(Clone, Copy)]
enum Permission {
	Read,
	Tokens,
	Write,
	Admin,
}
async fn require(
	state: &WebState,
	headers: &HeaderMap,
	csrf: Option<&str>,
	permission: Permission,
) -> Result<User, PanelError> {
	let user = auth::user(&state.store, headers)
		.await?
		.ok_or(PanelError(StatusCode::UNAUTHORIZED, "Oturum gerekli."))?;
	if let Some(csrf) = csrf.filter(|_| !user.bot) {
		check_origin(state, headers)?;
		if !auth::same_token(csrf, &user.csrf) {
			return Err(PanelError(
				StatusCode::FORBIDDEN,
				"Form süresi doldu. Sayfayı yenileyin.",
			));
		}
	}
	if matches!(permission, Permission::Tokens) && !user.can_tokens()
		|| matches!(permission, Permission::Write) && !user.can_write()
		|| matches!(permission, Permission::Admin) && !user.is_admin()
	{
		return Err(PanelError(
			StatusCode::FORBIDDEN,
			"Bu işlem için yetkiniz yok.",
		));
	}
	Ok(user)
}
async fn signin(State(state): State<WebState>, headers: HeaderMap) -> Result<Response, PanelError> {
	if auth::user(&state.store, &headers).await?.is_some() {
		return Ok(Redirect::to("/panel").into_response());
	}
	let csrf = auth::nonce();
	Ok((
		[(
			header::SET_COOKIE,
			auth::cookie_header(auth::LOGIN_COOKIE, &csrf, 600),
		)],
		views::signin(&csrf, None),
	)
		.into_response())
}
#[derive(Deserialize)]
struct LoginForm {
	email: String,
	password: String,
	#[serde(default)]
	csrf: String,
}
async fn login(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<LoginForm>,
) -> Result<Response, PanelError> {
	check_origin(&state, &headers)?;
	let cookie = auth::cookie(&headers, auth::LOGIN_COOKIE).ok_or(PanelError(
		StatusCode::FORBIDDEN,
		"Giriş formunu yenileyin.",
	))?;
	if !auth::same_token(&cookie, &form.csrf) {
		return Err(PanelError(
			StatusCode::FORBIDDEN,
			"Giriş formunu yenileyin.",
		));
	}
	let address = form.email.trim().to_ascii_lowercase();
	if address.len() > 254 || form.password.len() > 128 {
		return Err(bad("Giriş bilgileri geçersiz."));
	}
	let peer = headers
		.get("x-real-ip")
		.and_then(|v| v.to_str().ok())
		.filter(|v| v.parse::<std::net::IpAddr>().is_ok())
		.unwrap_or("local");
	let peer_key = Sha256::digest(format!("ip:{peer}").as_bytes()).to_vec();
	let peer_count:i32=query_scalar("INSERT INTO login_attempts(key,failures,window_start) VALUES($1,1,now()) ON CONFLICT(key) DO UPDATE SET failures=CASE WHEN login_attempts.window_start<now()-interval '10 minutes' THEN 1 ELSE login_attempts.failures+1 END,window_start=CASE WHEN login_attempts.window_start<now()-interval '10 minutes' THEN now() ELSE login_attempts.window_start END RETURNING failures").bind(peer_key).fetch_one(&state.store.pool).await?;
	if peer_count > 100 {
		return Err(PanelError(
			StatusCode::TOO_MANY_REQUESTS,
			"Çok sayıda giriş denemesi. 10 dakika sonra yeniden deneyin.",
		));
	}
	let attempt_key = Sha256::digest(format!("email:{address}").as_bytes()).to_vec();
	query("DELETE FROM login_attempts WHERE window_start<now()-interval '1 day'")
		.execute(&state.store.pool)
		.await?;
	let count:i32=query_scalar("INSERT INTO login_attempts(key,failures,window_start) VALUES($1,1,now()) ON CONFLICT(key) DO UPDATE SET failures=CASE WHEN login_attempts.window_start<now()-interval '10 minutes' THEN 1 ELSE login_attempts.failures+1 END,window_start=CASE WHEN login_attempts.window_start<now()-interval '10 minutes' THEN now() ELSE login_attempts.window_start END RETURNING failures")
		.bind(&attempt_key).fetch_one(&state.store.pool).await?;
	if count > 20 {
		return Err(PanelError(
			StatusCode::TOO_MANY_REQUESTS,
			"Çok sayıda giriş denemesi. 10 dakika sonra yeniden deneyin.",
		));
	}
	let account =
		query("SELECT id,default_tenant_id,password_hash,active FROM accounts WHERE email=$1")
			.bind(&address)
			.fetch_optional(&state.store.pool)
			.await?;
	let hash = account
		.as_ref()
		.map(|a| a.get::<String, _>("password_hash"))
		.unwrap_or(state.dummy_hash.clone());
	let verified = auth::verify_password(form.password, hash).await;
	if !verified || !account.as_ref().is_some_and(|a| a.get::<bool, _>("active")) {
		return Ok((
			StatusCode::UNAUTHORIZED,
			views::signin(&form.csrf, Some("E-posta veya parola hatalı.")),
		)
			.into_response());
	}
	let account = account.ok_or(bad("Giriş bilgileri geçersiz."))?;
	let id: Uuid = account.get("id");
	let tenant: Uuid = account.get("default_tenant_id");
	let token = auth::nonce();
	let csrf = auth::nonce();
	let mut tx = state.store.begin(tenant).await?;
	let role: String =
		query_scalar("SELECT role FROM memberships WHERE tenant_id=$1 AND account_id=$2")
			.bind(tenant)
			.bind(id)
			.fetch_one(&mut *tx)
			.await?;
	query("DELETE FROM web_sessions WHERE expires_at<=now()")
		.execute(&mut *tx)
		.await?;
	query("INSERT INTO web_sessions(token_hash,tenant_id,account_id,csrf,expires_at) VALUES($1,$2,$3,$4,now()+interval '8 hours')")
		.bind(Sha256::digest(token.as_bytes()).to_vec()).bind(tenant).bind(id).bind(&csrf).execute(&mut *tx).await?;
	query("DELETE FROM login_attempts WHERE key=$1")
		.bind(attempt_key)
		.execute(&mut *tx)
		.await?;
	auth::audit(
		&mut tx,
		&User {
			id,
			tenant,
			email: address,
			tenant_name: String::new(),
			role,
			csrf,
			bot: false,
		},
		"Oturum açıldı",
		"panel",
	)
	.await?;
	tx.commit().await?;
	let mut response = Redirect::to("/panel").into_response();
	response.headers_mut().append(
		header::SET_COOKIE,
		auth::cookie_header(auth::COOKIE, &token, 28800)
			.parse()
			.map_err(|_| bad("Oturum oluşturulamadı."))?,
	);
	response.headers_mut().append(
		header::SET_COOKIE,
		auth::cookie_header(auth::LOGIN_COOKIE, "", 0)
			.parse()
			.map_err(|_| bad("Oturum oluşturulamadı."))?,
	);
	Ok(response)
}
#[derive(Deserialize)]
struct CsrfForm {
	#[serde(default)]
	csrf: String,
}
async fn logout(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<CsrfForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Read).await?;
	let token = auth::cookie(&headers, auth::COOKIE).ok_or(bad("Oturum gerekli."))?;
	let mut tx = state.store.begin(user.tenant).await?;
	query("DELETE FROM web_sessions WHERE token_hash=$1")
		.bind(Sha256::digest(token.as_bytes()).to_vec())
		.execute(&mut *tx)
		.await?;
	auth::audit(&mut tx, &user, "Oturum kapatıldı", "panel").await?;
	tx.commit().await?;
	Ok((
		[(header::SET_COOKIE, auth::cookie_header(auth::COOKIE, "", 0))],
		Redirect::to("/signin"),
	)
		.into_response())
}
#[derive(Default, Deserialize)]
pub struct Filter {
	#[serde(default)]
	pub q: String,
	#[serde(default)]
	pub notice: String,
	#[serde(default = "first_page")]
	pub p: usize,
}
fn first_page() -> usize {
	1
}
async fn dashboard(
	State(state): State<WebState>,
	headers: HeaderMap,
	Query(filter): Query<Filter>,
) -> Result<Response, PanelError> {
	render(&state, &headers, "overview", filter).await
}
async fn page(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(page): Path<String>,
	Query(filter): Query<Filter>,
) -> Result<Response, PanelError> {
	if !["servers", "tokens", "sessions", "users", "audit", "account"].contains(&page.as_str()) {
		return Err(PanelError(StatusCode::NOT_FOUND, "Sayfa bulunamadı."));
	}
	render(&state, &headers, &page, filter).await
}
async fn render(
	state: &WebState,
	headers: &HeaderMap,
	page: &str,
	filter: Filter,
) -> Result<Response, PanelError> {
	let user = require(state, headers, None, Permission::Read).await?;
	if page == "users" && !user.is_admin() {
		return Err(PanelError(
			StatusCode::FORBIDDEN,
			"Kullanıcı yönetimi için yönetici rolü gerekli.",
		));
	}
	if filter.q.len() > 120 || filter.p == 0 || filter.p > 1_000_000 {
		return Err(bad("Arama veya sayfa numarası geçersiz."));
	}
	let data = views::load(&state.store, &user, &filter.q, filter.p, page).await?;
	Ok(views::panel(page, &user, data, &filter.notice).into_response())
}
#[derive(Deserialize)]
struct FarmForm {
	#[serde(default)]
	csrf: String,
	name: String,
}
async fn add_farm(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<FarmForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	let name =
		management::valid_name(&form.name).map_err(|_| bad("Grup adı 1–120 bayt olmalı."))?;
	let mut tx = state.store.begin(user.tenant).await?;
	let id = Uuid::new_v4();
	query("INSERT INTO farms(tenant_id,id,name) VALUES($1,$2,$3)")
		.bind(user.tenant)
		.bind(id)
		.bind(name)
		.execute(&mut *tx)
		.await?;
	auth::audit(&mut tx, &user, "Sunucu grubu oluşturuldu", &id.to_string()).await?;
	tx.commit().await?;
	Ok(Redirect::to("/panel/servers?notice=saved").into_response())
}
#[derive(Deserialize)]
struct ServerForm {
	#[serde(default)]
	csrf: String,
	name: String,
	host: String,
	port: u16,
	#[serde(default)]
	farm: String,
}
fn optional_uuid(value: &str) -> Result<Option<Uuid>, PanelError> {
	if value.is_empty() {
		Ok(None)
	} else {
		Uuid::parse_str(value)
			.map(Some)
			.map_err(|_| bad("Grup seçimi geçersiz."))
	}
}
async fn add_server(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<ServerForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	let name = management::valid_name(&form.name).map_err(|_| bad("Sunucu adı geçersiz."))?;
	let farm = optional_uuid(&form.farm)?;
	let config =
		tokio::task::spawn_blocking(move || management::make_config(&form.host, form.port))
			.await
			.map_err(|_| bad("Sertifika oluşturulamadı."))?
			.map_err(|_| bad("DNS/IP veya port geçersiz."))?;
	let mut tx = state.store.begin(user.tenant).await?;
	if let Some(farm) = farm {
		let owned: bool =
			query_scalar("SELECT EXISTS(SELECT FROM farms WHERE tenant_id=$1 AND id=$2)")
				.bind(user.tenant)
				.bind(farm)
				.fetch_one(&mut *tx)
				.await?;
		if !owned {
			return Err(PanelError(
				StatusCode::NOT_FOUND,
				"Sunucu grubu bulunamadı.",
			));
		}
	}
	let (id, credential) = management::register(
		&mut tx,
		&state.store.vault,
		user.tenant,
		&name,
		farm,
		config,
	)
	.await?;
	auth::audit(&mut tx, &user, "Sunucu kaydedildi", &id.to_string()).await?;
	tx.commit().await?;
	let settings =
		serde_json::to_string_pretty(&crate::install::settings(&state.origin, &credential))
			.map_err(anyhow::Error::from)?;
	Ok(if user.bot {
		axum::Json(json!({"id":id,"settings":serde_json::from_str::<serde_json::Value>(&settings).map_err(anyhow::Error::from)?})).into_response()
	} else {
		views::installation(&user, id, &settings).into_response()
	})
}
fn rotation_confirmation(hash: &[u8], csrf: &str) -> String {
	let mut digest = Sha256::new();
	digest.update(b"vhm-confirm-rotation-v1:");
	digest.update(hash);
	digest.update(csrf.as_bytes());
	URL_SAFE_NO_PAD.encode(digest.finalize())
}
async fn confirm_credential(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Write).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let row = query("SELECT name,credential_hash FROM servers WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.fetch_optional(&mut *tx)
		.await?
		.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	let proof = rotation_confirmation(&row.get::<Vec<u8>, _>("credential_hash"), &user.csrf);
	Ok(if user.bot {
		axum::Json(json!({"confirmation":proof})).into_response()
	} else {
		views::confirm_credential(&user, id, &row.get::<String, _>("name"), &proof).into_response()
	})
}
#[derive(Deserialize)]
struct RotateCredentialForm {
	#[serde(default)]
	csrf: String,
	#[serde(default)]
	confirmed: bool,
	#[serde(default)]
	confirmation: String,
}
async fn rotate_credential(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<RotateCredentialForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	if !form.confirmed {
		return Err(bad(
			"Kimliği yenilemek için etkilerini okuyup onay kutusunu işaretleyin.",
		));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	let hash: Vec<u8> =
		query_scalar("SELECT credential_hash FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
			.bind(user.tenant)
			.bind(id)
			.fetch_optional(&mut *tx)
			.await?
			.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	if !auth::same_token(
		&form.confirmation,
		&rotation_confirmation(&hash, &user.csrf),
	) {
		return Err(PanelError(
			StatusCode::CONFLICT,
			"Onay formu artık geçerli değil. Sunucu sayfasından yeniden açın.",
		));
	}
	let credential = format!("{}.{id}.{}", user.tenant, auth::nonce());
	query("UPDATE servers SET credential_hash=$3,agent_credential=$4 WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.bind(Sha256::digest(credential.as_bytes()).to_vec())
		.bind(state.store.vault.seal(
			credential.as_bytes(),
			&format!("{}:{id}:agent", user.tenant),
		)?)
		.execute(&mut *tx)
		.await?;
	auth::audit(
		&mut tx,
		&user,
		"Sunucu bağlantı kimliği yenilendi",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	let settings =
		serde_json::to_string_pretty(&crate::install::settings(&state.origin, &credential))
			.map_err(anyhow::Error::from)?;
	Ok(secret_response(
		&user,
		"Yeni sunucu bağlantı ayarı",
		"Önceki kimlik geçersizleştirildi. Aşağıdaki ayarı sunucunuza uygulayın.",
		&settings,
	)
	.into_response())
}

#[derive(Deserialize)]
struct EditServerForm {
	#[serde(default)]
	csrf: String,
	name: String,
	host: Option<String>,
	port: Option<u16>,
	#[serde(default)]
	farm: String,
	#[serde(default)]
	paused: bool,
}
async fn edit_server(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<EditServerForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	let name = management::valid_name(&form.name).map_err(|_| bad("Sunucu adı geçersiz."))?;
	let mut tx = state.store.begin(user.tenant).await?;
	let encrypted: Vec<u8> =
		query_scalar("SELECT configuration FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
			.bind(user.tenant)
			.bind(id)
			.fetch_optional(&mut *tx)
			.await?
			.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	let context = format!("{}:{id}:config", user.tenant);
	let mut config: serde_json::Value =
		serde_json::from_slice(&state.store.vault.open(&encrypted, &context)?)
			.map_err(anyhow::Error::from)?;
	let (old_host, old_port) = management::endpoint(&config)?;
	let endpoint_changed = management::update_endpoint(
		&mut config,
		form.host.as_deref().unwrap_or(&old_host),
		form.port.unwrap_or(old_port),
	)
	.map_err(|_| bad("DNS/IP veya port geçersiz."))?;
	if endpoint_changed {
		let code = Uuid::new_v4().to_string();
		config["configCode"] = code.clone().into();
		let sealed = state.store.vault.seal(
			&serde_json::to_vec(&config).map_err(anyhow::Error::from)?,
			&context,
		)?;
		query("UPDATE servers SET configuration=$3,config_code=$4 WHERE tenant_id=$1 AND id=$2")
			.bind(user.tenant)
			.bind(id)
			.bind(sealed)
			.bind(code)
			.execute(&mut *tx)
			.await?;
	}
	if let Some(farm) = optional_uuid(&form.farm)? {
		let owned: bool =
			query_scalar("SELECT EXISTS(SELECT FROM farms WHERE tenant_id=$1 AND id=$2)")
				.bind(user.tenant)
				.bind(farm)
				.fetch_one(&mut *tx)
				.await?;
		if !owned {
			return Err(PanelError(
				StatusCode::NOT_FOUND,
				"Sunucu grubu bulunamadı.",
			));
		}
	}
	let affected =
		query("UPDATE servers SET name=$3,farm_id=$4,paused=$5 WHERE tenant_id=$1 AND id=$2")
			.bind(user.tenant)
			.bind(id)
			.bind(name)
			.bind(optional_uuid(&form.farm)?)
			.bind(form.paused)
			.execute(&mut *tx)
			.await?
			.rows_affected();
	if affected != 1 {
		return Err(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."));
	}
	auth::audit(&mut tx, &user, "Sunucu güncellendi", &id.to_string()).await?;
	tx.commit().await?;
	Ok(Redirect::to("/panel/servers?notice=saved").into_response())
}
async fn server_configuration(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Read).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let row = query(
		"SELECT name,configuration,config_code,status FROM servers WHERE tenant_id=$1 AND id=$2",
	)
	.bind(user.tenant)
	.bind(id)
	.fetch_optional(&mut *tx)
	.await?
	.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	let config: serde_json::Value = serde_json::from_slice(&state.store.vault.open(
		&row.get::<Vec<u8>, _>("configuration"),
		&format!("{}:{id}:config", user.tenant),
	)?)
	.map_err(anyhow::Error::from)?;
	if user.bot {
		return Ok(axum::Json(json!({"id":id,"name":row.get::<String,_>("name"),"base_code":row.get::<String,_>("config_code"),"settings":vpnhood_protocol::config::editable(&config),"status":row.get::<Option<serde_json::Value>,_>("status")})).into_response());
	}
	Ok(views::configuration(
		&user,
		id,
		&row.get::<String, _>("name"),
		&row.get::<String, _>("config_code"),
		vpnhood_protocol::config::editable(&config),
		row.get("status"),
		None,
	)
	.into_response())
}
#[derive(Deserialize)]
struct ConfigurationForm {
	#[serde(default)]
	csrf: String,
	base_code: String,
	#[serde(deserialize_with = "json_text")]
	settings: String,
	#[serde(default)]
	confirmed: bool,
}
async fn save_configuration(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<ConfigurationForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	if !form.confirmed {
		return Err(bad("Yapılandırma değişikliğinin etkilerini onaylayın."));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	let row = query("SELECT name,configuration,config_code,status FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
		.bind(user.tenant).bind(id).fetch_optional(&mut *tx).await?
		.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	let current_code: String = row.get("config_code");
	if current_code != form.base_code {
		return Err(PanelError(
			StatusCode::CONFLICT,
			"Ayarlar başka bir işlemle değişmiş. Sayfayı yenileyin.",
		));
	}
	let context = format!("{}:{id}:config", user.tenant);
	let mut config: serde_json::Value = serde_json::from_slice(
		&state
			.store
			.vault
			.open(&row.get::<Vec<u8>, _>("configuration"), &context)?,
	)
	.map_err(anyhow::Error::from)?;
	let settings = serde_json::from_str::<serde_json::Value>(&form.settings);
	let validation = settings
		.as_ref()
		.map_err(|e| e.to_string())
		.and_then(vpnhood_protocol::config::validate);
	if let Err(error) = validation {
		if user.bot {
			return Ok(
				(StatusCode::BAD_REQUEST, axum::Json(json!({"error":error}))).into_response(),
			);
		}
		let submitted = settings.unwrap_or_else(|_| json!({}));
		return Ok((
			StatusCode::BAD_REQUEST,
			views::configuration(
				&user,
				id,
				&row.get::<String, _>("name"),
				&current_code,
				submitted,
				row.get("status"),
				Some(error),
			),
		)
			.into_response());
	}
	let settings = settings.expect("validated JSON");
	if vpnhood_protocol::config::editable(&config) != settings {
		vpnhood_protocol::config::replace(&mut config, settings)
			.map_err(|_| bad("Yapılandırma geçersiz."))?;
		let code = Uuid::new_v4().to_string();
		config["configCode"] = code.clone().into();
		let sealed = state.store.vault.seal(
			&serde_json::to_vec(&config).map_err(anyhow::Error::from)?,
			&context,
		)?;
		query("UPDATE servers SET configuration=$3,config_code=$4 WHERE tenant_id=$1 AND id=$2")
			.bind(user.tenant)
			.bind(id)
			.bind(sealed)
			.bind(code)
			.execute(&mut *tx)
			.await?;
		auth::audit(
			&mut tx,
			&user,
			"Sunucu yapılandırması değiştirildi",
			&id.to_string(),
		)
		.await?;
	}
	tx.commit().await?;
	Ok(Redirect::to(&format!("/manage/servers/{id}/configuration")).into_response())
}

#[derive(Deserialize)]
struct TokenForm {
	#[serde(default)]
	csrf: String,
	#[serde(default)]
	profile: String,
	server: Uuid,
	name: String,
	count: usize,
	max_clients: i32,
	max_gib: u64,
	#[serde(default)]
	expires: String,
}
async fn add_tokens(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<TokenForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Tokens).await?;
	let name = management::valid_name(&form.name).map_err(|_| bad("Anahtar adı geçersiz."))?;
	if !(1..=100).contains(&form.count)
		|| !(0..=10000).contains(&form.max_clients)
		|| form.max_gib > 1_000_000
	{
		return Err(bad("Adet veya kota geçersiz."));
	}
	let expires = if form.expires.is_empty() {
		None
	} else {
		Some(
			DateTime::parse_from_rfc3339(&form.expires)
				.map_err(|_| {
					bad("Süre RFC3339 biçiminde olmalı; örnek: 2026-12-31T23:59:00+03:00.")
				})?
				.with_timezone(&Utc),
		)
	};
	if expires.is_some_and(|e| e <= Utc::now()) {
		return Err(bad("Son kullanma zamanı gelecekte olmalı."));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	let owned: bool =
		query_scalar("SELECT EXISTS(SELECT FROM servers WHERE tenant_id=$1 AND id=$2)")
			.bind(user.tenant)
			.bind(form.server)
			.fetch_one(&mut *tx)
			.await?;
	if !owned {
		return Err(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."));
	}
	let profile = if let Some(id) = optional_uuid(&form.profile)? {
		let value: serde_json::Value =
			query_scalar("SELECT policy FROM access_profiles WHERE tenant_id=$1 AND id=$2")
				.bind(user.tenant)
				.bind(id)
				.fetch_optional(&mut *tx)
				.await?
				.ok_or(PanelError(StatusCode::NOT_FOUND, "Profil bulunamadı."))?;
		Some((
			id,
			serde_json::from_value(value).map_err(anyhow::Error::from)?,
		))
	} else {
		None
	};
	let keys = management::create_tokens(
		&mut tx,
		&state.store.vault,
		user.tenant,
		form.server,
		&name,
		form.count,
		form.max_clients,
		(form.max_gib * 1024 * 1024 * 1024) as i64,
		expires,
		profile,
	)
	.await?;
	auth::audit(
		&mut tx,
		&user,
		"Erişim anahtarları oluşturuldu",
		&format!("{} adet / {}", keys.len(), form.server),
	)
	.await?;
	tx.commit().await?;
	if user.bot {
		return Ok(axum::Json(json!({"keys":keys.into_iter().map(|(id,name,key)|json!({"id":id,"name":name,"key":key})).collect::<Vec<_>>()})).into_response());
	}
	let text = keys
		.into_iter()
		.map(|(_, name, key)| format!("{name}\n{key}"))
		.collect::<Vec<_>>()
		.join("\n\n");
	Ok(secret_response(
		&user,
		"Erişim anahtarları hazır",
		"Anahtarları güvenli biçimde saklayıp kullanıcılarınızla paylaşın. Bu sayfa önbelleğe alınmaz.",
		&text,
	)
	.into_response())
}
#[derive(Deserialize)]
struct BulkForm {
	#[serde(default)]
	csrf: String,
	ids: String,
	action: String,
	#[serde(default)]
	max_gib: u64,
	#[serde(default)]
	max_clients: i32,
	#[serde(default)]
	expires: String,
}
async fn bulk_tokens(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<BulkForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Tokens).await?;
	let ids: Vec<Uuid> = form
		.ids
		.split_whitespace()
		.map(Uuid::parse_str)
		.collect::<Result<_, _>>()
		.map_err(|_| bad("Anahtar seçimi geçersiz."))?;
	if ids.is_empty() || ids.len() > 100 {
		return Err(bad("1–100 anahtar seçin."));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	let owned = query(
		"SELECT id FROM access_tokens WHERE tenant_id=$1 AND id=ANY($2) ORDER BY id FOR UPDATE",
	)
	.bind(user.tenant)
	.bind(&ids)
	.fetch_all(&mut *tx)
	.await?;
	if owned.len() != ids.len() {
		return Err(PanelError(
			StatusCode::NOT_FOUND,
			"Seçilen anahtarlardan biri bulunamadı.",
		));
	}
	match form.action.as_str() {
		"revoke" | "enable" => {
			query("UPDATE access_tokens SET revoked=$3 WHERE tenant_id=$1 AND id=ANY($2)")
				.bind(user.tenant)
				.bind(&ids)
				.bind(form.action == "revoke")
				.execute(&mut *tx)
				.await?;
		}
		"quota" => {
			if form.max_gib > 1_000_000 || !(0..=10000).contains(&form.max_clients) {
				return Err(bad("Kota geçersiz."));
			}
			query(
				"UPDATE access_tokens SET max_traffic=$3,max_clients=$4 WHERE tenant_id=$1 AND id=ANY($2)",
			)
			.bind(user.tenant)
			.bind(&ids)
			.bind((form.max_gib * 1024 * 1024 * 1024) as i64)
			.bind(form.max_clients)
			.execute(&mut *tx)
			.await?;
		}
		"expiry" => {
			let expires = if form.expires.is_empty() {
				None
			} else {
				Some(
					DateTime::parse_from_rfc3339(&form.expires)
						.map_err(|_| bad("Son kullanma zamanı RFC3339 biçiminde olmalı."))?
						.with_timezone(&Utc),
				)
			};
			if expires.is_some_and(|e| e <= Utc::now()) {
				return Err(bad("Son kullanma zamanı gelecekte olmalı."));
			}
			query("UPDATE access_tokens SET expires_at=$3 WHERE tenant_id=$1 AND id=ANY($2)")
				.bind(user.tenant)
				.bind(&ids)
				.bind(expires)
				.execute(&mut *tx)
				.await?;
		}
		_ => return Err(bad("Toplu işlem geçersiz.")),
	}
	query(
		"UPDATE sessions SET pending_notify=true WHERE tenant_id=$1 AND token_id=ANY($2) AND NOT closed",
	)
	.bind(user.tenant)
	.bind(&ids)
	.execute(&mut *tx)
	.await?;
	auth::audit(
		&mut tx,
		&user,
		"Toplu anahtar işlemi",
		&format!("{} / {} adet", form.action, ids.len()),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to("/panel/tokens?notice=saved").into_response())
}
async fn close_session(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<i64>,
	Form(form): Form<CsrfForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Tokens).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let affected =
		query("UPDATE sessions SET closed=true,pending_notify=true WHERE tenant_id=$1 AND id=$2")
			.bind(user.tenant)
			.bind(id)
			.execute(&mut *tx)
			.await?
			.rows_affected();
	if affected != 1 {
		return Err(PanelError(StatusCode::NOT_FOUND, "Oturum bulunamadı."));
	}
	auth::audit(&mut tx, &user, "VPN oturumu sonlandırıldı", &id.to_string()).await?;
	tx.commit().await?;
	Ok(Redirect::to("/panel/sessions?notice=saved").into_response())
}
#[derive(Deserialize)]
struct UserForm {
	#[serde(default)]
	csrf: String,
	email: String,
	password: String,
	role: String,
}
async fn add_user(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<UserForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Admin).await?;
	if !["owner", "admin", "operator", "token_writer", "viewer"].contains(&form.role.as_str()) {
		return Err(bad("Rol geçersiz."));
	}
	if form.role == "owner" && !user.is_owner() {
		return Err(PanelError(
			StatusCode::FORBIDDEN,
			"Sahip rolünü yalnız sahip atayabilir.",
		));
	}
	let address = auth::email(&form.email).map_err(|_| bad("E-posta adresi geçersiz."))?;
	let hash = tokio::task::spawn_blocking(move || auth::hash_password(&form.password))
		.await
		.map_err(|_| bad("Parola işlenemedi."))?
		.map_err(|_| bad("Parola 16–128 bayt olmalı."))?;
	let mut tx = state.store.begin(user.tenant).await?;
	let id = Uuid::new_v4();
	auth::insert_account(&mut tx, user.tenant, id, &address, &hash, &form.role).await?;
	auth::audit(&mut tx, &user, "Panel kullanıcısı eklendi", &address).await?;
	tx.commit().await?;
	Ok(Redirect::to("/panel/users?notice=saved").into_response())
}
#[derive(Deserialize)]
struct EditUserForm {
	#[serde(default)]
	csrf: String,
	role: String,
	#[serde(default)]
	active: bool,
}
async fn edit_user(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<EditUserForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Admin).await?;
	if !["owner", "admin", "operator", "token_writer", "viewer"].contains(&form.role.as_str()) {
		return Err(bad("Rol geçersiz."));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	query("SELECT id FROM tenants WHERE id=$1 FOR UPDATE")
		.bind(user.tenant)
		.fetch_one(&mut *tx)
		.await?;
	let target=query("SELECT m.role,a.active FROM memberships m JOIN accounts a ON a.id=m.account_id WHERE m.tenant_id=$1 AND m.account_id=$2").bind(user.tenant).bind(id).fetch_optional(&mut *tx).await?.ok_or(PanelError(StatusCode::NOT_FOUND,"Kullanıcı bulunamadı."))?;
	let is_bot: bool = query_scalar("SELECT EXISTS(SELECT FROM bots WHERE tenant_id=$1 AND id=$2)")
		.bind(user.tenant)
		.bind(id)
		.fetch_one(&mut *tx)
		.await?;
	if is_bot {
		return Err(bad("Bot rolünü değiştirmek için yeni bot oluşturun."));
	}
	let target_role: String = target.get("role");
	if (target_role == "owner" || form.role == "owner") && !user.is_owner() {
		return Err(PanelError(
			StatusCode::FORBIDDEN,
			"Sahip rolünü yalnız sahip değiştirebilir.",
		));
	}
	if target.get::<bool, _>("active")
		&& (!form.active || !["admin", "owner"].contains(&form.role.as_str()))
		&& ["admin", "owner"].contains(&target_role.as_str())
	{
		let admins:i64=query_scalar("SELECT count(*) FROM memberships m JOIN accounts a ON a.id=m.account_id WHERE m.tenant_id=$1 AND m.role IN ('admin','owner') AND a.active").bind(user.tenant).fetch_one(&mut *tx).await?;
		if admins <= 1 {
			return Err(bad(
				"Kiracının son etkin yöneticisi devre dışı bırakılamaz.",
			));
		}
	}
	if target_role == "owner"
		&& target.get::<bool, _>("active")
		&& (!form.active || form.role != "owner")
	{
		let owners:i64=query_scalar("SELECT count(*) FROM memberships m JOIN accounts a ON a.id=m.account_id WHERE m.tenant_id=$1 AND m.role='owner' AND a.active").bind(user.tenant).fetch_one(&mut *tx).await?;
		if owners <= 1 {
			return Err(bad("Kiracının son etkin sahibi devre dışı bırakılamaz."));
		}
	}
	query("UPDATE memberships SET role=$3 WHERE tenant_id=$1 AND account_id=$2")
		.bind(user.tenant)
		.bind(id)
		.bind(&form.role)
		.execute(&mut *tx)
		.await?;
	query("UPDATE accounts SET active=$2 WHERE id=$1")
		.bind(id)
		.bind(form.active)
		.execute(&mut *tx)
		.await?;
	if !form.active {
		query("DELETE FROM web_sessions WHERE account_id=$1")
			.bind(id)
			.execute(&mut *tx)
			.await?;
	}
	auth::audit(
		&mut tx,
		&user,
		"Kullanıcı rolü/durumu güncellendi",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to("/panel/users?notice=saved").into_response())
}
#[derive(Deserialize)]
struct PasswordForm {
	#[serde(default)]
	csrf: String,
	current_password: String,
	new_password: String,
}
async fn change_password(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<PasswordForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Read).await?;
	let hash: String = query_scalar("SELECT password_hash FROM accounts WHERE id=$1")
		.bind(user.id)
		.fetch_one(&state.store.pool)
		.await?;
	if !auth::verify_password(form.current_password, hash).await {
		return Err(bad("Mevcut parola hatalı."));
	}
	let hash = tokio::task::spawn_blocking(move || auth::hash_password(&form.new_password))
		.await
		.map_err(|_| bad("Parola işlenemedi."))?
		.map_err(|_| bad("Parola 16–128 bayt olmalı."))?;
	let mut tx = state.store.begin(user.tenant).await?;
	query("UPDATE accounts SET password_hash=$2 WHERE id=$1")
		.bind(user.id)
		.bind(hash)
		.execute(&mut *tx)
		.await?;
	query("DELETE FROM web_sessions WHERE account_id=$1")
		.bind(user.id)
		.execute(&mut *tx)
		.await?;
	auth::audit(&mut tx, &user, "Parola değiştirildi", &user.id.to_string()).await?;
	tx.commit().await?;
	Ok((
		[(header::SET_COOKIE, auth::cookie_header(auth::COOKIE, "", 0))],
		Redirect::to("/signin"),
	)
		.into_response())
}
async fn update_tenant(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<FarmForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Admin).await?;
	let name =
		management::valid_name(&form.name).map_err(|_| bad("Çalışma alanı adı geçersiz."))?;
	let mut tx = state.store.begin(user.tenant).await?;
	query("UPDATE tenants SET name=$2 WHERE id=$1")
		.bind(user.tenant)
		.bind(name)
		.execute(&mut *tx)
		.await?;
	auth::audit(
		&mut tx,
		&user,
		"Çalışma alanı yeniden adlandırıldı",
		&user.tenant.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to("/panel/account?notice=saved").into_response())
}
async fn export_csv(
	State(state): State<WebState>,
	headers: HeaderMap,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Read).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let rows=query("SELECT t.name,s.name AS server,t.sent,t.received,t.max_traffic,t.revoked FROM access_tokens t JOIN servers s ON s.tenant_id=t.tenant_id AND s.id=t.server_id WHERE t.tenant_id=$1 ORDER BY t.name LIMIT 10000").bind(user.tenant).fetch_all(&mut *tx).await?;
	let mut csv = i18n::tr("Anahtar,Sunucu,Gönderilen bayt,Alınan bayt,Kota bayt,İptal") + "\r\n";
	for row in rows {
		csv += &format!(
			"{},{},{},{},{},{}\r\n",
			csv_cell(&row.get::<String, _>("name")),
			csv_cell(&row.get::<String, _>("server")),
			row.get::<i64, _>("sent"),
			row.get::<i64, _>("received"),
			row.get::<i64, _>("max_traffic"),
			row.get::<bool, _>("revoked")
		);
	}
	Ok((
		[
			(header::CONTENT_TYPE, "text/csv; charset=utf-8"),
			(
				header::CONTENT_DISPOSITION,
				"attachment; filename=usage.csv",
			),
		],
		csv,
	)
		.into_response())
}
fn csv_cell(value: &str) -> String {
	let prefix = if value.trim_start().starts_with(['=', '+', '-', '@']) {
		"'"
	} else {
		""
	};
	format!("\"{prefix}{}\"", value.replace('"', "\"\""))
}
async fn events(State(state): State<WebState>, headers: HeaderMap) -> Result<Response, PanelError> {
	require(&state, &headers, None, Permission::Read).await?;
	let ticker = tokio::time::interval(Duration::from_secs(15));
	let stream = futures_util::stream::unfold(
		(state, headers, ticker),
		|(state, headers, mut ticker)| async move {
			ticker.tick().await;
			let user = auth::user(&state.store, &headers).await.ok().flatten()?;
			let data = views::stats(&state.store, user.tenant).await.ok()?;
			Some((
				Ok::<_, Infallible>(Event::default().event("stats").data(data.to_string())),
				(state, headers, ticker),
			))
		},
	);
	Ok(Sse::new(stream)
		.keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
		.into_response())
}
async fn profiles(
	State(state): State<WebState>,
	headers: HeaderMap,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Read).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let rows = query(
		"SELECT id,name,is_public,policy FROM access_profiles WHERE tenant_id=$1 ORDER BY name LIMIT 500",
	)
	.bind(user.tenant)
	.fetch_all(&mut *tx)
	.await?;
	let profiles = rows
		.into_iter()
		.map(|r| views::Profile {
			id: r.get("id"),
			name: r.get("name"),
			is_public: r.get("is_public"),
			policy: r.get("policy"),
		})
		.collect();
	Ok(views::profiles(&user, profiles).into_response())
}
#[derive(Deserialize)]
struct ProfileForm {
	#[serde(default)]
	csrf: String,
	name: String,
	#[serde(deserialize_with = "json_text")]
	policy: String,
	#[serde(default)]
	apply_existing: bool,
}
fn parse_policy(text: &str) -> Result<crate::policy::AccessPolicy, PanelError> {
	let policy: crate::policy::AccessPolicy = serde_json::from_str(text)
		.map_err(|_| bad("Politika JSON biçimi veya alan adları geçersiz."))?;
	policy
		.validate()
		.map_err(|_| bad("Politika sınırları veya istemci politikaları geçersiz."))?;
	Ok(policy)
}
async fn add_profile(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<ProfileForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Tokens).await?;
	let name = management::valid_name(&form.name).map_err(|_| bad("Profil adı geçersiz."))?;
	let policy = parse_policy(&form.policy)?;
	let id = Uuid::new_v4();
	let mut tx = state.store.begin(user.tenant).await?;
	query("INSERT INTO access_profiles(tenant_id,id,name,is_public,policy) VALUES($1,$2,$3,$4,$5)")
		.bind(user.tenant)
		.bind(id)
		.bind(name)
		.bind(policy.is_public)
		.bind(serde_json::to_value(policy).map_err(anyhow::Error::from)?)
		.execute(&mut *tx)
		.await?;
	auth::audit(
		&mut tx,
		&user,
		"Erişim profili oluşturuldu",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to("/manage/profiles").into_response())
}
async fn edit_profile(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<ProfileForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Tokens).await?;
	let name = management::valid_name(&form.name).map_err(|_| bad("Profil adı geçersiz."))?;
	let policy = parse_policy(&form.policy)?;
	let mut tx = state.store.begin(user.tenant).await?;
	let public: bool = query_scalar(
		"SELECT is_public FROM access_profiles WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
	)
	.bind(user.tenant)
	.bind(id)
	.fetch_optional(&mut *tx)
	.await?
	.ok_or(PanelError(StatusCode::NOT_FOUND, "Profil bulunamadı."))?;
	if public != policy.is_public {
		return Err(bad("Profilin açık/özel türü değiştirilemez."));
	}
	let value = serde_json::to_value(&policy).map_err(anyhow::Error::from)?;
	query("UPDATE access_profiles SET name=$3,policy=$4 WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.bind(name)
		.bind(&value)
		.execute(&mut *tx)
		.await?;
	if form.apply_existing {
		let ids: Vec<Uuid> = query_scalar(
			"SELECT id FROM access_tokens WHERE tenant_id=$1 AND profile_id=$2 ORDER BY id FOR UPDATE",
		)
		.bind(user.tenant)
		.bind(id)
		.fetch_all(&mut *tx)
		.await?;
		query("UPDATE access_tokens t SET policy=$3,max_clients=$4,max_traffic=$5,farm_id=CASE WHEN ($3->>'farmAccess')::boolean THEN (SELECT s.farm_id FROM servers s WHERE s.tenant_id=t.tenant_id AND s.id=t.server_id) ELSE NULL END WHERE tenant_id=$1 AND id=ANY($2)")
			.bind(user.tenant).bind(&ids).bind(value).bind(policy.max_clients).bind(policy.max_total_traffic).execute(&mut *tx).await?;
		query("UPDATE sessions SET closed=true,pending_notify=true WHERE tenant_id=$1 AND token_id=ANY($2) AND NOT closed")
			.bind(user.tenant).bind(ids).execute(&mut *tx).await?;
	}
	auth::audit(
		&mut tx,
		&user,
		"Erişim profili değiştirildi",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to("/manage/profiles").into_response())
}
async fn token_policy(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Tokens).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let row = query(
		"SELECT name,policy,max_clients,max_traffic,group_name,description,customer_id,order_id,support_id FROM access_tokens WHERE tenant_id=$1 AND id=$2",
	)
	.bind(user.tenant)
	.bind(id)
	.fetch_optional(&mut *tx)
	.await?
	.ok_or(PanelError(StatusCode::NOT_FOUND, "Anahtar bulunamadı."))?;
	let mut policy: crate::policy::AccessPolicy =
		serde_json::from_value(row.get("policy")).map_err(anyhow::Error::from)?;
	policy.max_clients = row.get("max_clients");
	policy.max_total_traffic = row.get("max_traffic");
	Ok(views::token_policy(
		&user,
		id,
		&row.get::<String, _>("name"),
		serde_json::to_value(policy).map_err(anyhow::Error::from)?,
		json!({"name":row.get::<String,_>("name"),"group_name":row.get::<String,_>("group_name"),"description":row.get::<String,_>("description"),"customer_id":row.get::<String,_>("customer_id"),"order_id":row.get::<String,_>("order_id"),"support_id":row.get::<String,_>("support_id")}),
	)
	.into_response())
}
#[derive(Deserialize)]
struct TokenPolicyForm {
	#[serde(default)]
	csrf: String,
	#[serde(deserialize_with = "json_text")]
	policy: String,
	#[serde(default)]
	confirmed: bool,
}
async fn save_token_policy(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<TokenPolicyForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Tokens).await?;
	if !form.confirmed {
		return Err(bad("Etkin oturumların kapanacağını onaylayın."));
	}
	let policy = parse_policy(&form.policy)?;
	let mut tx = state.store.begin(user.tenant).await?;
	let exists: Option<Uuid> =
		query_scalar("SELECT id FROM access_tokens WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
			.bind(user.tenant)
			.bind(id)
			.fetch_optional(&mut *tx)
			.await?;
	if exists.is_none() {
		return Err(PanelError(StatusCode::NOT_FOUND, "Anahtar bulunamadı."));
	}
	query("UPDATE access_tokens t SET profile_id=NULL,policy=$3,max_clients=$4,max_traffic=$5,farm_id=CASE WHEN ($3->>'farmAccess')::boolean THEN (SELECT s.farm_id FROM servers s WHERE s.tenant_id=t.tenant_id AND s.id=t.server_id) ELSE NULL END WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant).bind(id).bind(serde_json::to_value(&policy).map_err(anyhow::Error::from)?).bind(policy.max_clients).bind(policy.max_total_traffic).execute(&mut *tx).await?;
	query("UPDATE sessions SET closed=true,pending_notify=true WHERE tenant_id=$1 AND token_id=$2 AND NOT closed").bind(user.tenant).bind(id).execute(&mut *tx).await?;
	auth::audit(
		&mut tx,
		&user,
		"Erişim politikası değiştirildi",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to("/panel/tokens?notice=saved").into_response())
}
async fn token_key(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Tokens).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let token = management::access_key(&mut tx, &state.store.vault, user.tenant, id).await?;
	Ok(secret_response(
		&user,
		"VPN erişim anahtarı",
		"Anahtarı yalnız yetkili VPN kullanıcısıyla paylaşın.",
		&token,
	)
	.into_response())
}

async fn server_routing(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Write).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let row = query(
		"SELECT name,configuration,config_code,management FROM servers WHERE tenant_id=$1 AND id=$2",
	)
	.bind(user.tenant)
	.bind(id)
	.fetch_optional(&mut *tx)
	.await?
	.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	let config: serde_json::Value = serde_json::from_slice(&state.store.vault.open(
		&row.get::<Vec<u8>, _>("configuration"),
		&format!("{}:{id}:config", user.tenant),
	)?)
	.map_err(anyhow::Error::from)?;
	let policy: crate::farms::ServerPolicy =
		serde_json::from_value(row.get("management")).map_err(anyhow::Error::from)?;
	Ok(views::routing(
		&user,
		id,
		&row.get::<String, _>("name"),
		&row.get::<String, _>("config_code"),
		serde_json::to_value(policy).map_err(anyhow::Error::from)?,
		crate::farms::advertisement(&config),
	)
	.into_response())
}
#[derive(Deserialize)]
struct RoutingForm {
	#[serde(default)]
	csrf: String,
	base_code: String,
	#[serde(deserialize_with = "json_text")]
	policy: String,
	#[serde(deserialize_with = "json_text")]
	advertisement: String,
	#[serde(default)]
	confirmed: bool,
}
async fn save_routing(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<RoutingForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	if !form.confirmed {
		return Err(bad("Yapılandırma değişikliğinin etkilerini onaylayın."));
	}
	let policy: crate::farms::ServerPolicy =
		serde_json::from_str(&form.policy).map_err(|_| bad("Sunucu politikası geçersiz."))?;
	policy
		.validate()
		.map_err(|_| bad("Sunucu politikası geçersiz."))?;
	let advertisement: serde_json::Value =
		serde_json::from_str(&form.advertisement).map_err(|_| bad("Duyuru ayarları geçersiz."))?;
	let mut tx = state.store.begin(user.tenant).await?;
	let row = query(
		"SELECT configuration,config_code FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
	)
	.bind(user.tenant)
	.bind(id)
	.fetch_optional(&mut *tx)
	.await?
	.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	if row.get::<String, _>("config_code") != form.base_code {
		return Err(PanelError(
			StatusCode::CONFLICT,
			"Ayarlar başka bir işlemle değişmiş. Sayfayı yenileyin.",
		));
	}
	let context = format!("{}:{id}:config", user.tenant);
	let mut config: serde_json::Value = serde_json::from_slice(
		&state
			.store
			.vault
			.open(&row.get::<Vec<u8>, _>("configuration"), &context)?,
	)
	.map_err(anyhow::Error::from)?;
	crate::farms::apply_advertisement(&mut config, &advertisement)
		.map_err(|_| bad("Duyuru ayarları geçersiz."))?;
	let code = Uuid::new_v4().to_string();
	config["configCode"] = code.clone().into();
	query(
		"UPDATE servers SET configuration=$3,config_code=$4,management=$5 WHERE tenant_id=$1 AND id=$2",
	)
	.bind(user.tenant)
	.bind(id)
	.bind(state.store.vault.seal(
		&serde_json::to_vec(&config).map_err(anyhow::Error::from)?,
		&context,
	)?)
	.bind(code)
	.bind(serde_json::to_value(policy).map_err(anyhow::Error::from)?)
	.execute(&mut *tx)
	.await?;
	auth::audit(
		&mut tx,
		&user,
		"Sunucu yönlendirmesi değiştirildi",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to(&format!("/manage/servers/{id}/routing")).into_response())
}
#[derive(Deserialize)]
struct ConfirmForm {
	#[serde(default)]
	csrf: String,
	#[serde(default)]
	confirmed: bool,
}
async fn create_access_code(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<ConfirmForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Tokens).await?;
	if !form.confirmed {
		return Err(bad("Erişim kodu oluşturmayı onaylayın."));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	let exists: bool =
		query_scalar("SELECT EXISTS(SELECT FROM access_tokens WHERE tenant_id=$1 AND id=$2)")
			.bind(user.tenant)
			.bind(id)
			.fetch_one(&mut *tx)
			.await?;
	if !exists {
		return Err(PanelError(StatusCode::NOT_FOUND, "Anahtar bulunamadı."));
	}
	let code = auth::nonce();
	query("INSERT INTO access_codes(tenant_id,token_id,code_hash) VALUES($1,$2,$3)")
		.bind(user.tenant)
		.bind(id)
		.bind(Sha256::digest(code.as_bytes()).to_vec())
		.execute(&mut *tx)
		.await?;
	auth::audit(&mut tx, &user, "Erişim kodu oluşturuldu", &id.to_string()).await?;
	tx.commit().await?;
	Ok(secret_response(
		&user,
		"Erişim kodu",
		"Bu kod ilgili anahtarın erişimini sağlar. Yalnız yetkili kullanıcıyla paylaşın.",
		&code,
	)
	.into_response())
}
async fn integrations(
	State(state): State<WebState>,
	headers: HeaderMap,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Admin).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let enabled: bool = query_scalar("SELECT EXISTS(SELECT FROM ad_hooks WHERE tenant_id=$1)")
		.bind(user.tenant)
		.fetch_one(&mut *tx)
		.await?;
	Ok(views::integrations(&user, enabled).into_response())
}
async fn configure_ad_hook(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<ConfirmForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Admin).await?;
	if !form.confirmed {
		return Err(bad("Ödül doğrulama bağlantısını oluşturmayı onaylayın."));
	}
	let credential = format!("{}.{}", user.tenant, auth::nonce());
	let mut tx = state.store.begin(user.tenant).await?;
	query("INSERT INTO ad_hooks(tenant_id,credential_hash) VALUES($1,$2) ON CONFLICT(tenant_id) DO UPDATE SET credential_hash=$2").bind(user.tenant).bind(Sha256::digest(credential.as_bytes()).to_vec()).execute(&mut *tx).await?;
	auth::audit(
		&mut tx,
		&user,
		"Ödül doğrulama bağlantısı oluşturuldu",
		&user.tenant.to_string(),
	)
	.await?;
	tx.commit().await?;
	let settings = json!({"url":format!("{}/api/hooks/ad/{}",state.origin,user.tenant),"method":"POST","authorization":format!("Bearer {credential}"),"body":{"sessionId":12345,"adData":"sid:12345;ad:00000000-0000-0000-0000-000000000001"}});
	Ok(secret_response(&user,"Ödül doğrulama bağlantısı","Bu kimliği yalnız reklam sonucunu doğrulayan güvenilir sunucuya verin. İstemci uygulamasına koymayın.",&serde_json::to_string_pretty(&settings).map_err(anyhow::Error::from)?).into_response())
}

async fn installation(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Write).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let name: String = query_scalar("SELECT name FROM servers WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.fetch_optional(&mut *tx)
		.await?
		.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	query("UPDATE install_jobs SET status='Failed',finished_at=now(),log='Yürütücü tamamlanamadı veya yeniden başlatıldı. Hedefi kontrol edip yeniden deneyin.' WHERE tenant_id=$1 AND server_id=$2 AND ((status='Queued' AND created_at<now()-interval '15 minutes') OR (status='Running' AND started_at<now()-interval '15 minutes'))").bind(user.tenant).bind(id).execute(&mut *tx).await?;
	let jobs=query("SELECT id,status,log,created_at FROM install_jobs WHERE tenant_id=$1 AND server_id=$2 ORDER BY created_at DESC LIMIT 25").bind(user.tenant).bind(id).fetch_all(&mut *tx).await?.into_iter().map(|r|views::InstallJob{id:r.get("id"),status:r.get("status"),log:r.get("log"),time:r.get::<DateTime<Utc>,_>("created_at").to_rfc3339()}).collect();
	tx.commit().await?;
	Ok(views::ssh_installation(&user, id, &name, jobs).into_response())
}
#[derive(Deserialize)]
struct InstallForm {
	#[serde(default)]
	csrf: String,
	host: String,
	port: u16,
	username: String,
	fingerprint: String,
	#[serde(default)]
	password: String,
	#[serde(default)]
	private_key: String,
	#[serde(default)]
	sudo_password: String,
	#[serde(default)]
	credential: String,
	#[serde(default)]
	confirmed: bool,
}
async fn start_installation(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<InstallForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	if !form.confirmed {
		return Err(bad("SSH kurulumu ve yapılandırma değişikliğini onaylayın."));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	let row = query(
		"SELECT credential_hash,agent_credential FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
	)
	.bind(user.tenant)
	.bind(id)
	.fetch_optional(&mut *tx)
	.await?
	.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	let credential = if !form.credential.is_empty() {
		form.credential
			.trim()
			.strip_prefix("Bearer ")
			.unwrap_or(form.credential.trim())
			.to_owned()
	} else if let Some(blob) = row.get::<Option<Vec<u8>>, _>("agent_credential") {
		String::from_utf8(
			state
				.store
				.vault
				.open(&blob, &format!("{}:{id}:agent", user.tenant))?,
		)
		.map_err(anyhow::Error::from)?
	} else {
		return Err(bad(
			"Eski kaydın bağlantı kimliğini sunucunun appsettings.json dosyasından girin.",
		));
	};
	use subtle::ConstantTimeEq;
	if !bool::from(
		row.get::<Vec<u8>, _>("credential_hash")
			.ct_eq(Sha256::digest(credential.as_bytes()).as_slice()),
	) {
		return Err(bad("Sunucu bağlantı kimliği geçersiz."));
	}
	let parameters = crate::install::SshInstall {
		host: management::normalize_host(&form.host).map_err(|_| bad("SSH adresi geçersiz."))?,
		port: form.port,
		username: form.username,
		fingerprint: form.fingerprint,
		password: form.password,
		private_key: form.private_key,
		sudo_password: form.sudo_password,
		credential,
		origin: state.origin.clone(),
	};
	parameters
		.validate()
		.map_err(|_| bad("SSH adresi, kullanıcı, parmak izi veya kimlik bilgisi geçersiz."))?;
	let count: i64 = query_scalar(
		"SELECT count(*) FROM install_jobs WHERE tenant_id=$1 AND status IN ('Queued','Running')",
	)
	.bind(user.tenant)
	.fetch_one(&mut *tx)
	.await?;
	if count >= 8 {
		return Err(PanelError(
			StatusCode::TOO_MANY_REQUESTS,
			"Çalışma alanında en fazla sekiz kurulum işi sırada olabilir.",
		));
	}
	let active:bool=query_scalar("SELECT EXISTS(SELECT FROM install_jobs WHERE tenant_id=$1 AND server_id=$2 AND status IN ('Queued','Running'))").bind(user.tenant).bind(id).fetch_one(&mut *tx).await?;
	if active {
		return Err(PanelError(
			StatusCode::CONFLICT,
			"Bu sunucunun devam eden bir kurulum işi var.",
		));
	}
	let job = Uuid::new_v4();
	query("INSERT INTO install_jobs(tenant_id,id,server_id,actor_id,host,ssh_port,ssh_user) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(user.tenant).bind(job).bind(id).bind(user.id).bind(&parameters.host).bind(i32::from(parameters.port)).bind(&parameters.username).execute(&mut *tx).await?;
	query("UPDATE servers SET agent_credential=$3 WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.bind(state.store.vault.seal(
			parameters.credential.as_bytes(),
			&format!("{}:{id}:agent", user.tenant),
		)?)
		.execute(&mut *tx)
		.await?;
	auth::audit(
		&mut tx,
		&user,
		"SSH sunucu kurulumu sıraya alındı",
		&format!("{id}/{job}"),
	)
	.await?;
	tx.commit().await?;
	tokio::spawn(crate::install::launch(
		state.store.clone(),
		user.tenant,
		job,
		parameters,
	));
	Ok(Redirect::to(&format!("/manage/servers/{id}/installation")).into_response())
}
async fn install_status(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Write).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let row = query("SELECT status,log FROM install_jobs WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.fetch_optional(&mut *tx)
		.await?
		.ok_or(PanelError(StatusCode::NOT_FOUND, "İş bulunamadı."))?;
	Ok(axum::Json(json!({"status":row.get::<String,_>("status"),"label":i18n::tr(&row.get::<String,_>("status")),"log":row.get::<String,_>("log")})).into_response())
}

async fn published_server_token(
	State(state): State<WebState>,
	Path((tenant, server)): Path<(Uuid, Uuid)>,
) -> Result<Response, PanelError> {
	let mut tx = state.store.begin(tenant).await?;
	let bytes: Vec<u8> =
		query_scalar("SELECT configuration FROM servers WHERE tenant_id=$1 AND id=$2")
			.bind(tenant)
			.bind(server)
			.fetch_optional(&mut *tx)
			.await?
			.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	let config: serde_json::Value = serde_json::from_slice(
		&state
			.store
			.vault
			.open(&bytes, &format!("{tenant}:{server}:config"))?,
	)
	.map_err(anyhow::Error::from)?;
	let token: vpnhood_protocol::ServerToken =
		serde_json::from_value(config["publicServerToken"].clone()).map_err(anyhow::Error::from)?;
	if token.urls.as_ref().is_none_or(Vec::is_empty) {
		return Err(PanelError(StatusCode::NOT_FOUND, "Yayımlama kapalı."));
	}
	let encrypted = token
		.encrypt(&crate::crypto::random::<16>())
		.map_err(anyhow::Error::msg)?;
	Ok((
		[(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
		encrypted,
	)
		.into_response())
}
#[derive(Deserialize)]
struct PublishForm {
	#[serde(default)]
	csrf: String,
}
async fn publish_token(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<PublishForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let bytes: Vec<u8> =
		query_scalar("SELECT configuration FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
			.bind(user.tenant)
			.bind(id)
			.fetch_optional(&mut *tx)
			.await?
			.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	let context = format!("{}:{id}:config", user.tenant);
	let mut config: serde_json::Value =
		serde_json::from_slice(&state.store.vault.open(&bytes, &context)?)
			.map_err(anyhow::Error::from)?;
	config["publicServerToken"]["urls"] =
		json!([format!("{}/s/{}/{id}", state.origin, user.tenant)]);
	config["publicServerToken"]["ct"] = json!(Utc::now());
	query("UPDATE servers SET configuration=$3 WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.bind(state.store.vault.seal(
			&serde_json::to_vec(&config).map_err(anyhow::Error::from)?,
			&context,
		)?)
		.execute(&mut *tx)
		.await?;
	auth::audit(
		&mut tx,
		&user,
		"Şifreli sunucu adresi yayımlama etkinleştirildi",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to(&format!("/manage/servers/{id}/configuration")).into_response())
}

async fn server_certificates(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Write).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let row =
		query("SELECT name,configuration,config_code FROM servers WHERE tenant_id=$1 AND id=$2")
			.bind(user.tenant)
			.bind(id)
			.fetch_optional(&mut *tx)
			.await?
			.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	let config: serde_json::Value = serde_json::from_slice(&state.store.vault.open(
		&row.get::<Vec<u8>, _>("configuration"),
		&format!("{}:{id}:config", user.tenant),
	)?)
	.map_err(anyhow::Error::from)?;
	Ok(views::certificates(
		&user,
		id,
		&row.get::<String, _>("name"),
		&row.get::<String, _>("config_code"),
		crate::certificates::describe(&config),
	)
	.into_response())
}
#[derive(Deserialize)]
struct CertificateForm {
	#[serde(default)]
	csrf: String,
	base_code: String,
	pfx: String,
	password: String,
	tls_name: String,
	#[serde(default)]
	make_default: bool,
	#[serde(default)]
	confirmed: bool,
}
async fn upload_certificate(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<CertificateForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	if !form.confirmed {
		return Err(bad("Sertifika değişikliğinin etkisini onaylayın."));
	}
	let name = management::normalize_host(&form.tls_name).map_err(|_| bad("TLS adı geçersiz."))?;
	let (certificate, hash) = tokio::task::spawn_blocking(move || {
		crate::certificates::import_pfx(&form.pfx, &form.password, &name)
	})
	.await
	.map_err(|_| bad("PFX işlenemedi."))?
	.map_err(|_| bad("PFX, parola, özel anahtar veya sertifika zamanı geçersiz."))?;
	let mut tx = state.store.begin(user.tenant).await?;
	let row = query(
		"SELECT configuration,config_code FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
	)
	.bind(user.tenant)
	.bind(id)
	.fetch_optional(&mut *tx)
	.await?
	.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	if row.get::<String, _>("config_code") != form.base_code {
		return Err(PanelError(
			StatusCode::CONFLICT,
			"Ayarlar başka bir işlemle değişmiş. Sayfayı yenileyin.",
		));
	}
	let context = format!("{}:{id}:config", user.tenant);
	let mut config: serde_json::Value = serde_json::from_slice(
		&state
			.store
			.vault
			.open(&row.get::<Vec<u8>, _>("configuration"), &context)?,
	)
	.map_err(anyhow::Error::from)?;
	let items = config["certificates"]
		.as_array_mut()
		.ok_or(bad("Sertifika dizisi eksik."))?;
	items.retain(|c| c["commonName"] != certificate["commonName"]);
	if items.len() >= 16 {
		return Err(bad("En fazla 16 sertifika yüklenebilir."));
	}
	if form.make_default {
		items.insert(0, certificate.clone());
	} else {
		items.push(certificate.clone());
	}
	if form.make_default {
		config["publicServerToken"]["ch"] = hash.into();
		config["publicServerToken"]["ct"] = json!(Utc::now());
		// Pin the replacement certificate; no implicit transition to CA trust.
		config["publicServerToken"]["isv"] = false.into();
		config["publicServerToken"]["hname"] = certificate["commonName"].clone();
	}
	let code = Uuid::new_v4().to_string();
	config["configCode"] = code.clone().into();
	query("UPDATE servers SET configuration=$3,config_code=$4 WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.bind(state.store.vault.seal(
			&serde_json::to_vec(&config).map_err(anyhow::Error::from)?,
			&context,
		)?)
		.bind(code)
		.execute(&mut *tx)
		.await?;
	auth::audit(
		&mut tx,
		&user,
		"Sunucu sertifikası yüklendi",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to(&format!("/manage/servers/{id}/certificates")).into_response())
}
#[derive(Deserialize)]
struct ChallengeForm {
	#[serde(default)]
	csrf: String,
	token: String,
	key_authorization: String,
	#[serde(default)]
	confirmed: bool,
}
async fn set_challenge(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<ChallengeForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	if !form.confirmed
		|| !(16..=256).contains(&form.token.len())
		|| !form
			.token
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
		|| form.key_authorization.len() > 1024
		|| !form
			.key_authorization
			.starts_with(&format!("{}.", form.token))
		|| !form
			.key_authorization
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
	{
		return Err(bad("HTTP-01 doğrulama verisi veya onayı geçersiz."));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	let bytes: Vec<u8> =
		query_scalar("SELECT configuration FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
			.bind(user.tenant)
			.bind(id)
			.fetch_optional(&mut *tx)
			.await?
			.ok_or(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."))?;
	let context = format!("{}:{id}:config", user.tenant);
	let mut config: serde_json::Value =
		serde_json::from_slice(&state.store.vault.open(&bytes, &context)?)
			.map_err(anyhow::Error::from)?;
	let code = Uuid::new_v4().to_string();
	config["configCode"] = code.clone().into();
	config["enableHttp01Challenge"] = true.into();
	query("UPDATE servers SET configuration=$3,config_code=$4 WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.bind(state.store.vault.seal(
			&serde_json::to_vec(&config).map_err(anyhow::Error::from)?,
			&context,
		)?)
		.bind(code)
		.execute(&mut *tx)
		.await?;
	query("DELETE FROM acme_challenges WHERE tenant_id=$1 AND server_id=$2 AND expires_at<=now()")
		.bind(user.tenant)
		.bind(id)
		.execute(&mut *tx)
		.await?;
	query("INSERT INTO acme_challenges(tenant_id,server_id,token,key_authorization,expires_at) VALUES($1,$2,$3,$4,now()+interval '15 minutes') ON CONFLICT(tenant_id,server_id,token) DO UPDATE SET key_authorization=$4,expires_at=now()+interval '15 minutes'").bind(user.tenant).bind(id).bind(form.token).bind(form.key_authorization).execute(&mut *tx).await?;
	auth::audit(
		&mut tx,
		&user,
		"HTTP-01 doğrulaması hazırlandı",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to(&format!("/manage/servers/{id}/certificates")).into_response())
}

async fn server_profiles(
	State(state): State<WebState>,
	headers: HeaderMap,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Read).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let rows=query("SELECT id,name,settings,management,version FROM server_profiles WHERE tenant_id=$1 ORDER BY name").bind(user.tenant).fetch_all(&mut *tx).await?.into_iter().map(|row|views::ServerProfile{id:row.get("id"),name:row.get("name"),settings:row.get("settings"),management:row.get("management"),version:row.get("version")}).collect();
	let servers = query("SELECT id,name FROM servers WHERE tenant_id=$1 ORDER BY name")
		.bind(user.tenant)
		.fetch_all(&mut *tx)
		.await?
		.into_iter()
		.map(|r| (r.get::<Uuid, _>("id"), r.get::<String, _>("name")))
		.collect();
	Ok(views::server_profiles(&user, rows, servers).into_response())
}
#[derive(Deserialize)]
struct ServerProfileForm {
	#[serde(default)]
	csrf: String,
	name: String,
	#[serde(deserialize_with = "json_text")]
	settings: String,
	#[serde(deserialize_with = "json_text")]
	management: String,
	#[serde(default)]
	version: Option<Uuid>,
}
fn parse_server_profile(
	form: &ServerProfileForm,
) -> Result<(String, serde_json::Value, serde_json::Value), PanelError> {
	let name = management::valid_name(&form.name).map_err(|_| bad("Profil adı geçersiz."))?;
	let settings: serde_json::Value =
		serde_json::from_str(&form.settings).map_err(|_| bad("Ayar JSON’u geçersiz."))?;
	vpnhood_protocol::config::validate(&settings)
		.map_err(|_| bad("Profil sunucu sözleşmesine uymuyor."))?;
	let policy: crate::farms::ServerPolicy =
		serde_json::from_str(&form.management).map_err(|_| bad("Sunucu politikası geçersiz."))?;
	policy
		.validate()
		.map_err(|_| bad("Sunucu politikası geçersiz."))?;
	Ok((
		name,
		settings,
		serde_json::to_value(policy).map_err(anyhow::Error::from)?,
	))
}
async fn add_server_profile(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<ServerProfileForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	let (name, settings, management) = parse_server_profile(&form)?;
	let mut tx = state.store.begin(user.tenant).await?;
	let id = Uuid::new_v4();
	query("INSERT INTO server_profiles(tenant_id,id,name,settings,management,version) VALUES($1,$2,$3,$4,$5,$6)").bind(user.tenant).bind(id).bind(name).bind(settings).bind(management).bind(Uuid::new_v4()).execute(&mut *tx).await?;
	auth::audit(
		&mut tx,
		&user,
		"Sunucu profili oluşturuldu",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to("/manage/server-profiles").into_response())
}
async fn edit_server_profile(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<ServerProfileForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	let (name, settings, management) = parse_server_profile(&form)?;
	let mut tx = state.store.begin(user.tenant).await?;
	let version: Uuid =
		query_scalar("SELECT version FROM server_profiles WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
			.bind(user.tenant)
			.bind(id)
			.fetch_optional(&mut *tx)
			.await?
			.ok_or(PanelError(StatusCode::NOT_FOUND, "Profil bulunamadı."))?;
	if Some(version) != form.version {
		return Err(PanelError(
			StatusCode::CONFLICT,
			"Profil değişti. Sayfayı yenileyin.",
		));
	}
	query("UPDATE server_profiles SET name=$3,settings=$4,management=$5,version=$6 WHERE tenant_id=$1 AND id=$2").bind(user.tenant).bind(id).bind(name).bind(settings).bind(management).bind(Uuid::new_v4()).execute(&mut *tx).await?;
	auth::audit(
		&mut tx,
		&user,
		"Sunucu profili değiştirildi",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to("/manage/server-profiles").into_response())
}
#[derive(Deserialize)]
struct ApplyServerProfileForm {
	#[serde(default)]
	csrf: String,
	version: Uuid,
	servers: String,
	#[serde(default)]
	confirmed: bool,
}
async fn apply_server_profile(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<ApplyServerProfileForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	if !form.confirmed {
		return Err(bad("Sunucu profili uygulamasını onaylayın."));
	}
	let ids: Vec<Uuid> = form
		.servers
		.split_whitespace()
		.map(Uuid::parse_str)
		.collect::<Result<_, _>>()
		.map_err(|_| bad("Sunucu seçimi geçersiz."))?;
	if ids.is_empty() || ids.len() > 100 {
		return Err(bad("Sunucu seçimi geçersiz."));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	let profile = query(
		"SELECT settings,management,version FROM server_profiles WHERE tenant_id=$1 AND id=$2 FOR SHARE",
	)
	.bind(user.tenant)
	.bind(id)
	.fetch_optional(&mut *tx)
	.await?
	.ok_or(PanelError(StatusCode::NOT_FOUND, "Profil bulunamadı."))?;
	if profile.get::<Uuid, _>("version") != form.version {
		return Err(PanelError(
			StatusCode::CONFLICT,
			"Profil değişti. Sayfayı yenileyin.",
		));
	}
	let rows = query(
		"SELECT id,configuration FROM servers WHERE tenant_id=$1 AND id=ANY($2) ORDER BY id FOR UPDATE",
	)
	.bind(user.tenant)
	.bind(&ids)
	.fetch_all(&mut *tx)
	.await?;
	if rows.len() != ids.len() {
		return Err(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."));
	}
	for row in rows {
		let server: Uuid = row.get("id");
		let context = format!("{}:{server}:config", user.tenant);
		let mut config: serde_json::Value = serde_json::from_slice(
			&state
				.store
				.vault
				.open(&row.get::<Vec<u8>, _>("configuration"), &context)?,
		)
		.map_err(anyhow::Error::from)?;
		vpnhood_protocol::config::replace(&mut config, profile.get("settings"))
			.map_err(|_| bad("Profil sunucu sözleşmesine uymuyor."))?;
		let code = Uuid::new_v4().to_string();
		config["configCode"] = json!(code);
		query(
			"UPDATE servers SET configuration=$3,config_code=$4,management=$5 WHERE tenant_id=$1 AND id=$2",
		)
		.bind(user.tenant)
		.bind(server)
		.bind(state.store.vault.seal(
			&serde_json::to_vec(&config).map_err(anyhow::Error::from)?,
			&context,
		)?)
		.bind(code)
		.bind(profile.get::<serde_json::Value, _>("management"))
		.execute(&mut *tx)
		.await?;
	}
	auth::audit(
		&mut tx,
		&user,
		"Sunucu profili topluca uygulandı",
		&format!("{id}/{}", ids.len()),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to("/panel/servers").into_response())
}

#[derive(Deserialize)]
struct ReportQuery {
	#[serde(default = "default_report_hours")]
	hours: i32,
	#[serde(default)]
	server: String,
}
fn default_report_hours() -> i32 {
	24
}
async fn reports(
	State(state): State<WebState>,
	headers: HeaderMap,
	Query(filter): Query<ReportQuery>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Read).await?;
	if ![1, 24, 48, 72, 168, 744].contains(&filter.hours) {
		return Err(bad("Rapor dönemi geçersiz."));
	}
	let server = optional_uuid(&filter.server)?;
	let mut tx = state.store.begin(user.tenant).await?;
	let servers: Vec<(Uuid, String)> =
		query("SELECT id,name FROM servers WHERE tenant_id=$1 ORDER BY name")
			.bind(user.tenant)
			.fetch_all(&mut *tx)
			.await?
			.into_iter()
			.map(|r| (r.get("id"), r.get("name")))
			.collect();
	if server.is_some_and(|id| !servers.iter().any(|(s, _)| *s == id)) {
		return Err(PanelError(StatusCode::NOT_FOUND, "Sunucu bulunamadı."));
	}
	// Aggregate in PostgreSQL so a long period over many servers remains bounded.
	let samples=query("WITH selected AS (SELECT date_bin(make_interval(secs=>$4),created_at,timestamptz '2000-01-01') AS bucket,status FROM server_samples WHERE tenant_id=$1 AND ($2::uuid IS NULL OR server_id=$2) AND created_at>now()-make_interval(hours=>$3)) SELECT bucket,jsonb_build_object('cpuUsage',avg(CASE WHEN jsonb_typeof(status#>'{cpuUsage}')='number' THEN greatest(0,least(1e15,(status#>'{cpuUsage}')::text::numeric)) END),'availableMemory',avg(CASE WHEN jsonb_typeof(status#>'{availableMemory}')='number' THEN greatest(0,least(1e15,(status#>'{availableMemory}')::text::numeric)) END),'sessionCount',avg(CASE WHEN jsonb_typeof(status#>'{sessionCount}')='number' THEN greatest(0,least(1e15,(status#>'{sessionCount}')::text::numeric)) END),'tunnelSpeed',jsonb_build_object('sent',avg(CASE WHEN jsonb_typeof(status#>'{tunnelSpeed,sent}')='number' THEN greatest(0,least(1e15,(status#>'{tunnelSpeed,sent}')::text::numeric)) END),'received',avg(CASE WHEN jsonb_typeof(status#>'{tunnelSpeed,received}')='number' THEN greatest(0,least(1e15,(status#>'{tunnelSpeed,received}')::text::numeric)) END))) AS status FROM selected GROUP BY bucket ORDER BY bucket").bind(user.tenant).bind(server).bind(filter.hours).bind((filter.hours*3600/744).max(60)).fetch_all(&mut *tx).await?.into_iter().map(|row|crate::reports::Sample{time:row.get("bucket"),status:row.get("status")}).collect::<Vec<_>>();
	let countries=query("SELECT coalesce(metadata->>'clientCountry','?') AS country,count(DISTINCT client_hash) AS devices FROM sessions WHERE tenant_id=$1 AND ($2::uuid IS NULL OR server_id=$2) AND created_at>now()-make_interval(hours=>$3) GROUP BY country ORDER BY devices DESC LIMIT 256").bind(user.tenant).bind(server).bind(filter.hours).fetch_all(&mut *tx).await?.into_iter().map(|r|(r.get::<String,_>("country"),r.get::<i64,_>("devices"))).collect();
	let range = samples
		.first()
		.zip(samples.last())
		.map(|(a, b)| {
			format!(
				"{} — {}",
				a.time
					.with_timezone(&chrono::FixedOffset::east_opt(10800).expect("fixed offset"))
					.format("%d.%m.%Y %H:%M"),
				b.time
					.with_timezone(&chrono::FixedOffset::east_opt(10800).expect("fixed offset"))
					.format("%d.%m.%Y %H:%M")
			)
		})
		.unwrap_or_default();
	Ok(views::reports(
		&user,
		servers,
		server,
		filter.hours,
		crate::reports::charts(&samples),
		countries,
		range,
	)
	.into_response())
}

async fn download_token(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Tokens).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let key = management::access_key(&mut tx, &state.store.vault, user.tenant, id).await?;
	Ok((
		[
			(header::CONTENT_TYPE, "text/plain; charset=utf-8".to_owned()),
			(
				header::CONTENT_DISPOSITION,
				format!("attachment; filename=hoodmin-{id}.vh"),
			),
		],
		key,
	)
		.into_response())
}
#[derive(Deserialize)]
struct TokenDetailsForm {
	#[serde(default)]
	csrf: String,
	name: String,
	group_name: String,
	description: String,
	customer_id: String,
	order_id: String,
	support_id: String,
}
async fn save_token_details(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<TokenDetailsForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Tokens).await?;
	let name = management::valid_name(&form.name).map_err(|_| bad("Anahtar adı geçersiz."))?;
	for (value, max) in [
		(&form.group_name, 120),
		(&form.description, 2000),
		(&form.customer_id, 128),
		(&form.order_id, 128),
		(&form.support_id, 128),
	] {
		if value.len() > max
			|| value
				.chars()
				.any(|c| c.is_control() && c != '\n' && c != '\t')
		{
			return Err(bad("Anahtar bilgileri geçersiz."));
		}
	}
	let mut tx = state.store.begin(user.tenant).await?;
	let affected=query("UPDATE access_tokens SET name=$3,group_name=$4,description=$5,customer_id=$6,order_id=$7,support_id=$8 WHERE tenant_id=$1 AND id=$2").bind(user.tenant).bind(id).bind(name).bind(form.group_name).bind(form.description).bind(form.customer_id).bind(form.order_id).bind(form.support_id).execute(&mut *tx).await?.rows_affected();
	if affected == 0 {
		return Err(PanelError(StatusCode::NOT_FOUND, "Anahtar bulunamadı."));
	}
	auth::audit(
		&mut tx,
		&user,
		"Anahtar bilgileri değiştirildi",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(Redirect::to(&format!("/manage/tokens/{id}/policy")).into_response())
}

async fn bots(State(state): State<WebState>, headers: HeaderMap) -> Result<Response, PanelError> {
	let user = require(&state, &headers, None, Permission::Admin).await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let bots=query("SELECT b.id,b.name,b.expires_at,b.revoked,m.role FROM bots b JOIN memberships m ON m.tenant_id=b.tenant_id AND m.account_id=b.id WHERE b.tenant_id=$1 ORDER BY b.created_at DESC LIMIT 200").bind(user.tenant).fetch_all(&mut *tx).await?.into_iter().map(|r|views::Bot{id:r.get("id"),name:r.get("name"),role:r.get("role"),expires:r.get::<DateTime<Utc>,_>("expires_at").to_rfc3339(),revoked:r.get("revoked")}).collect();
	Ok(views::bots(&user, bots).into_response())
}
#[derive(Deserialize)]
struct BotForm {
	#[serde(default)]
	csrf: String,
	name: String,
	role: String,
	lifetime_days: i32,
	#[serde(default)]
	confirmed: bool,
}
async fn create_bot(
	State(state): State<WebState>,
	headers: HeaderMap,
	Form(form): Form<BotForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Admin).await?;
	if !form.confirmed {
		return Err(bad("Botun yetkilerini ve erişim süresini onaylayın."));
	}
	if !["admin", "operator", "token_writer", "viewer"].contains(&form.role.as_str())
		|| !(1..=365).contains(&form.lifetime_days)
	{
		return Err(bad("Bot rolü veya süresi geçersiz."));
	}
	let name = management::valid_name(&form.name).map_err(|_| bad("Bot adı geçersiz."))?;
	let id = Uuid::new_v4();
	let key = format!("hmb.{}.{id}.{}", user.tenant, auth::nonce());
	let expires = Utc::now() + chrono::Duration::days(i64::from(form.lifetime_days));
	let mut tx = state.store.begin(user.tenant).await?;
	query("SELECT id FROM tenants WHERE id=$1 FOR UPDATE")
		.bind(user.tenant)
		.fetch_one(&mut *tx)
		.await?;
	let count: i64 = query_scalar(
		"SELECT count(*) FROM bots WHERE tenant_id=$1 AND NOT revoked AND expires_at>now()",
	)
	.bind(user.tenant)
	.fetch_one(&mut *tx)
	.await?;
	if count >= 100 {
		return Err(bad("En fazla 100 etkin bot olabilir."));
	}
	auth::insert_account(
		&mut tx,
		user.tenant,
		id,
		&format!("bot.{id}@hoodmin.invalid"),
		"!locked-bot",
		&form.role,
	)
	.await?;
	query("INSERT INTO bots(tenant_id,id,name,credential_hash,expires_at) VALUES($1,$2,$3,$4,$5)")
		.bind(user.tenant)
		.bind(id)
		.bind(name)
		.bind(Sha256::digest(key.as_bytes()).to_vec())
		.bind(expires)
		.execute(&mut *tx)
		.await?;
	auth::audit(
		&mut tx,
		&user,
		"Bot oluşturuldu",
		&format!("{id}/{}", form.role),
	)
	.await?;
	tx.commit().await?;
	Ok(secret_response(&user,"Bot API anahtarı","Bu anahtar botun seçilen yetkileriyle işlem yapar. Yalnız bu ekranda gösterilir; güvenli saklayın.",&serde_json::to_string_pretty(&json!({"id":id,"authorization":format!("Bearer {key}"),"baseUrl":format!("{}/api/manage",state.origin),"role":form.role,"expiresAt":expires})).map_err(anyhow::Error::from)?))
}
async fn revoke_bot(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(id): Path<Uuid>,
	Form(form): Form<ConfirmForm>,
) -> Result<Response, PanelError> {
	let user = require(&state, &headers, Some(&form.csrf), Permission::Admin).await?;
	if !form.confirmed {
		return Err(bad("Bot erişimini iptal etmeyi onaylayın."));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	let changed = query("UPDATE bots SET revoked=true WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.execute(&mut *tx)
		.await?
		.rows_affected();
	if changed == 0 {
		return Err(PanelError(StatusCode::NOT_FOUND, "Bot bulunamadı."));
	}
	query("UPDATE accounts SET active=false WHERE id=$1")
		.bind(id)
		.execute(&mut *tx)
		.await?;
	auth::audit(&mut tx, &user, "Bot erişimi iptal edildi", &id.to_string()).await?;
	tx.commit().await?;
	Ok(Redirect::to("/manage/bots").into_response())
}

fn json_text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
	let value = serde_json::Value::deserialize(deserializer)?;
	Ok(value
		.as_str()
		.map(str::to_owned)
		.unwrap_or_else(|| value.to_string()))
}
fn api_result(result: Result<Response, PanelError>) -> Response {
	match result{
		Err(PanelError(code,message))=>(code,axum::Json(json!({"error":message}))).into_response(),
		Ok(response) if response.status().is_redirection()=>(StatusCode::ACCEPTED,axum::Json(json!({"status":"accepted","location":response.headers().get(header::LOCATION).and_then(|v|v.to_str().ok())}))).into_response(),
		Ok(response) if response.headers().get(header::CONTENT_TYPE).and_then(|v|v.to_str().ok()).is_some_and(|v|v.starts_with("text/html"))=>(response.status(),axum::Json(if response.status().is_success(){json!({"status":"ok"})}else{json!({"error":"İstek tamamlanamadı."})})).into_response(),
		Ok(response)=>response
	}
}
async fn api_preflight(state: &WebState, headers: &HeaderMap) -> Result<(), PanelError> {
	if auth::bot_user(&state.store, headers).await?.is_none() {
		return Err(PanelError(
			StatusCode::UNAUTHORIZED,
			"Geçerli bot API anahtarı gerekli.",
		));
	}
	Ok(())
}
macro_rules! json_write {
	($name:ident,$form:ty,$handler:ident) => {
		async fn $name(
			State(state): State<WebState>,
			headers: HeaderMap,
			axum::Json(form): axum::Json<$form>,
		) -> Response {
			if let Err(error) = api_preflight(&state, &headers).await {
				return api_result(Err(error));
			}
			api_result($handler(State(state), headers, Form(form)).await)
		}
	};
	($name:ident,$id:ty,$form:ty,$handler:ident) => {
		async fn $name(
			State(state): State<WebState>,
			headers: HeaderMap,
			Path(id): Path<$id>,
			axum::Json(form): axum::Json<$form>,
		) -> Response {
			if let Err(error) = api_preflight(&state, &headers).await {
				return api_result(Err(error));
			}
			api_result($handler(State(state), headers, Path(id), Form(form)).await)
		}
	};
}
macro_rules! json_read {
	($name:ident,$id:ty,$handler:ident) => {
		async fn $name(
			State(state): State<WebState>,
			headers: HeaderMap,
			Path(id): Path<$id>,
		) -> Response {
			if let Err(error) = api_preflight(&state, &headers).await {
				return api_result(Err(error));
			}
			api_result($handler(State(state), headers, Path(id)).await)
		}
	};
}
json_write!(api_add_server, ServerForm, add_server);
json_write!(api_add_farm, FarmForm, add_farm);
json_write!(api_edit_server, Uuid, EditServerForm, edit_server);
json_write!(
	api_configuration,
	Uuid,
	ConfigurationForm,
	save_configuration
);
json_write!(api_routing, Uuid, RoutingForm, save_routing);
json_write!(api_certificates, Uuid, CertificateForm, upload_certificate);
json_write!(api_challenge, Uuid, ChallengeForm, set_challenge);
json_write!(api_publish, Uuid, PublishForm, publish_token);
json_write!(api_install, Uuid, InstallForm, start_installation);
json_write!(api_rotate, Uuid, RotateCredentialForm, rotate_credential);
json_write!(api_add_tokens, TokenForm, add_tokens);
json_write!(api_bulk_tokens, BulkForm, bulk_tokens);
json_write!(api_token_policy, Uuid, TokenPolicyForm, save_token_policy);
json_write!(
	api_token_details,
	Uuid,
	TokenDetailsForm,
	save_token_details
);
json_write!(api_access_code, Uuid, ConfirmForm, create_access_code);
json_write!(api_close_session, i64, CsrfForm, close_session);
json_write!(api_add_profile, ProfileForm, add_profile);
json_write!(api_edit_profile, Uuid, ProfileForm, edit_profile);
json_write!(
	api_add_server_profile,
	ServerProfileForm,
	add_server_profile
);
json_write!(
	api_edit_server_profile,
	Uuid,
	ServerProfileForm,
	edit_server_profile
);
json_write!(
	api_apply_server_profile,
	Uuid,
	ApplyServerProfileForm,
	apply_server_profile
);
json_write!(api_add_user, UserForm, add_user);
json_write!(api_edit_user, Uuid, EditUserForm, edit_user);
json_write!(api_tenant, FarmForm, update_tenant);
json_write!(api_integration, ConfirmForm, configure_ad_hook);
json_write!(api_add_bot, BotForm, create_bot);
json_write!(api_revoke_bot, Uuid, ConfirmForm, revoke_bot);
json_read!(api_get_configuration, Uuid, server_configuration);
json_read!(api_get_credential, Uuid, confirm_credential);
json_read!(api_get_key, Uuid, token_key);
json_read!(api_download_key, Uuid, download_token);
json_read!(api_install_status, Uuid, install_status);
macro_rules! json_list {
	($name:ident,$resource:literal) => {
		async fn $name(
			State(state): State<WebState>,
			headers: HeaderMap,
			Query(filter): Query<ApiListQuery>,
		) -> Response {
			api_list(
				State(state),
				headers,
				Path($resource.to_owned()),
				Query(filter),
			)
			.await
		}
	};
}
json_list!(api_servers, "servers");
json_list!(api_tokens, "tokens");
json_list!(api_sessions, "sessions");
json_list!(api_farms, "farms");
json_list!(api_profiles, "profiles");
json_list!(api_server_profiles, "server-profiles");
json_list!(api_users, "users");
json_list!(api_bots, "bots");
fn management_api() -> Router<WebState> {
	Router::new()
		.route("/servers", get(api_servers).post(api_add_server))
		.route("/servers/{id}", post(api_edit_server))
		.route(
			"/servers/{id}/configuration",
			get(api_get_configuration).post(api_configuration),
		)
		.route("/servers/{id}/routing", post(api_routing))
		.route("/servers/{id}/certificates", post(api_certificates))
		.route("/servers/{id}/challenge", post(api_challenge))
		.route("/servers/{id}/publish", post(api_publish))
		.route("/servers/{id}/installation", post(api_install))
		.route(
			"/servers/{id}/credential",
			get(api_get_credential).post(api_rotate),
		)
		.route("/install-jobs/{id}", get(api_install_status))
		.route("/farms", get(api_farms).post(api_add_farm))
		.route("/tokens", get(api_tokens).post(api_add_tokens))
		.route("/tokens/bulk", post(api_bulk_tokens))
		.route("/tokens/{id}/policy", post(api_token_policy))
		.route("/tokens/{id}/details", post(api_token_details))
		.route("/tokens/{id}/access-code", post(api_access_code))
		.route("/tokens/{id}/key", get(api_get_key))
		.route("/tokens/{id}/download", get(api_download_key))
		.route("/sessions", get(api_sessions))
		.route("/sessions/{id}/close", post(api_close_session))
		.route("/profiles", get(api_profiles).post(api_add_profile))
		.route("/profiles/{id}", post(api_edit_profile))
		.route(
			"/server-profiles",
			get(api_server_profiles).post(api_add_server_profile),
		)
		.route("/server-profiles/{id}", post(api_edit_server_profile))
		.route(
			"/server-profiles/{id}/apply",
			post(api_apply_server_profile),
		)
		.route("/users", get(api_users).post(api_add_user))
		.route("/users/{id}", post(api_edit_user))
		.route("/tenant", post(api_tenant))
		.route("/integrations", post(api_integration))
		.route("/bots", get(api_bots).post(api_add_bot))
		.route("/bots/{id}/revoke", post(api_revoke_bot))
}
#[derive(Deserialize)]
struct ApiListQuery {
	#[serde(default)]
	p: i64,
}
async fn api_list(
	State(state): State<WebState>,
	headers: HeaderMap,
	Path(resource): Path<String>,
	Query(filter): Query<ApiListQuery>,
) -> Response {
	api_result(api_list_result(state, headers, resource, filter).await)
}
async fn api_list_result(
	state: WebState,
	headers: HeaderMap,
	resource: String,
	filter: ApiListQuery,
) -> Result<Response, PanelError> {
	api_preflight(&state, &headers).await?;
	if !(0..=10000).contains(&filter.p) {
		return Err(bad("Arama veya sayfa numarası geçersiz."));
	}
	let user = require(
		&state,
		&headers,
		None,
		if ["users", "bots"].contains(&resource.as_str()) {
			Permission::Admin
		} else {
			Permission::Read
		},
	)
	.await?;
	let mut tx = state.store.begin(user.tenant).await?;
	let query = match resource.as_str() {
		"servers" => {
			"SELECT jsonb_build_object('id',id,'name',name,'farm_id',farm_id,'paused',paused,'last_seen',last_seen,'config_code',config_code,'management',management,'status',status) AS item FROM servers WHERE tenant_id=$1 ORDER BY id LIMIT 101 OFFSET $2"
		}
		"tokens" => {
			"SELECT jsonb_build_object('id',id,'name',name,'server_id',server_id,'farm_id',farm_id,'profile_id',profile_id,'policy',policy,'revoked',revoked,'sent',sent,'received',received,'cycle_sent',cycle_sent,'cycle_received',cycle_received,'expires_at',expires_at,'max_clients',max_clients,'max_traffic',max_traffic,'description',description,'group_name',group_name,'support_id',support_id,'customer_id',customer_id,'order_id',order_id) AS item FROM access_tokens WHERE tenant_id=$1 ORDER BY id LIMIT 101 OFFSET $2"
		}
		"sessions" => {
			"SELECT jsonb_build_object('id',id,'server_id',server_id,'token_id',token_id,'created_at',created_at,'closed',closed,'decision',decision,'expires_at',expires_at,'country',metadata->>'clientCountry') AS item FROM sessions WHERE tenant_id=$1 ORDER BY id DESC LIMIT 101 OFFSET $2"
		}
		"profiles" => {
			"SELECT jsonb_build_object('id',id,'name',name,'is_public',is_public,'policy',policy) AS item FROM access_profiles WHERE tenant_id=$1 ORDER BY id LIMIT 101 OFFSET $2"
		}
		"server-profiles" => {
			"SELECT jsonb_build_object('id',id,'name',name,'settings',settings,'management',management,'version',version) AS item FROM server_profiles WHERE tenant_id=$1 ORDER BY id LIMIT 101 OFFSET $2"
		}
		"farms" => {
			"SELECT jsonb_build_object('id',id,'name',name,'management',management) AS item FROM farms WHERE tenant_id=$1 ORDER BY id LIMIT 101 OFFSET $2"
		}
		"bots" => {
			"SELECT jsonb_build_object('id',b.id,'name',b.name,'role',m.role,'revoked',b.revoked,'expires_at',b.expires_at) AS item FROM bots b JOIN memberships m ON m.tenant_id=b.tenant_id AND m.account_id=b.id WHERE b.tenant_id=$1 ORDER BY b.id LIMIT 101 OFFSET $2"
		}
		"users" => {
			"SELECT jsonb_build_object('id',a.id,'email',a.email,'active',a.active,'role',m.role) AS item FROM memberships m JOIN accounts a ON a.id=m.account_id WHERE m.tenant_id=$1 AND NOT EXISTS(SELECT FROM bots b WHERE b.tenant_id=m.tenant_id AND b.id=a.id) ORDER BY a.id LIMIT 101 OFFSET $2"
		}
		_ => return Err(PanelError(StatusCode::NOT_FOUND, "API kaynağı bulunamadı.")),
	};
	let mut rows = sqlx_core::query::query(query)
		.bind(user.tenant)
		.bind(filter.p * 100)
		.fetch_all(&mut *tx)
		.await?
		.into_iter()
		.map(|r| r.get::<serde_json::Value, _>("item"))
		.collect::<Vec<_>>();
	let more = rows.len() > 100;
	rows.truncate(100);
	Ok(axum::Json(json!({"items":rows,"page":filter.p,"hasMore":more})).into_response())
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn spreadsheet_formula_is_neutralized() {
		assert_eq!(csv_cell("=SUM(1,2)"), "\"'=SUM(1,2)\"");
		assert_eq!(csv_cell("a\"b"), "\"a\"\"b\"");
	}
}
