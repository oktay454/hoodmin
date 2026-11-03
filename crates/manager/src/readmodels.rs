//! Tenant-scoped JSON read models shared by the management API.
use crate::{auth::User, store::Store};
use anyhow::Result;
use axum::Json;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx_core::{query::query, query_scalar::query_scalar, row::Row};
use uuid::Uuid;

#[derive(Clone, serde::Serialize)]
pub struct Farm {
	pub id: String,
	pub name: String,
}
#[derive(serde::Serialize)]
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
#[derive(serde::Serialize)]
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
#[derive(serde::Serialize)]
pub struct Session {
	pub id: i64,
	pub token: String,
	pub server: String,
	pub created: String,
	pub closed: bool,
}
#[derive(serde::Serialize)]
pub struct Account {
	pub id: String,
	pub email: String,
	pub role: String,
	pub active: bool,
}
#[derive(serde::Serialize)]
pub struct Audit {
	pub time: String,
	pub email: String,
	pub action: String,
	pub resource: String,
}
#[derive(serde::Serialize)]
pub struct Profile {
	pub id: Uuid,
	pub name: String,
	pub is_public: bool,
	pub policy: Value,
}
#[derive(serde::Serialize)]
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
#[derive(serde::Serialize)]
pub struct InstallJob {
	pub id: Uuid,
	pub status: String,
	pub log: String,
	pub time: String,
}
#[derive(serde::Serialize)]
pub struct ServerProfile {
	pub assigned_farms: i64,
	pub id: Uuid,
	pub name: String,
	pub settings: Value,
	pub management: Value,
	pub version: Uuid,
}
#[derive(serde::Serialize)]
pub struct Bot {
	pub id: Uuid,
	pub name: String,
	pub role: String,
	pub expires: String,
	pub revoked: bool,
}

pub fn error(message: &str) -> Json<Value> {
	Json(json!({"error":message}))
}
pub fn secret(_user: &User, title: &str, description: &str, value: &str) -> Json<Value> {
	Json(json!({"title":title,"description":description,"value":value}))
}
pub fn panel(page: &str, _user: &User, data: Data, notice: &str) -> Json<Value> {
	Json(json!({"page":page,"data":data,"notice":notice}))
}
pub fn confirm_credential(_user: &User, id: Uuid, name: &str, proof: &str) -> Json<Value> {
	Json(json!({"id":id,"name":name,"proof":proof}))
}
pub fn configuration(
	_user: &User,
	id: Uuid,
	name: &str,
	code: &str,
	settings: Value,
	status: Option<Value>,
	error: Option<String>,
) -> Json<Value> {
	Json(json!({"id":id,"name":name,"code":code,"settings":settings,"status":status,"error":error}))
}
pub fn profiles(_user: &User, profiles: Vec<Profile>) -> Json<Value> {
	Json(json!({"profiles":profiles}))
}
pub fn token_policy(
	_user: &User,
	id: Uuid,
	name: &str,
	policy: Value,
	details: Value,
) -> Json<Value> {
	Json(json!({"id":id,"name":name,"policy":policy,"details":details}))
}
pub fn certificates(
	_user: &User,
	id: Uuid,
	name: &str,
	code: &str,
	certificates: Vec<(String, String, String)>,
) -> Json<Value> {
	Json(json!({"id":id,"name":name,"code":code,"certificates":certificates}))
}
pub fn routing(
	_user: &User,
	id: Uuid,
	name: &str,
	code: &str,
	policy: Value,
	advertisement: Value,
) -> Json<Value> {
	Json(json!({"id":id,"name":name,"code":code,"policy":policy,"advertisement":advertisement}))
}
pub fn installation(_user: &User, id: Uuid, settings: &str) -> Json<Value> {
	Json(json!({"id":id,"settings":settings}))
}
pub fn ssh_installation(_user: &User, id: Uuid, name: &str, jobs: Vec<InstallJob>) -> Json<Value> {
	Json(json!({"id":id,"name":name,"jobs":jobs}))
}
pub fn server_profiles(
	_user: &User,
	profiles: Vec<ServerProfile>,
	servers: Vec<(Uuid, String)>,
) -> Json<Value> {
	Json(json!({"profiles":profiles,"servers":servers}))
}
pub fn reports(
	_user: &User,
	servers: Vec<(Uuid, String)>,
	server: Option<Uuid>,
	hours: i32,
	charts: Vec<crate::reports::Chart>,
	countries: Vec<(String, i64)>,
	range: String,
) -> Json<Value> {
	Json(
		json!({"servers":servers,"server":server,"hours":hours,"charts":charts,"countries":countries,"range":range}),
	)
}
pub fn bots(_user: &User, bots: Vec<Bot>) -> Json<Value> {
	Json(json!({"bots":bots}))
}
pub fn farm_publication(
	_user: &User,
	id: Uuid,
	name: &str,
	enabled: bool,
	version: Uuid,
) -> Json<Value> {
	Json(json!({"id":id,"name":name,"enabled":enabled,"version":version}))
}
pub fn farm_certificates(
	_user: &User,
	id: Uuid,
	name: &str,
	version: Uuid,
	certificates: Vec<(String, String, String)>,
) -> Json<Value> {
	Json(json!({"id":id,"name":name,"version":version,"certificates":certificates}))
}
pub fn destinations(
	_user: &User,
	farm: Uuid,
	name: &str,
	items: Vec<crate::publication::Destination>,
	servers: Vec<(Uuid, String)>,
) -> Json<Value> {
	Json(json!({"farm":farm,"name":name,"items":items,"servers":servers}))
}
pub fn destination_edit(
	_user: &User,
	item: crate::publication::Destination,
	servers: Vec<(Uuid, String)>,
) -> Json<Value> {
	Json(json!({"item":item,"servers":servers}))
}
pub fn farm_server_profile(
	_user: &User,
	id: Uuid,
	name: &str,
	version: Uuid,
	selected: Option<Uuid>,
	members: i64,
	profiles: Vec<(Uuid, String, Uuid)>,
) -> Json<Value> {
	Json(
		json!({"id":id,"name":name,"version":version,"selected":selected,"members":members,"profiles":profiles}),
	)
}
pub fn autosign(_user: &User, server: Uuid, name: &str, order: Option<Value>) -> Json<Value> {
	Json(json!({"server":server,"name":name,"order":order}))
}
