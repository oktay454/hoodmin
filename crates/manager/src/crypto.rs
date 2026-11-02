use aes_gcm::{
	Aes256Gcm, KeyInit, Nonce,
	aead::{Aead, Payload},
};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
const MAGIC: &[u8] = b"HMK1";
#[derive(Clone)]
pub struct Vault {
	current: String,
	keys: BTreeMap<String, [u8; 32]>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Keyring {
	version: u32,
	current: String,
	keys: BTreeMap<String, String>,
}
impl Vault {
	pub fn new(key: &[u8]) -> Result<Self> {
		let key: [u8; 32] = key
			.try_into()
			.map_err(|_| anyhow::anyhow!("Ana anahtar 32 bayt olmalı."))?;
		let id = key_id(&key);
		Ok(Self {
			current: id.clone(),
			keys: BTreeMap::from([(id, key)]),
		})
	}
	pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
		if bytes.len() == 32 {
			return Self::new(bytes);
		}
		let ring: Keyring = serde_json::from_slice(bytes)
			.map_err(|_| anyhow::anyhow!("Anahtar halkası geçersiz."))?;
		ensure!(
			ring.version == 1 && !ring.keys.is_empty() && ring.keys.len() <= 32,
			"Anahtar halkası sürümü/boyutu geçersiz."
		);
		let mut keys = BTreeMap::new();
		for (id, value) in ring.keys {
			let key: [u8; 32] = STANDARD
				.decode(value)?
				.try_into()
				.map_err(|_| anyhow::anyhow!("Anahtar boyutu geçersiz."))?;
			ensure!(key_id(&key) == id, "Anahtar kimliği uyuşmuyor.");
			keys.insert(id, key);
		}
		ensure!(
			keys.contains_key(&ring.current),
			"Etkin anahtar bulunamadı."
		);
		Ok(Self {
			current: ring.current,
			keys,
		})
	}
	pub fn id(&self) -> &str {
		&self.current
	}
	pub fn ids(&self) -> Vec<String> {
		self.keys.keys().cloned().collect()
	}
	pub fn select(&self, id: &str) -> Result<Self> {
		ensure!(
			self.keys.contains_key(id),
			"Veritabanının etkin anahtarı dosyada bulunamadı."
		);
		let mut v = self.clone();
		v.current = id.to_owned();
		Ok(v)
	}
	pub fn add(&self, key: [u8; 32]) -> Result<Self> {
		ensure!(
			self.keys.len() < 32,
			"Anahtar halkası dolu; eski yedeklerin anahtar gereksinimini denetleyin."
		);
		let mut v = self.clone();
		v.keys.insert(key_id(&key), key);
		Ok(v)
	}
	pub fn encode(&self) -> Result<Vec<u8>> {
		Ok(serde_json::to_vec_pretty(&Keyring {
			version: 1,
			current: self.current.clone(),
			keys: self
				.keys
				.iter()
				.map(|(id, key)| (id.clone(), STANDARD.encode(key)))
				.collect(),
		})?)
	}
	pub fn seal(&self, data: &[u8], context: &str) -> Result<Vec<u8>> {
		let nonce = random::<12>();
		let cipher = Aes256Gcm::new_from_slice(&self.keys[&self.current]).expect("32 bytes");
		let encrypted = cipher
			.encrypt(
				Nonce::from_slice(&nonce),
				Payload {
					msg: data,
					aad: context.as_bytes(),
				},
			)
			.map_err(|_| anyhow::anyhow!("Şifreleme başarısız."))?;
		Ok([MAGIC, self.current.as_bytes(), &nonce, &encrypted].concat())
	}
	pub fn open(&self, data: &[u8], context: &str) -> Result<Vec<u8>> {
		// Legacy random nonces can begin with MAGIC. A versioned envelope must authenticate
		// before it is accepted; otherwise still try the authenticated legacy representation.
		if data.starts_with(MAGIC)
			&& data.len() >= 64
			&& let Ok(id) = std::str::from_utf8(&data[4..36])
			&& let Some(key) = self.keys.get(id)
			&& let Ok(plain) = decrypt(key, &data[36..], context)
		{
			return Ok(plain);
		}

		for key in self.keys.values() {
			if let Ok(plain) = decrypt(key, data, context) {
				return Ok(plain);
			}
		}
		bail!("Şifreli veri doğrulanamadı.")
	}
}
fn key_id(key: &[u8]) -> String {
	format!("{:x}", Sha256::digest(key))[..32].to_owned()
}
fn decrypt(key: &[u8; 32], data: &[u8], context: &str) -> Result<Vec<u8>> {
	ensure!(data.len() >= 28, "Şifreli veri geçersiz.");
	Aes256Gcm::new_from_slice(key)
		.expect("32 bytes")
		.decrypt(
			Nonce::from_slice(&data[..12]),
			Payload {
				msg: &data[12..],
				aad: context.as_bytes(),
			},
		)
		.map_err(|_| anyhow::anyhow!("Şifreli veri doğrulanamadı."))
}
pub fn random<const N: usize>() -> [u8; N] {
	let mut bytes = [0; N];
	OsRng.fill_bytes(&mut bytes);
	bytes
}
pub fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
	write_private_file(path, bytes).map(drop)
}
pub(crate) fn write_private_file(path: &std::path::Path, bytes: &[u8]) -> Result<std::fs::File> {
	use std::{io::Write, os::unix::fs::OpenOptionsExt};
	let mut file = std::fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.mode(0o600)
		.open(path)
		.with_context(|| format!("Özel dosya oluşturulamadı: {}", path.display()))?;
	file.write_all(bytes)?;
	file.sync_all()?;
	Ok(file)
}
pub fn maintenance_lock(key: &std::path::Path) -> Result<std::fs::File> {
	use std::os::unix::fs::OpenOptionsExt;
	let path = key.with_extension("maintenance.lock");
	if let Ok(meta) = std::fs::symlink_metadata(&path) {
		ensure!(
			meta.is_file() && !meta.file_type().is_symlink(),
			"Bakım kilidi normal dosya olmalı."
		);
	}
	let file = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(false)
		.mode(0o600)
		.custom_flags(0o400000)
		.open(path)?; // Linux O_NOFOLLOW: never chown through a raced symlink.
	let meta = std::fs::metadata(key)?;
	use std::os::unix::fs::MetadataExt;
	ensure!(
		file.metadata()?.nlink() == 1,
		"Bakım kilidinin başka bağlantısı olamaz."
	);
	if file.metadata()?.uid() != meta.uid() {
		std::os::unix::fs::fchown(&file, Some(meta.uid()), Some(meta.gid()))?;
	}
	file.lock()?;
	Ok(file)
}
pub fn replace_key(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
	use std::os::unix::fs::MetadataExt;
	let metadata = std::fs::symlink_metadata(path)?;
	ensure!(
		metadata.is_file() && !metadata.file_type().is_symlink(),
		"Anahtar normal dosya olmalı."
	);
	let tmp = path.with_file_name(format!(".key-{}", uuid::Uuid::new_v4()));
	let file = write_private_file(&tmp, bytes)?;
	std::os::unix::fs::fchown(&file, Some(metadata.uid()), Some(metadata.gid()))?;
	file.set_permissions(metadata.permissions())?;
	file.sync_all()?;
	std::fs::rename(tmp, path)?;
	std::fs::File::open(path.parent().context("Anahtar dizini geçersiz.")?)?.sync_all()?;
	Ok(())
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn encrypted_secrets_are_bound_to_their_owner() {
		let v = Vault::new(&[1; 32]).unwrap();
		let e = v.seal(b"private", "tenant:server").unwrap();
		assert_eq!(v.open(&e, "tenant:server").unwrap(), b"private");
		assert!(v.open(&e, "other:server").is_err());
		let mut e = e;
		e[48] ^= 1;
		assert!(v.open(&e, "tenant:server").is_err());
		assert!(v.open(&[], "tenant:server").is_err());
	}
	#[test]
	fn maintenance_lock_rejects_links_and_serializes_writers() {
		let dir = std::env::temp_dir().join(format!("hoodmin-lock-{}", uuid::Uuid::new_v4()));
		std::fs::create_dir(&dir).unwrap();
		let key = dir.join("master.key");
		write_private(&key, &[1; 32]).unwrap();
		let held = maintenance_lock(&key).unwrap();
		let second = std::fs::File::open(key.with_extension("maintenance.lock")).unwrap();
		assert!(second.try_lock().is_err());
		drop(held);
		drop(second);
		std::fs::remove_file(key.with_extension("maintenance.lock")).unwrap();
		std::os::unix::fs::symlink(&key, key.with_extension("maintenance.lock")).unwrap();
		assert!(maintenance_lock(&key).is_err());
		assert_eq!(std::fs::read(&key).unwrap(), vec![1; 32]);
		std::fs::remove_dir_all(dir).unwrap();
	}
	#[test]
	fn raw_legacy_ciphertext_remains_readable() {
		let key = [7; 32];
		let mut collision = [3; 12];
		collision[..4].copy_from_slice(MAGIC);
		for nonce in [[3; 12], collision] {
			let encrypted = Aes256Gcm::new_from_slice(&key)
				.unwrap()
				.encrypt(
					Nonce::from_slice(&nonce),
					Payload {
						msg: b"legacy",
						aad: b"owner",
					},
				)
				.unwrap();
			let raw = [nonce.as_slice(), encrypted.as_slice()].concat();
			assert_eq!(
				Vault::new(&key).unwrap().open(&raw, "owner").unwrap(),
				b"legacy"
			);
		}
	}
	#[test]
	fn dual_ring_recovers_pre_commit_and_post_commit_states() {
		let old = Vault::new(&[1; 32]).unwrap();
		let ring = old.add([2; 32]).unwrap();
		let next = Vault::new(&[2; 32]).unwrap();
		let encoded = Vault::from_bytes(&ring.encode().unwrap()).unwrap();
		let current = encoded.select(next.id()).unwrap();
		assert_eq!(
			current.open(&old.seal(b"old", "a").unwrap(), "a").unwrap(),
			b"old"
		);
		assert_eq!(
			current.open(&next.seal(b"new", "a").unwrap(), "a").unwrap(),
			b"new"
		);
		assert!(old.open(&next.seal(b"new", "a").unwrap(), "a").is_err());
	}
}
