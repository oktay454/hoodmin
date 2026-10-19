use crate::{
	auth::User,
	i18n::{self, tr},
	store::Store,
};
use anyhow::Result;
use axum::response::Html;
use chrono::{DateTime, Utc};
use leptos::prelude::*;
use serde_json::{Value, json};
use sqlx_core::{query::query, query_scalar::query_scalar, row::Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct Farm {
	pub id: String,
	pub name: String,
}
pub struct Server {
	pub id: String,
	pub name: String,
	pub host: String,
	pub port: u16,
	pub farm: String,
	pub group: Option<String>,
	pub status: String,
	pub online: bool,
	pub paused: bool,
	pub cpu: String,
	pub memory: String,
	pub sessions: i64,
	pub seen: String,
}
pub struct Token {
	pub id: String,
	pub name: String,
	pub server: String,
	pub traffic: String,
	pub quota: String,
	pub state: String,
	pub clients: i32,
	pub expires: String,
	pub revoked: bool,
}
pub struct Session {
	pub id: i64,
	pub token: String,
	pub server: String,
	pub created: String,
	pub closed: bool,
}
pub struct Account {
	pub id: String,
	pub email: String,
	pub role: String,
	pub active: bool,
}
pub struct Audit {
	pub time: String,
	pub email: String,
	pub action: String,
	pub resource: String,
}
pub struct Profile {
	pub id: Uuid,
	pub name: String,
	pub is_public: bool,
	pub policy: Value,
}
pub struct Data {
	pub profiles: Vec<Profile>,
	pub stats: Value,
	pub servers: Vec<Server>,
	pub farms: Vec<Farm>,
	pub tokens: Vec<Token>,
	pub sessions: Vec<Session>,
	pub users: Vec<Account>,
	pub audit: Vec<Audit>,
	pub query: String,
	pub page: usize,
	pub more: bool,
}
pub fn bytes(value: u128) -> String {
	let mut size = value as f64;
	let mut unit = "B";
	for next in ["KiB", "MiB", "GiB", "TiB", "PiB"] {
		if size < 1024.0 {
			break;
		}
		size /= 1024.0;
		unit = next;
	}
	if unit == "B" {
		format!("{value} B")
	} else {
		format!("{size:.1} {unit}")
	}
}
fn time(value: Option<DateTime<Utc>>) -> String {
	value
		.map(|v| {
			v.with_timezone(&chrono::FixedOffset::east_opt(10800).expect("Istanbul offset"))
				.format("%d.%m.%Y %H:%M")
				.to_string()
		})
		.unwrap_or("Henüz bağlanmadı".into())
}
pub async fn stats(store: &Store, tenant: Uuid) -> Result<Value> {
	let mut tx = store.begin(tenant).await?;
	let row=query("SELECT count(*) AS total,count(*) FILTER(WHERE last_seen>now()-make_interval(secs=>heartbeat_seconds) AND NOT paused) AS online FROM servers WHERE tenant_id=$1").bind(tenant).fetch_one(&mut *tx).await?;
	let tokens=query("SELECT count(*) AS total,coalesce(sum(sent::numeric+received::numeric),0)::text AS traffic FROM access_tokens WHERE tenant_id=$1").bind(tenant).fetch_one(&mut *tx).await?;
	let sessions:i64=query_scalar("SELECT count(*) FROM sessions WHERE tenant_id=$1 AND NOT closed AND last_seen>now()-interval '30 minutes'").bind(tenant).fetch_one(&mut *tx).await?;
	Ok(
		json!({"servers":row.get::<i64,_>("total"),"online":row.get::<i64,_>("online"),"tokens":tokens.get::<i64,_>("total"),"activeSessions":sessions,"traffic":bytes(tokens.get::<String,_>("traffic").parse::<u128>().unwrap_or(0))}),
	)
}
pub async fn load(
	store: &Store,
	user: &User,
	search: &str,
	page: usize,
	current: &str,
) -> Result<Data> {
	let stats = stats(store, user.tenant).await?;
	let mut tx = store.begin(user.tenant).await?;
	let needle = search.to_lowercase();
	let rows=query("SELECT s.id,s.name,s.farm_id,s.paused,s.last_seen,s.heartbeat_seconds,s.status,s.configuration,f.name AS group_name FROM servers s LEFT JOIN farms f ON f.tenant_id=s.tenant_id AND f.id=s.farm_id WHERE s.tenant_id=$1 ORDER BY s.name LIMIT 500").bind(user.tenant).fetch_all(&mut *tx).await?;
	let servers = rows
		.into_iter()
		.filter(|r| {
			current != "servers" || r.get::<String, _>("name").to_lowercase().contains(&needle)
		})
		.map(|r| -> Result<Server> {
			let paused: bool = r.get("paused");
			let seen: Option<DateTime<Utc>> = r.get("last_seen");
			let status: Option<Value> = r.get("status");
			let online = seen.is_some_and(|s| {
				s > Utc::now()
					- chrono::Duration::seconds(i64::from(r.get::<i32, _>("heartbeat_seconds")))
			}) && !paused;
			let error = status
				.as_ref()
				.and_then(|v| v["configError"].as_str())
				.is_some_and(|e| !e.is_empty());
			let (host, port) = if current == "servers" && user.can_write() {
				let id: Uuid = r.get("id");
				let bytes = store.vault.open(
					&r.get::<Vec<u8>, _>("configuration"),
					&format!("{}:{id}:config", user.tenant),
				)?;
				crate::management::endpoint(&serde_json::from_slice::<Value>(&bytes)?)?
			} else {
				(String::new(), 443)
			};
			Ok(Server {
				id: r.get::<Uuid, _>("id").to_string(),
				name: r.get("name"),
				host,
				port,
				farm: r
					.get::<Option<Uuid>, _>("farm_id")
					.map(|v| v.to_string())
					.unwrap_or_default(),
				group: r.get::<Option<String>, _>("group_name"),
				status: if paused {
					"Bakım"
				} else if error {
					"Yapılandırma hatası"
				} else if online {
					"Çevrimiçi"
				} else if seen.is_some() {
					"Çevrimdışı"
				} else {
					"Bağlantı bekliyor"
				}
				.into(),
				online,
				paused,
				cpu: status
					.as_ref()
					.and_then(|v| v["cpuUsage"].as_i64())
					.map(|v| format!("%{v}"))
					.unwrap_or("—".into()),
				memory: status
					.as_ref()
					.and_then(|v| v["usedMemory"].as_u64())
					.map(|v| bytes(v.into()))
					.unwrap_or("—".into()),
				sessions: status
					.as_ref()
					.and_then(|v| v["sessionCount"].as_i64())
					.unwrap_or(0),
				seen: time(seen),
			})
		})
		.collect::<Result<Vec<_>>>()?;
	let farms = query("SELECT id,name FROM farms WHERE tenant_id=$1 ORDER BY name LIMIT 500")
		.bind(user.tenant)
		.fetch_all(&mut *tx)
		.await?
		.into_iter()
		.map(|r| Farm {
			id: r.get::<Uuid, _>("id").to_string(),
			name: r.get("name"),
		})
		.collect();
	let mut token_rows=query("SELECT t.id,t.name,t.sent,t.received,t.max_traffic,t.max_clients,t.expires_at,t.revoked,s.name AS server FROM access_tokens t JOIN servers s ON s.tenant_id=t.tenant_id AND s.id=t.server_id WHERE t.tenant_id=$1 AND ($2='' OR t.name ILIKE '%'||$2||'%') ORDER BY t.name,t.id LIMIT 101 OFFSET $3").bind(user.tenant).bind(search).bind(((page-1)*100)as i64).fetch_all(&mut *tx).await?;
	let more = token_rows.len() > 100;
	token_rows.truncate(100);
	let tokens = token_rows
		.into_iter()
		.map(|r| Token {
			id: r.get::<Uuid, _>("id").to_string(),
			name: r.get("name"),
			server: r.get("server"),
			traffic: bytes(r.get::<i64, _>("sent") as u128 + r.get::<i64, _>("received") as u128),
			quota: if r.get::<i64, _>("max_traffic") == 0 {
				"Sınırsız".into()
			} else {
				bytes(r.get::<i64, _>("max_traffic") as u128)
			},
			state: if r.get::<bool, _>("revoked") {
				"İptal"
			} else if r
				.get::<Option<DateTime<Utc>>, _>("expires_at")
				.is_some_and(|e| e <= Utc::now())
			{
				"Süresi doldu"
			} else if r.get::<i64, _>("max_traffic") > 0
				&& i128::from(r.get::<i64, _>("sent")) + i128::from(r.get::<i64, _>("received"))
					>= i128::from(r.get::<i64, _>("max_traffic"))
			{
				"Kota doldu"
			} else {
				"Etkin"
			}
			.into(),
			clients: r.get("max_clients"),
			expires: r
				.get::<Option<DateTime<Utc>>, _>("expires_at")
				.map(|v| {
					v.with_timezone(&chrono::FixedOffset::east_opt(10800).expect("Istanbul offset"))
						.format("%d.%m.%Y")
						.to_string()
				})
				.unwrap_or("Süresiz".into()),
			revoked: r.get("revoked"),
		})
		.collect();
	let sessions=query("SELECT x.id,x.created_at,x.closed,t.name AS token,s.name AS server FROM sessions x JOIN access_tokens t ON t.tenant_id=x.tenant_id AND t.id=x.token_id JOIN servers s ON s.tenant_id=x.tenant_id AND s.id=x.server_id WHERE x.tenant_id=$1 ORDER BY x.created_at DESC LIMIT 200").bind(user.tenant).fetch_all(&mut *tx).await?.into_iter().map(|r|Session{id:r.get("id"),token:r.get("token"),server:r.get("server"),created:time(Some(r.get("created_at"))),closed:r.get("closed")}).collect();
	let users = if user.is_admin() {
		query("SELECT a.id,a.email,a.active,m.role FROM memberships m JOIN accounts a ON a.id=m.account_id WHERE m.tenant_id=$1 AND NOT EXISTS(SELECT FROM bots b WHERE b.tenant_id=m.tenant_id AND b.id=a.id) ORDER BY a.email LIMIT 200").bind(user.tenant).fetch_all(&mut *tx).await?.into_iter().map(|r|Account{id:r.get::<Uuid,_>("id").to_string(),email:r.get("email"),role:r.get("role"),active:r.get("active")}).collect()
	} else {
		Vec::new()
	};
	let audit=query("SELECT created_at,actor_email,action,resource FROM audit_events WHERE tenant_id=$1 ORDER BY id DESC LIMIT 200").bind(user.tenant).fetch_all(&mut *tx).await?.into_iter().map(|r|Audit{time:time(Some(r.get("created_at"))),email:r.get("actor_email"),action:r.get("action"),resource:r.get("resource")}).collect();
	let profiles = query(
		"SELECT id,name,is_public,policy FROM access_profiles WHERE tenant_id=$1 ORDER BY name LIMIT 500",
	)
	.bind(user.tenant)
	.fetch_all(&mut *tx)
	.await?
	.into_iter()
	.map(|r| Profile {
		id: r.get("id"),
		name: r.get("name"),
		is_public: r.get("is_public"),
		policy: r.get("policy"),
	})
	.collect();
	Ok(Data {
		profiles,
		stats,
		servers,
		farms,
		tokens,
		sessions,
		users,
		audit,
		query: search.into(),
		page,
		more,
	})
}
fn icon(name: &str) -> AnyView {
	let path = match name {
		"servers" => "M3 3h18v7H3z M3 14h18v7H3z M6 6h.01 M6 17h.01 M10 6h8 M10 17h8",
		"tokens" => "M15 3a6 6 0 1 1-4.8 9.6L3 20v1h4v-3h3v-3l1.4-1.4 M16 7h.01",
		"sessions" => "M2 12h4l3-8 6 16 3-8h4",
		"audit" => "M12 3 3 7v6c0 4 5 7 9 9 4-2 9-5 9-9V7z M8 12l3 3 5-6",
		"users" => {
			"M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8 M20 8v6 M17 11h6"
		}
		"account" => "M20 21a8 8 0 0 0-16 0 M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8",
		_ => "M3 11 12 3l9 8 M5 10v11h14V10 M9 21v-7h6v7",
	};
	view!{<svg class="ui-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d=path/></svg>}.into_any()
}
fn language_switch() -> AnyView {
	let locale = i18n::locale();
	view!{<form method="get" action="/language" class="language-switch"><label><span class="language-label">{tr("Dil")}</span><select name="lang" aria-label=tr("Dil")><option value="tr" lang="tr" selected={locale==i18n::Locale::Tr}>{tr("Türkçe")}</option><option value="en" lang="en" selected={locale==i18n::Locale::En}>{tr("English")}</option><option value="ar" lang="ar" selected={locale==i18n::Locale::Ar}>{tr("العربية")}</option></select></label><input type="hidden" name="return_to" value=i18n::return_to()/><button class="button secondary language-apply" type="submit" aria-label=tr("Dili değiştir")>{tr("↵")}</button></form>}.into_any()
}
fn theme_switch() -> AnyView {
	let light = i18n::theme() == i18n::Theme::Light;
	let label = tr(if light {
		"Koyu temaya geç"
	} else {
		"Açık temaya geç"
	});
	view!{<form method="get" action="/theme" class="theme-switch"><input type="hidden" name="theme" value=if light {"dark"} else {"light"}/><input type="hidden" name="return_to" value=i18n::return_to()/><button class="button theme-toggle" type="submit" aria-label=label.clone() title=label><svg class="sun-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2 M12 20v2 M2 12h2 M20 12h2 M5 5l1.5 1.5 M17.5 17.5L19 19 M5 19l1.5-1.5 M17.5 6.5L19 5"/></svg><svg class="moon-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20.5 13a8.6 8.6 0 0 1-9.5-9.5A8.6 8.6 0 1 0 20.5 13Z"/></svg></button><span class="theme-feedback" role="status" aria-live="polite"></span></form>}.into_any()
}
fn help(title: &str, body: &str) -> AnyView {
	view!{<button class="help-trigger" type="button" aria-haspopup="dialog" aria-label=format!("{}: {}",tr("Yardım"),tr(title)) title=tr("Açıklamayı aç") data-help-title=tr(title) data-help-body=tr(body)>{tr("?")}</button>}.into_any()
}
fn dialogs() -> AnyView {
	view!{
		<dialog id="help-dialog" class="modal" aria-labelledby="help-title" aria-describedby="help-body"><div class="modal-caption">{tr("KISA REHBER")}</div><h2 id="help-title"></h2><p id="help-body"></p><form method="dialog" class="modal-actions"><button class="button secondary" autofocus>{tr("Kapat")}</button></form></dialog>
		<dialog id="confirm-dialog" class="modal" aria-labelledby="confirm-title" aria-describedby="confirm-body"><div class="modal-caption warning-text">{tr("İŞLEM ONAYI")}</div><h2 id="confirm-title">{tr("Devam etmek istiyor musunuz?")}</h2><p id="confirm-body"></p><div class="modal-actions"><button id="confirm-cancel" class="button secondary" type="button" autofocus>{tr("Vazgeç")}</button><button id="confirm-proceed" class="button danger" type="button">{tr("Onayla ve uygula")}</button></div></dialog>
	}.into_any()
}
fn document(title: &str, content: AnyView) -> Html<String> {
	let title = format!("{} · {}", tr(title), tr("Hoodmin"));
	Html(format!("<!doctype html>{}",view!{
		<html lang=i18n::locale().code() dir=i18n::locale().direction() data-theme=i18n::theme().code() data-theme-light=tr("Açık temaya geç") data-theme-dark=tr("Koyu temaya geç") data-theme-error=tr("Tema kaydedilemedi. Yeniden deneyin.") data-copy-success=tr("Kopyalandı.") data-copy-fallback=tr("Metin seçildi. Kopyalama kısayolunu kullanabilirsiniz.")><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width,initial-scale=1"/><meta name="color-scheme" content=i18n::theme().code()/><title>{title}</title><link rel="stylesheet" href="/assets/app.css"/><script src="/assets/app.js" defer></script></head><body>{content}{dialogs()}</body></html>
	}.to_html()))
}
fn csrf(value: &str) -> AnyView {
	view! {<input type="hidden" name="csrf" value=value.to_owned()/>}.into_any()
}
fn layout(user: &User, title: &str, active: &str, content: AnyView) -> Html<String> {
	let user = user.clone();
	let title = tr(title);
	let active = active.to_owned();
	let navigation = [
		("overview", "Genel bakış", "/panel"),
		("servers", "Sunucular", "/panel/servers"),
		("tokens", "Erişim anahtarları", "/panel/tokens"),
		("sessions", "VPN oturumları", "/panel/sessions"),
		("reports", "Kullanım raporları", "/reports"),
		("audit", "Denetim kaydı", "/panel/audit"),
	];
	let items=navigation.into_iter().map(|(id,label,url)|view!{<a href=url class:active={active==id}>{icon(id)}<span>{tr(label)}</span></a>}).collect_view();
	document(&title.clone(),view!{
		<div class="app-shell"><aside class="sidebar"><a class="brand" href="/panel"><span class="brand-mark">{tr("HM")}</span><span>{tr("Hoodmin")}<small>{tr("VPN yönetimi")}</small></span></a><div class="workspace"><span class="eyebrow">{tr("ÇALIŞMA ALANI")}</span><strong>{user.tenant_name.clone()}</strong><span class="small muted">{tr("VPN sunucularınız, tek panel.")}</span></div><nav aria-label=tr("Ana gezinme")><span class="nav-section">{tr("VPN VE ERİŞİM")}</span>{items}{user.is_admin().then(||view!{<a href="/panel/users" class:active={active=="users"}>{icon("users")}<span>{tr("Panel kullanıcıları")}</span></a>})}</nav><div class="sidebar-foot"><span class="connection-dot"></span><span>{tr("VPN ortamınızın yönetimi")}</span></div></aside><div class="main-shell"><header class="topbar"><div><span class="eyebrow">{tr("KONTROL PANELİ")}</span><h1>{title}</h1></div><div class="profile"><a class="account-link" href="/panel/account">{icon("account")}<span><strong>{tr("Hesabım")}</strong><bdi dir="ltr">{user.email.clone()}</bdi></span><span aria-hidden="true">{tr("↗")}</span></a>{theme_switch()}{language_switch()}<span class="role-pill">{role_label(&user.role)}</span><form method="post" action="/logout">{csrf(&user.csrf)}<button class="button ghost" type="submit">{tr("Çıkış")}</button></form></div></header><main><div class="page-content">{content}</div></main><footer>{tr("Hoodmin · VPN yönetimi · Saatler İstanbul (UTC+03)")}</footer></div></div>
	}.into_any())
}
fn role_label(role: &str) -> String {
	tr(match role {
		"owner" => "Sahip",
		"token_writer" => "Erişim anahtarı yazarı",
		"admin" => "Yönetici",
		"operator" => "Operatör",
		_ => "Salt okunur",
	})
}
pub fn signin(token: &str, error: Option<&str>) -> Html<String> {
	document("Giriş",view!{<div class="login-shell"><section class="login-intro"><span class="brand-mark large">{tr("HM")}</span><span class="eyebrow">{tr("HOODMIN")}</span><h1>{tr("VPN erişimini tek yerden yönetin.")}</h1><p>{tr("VPN sunucularını izleyin; bağlantı, erişim anahtarı ve trafik kotalarını yönetin.")}</p><div class="intro-points"><span>{tr("01 · Kiracınıza özel çalışma alanı")}</span><span>{tr("02 · Toplu erişim yönetimi")}</span><span>{tr("03 · Canlı VPN sunucusu durumu")}</span></div></section><section class="login-card"><div class="appearance-controls">{theme_switch()}{language_switch()}</div><span class="eyebrow">{tr("HOŞ GELDİNİZ")}</span><h2>{tr("Çalışma alanınıza giriş yapın")}</h2><p class="muted">{tr("Yöneticinizin oluşturduğu hesabı kullanın.")}</p>{error.map(|message|view!{<p class="alert error" role="alert">{tr(message)}</p>})}<form method="post" action="/signin" class="stack">{csrf(token)}<label for="email">{tr("E-posta")}</label><input id="email" type="email" name="email" autocomplete="username" required maxlength="254"/><label for="password">{tr("Parola")}</label><input id="password" type="password" name="password" autocomplete="current-password" required maxlength="128"/><button class="button primary" type="submit">{tr("Giriş yap")}<span>{tr("→")}</span></button></form><p class="small muted login-note">{tr("Hesabınıza erişemiyorsanız çalışma alanı yöneticinizle iletişime geçin.")}</p></section></div>}.into_any())
}
pub fn error(message: &str) -> Html<String> {
	document("İşlem tamamlanamadı",view!{<main class="error-page"><div class="appearance-controls">{theme_switch()}{language_switch()}</div><span class="brand-mark">{tr("HM")}</span><h1>{tr("İşlem tamamlanamadı")}</h1><p>{tr(message)}</p><a class="button primary" href="/panel">{tr("Panele dön")}</a></main>}.into_any())
}
pub fn secret(user: &User, title: &str, description: &str, value: &str) -> Html<String> {
	layout(user,title,"",view!{<section class="card secret-card"><span class="eyebrow">{tr("GÜVENLİ PAYLAŞIM")}</span><h2>{tr(title)}</h2><p class="muted">{tr(description)}</p><textarea id="secret-output" class="secret-output" readonly rows="14" aria-label=tr("Oluşturulan bağlantı bilgileri")>{value.to_owned()}</textarea><div class="actions"><button class="button primary" type="button" data-copy="secret-output">{tr("Kopyala")}</button><a class="button secondary" href="/panel">{tr("Panele dön")}</a></div><p class="small muted" id="copy-feedback" aria-live="polite"></p></section>}.into_any())
}
fn stat(
	label: &'static str,
	name: &'static str,
	value: String,
	detail: &'static str,
) -> impl IntoView {
	view! {<div class="stat"><span class="eyebrow">{tr(label)}</span><strong data-stat=name>{value}</strong><span class="small muted">{tr(detail)}</span></div>}
}
pub fn panel(page: &str, user: &User, data: Data, notice: &str) -> Html<String> {
	let title = match page {
		"servers" => "Sunucular",
		"tokens" => "Erişim anahtarları",
		"sessions" => "VPN oturumları",
		"users" => "Panel kullanıcıları",
		"audit" => "Denetim kaydı",
		"account" => "Hesabım",
		_ => "Genel bakış",
	};
	let content = match page {
		"servers" => servers(user, data),
		"tokens" => tokens(user, data),
		"sessions" => sessions(user, data),
		"users" => users(user, data),
		"audit" => audit(data),
		"account" => account(user),
		_ => overview(user, data),
	};
	layout(user,title,page,view!{<div class="panel-body">{(notice=="saved").then(||view!{<div class="alert success" role="status">{tr("Değişiklikler kaydedildi.")}</div>})}{content}</div>}.into_any())
}
fn overview(user: &User, data: Data) -> AnyView {
	let counters = vec![
		stat(
			"SUNUCULAR",
			"servers",
			data.stats["servers"].to_string(),
			"Çalışma alanınıza kayıtlı",
		),
		stat(
			"ÇEVRİMİÇİ",
			"online",
			data.stats["online"].to_string(),
			"Bildirim aralığında yanıt verdi",
		),
		stat(
			"AKTİF OTURUMLAR",
			"activeSessions",
			data.stats["activeSessions"].to_string(),
			"Son 30 dakikada etkin",
		),
		stat(
			"TOPLAM TRAFİK",
			"traffic",
			data.stats["traffic"].as_str().unwrap_or("0 B").into(),
			"Bildirilen toplam kullanım",
		),
	]
	.into_iter()
	.collect_view();
	let no_servers = data.servers.is_empty();
	let rows=data.servers.into_iter().take(6).map(|server|view!{
		<tr><td><strong>{server.name}</strong><span class="table-sub">{server.group.unwrap_or_else(||tr("Grupsuz"))}</span></td><td><span class="badge" class:online=server.online>{tr(&server.status)}</span></td><td><bdi dir="ltr">{server.cpu}</bdi></td><td><bdi dir="ltr">{server.memory}</bdi></td><td><bdi dir="ltr">{server.sessions}</bdi></td><td><bdi dir="auto">{tr(&server.seen)}</bdi></td></tr>
	}).collect_view();
	view!{
		<div class="overview-toolbar"><div><h2>{tr("Çalışma alanı özeti")}</h2><p class="muted">{tr("Sunucularınızın durumunu ve çalışma alanınızın kullanımını buradan izleyin.")}</p></div><a class="button primary" href="/panel/servers">{icon("servers")}{tr(if user.can_write(){"Sunucuları yönet"}else{"Sunucuları görüntüle"})}</a></div>
		<section class="stats-grid" aria-label=tr("Altyapı özeti")>{counters}</section>
		<div class="dashboard-grid"><section class="card status-panel"><header class="panel-heading"><h2>{tr("Sunucu durumu")}{help("VPN sunucusu yönetimi", "Her sunucu yalnız kendi çalışma alanınıza aittir. Önce sunucuyu kaydedin, ardından oluşturulan bağlantı ayarını VPN sunucusuna uygulayın. Çevrimiçi durumu en az 90 saniyelik toleransla, yapılandırılan bildirim aralığının üç katına göre belirlenir.")}</h2><a class="text-link" href="/panel/servers">{tr("Tümünü görüntüle →")}</a></header><div class="table-wrap"><table><thead><tr><th>{tr("Sunucu")}</th><th>{tr("Durum")}</th><th>{tr("CPU")}</th><th>{tr("Bellek")}</th><th>{tr("Oturum")}</th><th>{tr("Son bildirim")}</th></tr></thead><tbody>{rows}</tbody></table></div>{no_servers.then(||view!{<p class="empty-state">{tr("Henüz sunucunuz yoksa Sunucular sayfasından ekleyebilirsiniz.")}</p>})}<div class="panel-note">{tr("VPN trafiği panelden geçmez; istemci doğrudan VPN sunucunuza bağlanır.")}</div></section>
		<aside class="card quick-actions"><header class="panel-heading"><h2>{tr("Hızlı erişim")}</h2></header><a class="action-tile" href="/panel/servers">{icon("servers")}<span><strong>{tr("Sunucular")}</strong><small>{tr("Bağlantı, durum ve bakım ayarları")}</small></span><span class="action-arrow" aria-hidden="true">"↗"</span></a><a class="action-tile" href="/panel/tokens">{icon("tokens")}<span><strong>{tr("Erişim anahtarları")}</strong><small>{tr("Toplu üretim, kota ve erişim yönetimi")}</small></span><span class="action-arrow" aria-hidden="true">"↗"</span></a><a class="action-tile" href="/panel/sessions">{icon("sessions")}<span><strong>{tr("VPN oturumları")}</strong><small>{tr("Bağlantıları ve oturum geçmişini inceleyin")}</small></span><span class="action-arrow" aria-hidden="true">"↗"</span></a><a class="action-tile" href="/panel/audit">{icon("audit")}<span><strong>{tr("Denetim kaydı")}</strong><small>{tr("Yapılan yönetim işlemlerini izleyin")}</small></span><span class="action-arrow" aria-hidden="true">"↗"</span></a></aside></div>
	}.into_any()
}
fn farm_select(farms: Vec<Farm>, selected: String) -> impl IntoView {
	view! {<select name="farm" aria-label=tr("Sunucu grubu")><option value="">{tr("Grupsuz")}</option>{farms.into_iter().map(|f|view!{<option value=f.id.clone() selected={selected==f.id}>{f.name}</option>}).collect_view()}</select>}
}
fn servers(user: &User, data: Data) -> AnyView {
	let farms = data.farms.clone();
	let group_links=data.farms.iter().map(|farm|view!{<a class="button secondary" href=format!("/manage/farms/{}/publication",farm.id)>{farm.name.clone()}</a>}).collect_view();
	let farm_options = data.farms.clone();
	let rows=data.servers.into_iter().map(|server|{
		let controls=user.can_write().then(||view!{<a class="button secondary" href=format!("/manage/servers/{}/configuration",server.id)>{tr("Gelişmiş ayarlar")}</a><details class="edit-details"><summary>{tr("Düzenle")}</summary><form method="post" action=format!("/manage/servers/{}",server.id) class="stack compact-form">{csrf(&user.csrf)}<label>{tr("Sunucu adı")}<input name="name" value=server.name.clone() required maxlength="120"/></label><label>{tr("VPN adresi (DNS veya IP)")}<input name="host" value=server.host.clone() required maxlength="253"/></label><label>{tr("TCP portu")}<input type="number" name="port" value=server.port min="1" max="65535" required/></label><p class="small muted">{tr("Adres veya port değişikliği sunucunun sonraki bildiriminde uygulanır. Daha önce paylaşılan erişim anahtarlarındaki adres kendiliğinden güncellenmez.")}</p>{farm_select(farms.clone(),server.farm.clone())}<label class="checkbox"><input type="checkbox" name="paused" value="true" checked=server.paused/>{tr("Yeni oturumları durdur (bakım)")}</label><button class="button secondary" type="submit">{tr("Kaydet")}</button></form><div class="credential-action"><p class="small muted">{tr("Kimliği yenilerseniz yeni bağlantı ayarını sunucuya uygulamanız gerekir.")}</p><a class="button danger-outline" href=format!("/manage/servers/{}/credential",server.id)>{tr("Bağlantı kimliğini yenile")}</a>{help("Bağlantı kimliği", "Bu kimlik VPN sunucusunun yönetim paneline bağlanmasını sağlar. Yenileme eski kimliği geçersiz kılar. Yeni ayarı sunucuya uygulayana kadar durum ve yönetim iletişimi durur. VPN erişim anahtarları ayrı bilgilerdir.")}</div></details>});
		view!{<tr><td><strong>{server.name}</strong><span class="table-sub"><bdi dir="auto">{tr(&server.seen)}</bdi></span></td><td>{server.group.unwrap_or_else(||tr("Grupsuz"))}</td><td><span class="badge" class:online=server.online>{tr(&server.status)}</span></td><td><bdi dir="ltr">{server.cpu}</bdi></td><td><bdi dir="ltr">{server.memory}</bdi></td><td><bdi dir="ltr">{server.sessions}</bdi></td><td>{controls}</td></tr>}
	}).collect_view();
	view!{<div class="section-heading"><div><h2>{tr("VPN sunucularınızı yönetin")}{help("VPN sunucusu yönetimi", "Her sunucu yalnız kendi çalışma alanınıza aittir. Önce sunucuyu kaydedin, ardından oluşturulan bağlantı ayarını VPN sunucusuna uygulayın. Çevrimiçi durumu en az 90 saniyelik toleransla, yapılandırılan bildirim aralığının üç katına göre belirlenir.")}</h2><p class="muted">{tr("Her sunucu yalnızca çalışma alanınıza aittir.")}</p><a class="text-link" href="/manage/server-profiles">{tr("Sunucu profilleri")}</a></div><form method="get" class="search"><input type="search" name="q" value=data.query placeholder=tr("Sunucu ara") aria-label=tr("Sunucu ara")/><button class="button ghost" type="submit">{tr("Ara")}</button></form></div><section class="card table-wrap"><table><thead><tr><th>{tr("Sunucu")}</th><th>{tr("Grup")}</th><th>{tr("Durum")}</th><th>{tr("CPU")}</th><th>{tr("Bellek")}</th><th>{tr("Oturum")}</th><th>{tr("Yönetim")}</th></tr></thead><tbody>{rows}</tbody></table></section><section class="card"><h2>{tr("Sunucu grupları")}</h2><div class="actions">{group_links}</div></section>{user.can_write().then(||view!{<div class="form-grid"><section class="card"><span class="eyebrow">{tr("YENİ SUNUCU")}</span><h2>{tr("Sunucu ekle")}</h2><form method="post" action="/manage/servers" class="stack">{csrf(&user.csrf)}<label for="server-name">{tr("Sunucu adı")}</label><input id="server-name" name="name" required maxlength="120" placeholder=tr("Örn. İstanbul 01")/><label for="server-host">{tr("VPN adresi (DNS veya IP)")}{help("VPN bağlantı adresi", "VPN istemcilerinin bağlanacağı sunucunun dış IP adresini veya DNS adını yazın. Bu alan yönetim panelinin adresi değildir. TCP portu VPN sunucusunun dinlediği ve güvenlik duvarında açılmış port olmalıdır.")}</label><input id="server-host" name="host" required maxlength="253" placeholder=tr("vpn.ornek.com")/><label for="server-port">{tr("TCP portu")}</label><input id="server-port" type="number" name="port" value="443" min="1" max="65535" required/><label>{tr("Sunucu grubu")}{farm_select(farm_options,String::new())}</label><button class="button primary" type="submit">{tr("Sunucuyu kaydet")}</button><p class="small muted">{tr("Kayıttan sonra sunucunuza ekleyeceğiniz bağlantı ayarları gösterilir.")}</p></form></section><section class="card"><span class="eyebrow">{tr("DÜZENLEME")}</span><h2>{tr("Sunucu grubu oluştur")}{help("Sunucu grupları", "Gruplar sunucuları bölge veya amaca göre düzenler. Erişim profilinde farmAccess açılırsa aynı grubun uygun sunucuları arasında yönlendirme yapılır. Sunucular yalnız kendi kiracınıza aittir.")}</h2><p class="muted">{tr("Sunucularınızı bölge veya kullanım amacına göre gruplandırın.")}</p><form method="post" action="/manage/farms" class="stack">{csrf(&user.csrf)}<label for="farm-name">{tr("Grup adı")}</label><input id="farm-name" name="name" required maxlength="120" placeholder=tr("Örn. Türkiye")/><button class="button secondary" type="submit">{tr("Grup oluştur")}</button></form></section></div>})}}.into_any()
}
fn tokens(user: &User, data: Data) -> AnyView {
	let paging = pagination(data.page, &data.query, data.more);
	let search = data.query.clone();
	let rows=data.tokens.into_iter().map(|token|{
		let controls=user.can_tokens().then(||view!{<a class="button secondary" href=format!("/manage/tokens/{}/policy",token.id)>{tr("Politika")}</a><a class="button ghost" href=format!("/manage/tokens/{}/key",token.id)>{tr("Erişim anahtarı")}</a><form method="post" action="/manage/tokens/bulk" data-confirm={(!token.revoked).then(||tr("Bu erişim anahtarı iptal edilecek. Sunucunun sonraki bildiriminde ilgili bağlantılar kapatılabilir. Devam edilsin mi?"))}>{csrf(&user.csrf)}<input type="hidden" name="ids" value=token.id.clone()/><input type="hidden" name="action" value=if token.revoked{"enable"}else{"revoke"}/><button class="button ghost" type="submit">{tr(if token.revoked{"Etkinleştir"}else{"İptal et"})}</button></form>});
		view!{<tr><td>{user.can_tokens().then(||view!{<input type="checkbox" class="token-select" value=token.id aria-label=tr("{name} anahtarını seç").replace("{name}",&token.name)/>})}</td><td><strong>{token.name}</strong><span class="table-sub">{token.server}</span></td><td><bdi dir="ltr">{token.traffic}</bdi></td><td><bdi dir="auto">{tr(&token.quota)}</bdi></td><td><bdi dir="ltr">{token.clients}</bdi></td><td><bdi dir="auto">{tr(&token.expires)}</bdi></td><td><span class="badge" class:online={token.state=="Etkin"}>{tr(&token.state)}</span></td><td>{controls}</td></tr>}
	}).collect_view();
	let profile_options = data
		.profiles
		.into_iter()
		.map(|p| view! {<option value=p.id.to_string()>{p.name}</option>})
		.collect_view();
	let options = data
		.servers
		.into_iter()
		.map(|s| view! {<option value=s.id>{s.name}</option>})
		.collect_view();
	view!{<div class="section-heading"><div><h2>{tr("VPN erişim anahtarları ve kotaları")}</h2><p class="muted">{tr("Anahtarları tek tek veya topluca yönetin.")}</p></div><div class="actions"><a class="button secondary" href="/manage/profiles">{tr("Erişim profilleri")}</a><form method="get" class="search"><input type="search" name="q" value=search placeholder=tr("Anahtar ara") aria-label=tr("Anahtar ara")/><button class="button ghost" type="submit">{tr("Ara")}</button></form><a class="button secondary" href="/export/usage.csv">{tr("Kullanım raporunu indir")}</a></div></div><section class="card table-wrap"><table><thead><tr><th>{user.can_tokens().then(||view!{<input type="checkbox" id="select-all" aria-label=tr("Tüm anahtarları seç")/>})}</th><th>{tr("Anahtar / Sunucu")}</th><th>{tr("Kullanım")}</th><th>{tr("Kota")}</th><th>{tr("Cihaz")}</th><th>{tr("Son tarih")}</th><th>{tr("Durum")}</th><th>{tr("İşlem")}</th></tr></thead><tbody>{rows}</tbody></table></section>{paging}{user.can_tokens().then(||view!{<section class="card bulk-bar"><form method="post" action="/manage/tokens/bulk" id="bulk-form" class="bulk-form" data-confirm=tr("Seçtiğiniz anahtarların erişim veya kota ayarları değiştirilecek. Seçimi ve işlemi kontrol edin. Devam edilsin mi?")>{csrf(&user.csrf)}<input type="hidden" id="selected-ids" name="ids"/><strong><span id="selected-count">{tr("0")}</span>{tr(" anahtar seçildi")}</strong><select name="action" aria-label=tr("Toplu işlem")><option value="revoke">{tr("Seçilenleri iptal et")}</option><option value="enable">{tr("Seçilenleri etkinleştir")}</option><option value="quota">{tr("Seçilenlerin kotasını değiştir")}</option><option value="expiry">{tr("Seçilenlerin süresini değiştir")}</option></select><input type="number" name="max_gib" value="0" min="0" max="1000000" aria-label=tr("Yeni trafik kotası GiB")/><input type="number" name="max_clients" value="1" min="0" max="10000" aria-label=tr("Yeni cihaz kotası")/><input name="expires" placeholder=tr("Son tarih; boş = süresiz") aria-label=tr("Yeni son kullanma zamanı")/><button class="button secondary" type="submit">{tr("Uygula")}</button></form></section><section class="card"><span class="eyebrow">{tr("TEKLİ VE TOPLU ÜRETİM")}</span><h2>{tr("Erişim anahtarı oluştur")}{help("VPN erişim anahtarı", "VPN istemcisinin bağlanması için kullanılır; panel hesabı değildir. Adet alanıyla en fazla 100 anahtar oluşturabilirsiniz. Trafik kotasında 0 sınırsızdır; son kullanma zamanı boşsa süre sınırı yoktur. Zamanı saat dilimiyle ISO 8601 biçiminde yazın.")}</h2><form method="post" action="/manage/tokens" class="form-fields">{csrf(&user.csrf)}<label>{tr("Sunucu")}<select name="server" required>{options}</select></label><label>{tr("Erişim profili")}<select name="profile"><option value="">{tr("Formdaki sınırları kullan")}</option>{profile_options}</select><small>{tr("Profil seçilirse profilin cihaz ve trafik sınırları kullanılır.")}</small></label><label>{tr("Anahtar adı / öneki")}<input name="name" required maxlength="110" placeholder=tr("Örn. Ekip")/></label><label>{tr("Adet")}<input type="number" name="count" value="1" min="1" max="100" required/></label><label><span>{tr("Cihaz sınırı")}{help("Cihaz sınırı", "Bir erişim anahtarını aynı anda kullanabilecek farklı cihazların sayısıdır. Bu değer panel kullanıcılarının sayısını sınırlamaz.")}</span><input type="number" name="max_clients" value="1" min="0" max="10000" required/></label><label>{tr("Trafik kotası (GiB)")}<input type="number" name="max_gib" value="0" min="0" max="1000000" required/><small>{tr("0 = sınırsız")}</small></label><label>{tr("Son kullanma zamanı (isteğe bağlı)")}<input name="expires" placeholder=tr("2026-12-31T23:59:00+03:00")/></label><div class="form-submit"><button class="button primary" type="submit">{tr("Anahtarları oluştur")}</button></div></form></section>})}}.into_any()
}
fn pagination(page: usize, search: &str, more: bool) -> AnyView {
	let query = search
		.bytes()
		.map(|b| {
			if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
				(b as char).to_string()
			} else {
				format!("%{b:02X}")
			}
		})
		.collect::<String>();
	view!{<nav class="pagination" aria-label=tr("Anahtar sayfaları")><span class="small muted">{tr("Sayfa {page} · En fazla 100 kayıt").replace("{page}",&page.to_string())}</span><div class="actions">{(page>1).then(||view!{<a class="button secondary" href=format!("/panel/tokens?p={}&q={query}",page-1)>{tr("← Önceki")}</a>})}{more.then(||view!{<a class="button secondary" href=format!("/panel/tokens?p={}&q={query}",page+1)>{tr("Sonraki →")}</a>})}</div></nav>}.into_any()
}
fn sessions(user: &User, data: Data) -> AnyView {
	let rows=data.sessions.into_iter().map(|session|view!{<tr><td><strong>{format!("#{}",session.id)}</strong></td><td>{session.token}</td><td>{session.server}</td><td><bdi dir="ltr">{session.created}</bdi></td><td><span class="badge" class:online={!session.closed}>{tr(if session.closed{"Kapalı"}else{"Açık"})}</span></td><td>{(user.can_tokens()&&!session.closed).then(||view!{<form method="post" action=format!("/manage/sessions/{}/close",session.id) data-confirm=tr("Bu VPN oturumu sunucunun sonraki durum bildiriminde sonlandırılacak. Devam edilsin mi?")>{csrf(&user.csrf)}<button class="button ghost" type="submit">{tr("Sonlandır")}</button></form>})}</td></tr>}).collect_view();
	view!{<div class="section-heading"><div><h2>{tr("Bağlantıları takip edin")}{help("VPN oturumları", "Bir VPN istemcisinin bağlantısını gösterir. Sonlandırma isteği sunucunun sonraki durum bildiriminde uygulanır. Kapalı kayıtlar bağlantı geçmişidir.")}</h2><p class="muted">{tr("Son 200 VPN oturumu. Sonlandırma kararı sunucunun bir sonraki durum bildiriminde iletilir.")}</p></div></div><section class="card table-wrap"><table><thead><tr><th>{tr("Oturum")}</th><th>{tr("Anahtar")}</th><th>{tr("Sunucu")}</th><th>{tr("Başlangıç")}</th><th>{tr("Durum")}</th><th>{tr("İşlem")}</th></tr></thead><tbody>{rows}</tbody></table></section>}.into_any()
}
fn users(user: &User, data: Data) -> AnyView {
	let rows=data.users.into_iter().map(|account|view!{<tr><td><strong>{account.email}</strong></td><td><form method="post" action=format!("/manage/users/{}",account.id) class="inline-form">{csrf(&user.csrf)}<select name="role" aria-label=tr("Kullanıcı rolü")><option value="owner" selected={account.role=="owner"} disabled={!user.is_owner()}>{tr("Sahip")}</option><option value="token_writer" selected={account.role=="token_writer"}>{tr("Erişim anahtarı yazarı")}</option><option value="admin" selected={account.role=="admin"}>{tr("Yönetici")}</option><option value="operator" selected={account.role=="operator"}>{tr("Operatör")}</option><option value="viewer" selected={account.role=="viewer"}>{tr("Salt okunur")}</option></select><label class="checkbox"><input type="checkbox" name="active" value="true" checked=account.active/>{tr("Etkin")}</label><button class="button secondary" type="submit">{tr("Kaydet")}</button></form></td></tr>}).collect_view();
	view!{<div class="section-heading"><div><h2>{tr("Çalışma alanını birlikte yönetin")}{help("Panel kullanıcıları", "Bunlar yönetim paneline giriş yapan hesaplardır. Yönetici tüm işlemleri yapar; operatör sunucuları, anahtarları ve VPN oturumlarını yönetir; salt okunur kullanıcı değişiklik yapamaz. VPN kullanıcısına erişim anahtarı verilir.")}</h2><p class="muted">{tr("Sahip rol atamalarını korur; yönetici ekip ve projeyi, operatör sunucuları, anahtar yazarı yalnız erişim anahtarlarını yönetir. Salt okunur rol değişiklik yapamaz.")}</p><a class="text-link" href="/manage/bots">{tr("Botlar ve API anahtarları")}</a></div></div><section class="card table-wrap"><table><thead><tr><th>{tr("Kullanıcı")}</th><th>{tr("Rol ve erişim")}</th></tr></thead><tbody>{rows}</tbody></table></section><section class="card"><h2>{tr("Kullanıcı ekle")}</h2><form method="post" action="/manage/users" class="form-fields">{csrf(&user.csrf)}<label>{tr("E-posta")}<input type="email" name="email" autocomplete="off" maxlength="254" required/></label><label>{tr("Başlangıç parolası")}<input type="password" name="password" autocomplete="new-password" minlength="16" maxlength="128" required/></label><label>{tr("Rol")}<select name="role"><option value="viewer">{tr("Salt okunur")}</option><option value="operator">{tr("Operatör")}</option><option value="admin">{tr("Yönetici")}</option><option value="token_writer">{tr("Erişim anahtarı yazarı")}</option>{user.is_owner().then(||view!{<option value="owner">{tr("Sahip")}</option>})}</select></label><div class="form-submit"><button class="button primary" type="submit">{tr("Kullanıcı ekle")}</button></div></form></section>}.into_any()
}
fn audit(data: Data) -> AnyView {
	let rows=data.audit.into_iter().map(|event|view!{<tr><td><bdi dir="ltr">{event.time}</bdi></td><td>{event.email}</td><td><strong>{tr(&event.action)}</strong></td><td class="resource">{event.resource}</td></tr>}).collect_view();
	view!{<div class="section-heading"><div><h2>{tr("İşlem geçmişi")}{help("Denetim kaydı", "Kimin hangi yönetim işlemini ne zaman yaptığını gösterir. Son 200 işlem listelenir. Parolalar ve bağlantı sırları kayda yazılmaz.")}</h2><p class="muted">{tr("Çalışma alanınızda yapılan son 200 işlem. Parolalar ve erişim sırları bu kayıtlara yazılmaz.")}</p></div></div><section class="card table-wrap"><table><thead><tr><th>{tr("Zaman")}</th><th>{tr("Kullanıcı")}</th><th>{tr("İşlem")}</th><th>{tr("Kaynak")}</th></tr></thead><tbody>{rows}</tbody></table></section>}.into_any()
}
fn account(user: &User) -> AnyView {
	view!{<div>{user.is_admin().then(||view!{<section class="card account-card"><h2>{tr("Çalışma alanı")}</h2><form method="post" action="/manage/tenant" class="stack">{csrf(&user.csrf)}<label>{tr("Çalışma alanı adı")}<input name="name" value=user.tenant_name.clone() maxlength="120" required/></label><button class="button secondary" type="submit">{tr("Adı kaydet")}</button></form></section>})}<section class="card account-card"><span class="eyebrow">{tr("HESAP GÜVENLİĞİ")}</span><h2>{tr("Parolanızı değiştirin")}{help("Hesap güvenliği", "Mevcut parolanızı doğrulayarak en az 16 bayt uzunluğunda yeni parola belirleyin. Değişiklik tüm açık panel oturumlarınızı kapatır. Bu işlem VPN erişim anahtarlarını değiştirmez.")}</h2><p class="muted">{tr("Parola değiştiğinde açık panel oturumlarınız kapatılır.")}</p><form method="post" action="/manage/password" class="stack">{csrf(&user.csrf)}<label>{tr("Mevcut parola")}<input type="password" name="current_password" autocomplete="current-password" maxlength="128" required/></label><label>{tr("Yeni parola")}<input type="password" name="new_password" autocomplete="new-password" minlength="16" maxlength="128" required/></label><button class="button primary" type="submit">{tr("Parolayı değiştir")}</button></form></section></div>}.into_any()
}
pub fn confirm_credential(user: &User, id: Uuid, name: &str, proof: &str) -> Html<String> {
	layout(user, "Bağlantı kimliğini yenile", "servers", view!{
		<div class="page-content"><section class="card confirmation-card"><div class="modal-caption warning-text">{tr("GERİ ALINAMAYAN İŞLEM")}</div><h2>{tr("Bağlantı kimliği yenilensin mi?")}</h2><p class="confirmation-target"><span>{tr("Sunucu")}</span><strong>{name.to_owned()}</strong></p><div class="alert warning"><strong>{tr("Eski bağlantı kimliği hemen geçersizleşir.")}</strong><p>{tr("Yeni ayarı sunucuya uygulayana kadar panel sunucuyla haberleşemez. Bu işlem mevcut VPN erişim anahtarlarını değiştirmez.")}</p></div><p>{tr("Devam ederseniz yeni bağlantı ayarı yalnız bir kez gösterilir. Ayarı güvenli biçimde kaydedip sunucunun storage/appsettings.json dosyasına uygulamanız gerekir.")}</p><form method="post" action=format!("/manage/servers/{id}/credential") class="stack">{csrf(&user.csrf)}<input type="hidden" name="confirmation" value=proof.to_owned()/><label class="checkbox confirmation-check"><input type="checkbox" name="confirmed" value="true" required/>{tr("Eski kimliğin geçersizleşeceğini ve yeni ayarı sunucuya uygulamam gerektiğini anladım.")}</label><div class="actions"><a class="button secondary" href="/panel/servers" autofocus>{tr("Vazgeç")}</a><button class="button danger" type="submit">{tr("Onayla ve kimliği yenile")}</button></div></form></section></div>
	}.into_any())
}

#[allow(clippy::too_many_arguments)]
pub fn configuration(
	user: &User,
	id: Uuid,
	name: &str,
	code: &str,
	settings: Value,
	status: Option<Value>,
	error: Option<String>,
) -> Html<String> {
	let status = status.unwrap_or(Value::Null);
	let applied = status["configCode"].as_str() == Some(code)
		&& status["configError"].as_str().is_none_or(str::is_empty);
	let state = tr(if applied {
		"Uygulandı"
	} else if status["configError"]
		.as_str()
		.is_some_and(|e| !e.is_empty())
	{
		"Yapılandırma hatası"
	} else {
		"Sunucunun uygulaması bekleniyor"
	});
	let server_error = status["configError"].as_str().unwrap_or("").to_owned();
	let endpoints = serde_json::to_string_pretty(&status["endPointStatuses"]).unwrap_or_default();
	let system=serde_json::to_string_pretty(&json!({"totalSwapMemory":status["totalSwapMemory"],"availableSwapMemory":status["availableSwapMemory"],"tcpCongestionControl":status["tcpCongestionControl"]})).unwrap_or_default();
	let catalog: Value =
		serde_json::from_str(vpnhood_protocol::config::CATALOG).expect("embedded catalog");
	let mut fields = Vec::new();
	for (class, prefix) in [
		("ServerConfig", ""),
		("SessionOptions", "Session."),
		("TrackingOptions", "Tracking."),
		("NetFilterOptions", "NetFilter."),
	] {
		for field in catalog["classes"][class]["fields"]
			.as_array()
			.expect("catalog fields")
		{
			let name = field["name"].as_str().unwrap_or_default();
			if prefix.is_empty()
				&& (vpnhood_protocol::config::PROTECTED.contains(&name)
					|| ["Session", "Tracking", "NetFilter"].contains(&name))
			{
				continue;
			}
			fields.push((
				format!("{prefix}{name}"),
				field["type"].as_str().unwrap_or_default().to_owned(),
			));
		}
	}
	let reference = fields
		.into_iter()
		.map(
			|(name, kind)| view! {<tr><td><code>{name}</code></td><td><code>{kind}</code></td></tr>},
		)
		.collect_view();
	layout(user, "Gelişmiş ayarlar", "servers", view!{
		<div class="section-heading"><div><h2>{name.to_owned()}</h2><p class="muted">{tr("Gelişmiş sunucu yapılandırması")}</p></div><div class="actions"><a class="button secondary" href=format!("/manage/servers/{id}/installation")>{tr("Kurulum / yeniden kurulum")}</a><a class="button secondary" href=format!("/manage/servers/{id}/routing")>{tr("Yönlendirme ve duyuru")}</a><a class="button secondary" href=format!("/manage/servers/{id}/certificates")>{tr("Sertifikalar")}</a><a class="button secondary" href="/panel/servers">{tr("Sunuculara dön")}</a></div></div>
		<section class="card"><h2>{tr("Uygulama durumu")}</h2><p><span class="badge" class:online=applied>{state}</span></p><p class="small muted">{tr("Kaydedilen sürüm")}" "<code>{code.to_owned()}</code></p>{(!server_error.is_empty()).then(||view!{<div class="alert warning">{server_error}</div>})}<details><summary>{tr("Dinleyici sonuçları")}</summary><pre dir="ltr">{endpoints}</pre></details><details><summary>{tr("Sunucunun bildirdiği sistem ayarları")}</summary><pre dir="ltr">{system}</pre></details></section>
		<section class="card"><h2>{tr("Ayarlar (JSON)")}{help("Gelişmiş ayarlar", "Session oturum sınırlarını, NetFilter ağ erişim kurallarını, Tracking trafik izleme alanlarını belirler. null varsayılanı kullanır. Süreler gün.saat:dakika:saniye; tamponlar send/receive bayt değerleridir. Dinleme uçları IP:port biçimindedir. Bu ayarlar VPN sunucusunda uygulanır; istemcilere duyurulan genel adres Sunucular ekranından değiştirilir.")}</h2><div class="alert warning">{tr("Dinleyici veya ağ kuralları değişikliği bağlantıları etkileyebilir. Sunucunun yerel appsettings.json ayarları merkezi ayarları geçersiz kılabilir.")}</div>{error.map(|e|view!{<p class="alert warning" role="alert">{e}</p>})}<form method="post" action=format!("/manage/servers/{id}/configuration") class="stack">{csrf(&user.csrf)}<input type="hidden" name="base_code" value=code.to_owned()/><label>{tr("Ayarlar (JSON)")}<textarea class="config-editor" name="settings" dir="ltr" spellcheck="false" rows="24" readonly={!user.can_write()} required>{serde_json::to_string_pretty(&settings).unwrap_or_default()}</textarea></label>{user.can_write().then(||view!{<label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("Ayarların VPN bağlantılarını etkileyebileceğini anladım.")}</label><button class="button primary" type="submit">{tr("Doğrula ve sunucuya uygula")}</button>})}</form></section>
		<section class="card"><h2>{tr("Sunucu adresini yayımla")}</h2><p class="muted">{tr("Yeni erişim anahtarlarına şifreli adres güncelleme URL’si ekler. Önceden dağıtılan anahtarlar yeniden paylaşılmalıdır.")}</p>{user.can_write().then(||view!{<form method="post" action=format!("/manage/servers/{id}/publish")>{csrf(&user.csrf)}<button class="button secondary" type="submit">{tr("Yayımlamayı etkinleştir")}</button></form>})}</section><section class="card"><details><summary>{tr("Desteklenen alanlar ve türleri")}</summary><p class="muted">{tr("Alan adları kaynak sözleşmesiyle aynıdır. Bilinmeyen alanlar reddedilir. Soru işareti nullable alanı belirtir.")}</p><div class="table-wrap"><table><thead><tr><th>{tr("Alan")}</th><th>{tr("Tür")}</th></tr></thead><tbody>{reference}</tbody></table></div></details></section>
	}.into_any())
}

fn policy_help() -> AnyView {
	view!{<div class="alert warning"><p>{tr("Hız: maxUploadMbps ve maxDownloadMbps; 0 sınırsızdır. Trafik kotaları bayttır. maxCycleTraffic için cycleSeconds ile dönemi açıkça belirleyin. lifetimeDays ilk başarılı bağlantıdan itibaren işler.")}</p><p>{tr("deviceLifeSpan cihaz kayıtlarının gün cinsinden ömrüdür; 0 yalnız eşzamanlı oturum sınırını kullanır. clientPolicies istemciye duyurulan cpols kurallarıdır. Reklam gerektiren planlar doğrulanmış ödül entegrasyonu olmadan kabul edilmez.")}</p></div>}.into_any()
}
pub fn profiles(user: &User, profiles: Vec<Profile>) -> Html<String> {
	let rows = profiles.into_iter().map(|profile|view!{
		<section class="card"><h2>{profile.name.clone()}</h2><p class="muted">{tr(if profile.is_public {"Açık profil"} else {"Özel profil"})}</p><form method="post" action=format!("/manage/profiles/{}",profile.id) class="stack" data-confirm=tr("Profil değişikliği yeni anahtarlarda kullanılacak. Mevcut anahtarlara uygula seçilirse onların etkin oturumları kapatılacak. Devam edilsin mi?")>{csrf(&user.csrf)}<label>{tr("Profil adı")}<input name="name" value=profile.name required maxlength="120" readonly={!user.can_tokens()}/></label><label>{tr("Politika (JSON)")}<textarea class="config-editor" name="policy" rows="18" dir="ltr" readonly={!user.can_tokens()}>{serde_json::to_string_pretty(&profile.policy).unwrap_or_default()}</textarea></label>{user.can_tokens().then(||view!{<label class="checkbox"><input type="checkbox" name="apply_existing" value="true"/>{tr("Mevcut anahtarlara uygula ve etkin oturumları kapat")}</label><button class="button primary" type="submit">{tr("Kaydet")}</button>})}</form></section>
	}).collect_view();
	layout(user, "Erişim profilleri", "tokens", view!{<div class="section-heading"><div><h2>{tr("Erişim profilleri")}</h2>{user.is_admin().then(||view!{<a class="text-link" href="/manage/integrations">{tr("Ödül doğrulama bağlantısı")}</a>})}</div><a class="button secondary" href="/panel/tokens">{tr("Anahtarlara dön")}</a></div>{policy_help()}{rows}{user.can_tokens().then(||view!{<section class="card"><h2>{tr("Profil oluştur")}</h2><form method="post" action="/manage/profiles" class="stack">{csrf(&user.csrf)}<label>{tr("Profil adı")}<input name="name" required maxlength="120"/></label><label>{tr("Politika (JSON)")}<textarea class="config-editor" name="policy" rows="18" dir="ltr">{serde_json::to_string_pretty(&crate::policy::AccessPolicy::default()).unwrap_or_default()}</textarea></label><button class="button primary" type="submit">{tr("Profil oluştur")}</button></form></section>})}}.into_any())
}
pub fn token_policy(
	user: &User,
	id: Uuid,
	name: &str,
	policy: Value,
	details: Value,
) -> Html<String> {
	let metadata=[("name","Anahtar adı / öneki",120),("group_name","Grup",120),("description","Açıklama",2000),("customer_id","Müşteri kimliği",128),("order_id","Sipariş kimliği",128),("support_id","Destek kodu",128)].into_iter().map(|(field,label,max)|view!{<label>{tr(label)}<input name=field value=details[field].as_str().unwrap_or_default().to_owned() maxlength=max/></label>}).collect_view();
	layout(user,"Erişim politikası","tokens",view!{<div class="section-heading"><h2>{name.to_owned()}</h2><a class="button secondary" href="/panel/tokens">{tr("Anahtarlara dön")}</a></div>{policy_help()}<section class="card"><h2>{tr("Anahtar bilgileri")}</h2><form method="post" action=format!("/manage/tokens/{id}/details") class="stack">{csrf(&user.csrf)}{metadata}<button class="button primary" type="submit">{tr("Kaydet")}</button><a class="button secondary" href=format!("/manage/tokens/{id}/download")>{tr("Anahtarı indir")}</a></form></section><section class="card"><h2>{tr("Erişim politikası")}</h2><form method="post" action=format!("/manage/tokens/{id}/policy") class="stack">{csrf(&user.csrf)}<label>{tr("Politika (JSON)")}<textarea class="config-editor" name="policy" rows="24" dir="ltr" spellcheck="false">{serde_json::to_string_pretty(&policy).unwrap_or_default()}</textarea></label><label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("Politikayı değiştirmek için etkin VPN oturumlarının kapatılmasını onaylıyorum.")}</label><button class="button primary" type="submit">{tr("Doğrula ve uygula")}</button></form></section><section class="card"><h2>{tr("Erişim kodu")}</h2><form method="post" action=format!("/manage/tokens/{id}/access-code") class="stack">{csrf(&user.csrf)}<label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("Bu anahtara erişim sağlayan bir kod oluşturmayı onaylıyorum.")}</label><button class="button secondary" type="submit">{tr("Erişim kodu oluştur")}</button></form></section>}.into_any())
}

pub fn certificates(
	user: &User,
	id: Uuid,
	name: &str,
	code: &str,
	certificates: Vec<(String, String, String)>,
) -> Html<String> {
	let rows=certificates.into_iter().enumerate().map(|(i,(name,expiry,fingerprint))|view!{<tr><td>{i+1}</td><td>{name}</td><td><bdi dir="ltr">{expiry}</bdi></td><td><code dir="ltr">{fingerprint}</code></td></tr>}).collect_view();
	layout(user,"Sertifikalar","servers",view!{<div class="section-heading"><h2>{name.to_owned()}</h2><a class="button primary" href=format!("/manage/servers/{id}/autosign")>{tr("Otomatik sertifika (AutoSign)")}</a><a class="button secondary" href=format!("/manage/servers/{id}/configuration")>{tr("Gelişmiş ayarlar")}</a></div><section class="card table-wrap"><table><thead><tr><th>{tr("Öncelik")}</th><th>{tr("TLS adı")}</th><th>{tr("Son tarih")}</th><th>"SHA-256"</th></tr></thead><tbody>{rows}</tbody></table></section><section class="card"><h2>{tr("PFX sertifikası yükle")}</h2><p class="muted">{tr("PFX dışa aktarılabilir özel anahtar içermelidir. Özel anahtar şifreli saklanır. Varsayılan sertifika değişirse eski erişim anahtarları yeniden paylaşılmalı veya adres güncellemesiyle yenilenmelidir.")}</p><form method="post" action=format!("/manage/servers/{id}/certificates") class="stack">{csrf(&user.csrf)}<input type="hidden" name="base_code" value=code.to_owned()/><label>{tr("TLS adı")}<input name="tls_name" required maxlength="253"/></label><label>{tr("PFX dosyası")}<input type="file" accept=".pfx,.p12" data-pfx-file="pfx-data"/></label><label>{tr("PFX verisi (Base64)")}<textarea id="pfx-data" name="pfx" rows="3" dir="ltr" required maxlength="350000"/></label><label>{tr("PFX parolası")}<input type="password" name="password" autocomplete="new-password" maxlength="1024"/></label><label class="checkbox"><input type="checkbox" name="make_default" value="true"/>{tr("Varsayılan sertifika olarak kullan")}</label><label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("Sertifika değişikliğinin bağlantı anahtarlarını etkileyebileceğini anladım.")}</label><button class="button primary" type="submit">{tr("Doğrula ve yükle")}</button></form></section><section class="card"><h2>"ACME HTTP-01"</h2><p class="muted">{tr("Sertifika sağlayıcınızın token ve keyAuthorization değerlerini girin. Yanıt 15 dakika geçerlidir; VPN sunucusunda HTTP 80 dinleyicisi açılır. Genel TCP 80 yönlendirmesi gereklidir. Bu işlem tek başına sertifika üretmez veya yenilemez.")}</p><form method="post" action=format!("/manage/servers/{id}/challenge") class="stack">{csrf(&user.csrf)}<label>"token"<input name="token" required minlength="16" maxlength="256" dir="ltr"/></label><label>"keyAuthorization"<input name="key_authorization" required maxlength="1024" dir="ltr"/></label><label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("VPN sunucusunda HTTP 80 dinleyicisinin açılmasını onaylıyorum.")}</label><button class="button secondary" type="submit">{tr("Doğrulamayı hazırla")}</button></form></section>}.into_any())
}

pub fn routing(
	user: &User,
	id: Uuid,
	name: &str,
	code: &str,
	policy: Value,
	advertisement: Value,
) -> Html<String> {
	layout(user,"Yönlendirme ve duyuru","servers",view!{<div class="section-heading"><h2>{name.to_owned()}</h2><a class="button secondary" href=format!("/manage/servers/{id}/configuration")>{tr("Gelişmiş ayarlar")}</a></div><section class="card"><p class="muted">{tr("power yük dağıtımındaki göreli kapasitedir; 0 işlemci sayısını kullanır. location CC/Region biçimindedir. farmAccess seçilen anahtarlar yalnız kendi gruplarında yönlendirilir. maxNormalSessions ve maxPremiumSessions yeni oturumları sınırlar; 0 sınırsızdır.")}</p><p class="muted">{tr("Duyuru istemcilerin bağlanacağı genel adresleri belirler. Dinleyiciler ayrı ayarlanır. ep_st: 0 otomatik, 1 DNS önce, 2 IP önce, 3 yalnız DNS, 4 yalnız IP. urls şifreli sunucu adresini sunan HTTPS kaynaklarıdır. path ters vekil yolu; loc2 konum ve etiketlerdir.")}</p><form method="post" action=format!("/manage/servers/{id}/routing") class="stack">{csrf(&user.csrf)}<input type="hidden" name="base_code" value=code.to_owned()/><label>{tr("Sunucu politikası (JSON)")}<textarea class="config-editor" name="policy" dir="ltr" rows="14">{serde_json::to_string_pretty(&policy).unwrap_or_default()}</textarea></label><label>{tr("İstemci duyurusu (JSON)")}<textarea class="config-editor" name="advertisement" dir="ltr" rows="14">{serde_json::to_string_pretty(&advertisement).unwrap_or_default()}</textarea></label><label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("Ayarların VPN bağlantılarını etkileyebileceğini anladım.")}</label><button class="button primary" type="submit">{tr("Doğrula ve uygula")}</button></form></section>}.into_any())
}
pub fn integrations(user: &User, enabled: bool) -> Html<String> {
	layout(user,"Ödül doğrulama","account",view!{<section class="card"><h2>{tr("Ödül doğrulama bağlantısı")}</h2><p>{tr(if enabled{"Etkin"}else{"Kapalı"})}</p><p class="muted">{tr("Hoodmin ödüllü reklamı yalnız güvenilir sunucunun doğrulanmış bildirimiyle kabul eder. Bu bağlantı bir reklam sağlayıcısı değildir. Mevcut kimliği yenilemek sağlayıcıdaki ayarı güncellemeyi gerektirir.")}</p><form method="post" action="/manage/integrations" class="stack">{csrf(&user.csrf)}<label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("Yeni doğrulama kimliğini sağlayıcıya uygulamam gerektiğini anladım.")}</label><button class="button primary" type="submit">{tr("Bağlantıyı oluştur veya yenile")}</button></form></section>}.into_any())
}

pub fn installation(user: &User, id: Uuid, settings: &str) -> Html<String> {
	let value: Value = serde_json::from_str(settings).expect("generated installation settings");
	let (linux, windows) = crate::install::commands(&value);
	layout(user,"Sunucu bağlantısı hazır","servers",view!{<section class="card"><h2>{tr("Sunucu bağlantısı hazır")}</h2><p class="muted">{tr("Resmi hazır VpnHood kurulumu kullanılır. Kimlik içeren komut ve ayarları güvenli saklayın.")}</p><a class="button primary" href=format!("/manage/servers/{id}/installation")>{tr("SSH ile parola veya özel anahtar kullanarak kur")}</a></section><section class="card"><h2>{tr("Linux: elle komut çalıştır")}</h2><textarea dir="ltr" rows="5" readonly>{linux}</textarea></section><section class="card"><h2>{tr("Windows: PowerShell komutu")}</h2><textarea dir="ltr" rows="5" readonly>{windows}</textarea></section><section class="card"><h2>"appsettings.json"</h2><p class="muted">{tr("Mevcut kurulumda storage/appsettings.json dosyasına kaydedin. Bu sayfa önbelleğe alınmaz.")}</p><textarea id="secret-output" dir="ltr" rows="12" readonly>{settings.to_owned()}</textarea><div class="actions"><button class="button secondary" type="button" data-copy="secret-output">{tr("Kopyala")}</button><span id="copy-feedback" role="status" class="small muted"></span></div><a class="button secondary" href="/panel/servers">{tr("Sunuculara dön")}</a></section>}.into_any())
}
pub struct InstallJob {
	pub id: Uuid,
	pub status: String,
	pub log: String,
	pub time: String,
}
pub fn ssh_installation(user: &User, id: Uuid, name: &str, jobs: Vec<InstallJob>) -> Html<String> {
	let jobs=jobs.into_iter().map(|job|view!{<section class="card" data-install-job=format!("/manage/install-jobs/{}",job.id)><h3><span data-install-status>{tr(&job.status)}</span><small>{job.time}</small></h3><pre data-install-log dir="ltr">{job.log}</pre></section>}).collect_view();
	layout(user,"Kurulum / yeniden kurulum","servers",view!{<div class="section-heading"><h2>{name.to_owned()}</h2><a class="button secondary" href=format!("/manage/servers/{id}/configuration")>{tr("Gelişmiş ayarlar")}</a></div><section class="card"><h2>{tr("SSH ile resmi sunucu kurulumu")}</h2><div class="alert warning">{tr("Hedefte root yetkisiyle resmi kurulum betiği çalışır ve appsettings.json değiştirilir. Önceki dosya hedefte yedeklenir. SSH parolası ve özel anahtarı iş tamamlanınca silinir. Şifreli agent kimliği sonraki kurulumlar için saklanır.")}</div><form method="post" action=format!("/manage/servers/{id}/installation") class="stack">{csrf(&user.csrf)}<div class="form-fields"><label>{tr("SSH adresi")}<input name="host" required maxlength="253"/></label><label>{tr("SSH portu")}<input name="port" type="number" min="1" max="65535" value="22" required/></label><label>{tr("SSH kullanıcısı")}<input name="username" value="root" maxlength="64" required/></label><label>{tr("Sunucu anahtarı parmak izi (SHA256)")}<input name="fingerprint" dir="ltr" placeholder="SHA256:..." required/>{help("SSH sunucu doğrulaması","Parmak izini hedefte güvenilir bir oturumdan ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub -E sha256 ile okuyun. Anahtar eşleşmezse bağlantı kurulmaz.")}</label></div><label>{tr("SSH parolası (parola ile kurulum)")}<input type="password" name="password" autocomplete="off" maxlength="1024"/></label><label>{tr("SSH özel anahtarı (anahtar ile kurulum)")}<textarea name="private_key" dir="ltr" rows="5" maxlength="65536" autocomplete="off"/></label><p class="small muted">{tr("Parola veya özel anahtardan yalnız birini girin. Parolalı özel anahtar desteklenmez. root dışındaki kullanıcı için parolasız sudo veya aşağıdaki sudo parolası gerekir.")}</p><label>{tr("sudo parolası (gerekiyorsa)")}<input type="password" name="sudo_password" autocomplete="off" maxlength="1024"/></label><details><summary>{tr("Eski kayıt için bağlantı kimliği")}</summary><label>{tr("Sunucu bağlantı kimliği")}<input name="credential" type="password" autocomplete="off" maxlength="200"/></label></details><label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("Hedefte kurulum ve yapılandırma değişikliğini onaylıyorum.")}</label><button class="button primary" type="submit">{tr("Kurulumu başlat")}</button></form></section>{jobs}}.into_any())
}

pub struct ServerProfile {
	pub assigned_farms: i64,
	pub id: Uuid,
	pub name: String,
	pub settings: Value,
	pub management: Value,
	pub version: Uuid,
}
pub fn server_profiles(
	user: &User,
	profiles: Vec<ServerProfile>,
	servers: Vec<(Uuid, String)>,
) -> Html<String> {
	let list = servers
		.into_iter()
		.map(|(id, name)| view! {<tr><td>{name}</td><td><code>{id.to_string()}</code></td></tr>})
		.collect_view();
	let rows=profiles.into_iter().map(|profile|view!{<section class="card"><h2>{profile.name.clone()}</h2><form method="post" action=format!("/manage/server-profiles/{}",profile.id) class="stack">{csrf(&user.csrf)}<input type="hidden" name="version" value=profile.version.to_string()/><label>{tr("Profil adı")}<input name="name" value=profile.name required maxlength="120" readonly={!user.can_write()}/></label><label>{tr("Ayarlar (JSON)")}<textarea class="config-editor" name="settings" rows="12" dir="ltr" readonly={!user.can_write()}>{serde_json::to_string_pretty(&profile.settings).unwrap_or_default()}</textarea></label><label>{tr("Sunucu politikası (JSON)")}<textarea class="config-editor" name="management" rows="12" dir="ltr" readonly={!user.can_write()}>{serde_json::to_string_pretty(&profile.management).unwrap_or_default()}</textarea></label>{user.can_write().then(||view!{<p class="muted">{tr("Bağlı grup sayısı")}{": "}{profile.assigned_farms}</p>{(profile.assigned_farms>0).then(||view!{<label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("Bağlı gruplardaki tüm sunuculara bu değişikliği uygulamayı onaylıyorum.")}</label>})}<button class="button primary" type="submit">{tr("Kaydet")}</button>})}</form>{user.can_write().then(||view!{<details><summary>{tr("Sunuculara uygula")}</summary><form method="post" action=format!("/manage/server-profiles/{}/apply",profile.id) class="stack">{csrf(&user.csrf)}<input type="hidden" name="version" value=profile.version.to_string()/><label>{tr("Sunucu kimlikleri (boşlukla ayırın)")}<textarea name="servers" required rows="3" dir="ltr"/></label><label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("Seçili sunucuların ayarlarının değişmesini onaylıyorum.")}</label><button class="button secondary" type="submit">{tr("Topluca uygula")}</button></form></details>})}</section>}).collect_view();
	layout(user,"Sunucu profilleri","servers",view!{<div class="section-heading"><h2>{tr("Sunucu profilleri")}</h2><a class="button secondary" href="/panel/servers">{tr("Sunuculara dön")}</a></div><div class="alert warning">{tr("Gruba bağlı profil kaydedildiğinde değişiklikler tüm grup üyelerine uygulanır. Bağlı olmayan profiller yalnız seçilerek uygulanır. Dinleyici, ağ ve kapasite değişebilir; sertifika, kimlik ve duyurulan adres korunur.")}</div>{rows}{user.can_write().then(||view!{<section class="card"><h2>{tr("Profil oluştur")}</h2><form method="post" action="/manage/server-profiles" class="stack">{csrf(&user.csrf)}<label>{tr("Profil adı")}<input name="name" required maxlength="120"/></label><label>{tr("Ayarlar (JSON)")}<textarea class="config-editor" name="settings" rows="12" dir="ltr">"{\n\t\"Session\": {},\n\t\"Tracking\": {},\n\t\"NetFilter\": {}\n}"</textarea></label><label>{tr("Sunucu politikası (JSON)")}<textarea class="config-editor" name="management" rows="12" dir="ltr">{serde_json::to_string_pretty(&crate::farms::ServerPolicy::default()).unwrap_or_default()}</textarea></label><button class="button primary" type="submit">{tr("Profil oluştur")}</button></form></section>})}<section class="card table-wrap"><h2>{tr("Sunucu kimlikleri")}</h2><table><thead><tr><th>{tr("Sunucu")}</th><th>{tr("Kimlik")}</th></tr></thead><tbody>{list}</tbody></table></section>}.into_any())
}

pub fn reports(
	user: &User,
	servers: Vec<(Uuid, String)>,
	server: Option<Uuid>,
	hours: i32,
	charts: Vec<crate::reports::Chart>,
	countries: Vec<(String, i64)>,
	range: String,
) -> Html<String> {
	let options = servers
		.into_iter()
		.map(
			|(id, name)| view! {<option value=id.to_string() selected=server==Some(id)>{name}</option>},
		)
		.collect_view();
	let periods = [
		(1, "Son saat"),
		(24, "Son 24 saat"),
		(48, "Son 48 saat"),
		(72, "Son 72 saat"),
		(168, "Son 7 gün"),
		(744, "Son 31 gün"),
	]
	.into_iter()
	.map(|(value, label)| view! {<option value=value selected=hours==value>{tr(label)}</option>})
	.collect_view();
	let charts=charts.into_iter().map(|chart|view!{<section class="card report-chart"><h2>{tr(chart.title)}" "<small>{chart.unit}</small></h2><p class="small muted">{tr("Tepe")}" "{format!("{:.2} {}",chart.peak,chart.unit)}</p>{(chart.count>0).then(||view!{<svg viewBox="0 0 600 160" role="img" aria-label=tr(chart.title)><path d="M10 10V140H590" fill="none" stroke="var(--line)"/><polyline points=chart.points fill="none" stroke="var(--accent)" stroke-width="2"/></svg>})}{(chart.count==0).then(||view!{<p class="empty-state">{tr("Bu dönemde ölçüm yok.")}</p>})}</section>}).collect_view();
	let countries = countries
		.into_iter()
		.map(
			|(country, count)| view! {<tr><td><bdi dir="ltr">{country}</bdi></td><td>{count}</td></tr>},
		)
		.collect_view();
	layout(user,"Kullanım raporları","reports",view!{<div class="section-heading"><h2>{tr("Kullanım raporları")}</h2><form method="get" action="/reports" class="actions"><label>{tr("Sunucu")}<select name="server"><option value="" selected=server.is_none()>{tr("Tüm sunucular")}</option>{options}</select></label><label>{tr("Dönem")}<select name="hours">{periods}</select></label><button class="button secondary" type="submit">{tr("Göster")}</button></form></div><p class="muted">{tr("Grafikler bildirilen ölçümlerin zaman aralığı ortalamasıdır; boş aralıklar sıfır kullanım sayılmaz. Tüm sunucular seçildiğinde sunucuların ortalaması gösterilir. Veriler 31 gün saklanır.")}</p><p class="small muted"><bdi dir="ltr">{range}</bdi></p><div class="form-grid">{charts}</div><section class="card table-wrap"><h2>{tr("Ülkelere göre cihazlar")}</h2><p class="muted">{tr("Seçili dönemde açılan oturumların bildirdiği ülke ve benzersiz cihaz sayısıdır. Soru işareti bilinmeyen ülkedir; IP konumlandırması yapılmaz.")}</p><table><thead><tr><th>{tr("Ülke")}</th><th>{tr("Kullanılan cihazlar")}</th></tr></thead><tbody>{countries}</tbody></table></section>}.into_any())
}

pub struct Bot {
	pub id: Uuid,
	pub name: String,
	pub role: String,
	pub expires: String,
	pub revoked: bool,
}
pub fn bots(user: &User, bots: Vec<Bot>) -> Html<String> {
	let rows=bots.into_iter().map(|bot|view!{<tr><td>{bot.name}</td><td>{role_label(&bot.role)}</td><td><bdi dir="ltr">{bot.expires}</bdi></td><td>{if bot.revoked{tr("İptal edildi")}else{tr("Etkin")}}</td><td>{(!bot.revoked).then(||view!{<form method="post" action=format!("/manage/bots/{}/revoke",bot.id) data-confirm=tr("Bu botun API erişimi hemen iptal edilecek. Devam edilsin mi?")>{csrf(&user.csrf)}<input type="hidden" name="confirmed" value="true"/><button class="button danger" type="submit">{tr("İptal et")}</button></form>})}</td></tr>}).collect_view();
	layout(user,"Botlar ve API anahtarları","users",view!{<section class="card"><h2>{tr("Botlar ve API anahtarları")}</h2><p class="muted">{tr("Bot, seçilen ekip rolüyle yalnız kendi kiracınızda API işlemleri yapar. Anahtarın yalnız özeti saklanır. Süre dolması veya iptal sonraki istekte uygulanır. Bot sahibi rolü alamaz.")}</p><table><thead><tr><th>{tr("Ad")}</th><th>{tr("Rol")}</th><th>{tr("Son tarih")}</th><th>{tr("Durum")}</th><th>{tr("İşlem")}</th></tr></thead><tbody>{rows}</tbody></table></section><section class="card"><h2>{tr("Bot oluştur")}</h2><form method="post" action="/manage/bots" class="stack">{csrf(&user.csrf)}<label>{tr("Ad")}<input name="name" required maxlength="120"/></label><label>{tr("Rol")}<select name="role"><option value="viewer">{tr("Salt okunur")}</option><option value="token_writer">{tr("Erişim anahtarı yazarı")}</option><option value="operator">{tr("Operatör")}</option><option value="admin">{tr("Yönetici")}</option></select></label><label>{tr("Erişim süresi (gün)")}<input name="lifetime_days" type="number" value="90" min="1" max="365" required/></label><label class="checkbox"><input name="confirmed" type="checkbox" value="true" required/>{tr("Seçilen rol ve süreyle API erişimi verilmesini onaylıyorum.")}</label><button class="button primary" type="submit">{tr("Bot oluştur")}</button></form></section>}.into_any())
}

pub fn farm_publication(
	user: &User,
	id: Uuid,
	name: &str,
	enabled: bool,
	version: Uuid,
) -> Html<String> {
	layout(user,"Grup adresi yayımı","servers",view!{<div class="section-heading"><h2>{name.to_owned()}</h2><div class="actions"><a class="button secondary" href=format!("/manage/farms/{id}/server-profile")>{tr("Grup sunucu profili")}</a><a class="button secondary" href=format!("/manage/farms/{id}/destinations")>{tr("Harici adres yayımı")}</a><a class="button secondary" href=format!("/manage/farms/{id}/certificates")>{tr("Grup sertifikaları")}</a><a class="button secondary" href="/panel/servers">{tr("Sunuculara dön")}</a></div></div><section class="card"><h2>{tr("Grup adresi yayımı")}</h2><p>{tr(if enabled{"Etkin"}else{"Kapalı"})}</p><p class="muted">{tr("Grup erişimi olan yeni anahtarlara şifreli adres yenileme bağlantısı ekler. İlk sunucu çevrimdışıysa istemci uygulaması bu bağlantıdan uygun grup sunucusunun adresini ve sertifika bilgisini alabilir. Eski anahtarlar yeniden paylaşılmalıdır. Sunucu sırları veya sertifikaları kendiliğinden ortaklaştırılmaz.")}</p>{user.can_write().then(||view!{<form method="post" action=format!("/manage/farms/{id}/publication") class="stack">{csrf(&user.csrf)}<input type="hidden" name="version" value=version.to_string()/><label class="checkbox"><input type="checkbox" name="confirmed" value="true" required/>{tr("Yeni grup anahtarlarında adres yenileme bağlantısı eklenmesini onaylıyorum.")}</label><button class="button primary" type="submit">{tr("Yayımlamayı etkinleştir")}</button></form>})}</section>}.into_any())
}

pub fn farm_certificates(
	user: &User,
	id: Uuid,
	name: &str,
	version: Uuid,
	certificates: Vec<(String, String, String)>,
) -> Html<String> {
	let rows=certificates.into_iter().map(|(name,expires,hash)|view!{<tr><td>{name}</td><td>{expires}</td><td><code dir="ltr">{hash}</code></td></tr>}).collect_view();
	layout(user,"Grup sertifikaları","servers",view!{<div class="section-heading"><h2>{name.to_owned()}</h2><a class="button secondary" href=format!("/manage/farms/{id}/publication")>{tr("Gruba dön")}</a></div><section class="card table-wrap"><h2>{tr("Grup sertifikaları")}</h2><table><thead><tr><th>{tr("TLS adı")}</th><th>{tr("Son tarih")}</th><th>"SHA-256"</th></tr></thead><tbody>{rows}</tbody></table></section>{user.can_write().then(||view!{<section class="card"><h2>{tr("PFX sertifikası yükle")}</h2><div class="alert warning">{tr("Sertifika ve özel anahtar grubun mevcut ve yeni üyelerine dağıtılır. Varsayılan değişimi yeni bağlantıları etkiler; adres yenileme bağlantısı olmayan eski anahtarlar yeniden paylaşılmalıdır. VPN adresleri ve sunucu sırları korunur.")}</div><form method="post" action=format!("/manage/farms/{id}/certificates") class="stack">{csrf(&user.csrf)}<input type="hidden" name="base_code" value=version.to_string()/><label>{tr("TLS adı")}<input name="tls_name" required maxlength="253"/></label><label>{tr("PFX dosyası")}<input type="file" accept=".pfx,.p12" data-pfx-file="pfx-data"/></label><label>{tr("PFX verisi (Base64)")}<textarea id="pfx-data" name="pfx" dir="ltr" rows="4" required maxlength="350000"/></label><label>{tr("PFX parolası")}<input name="password" type="password" maxlength="1024" autocomplete="new-password"/></label><label class="checkbox"><input name="make_default" type="checkbox" value="true"/>{tr("Varsayılan sertifika olarak kullan")}</label><label class="checkbox"><input name="confirmed" type="checkbox" value="true" required/>{tr("Grup üyelerine özel anahtar ve sertifika dağıtılmasını onaylıyorum.")}</label><button class="button primary" type="submit">{tr("Doğrula ve yükle")}</button></form></section>})}}.into_any())
}

pub fn destinations(
	user: &User,
	farm: Uuid,
	name: &str,
	items: Vec<crate::publication::Destination>,
	servers: Vec<(Uuid, String)>,
) -> Html<String> {
	let rows=items.into_iter().map(|d|view!{<tr><td>{d.name}</td><td>{d.seed_name}</td><td><bdi dir="ltr">{d.publish_url}</bdi></td><td>{tr(&d.status)}<p class="small">{publication_message(&d.message)}</p></td><td>{if d.auto_minutes==0{tr("Elle")}else{format!("{} min",d.auto_minutes)}}</td><td><bdi dir="ltr">{d.last_attempt.map(|t|t.to_rfc3339()).unwrap_or_else(||"—".into())}</bdi></td><td><bdi dir="ltr">{d.last_success.map(|t|t.to_rfc3339()).unwrap_or_else(||"—".into())}</bdi></td><td><div class="actions">{user.is_admin().then(||view!{<a class="button secondary" href=format!("/manage/publications/{}",d.id)>{tr("Düzenle")}</a><form method="post" action=format!("/manage/publications/{}/remove",d.id) data-confirm=tr("Bu yayım hedefi ve kayıtlı kimliği kaldırılacak. Eski anahtarlardaki bağlantı ve harici dosya kendiliğinden silinmez. Devam edilsin mi?")>{csrf(&user.csrf)}<input type="hidden" name="version" value=d.version.to_string()/><input type="hidden" name="confirmed" value="true"/><button class="button danger" type="submit">{tr("Kaldır")}</button></form>})}{(user.can_write() && d.enabled).then(||view!{<form method="post" action=format!("/manage/publications/{}/push",d.id) data-confirm=tr("Şifreli grup adresi harici hedeflere gönderilecek. Devam edilsin mi?")>{csrf(&user.csrf)}<input name="version" type="hidden" value=d.version.to_string()/><input name="confirmed" type="hidden" value="true"/><button class="button secondary" type="submit">{tr("Şimdi yayımla")}</button></form>})}</div></td></tr>}).collect_view();
	layout(user,"Harici adres yayımı","servers",view!{<div class="section-heading"><h2>{name.to_owned()}</h2><a class="button secondary" href=format!("/manage/farms/{farm}/publication")>{tr("Gruba dön")}</a></div><section class="card"><h2>{tr("Harici adres yayımı")}</h2><p class="muted">{tr("Şifreli grup adresini kendi dosya veya CDN hizmetinize gönderin. Doğrulanan adresler yalnız seçilen başlangıç sunucusunun yeni grup anahtarlarına eklenir. Eski anahtarlar yeniden paylaşılmalıdır.")}</p>{user.can_write().then(||view!{<form method="post" action=format!("/manage/farms/{farm}/push") data-confirm=tr("Şifreli grup adresi harici hedeflere gönderilecek. Devam edilsin mi?")>{csrf(&user.csrf)}<input name="confirmed" type="hidden" value="true"/><button class="button primary" type="submit">{tr("Tümünü yayımla")}</button></form>})}<div class="table-wrap"><table><thead><tr><th>{tr("Ad")}</th><th>{tr("Başlangıç sunucusu")}</th><th>{tr("Yayım adresi")}</th><th>{tr("Durum")}</th><th>{tr("Otomatik yayım")}</th><th>{tr("Son deneme (UTC)")}</th><th>{tr("Son başarılı yayım (UTC)")}</th><th>{tr("İşlem")}</th></tr></thead><tbody>{rows}</tbody></table></div></section>{user.is_admin().then(||view!{<section class="card"><h2>{tr("Yayım hedefi ekle")}</h2>{destination_form(user,farm,None,servers)}</section>})}}.into_any())
}
pub fn destination_edit(
	user: &User,
	item: crate::publication::Destination,
	servers: Vec<(Uuid, String)>,
) -> Html<String> {
	let farm = item.farm;
	layout(user,"Harici adres yayımı","servers",view!{<div class="section-heading"><h2>{item.name.clone()}</h2><a class="button secondary" href=format!("/manage/farms/{farm}/destinations")>{tr("Yayım listesine dön")}</a></div><section class="card">{destination_form(user,farm,Some(item),servers)}</section>}.into_any())
}
fn destination_form(
	user: &User,
	farm: Uuid,
	item: Option<crate::publication::Destination>,
	servers: Vec<(Uuid, String)>,
) -> AnyView {
	let create = item.is_none();
	let seed = item.as_ref().map(|d| d.seed);
	let options = servers
		.into_iter()
		.map(
			|(id, name)| view! {<option value=id.to_string() selected=seed==Some(id)>{name}</option>},
		)
		.collect_view();
	let action = item
		.as_ref()
		.map(|d| format!("/manage/publications/{}", d.id))
		.unwrap_or_else(|| format!("/manage/farms/{farm}/destinations"));
	let version = item
		.as_ref()
		.map(|d| d.version.to_string())
		.unwrap_or_default();
	view!{<form method="post" action=action class="stack">{csrf(&user.csrf)}{(!create).then(||view!{<input type="hidden" name="version" value=version/>})}<label>{tr("Ad")}<input name="name" required maxlength="120" value=item.as_ref().map(|d|d.name.clone()).unwrap_or_default()/></label><label>{tr("Başlangıç sunucusu")}{help("Başlangıç sunucusu", "Her erişim anahtarı adres dosyasını ilk sunucusunun sırrıyla çözer. Farklı başlangıç sunucularının her biri için ayrı yayım hedefi oluşturun.")}<select name="seed" required>{options}</select></label><label>{tr("Yayım adresi")}<input type="url" name="publish_url" dir="ltr" required maxlength="2048" value=item.as_ref().map(|d|d.publish_url.clone()).unwrap_or_default()/></label><p class="small muted">{tr("İstemcilerin okuyacağı genel HTTPS dosya adresi. Yükleme kimliği bu adrese gönderilmez.")}</p><label>{tr("Otomatik yayım aralığı (dakika)")}<input type="number" name="auto_minutes" min="0" max="1440" value=item.as_ref().map(|d|d.auto_minutes).unwrap_or(0)/></label><p class="small muted">{tr("0 yalnız elle; 5–1440 dakika düzenli yayım. Panel yeniden başlasa da sıradaki işler korunur.")}</p><label class="checkbox"><input type="checkbox" name="enabled" value="true" checked=item.as_ref().is_none_or(|d|d.enabled)/>{tr("Etkin")}</label>{(!create).then(||view!{<label class="checkbox"><input type="checkbox" name="replace_settings" value="true"/>{tr("Yükleme ayarlarını değiştir; aşağıdaki alanları yeniden gir")}</label><p class="small muted">{tr("Kayıtlı kimlik ve şablonlar gösterilmez. Bu seçenek kapalıysa mevcut yükleme ayarları korunur.")}</p>})}{create.then(||view!{<input name="replace_settings" type="hidden" value="true"/>})}<label>{tr("Yükleme adresi")}<input type="url" name="upload_url" dir="ltr" maxlength="2048" required=create/></label><label>{tr("HTTP yöntemi")}<select name="method"><option value="PUT_POST">"PUT → POST"</option><option value="PUT">"PUT"</option><option value="POST">"POST"</option></select></label><label>{tr("Yayıncı kimliği")}<input type="password" name="secret" autocomplete="new-password" maxlength="4096"/></label><label>{tr("Başlıklar")}<textarea name="headers" dir="ltr" rows="3" maxlength="16384" placeholder="Authorization: Bearer {access_token}"/></label><label>{tr("Form verisi")}<textarea name="form_data" dir="ltr" rows="3" maxlength="16384" placeholder="payload: {farm_token}"/></label><label>{tr("Gövde")}<textarea name="body" dir="ltr" rows="3" maxlength="32768">{if create{"{farm_token}"}else{""}}</textarea></label><label>{tr("Gövde içerik türü")}{help("Yükleme ayarları", "Ham şifreli metin için text/plain kullanın. Hedef API başka bir gövde biçimi gerektiriyorsa şablonu ve içerik türünü buna göre ayarlayın. Form verisi multipart olarak gönderilir; gövdeyle birlikte kullanılmaz.")}<input name="content_type" dir="ltr" value="text/plain; charset=utf-8" maxlength="120"/></label><p class="small muted">{tr("{farm_token} şifreli adres, {access_token} yayıncı kimliğidir. Alanları Ad: Değer satırlarıyla girin. Form verisi kullanırken gövdeyi boş bırakın; multipart içerik türü otomatik belirlenir. Ham metin JSON ile sarılmaz.")}</p><label class="checkbox"><input name="confirmed" type="checkbox" value="true" required/>{tr("Şifreli adresin ve girdiğim yayıncı kimliğinin belirtilen harici hedefe gönderilmesini onaylıyorum.")}</label><button class="button primary" type="submit">{tr("Kaydet ve sıraya al")}</button></form>}.into_any()
}

fn publication_message(message: &str) -> String {
	for (prefix, label) in [
		("Yükleme HTTP ", "Yükleme HTTP"),
		("Dosya denetimi HTTP ", "Dosya denetimi HTTP"),
	] {
		if let Some(code) = message.strip_prefix(prefix) {
			return format!("{} {}", tr(label), code);
		}
	}
	tr(message)
}

#[allow(clippy::too_many_arguments)]
pub fn farm_server_profile(
	user: &User,
	id: Uuid,
	name: &str,
	version: Uuid,
	selected: Option<Uuid>,
	members: i64,
	profiles: Vec<(Uuid, String, Uuid)>,
) -> Html<String> {
	let rows=profiles.into_iter().map(|(profile,name,profile_version)| view! {
		<section class="card"><h2>{name}{(selected==Some(profile)).then(||view!{<span class="badge">{tr("Atanmış profil")}</span>})}</h2>
		{user.can_write().then(||view!{<form method="post" action=format!("/manage/farms/{id}/server-profile") class="stack">
			{csrf(&user.csrf)}<input type="hidden" name="version" value=version.to_string()/><input type="hidden" name="profile" value=profile.to_string()/><input type="hidden" name="profile_version" value=profile_version.to_string()/>
			<label class="checkbox"><input name="confirmed" type="checkbox" value="true" required/>{tr("Mevcut ve gelecekteki grup üyelerine bu profilin uygulanmasını onaylıyorum.")}</label><button class="button primary" type="submit">{tr("Ata ve uygula")}</button>
		</form>})}</section>
	}).collect_view();
	layout(user,"Grup sunucu profili","servers",view!{
		<div class="section-heading"><h2>{name.to_owned()}</h2><div class="actions"><a class="button secondary" href="/manage/server-profiles">{tr("Sunucu profilleri")}</a><a class="button secondary" href=format!("/manage/farms/{id}/publication")>{tr("Gruba dön")}</a></div></div>
		<section class="card"><h2>{tr("Grup sunucu profili")}{help("Otomatik profil uygulaması","Atama mevcut üyelere hemen uygulanır. Yeni veya gruba taşınan sunucu güncel profili alır. Profil kaydedilince bağlı tüm gruplar tek veritabanı işleminde güncellenir; çevrimdışı sunucu yeniden bağlandığında alır. Tekil ayarlar sonraki profil değişiminde üzerine yazılır. Atamayı kaldırmak son ayarları geri almaz.")}</h2><p>{tr("Grup üyesi sayısı")}{": "}{members}</p><div class="alert warning">{tr("Bu işlem dinleyici, ağ ve kapasite ayarlarını değiştirebilir. Sertifika, sunucu sırrı ve istemci adresi korunur. Dinleyici portlarının duyurulan adres ve güvenlik duvarıyla uyumunu denetleyin.")}</div><p>{tr(if selected.is_some(){"Atanmış profil"}else{"Profil atanmamış"})}</p></section>
		{rows}
		{(selected.is_some()&&user.can_write()).then(||view!{<section class="card"><h2>{tr("Profil atamasını kaldır")}</h2><form method="post" action=format!("/manage/farms/{id}/server-profile") class="stack">{csrf(&user.csrf)}<input type="hidden" name="version" value=version.to_string()/><input type="hidden" name="profile" value=""/><label class="checkbox"><input name="confirmed" type="checkbox" value="true" required/>{tr("Otomatik profil uygulamasını durdurmayı onaylıyorum. Son ayarlar korunacak.")}</label><button class="button secondary" type="submit">{tr("Atamayı kaldır")}</button></form></section>})}
	}.into_any())
}

pub fn autosign(user: &User, server: Uuid, name: &str, order: Option<Value>) -> Html<String> {
	let data = order.unwrap_or_else(|| json!({"staging":true,"enabled":true}));
	let text = |key: &str| {
		let value = data[key].as_str().unwrap_or("");
		DateTime::parse_from_rfc3339(value)
			.map(|v| v.with_timezone(&Utc).to_rfc3339())
			.unwrap_or_else(|_| value.to_owned())
	};
	let version = text("version");
	let status = text("status");
	let message = text("message");
	let details = ["last_attempt", "last_success", "expires_at", "next_due"]
		.into_iter()
		.zip([
			"Son deneme (UTC)",
			"Son başarılı işlem (UTC)",
			"Son tarih",
			"Sonraki kontrol (UTC)",
		])
		.map(
			|(key, title)| view! {<p><strong>{tr(title)}{": "}</strong><bdi dir="ltr">{text(key)}</bdi></p>},
		)
		.collect_view();
	layout(user,"Otomatik sertifika (AutoSign)","servers",view!{
		<div class="section-heading"><h2>{name.to_owned()}</h2><a class="button secondary" href=format!("/manage/servers/{server}/certificates")>{tr("Sertifikalar")}</a></div>
		<section class="card"><h2>{tr("Otomatik sertifika (AutoSign)")}{help("AutoSign nasıl çalışır?","Alan adının DNS kaydı VPN sunucusuna gitmeli; dış TCP80 doğrulaması sunucuya ulaşmalıdır. Test CA modu sertifikayı VPN'e uygulamaz. Gerçek CA modu geçerli PFX'i varsayılan sertifika olarak uygular. Günde iki kez süre kontrol edilir; yalnız gerektiğinde yenilenir. Kapatmak son sertifikayı kaldırmaz. Eski erişim anahtarlarının parmak izi adres yayımı veya yeniden paylaşım yoluyla güncellenmelidir.")}</h2>
		<p>{tr("Durum")}{": "}{tr(&status)}</p><p>{tr(&message)}</p>{details}
		</section>
		{user.is_admin().then(||view!{<section class="card"><form method="post" action=format!("/manage/servers/{server}/autosign") class="stack">{csrf(&user.csrf)}
			{(!version.is_empty()).then(||view!{<input type="hidden" name="version" value=version/>})}
			<label>{tr("Alan adı (FQDN)")}<input name="domain" value=text("domain") required maxlength="253"/></label>
			<label>{tr("CA iletişim e-postası")}<input name="email" type="email" value=text("email") required maxlength="254"/></label>
			<label class="checkbox"><input name="staging" type="checkbox" value="true" checked=data["staging"].as_bool().unwrap_or(true)/>{tr("Test CA: doğrula, VPN sertifikasını değiştirme")}</label>
			<label class="checkbox"><input name="enabled" type="checkbox" value="true" checked=data["enabled"].as_bool().unwrap_or(true)/>{tr("Otomatik yenileme etkin")}</label>
			<p><a class="text-link" href="https://letsencrypt.org/repository/" target="_blank" rel="noopener">{tr("Let's Encrypt hizmet koşulları")}</a></p>
			<label class="checkbox"><input name="agree_tos" type="checkbox" value="true"/>{tr("Alan adı ve e-postanın Let's Encrypt'e gönderilmesini ve hizmet koşullarını kabul ediyorum.")}</label>
			<label class="checkbox"><input name="confirmed" type="checkbox" value="true" required/>{tr("HTTP-01 dinleyicisinin geçici açılmasını ve gerçek CA modunda sertifika değişikliğini onaylıyorum.")}</label>
			<button class="button primary" type="submit">{tr("Kaydet ve kontrol et")}</button>
		</form></section>})}
	}.into_any())
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn html_input_is_escaped() {
		let html = error("<script>alert(1)</script>").0;
		assert!(!html.contains("<script>alert(1)</script>"));
		assert!(html.contains("&lt;script&gt;"));
	}
}
