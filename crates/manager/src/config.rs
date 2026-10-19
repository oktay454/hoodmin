use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sqlx_core::row::Row;
use sqlx_postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgSslMode};
use std::{
	net::SocketAddr,
	path::{Path, PathBuf},
	str::FromStr,
	time::Duration,
};

pub const DEFAULT_FILE: &str = "/etc/hoodmin/hoodmin.json";
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
	pub database_url: Option<String>,
	pub master_key_file: PathBuf,
	pub public_origin: String,
	pub listen: SocketAddr,
}
impl Default for Settings {
	fn default() -> Self {
		Self {
			database_url: None,
			master_key_file: "/var/lib/hoodmin/master.key".into(),
			public_origin: "https://hoodmin.example.invalid".into(),
			listen: "127.0.0.1:3080".parse().expect("loopback"),
		}
	}
}
impl Settings {
	pub fn load(path: &Path) -> Result<Self> {
		use std::os::unix::fs::PermissionsExt;
		let metadata = std::fs::metadata(path).context("Yapılandırma dosyası okunamadı.")?;
		ensure!(
			metadata.permissions().mode() & 0o027 == 0,
			"Yapılandırma dosyası yalnız sahibince yazılabilir ve herkese kapalı olmalı (0640)."
		);
		serde_json::from_slice(&std::fs::read(path)?)
			.map_err(|_| anyhow::anyhow!("Yapılandırma JSON biçimi geçersiz."))
	}
	pub fn validate(&self) -> Result<()> {
		ensure!(
			self.listen.ip().is_loopback(),
			"Hizmet yalnız loopback üzerinde dinleyebilir."
		);
		validate_origin(&self.public_origin)?;
		if let Some(url) = &self.database_url {
			database_options(url)?;
		}
		Ok(())
	}
	pub fn db(&self) -> Result<&str> {
		self.database_url
			.as_deref()
			.filter(|s| !s.is_empty())
			.context("Veritabanı yapılandırılmadı. hoodmin-cli config set kullanın.")
	}
	pub fn vault(&self) -> Result<crate::crypto::Vault> {
		use std::os::unix::fs::PermissionsExt;
		let metadata =
			std::fs::metadata(&self.master_key_file).context("Ana anahtar dosyası okunamadı.")?;
		ensure!(
			metadata.permissions().mode() & 0o077 == 0,
			"Ana anahtar dosyası 0600 olmalı."
		);
		crate::crypto::Vault::new(&std::fs::read(&self.master_key_file)?)
	}
}
pub fn validate_origin(value: &str) -> Result<()> {
	let url = url::Url::parse(value).map_err(|_| anyhow::anyhow!("HTTPS adresi geçersiz."))?;
	ensure!(
		url.scheme() == "https"
			&& url.host_str().is_some()
			&& url.username().is_empty()
			&& url.password().is_none()
			&& url.path() == "/"
			&& url.query().is_none()
			&& url.fragment().is_none()
			&& !value.ends_with('/'),
		"Yalnız HTTPS kök adresi kullanılabilir; yol veya sonda / eklemeyin."
	);
	Ok(())
}
pub fn database_options(value: &str) -> Result<PgConnectOptions> {
	ensure!(
		value.starts_with("postgresql:") || value.starts_with("postgres:"),
		"PostgreSQL bağlantı dizesi gerekli."
	);
	let options = PgConnectOptions::from_str(value)
		.map_err(|_| anyhow::anyhow!("PostgreSQL bağlantı dizesi geçersiz."))?;
	ensure!(
		options.get_database().is_some_and(|s| !s.is_empty()),
		"Veritabanı adı açıkça belirtilmeli."
	);
	ensure!(
		options.get_socket().is_some() || matches!(options.get_ssl_mode(), PgSslMode::VerifyFull),
		"TCP veritabanı bağlantısında sslmode=verify-full zorunludur."
	);
	Ok(options.application_name("hoodmin"))
}
pub async fn pool(value: &str) -> Result<PgPool> {
	Ok(PgPoolOptions::new()
		.max_connections(12)
		.acquire_timeout(Duration::from_secs(3))
		.after_connect(|conn, _| {
			Box::pin(async move {
				let row = sqlx_core::query::query(
					"SELECT rolsuper,rolbypassrls FROM pg_roles WHERE rolname=current_user",
				)
				.fetch_one(&mut *conn)
				.await?;
				if row.get::<bool, _>("rolsuper") || row.get::<bool, _>("rolbypassrls") {
					tracing::error!(
						"Veritabanı uygulama rolü süper kullanıcı veya BYPASSRLS olamaz."
					);
					return Err(sqlx_core::error::Error::Protocol(
						"Hoodmin uygulama rolü RLS atlayamaz.".into(),
					));
				}
				Ok(())
			})
		})
		.connect_with(database_options(value)?)
		.await?)
}
pub async fn check_schema(pool: &PgPool) -> Result<()> {
	sqlx_core::query::query("SELECT t.id,s.id,k.id,w.token_hash,b.id,p.id,a.id,f.server_profile_id FROM tenants t,servers s,access_tokens k,web_sessions w,bots b,external_publications p,acme_orders a,farms f WHERE FALSE").execute(pool).await?;
	Ok(())
}
pub fn redact(error: &anyhow::Error, database_url: Option<&str>) -> String {
	let mut message = format!("{error:#}");
	if let Some(dsn) = database_url {
		message = message.replace(dsn, "[bağlantı dizesi gizlendi]");
		if let Ok(url) = url::Url::parse(dsn)
			&& let Some(password) = url.password().filter(|p| !p.is_empty())
		{
			message = message.replace(password, "[gizli]");
			let plain = percent_encoding::percent_decode_str(password).decode_utf8_lossy();
			message = message.replace(plain.as_ref(), "[gizli]");
		}
	}
	message
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn remote_database_requires_verified_tls() {
		assert!(
			database_options("postgresql://hoodmin@db.example/hoodmin?sslmode=verify-full").is_ok()
		);
		assert!(
			database_options("postgresql://hoodmin@db.example/hoodmin?sslmode=disable").is_err()
		);
		assert!(
			database_options("postgresql:///hoodmin?host=/var/run/postgresql&user=hoodmin").is_ok()
		);
		let error = anyhow::anyhow!("password secret%21 / secret!");
		assert!(
			!redact(
				&error,
				Some("postgresql://app:secret%21@db.example/hoodmin")
			)
			.contains("secret")
		);
	}
}
