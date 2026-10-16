use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use openssl::{asn1::Asn1Time, hash::MessageDigest, pkcs12::Pkcs12};
use serde_json::{Value, json};

/// Validate and normalize an uploaded PFX. Stored PFX has no password because .NET CertificateData has no password field.
pub fn import_pfx(encoded: &str, password: &str, tls_name: &str) -> Result<(Value, String)> {
	ensure!(
		encoded.len() <= 350_000 && password.len() <= 1024,
		"PFX boyutu geçersiz"
	);
	let raw = STANDARD.decode(encoded)?;
	ensure!(raw.len() <= 256 * 1024, "PFX en fazla 256 KiB olabilir");
	let parsed = Pkcs12::from_der(&raw)?.parse2(password)?;
	let key = parsed
		.pkey
		.ok_or_else(|| anyhow::anyhow!("PFX özel anahtarı eksik"))?;
	let cert = parsed
		.cert
		.ok_or_else(|| anyhow::anyhow!("PFX sertifikası eksik"))?;
	let public = cert.public_key()?;
	ensure!(
		key.public_eq(&public),
		"Sertifika ve özel anahtar eşleşmiyor"
	);
	let now = Asn1Time::days_from_now(0)?;
	ensure!(
		cert.not_after() > now.as_ref() && cert.not_before() <= now.as_ref(),
		"Sertifikanın geçerlilik zamanı uygun değil"
	);
	let mut builder = Pkcs12::builder();
	builder.name(tls_name).pkey(&key).cert(&cert);
	if let Some(chain) = parsed.ca {
		builder.ca(chain);
	}
	let pfx = builder.build2("")?.to_der()?;
	let hash = STANDARD.encode(cert.digest(MessageDigest::sha1())?);
	Ok((
		json!({"commonName":tls_name,"rawData":STANDARD.encode(pfx)}),
		hash,
	))
}
pub fn describe(config: &Value) -> Vec<(String, String, String)> {
	config["certificates"]
		.as_array()
		.into_iter()
		.flatten()
		.filter_map(|value| {
			let raw = STANDARD.decode(value["rawData"].as_str()?).ok()?;
			let parsed = Pkcs12::from_der(&raw).ok()?.parse2("").ok()?;
			let cert = parsed.cert?;
			Some((
				value["commonName"].as_str()?.to_owned(),
				cert.not_after().to_string(),
				cert.digest(MessageDigest::sha256())
					.ok()?
					.iter()
					.map(|v| format!("{v:02X}"))
					.collect::<Vec<_>>()
					.join(":"),
			))
		})
		.collect()
}
