use crate::{crypto::random, store::Store};
use anyhow::{Result, ensure};
use argon2::{
	Argon2, PasswordHasher, PasswordVerifier,
	password_hash::{PasswordHash, SaltString},
};
use axum::http::HeaderMap;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row, transaction::Transaction};
use sqlx_postgres::Postgres;
use subtle::ConstantTimeEq;
use uuid::Uuid;

pub const COOKIE: &str = "__Host-vhm_session";
pub const LOGIN_COOKIE: &str = "__Host-vhm_login";
#[derive(Clone)]
pub struct User {
	pub id: Uuid,
	pub tenant: Uuid,
	pub email: String,
	pub tenant_name: String,
	pub role: String,
	pub csrf: String,
	pub bot: bool,
	pub json: bool,
}
impl User {
	pub fn can_write(&self) -> bool {
		self.is_admin() || self.role == "operator"
	}
	pub fn is_admin(&self) -> bool {
		matches!(self.role.as_str(), "admin" | "owner")
	}
	pub fn can_tokens(&self) -> bool {
		self.can_write() || self.role == "token_writer"
	}
	pub fn is_owner(&self) -> bool {
		self.role == "owner"
	}
}
pub fn nonce() -> String {
	URL_SAFE_NO_PAD.encode(random::<32>())
}
pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
	headers
		.get("cookie")?
		.to_str()
		.ok()?
		.split(';')
		.find_map(|part| {
			let (key, value) = part.trim().split_once('=')?;
			(key == name
				&& value.len() == 43
				&& value
					.bytes()
					.all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'))
			.then(|| value.to_owned())
		})
}
pub fn cookie_header(name: &str, token: &str, max_age: i64) -> String {
	format!("{name}={token}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={max_age}")
}
pub fn same_token(left: &str, right: &str) -> bool {
	bool::from(left.as_bytes().ct_eq(right.as_bytes()))
}
pub fn hash_password(password: &str) -> Result<String> {
	ensure!(
		(16..=128).contains(&password.len()),
		"Parola 16–128 bayt olmalı."
	);
	let salt = SaltString::generate(&mut rand::rngs::OsRng);
	Argon2::default()
		.hash_password(password.as_bytes(), &salt)
		.map(|v| v.to_string())
		.map_err(|_| anyhow::anyhow!("Parola özeti oluşturulamadı."))
}
pub async fn verify_password(password: String, hash: String) -> bool {
	tokio::task::spawn_blocking(move || {
		if password.len() > 128 {
			return false;
		}
		PasswordHash::new(&hash).is_ok_and(|hash| {
			Argon2::default()
				.verify_password(password.as_bytes(), &hash)
				.is_ok()
		})
	})
	.await
	.unwrap_or(false)
}
pub fn email(value: &str) -> Result<String> {
	let value = value.trim().to_ascii_lowercase();
	ensure!(
		value.len() <= 254
			&& value.len() >= 3
			&& value.contains('@')
			&& !value.chars().any(char::is_whitespace),
		"E-posta adresi geçersiz."
	);
	Ok(value)
}
pub async fn user(store: &Store, headers: &HeaderMap) -> Result<Option<User>> {
	if headers.contains_key("authorization") {
		return bot_user(store, headers).await;
	}
	let Some(token) = cookie(headers, COOKIE) else {
		return Ok(None);
	};
	let hash = Sha256::digest(token.as_bytes()).to_vec();
	let session = query("SELECT s.tenant_id,s.account_id,s.csrf,a.email FROM web_sessions s JOIN accounts a ON a.id=s.account_id WHERE s.token_hash=$1 AND s.expires_at>now() AND a.active")
		.bind(hash).fetch_optional(&store.pool).await?;
	let Some(session) = session else {
		return Ok(None);
	};
	let tenant: Uuid = session.get("tenant_id");
	let id: Uuid = session.get("account_id");
	let mut tx = store.begin(tenant).await?;
	let membership = query("SELECT m.role,t.name FROM memberships m JOIN tenants t ON t.id=m.tenant_id WHERE m.tenant_id=$1 AND m.account_id=$2")
		.bind(tenant).bind(id).fetch_optional(&mut *tx).await?;
	let Some(membership) = membership else {
		return Ok(None);
	};
	Ok(Some(User {
		id,
		tenant,
		email: session.get("email"),
		tenant_name: membership.get("name"),
		role: membership.get("role"),
		csrf: session.get("csrf"),
		bot: false,
		json: headers
			.get("accept")
			.is_some_and(|v| v == "application/json"),
	}))
}
pub async fn bot_user(store: &Store, headers: &HeaderMap) -> Result<Option<User>> {
	let Some(key) = headers
		.get("authorization")
		.and_then(|h| h.to_str().ok())
		.and_then(|h| h.strip_prefix("Bearer "))
		.filter(|k| k.len() <= 200)
	else {
		return Ok(None);
	};
	let parts = key.split('.').collect::<Vec<_>>();
	if parts.len() != 4 || parts[0] != "hmb" {
		return Ok(None);
	}
	let (Ok(tenant), Ok(id)) = (Uuid::parse_str(parts[1]), Uuid::parse_str(parts[2])) else {
		return Ok(None);
	};
	let mut tx = store.begin(tenant).await?;
	let row=query("SELECT b.credential_hash,a.email,m.role,t.name FROM bots b JOIN accounts a ON a.id=b.id JOIN memberships m ON m.tenant_id=b.tenant_id AND m.account_id=b.id JOIN tenants t ON t.id=b.tenant_id WHERE b.tenant_id=$1 AND b.id=$2 AND NOT b.revoked AND b.expires_at>now() AND a.active").bind(tenant).bind(id).fetch_optional(&mut *tx).await?;
	let Some(row) = row else {
		return Ok(None);
	};
	if !bool::from(
		row.get::<Vec<u8>, _>("credential_hash")
			.ct_eq(Sha256::digest(key.as_bytes()).as_slice()),
	) {
		return Ok(None);
	}
	Ok(Some(User {
		id,
		tenant,
		email: row.get("email"),
		tenant_name: row.get("name"),
		role: row.get("role"),
		csrf: String::new(),
		bot: true,
		json: true,
	}))
}
pub async fn create_account(
	store: &Store,
	tenant: Uuid,
	tenant_name: &str,
	address: &str,
	password: &str,
	role: &str,
) -> Result<Uuid> {
	ensure!(
		["owner", "admin", "operator", "token_writer", "viewer"].contains(&role),
		"Rol geçersiz."
	);
	ensure!(
		!tenant_name.trim().is_empty() && tenant_name.len() <= 120,
		"Kiracı adı geçersiz."
	);
	let address = email(address)?;
	let hash = hash_password(password)?;
	let id = Uuid::new_v4();
	let mut tx = store.begin(tenant).await?;
	query("INSERT INTO tenants(id,name) VALUES($1,$2) ON CONFLICT(id) DO NOTHING")
		.bind(tenant)
		.bind(tenant_name)
		.execute(&mut *tx)
		.await?;
	insert_account(&mut tx, tenant, id, &address, &hash, role).await?;
	tx.commit().await?;
	Ok(id)
}
pub async fn insert_account(
	tx: &mut Transaction<'_, Postgres>,
	tenant: Uuid,
	id: Uuid,
	address: &str,
	hash: &str,
	role: &str,
) -> Result<()> {
	query("INSERT INTO accounts(id,email,password_hash,default_tenant_id) VALUES($1,$2,$3,$4)")
		.bind(id)
		.bind(address)
		.bind(hash)
		.bind(tenant)
		.execute(&mut **tx)
		.await?;
	query("INSERT INTO memberships(tenant_id,account_id,role) VALUES($1,$2,$3)")
		.bind(tenant)
		.bind(id)
		.bind(role)
		.execute(&mut **tx)
		.await?;
	Ok(())
}
pub async fn audit(
	tx: &mut Transaction<'_, Postgres>,
	user: &User,
	action: &str,
	resource: &str,
) -> Result<()> {
	query(
		"INSERT INTO audit_events(tenant_id,actor_id,actor_email,action,resource) VALUES($1,$2,$3,$4,$5)",
	)
	.bind(user.tenant)
	.bind(user.id)
	.bind(&user.email)
	.bind(action)
	.bind(resource)
	.execute(&mut **tx)
	.await?;
	Ok(())
}
#[cfg(test)]
mod tests {
	use super::*;
	#[tokio::test]
	async fn password_checks_and_cookie_flags() {
		let hash = hash_password("a-long-test-password-123").unwrap();
		assert!(hash.starts_with("$argon2id$"));
		assert!(verify_password("a-long-test-password-123".into(), hash.clone()).await);
		assert!(!verify_password("wrong-password".into(), hash).await);
		assert!(hash_password("short").is_err());
		let value = cookie_header(COOKIE, &nonce(), 28800);
		for flag in ["Secure", "HttpOnly", "SameSite=Strict", "Path=/"] {
			assert!(value.contains(flag));
		}
	}
}
