//! VpnHood 9.0.856 erişim yöneticisi (access manager) tel sözleşmesi.
pub mod config;
use aes::cipher::{
	BlockEncryptMut, KeyIvInit,
	block_padding::{NoPadding, Pkcs7},
};
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
	#[serde(default)]
	pub client_country: Option<String>,
	#[serde(default)]
	pub extra_data: Option<String>,
	#[serde(default)]
	pub server_location: Option<String>,
	#[serde(default)]
	pub plan_id: ConnectPlanId,
	#[serde(default = "yes")]
	pub allow_redirect: bool,
	#[serde(default)]
	pub is_ip_v6_supported: Option<bool>,
	#[serde(default)]
	pub access_code: Option<String>,
	#[serde(default)]
	pub user_review: Option<serde_json::Value>,
	#[serde(default)]
	pub allow_anonymous_tracker: bool,
	#[serde(default)]
	pub remote_end_point: Option<String>,
	#[serde(default)]
	pub client_ip: Option<std::net::IpAddr>,
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
	#[serde(default)]
	pub ad_data: Option<String>,
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
	#[serde(default)]
	pub can_extend_by_rewarded_ad: bool,
	#[serde(default)]
	pub is_user_review_recommended: bool,
	#[serde(default)]
	pub user_review_recommended: i32,
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
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub extra_data: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub ga_measurement_id: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub server_location: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub access_info: Option<AccessInfo>,
	#[serde(default)]
	pub ad_requirement: AdRequirement,
	#[serde(default)]
	pub suppressed_to: SessionSuppressType,
	#[serde(default)]
	pub suppressed_by: SessionSuppressType,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub redirect_server_tokens: Option<Vec<ServerToken>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub redirect_host_end_point: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub redirect_host_end_points: Option<Vec<String>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub access_key: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub client_country: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub client_public_address: Option<std::net::IpAddr>,
	#[serde(default)]
	pub revision: i64,
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
			extra_data: None,
			ga_measurement_id: None,
			server_location: None,
			access_info: None,
			ad_requirement: AdRequirement::None,
			suppressed_to: SessionSuppressType::None,
			suppressed_by: SessionSuppressType::None,
			redirect_server_tokens: None,
			redirect_host_end_point: None,
			redirect_host_end_points: None,
			access_key: None,
			client_country: None,
			client_public_address: None,
			revision: 0,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccessToken {
	#[serde(default, rename = "ispub", skip_serializing_if = "is_false")]
	pub is_public: bool,
	#[serde(default, rename = "cpols", skip_serializing_if = "Vec::is_empty")]
	pub client_policies: Vec<serde_json::Value>,
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerToken {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub urls: Option<Vec<String>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub url: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub path: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub loc: Option<Vec<String>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub loc2: Option<Vec<String>>,
	pub ct: DateTime<Utc>,
	pub hname: String,
	pub hport: u16,
	pub isv: bool,
	#[serde(default, with = "nullable_bytes")]
	pub sec: Vec<u8>,
	#[serde(default, with = "nullable_bytes")]
	pub ch: Vec<u8>,
	#[serde(default, deserialize_with = "nullable_vec")]
	pub ep: Vec<String>,
	#[serde(default, deserialize_with = "endpoint_strategy")]
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

pub mod nullable_bytes {
	use super::*;
	pub use bytes::serialize;
	pub fn deserialize<'de, D: serde::Deserializer<'de>>(
		deserializer: D,
	) -> Result<Vec<u8>, D::Error> {
		Option::<String>::deserialize(deserializer)?
			.map(|s| STANDARD.decode(s).map_err(serde::de::Error::custom))
			.unwrap_or(Ok(vec![]))
	}
}
fn nullable_vec<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
	deserializer: D,
) -> Result<Vec<T>, D::Error> {
	Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}
fn endpoint_strategy<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
	let value = serde_json::Value::deserialize(deserializer)?;
	if let Some(n) = value.as_u64().filter(|n| *n <= 4) {
		return Ok(n as u8);
	}
	match value.as_str().map(str::to_ascii_lowercase).as_deref() {
		Some("auto") => Ok(0),
		Some("dnsfirst") => Ok(1),
		Some("ipfirst" | "tokenfirst") => Ok(2),
		Some("dnsonly") => Ok(3),
		Some("iponly" | "tokenonly") => Ok(4),
		_ => Err(serde::de::Error::custom("Invalid endpoint strategy")),
	}
}

fn yes() -> bool {
	true
}
fn is_false(value: &bool) -> bool {
	!*value
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectPlanId {
	#[default]
	Normal,
	NormalByRewardedAd,
	PremiumByTrial,
	PremiumByRewardedAd,
	Status,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum AdRequirement {
	#[default]
	None,
	Flexible,
	Rewarded,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionSuppressType {
	#[default]
	None,
	YourSelf,
	Other,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessInfo {
	pub is_new: bool,
	pub created_time: DateTime<Utc>,
	pub last_used_time: DateTime<Utc>,
	pub expiration_time: Option<DateTime<Utc>>,
	pub is_premium: bool,
	pub max_cycle_traffic: i64,
	pub max_total_traffic: i64,
	pub max_device_count: i32,
	pub max_speed_mbps: Option<Traffic>,
	pub device_life_span: i32,
	pub devices_summary: Option<serde_json::Value>,
}
impl ServerToken {
	/// .NET ServerToken.Encrypt: AES-128-CBC/PKCS7, Base64(IV).Base64(ciphertext).
	pub fn encrypt(&self, iv: &[u8; 16]) -> Result<String, String> {
		self.encrypt_for(&self.sec, iv)
	}
	/// A URL update is encrypted for the previous token, and may contain a different target secret.
	pub fn encrypt_for(&self, key: &[u8], iv: &[u8; 16]) -> Result<String, String> {
		let cipher = cbc::Encryptor::<aes::Aes128>::new_from_slices(key, iv)
			.map_err(|_| "ServerToken secret must be 16 bytes")?;
		let plaintext = serde_json::to_vec(self).map_err(|e| e.to_string())?;
		Ok(format!(
			"{}.{}",
			STANDARD.encode(iv),
			STANDARD.encode(cipher.encrypt_padded_vec_mut::<Pkcs7>(&plaintext))
		))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn nullable_server_token_and_legacy_strategy_are_supported() {
		let value = serde_json::json!({"ct":"2026-10-16T00:00:00Z","hname":"vpn.example.com","hport":443,"isv":true,"sec":null,"ch":null,"ep":null,"ep_st":"TokenOnly"});
		let token: ServerToken = serde_json::from_value(value).unwrap();
		assert!(token.ch.is_empty() && token.ep.is_empty());
		assert_eq!(token.ep_st, 4);
		assert_eq!(serde_json::to_value(token).unwrap()["ep_st"], 4);
		assert!(
			config::validate_client_policies(&[serde_json::json!({"ccs":["*"],"pbtdl":3600})])
				.is_ok()
		);
	}
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
