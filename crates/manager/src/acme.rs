//! Unprivileged Certbot jobs; hooks use a leased tenant-bound record, never API credentials.
use crate::{certificates, store::Store};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};

use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};
use std::{path::Path, process::Stdio, time::Duration};
use uuid::Uuid;

pub fn validate(domain: &str, email: &str) -> Result<()> {
	ensure!(
		crate::management::normalize_host(domain)? == domain
			&& domain.contains('.')
			&& domain.parse::<std::net::IpAddr>().is_err(),
		"Alan adı geçersiz."
	);
	ensure!(
		email.len() <= 254
			&& email.split('@').count() == 2
			&& email.split('@').all(|p| !p.is_empty())
			&& email.bytes().all(|b| b.is_ascii_graphic())
			&& !email.starts_with('-'),
		"E-posta geçersiz."
	);
	Ok(())
}
async fn save(
	store: &Store,
	tx: &mut sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres>,
	tenant: Uuid,
	server: Uuid,
	config: &mut Value,
) -> Result<String> {
	let code = Uuid::new_v4().to_string();
	config["configCode"] = code.clone().into();
	query("UPDATE servers SET configuration=$3,config_code=$4 WHERE tenant_id=$1 AND id=$2")
		.bind(tenant)
		.bind(server)
		.bind(store.vault.seal(
			&serde_json::to_vec(config)?,
			&format!("{tenant}:{server}:config"),
		)?)
		.bind(&code)
		.execute(&mut **tx)
		.await?;
	Ok(code)
}
pub async fn applied(store: &Store, tenant: Uuid, server: Uuid, code: &str) -> Result<()> {
	for _ in 0..60 {
		let mut tx = store.begin(tenant).await?;
		let r = query("SELECT config_code,status FROM servers WHERE tenant_id=$1 AND id=$2")
			.bind(tenant)
			.bind(server)
			.fetch_one(&mut *tx)
			.await?;
		ensure!(
			r.get::<String, _>("config_code") == code,
			"Sunucu ayarı eşzamanlı değişti."
		);
		let status = r.get::<Option<Value>, _>("status").unwrap_or_default();
		if status["configCode"].as_str() == Some(code) {
			ensure!(
				status["configError"].is_null(),
				"Sunucu yapılandırmayı reddetti."
			);
			return Ok(());
		}
		drop(tx);
		tokio::time::sleep(Duration::from_secs(2)).await;
	}
	anyhow::bail!("Sunucu sürüm onayı zaman aşımı.")
}
pub async fn hook(store: &Store, tenant: Uuid, id: Uuid, cleanup: bool) -> Result<()> {
	let mut tx = store.begin(tenant).await?;
	let job = query(
		"SELECT * FROM acme_orders WHERE tenant_id=$1 AND id=$2 AND lease_until>now() FOR UPDATE",
	)
	.bind(tenant)
	.bind(id)
	.fetch_one(&mut *tx)
	.await?;
	let server: Uuid = job.get("server_id");
	let r = query(
		"SELECT configuration,config_code FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
	)
	.bind(tenant)
	.bind(server)
	.fetch_one(&mut *tx)
	.await?;
	let mut config: Value = serde_json::from_slice(&store.vault.open(
		&r.get::<Vec<u8>, _>("configuration"),
		&format!("{tenant}:{server}:config"),
	)?)?;
	let code = if cleanup {
		let Some(previous) = job.get::<Option<bool>, _>("challenge_previous") else {
			return Ok(());
		};
		let saved: Option<String> = job.get("challenge_code");
		ensure!(
			saved.as_deref() == Some(&r.get::<String, _>("config_code"))
				|| config["enableHttp01Challenge"].as_bool() == Some(previous),
			"HTTP-01 ayarı eşzamanlı değişti; temizliği denetleyin."
		);
		config["enableHttp01Challenge"] = previous.into();
		let code = save(store, &mut tx, tenant, server, &mut config).await?;
		query("DELETE FROM acme_challenges WHERE tenant_id=$1 AND server_id=$2 AND token=$3")
			.bind(tenant)
			.bind(server)
			.bind(job.get::<Option<String>, _>("challenge_token"))
			.execute(&mut *tx)
			.await?;
		query("UPDATE acme_orders SET challenge_previous=NULL,challenge_code=NULL,challenge_token=NULL WHERE tenant_id=$1 AND id=$2").bind(tenant).bind(id).execute(&mut *tx).await?;
		code
	} else {
		ensure!(
			std::env::var("CERTBOT_DOMAIN")? == job.get::<String, _>("domain"),
			"Doğrulama alan adı uyuşmuyor."
		);
		let token = std::env::var("CERTBOT_TOKEN")?;
		let proof = std::env::var("CERTBOT_VALIDATION")?;
		ensure!(
			(16..=256).contains(&token.len())
				&& token
					.bytes()
					.all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
				&& proof.starts_with(&format!("{token}."))
				&& proof.len() <= 1024
				&& proof
					.bytes()
					.all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b)),
			"HTTP-01 verisi geçersiz."
		);
		let previous = config["enableHttp01Challenge"].as_bool().unwrap_or(false);
		config["enableHttp01Challenge"] = true.into();
		let code = save(store, &mut tx, tenant, server, &mut config).await?;
		query("INSERT INTO acme_challenges(tenant_id,server_id,token,key_authorization,expires_at) VALUES($1,$2,$3,$4,now()+interval '15 minutes') ON CONFLICT(tenant_id,server_id,token) DO UPDATE SET key_authorization=$4,expires_at=now()+interval '15 minutes'").bind(tenant).bind(server).bind(&token).bind(proof).execute(&mut *tx).await?;
		query("UPDATE acme_orders SET challenge_previous=$3,challenge_code=$4,challenge_token=$5 WHERE tenant_id=$1 AND id=$2").bind(tenant).bind(id).bind(previous).bind(&code).bind(token).execute(&mut *tx).await?;
		code
	};
	tx.commit().await?;
	applied(store, tenant, server, &code).await
}
async fn run(store: &Store, tenant: Uuid, id: Uuid, key: &Path, db: &str) -> Result<()> {
	// Recover the interrupted hook before attempting another authorization.
	hook(store, tenant, id, true).await?;
	let mut tx = store.begin(tenant).await?;
	let job = query(
		"SELECT server_id,domain,email,staging FROM acme_orders WHERE tenant_id=$1 AND id=$2",
	)
	.bind(tenant)
	.bind(id)
	.fetch_one(&mut *tx)
	.await?;
	drop(tx);
	let server: Uuid = job.get("server_id");
	let domain: String = job.get("domain");
	let email: String = job.get("email");
	let staging: bool = job.get("staging");
	validate(&domain, &email)?;
	let hash = Sha256::digest(format!("{domain}:{email}:{staging}").as_bytes());
	let dir = key
		.parent()
		.unwrap()
		.join("acme")
		.join(tenant.to_string())
		.join(id.to_string())
		.join(format!("{hash:x}"));
	tokio::fs::create_dir_all(&dir).await?;
	use std::os::unix::fs::PermissionsExt;
	std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
	let binary = std::env::current_exe()?.to_string_lossy().into_owned();
	ensure!(
		binary
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b)),
		"Kanca ikili yolu desteklenmiyor."
	);
	let hook_command = format!("{binary} acme-hook --tenant {tenant} --job {id}");
	let mut process = tokio::process::Command::new("/usr/bin/certbot");
	process
		.env_clear()
		.env("PATH", "/usr/bin:/bin")
		.env("LANG", "C.UTF-8")
		.env("HOME", &dir)
		.env("DATABASE_URL", db)
		.env("VHM_MASTER_KEY_FILE", key)
		.args([
			"certonly",
			"--non-interactive",
			"--agree-tos",
			"--manual",
			"--preferred-challenges",
			"http",
			"--manual-auth-hook",
			&hook_command,
			"--manual-cleanup-hook",
			&format!("{hook_command} --cleanup"),
			"--config-dir",
		])
		.arg(dir.join("config"))
		.arg("--work-dir")
		.arg(dir.join("work"))
		.arg("--logs-dir")
		.arg(dir.join("logs"))
		.args([
			"--cert-name",
			"hoodmin",
			"--domain",
			&domain,
			"--email",
			&email,
			"--key-type",
			"rsa",
			"--rsa-key-size",
			"3072",
			"--keep-until-expiring",
			"--server",
			if staging {
				"https://acme-staging-v02.api.letsencrypt.org/directory"
			} else {
				"https://acme-v02.api.letsencrypt.org/directory"
			},
		])
		.stdin(Stdio::null())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.kill_on_drop(true);
	use std::os::unix::process::CommandExt;
	process.as_std_mut().process_group(0);
	let mut child = process.spawn()?;
	let pid = child
		.id()
		.ok_or_else(|| anyhow::anyhow!("Certbot başlatılamadı."))?;
	let result = tokio::time::timeout(Duration::from_secs(720), child.wait()).await;
	if result.is_err() {
		let _ = tokio::process::Command::new("/bin/kill")
			.args(["-KILL", "--", &format!("-{pid}")])
			.status()
			.await;
		let _ = child.wait().await;
	}
	let cleaned = hook(store, tenant, id, true).await;
	let status = result.map_err(|_| anyhow::anyhow!("Certbot zaman aşımı."))??;
	cleaned?;
	ensure!(
		status.success(),
		"Certbot sertifika alamadı; DNS/TCP80 ve özel Certbot günlüğünü denetleyin."
	);
	let lineage = dir.join("config/live/hoodmin");
	let cert = openssl::x509::X509::from_pem(&std::fs::read(lineage.join("cert.pem"))?)?;
	let private =
		openssl::pkey::PKey::private_key_from_pem(&std::fs::read(lineage.join("privkey.pem"))?)?;
	let public = cert.public_key()?;
	ensure!(private.public_eq(&public), "Sertifika anahtarı uyuşmuyor.");
	let mut ca = openssl::stack::Stack::new()?;
	for c in openssl::x509::X509::stack_from_pem(&std::fs::read(lineage.join("chain.pem"))?)? {
		ca.push(c)?;
	}
	if !staging {
		let mut trust = openssl::x509::store::X509StoreBuilder::new()?;
		trust.set_default_paths()?;
		let mut context = openssl::x509::X509StoreContext::new()?;
		ensure!(
			context.init(&trust.build(), &cert, &ca, |c| c.verify_cert())?,
			"CA zinciri doğrulanamadı."
		);
	}
	let mut builder = openssl::pkcs12::Pkcs12::builder();
	builder.name(&domain).pkey(&private).cert(&cert).ca(ca);
	let (certificate, hash) =
		certificates::import_pfx(&STANDARD.encode(builder.build2("")?.to_der()?), "", &domain)?;
	ensure!(
		certificates::dns_name(&certificate).as_deref() == Some(&domain),
		"Sertifika alan adı uyuşmuyor."
	);
	let expiration = chrono::NaiveDateTime::parse_from_str(
		&cert.not_after().to_string(),
		"%b %e %H:%M:%S %Y GMT",
	)?
	.and_utc();
	if !staging {
		let mut tx = store.begin(tenant).await?;
		let r = query(
			"SELECT configuration,config_code FROM servers WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
		)
		.bind(tenant)
		.bind(server)
		.fetch_one(&mut *tx)
		.await?;
		let mut config: Value = serde_json::from_slice(&store.vault.open(
			&r.get::<Vec<u8>, _>("configuration"),
			&format!("{tenant}:{server}:config"),
		)?)?;
		let code = if config["publicServerToken"]["ch"].as_str() == Some(&hash) {
			r.get::<String, _>("config_code")
		} else {
			certificates::attach(&mut config, certificate, &hash, true)?;
			save(store, &mut tx, tenant, server, &mut config).await?
		};
		tx.commit().await?;
		applied(store, tenant, server, &code).await?;
	}
	let mut tx = store.begin(tenant).await?;
	query("UPDATE acme_orders SET expires_at=$3,last_success=now() WHERE tenant_id=$1 AND id=$2")
		.bind(tenant)
		.bind(id)
		.bind(expiration)
		.execute(&mut *tx)
		.await?;
	tx.commit().await?;
	Ok(())
}
pub async fn worker(store: Store, db: String, key: std::path::PathBuf) {
	let mut tick = tokio::time::interval(Duration::from_secs(5));
	loop {
		tick.tick().await;
		let Ok(rows) = query("SELECT tenant_id,id FROM hoodmin_due_acme()")
			.fetch_all(&store.pool)
			.await
		else {
			continue;
		};
		for row in rows {
			if execute(&store, row.get("tenant_id"), row.get("id"), &key, &db)
				.await
				.is_err()
			{
				tracing::warn!("ACME görevinin veritabanı işlemi tamamlanamadı.");
			}
		}
	}
}
async fn execute(store: &Store, tenant: Uuid, id: Uuid, key: &Path, db: &str) -> Result<()> {
	let mut tx = store.begin(tenant).await?;
	let row=query("UPDATE acme_orders SET status='Running',message='',last_attempt=now(),lease_until=now()+interval '20 minutes' WHERE tenant_id=$1 AND id=$2 AND enabled AND next_due<=now() AND (lease_until IS NULL OR lease_until<now()) RETURNING staging").bind(tenant).bind(id).fetch_optional(&mut *tx).await?;
	let Some(row) = row else { return Ok(()) };
	let staging: bool = row.get("staging");
	tx.commit().await?;
	let (status, message) = match run(store, tenant, id, key, db).await {
		Ok(()) => (
			if staging { "Validated" } else { "Succeeded" },
			"Tamamlandı.".to_owned(),
		),
		Err(e) => (
			"Failed",
			e.to_string().chars().take(200).collect::<String>(),
		),
	};
	let mut tx = store.begin(tenant).await?;
	query("UPDATE acme_orders SET status=$3,message=$4,lease_until=NULL,next_due=now()+interval '12 hours' WHERE tenant_id=$1 AND id=$2").bind(tenant).bind(id).bind(status).bind(message).execute(&mut *tx).await?;
	tx.commit().await?;
	Ok(())
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn acme_domain_and_contact_are_bounded() {
		assert!(validate("vpn.example.com", "vpn@example.com").is_ok());
		for d in [
			"127.0.0.1",
			"*.example.com",
			"localhost",
			"example.com/arg",
			"-bad.example.com",
			"EXAMPLE.COM",
		] {
			assert!(validate(d, "a@b.com").is_err());
		}
		assert!(validate("vpn.example.com", "a\n@b.com").is_err());
	}
}
