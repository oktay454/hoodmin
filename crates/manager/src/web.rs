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
		.route("/manage/farms", post(add_farm))
		.route("/manage/servers", post(add_server))
		.route("/manage/servers/{id}", post(edit_server))
		.route(
			"/manage/servers/{id}/credential",
			get(confirm_credential).post(rotate_credential),
		)
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
		.layer(DefaultBodyLimit::max(64 * 1024))
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
	if let Some(csrf) = csrf {
		check_origin(state, headers)?;
		if !auth::same_token(csrf, &user.csrf) {
			return Err(PanelError(
				StatusCode::FORBIDDEN,
				"Form süresi doldu. Sayfayı yenileyin.",
			));
		}
	}
	if matches!(permission, Permission::Write) && !user.can_write()
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
	let settings=serde_json::to_string_pretty(&json!({"HttpAccessManager":{"BaseUrl":state.origin,"Authorization":format!("Bearer {credential}")},"AllowAnonymousTracker":false,"LogLevel":"Information"})).map_err(anyhow::Error::from)?;
	Ok(views::secret(&user,"Sunucu bağlantısı hazır","Aşağıdaki ayarları sunucunun storage/appsettings.json dosyasına kaydedin. Kimlik bilgisi yalnızca bu ekranda gösterilir.",&settings).into_response())
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
	Ok(views::confirm_credential(&user, id, &row.get::<String, _>("name"), &proof).into_response())
}
#[derive(Deserialize)]
struct RotateCredentialForm {
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
	query("UPDATE servers SET credential_hash=$3 WHERE tenant_id=$1 AND id=$2")
		.bind(user.tenant)
		.bind(id)
		.bind(Sha256::digest(credential.as_bytes()).to_vec())
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
	let settings=serde_json::to_string_pretty(&json!({"HttpAccessManager":{"BaseUrl":state.origin,"Authorization":format!("Bearer {credential}")},"AllowAnonymousTracker":false,"LogLevel":"Information"})).map_err(anyhow::Error::from)?;
	Ok(views::secret(
		&user,
		"Yeni sunucu bağlantı ayarı",
		"Önceki kimlik geçersizleştirildi. Aşağıdaki ayarı sunucunuza uygulayın.",
		&settings,
	)
	.into_response())
}

#[derive(Deserialize)]
struct EditServerForm {
	csrf: String,
	name: String,
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
#[derive(Deserialize)]
struct TokenForm {
	csrf: String,
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
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
	let name = management::valid_name(&form.name).map_err(|_| bad("Anahtar adı geçersiz."))?;
	if !(1..=100).contains(&form.count)
		|| !(1..=10000).contains(&form.max_clients)
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
	let text = keys
		.into_iter()
		.map(|(_, name, key)| format!("{name}\n{key}"))
		.collect::<Vec<_>>()
		.join("\n\n");
	Ok(views::secret(
		&user,
		"Erişim anahtarları hazır",
		"Anahtarları güvenli biçimde saklayıp kullanıcılarınızla paylaşın. Bu sayfa önbelleğe alınmaz.",
		&text,
	)
	.into_response())
}
#[derive(Deserialize)]
struct BulkForm {
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
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
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
			if form.max_gib > 1_000_000 || !(1..=10000).contains(&form.max_clients) {
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
	let user = require(&state, &headers, Some(&form.csrf), Permission::Write).await?;
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
	if !["admin", "operator", "viewer"].contains(&form.role.as_str()) {
		return Err(bad("Rol geçersiz."));
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
	if !["admin", "operator", "viewer"].contains(&form.role.as_str()) {
		return Err(bad("Rol geçersiz."));
	}
	let mut tx = state.store.begin(user.tenant).await?;
	query("SELECT id FROM tenants WHERE id=$1 FOR UPDATE")
		.bind(user.tenant)
		.fetch_one(&mut *tx)
		.await?;
	let target=query("SELECT m.role,a.active FROM memberships m JOIN accounts a ON a.id=m.account_id WHERE m.tenant_id=$1 AND m.account_id=$2").bind(user.tenant).bind(id).fetch_optional(&mut *tx).await?.ok_or(PanelError(StatusCode::NOT_FOUND,"Kullanıcı bulunamadı."))?;
	if target.get::<String, _>("role") == "admin"
		&& target.get::<bool, _>("active")
		&& (!form.active || form.role != "admin")
	{
		let admins:i64=query_scalar("SELECT count(*) FROM memberships m JOIN accounts a ON a.id=m.account_id WHERE m.tenant_id=$1 AND m.role='admin' AND a.active").bind(user.tenant).fetch_one(&mut *tx).await?;
		if admins <= 1 {
			return Err(bad(
				"Kiracının son etkin yöneticisi devre dışı bırakılamaz.",
			));
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
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn spreadsheet_formula_is_neutralized() {
		assert_eq!(csv_cell("=SUM(1,2)"), "\"'=SUM(1,2)\"");
		assert_eq!(csv_cell("a\"b"), "\"a\"\"b\"");
	}
}
