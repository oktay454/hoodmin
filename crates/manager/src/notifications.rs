//! Tenant-isolated notifications. No VPN keys or diagnostic logs enter event payloads.
use crate::{auth::User, store::Store};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx_core::{query::query, query_scalar::query_scalar, row::Row};
use uuid::Uuid;
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
	Webhook {
		url: String,
		#[serde(default)]
		bearer: String,
	},
	Smtp {
		host: String,
		port: u16,
		username: String,
		password: String,
		from: String,
		to: String,
	},
}
impl Target {
	pub fn validate(&self) -> Result<()> {
		match self {
			Self::Webhook { url, bearer } => {
				crate::publication::validate_url(url)?;
				ensure!(
					bearer.len() <= 4096 && !bearer.chars().any(char::is_control),
					"Bildirim kimliği geçersiz."
				);
			}
			Self::Smtp {
				host,
				port,
				username,
				password,
				from,
				to,
			} => {
				ensure!(
					crate::management::normalize_host(host)? == *host && *port > 0,
					"SMTP hedefi geçersiz."
				);
				for address in [from, to] {
					crate::auth::email(address)?;
					ensure!(
						address
							.bytes()
							.all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b)),
						"SMTP adresi geçersiz."
					);
				}
				ensure!(
					username.len() <= 254
						&& password.len() <= 4096
						&& !username.contains(':')
						&& !username
							.chars()
							.chain(password.chars())
							.any(char::is_control),
					"SMTP kimliği geçersiz."
				);
			}
		}
		Ok(())
	}
	fn summary(&self) -> Value {
		match self {
			Self::Webhook { url, .. } => {
				let u = url::Url::parse(url).ok();
				json!({"kind":"webhook","host":u.and_then(|u|u.host_str().map(str::to_owned))})
			}
			Self::Smtp {
				host,
				port,
				from,
				to,
				..
			} => json!({"kind":"smtp","host":host,"port":port,"from":from,"to":to}),
		}
	}
	pub async fn send(&self, event: &Value) -> Result<()> {
		self.validate()?;
		match self {
			Self::Webhook { url, bearer } => {
				crate::publication::notify_webhook(url, bearer, &serde_json::to_vec(event)?)
					.await?;
			}
			Self::Smtp {
				host,
				port,
				username,
				password,
				from,
				to,
			} => {
				crate::publication::notify_smtp(host, *port, username, password, from, to, event)
					.await?;
			}
		}
		Ok(())
	}
}
#[derive(Debug)]
pub struct Conflict;
impl std::fmt::Display for Conflict {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("Bildirim hedefi değişti; listeyi yenileyin.")
	}
}
impl std::error::Error for Conflict {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
	pub name: String,
	pub enabled: bool,
	pub version: Option<Uuid>,
	pub target: Option<Target>,
	#[serde(default)]
	pub confirmed: bool,
	#[serde(default)]
	pub csrf: String,
}
pub async fn list(store: &Store, user: &User) -> Result<Value> {
	let mut tx = store.begin(user.tenant).await?;
	let mut channels = Vec::new();
	for row in query("SELECT id,name,enabled,version,specification FROM notification_channels WHERE tenant_id=$1 ORDER BY name").bind(user.tenant).fetch_all(&mut *tx).await?{
		let id:Uuid=row.get("id");let spec:Target=serde_json::from_slice(&store.vault.open(&row.get::<Vec<u8>,_>("specification"),&format!("{}:{id}:notification",user.tenant))?)?;
		channels.push(json!({"id":id,"name":row.get::<String,_>("name"),"enabled":row.get::<bool,_>("enabled"),"version":row.get::<Uuid,_>("version"),"target":spec.summary()}));
	}
	let deliveries=query("SELECT id,channel_id,status,attempts,message,event,created_at FROM notification_deliveries WHERE tenant_id=$1 ORDER BY created_at DESC LIMIT 100").bind(user.tenant).fetch_all(&mut *tx).await?.iter().map(|r|json!({"id":r.get::<Uuid,_>("id"),"channel_id":r.get::<Uuid,_>("channel_id"),"status":r.get::<String,_>("status"),"attempts":r.get::<i32,_>("attempts"),"message":r.get::<String,_>("message"),"event":r.get::<Value,_>("event"),"created_at":r.get::<chrono::DateTime<chrono::Utc>,_>("created_at")})).collect::<Vec<_>>();
	Ok(json!({"channels":channels,"deliveries":deliveries}))
}
pub async fn save(store: &Store, user: &User, id: Uuid, form: Change) -> Result<Value> {
	ensure!(
		form.confirmed,
		"Bildirim hedefindeki değişikliği onaylayın."
	);
	ensure!(
		(1..=120).contains(&form.name.trim().len()),
		"Bildirim adı geçersiz."
	);
	let mut tx = store.begin(user.tenant).await?;
	query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
		.bind(format!("{}:notifications", user.tenant))
		.execute(&mut *tx)
		.await?;
	let existing = query(
		"SELECT version,specification FROM notification_channels WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
	)
	.bind(user.tenant)
	.bind(id)
	.fetch_optional(&mut *tx)
	.await?;
	if existing.as_ref().map(|r| r.get::<Uuid, _>("version")) != form.version {
		return Err(Conflict.into());
	}
	let encrypted = if let Some(target) = form.target {
		target.validate()?;
		store.vault.seal(
			&serde_json::to_vec(&target)?,
			&format!("{}:{id}:notification", user.tenant),
		)?
	} else {
		existing
			.as_ref()
			.ok_or_else(|| anyhow::anyhow!("Yeni bildirim hedefi gerekli."))?
			.get("specification")
	};
	let count: i64 = query_scalar("SELECT count(*) FROM notification_channels WHERE tenant_id=$1")
		.bind(user.tenant)
		.fetch_one(&mut *tx)
		.await?;
	ensure!(
		existing.is_some() || count < 8,
		"En fazla sekiz bildirim kanalı eklenebilir."
	);
	query("INSERT INTO notification_channels(tenant_id,id,name,enabled,version,specification) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(tenant_id,id) DO UPDATE SET name=$3,enabled=$4,version=$5,specification=$6").bind(user.tenant).bind(id).bind(form.name.trim()).bind(form.enabled).bind(Uuid::new_v4()).bind(encrypted).execute(&mut *tx).await?;
	if !form.enabled {
		query("UPDATE notification_deliveries SET status='Cancelled',lease_until=NULL WHERE tenant_id=$1 AND channel_id=$2 AND status IN ('Queued','Retry')").bind(user.tenant).bind(id).execute(&mut *tx).await?;
	}
	crate::auth::audit(&mut tx, user, "Bildirim kanalı kaydedildi", &id.to_string()).await?;
	tx.commit().await?;
	Ok(json!({"id":id}))
}
pub async fn test(store: &Store, user: &User, id: Uuid) -> Result<Value> {
	let mut tx = store.begin(user.tenant).await?;
	let active: bool = query_scalar(
		"SELECT EXISTS(SELECT FROM notification_channels WHERE tenant_id=$1 AND id=$2 AND enabled)",
	)
	.bind(user.tenant)
	.bind(id)
	.fetch_one(&mut *tx)
	.await?;
	ensure!(active, "Etkin bildirim kanalı bulunamadı.");
	let delivery = Uuid::new_v4();
	let event = json!({"id":delivery,"type":"test","resource":id,"state":"test","at":chrono::Utc::now(),"tenant":user.tenant});
	query("INSERT INTO notification_deliveries(tenant_id,id,channel_id,event) VALUES($1,$2,$3,$4)")
		.bind(user.tenant)
		.bind(delivery)
		.bind(id)
		.bind(event)
		.execute(&mut *tx)
		.await?;
	crate::auth::audit(
		&mut tx,
		user,
		"Bildirim sınaması sıraya alındı",
		&id.to_string(),
	)
	.await?;
	tx.commit().await?;
	Ok(json!({"id":delivery,"status":"Queued"}))
}
async fn observe(store: &Store, tenant: Uuid, settings: &crate::config::Settings) -> Result<()> {
	let mut tx = store.begin(tenant).await?;
	query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
		.bind(format!("{tenant}:notifications"))
		.execute(&mut *tx)
		.await?;
	let snapshots=query("SELECT 'server/'||id::text AS resource,CASE WHEN last_seen>now()-make_interval(secs=>heartbeat_seconds) THEN 'healthy' ELSE 'offline' END AS state FROM servers WHERE tenant_id=$1 AND NOT paused UNION ALL SELECT 'autosign/'||server_id::text,CASE WHEN status='Failed' THEN 'failed' ELSE 'healthy' END FROM acme_orders WHERE tenant_id=$1 AND enabled AND status IN ('Failed','Validated','Succeeded') UNION ALL SELECT 'publication/'||id::text,CASE WHEN status='Failed' THEN 'failed' ELSE 'healthy' END FROM external_publications WHERE tenant_id=$1 AND enabled AND status IN ('Failed','Succeeded')").bind(tenant).fetch_all(&mut *tx).await?;
	let mut snapshots = snapshots
		.iter()
		.map(|r| (r.get::<String, _>("resource"), r.get::<String, _>("state")))
		.collect::<Vec<_>>();
	if settings.backup.notify_tenant == Some(tenant) {
		let path = settings
			.master_key_file
			.with_file_name("backup-status.json");
		if let Ok(bytes) = std::fs::read(path)
			&& bytes.len() <= 4096
			&& let Ok(status) = serde_json::from_slice::<Value>(&bytes)
		{
			if let Some(success) = status["success"].as_bool() {
				snapshots.push((
					"backup".into(),
					if success { "healthy" } else { "failed" }.into(),
				));
			}
			if let Some(pressure) = status["pressure"].as_bool() {
				snapshots.push((
					"backup_disk".into(),
					if pressure { "pressure" } else { "healthy" }.into(),
				));
			}
		}
	}
	for (resource, state) in snapshots {
		let previous: Option<String> = query_scalar(
			"SELECT state FROM notification_states WHERE tenant_id=$1 AND resource=$2",
		)
		.bind(tenant)
		.bind(&resource)
		.fetch_optional(&mut *tx)
		.await?;
		if previous.as_deref() == Some(&state) {
			continue;
		}
		query("INSERT INTO notification_states(tenant_id,resource,state) VALUES($1,$2,$3) ON CONFLICT(tenant_id,resource) DO UPDATE SET state=$3").bind(tenant).bind(&resource).bind(&state).execute(&mut *tx).await?;
		if previous.is_none() && state == "healthy" {
			continue;
		}
		let event = json!({"id":Uuid::new_v4(),"type":"state_changed","resource":resource,"state":state,"at":chrono::Utc::now(),"tenant":tenant});
		query("INSERT INTO notification_deliveries(tenant_id,id,channel_id,event) SELECT tenant_id,gen_random_uuid(),id,$2 FROM notification_channels WHERE tenant_id=$1 AND enabled").bind(tenant).bind(event).execute(&mut *tx).await?;
	}
	// Bound storage while retaining failed delivery diagnostics for thirty days.
	query("DELETE FROM notification_deliveries WHERE tenant_id=$1 AND created_at<now()-interval '30 days' AND status NOT IN ('Queued','Retry')").bind(tenant).execute(&mut *tx).await?;
	tx.commit().await?;
	Ok(())
}
async fn deliver(store: &Store, tenant: Uuid) -> Result<()> {
	let mut tx = store.begin(tenant).await?;
	let row=query("SELECT d.id,d.channel_id,d.event,c.specification FROM notification_deliveries d JOIN notification_channels c ON c.tenant_id=d.tenant_id AND c.id=d.channel_id WHERE d.tenant_id=$1 AND c.enabled AND d.status IN ('Queued','Retry') AND d.next_due<=now() AND (d.lease_until IS NULL OR d.lease_until<now()) ORDER BY d.next_due LIMIT 1 FOR UPDATE OF d SKIP LOCKED").bind(tenant).fetch_optional(&mut *tx).await?;
	let Some(row) = row else { return Ok(()) };
	let id: Uuid = row.get("id");
	let channel: Uuid = row.get("channel_id");
	let event: Value = row.get("event");
	query("UPDATE notification_deliveries SET lease_until=now()+interval '2 minutes',attempts=attempts+1 WHERE tenant_id=$1 AND id=$2").bind(tenant).bind(id).execute(&mut *tx).await?;
	tx.commit().await?;
	let result = async {
		let target: Target = serde_json::from_slice(&store.vault.open(
			&row.get::<Vec<u8>, _>("specification"),
			&format!("{tenant}:{channel}:notification"),
		)?)?;
		target.send(&event).await
	}
	.await;
	let mut tx = store.begin(tenant).await?;
	// Never log target URLs, credentials, response bodies or SMTP diagnostics.
	let message = if result.is_ok() {
		"Delivered"
	} else {
		"TLS/DNS/transport or remote response failed"
	};
	query("UPDATE notification_deliveries SET status=CASE WHEN $3 THEN 'Delivered' WHEN attempts>=8 THEN 'Failed' ELSE 'Retry' END,message=$4,lease_until=NULL,next_due=now()+make_interval(secs=>LEAST(3600,30*power(2,LEAST(attempts,7))::integer)) WHERE tenant_id=$1 AND id=$2").bind(tenant).bind(id).bind(result.is_ok()).bind(message).execute(&mut *tx).await?;
	tx.commit().await?;
	if result.is_err() {
		tracing::warn!(%tenant,%id,"Bildirim gönderilemedi; yeniden deneme politikası uygulanıyor");
	}
	Ok(())
}
pub async fn worker(store: Store, settings: crate::config::Settings) {
	loop {
		let result = async {
			let tenants: Vec<Uuid> = query_scalar("SELECT id FROM hoodmin_vault_tenants()")
				.fetch_all(&store.pool)
				.await?;
			for tenant in tenants {
				observe(&store, tenant, &settings).await?;
				for _ in 0..8 {
					deliver(&store, tenant).await?;
				}
			}
			anyhow::Ok(())
		}
		.await;
		if let Err(error) = result {
			tracing::error!(%error,"Bildirim işleyicisi hazır değil");
		}
		tokio::time::sleep(std::time::Duration::from_secs(10)).await;
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn targets_reject_private_networks_and_header_injection() {
		assert!(
			Target::Webhook {
				url: "https://127.0.0.1/".into(),
				bearer: String::new()
			}
			.validate()
			.is_err()
		);
		assert!(
			Target::Webhook {
				url: "https://example.com/".into(),
				bearer: "x\r\ny".into()
			}
			.validate()
			.is_err()
		);
		assert!(
			Target::Smtp {
				host: "mail.example.com".into(),
				port: 465,
				username: "u".into(),
				password: "p".into(),
				from: "a@b\r\nBcc:c@d".into(),
				to: "c@d".into()
			}
			.validate()
			.is_err()
		);
	}
}
