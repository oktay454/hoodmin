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
pub struct Data {
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
	let row=query("SELECT count(*) AS total,count(*) FILTER(WHERE last_seen>now()-interval '90 seconds' AND NOT paused) AS online FROM servers WHERE tenant_id=$1").bind(tenant).fetch_one(&mut *tx).await?;
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
	let rows=query("SELECT s.id,s.name,s.farm_id,s.paused,s.last_seen,s.status,s.configuration,f.name AS group_name FROM servers s LEFT JOIN farms f ON f.tenant_id=s.tenant_id AND f.id=s.farm_id WHERE s.tenant_id=$1 ORDER BY s.name LIMIT 500").bind(user.tenant).fetch_all(&mut *tx).await?;
	let servers = rows
		.into_iter()
		.filter(|r| {
			current != "servers" || r.get::<String, _>("name").to_lowercase().contains(&needle)
		})
		.map(|r| -> Result<Server> {
			let paused: bool = r.get("paused");
			let seen: Option<DateTime<Utc>> = r.get("last_seen");
			let status: Option<Value> = r.get("status");
			let online =
				seen.is_some_and(|s| s > Utc::now() - chrono::Duration::seconds(90)) && !paused;
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
		query("SELECT a.id,a.email,a.active,m.role FROM memberships m JOIN accounts a ON a.id=m.account_id WHERE m.tenant_id=$1 ORDER BY a.email LIMIT 200").bind(user.tenant).fetch_all(&mut *tx).await?.into_iter().map(|r|Account{id:r.get::<Uuid,_>("id").to_string(),email:r.get("email"),role:r.get("role"),active:r.get("active")}).collect()
	} else {
		Vec::new()
	};
	let audit=query("SELECT created_at,actor_email,action,resource FROM audit_events WHERE tenant_id=$1 ORDER BY id DESC LIMIT 200").bind(user.tenant).fetch_all(&mut *tx).await?.into_iter().map(|r|Audit{time:time(Some(r.get("created_at"))),email:r.get("actor_email"),action:r.get("action"),resource:r.get("resource")}).collect();
	Ok(Data {
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
		("audit", "Denetim kaydı", "/panel/audit"),
	];
	let items=navigation.into_iter().map(|(id,label,url)|view!{<a href=url class:active={active==id}>{icon(id)}<span>{tr(label)}</span></a>}).collect_view();
	document(&title.clone(),view!{
		<div class="app-shell"><aside class="sidebar"><a class="brand" href="/panel"><span class="brand-mark">{tr("HM")}</span><span>{tr("Hoodmin")}<small>{tr("Yönetim konsolu")}</small></span></a><div class="workspace"><span class="eyebrow">{tr("ÇALIŞMA ALANI")}</span><strong>{user.tenant_name.clone()}</strong><span class="small muted">{tr("Kendi sunucularınız, tek panel.")}</span></div><nav aria-label=tr("Ana gezinme")><span class="nav-section">{tr("ALTYAPI VE ERİŞİM")}</span>{items}{user.is_admin().then(||view!{<a href="/panel/users" class:active={active=="users"}>{icon("users")}<span>{tr("Panel kullanıcıları")}</span></a>})}</nav><div class="sidebar-foot"><span class="connection-dot"></span><span>{tr("Özel altyapınızda çalışıyor")}</span></div></aside><div class="main-shell"><header class="topbar"><div><span class="eyebrow">{tr("KONTROL PANELİ")}</span><h1>{title}</h1></div><div class="profile"><a class="account-link" href="/panel/account">{icon("account")}<span><strong>{tr("Hesabım")}</strong><bdi dir="ltr">{user.email.clone()}</bdi></span><span aria-hidden="true">{tr("↗")}</span></a>{theme_switch()}{language_switch()}<span class="role-pill">{role_label(&user.role)}</span><form method="post" action="/logout">{csrf(&user.csrf)}<button class="button ghost" type="submit">{tr("Çıkış")}</button></form></div></header><main>{content}</main><footer>{tr("Hoodmin · Saatler İstanbul (UTC+03) · Kendi özel altyapınız.")}</footer></div></div>
	}.into_any())
}
fn role_label(role: &str) -> String {
	tr(match role {
		"admin" => "Yönetici",
		"operator" => "Operatör",
		_ => "Salt okunur",
	})
}
pub fn signin(token: &str, error: Option<&str>) -> Html<String> {
	document("Giriş",view!{<div class="login-shell"><section class="login-intro"><span class="brand-mark large">{tr("HM")}</span><span class="eyebrow">{tr("HOODMIN")}</span><h1>{tr("Sunucularınızın kontrolü sizde.")}</h1><p>{tr("Sunucuları izleyin, erişim anahtarlarını yönetin ve kullanımınızı tek yerden takip edin.")}</p><div class="intro-points"><span>{tr("01 · Kiracınıza özel çalışma alanı")}</span><span>{tr("02 · Toplu erişim yönetimi")}</span><span>{tr("03 · Canlı sunucu durumu")}</span></div></section><section class="login-card"><div class="appearance-controls">{theme_switch()}{language_switch()}</div><span class="eyebrow">{tr("HOŞ GELDİNİZ")}</span><h2>{tr("Çalışma alanınıza giriş yapın")}</h2><p class="muted">{tr("Yöneticinizin oluşturduğu hesabı kullanın.")}</p>{error.map(|message|view!{<p class="alert error" role="alert">{tr(message)}</p>})}<form method="post" action="/signin" class="stack">{csrf(token)}<label for="email">{tr("E-posta")}</label><input id="email" type="email" name="email" autocomplete="username" required maxlength="254"/><label for="password">{tr("Parola")}</label><input id="password" type="password" name="password" autocomplete="current-password" required maxlength="128"/><button class="button primary" type="submit">{tr("Giriş yap")}<span>{tr("→")}</span></button></form><p class="small muted login-note">{tr("Hesabınıza erişemiyorsanız çalışma alanı yöneticinizle iletişime geçin.")}</p></section></div>}.into_any())
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
	layout(user,title,page,view!{<div class="page-content">{(notice=="saved").then(||view!{<div class="alert success" role="status">{tr("Değişiklikler kaydedildi.")}</div>})}{content}</div>}.into_any())
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
			"Son 90 saniyede bildirim aldı",
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
		<div class="dashboard-grid"><section class="card status-panel"><header class="panel-heading"><h2>{tr("Sunucu durumu")}{help("Sunucu yönetimi", "Her sunucu yalnız kendi çalışma alanınıza aittir. Önce sunucuyu kaydedin, ardından oluşturulan bağlantı ayarını VPN sunucusuna uygulayın. Çevrimiçi durumu son 90 saniyedeki bildirimlere göre belirlenir.")}</h2><a class="text-link" href="/panel/servers">{tr("Tümünü görüntüle →")}</a></header><div class="table-wrap"><table><thead><tr><th>{tr("Sunucu")}</th><th>{tr("Durum")}</th><th>{tr("CPU")}</th><th>{tr("Bellek")}</th><th>{tr("Oturum")}</th><th>{tr("Son bildirim")}</th></tr></thead><tbody>{rows}</tbody></table></div>{no_servers.then(||view!{<p class="empty-state">{tr("Henüz sunucunuz yoksa Sunucular sayfasından ekleyebilirsiniz.")}</p>})}<div class="panel-note">{tr("Sunucularınız kendi özel altyapınızda çalışır.")}</div></section>
		<aside class="card quick-actions"><header class="panel-heading"><h2>{tr("Hızlı erişim")}</h2></header><a class="action-tile" href="/panel/servers">{icon("servers")}<span><strong>{tr("Sunucular")}</strong><small>{tr("Bağlantı, durum ve bakım ayarları")}</small></span><span class="action-arrow" aria-hidden="true">"↗"</span></a><a class="action-tile" href="/panel/tokens">{icon("tokens")}<span><strong>{tr("Erişim anahtarları")}</strong><small>{tr("Toplu üretim, kota ve erişim yönetimi")}</small></span><span class="action-arrow" aria-hidden="true">"↗"</span></a><a class="action-tile" href="/panel/sessions">{icon("sessions")}<span><strong>{tr("VPN oturumları")}</strong><small>{tr("Bağlantıları ve oturum geçmişini inceleyin")}</small></span><span class="action-arrow" aria-hidden="true">"↗"</span></a><a class="action-tile" href="/panel/audit">{icon("audit")}<span><strong>{tr("Denetim kaydı")}</strong><small>{tr("Yapılan yönetim işlemlerini izleyin")}</small></span><span class="action-arrow" aria-hidden="true">"↗"</span></a></aside></div>
	}.into_any()
}
fn farm_select(farms: Vec<Farm>, selected: String) -> impl IntoView {
	view! {<select name="farm" aria-label=tr("Sunucu grubu")><option value="">{tr("Grupsuz")}</option>{farms.into_iter().map(|f|view!{<option value=f.id.clone() selected={selected==f.id}>{f.name}</option>}).collect_view()}</select>}
}
fn servers(user: &User, data: Data) -> AnyView {
	let farms = data.farms.clone();
	let farm_options = data.farms.clone();
	let rows=data.servers.into_iter().map(|server|{
		let controls=user.can_write().then(||view!{<details class="edit-details"><summary>{tr("Düzenle")}</summary><form method="post" action=format!("/manage/servers/{}",server.id) class="stack compact-form">{csrf(&user.csrf)}<label>{tr("Sunucu adı")}<input name="name" value=server.name.clone() required maxlength="120"/></label><label>{tr("VPN adresi (DNS veya IP)")}<input name="host" value=server.host.clone() required maxlength="253"/></label><label>{tr("TCP portu")}<input type="number" name="port" value=server.port min="1" max="65535" required/></label><p class="small muted">{tr("Adres veya port değişikliği sunucunun sonraki bildiriminde uygulanır. Daha önce paylaşılan erişim anahtarlarındaki adres kendiliğinden güncellenmez.")}</p>{farm_select(farms.clone(),server.farm.clone())}<label class="checkbox"><input type="checkbox" name="paused" value="true" checked=server.paused/>{tr("Yeni oturumları durdur (bakım)")}</label><button class="button secondary" type="submit">{tr("Kaydet")}</button></form><div class="credential-action"><p class="small muted">{tr("Kimliği yenilerseniz yeni bağlantı ayarını sunucuya uygulamanız gerekir.")}</p><a class="button danger-outline" href=format!("/manage/servers/{}/credential",server.id)>{tr("Bağlantı kimliğini yenile")}</a>{help("Bağlantı kimliği", "Bu kimlik VPN sunucusunun yönetim paneline bağlanmasını sağlar. Yenileme eski kimliği geçersiz kılar. Yeni ayarı sunucuya uygulayana kadar durum ve yönetim iletişimi durur. VPN erişim anahtarları ayrı bilgilerdir.")}</div></details>});
		view!{<tr><td><strong>{server.name}</strong><span class="table-sub"><bdi dir="auto">{tr(&server.seen)}</bdi></span></td><td>{server.group.unwrap_or_else(||tr("Grupsuz"))}</td><td><span class="badge" class:online=server.online>{tr(&server.status)}</span></td><td><bdi dir="ltr">{server.cpu}</bdi></td><td><bdi dir="ltr">{server.memory}</bdi></td><td><bdi dir="ltr">{server.sessions}</bdi></td><td>{controls}</td></tr>}
	}).collect_view();
	view!{<div class="section-heading"><div><h2>{tr("Kendi sunucularınızı yönetin")}{help("Sunucu yönetimi", "Her sunucu yalnız kendi çalışma alanınıza aittir. Önce sunucuyu kaydedin, ardından oluşturulan bağlantı ayarını VPN sunucusuna uygulayın. Çevrimiçi durumu son 90 saniyedeki bildirimlere göre belirlenir.")}</h2><p class="muted">{tr("Her sunucu yalnızca çalışma alanınıza aittir.")}</p></div><form method="get" class="search"><input type="search" name="q" value=data.query placeholder=tr("Sunucu ara") aria-label=tr("Sunucu ara")/><button class="button ghost" type="submit">{tr("Ara")}</button></form></div><section class="card table-wrap"><table><thead><tr><th>{tr("Sunucu")}</th><th>{tr("Grup")}</th><th>{tr("Durum")}</th><th>{tr("CPU")}</th><th>{tr("Bellek")}</th><th>{tr("Oturum")}</th><th>{tr("Yönetim")}</th></tr></thead><tbody>{rows}</tbody></table></section>{user.can_write().then(||view!{<div class="form-grid"><section class="card"><span class="eyebrow">{tr("YENİ SUNUCU")}</span><h2>{tr("Sunucu ekle")}</h2><form method="post" action="/manage/servers" class="stack">{csrf(&user.csrf)}<label for="server-name">{tr("Sunucu adı")}</label><input id="server-name" name="name" required maxlength="120" placeholder=tr("Örn. İstanbul 01")/><label for="server-host">{tr("VPN adresi (DNS veya IP)")}{help("VPN bağlantı adresi", "VPN istemcilerinin bağlanacağı sunucunun dış IP adresini veya DNS adını yazın. Bu alan yönetim panelinin adresi değildir. TCP portu VPN sunucusunun dinlediği ve güvenlik duvarında açılmış port olmalıdır.")}</label><input id="server-host" name="host" required maxlength="253" placeholder=tr("vpn.ornek.com")/><label for="server-port">{tr("TCP portu")}</label><input id="server-port" type="number" name="port" value="443" min="1" max="65535" required/><label>{tr("Sunucu grubu")}{farm_select(farm_options,String::new())}</label><button class="button primary" type="submit">{tr("Sunucuyu kaydet")}</button><p class="small muted">{tr("Kayıttan sonra sunucunuza ekleyeceğiniz bağlantı ayarları gösterilir.")}</p></form></section><section class="card"><span class="eyebrow">{tr("DÜZENLEME")}</span><h2>{tr("Sunucu grubu oluştur")}{help("Sunucu grupları", "Gruplar sunucuları bölge veya amaca göre düzenler. Şu anda grup oluşturmak yük dağıtımını etkinleştirmez ve sunucuyu başka kiracılarla paylaşmaz.")}</h2><p class="muted">{tr("Sunucularınızı bölge veya kullanım amacına göre gruplandırın.")}</p><form method="post" action="/manage/farms" class="stack">{csrf(&user.csrf)}<label for="farm-name">{tr("Grup adı")}</label><input id="farm-name" name="name" required maxlength="120" placeholder=tr("Örn. Türkiye")/><button class="button secondary" type="submit">{tr("Grup oluştur")}</button></form></section></div>})}}.into_any()
}
fn tokens(user: &User, data: Data) -> AnyView {
	let paging = pagination(data.page, &data.query, data.more);
	let search = data.query.clone();
	let rows=data.tokens.into_iter().map(|token|{
		let controls=user.can_write().then(||view!{<form method="post" action="/manage/tokens/bulk" data-confirm={(!token.revoked).then(||tr("Bu erişim anahtarı iptal edilecek. Sunucunun sonraki bildiriminde ilgili bağlantılar kapatılabilir. Devam edilsin mi?"))}>{csrf(&user.csrf)}<input type="hidden" name="ids" value=token.id.clone()/><input type="hidden" name="action" value=if token.revoked{"enable"}else{"revoke"}/><button class="button ghost" type="submit">{tr(if token.revoked{"Etkinleştir"}else{"İptal et"})}</button></form>});
		view!{<tr><td>{user.can_write().then(||view!{<input type="checkbox" class="token-select" value=token.id aria-label=tr("{name} anahtarını seç").replace("{name}",&token.name)/>})}</td><td><strong>{token.name}</strong><span class="table-sub">{token.server}</span></td><td><bdi dir="ltr">{token.traffic}</bdi></td><td><bdi dir="auto">{tr(&token.quota)}</bdi></td><td><bdi dir="ltr">{token.clients}</bdi></td><td><bdi dir="auto">{tr(&token.expires)}</bdi></td><td><span class="badge" class:online={token.state=="Etkin"}>{tr(&token.state)}</span></td><td>{controls}</td></tr>}
	}).collect_view();
	let options = data
		.servers
		.into_iter()
		.map(|s| view! {<option value=s.id>{s.name}</option>})
		.collect_view();
	view!{<div class="section-heading"><div><h2>{tr("Erişimi ihtiyacınıza göre sınırlandırın")}</h2><p class="muted">{tr("Anahtarları tek tek veya topluca yönetin.")}</p></div><div class="actions"><form method="get" class="search"><input type="search" name="q" value=search placeholder=tr("Anahtar ara") aria-label=tr("Anahtar ara")/><button class="button ghost" type="submit">{tr("Ara")}</button></form><a class="button secondary" href="/export/usage.csv">{tr("Kullanım raporunu indir")}</a></div></div><section class="card table-wrap"><table><thead><tr><th>{user.can_write().then(||view!{<input type="checkbox" id="select-all" aria-label=tr("Tüm anahtarları seç")/>})}</th><th>{tr("Anahtar / Sunucu")}</th><th>{tr("Kullanım")}</th><th>{tr("Kota")}</th><th>{tr("Cihaz")}</th><th>{tr("Son tarih")}</th><th>{tr("Durum")}</th><th>{tr("İşlem")}</th></tr></thead><tbody>{rows}</tbody></table></section>{paging}{user.can_write().then(||view!{<section class="card bulk-bar"><form method="post" action="/manage/tokens/bulk" id="bulk-form" class="bulk-form" data-confirm=tr("Seçtiğiniz anahtarların erişim veya kota ayarları değiştirilecek. Seçimi ve işlemi kontrol edin. Devam edilsin mi?")>{csrf(&user.csrf)}<input type="hidden" id="selected-ids" name="ids"/><strong><span id="selected-count">{tr("0")}</span>{tr(" anahtar seçildi")}</strong><select name="action" aria-label=tr("Toplu işlem")><option value="revoke">{tr("Seçilenleri iptal et")}</option><option value="enable">{tr("Seçilenleri etkinleştir")}</option><option value="quota">{tr("Seçilenlerin kotasını değiştir")}</option><option value="expiry">{tr("Seçilenlerin süresini değiştir")}</option></select><input type="number" name="max_gib" value="0" min="0" max="1000000" aria-label=tr("Yeni trafik kotası GiB")/><input type="number" name="max_clients" value="1" min="1" max="10000" aria-label=tr("Yeni cihaz kotası")/><input name="expires" placeholder=tr("Son tarih; boş = süresiz") aria-label=tr("Yeni son kullanma zamanı")/><button class="button secondary" type="submit">{tr("Uygula")}</button></form></section><section class="card"><span class="eyebrow">{tr("TEKLİ VE TOPLU ÜRETİM")}</span><h2>{tr("Erişim anahtarı oluştur")}{help("VPN erişim anahtarı", "VPN istemcisinin bağlanması için kullanılır; panel hesabı değildir. Adet alanıyla en fazla 100 anahtar oluşturabilirsiniz. Trafik kotasında 0 sınırsızdır; son kullanma zamanı boşsa süre sınırı yoktur. Zamanı saat dilimiyle ISO 8601 biçiminde yazın.")}</h2><form method="post" action="/manage/tokens" class="form-fields">{csrf(&user.csrf)}<label>{tr("Sunucu")}<select name="server" required>{options}</select></label><label>{tr("Anahtar adı / öneki")}<input name="name" required maxlength="110" placeholder=tr("Örn. Ekip")/></label><label>{tr("Adet")}<input type="number" name="count" value="1" min="1" max="100" required/></label><label><span>{tr("Cihaz sınırı")}{help("Cihaz sınırı", "Bir erişim anahtarını aynı anda kullanabilecek farklı cihazların sayısıdır. Bu değer panel kullanıcılarının sayısını sınırlamaz.")}</span><input type="number" name="max_clients" value="1" min="1" max="10000" required/></label><label>{tr("Trafik kotası (GiB)")}<input type="number" name="max_gib" value="0" min="0" max="1000000" required/><small>{tr("0 = sınırsız")}</small></label><label>{tr("Son kullanma zamanı (isteğe bağlı)")}<input name="expires" placeholder=tr("2026-12-31T23:59:00+03:00")/></label><div class="form-submit"><button class="button primary" type="submit">{tr("Anahtarları oluştur")}</button></div></form></section>})}}.into_any()
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
	let rows=data.sessions.into_iter().map(|session|view!{<tr><td><strong>{format!("#{}",session.id)}</strong></td><td>{session.token}</td><td>{session.server}</td><td><bdi dir="ltr">{session.created}</bdi></td><td><span class="badge" class:online={!session.closed}>{tr(if session.closed{"Kapalı"}else{"Açık"})}</span></td><td>{(user.can_write()&&!session.closed).then(||view!{<form method="post" action=format!("/manage/sessions/{}/close",session.id) data-confirm=tr("Bu VPN oturumu sunucunun sonraki durum bildiriminde sonlandırılacak. Devam edilsin mi?")>{csrf(&user.csrf)}<button class="button ghost" type="submit">{tr("Sonlandır")}</button></form>})}</td></tr>}).collect_view();
	view!{<div class="section-heading"><div><h2>{tr("Bağlantıları takip edin")}{help("VPN oturumları", "Bir VPN istemcisinin bağlantısını gösterir. Sonlandırma isteği sunucunun sonraki durum bildiriminde uygulanır. Kapalı kayıtlar bağlantı geçmişidir.")}</h2><p class="muted">{tr("Son 200 VPN oturumu. Sonlandırma kararı sunucunun bir sonraki durum bildiriminde iletilir.")}</p></div></div><section class="card table-wrap"><table><thead><tr><th>{tr("Oturum")}</th><th>{tr("Anahtar")}</th><th>{tr("Sunucu")}</th><th>{tr("Başlangıç")}</th><th>{tr("Durum")}</th><th>{tr("İşlem")}</th></tr></thead><tbody>{rows}</tbody></table></section>}.into_any()
}
fn users(user: &User, data: Data) -> AnyView {
	let rows=data.users.into_iter().map(|account|view!{<tr><td><strong>{account.email}</strong></td><td><form method="post" action=format!("/manage/users/{}",account.id) class="inline-form">{csrf(&user.csrf)}<select name="role" aria-label=tr("Kullanıcı rolü")><option value="admin" selected={account.role=="admin"}>{tr("Yönetici")}</option><option value="operator" selected={account.role=="operator"}>{tr("Operatör")}</option><option value="viewer" selected={account.role=="viewer"}>{tr("Salt okunur")}</option></select><label class="checkbox"><input type="checkbox" name="active" value="true" checked=account.active/>{tr("Etkin")}</label><button class="button secondary" type="submit">{tr("Kaydet")}</button></form></td></tr>}).collect_view();
	view!{<div class="section-heading"><div><h2>{tr("Çalışma alanını birlikte yönetin")}{help("Panel kullanıcıları", "Bunlar yönetim paneline giriş yapan hesaplardır. Yönetici tüm işlemleri yapar; operatör sunucuları, anahtarları ve VPN oturumlarını yönetir; salt okunur kullanıcı değişiklik yapamaz. VPN kullanıcısına erişim anahtarı verilir.")}</h2><p class="muted">{tr("Yönetici tüm işlemleri, operatör sunucu ve anahtar işlemlerini yapabilir. Salt okunur rol değişiklik yapamaz.")}</p></div></div><section class="card table-wrap"><table><thead><tr><th>{tr("Kullanıcı")}</th><th>{tr("Rol ve erişim")}</th></tr></thead><tbody>{rows}</tbody></table></section><section class="card"><h2>{tr("Kullanıcı ekle")}</h2><form method="post" action="/manage/users" class="form-fields">{csrf(&user.csrf)}<label>{tr("E-posta")}<input type="email" name="email" autocomplete="off" maxlength="254" required/></label><label>{tr("Başlangıç parolası")}<input type="password" name="password" autocomplete="new-password" minlength="16" maxlength="128" required/></label><label>{tr("Rol")}<select name="role"><option value="viewer">{tr("Salt okunur")}</option><option value="operator">{tr("Operatör")}</option><option value="admin">{tr("Yönetici")}</option></select></label><div class="form-submit"><button class="button primary" type="submit">{tr("Kullanıcı ekle")}</button></div></form></section>}.into_any()
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
