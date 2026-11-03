use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
	collections::BTreeMap,
	fs,
	path::{Path, PathBuf},
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
	pub enabled: bool,
	pub notify_tenant: Option<uuid::Uuid>,
	pub directory: PathBuf,
	pub keep_min: usize,
	pub max_count: usize,
	pub max_age_days: u32,
	pub max_bytes: u64,
	pub min_free_bytes: u64,
}
impl Default for Policy {
	fn default() -> Self {
		Self {
			enabled: false,
			notify_tenant: None,
			directory: "/var/backups/hoodmin".into(),
			keep_min: 3,
			max_count: 30,
			max_age_days: 30,
			max_bytes: 0,
			min_free_bytes: 1_073_741_824,
		}
	}
}
impl Policy {
	pub fn validate(&self) -> Result<()> {
		ensure!(
			self.directory.is_absolute() && self.directory.components().count() > 2,
			"Yedek dizini mutlak ve kök dizinden ayrı olmalı."
		);
		ensure!(
			self.keep_min >= 1 && self.keep_min <= 10000 && self.max_count >= self.keep_min,
			"En az bir yedek korunmalı; max_count keep_min değerinden küçük olamaz."
		);
		Ok(())
	}
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
	version: u32,
	completed_at: chrono::DateTime<chrono::Utc>,
	files: BTreeMap<String, String>,
}
const MARKER: &str = "hoodmin-backup.json";
pub fn complete(path: &Path) -> Result<()> {
	let files = inventory(path)?;
	crate::crypto::write_private(
		&path.join(MARKER),
		&serde_json::to_vec_pretty(&Manifest {
			version: 1,
			completed_at: chrono::Utc::now(),
			files,
		})?,
	)?;
	fs::File::open(path)?.sync_all()?;
	Ok(())
}
fn inventory(base: &Path) -> Result<BTreeMap<String, String>> {
	fn visit(base: &Path, path: &Path, files: &mut BTreeMap<String, String>) -> Result<()> {
		use sha2::{Digest, Sha256};
		use std::os::unix::ffi::OsStrExt;
		for entry in fs::read_dir(path)? {
			let path = entry?.path();
			if path == base.join(MARKER) {
				continue;
			}
			let metadata = fs::symlink_metadata(&path)?;
			let key = path
				.strip_prefix(base)?
				.to_str()
				.context("Yedek dosya adı UTF-8 olmalı.")?
				.to_owned();
			let value = if metadata.file_type().is_symlink() {
				format!(
					"link:{:x}",
					Sha256::digest(fs::read_link(&path)?.as_os_str().as_bytes())
				)
			} else if metadata.is_dir() {
				visit(base, &path, files)?;
				"directory".into()
			} else {
				ensure!(metadata.is_file(), "Yedekte özel dosya olamaz.");
				format!("sha256:{}", crate::cli_backup::digest(&path)?)
			};
			files.insert(key, value);
		}
		Ok(())
	}
	let mut files = BTreeMap::new();
	visit(base, base, &mut files)?;
	Ok(files)
}

fn bytes(path: &Path) -> Result<u64> {
	let m = fs::symlink_metadata(path)?;
	if m.is_dir() {
		let mut sum = 0u64;
		for e in fs::read_dir(path)? {
			sum = sum
				.checked_add(bytes(&e?.path())?)
				.context("Yedek boyutu taşması.")?;
		}
		Ok(sum)
	} else {
		Ok(m.len())
	}
}
fn normal(path: &Path) -> bool {
	fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
}
pub(crate) fn verify(path: &Path) -> Result<()> {
	valid(path).map(|_| ())
}
fn valid(path: &Path) -> Result<Manifest> {
	ensure!(normal(&path.join(MARKER)), "Yedek tamamlanma kaydı yok.");
	let m: Manifest = serde_json::from_slice(&fs::read(path.join(MARKER))?)?;
	ensure!(m.version == 1, "Yedek kaydı geçersiz.");
	for name in [
		"database.dump",
		"master.key",
		"hoodmin.json",
		"package-version.txt",
	] {
		ensure!(
			normal(&path.join(name)) && m.files.contains_key(name),
			"Temel yedek dosyası eksik."
		);
	}
	ensure!(
		inventory(path)? == m.files,
		"Yedek ağacının bütünlüğü geçersiz."
	);
	Ok(m)
}
pub fn free_bytes(path: &Path) -> Result<u64> {
	let output = std::process::Command::new("df")
		.args(["--output=avail", "-B1", "--"])
		.arg(path)
		.env("LC_ALL", "C")
		.output()?;
	ensure!(output.status.success(), "Boş disk alanı okunamadı.");
	String::from_utf8(output.stdout)?
		.lines()
		.last()
		.context("Disk alanı eksik.")?
		.trim()
		.parse()
		.context("Disk alanı geçersiz.")
}
#[derive(Serialize)]
pub struct Report {
	pub apply: bool,
	pub valid: usize,
	pub ignored: usize,
	pub removed: Vec<String>,
	pub retained: usize,
	pub bytes: u64,
	pub total_bytes: u64,
	pub free_bytes: u64,
	pub pressure: bool,
}
pub fn prune(policy: &Policy, apply: bool) -> Result<Report> {
	policy.validate()?;
	use std::os::unix::fs::MetadataExt;
	let latest = fs::symlink_metadata(policy.directory.join("latest.age"))
		.ok()
		.filter(|m| m.is_file() && !m.file_type().is_symlink());
	ensure!(
		policy.directory.is_dir() && !policy.directory.is_symlink(),
		"Yedek dizini normal dizin olmalı."
	);
	let mut entries = Vec::new();
	let mut ignored = 0;
	for entry in fs::read_dir(&policy.directory)? {
		let e = entry?;
		let path = e.path();
		let name = e.file_name().to_string_lossy().into_owned();
		if !name.starts_with("hoodmin-") || !e.file_type()?.is_dir() {
			continue;
		}
		match valid(&path) {
			Ok(m) => {
				let archive = path.with_extension("age");
				let archive = if normal(&archive) {
					Some(fs::metadata(archive)?)
				} else {
					None
				};
				let is_latest = archive
					.as_ref()
					.zip(latest.as_ref())
					.is_some_and(|(a, l)| a.dev() == l.dev() && a.ino() == l.ino());
				let size = bytes(&path)? + archive.as_ref().map_or(0, |m| m.len());
				entries.push((m.completed_at, path, size, is_latest));
			}
			Err(_) => ignored += 1,
		}
	}
	entries.sort_by_key(|a| std::cmp::Reverse(a.0));
	let free = free_bytes(&policy.directory)?;
	let mut total: u64 = entries.iter().map(|e| e.2).sum();
	let total_bytes = total;
	let mut remaining = entries.len();
	let mut reclaimed = 0;
	let mut removed = Vec::new();
	for (i, (date, path, size, is_latest)) in entries.iter().enumerate().rev() {
		if i < policy.keep_min || *is_latest {
			continue;
		}
		let age = chrono::Utc::now().signed_duration_since(*date).num_days();
		let expired = policy.max_age_days > 0 && age >= i64::from(policy.max_age_days);
		if remaining <= policy.max_count
			&& !expired
			&& (policy.max_bytes == 0 || total <= policy.max_bytes)
			&& free.saturating_add(reclaimed) >= policy.min_free_bytes
		{
			continue;
		}
		if apply {
			// Revalidate just before deletion. Never follow directory or file symlinks.
			ensure!(!path.is_symlink(), "Yedek yolu değişti.");
			valid(path)?;
			let quarantine = policy
				.directory
				.join(format!(".prune-{}", uuid::Uuid::new_v4()));
			fs::rename(path, &quarantine)?;
			fs::remove_dir_all(quarantine)?;
			let archive = path.with_extension("age");
			if normal(&archive) {
				fs::remove_file(archive)?;
			}
		}
		removed.push(path.file_name().unwrap().to_string_lossy().into_owned());
		remaining -= 1;
		total -= size;
		reclaimed += size;
	}
	let actual_free = if apply {
		free_bytes(&policy.directory)?
	} else {
		free.saturating_add(reclaimed)
	};
	Ok(Report {
		apply,
		valid: entries.len(),
		ignored,
		removed,
		retained: remaining,
		bytes: total,
		total_bytes,
		free_bytes: actual_free,
		pressure: remaining > policy.max_count
			|| (policy.max_bytes > 0 && total > policy.max_bytes)
			|| actual_free < policy.min_free_bytes,
	})
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn latest_encrypted_copy_survives_newer_plain_backups() {
		let root = std::env::temp_dir().join(format!("hoodmin-latest-{}", uuid::Uuid::new_v4()));
		fs::create_dir(&root).unwrap();
		for i in 0..3 {
			let p = root.join(format!("hoodmin-{i}"));
			fs::create_dir(&p).unwrap();
			for n in [
				"database.dump",
				"master.key",
				"hoodmin.json",
				"package-version.txt",
			] {
				fs::write(p.join(n), b"data").unwrap();
			}
			complete(&p).unwrap();
			let mut m: Manifest =
				serde_json::from_slice(&fs::read(p.join(MARKER)).unwrap()).unwrap();
			m.completed_at = chrono::Utc::now() - chrono::Duration::days(3 - i);
			fs::write(p.join(MARKER), serde_json::to_vec(&m).unwrap()).unwrap();
		}
		fs::write(root.join("hoodmin-0.age"), b"encrypted").unwrap();
		fs::hard_link(root.join("hoodmin-0.age"), root.join("latest.age")).unwrap();
		let report = prune(
			&Policy {
				directory: root.clone(),
				keep_min: 1,
				max_count: 1,
				max_age_days: 0,
				min_free_bytes: 0,
				..Default::default()
			},
			true,
		)
		.unwrap();
		assert_eq!(report.retained, 2);
		assert!(report.pressure);
		assert!(root.join("hoodmin-0.age").exists());
		assert!(root.join("hoodmin-2").exists());
		assert!(!root.join("hoodmin-1").exists());
		fs::remove_dir_all(root).unwrap();
	}
	#[test]
	fn retention_preserves_minimum_and_ignores_partial_and_symlinks() {
		let root = std::env::temp_dir().join(format!("hoodmin-retention-{}", uuid::Uuid::new_v4()));
		fs::create_dir(&root).unwrap();
		for i in 0..5 {
			let p = root.join(format!("hoodmin-{i}"));
			fs::create_dir(&p).unwrap();
			for n in [
				"database.dump",
				"master.key",
				"hoodmin.json",
				"package-version.txt",
			] {
				fs::write(p.join(n), b"data").unwrap();
			}
			complete(&p).unwrap();
		}
		let corrupt = root.join("hoodmin-corrupt");
		fs::create_dir(&corrupt).unwrap();
		for name in [
			"database.dump",
			"master.key",
			"hoodmin.json",
			"package-version.txt",
			"extra-cert",
		] {
			fs::write(corrupt.join(name), b"original").unwrap();
		}
		complete(&corrupt).unwrap();
		fs::write(corrupt.join("extra-cert"), b"corrupt").unwrap();
		fs::create_dir(root.join("hoodmin-partial")).unwrap();
		std::os::unix::fs::symlink(root.join("hoodmin-0"), root.join("hoodmin-link")).unwrap();
		let policy = Policy {
			directory: root.clone(),
			keep_min: 2,
			max_count: 2,
			min_free_bytes: 0,
			..Default::default()
		};
		let report = prune(&policy, false).unwrap();
		assert_eq!(report.removed.len(), 3);
		assert_eq!(report.ignored, 2);
		assert!(root.join("hoodmin-0").exists());
		let report = prune(&policy, true).unwrap();
		assert_eq!(report.retained, 2);
		assert!(root.join("hoodmin-partial").exists());
		assert!(root.join("hoodmin-link").is_symlink());
		assert!(prune(&policy, true).unwrap().removed.is_empty());
		let pressure = prune(
			&Policy {
				max_bytes: 1,
				..policy
			},
			true,
		)
		.unwrap();
		assert!(pressure.pressure);
		assert!(pressure.removed.is_empty());
		assert_eq!(pressure.retained, 2);
		fs::remove_dir_all(root).unwrap();
	}
}
