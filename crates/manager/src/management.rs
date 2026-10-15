use crate::{
	auth::nonce,
	crypto::{Vault, random},
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
use sqlx_core::{query::query, query_scalar::query_scalar, transaction::Transaction};
use sqlx_postgres::Postgres;
use uuid::Uuid;
use vpnhood_protocol::{AccessToken, ServerToken};

pub fn valid_name(value: &str) -> Result<String> {
	let value = value.trim();
	ensure!(
		!value.is_empty() && value.len() <= 120,
		"Ad 1–120 bayt olmalı."
	);
	Ok(value.into())
}
pub fn make_config(host: &str, port: u16) -> Result<Value> {
	ensure!(port > 0, "Port geçersiz.");
	let ip = host.parse::<std::net::IpAddr>().ok();
	ensure!(
		host.len() <= 253
			&& !host.is_empty()
			&& (ip.is_some()
				|| host.split('.').all(|p| !p.is_empty()
					&& p.len() <= 63
					&& !p.starts_with('-')
					&& !p.ends_with('-')
					&& p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))),
		"DNS/IP adresi geçersiz."
	);
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
	let id = Uuid::new_v4();
	let code = Uuid::new_v4().to_string();
	config["configCode"] = code.clone().into();
	let credential = format!("{tenant}.{id}.{}", nonce());
	let hash = Sha256::digest(credential.as_bytes()).to_vec();
	let encrypted = vault.seal(
		&serde_json::to_vec(&config)?,
		&format!("{tenant}:{id}:config"),
	)?;
	query("INSERT INTO servers(tenant_id,id,name,farm_id,credential_hash,configuration,config_code) VALUES($1,$2,$3,$4,$5,$6,$7)")
		.bind(tenant).bind(id).bind(name).bind(farm).bind(hash).bind(encrypted).bind(code).execute(&mut **tx).await?;
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
) -> Result<Vec<(Uuid, String, String)>> {
	ensure!(
		(1..=100).contains(&count) && (1..=10000).contains(&max_clients) && max_traffic >= 0,
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
		query("INSERT INTO access_tokens(tenant_id,id,server_id,name,secret,max_clients,max_traffic,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
			.bind(tenant).bind(id).bind(server).bind(&token_name).bind(encrypted).bind(max_clients).bind(max_traffic).bind(expires).execute(&mut **tx).await?;
		let token = AccessToken {
			v: 4,
			name: token_name.clone(),
			sid: None,
			tid: id.to_string(),
			iat: Utc::now(),
			sec: secret,
			ser: serde_json::from_value::<ServerToken>(config["publicServerToken"].clone())?,
			tags: vec![],
		};
		keys.push((id, token_name, token.to_access_key()?));
	}
	Ok(keys)
}
