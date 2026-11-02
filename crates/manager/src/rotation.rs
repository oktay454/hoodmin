use crate::{
	config::Settings,
	crypto::{Vault, maintenance_lock, random, replace_key},
};
use anyhow::{Result, ensure};
use clap::Subcommand;
use sqlx_core::{query::query, query_scalar::query_scalar, row::Row};
use sqlx_postgres::PgPool;
use uuid::Uuid;
pub const LOCK: i64 = 684663002;
#[derive(Subcommand)]
pub enum Action {
	Status,
	Rotate {
		#[arg(long)]
		confirmed: bool,
	},
}
pub async fn bind(pool: &PgPool, vault: Vault) -> Result<Vault> {
	let id: Option<String> = query_scalar("SELECT active_key_id FROM vault_state WHERE id")
		.fetch_one(pool)
		.await?;
	match id {
		Some(id) => vault.select(&id),
		None => Ok(vault),
	}
}
pub async fn run(settings: &Settings, action: Action, peer_helper: bool) -> Result<()> {
	let options = crate::config::database_options(settings.db()?)?;
	if !peer_helper {
		crate::cli::root()?;
		if options.get_socket().is_some() {
			// Root maintains files; the local service identity authenticates via PostgreSQL peer.
			drop(maintenance_lock(&settings.master_key_file)?);
			use std::{
				io::Write,
				process::{Command, Stdio},
			};
			let mut command = Command::new("runuser");
			command
				.args(["-u", options.get_username(), "--"])
				.arg(std::env::current_exe()?)
				.args(["keys", "--peer-helper"]);
			match action {
				Action::Status => {
					command.arg("status");
				}
				Action::Rotate { confirmed } => {
					command.arg("rotate");
					if confirmed {
						command.arg("--confirmed");
					}
				}
			}
			let mut child = command.stdin(Stdio::piped()).spawn()?;
			child
				.stdin
				.take()
				.unwrap()
				.write_all(&serde_json::to_vec(settings)?)?;
			ensure!(
				child.wait()?.success(),
				"Yerel anahtar işlemi tamamlanamadı."
			);
			return Ok(());
		}
	} else {
		use std::os::unix::fs::MetadataExt;
		let identity = std::process::Command::new("id").arg("-un").output()?;
		let uid = std::process::Command::new("id").arg("-u").output()?;
		ensure!(
			options.get_socket().is_some()
				&& String::from_utf8(identity.stdout)?.trim() == options.get_username()
				&& String::from_utf8(uid.stdout)?.trim().parse::<u32>()?
					== std::fs::metadata(&settings.master_key_file)?.uid(),
			"Yerel anahtar yardımcısı yalnız eşleşen hizmet hesabında çalışabilir."
		);
	}
	let _lock = maintenance_lock(&settings.master_key_file)?;
	let pool = crate::config::pool(settings.db()?).await?;
	let old = bind(&pool, settings.vault()?).await?;
	match action {
		Action::Status => println!(
			"{}",
			serde_json::json!({"active":old.id(),"available":old.ids()})
		),
		Action::Rotate { confirmed } => {
			ensure!(
				confirmed,
				"Anahtar döndürme için --confirmed gerekli; önce bağımsız yedeğinizi alın."
			);
			let key = random::<32>();
			let next = Vault::new(&key)?;
			let dual = old.add(key)?;
			// Durable dual-reader file comes before any database ciphertext changes.
			replace_key(&settings.master_key_file, &dual.encode()?)?;
			let new = dual.select(next.id())?;
			let mut tx = pool.begin().await?;
			query("SELECT pg_advisory_xact_lock($1)")
				.bind(LOCK)
				.execute(&mut *tx)
				.await?;
			let active: Option<String> =
				query_scalar("SELECT active_key_id FROM vault_state WHERE id FOR UPDATE")
					.fetch_one(&mut *tx)
					.await?;
			ensure!(
				active.as_deref().is_none_or(|id| id == old.id()),
				"Etkin anahtar değişti; işlemi yeniden başlatın."
			);
			let tenants: Vec<Uuid> = query_scalar("SELECT id FROM hoodmin_vault_tenants()")
				.fetch_all(&mut *tx)
				.await?;
			let mut count = 0;
			for tenant in tenants {
				query("SELECT set_config('app.tenant_id',$1,true)")
					.bind(tenant.to_string())
					.execute(&mut *tx)
					.await?;
				for (table, column, suffix, session) in [
					("servers", "configuration", "config", false),
					("servers", "agent_credential", "agent", false),
					("access_tokens", "secret", "token", false),
					("sessions", "session_key", "session", true),
					(
						"notification_channels",
						"specification",
						"notification",
						false,
					),
					("farms", "certificates", "certificates", false),
					("external_publications", "specification", "publisher", false),
				] {
					let sql = format!(
						"SELECT id,{} AS context_id,{column} AS value FROM {table} WHERE tenant_id=$1 AND {column} IS NOT NULL ORDER BY id FOR UPDATE",
						if session { "server_id" } else { "id" }
					);
					for row in query(&sql).bind(tenant).fetch_all(&mut *tx).await? {
						let context =
							format!("{tenant}:{}:{suffix}", row.get::<Uuid, _>("context_id"));
						let plain = old.open(&row.get::<Vec<u8>, _>("value"), &context)?;
						let sealed = new.seal(&plain, &context)?;
						let update =
							format!("UPDATE {table} SET {column}=$3 WHERE tenant_id=$1 AND id=$2");
						let q = query(&update).bind(tenant);
						if session {
							q.bind(row.get::<i64, _>("id"))
								.bind(sealed)
								.execute(&mut *tx)
								.await?;
						} else {
							q.bind(row.get::<Uuid, _>("id"))
								.bind(sealed)
								.execute(&mut *tx)
								.await?;
						}
						count += 1;
					}
				}
			}
			query("UPDATE vault_state SET active_key_id=$1,rotated_at=now() WHERE id")
				.bind(new.id())
				.execute(&mut *tx)
				.await?;
			tx.commit().await?;
			// A crash here is recoverable: startup chooses DB's active ID from the durable dual file.
			replace_key(&settings.master_key_file, &new.encode()?)?;
			println!(
				"{}",
				serde_json::json!({"active":new.id(),"rewrapped":count,"previous_keys_retained":true})
			);
		}
	}
	Ok(())
}
