use aes_gcm::{
	Aes256Gcm, KeyInit, Nonce,
	aead::{Aead, Payload},
};
use anyhow::{Context, Result, bail};
use rand::{RngCore, rngs::OsRng};

#[derive(Clone)]
pub struct Vault(Aes256Gcm);
impl Vault {
	pub fn new(key: &[u8]) -> Result<Self> {
		Ok(Self(Aes256Gcm::new_from_slice(key).map_err(|_| {
			anyhow::anyhow!("Ana anahtar 32 bayt olmalı.")
		})?))
	}
	pub fn seal(&self, data: &[u8], context: &str) -> Result<Vec<u8>> {
		let nonce = random::<12>();
		let encrypted = self
			.0
			.encrypt(
				Nonce::from_slice(&nonce),
				Payload {
					msg: data,
					aad: context.as_bytes(),
				},
			)
			.map_err(|_| anyhow::anyhow!("Şifreleme başarısız."))?;
		Ok([nonce.as_slice(), encrypted.as_slice()].concat())
	}
	pub fn open(&self, data: &[u8], context: &str) -> Result<Vec<u8>> {
		if data.len() < 28 {
			bail!("Şifreli veri geçersiz.");
		}
		self.0
			.decrypt(
				Nonce::from_slice(&data[..12]),
				Payload {
					msg: &data[12..],
					aad: context.as_bytes(),
				},
			)
			.map_err(|_| anyhow::anyhow!("Şifreli veri doğrulanamadı."))
	}
}
pub fn random<const N: usize>() -> [u8; N] {
	let mut bytes = [0; N];
	OsRng.fill_bytes(&mut bytes);
	bytes
}
pub fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
	use std::{io::Write, os::unix::fs::OpenOptionsExt};
	let mut file = std::fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.mode(0o600)
		.open(path)
		.with_context(|| format!("Özel dosya oluşturulamadı: {}", path.display()))?;
	file.write_all(bytes)?;
	file.sync_all()?;
	Ok(())
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn encrypted_secrets_are_bound_to_their_owner() {
		let vault = Vault::new(&[1; 32]).unwrap();
		let encrypted = vault.seal(b"private", "tenant:server").unwrap();
		assert_eq!(vault.open(&encrypted, "tenant:server").unwrap(), b"private");
		assert!(vault.open(&encrypted, "other:server").is_err());
		let mut tampered = encrypted.clone();
		tampered[12] ^= 1;
		assert!(vault.open(&tampered, "tenant:server").is_err());
		assert!(vault.open(&[], "tenant:server").is_err());
	}
}
