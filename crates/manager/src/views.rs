use crate::{auth::User, store::Store};
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
	pub farm: String,
	pub group: String,
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
	let rows=query("SELECT s.id,s.name,s.farm_id,s.paused,s.last_seen,s.status,f.name AS group_name FROM servers s LEFT JOIN farms f ON f.tenant_id=s.tenant_id AND f.id=s.farm_id WHERE s.tenant_id=$1 ORDER BY s.name LIMIT 500").bind(user.tenant).fetch_all(&mut *tx).await?;
	let servers = rows
		.into_iter()
		.filter(|r| {
			current != "servers" || r.get::<String, _>("name").to_lowercase().contains(&needle)
		})
		.map(|r| {
			let paused: bool = r.get("paused");
			let seen: Option<DateTime<Utc>> = r.get("last_seen");
			let status: Option<Value> = r.get("status");
			let online =
				seen.is_some_and(|s| s > Utc::now() - chrono::Duration::seconds(90)) && !paused;
			let error = status
				.as_ref()
				.and_then(|v| v["configError"].as_str())
				.is_some_and(|e| !e.is_empty());
			Server {
				id: r.get::<Uuid, _>("id").to_string(),
				name: r.get("name"),
				farm: r
					.get::<Option<Uuid>, _>("farm_id")
					.map(|v| v.to_string())
					.unwrap_or_default(),
				group: r
					.get::<Option<String>, _>("group_name")
					.unwrap_or("Grupsuz".into()),
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
			}
		})
		.collect();
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
fn document(title: &str, content: AnyView) -> Html<String> {
	let title = format!("{title} · VpnHood Yönetici");
	Html(format!("<!doctype html>{}",view!{
		<html lang="tr"><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width,initial-scale=1"/><title>{title}</title><link rel="stylesheet" href="/assets/app.css"/><script src="/assets/app.js" defer></script></head><body>{content}</body></html>
	}.to_html()))
}
fn csrf(value: &str) -> AnyView {
	view! {<input type="hidden" name="csrf" value=value.to_owned()/>}.into_any()
}
fn layout(user: &User, title: &str, active: &str, content: AnyView) -> Html<String> {
	let user = user.clone();
	let title = title.to_owned();
	let active = active.to_owned();
	let navigation = [
		("overview", "Genel bakış", "/panel"),
		("servers", "Sunucular", "/panel/servers"),
		("tokens", "Erişim anahtarları", "/panel/tokens"),
		("sessions", "VPN oturumları", "/panel/sessions"),
		("audit", "Denetim kaydı", "/panel/audit"),
	];
	let items=navigation.into_iter().map(|(id,label,url)|view!{<a href=url class:active={active==id}><span class="nav-dot"></span>{label}</a>}).collect_view();
	document(&title.clone(),view!{
		<div class="app-shell"><aside class="sidebar"><a class="brand" href="/panel"><span class="brand-mark">"VH"</span><span>"VpnHood"<small>"Yönetici"</small></span></a><div class="workspace"><span class="eyebrow">"ÇALIŞMA ALANI"</span><strong>{user.tenant_name.clone()}</strong><span class="small muted">"Kendi sunucularınız, tek panel."</span></div><nav aria-label="Ana gezinme">{items}{user.is_admin().then(||view!{<a href="/panel/users" class:active={active=="users"}><span class="nav-dot"></span>"Kullanıcılar"</a>})}</nav><div class="sidebar-foot"><span class="connection-dot"></span><span>"Özel altyapınızda çalışıyor"</span></div></aside><div class="main-shell"><header class="topbar"><div><span class="eyebrow">"KONTROL PANELİ"</span><h1>{title}</h1></div><div class="profile"><a href="/panel/account">{user.email.clone()}</a><span class="role-pill">{role_label(&user.role)}</span><form method="post" action="/logout">{csrf(&user.csrf)}<button class="button ghost" type="submit">"Çıkış"</button></form></div></header><main>{content}</main><footer>"VpnHood Yönetici · Saatler İstanbul (UTC+03) · Kendi özel altyapınız."</footer></div></div>
	}.into_any())
}
fn role_label(role: &str) -> &'static str {
	match role {
		"admin" => "Yönetici",
		"operator" => "Operatör",
		_ => "Salt okunur",
	}
}
pub fn signin(token: &str, error: Option<&str>) -> Html<String> {
	document("Giriş",view!{<div class="login-shell"><section class="login-intro"><span class="brand-mark large">"VH"</span><span class="eyebrow">"VPNHOOD YÖNETİCİ"</span><h1>"Sunucularınızın kontrolü sizde."</h1><p>"Sunucuları izleyin, erişim anahtarlarını yönetin ve kullanımınızı tek yerden takip edin."</p><div class="intro-points"><span>"01 · Kiracınıza özel çalışma alanı"</span><span>"02 · Toplu erişim yönetimi"</span><span>"03 · Canlı sunucu durumu"</span></div></section><section class="login-card"><span class="eyebrow">"HOŞ GELDİNİZ"</span><h2>"Çalışma alanınıza giriş yapın"</h2><p class="muted">"Yöneticinizin oluşturduğu hesabı kullanın."</p>{error.map(|message|view!{<p class="alert error" role="alert">{message.to_owned()}</p>})}<form method="post" action="/signin" class="stack">{csrf(token)}<label for="email">"E-posta"</label><input id="email" type="email" name="email" autocomplete="username" required maxlength="254"/><label for="password">"Parola"</label><input id="password" type="password" name="password" autocomplete="current-password" required maxlength="128"/><button class="button primary" type="submit">"Giriş yap"<span>"→"</span></button></form><p class="small muted login-note">"Hesabınıza erişemiyorsanız çalışma alanı yöneticinizle iletişime geçin."</p></section></div>}.into_any())
}
pub fn error(message: &str) -> Html<String> {
	document("İşlem tamamlanamadı",view!{<main class="error-page"><span class="brand-mark">"VH"</span><h1>"İşlem tamamlanamadı"</h1><p>{message.to_owned()}</p><a class="button primary" href="/panel">"Panele dön"</a></main>}.into_any())
}
pub fn secret(user: &User, title: &str, description: &str, value: &str) -> Html<String> {
	layout(user,title,"",view!{<section class="card secret-card"><span class="eyebrow">"GÜVENLİ PAYLAŞIM"</span><h2>{title.to_owned()}</h2><p class="muted">{description.to_owned()}</p><textarea id="secret-output" class="secret-output" readonly rows="14" aria-label="Oluşturulan bağlantı bilgileri">{value.to_owned()}</textarea><div class="actions"><button class="button primary" type="button" data-copy="secret-output">"Kopyala"</button><a class="button secondary" href="/panel">"Panele dön"</a></div><p class="small muted" id="copy-feedback" aria-live="polite"></p></section>}.into_any())
}
fn stat(
	label: &'static str,
	name: &'static str,
	value: String,
	detail: &'static str,
) -> impl IntoView {
	view! {<div class="stat"><span class="eyebrow">{label}</span><strong data-stat=name>{value}</strong><span class="small muted">{detail}</span></div>}
}
pub fn panel(page: &str, user: &User, data: Data, notice: &str) -> Html<String> {
	let title = match page {
		"servers" => "Sunucular",
		"tokens" => "Erişim anahtarları",
		"sessions" => "VPN oturumları",
		"users" => "Kullanıcılar",
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
	layout(user,title,page,view!{<div class="page-content">{(notice=="saved").then(||view!{<div class="alert success" role="status">"Değişiklikler kaydedildi."</div>})}{content}</div>}.into_any())
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
	let cards = data
		.servers
		.into_iter()
		.take(6)
		.map(server_summary)
		.collect_view();
	view!{<section class="welcome"><div><span class="eyebrow">"ALTYAPINIZ BİR BAKIŞTA"</span><h2>"Her bağlantı kontrolünüz altında."</h2><p>"Sunucularınızın durumunu ve çalışma alanınızın kullanımını buradan izleyin."</p></div><a class="button primary" href="/panel/servers">{if user.can_write(){"Sunucuları yönet"}else{"Sunucuları görüntüle"}}<span>"↗"</span></a></section><section class="stats-grid" aria-label="Altyapı özeti">{counters}</section><div class="section-heading"><div><h2>"Sunucu durumu"</h2><p class="muted">"Sunucularınız kendi özel altyapınızda çalışır."</p></div><a class="text-link" href="/panel/servers">"Tümünü görüntüle →"</a></div><section class="server-grid">{cards}</section><div class="empty-note">"Henüz sunucunuz yoksa Sunucular sayfasından ekleyebilirsiniz."</div><section class="card info-band"><div><span class="eyebrow">"ERİŞİM YÖNETİMİ"</span><h3>"Anahtarları topluca oluşturun ve yönetin."</h3><p class="muted">"Cihaz sayısı, trafik kotası ve geçerlilik süresini ihtiyacınıza göre belirleyin."</p></div><a class="button secondary" href="/panel/tokens">"Erişim anahtarları →"</a></section>}.into_any()
}
fn server_summary(server: Server) -> impl IntoView {
	view! {<article class="card server-card"><div class="server-card-top"><span class="server-icon">"↗"</span><span class="badge" class:online=server.online>{server.status}</span></div><h3>{server.name}</h3><span class="small muted">{server.group}</span><dl class="metrics"><div><dt>"CPU"</dt><dd>{server.cpu}</dd></div><div><dt>"Bellek"</dt><dd>{server.memory}</dd></div><div><dt>"Oturum"</dt><dd>{server.sessions}</dd></div></dl><p class="small muted last-seen">{server.seen}</p></article>}
}
fn farm_select(farms: Vec<Farm>, selected: String) -> impl IntoView {
	view! {<select name="farm" aria-label="Sunucu grubu"><option value="">"Grupsuz"</option>{farms.into_iter().map(|f|view!{<option value=f.id.clone() selected={selected==f.id}>{f.name}</option>}).collect_view()}</select>}
}
fn servers(user: &User, data: Data) -> AnyView {
	let farms = data.farms.clone();
	let farm_options = data.farms.clone();
	let rows=data.servers.into_iter().map(|server|{
		let controls=user.can_write().then(||view!{<details class="edit-details"><summary>"Düzenle"</summary><form method="post" action=format!("/manage/servers/{}",server.id) class="stack compact-form">{csrf(&user.csrf)}<label>"Sunucu adı"<input name="name" value=server.name.clone() required maxlength="120"/></label>{farm_select(farms.clone(),server.farm.clone())}<label class="checkbox"><input type="checkbox" name="paused" value="true" checked=server.paused/>"Yeni oturumları durdur (bakım)"</label><button class="button secondary" type="submit">"Kaydet"</button></form><form method="post" action=format!("/manage/servers/{}/credential",server.id) class="stack"><p class="small muted">"Kimliği yenilerseniz yeni bağlantı ayarını sunucuya uygulamanız gerekir."</p>{csrf(&user.csrf)}<button class="button ghost" type="submit">"Bağlantı kimliğini yenile"</button></form></details>});
		view!{<tr><td><strong>{server.name}</strong><span class="table-sub">{server.seen}</span></td><td>{server.group}</td><td><span class="badge" class:online=server.online>{server.status}</span></td><td>{server.cpu}</td><td>{server.memory}</td><td>{server.sessions}</td><td>{controls}</td></tr>}
	}).collect_view();
	view!{<div class="section-heading"><div><h2>"Kendi sunucularınızı yönetin"</h2><p class="muted">"Her sunucu yalnızca çalışma alanınıza aittir."</p></div><form method="get" class="search"><input type="search" name="q" value=data.query placeholder="Sunucu ara" aria-label="Sunucu ara"/><button class="button ghost" type="submit">"Ara"</button></form></div><section class="card table-wrap"><table><thead><tr><th>"Sunucu"</th><th>"Grup"</th><th>"Durum"</th><th>"CPU"</th><th>"Bellek"</th><th>"Oturum"</th><th>"Yönetim"</th></tr></thead><tbody>{rows}</tbody></table></section>{user.can_write().then(||view!{<div class="form-grid"><section class="card"><span class="eyebrow">"YENİ SUNUCU"</span><h2>"Sunucu ekle"</h2><form method="post" action="/manage/servers" class="stack">{csrf(&user.csrf)}<label for="server-name">"Sunucu adı"</label><input id="server-name" name="name" required maxlength="120" placeholder="Örn. İstanbul 01"/><label for="server-host">"VPN adresi (DNS veya IP)"</label><input id="server-host" name="host" required maxlength="253" placeholder="vpn.ornek.com"/><label for="server-port">"TCP portu"</label><input id="server-port" type="number" name="port" value="443" min="1" max="65535" required/><label>"Sunucu grubu"{farm_select(farm_options,String::new())}</label><button class="button primary" type="submit">"Sunucuyu kaydet"</button><p class="small muted">"Kayıttan sonra sunucunuza ekleyeceğiniz bağlantı ayarları gösterilir."</p></form></section><section class="card"><span class="eyebrow">"DÜZENLEME"</span><h2>"Sunucu grubu oluştur"</h2><p class="muted">"Sunucularınızı bölge veya kullanım amacına göre gruplandırın."</p><form method="post" action="/manage/farms" class="stack">{csrf(&user.csrf)}<label for="farm-name">"Grup adı"</label><input id="farm-name" name="name" required maxlength="120" placeholder="Örn. Türkiye"/><button class="button secondary" type="submit">"Grup oluştur"</button></form></section></div>})}}.into_any()
}
fn tokens(user: &User, data: Data) -> AnyView {
	let paging = pagination(data.page, &data.query, data.more);
	let search = data.query.clone();
	let rows=data.tokens.into_iter().map(|token|{
		let controls=user.can_write().then(||view!{<form method="post" action="/manage/tokens/bulk">{csrf(&user.csrf)}<input type="hidden" name="ids" value=token.id.clone()/><input type="hidden" name="action" value=if token.revoked{"enable"}else{"revoke"}/><button class="button ghost" type="submit">{if token.revoked{"Etkinleştir"}else{"İptal et"}}</button></form>});
		view!{<tr><td>{user.can_write().then(||view!{<input type="checkbox" class="token-select" value=token.id aria-label=format!("{} anahtarını seç",token.name)/>})}</td><td><strong>{token.name}</strong><span class="table-sub">{token.server}</span></td><td>{token.traffic}</td><td>{token.quota}</td><td>{token.clients}</td><td>{token.expires}</td><td><span class="badge" class:online={token.state=="Etkin"}>{token.state.clone()}</span></td><td>{controls}</td></tr>}
	}).collect_view();
	let options = data
		.servers
		.into_iter()
		.map(|s| view! {<option value=s.id>{s.name}</option>})
		.collect_view();
	view!{<div class="section-heading"><div><h2>"Erişimi ihtiyacınıza göre sınırlandırın"</h2><p class="muted">"Anahtarları tek tek veya topluca yönetin."</p></div><div class="actions"><form method="get" class="search"><input type="search" name="q" value=search placeholder="Anahtar ara" aria-label="Anahtar ara"/><button class="button ghost" type="submit">"Ara"</button></form><a class="button secondary" href="/export/usage.csv">"Kullanım raporunu indir"</a></div></div><section class="card table-wrap"><table><thead><tr><th>{user.can_write().then(||view!{<input type="checkbox" id="select-all" aria-label="Tüm anahtarları seç"/>})}</th><th>"Anahtar / Sunucu"</th><th>"Kullanım"</th><th>"Kota"</th><th>"Cihaz"</th><th>"Son tarih"</th><th>"Durum"</th><th>"İşlem"</th></tr></thead><tbody>{rows}</tbody></table></section>{paging}{user.can_write().then(||view!{<section class="card bulk-bar"><form method="post" action="/manage/tokens/bulk" id="bulk-form" class="bulk-form">{csrf(&user.csrf)}<input type="hidden" id="selected-ids" name="ids"/><strong><span id="selected-count">"0"</span>" anahtar seçildi"</strong><select name="action" aria-label="Toplu işlem"><option value="revoke">"Seçilenleri iptal et"</option><option value="enable">"Seçilenleri etkinleştir"</option><option value="quota">"Seçilenlerin kotasını değiştir"</option><option value="expiry">"Seçilenlerin süresini değiştir"</option></select><input type="number" name="max_gib" value="0" min="0" max="1000000" aria-label="Yeni trafik kotası GiB"/><input type="number" name="max_clients" value="1" min="1" max="10000" aria-label="Yeni cihaz kotası"/><input name="expires" placeholder="Son tarih; boş = süresiz" aria-label="Yeni son kullanma zamanı"/><button class="button secondary" type="submit">"Uygula"</button></form></section><section class="card"><span class="eyebrow">"TEKLİ VE TOPLU ÜRETİM"</span><h2>"Erişim anahtarı oluştur"</h2><form method="post" action="/manage/tokens" class="form-fields">{csrf(&user.csrf)}<label>"Sunucu"<select name="server" required>{options}</select></label><label>"Anahtar adı / öneki"<input name="name" required maxlength="110" placeholder="Örn. Ekip"/></label><label>"Adet"<input type="number" name="count" value="1" min="1" max="100" required/></label><label>"Cihaz sınırı"<input type="number" name="max_clients" value="1" min="1" max="10000" required/></label><label>"Trafik kotası (GiB)"<input type="number" name="max_gib" value="0" min="0" max="1000000" required/><small>"0 = sınırsız"</small></label><label>"Son kullanma zamanı (isteğe bağlı)"<input name="expires" placeholder="2026-12-31T23:59:00+03:00"/></label><div class="form-submit"><button class="button primary" type="submit">"Anahtarları oluştur"</button></div></form></section>})}}.into_any()
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
	view!{<nav class="pagination" aria-label="Anahtar sayfaları"><span class="small muted">{format!("Sayfa {page} · En fazla 100 kayıt")}</span><div class="actions">{(page>1).then(||view!{<a class="button secondary" href=format!("/panel/tokens?p={}&q={query}",page-1)>"← Önceki"</a>})}{more.then(||view!{<a class="button secondary" href=format!("/panel/tokens?p={}&q={query}",page+1)>"Sonraki →"</a>})}</div></nav>}.into_any()
}
fn sessions(user: &User, data: Data) -> AnyView {
	let rows=data.sessions.into_iter().map(|session|view!{<tr><td><strong>{format!("#{}",session.id)}</strong></td><td>{session.token}</td><td>{session.server}</td><td>{session.created}</td><td><span class="badge" class:online={!session.closed}>{if session.closed{"Kapalı"}else{"Açık"}}</span></td><td>{(user.can_write()&&!session.closed).then(||view!{<form method="post" action=format!("/manage/sessions/{}/close",session.id)>{csrf(&user.csrf)}<button class="button ghost" type="submit">"Sonlandır"</button></form>})}</td></tr>}).collect_view();
	view!{<div class="section-heading"><div><h2>"Bağlantıları takip edin"</h2><p class="muted">"Son 200 VPN oturumu. Sonlandırma kararı sunucunun bir sonraki durum bildiriminde iletilir."</p></div></div><section class="card table-wrap"><table><thead><tr><th>"Oturum"</th><th>"Anahtar"</th><th>"Sunucu"</th><th>"Başlangıç"</th><th>"Durum"</th><th>"İşlem"</th></tr></thead><tbody>{rows}</tbody></table></section>}.into_any()
}
fn users(user: &User, data: Data) -> AnyView {
	let rows=data.users.into_iter().map(|account|view!{<tr><td><strong>{account.email}</strong></td><td><form method="post" action=format!("/manage/users/{}",account.id) class="inline-form">{csrf(&user.csrf)}<select name="role" aria-label="Kullanıcı rolü"><option value="admin" selected={account.role=="admin"}>"Yönetici"</option><option value="operator" selected={account.role=="operator"}>"Operatör"</option><option value="viewer" selected={account.role=="viewer"}>"Salt okunur"</option></select><label class="checkbox"><input type="checkbox" name="active" value="true" checked=account.active/>"Etkin"</label><button class="button secondary" type="submit">"Kaydet"</button></form></td></tr>}).collect_view();
	view!{<div class="section-heading"><div><h2>"Çalışma alanını birlikte yönetin"</h2><p class="muted">"Yönetici tüm işlemleri, operatör sunucu ve anahtar işlemlerini yapabilir. Salt okunur rol değişiklik yapamaz."</p></div></div><section class="card table-wrap"><table><thead><tr><th>"Kullanıcı"</th><th>"Rol ve erişim"</th></tr></thead><tbody>{rows}</tbody></table></section><section class="card"><h2>"Kullanıcı ekle"</h2><form method="post" action="/manage/users" class="form-fields">{csrf(&user.csrf)}<label>"E-posta"<input type="email" name="email" autocomplete="off" maxlength="254" required/></label><label>"Başlangıç parolası"<input type="password" name="password" autocomplete="new-password" minlength="16" maxlength="128" required/></label><label>"Rol"<select name="role"><option value="viewer">"Salt okunur"</option><option value="operator">"Operatör"</option><option value="admin">"Yönetici"</option></select></label><div class="form-submit"><button class="button primary" type="submit">"Kullanıcı ekle"</button></div></form></section>}.into_any()
}
fn audit(data: Data) -> AnyView {
	let rows=data.audit.into_iter().map(|event|view!{<tr><td>{event.time}</td><td>{event.email}</td><td><strong>{event.action}</strong></td><td class="resource">{event.resource}</td></tr>}).collect_view();
	view!{<div class="section-heading"><div><h2>"İşlem geçmişi"</h2><p class="muted">"Çalışma alanınızda yapılan son 200 işlem. Parolalar ve erişim sırları bu kayıtlara yazılmaz."</p></div></div><section class="card table-wrap"><table><thead><tr><th>"Zaman"</th><th>"Kullanıcı"</th><th>"İşlem"</th><th>"Kaynak"</th></tr></thead><tbody>{rows}</tbody></table></section>}.into_any()
}
fn account(user: &User) -> AnyView {
	view!{<div>{user.is_admin().then(||view!{<section class="card account-card"><h2>"Çalışma alanı"</h2><form method="post" action="/manage/tenant" class="stack">{csrf(&user.csrf)}<label>"Çalışma alanı adı"<input name="name" value=user.tenant_name.clone() maxlength="120" required/></label><button class="button secondary" type="submit">"Adı kaydet"</button></form></section>})}<section class="card account-card"><span class="eyebrow">"HESAP GÜVENLİĞİ"</span><h2>"Parolanızı değiştirin"</h2><p class="muted">"Parola değiştiğinde açık panel oturumlarınız kapatılır."</p><form method="post" action="/manage/password" class="stack">{csrf(&user.csrf)}<label>"Mevcut parola"<input type="password" name="current_password" autocomplete="current-password" maxlength="128" required/></label><label>"Yeni parola"<input type="password" name="new_password" autocomplete="new-password" minlength="16" maxlength="128" required/></label><button class="button primary" type="submit">"Parolayı değiştir"</button></form></section></div>}.into_any()
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
