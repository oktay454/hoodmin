use crate::{
	auth::nonce,
	crypto::{Vault, random},
	policy::AccessPolicy,
};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use openssl::{
	asn1::Asn1Time,
	bn::{BigNum, MsbOption},
	hash::MessageDigest,
	pkcs12::Pkcs12,
	pkey::PKey,
	rsa::Rsa,
	x509::{
		X509, X509NameBuilder,
		extension::{BasicConstraints, KeyUsage, SubjectAlternativeName},
	},
};
use serde_json::{Value, json};
use sqlx_core::{query::query, query_scalar::query_scalar, row::Row, transaction::Transaction};
use sqlx_postgres::Postgres;
use uuid::Uuid;
use vpnhood_protocol::AccessToken;

pub fn valid_name(value: &str) -> Result<String> {
	let value = value.trim();
	ensure!(
		!value.is_empty() && value.len() <= 120,
		"Ad 1–120 bayt olmalı."
	);
	Ok(value.into())
}
pub fn normalize_host(host: &str) -> Result<String> {
	let host = host.trim();
	let ip = host.parse::<std::net::IpAddr>().ok();
	ensure!(
		host.len() <= 253
			&& !host.is_empty()
			&& (ip.is_some()
				|| host.split('.').all(|part| !part.is_empty()
					&& part.len() <= 63
					&& !part.starts_with('-')
					&& !part.ends_with('-')
					&& part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))),
		"DNS/IP adresi geçersiz."
	);
	let parts = host.split('.').collect::<Vec<_>>();
	ensure!(
		ip.is_some()
			|| parts.len() != 4
			|| !parts
				.iter()
				.all(|part| part.bytes().all(|b| b.is_ascii_digit())),
		"IPv4 adresi geçersiz."
	);
	Ok(ip
		.map(|ip| ip.to_string())
		.unwrap_or_else(|| host.to_ascii_lowercase()))
}
pub fn endpoint(config: &Value) -> Result<(String, u16)> {
	let token = &config["publicServerToken"];
	// Sanal TLS adı ile istemcinin bağlandığı genel IP farklı olabilir.
	if token["isv"].as_bool() == Some(false)
		&& let Some(address) = token["ep"]
			.as_array()
			.and_then(|values| values.first())
			.and_then(Value::as_str)
			.and_then(|value| value.parse::<std::net::SocketAddr>().ok())
	{
		return Ok((address.ip().to_string(), address.port()));
	}
	let host = token["hname"]
		.as_str()
		.ok_or_else(|| anyhow::anyhow!("Sunucu adresi eksik."))?;
	let port = token["hport"]
		.as_u64()
		.and_then(|v| u16::try_from(v).ok())
		.filter(|v| *v > 0)
		.ok_or_else(|| anyhow::anyhow!("Sunucu portu eksik."))?;
	Ok((host.into(), port))
}
pub fn update_endpoint(config: &mut Value, host: &str, port: u16) -> Result<bool> {
	let host = normalize_host(host)?;
	ensure!(port > 0, "Port geçersiz.");
	let (old_host, old_port) = endpoint(config)?;
	if host == old_host && port == old_port {
		return Ok(false);
	}
	let listeners = if port != old_port {
		let current = config["tcpEndPoints"]
			.as_array()
			.ok_or_else(|| anyhow::anyhow!("TCP dinleyicileri eksik."))?;
		Some(
			current
				.iter()
				.map(|value| -> Result<Value> {
					let mut listener: std::net::SocketAddr = value
						.as_str()
						.ok_or_else(|| anyhow::anyhow!("TCP dinleyicisi geçersiz."))?
						.parse()?;
					if listener.port() == old_port {
						listener.set_port(port);
					}
					Ok(listener.to_string().into())
				})
				.collect::<Result<Vec<_>>>()?,
		)
	} else {
		None
	};
	let ip = host.parse::<std::net::IpAddr>().ok();
	let token = &mut config["publicServerToken"];
	token["hname"] = host.into();
	token["hport"] = port.into();
	token["isv"] = ip.is_none().into();
	token["ep"] = json!(
		ip.map(|ip| std::net::SocketAddr::new(ip, port).to_string())
			.into_iter()
			.collect::<Vec<_>>()
	);
	token["ep_st"] = json!(if ip.is_some() { 4 } else { 3 });
	token["ct"] = json!(Utc::now());
	if let Some(listeners) = listeners {
		config["tcpEndPoints"] = listeners.into();
	}
	Ok(true)
}
pub fn make_config(host: &str, port: u16) -> Result<Value> {
	let host = normalize_host(host)?;
	let host = host.as_str();
	ensure!(port > 0, "Port geçersiz.");
	let ip = host.parse::<std::net::IpAddr>().ok();
	let key = PKey::from_rsa(Rsa::generate(3072)?)?;
	let mut name = X509NameBuilder::new()?;
	name.append_entry_by_text("CN", host)?;
	let name = name.build();
	let mut certificate = X509::builder()?;
	certificate.set_version(2)?;
	let mut serial = BigNum::new()?;
	serial.rand(128, MsbOption::MAYBE_ZERO, false)?;
	let serial = serial.to_asn1_integer()?;
	certificate.set_serial_number(&serial)?;
	certificate.set_subject_name(&name)?;
	certificate.set_issuer_name(&name)?;
	certificate.set_pubkey(&key)?;
	let not_before = Asn1Time::days_from_now(0)?;
	certificate.set_not_before(&not_before)?;
	let not_after = Asn1Time::days_from_now(365)?;
	certificate.set_not_after(&not_after)?;
	certificate.append_extension(BasicConstraints::new().build()?)?;
	certificate.append_extension(
		KeyUsage::new()
			.digital_signature()
			.key_encipherment()
			.build()?,
	)?;
	let mut san = SubjectAlternativeName::new();
	if ip.is_some() {
		san.ip(host);
	} else {
		san.dns(host);
	}
	let extension = san.build(&certificate.x509v3_context(None, None))?;
	certificate.append_extension(extension)?;
	certificate.sign(&key, MessageDigest::sha256())?;
	let certificate = certificate.build();
	let pfx = Pkcs12::builder()
		.name(host)
		.pkey(&key)
		.cert(&certificate)
		.build2("")?
		.to_der()?;
	let secret = random::<16>();
	// SHA-1 yalnızca VpnHood'un mevcut sertifika parmak izi biçimidir; imza SHA-256'dır.
	let hash = certificate.digest(MessageDigest::sha1())?;
	let endpoint = ip
		.map(|ip| std::net::SocketAddr::new(ip, port).to_string())
		.into_iter()
		.collect::<Vec<_>>();
	Ok(
		json!({"serverSecret":STANDARD.encode(secret),"tcpEndPoints":[format!("0.0.0.0:{port}"),format!("[::]:{port}")],"udpEndPoints":[],"quicEndPoints":[],"updateStatusInterval":"00:00:15","logAnonymizer":true,"tcpCongestionControl":null,"certificates":[{"commonName":host,"rawData":STANDARD.encode(pfx)}],"publicServerToken":{"ct":Utc::now(),"hname":host,"hport":port,"isv":ip.is_none(),"sec":STANDARD.encode(secret),"ch":STANDARD.encode(hash),"ep":endpoint,"ep_st":if ip.is_some(){4}else{3}}}),
	)
}
pub async fn register(
	tx: &mut Transaction<'_, Postgres>,
	vault: &Vault,
	tenant: Uuid,
	name: &str,
	farm: Option<Uuid>,
	mut config: Value,
) -> Result<(Uuid, String)> {
	use sha2::{Digest, Sha256};
	if let Some(shared) = crate::farms::load_certificates(tx, vault, tenant, farm).await? {
		crate::farms::apply_certificates(&mut config, &shared)?;
	}
	let id = Uuid::new_v4();
	let code = Uuid::new_v4().to_string();
	config["configCode"] = code.clone().into();
	let credential = format!("{tenant}.{id}.{}", nonce());
	let hash = Sha256::digest(credential.as_bytes()).to_vec();
	let encrypted = vault.seal(
		&serde_json::to_vec(&config)?,
		&format!("{tenant}:{id}:config"),
	)?;
	query("INSERT INTO servers(tenant_id,id,name,farm_id,credential_hash,configuration,config_code,agent_credential) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
		.bind(tenant).bind(id).bind(name).bind(farm).bind(hash).bind(encrypted).bind(code).bind(vault.seal(credential.as_bytes(),&format!("{tenant}:{id}:agent"))?).execute(&mut **tx).await?;
	Ok((id, credential))
}
#[allow(clippy::too_many_arguments)]
pub async fn create_tokens(
	tx: &mut Transaction<'_, Postgres>,
	vault: &Vault,
	tenant: Uuid,
	server: Uuid,
	name: &str,
	count: usize,
	max_clients: i32,
	max_traffic: i64,
	expires: Option<DateTime<Utc>>,
	profile: Option<(Uuid, AccessPolicy)>,
) -> Result<Vec<(Uuid, String, String)>> {
	ensure!(
		(1..=100).contains(&count) && (0..=10000).contains(&max_clients) && max_traffic >= 0,
		"Kota veya adet geçersiz."
	);
	let config: Vec<u8> =
		query_scalar("SELECT configuration FROM servers WHERE tenant_id=$1 AND id=$2")
			.bind(tenant)
			.bind(server)
			.fetch_one(&mut **tx)
			.await?;
	let config: Value =
		serde_json::from_slice(&vault.open(&config, &format!("{tenant}:{server}:config"))?)?;
	let policy = profile
		.as_ref()
		.map(|(_, p)| p.clone())
		.unwrap_or_else(|| AccessPolicy {
			max_clients,
			max_total_traffic: max_traffic,
			..Default::default()
		});
	policy.validate()?;
	let server_farm: Option<Uuid> =
		query_scalar("SELECT farm_id FROM servers WHERE tenant_id=$1 AND id=$2")
			.bind(tenant)
			.bind(server)
			.fetch_one(&mut **tx)
			.await?;
	let token_farm = crate::farms::token_farm(&policy, server_farm)?;
	let advertised =
		crate::farms::advertised_token(tx, tenant, server, token_farm, &config).await?;
	let mut keys = Vec::new();
	for index in 0..count {
		let token_name = if count > 1 {
			format!("{name} {:03}", index + 1)
		} else {
			name.into()
		};
		let id = Uuid::new_v4();
		let secret = random::<16>().to_vec();
		let encrypted = vault.seal(&secret, &format!("{tenant}:{id}:token"))?;
		query("INSERT INTO access_tokens(tenant_id,id,server_id,name,secret,max_clients,max_traffic,expires_at,policy,profile_id,farm_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
			.bind(tenant).bind(id).bind(server).bind(&token_name).bind(encrypted).bind(policy.max_clients).bind(policy.max_total_traffic).bind(expires).bind(serde_json::to_value(&policy)?).bind(profile.as_ref().map(|(id,_)| *id)).bind(token_farm).execute(&mut **tx).await?;
		let token = AccessToken {
			v: 4,
			is_public: policy.is_public,
			client_policies: policy.client_policies.clone(),
			name: token_name.clone(),
			sid: None,
			tid: id.to_string(),
			iat: Utc::now(),
			sec: secret,
			ser: advertised.clone(),
			tags: vec![],
		};
		keys.push((id, token_name, token.to_access_key()?));
	}
	Ok(keys)
}

pub async fn access_key(
	tx: &mut Transaction<'_, Postgres>,
	vault: &Vault,
	tenant: Uuid,
	id: Uuid,
) -> Result<String> {
	let token = query("SELECT * FROM access_tokens WHERE tenant_id=$1 AND id=$2")
		.bind(tenant)
		.bind(id)
		.fetch_one(&mut **tx)
		.await?;
	let server: Uuid = token.get("server_id");
	let bytes: Vec<u8> =
		query_scalar("SELECT configuration FROM servers WHERE tenant_id=$1 AND id=$2")
			.bind(tenant)
			.bind(server)
			.fetch_one(&mut **tx)
			.await?;
	let config: Value =
		serde_json::from_slice(&vault.open(&bytes, &format!("{tenant}:{server}:config"))?)?;
	let policy: AccessPolicy = serde_json::from_value(token.get("policy"))?;
	let advertised =
		crate::farms::advertised_token(tx, tenant, server, token.get("farm_id"), &config).await?;
	Ok(AccessToken {
		v: 4,
		name: token.get("name"),
		sid: Some(token.get("support_id")),
		tid: id.to_string(),
		iat: token.get("created_at"),
		sec: vault.open(token.get("secret"), &format!("{tenant}:{id}:token"))?,
		ser: advertised,
		tags: vec![],
		is_public: policy.is_public,
		client_policies: policy.client_policies,
	}
	.to_access_key()?)
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn endpoint_change_preserves_identity_and_other_settings() {
		let mut config = make_config("192.0.2.10", 443).unwrap();
		config["tcpEndPoints"]
			.as_array_mut()
			.unwrap()
			.push(json!("127.0.0.1:2443"));
		config["udpEndPoints"] = json!(["0.0.0.0:5555"]);
		config["customPolicy"] = json!({"keep":true});
		let before = config.clone();
		assert!(update_endpoint(&mut config, " VPN.Example.COM ", 8443).unwrap());
		assert_eq!(endpoint(&config).unwrap(), ("vpn.example.com".into(), 8443));
		assert_eq!(
			config["tcpEndPoints"],
			json!(["0.0.0.0:8443", "[::]:8443", "127.0.0.1:2443"])
		);
		for field in [
			"serverSecret",
			"certificates",
			"udpEndPoints",
			"quicEndPoints",
			"customPolicy",
		] {
			assert_eq!(config[field], before[field]);
		}
		for field in ["sec", "ch"] {
			assert_eq!(
				config["publicServerToken"][field],
				before["publicServerToken"][field]
			);
		}
		assert_eq!(config["publicServerToken"]["ep"], json!([]));
		assert!(!update_endpoint(&mut config, "vpn.example.com", 8443).unwrap());
		assert!(update_endpoint(&mut config, "2001:db8::10", 9443).unwrap());
		assert_eq!(
			config["publicServerToken"]["ep"],
			json!(["[2001:db8::10]:9443"])
		);
	}
	#[test]
	fn endpoint_uses_public_ip_when_tls_name_is_virtual() {
		let mut config = json!({"publicServerToken":{"hname":"virtual.invalid","hport":443,"isv":false,"ep":["192.0.2.15:8443"]},"tcpEndPoints":["0.0.0.0:8443"]});
		let before = config.clone();
		assert_eq!(endpoint(&config).unwrap(), ("192.0.2.15".into(), 8443));
		assert!(!update_endpoint(&mut config, "192.0.2.15", 8443).unwrap());
		assert_eq!(config, before);
		config["publicServerToken"]["isv"] = true.into();
		assert_eq!(endpoint(&config).unwrap(), ("virtual.invalid".into(), 443));
	}
	#[test]
	fn invalid_endpoint_cannot_mutate_configuration() {
		let mut config = make_config("192.0.2.10", 443).unwrap();
		let before = config.clone();
		for host in [
			"",
			"https://vpn.example.com",
			"vpn.example.com/path",
			"999.1.2.3",
			"-bad.example",
			"bad..example",
			"bad host",
		] {
			assert!(update_endpoint(&mut config, host, 443).is_err());
			assert_eq!(config, before);
		}
		assert!(update_endpoint(&mut config, "vpn.example.com", 0).is_err());
		assert_eq!(config, before);
	}
}
