//! VpnHood 9.0.856 erişim yöneticisi (access manager) tel sözleşmesi.
use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::NoPadding};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// .NET byte[] alanları JSON içinde sayı dizisi değil Base64 metnidir.
pub mod bytes {
	use super::*;
	pub fn serialize<S: serde::Serializer>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_str(&STANDARD.encode(value))
	}
	pub fn deserialize<'de, D: serde::Deserializer<'de>>(
		deserializer: D,
	) -> Result<Vec<u8>, D::Error> {
		let text = String::deserialize(deserializer)?;
		STANDARD.decode(text).map_err(serde::de::Error::custom)
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Traffic {
	pub sent: i64,
	pub received: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum SessionErrorCode {
	#[default]
	Ok,
	AccessError,
	PlanRejected,
	GeneralError,
	SessionClosed,
	SessionSuppressedBy,
	SessionError,
	SessionExpired,
	AccessExpired,
	AccessCodeRejected,
	AccessLocked,
	AccessTrafficOverflow,
	DailyLimitExceeded,
	NoServerAvailable,
	PremiumLocation,
	AdError,
	RewardedAdRejected,
	Maintenance,
	RedirectHost,
	UnsupportedClient,
	UnsupportedServer,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
	pub client_id: String,
	pub client_version: String,
	pub user_agent: String,
	#[serde(default)]
	pub min_protocol_version: i32,
	#[serde(default)]
	pub max_protocol_version: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRequest {
	pub token_id: String,
	pub client_info: ClientInfo,
	#[serde(with = "bytes")]
	pub encrypted_client_id: Vec<u8>,
	pub host_end_point: String,
	#[serde(default)]
	pub protocol_version: i32,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionUsage {
	pub session_id: u64,
	#[serde(default)]
	pub sent: i64,
	#[serde(default)]
	pub received: i64,
	#[serde(default)]
	pub error_code: SessionErrorCode,
	#[serde(default)]
	pub closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessUsage {
	pub is_premium: bool,
	pub cycle_traffic: Traffic,
	pub total_traffic: Traffic,
	pub max_traffic: i64,
	pub expiration_time: Option<DateTime<Utc>>,
	pub active_client_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
	pub error_code: SessionErrorCode,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error_message: Option<String>,
	pub session_id: u64,
	#[serde(with = "bytes")]
	pub session_key: Vec<u8>,
	pub protocol_version: i32,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub created_time: Option<DateTime<Utc>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub access_usage: Option<AccessUsage>,
	pub server_tags: Vec<String>,
}
impl SessionResponse {
	pub fn error(code: SessionErrorCode) -> Self {
		Self {
			error_code: code,
			error_message: None,
			session_id: 0,
			session_key: vec![],
			protocol_version: 0,
			created_time: None,
			access_usage: None,
			server_tags: vec![],
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCommand {
	pub config_code: String,
	pub session_responses: BTreeMap<u64, SessionResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
	#[serde(default)]
	pub session_usages: Vec<SessionUsage>,
	#[serde(flatten)]
	pub details: serde_json::Map<String, serde_json::Value>,
}

/// GUID .NET ToByteArray biçimi ilk üç alanı küçük sonlu (little-endian) saklar.
/// VpnHood 128 bit token sırrı ile AES-CBC / sıfır IV / dolgusuz şifreleme kullanır.
pub fn encrypted_client_id(client_id: &str, secret: &[u8]) -> Option<Vec<u8>> {
	if secret.len() != 16 {
		return None;
	}
	let input = match Uuid::parse_str(client_id) {
		Ok(id) => id.to_bytes_le().to_vec(),
		Err(_) => client_id.as_bytes().to_vec(),
	};
	if input.is_empty() || input.len() % 16 != 0 {
		return None;
	}
	let cipher = cbc::Encryptor::<aes::Aes128>::new_from_slices(secret, &[0; 16]).ok()?;
	Some(cipher.encrypt_padded_vec_mut::<NoPadding>(&input))
}
pub fn validate_client_id(client_id: &str, secret: &[u8], proof: &[u8]) -> bool {
	encrypted_client_id(client_id, secret).is_some_and(|expected| bool::from(expected.ct_eq(proof)))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessToken {
	pub v: u8,
	pub name: String,
	pub sid: Option<String>,
	pub tid: String,
	pub iat: DateTime<Utc>,
	#[serde(with = "bytes")]
	pub sec: Vec<u8>,
	pub ser: ServerToken,
	pub tags: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerToken {
	pub ct: DateTime<Utc>,
	pub hname: String,
	pub hport: u16,
	pub isv: bool,
	#[serde(with = "bytes")]
	pub sec: Vec<u8>,
	#[serde(with = "bytes")]
	pub ch: Vec<u8>,
	pub ep: Vec<String>,
	pub ep_st: u8,
}
impl AccessToken {
	pub fn to_access_key(&self) -> Result<String, serde_json::Error> {
		Ok(format!(
			"vh://{}",
			STANDARD.encode(serde_json::to_vec(self)?)
		))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn rejects_bad_proofs_without_panicking() {
		assert!(!validate_client_id("short", &[1; 16], &[0; 16]));
		assert!(!validate_client_id(
			"00112233-4455-6677-8899-aabbccddeeff",
			&[1; 32],
			&[0; 16]
		));
		let id = "00112233-4455-6677-8899-aabbccddeeff";
		let proof = encrypted_client_id(id, &[1; 16]).unwrap();
		assert!(validate_client_id(id, &[1; 16], &proof));
		assert!(!validate_client_id(id, &[2; 16], &proof));
	}
	#[test]
	fn dotnet_json_representation_is_preserved() {
		let response = SessionResponse {
			session_key: vec![0, 1, 2, 255],
			..SessionResponse::error(SessionErrorCode::Ok)
		};
		let value = serde_json::to_value(response).unwrap();
		assert_eq!(value["sessionKey"], "AAEC/w==");
		assert_eq!(value["errorCode"], "Ok");
		let usage: SessionUsage =
			serde_json::from_str(r#"{"sessionId":18446744073709551615,"sent":7}"#).unwrap();
		assert_eq!(usage.session_id, u64::MAX);
		assert_eq!(usage.error_code, SessionErrorCode::Ok);
	}
}
