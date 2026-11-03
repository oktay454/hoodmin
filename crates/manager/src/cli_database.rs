use crate::config::Settings;
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use sha2::{Digest, Sha256};
use sqlx_core::{
	connection::Connection, executor::Executor, query::query, query_scalar::query_scalar,
};
use sqlx_postgres::{PgConnectOptions, PgConnection};
use std::path::PathBuf;
#[derive(Subcommand)]
pub enum Action {
	/// Veritabanını/rolü ve şemayı kur. Uzak yönetici sır dosyası veya --local seç.
	Init {
		#[arg(long, conflicts_with = "local")]
		admin_url_file: Option<PathBuf>,
		#[arg(long)]
		local: bool,
		#[arg(long, hide = true)]
		internal_local: bool,
	},
	/// Uygulama rolünü ve bağlantıyı denetle.
	Check,
}
pub async fn run(settings: &Settings, action: Action) -> Result<()> {
	match action {
		Action::Check => {
			let pool = crate::config::pool(settings.db()?).await?;
			crate::config::check_schema(&pool).await?;
			println!("Veritabanı bağlantısı, uygulama rolü ve şema hazır.");
		}
		Action::Init {
			admin_url_file,
			local,
			internal_local,
		} => {
			if local {
				return crate::cli::local_bootstrap(settings);
			}
			let actual;
			let (settings, admin) = if internal_local {
				use std::io::Read;
				let mut text = String::new();
				std::io::stdin().take(65536).read_to_string(&mut text)?;
				actual = serde_json::from_str::<Settings>(&text)
					.context("İç kurulum girdisi geçersiz.")?;
				let app = crate::config::database_options(actual.db()?)?;
				let socket = app.get_socket().context("Yerel soket gerekli.")?;
				let admin = PgConnectOptions::new()
					.host(&socket.to_string_lossy())
					.port(app.get_port())
					.username("postgres")
					.database("postgres");
				(&actual, admin)
			} else {
				let path = admin_url_file.context("--admin-url-file veya --local gerekli.")?;
				(
					settings,
					crate::config::database_options(&crate::cli::read_secret(&path)?)?,
				)
			};
			bootstrap(settings, admin).await?;
			println!("Veritabanı şeması kuruldu; geçiş özetleri doğrulandı.");
		}
	}
	Ok(())
}
pub(crate) fn identifier(value: &str) -> Result<String> {
	ensure!(
		!value.is_empty() && value.len() <= 63 && !value.contains('\0'),
		"Veritabanı veya rol adı geçersiz."
	);
	Ok(format!("\"{}\"", value.replace('"', "\"\"")))
}
async fn bootstrap(settings: &Settings, admin: PgConnectOptions) -> Result<()> {
	let app = crate::config::database_options(settings.db()?)?;
	let role = app.get_username();
	let db = app.get_database().context("Veritabanı adı gerekli.")?;
	let quoted_role = identifier(role)?;
	let quoted_db = identifier(db)?;
	let mut connection = PgConnection::connect_with(&admin)
		.await
		.context("Yönetici veritabanı bağlantısı kurulamadı.")?;
	let bypass: bool =
		query_scalar("SELECT rolsuper OR rolbypassrls FROM pg_roles WHERE rolname=current_user")
			.fetch_one(&mut connection)
			.await?;
	ensure!(
		bypass,
		"Şema yöneticisi SUPERUSER veya BYPASSRLS yetkisine sahip olmalı; bu yetkiyi uygulama rolüne vermeyin."
	);
	query("SELECT pg_advisory_lock(684663001)")
		.execute(&mut connection)
		.await?;
	if !query_scalar::<_, bool>("SELECT EXISTS(SELECT FROM pg_roles WHERE rolname=$1)")
		.bind(role)
		.fetch_one(&mut connection)
		.await?
	{
		connection
			.execute(
				format!(
					"CREATE ROLE {quoted_role} LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS"
				)
				.as_str(),
			)
			.await?;
		if let Some(password) = url::Url::parse(settings.db()?)?.password() {
			let password = percent_encoding::percent_decode_str(password).decode_utf8()?;
			query("SELECT set_config('hoodmin.bootstrap_password',$1,false)")
				.bind(password.as_ref())
				.execute(&mut connection)
				.await?;
			// Password remains a bound parameter, never part of a logged SQL statement.
			query("SELECT set_config('hoodmin.bootstrap_role',$1,false)")
				.bind(role)
				.execute(&mut connection)
				.await?;
			connection.execute("DO $$ BEGIN EXECUTE format('ALTER ROLE %I PASSWORD %L',current_setting('hoodmin.bootstrap_role'),current_setting('hoodmin.bootstrap_password')); END $$;").await?;
			connection
				.execute("RESET hoodmin.bootstrap_password; RESET hoodmin.bootstrap_role;")
				.await?;
		}
	}
	if !query_scalar::<_, bool>("SELECT EXISTS(SELECT FROM pg_database WHERE datname=$1)")
		.bind(db)
		.fetch_one(&mut connection)
		.await?
	{
		connection
			.execute(format!("CREATE DATABASE {quoted_db}").as_str())
			.await?;
	}
	query("SELECT pg_advisory_unlock(684663001)")
		.execute(&mut connection)
		.await?;
	drop(connection);
	migrate_existing(settings, admin).await
}
pub(crate) async fn migrate_existing(settings: &Settings, admin: PgConnectOptions) -> Result<()> {
	let app = crate::config::database_options(settings.db()?)?;
	let quoted_role = identifier(app.get_username())?;
	let quoted_db = identifier(app.get_database().context("Veritabanı adı gerekli.")?)?;
	let mut connection =
		PgConnection::connect_with(&admin.database(app.get_database().unwrap())).await?;
	apply_locked(&mut connection, &quoted_role, &quoted_db).await
}
pub(crate) async fn apply_locked(
	connection: &mut PgConnection,
	quoted_role: &str,
	quoted_db: &str,
) -> Result<()> {
	query("SELECT pg_advisory_lock(684663001)")
		.execute(&mut *connection)
		.await?;
	let untracked:bool=query_scalar("SELECT to_regclass('public.tenants') IS NOT NULL AND to_regclass('public._hoodmin_migrations') IS NULL").fetch_one(&mut *connection).await?;
	ensure!(
		!untracked,
		"İzlenmeyen mevcut şema bulundu; boş veritabanı kullanın."
	);
	connection.execute("CREATE TABLE IF NOT EXISTS _hoodmin_migrations(name text PRIMARY KEY,checksum text NOT NULL,applied_at timestamptz NOT NULL DEFAULT now())").await?;
	let mut files = std::fs::read_dir("/usr/share/hoodmin/migrations")?
		.map(|e| e.map(|e| e.path()))
		.collect::<std::io::Result<Vec<_>>>()?;
	files.sort();
	ensure!(!files.is_empty(), "Paket şema dosyaları bulunamadı.");
	for path in files
		.into_iter()
		.filter(|p| p.extension().is_some_and(|e| e == "sql"))
	{
		let name = path
			.file_name()
			.context("Geçiş adı geçersiz.")?
			.to_str()
			.context("Geçiş adı UTF-8 olmalı.")?;
		let data = std::fs::read_to_string(&path)?;
		let hash = format!("{:x}", Sha256::digest(data.as_bytes()));
		if let Some(old) =
			query_scalar::<_, String>("SELECT checksum FROM _hoodmin_migrations WHERE name=$1")
				.bind(name)
				.fetch_optional(&mut *connection)
				.await?
		{
			ensure!(old == hash, "Geçiş özeti uyuşmuyor: {name}");
			continue;
		}
		let begin = data.find("BEGIN;").context("Geçiş BEGIN içermiyor.")?;
		let body = data[begin + 6..]
			.trim_end()
			.strip_suffix("COMMIT;")
			.context("Geçiş COMMIT içermiyor.")?;
		let body = body
			.replace(" TO hoodmin;", &format!(" TO {quoted_role};"))
			.replace(" FROM hoodmin;", &format!(" FROM {quoted_role};"));
		let mut tx = connection.begin().await?;
		tx.execute(body.as_str()).await?;
		query("INSERT INTO _hoodmin_migrations(name,checksum) VALUES($1,$2)")
			.bind(name)
			.bind(&hash)
			.execute(&mut *tx)
			.await?;
		tx.commit().await?;
	}
	connection.execute(format!("GRANT CONNECT ON DATABASE {quoted_db} TO {quoted_role}; GRANT USAGE ON SCHEMA public TO {quoted_role}; GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA public TO {quoted_role}; GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO {quoted_role}; REVOKE UPDATE,DELETE ON audit_events FROM {quoted_role}; REVOKE ALL ON _hoodmin_migrations FROM {quoted_role}; REVOKE ALL ON _hoodmin_migrations FROM PUBLIC; GRANT SELECT ON _hoodmin_migrations TO {quoted_role}; GRANT EXECUTE ON FUNCTION hoodmin_vault_tenants() TO {quoted_role}; GRANT EXECUTE ON FUNCTION hoodmin_due_acme() TO {quoted_role}; GRANT EXECUTE ON FUNCTION hoodmin_due_publications() TO {quoted_role};").as_str()).await?;
	query("SELECT pg_advisory_unlock(684663001)")
		.execute(&mut *connection)
		.await?;
	Ok(())
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn identifiers_are_quoted() {
		assert_eq!(identifier("a\"b").unwrap(), "\"a\"\"b\"");
		assert!(identifier("").is_err());
	}
}
