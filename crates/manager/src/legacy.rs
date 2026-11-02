use crate::{
	acme, auth,
	crypto::{Vault, random, write_private},
	store,
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::Subcommand;
use sha2::{Digest, Sha256};
use sqlx_core::row::Row;
use sqlx_postgres::PgPoolOptions;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum Command {
	AcmeHook {
		#[arg(long)]
		tenant: Uuid,
		#[arg(long)]
		job: Uuid,
		#[arg(long)]
		cleanup: bool,
	},
	CreateAccount {
		#[arg(long)]
		first: bool,
		#[arg(long)]
		tenant: Uuid,
		#[arg(long)]
		tenant_name: String,
		#[arg(long)]
		email: String,
		#[arg(long)]
		password_file: PathBuf,
		#[arg(long, default_value = "admin")]
		role: String,
	},
	RegisterServer {
		#[arg(long)]
		tenant: Uuid,
		#[arg(long)]
		name: String,
		#[arg(long)]
		configuration: PathBuf,
		#[arg(long)]
		credential_out: PathBuf,
	},
	CreateToken {
		#[arg(long)]
		tenant: Uuid,
		#[arg(long)]
		server: Uuid,
		#[arg(long)]
		name: String,
		#[arg(long, default_value_t = 1)]
		max_clients: i32,
		#[arg(long, default_value_t = 0)]
		max_traffic: i64,
		#[arg(long)]
		expires_at: Option<chrono::DateTime<chrono::Utc>>,
		#[arg(long)]
		key_out: PathBuf,
	},
	RevokeToken {
		#[arg(long)]
		tenant: Uuid,
		#[arg(long)]
		token: Uuid,
	},
}

pub async fn run(command: Command, database_url: String, master_key_file: PathBuf) -> Result<()> {
	let metadata = std::fs::metadata(&master_key_file)?;
	use std::os::unix::fs::PermissionsExt;
	ensure!(
		metadata.permissions().mode() & 0o077 == 0,
		"Ana anahtar yalnızca sahibince okunabilir olmalı (0600)."
	);
	let vault = Vault::from_bytes(&std::fs::read(&master_key_file)?)?;
	let pool = PgPoolOptions::new()
		.max_connections(12)
		.connect_with(crate::config::database_options(&database_url)?)
		.await?;
	let role = sqlx_core::query::query(
		"SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname=current_user",
	)
	.fetch_one(&pool)
	.await?;
	ensure!(
		!role.get::<bool, _>("rolsuper") && !role.get::<bool, _>("rolbypassrls"),
		"Uygulama veri tabanı rolü RLS atlayamaz."
	);
	let vault = crate::rotation::bind(&pool, vault).await?;
	let state = store::Store { pool, vault };
	match command {
		Command::AcmeHook {
			tenant,
			job,
			cleanup,
		} => acme::hook(&state, tenant, job, cleanup).await?,
		Command::CreateAccount {
			first,
			tenant,
			tenant_name,
			email,
			password_file,
			role,
		} => {
			if first {
				let count: i64 =
					sqlx_core::query_scalar::query_scalar("SELECT count(*) FROM accounts")
						.fetch_one(&state.pool)
						.await?;
				ensure!(
					count == 0,
					"Mevcut hesaplar var; ilk sahip işlemi tekrarlanamaz."
				);
			}
			let password = std::fs::read_to_string(password_file)?;
			let id = auth::create_account(
				&state,
				tenant,
				&tenant_name,
				&email,
				password.trim_end(),
				&role,
			)
			.await?;
			println!("Panel kullanıcısı oluşturuldu: {id}");
		}
		Command::RegisterServer {
			tenant,
			name,
			configuration,
			credential_out,
		} => {
			ensure!(
				!name.is_empty() && name.len() <= 120,
				"Sunucu adı 1–120 bayt olmalı."
			);
			let mut config: serde_json::Value =
				serde_json::from_slice(&std::fs::read(configuration)?)?;
			ensure!(config.is_object(), "Yapılandırma bir JSON nesnesi olmalı.");
			let server = Uuid::new_v4();
			let code = Uuid::new_v4().to_string();
			config["configCode"] = code.clone().into();
			let credential = format!(
				"{tenant}.{server}.{}",
				URL_SAFE_NO_PAD.encode(random::<32>())
			);
			let hash = Sha256::digest(credential.as_bytes()).to_vec();
			let encrypted = state.vault.seal(
				&serde_json::to_vec(&config)?,
				&format!("{tenant}:{server}:config"),
			)?;
			let mut tx = state.begin(tenant).await?;
			sqlx_core::query::query(
				"INSERT INTO tenants(id,name) VALUES($1,$2) ON CONFLICT(id) DO NOTHING",
			)
			.bind(tenant)
			.bind(format!("Kiracı {tenant}"))
			.execute(&mut *tx)
			.await?;
			sqlx_core::query::query("INSERT INTO servers(tenant_id,id,name,credential_hash,configuration,config_code,agent_credential) VALUES($1,$2,$3,$4,$5,$6,$7)")
				.bind(tenant).bind(server).bind(name).bind(hash).bind(encrypted).bind(code).bind(state.vault.seal(credential.as_bytes(),&format!("{tenant}:{server}:agent"))?).execute(&mut *tx).await?;
			write_private(&credential_out, credential.as_bytes())?;
			tx.commit().await?;
			println!(
				"Sunucu kaydedildi: {server}. Kimlik bilgisi: {}",
				credential_out.display()
			);
		}
		Command::CreateToken {
			tenant,
			server,
			name,
			max_clients,
			max_traffic,
			expires_at,
			key_out,
		} => {
			ensure!(
				(0..=10000).contains(&max_clients) && max_traffic >= 0,
				"Kota değerleri geçersiz."
			);
			ensure!(
				!name.is_empty() && name.len() <= 120,
				"Anahtar adı 1–120 bayt olmalı."
			);
			let mut tx = state.begin(tenant).await?;
			let configuration: Vec<u8> = sqlx_core::query_scalar::query_scalar(
				"SELECT configuration FROM servers WHERE tenant_id=$1 AND id=$2",
			)
			.bind(tenant)
			.bind(server)
			.fetch_one(&mut *tx)
			.await
			.context("Sunucu bu kiracıya ait değil.")?;
			let config: serde_json::Value = serde_json::from_slice(
				&state
					.vault
					.open(&configuration, &format!("{tenant}:{server}:config"))?,
			)?;
			// Test ve ilk aşama yapılandırması açık bir publicServerToken içerir; sunucuya gönderilmez.
			let server_token: vpnhood_protocol::ServerToken =
				serde_json::from_value(config["publicServerToken"].clone())?;
			let id = Uuid::new_v4();
			let secret = random::<16>().to_vec();
			let encrypted = state.vault.seal(&secret, &format!("{tenant}:{id}:token"))?;
			sqlx_core::query::query("INSERT INTO access_tokens(tenant_id,id,server_id,name,secret,max_clients,max_traffic,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
				.bind(tenant).bind(id).bind(server).bind(&name).bind(encrypted).bind(max_clients).bind(max_traffic).bind(expires_at)
				.execute(&mut *tx).await?;
			let token = vpnhood_protocol::AccessToken {
				v: 4,
				name,
				sid: None,
				tid: id.to_string(),
				iat: chrono::Utc::now(),
				sec: secret,
				ser: server_token,
				tags: vec![],
				..Default::default()
			};
			write_private(&key_out, token.to_access_key()?.as_bytes())?;
			tx.commit().await?;
			println!(
				"Erişim anahtarı oluşturuldu: {id}. Özel dosya: {}",
				key_out.display()
			);
		}
		Command::RevokeToken { tenant, token } => {
			let mut tx = state.begin(tenant).await?;
			let affected = sqlx_core::query::query(
				"UPDATE access_tokens SET revoked=true WHERE tenant_id=$1 AND id=$2",
			)
			.bind(tenant)
			.bind(token)
			.execute(&mut *tx)
			.await?
			.rows_affected();
			ensure!(affected == 1, "Anahtar bulunamadı.");
			tx.commit().await?;
			println!("Anahtar iptal edildi: {token}");
		}
	}
	Ok(())
}
