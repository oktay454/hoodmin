//! Farm profile changes and membership changes serialize per tenant before row locks.
use crate::{crypto::Vault, store::Store};
use anyhow::Result;
use serde_json::Value;
use sqlx_core::{query::query, row::Row, transaction::Transaction};
use sqlx_postgres::{PgRow, Postgres};
use uuid::Uuid;

pub struct Profile {
	pub settings: Value,
	pub management: Value,
}
pub async fn lock(tx: &mut Transaction<'_, Postgres>, tenant: Uuid) -> Result<()> {
	query("SELECT pg_advisory_xact_lock(hashtextextended($1,59833))")
		.bind(tenant.to_string())
		.execute(&mut **tx)
		.await?;
	Ok(())
}
pub async fn assigned(
	tx: &mut Transaction<'_, Postgres>,
	tenant: Uuid,
	farm: Option<Uuid>,
) -> Result<Option<Profile>> {
	let Some(farm) = farm else { return Ok(None) };
	let row = query("SELECT server_profile_id FROM farms WHERE tenant_id=$1 AND id=$2 FOR SHARE")
		.bind(tenant)
		.bind(farm)
		.fetch_optional(&mut **tx)
		.await?;
	let Some(id) = row.and_then(|r| r.get::<Option<Uuid>, _>("server_profile_id")) else {
		return Ok(None);
	};
	let row = query(
		"SELECT settings,management FROM server_profiles WHERE tenant_id=$1 AND id=$2 FOR SHARE",
	)
	.bind(tenant)
	.bind(id)
	.fetch_one(&mut **tx)
	.await?;
	Ok(Some(Profile {
		settings: row.get("settings"),
		management: row.get("management"),
	}))
}
pub fn replace(config: &mut Value, profile: &Profile) -> Result<()> {
	vpnhood_protocol::config::replace(config, profile.settings.clone()).map_err(anyhow::Error::msg)
}
pub async fn apply(
	store: &Store,
	tx: &mut Transaction<'_, Postgres>,
	tenant: Uuid,
	rows: Vec<PgRow>,
	profile: &Profile,
) -> Result<usize> {
	let mut changed = 0;
	for row in rows {
		let id: Uuid = row.get("id");
		let context = format!("{tenant}:{id}:config");
		let mut config: Value = serde_json::from_slice(
			&store
				.vault
				.open(&row.get::<Vec<u8>, _>("configuration"), &context)?,
		)?;
		if vpnhood_protocol::config::editable(&config) == profile.settings
			&& row.get::<Value, _>("management") == profile.management
		{
			continue;
		}
		replace(&mut config, profile)?;
		save(
			&store.vault,
			tx,
			tenant,
			id,
			&mut config,
			&profile.management,
		)
		.await?;
		changed += 1;
	}
	Ok(changed)
}
pub async fn save(
	vault: &Vault,
	tx: &mut Transaction<'_, Postgres>,
	tenant: Uuid,
	id: Uuid,
	config: &mut Value,
	management: &Value,
) -> Result<()> {
	let code = Uuid::new_v4().to_string();
	config["configCode"] = code.clone().into();
	let sealed = vault.seal(
		&serde_json::to_vec(config)?,
		&format!("{tenant}:{id}:config"),
	)?;
	query(
		"UPDATE servers SET configuration=$3,config_code=$4,management=$5 WHERE tenant_id=$1 AND id=$2",
	)
	.bind(tenant)
	.bind(id)
	.bind(sealed)
	.bind(code)
	.bind(management)
	.execute(&mut **tx)
	.await?;
	Ok(())
}
