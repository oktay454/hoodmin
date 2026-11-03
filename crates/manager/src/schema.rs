//! Immutable migration catalog: application readiness never applies DDL.
use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::PgPool;
use std::collections::BTreeMap;

pub const FILES: &[(&str, &str)] = &[
	(
		"0001_initial.sql",
		include_str!("../../../migrations/0001_initial.sql"),
	),
	(
		"0002_panel.sql",
		include_str!("../../../migrations/0002_panel.sql"),
	),
	(
		"0003_management_contract.sql",
		include_str!("../../../migrations/0003_management_contract.sql"),
	),
	(
		"0004_farm_and_rewards.sql",
		include_str!("../../../migrations/0004_farm_and_rewards.sql"),
	),
	(
		"0005_install_jobs.sql",
		include_str!("../../../migrations/0005_install_jobs.sql"),
	),
	(
		"0006_server_profiles.sql",
		include_str!("../../../migrations/0006_server_profiles.sql"),
	),
	(
		"0007_team_bots.sql",
		include_str!("../../../migrations/0007_team_bots.sql"),
	),
	(
		"0008_farm_publication.sql",
		include_str!("../../../migrations/0008_farm_publication.sql"),
	),
	(
		"0009_external_publications.sql",
		include_str!("../../../migrations/0009_external_publications.sql"),
	),
	(
		"0010_farm_server_profiles.sql",
		include_str!("../../../migrations/0010_farm_server_profiles.sql"),
	),
	(
		"0011_acme_orders.sql",
		include_str!("../../../migrations/0011_acme_orders.sql"),
	),
	(
		"0012_operations.sql",
		include_str!("../../../migrations/0012_operations.sql"),
	),
	(
		"0013_notifications.sql",
		include_str!("../../../migrations/0013_notifications.sql"),
	),
	(
		"0014_upgrade.sql",
		include_str!("../../../migrations/0014_upgrade.sql"),
	),
];
pub fn hashes() -> BTreeMap<String, String> {
	FILES
		.iter()
		.map(|(name, text)| {
			(
				name.to_string(),
				format!("{:x}", Sha256::digest(text.as_bytes())),
			)
		})
		.collect()
}
pub fn digest() -> String {
	format!(
		"{:x}",
		Sha256::digest(serde_json::to_vec(&hashes()).expect("catalog"))
	)
}
pub fn pending(applied: &BTreeMap<String, String>) -> Result<Vec<String>> {
	let expected = hashes();
	for (name, hash) in applied {
		ensure!(
			expected.get(name) == Some(hash),
			"Desteklenmeyen veya değiştirilmiş şema geçişi: {name}"
		);
	}
	let mut missing = false;
	for (name, _) in FILES {
		if !applied.contains_key(*name) {
			missing = true;
		} else {
			ensure!(!missing, "Şema geçiş sırası eksik: {name}");
		}
	}
	Ok(expected
		.keys()
		.filter(|name| !applied.contains_key(*name))
		.cloned()
		.collect())
}
pub async fn check(pool: &PgPool) -> Result<()> {
	let rows = query("SELECT name,checksum FROM _hoodmin_migrations")
		.fetch_all(pool)
		.await?;
	let applied = rows
		.into_iter()
		.map(|row| (row.get("name"), row.get("checksum")))
		.collect();
	ensure!(
		pending(&applied)?.is_empty(),
		"Veritabanı yükseltmesi bekleniyor."
	);
	Ok(())
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn catalog_rejects_modified_newer_and_gapped_histories() {
		let full = hashes();
		assert!(pending(&full).unwrap().is_empty());
		let mut old = full.clone();
		old.remove("0014_upgrade.sql");
		assert_eq!(pending(&old).unwrap(), ["0014_upgrade.sql"]);
		old.insert("0014_upgrade.sql".into(), "wrong".into());
		assert!(pending(&old).is_err());
		let mut gap = full.clone();
		gap.remove("0002_panel.sql");
		assert!(pending(&gap).is_err());
		let mut newer = full;
		newer.insert("0015_future.sql".into(), "future".into());
		assert!(pending(&newer).is_err());
	}
}
