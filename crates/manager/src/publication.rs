//! External publishing: encrypted tenant-owned configuration, durable leases, pinned public DNS.
use crate::{crypto, farms, store::Store};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, query_scalar::query_scalar, row::Row};
use std::{net::IpAddr, path::PathBuf, process::Stdio, time::Duration};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Specification {
	pub upload_url: String,
	pub method: String,
	pub secret: String,
	pub headers: String,
	pub form_data: String,
	pub body: String,
	pub content_type: String,
}
impl Specification {
	pub fn validate(&self) -> Result<()> {
		ensure!(
			matches!(self.method.as_str(), "PUT" | "POST" | "PUT_POST"),
			"Yöntem geçersiz."
		);
		ensure!(
			self.secret.len() <= 4096 && !self.secret.chars().any(char::is_control),
			"Yayıncı kimliği geçersiz."
		);
		validate_url(&render_url(&self.upload_url, "validation"))?;
		ensure!(self.upload_url.len() <= 2048, "Yükleme adresi çok uzun.");
		let headers = pairs(&self.headers)?;
		for (name, value) in headers {
			ensure!(
				name.bytes()
					.all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)),
				"Başlık adı geçersiz."
			);
			ensure!(
				!matches!(
					name.to_ascii_lowercase().as_str(),
					"host"
						| "content-length" | "content-type"
						| "connection" | "transfer-encoding"
						| "expect" | "cookie"
						| "te" | "trailer" | "upgrade"
				) && !name.to_ascii_lowercase().starts_with("proxy-")
					&& !name.to_ascii_lowercase().starts_with("x-forwarded-"),
				"Bu başlık yönetilemez."
			);
			ensure!(
				!value.chars().any(char::is_control),
				"Başlık değeri geçersiz."
			);
		}
		ensure!(
			self.form_data.len() <= 16384 && self.body.len() <= 32768,
			"Şablon çok uzun."
		);
		let form = pairs(&self.form_data)?;
		ensure!(
			form.is_empty() || self.body.is_empty(),
			"Body ve Form-Data birlikte kullanılamaz."
		);
		ensure!(
			self.content_type.len() <= 120
				&& self.content_type.contains('/')
				&& self
					.content_type
					.bytes()
					.all(|b| b.is_ascii_graphic() || b == b' '),
			"İçerik türü geçersiz."
		);
		Ok(())
	}
	pub fn payload(&self, token: &str) -> Result<(Vec<u8>, String)> {
		let substitute = |value: &str| {
			value
				.replace("{farm_token}", token)
				.replace("{access_token}", &self.secret)
		};
		let fields = pairs(&self.form_data)?;
		if fields.is_empty() {
			return Ok((
				substitute(&self.body).into_bytes(),
				self.content_type.clone(),
			));
		}
		let boundary = format!("hoodmin-{}", Uuid::new_v4().simple());
		let mut body = String::new();
		for (name, value) in fields {
			ensure!(
				name.bytes()
					.all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
				"Form alanı geçersiz."
			);
			body.push_str(&format!(
				"--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{}\r\n",
				substitute(&value)
			));
		}
		body.push_str(&format!("--{boundary}--\r\n"));
		Ok((
			body.into_bytes(),
			format!("multipart/form-data; boundary={boundary}"),
		))
	}
}
pub fn pairs(text: &str) -> Result<Vec<(String, String)>> {
	ensure!(text.len() <= 16384, "Alanlar çok uzun.");
	let mut rows: Vec<(String, String)> = vec![];
	for line in text.lines().filter(|v| !v.trim().is_empty()) {
		let (name, value) = line
			.split_once(':')
			.ok_or_else(|| anyhow::anyhow!("Alanlar Ad: Değer biçiminde olmalı."))?;
		let name = name.trim();
		ensure!(
			!name.is_empty() && name.len() <= 120 && !name.chars().any(char::is_control),
			"Alan adı geçersiz."
		);
		ensure!(
			!rows.iter().any(|(n, _)| name.eq_ignore_ascii_case(n)),
			"Alan adı tekrarlanamaz."
		);
		rows.push((name.to_owned(), value.trim().to_owned()));
	}
	ensure!(rows.len() <= 32, "En fazla 32 alan kullanılabilir.");
	Ok(rows)
}
fn render_url(template: &str, secret: &str) -> String {
	let encoded = url::form_urlencoded::byte_serialize(secret.as_bytes()).collect::<String>();
	template.replace("{access_token}", &encoded)
}
pub fn validate_url(value: &str) -> Result<url::Url> {
	ensure!(
		value.len() <= 8192 && !value.chars().any(char::is_control),
		"HTTPS adresi geçersiz."
	);
	let url = url::Url::parse(value).map_err(|_| anyhow::anyhow!("HTTPS adresi geçersiz."))?;
	ensure!(
		url.scheme() == "https"
			&& url.host_str().is_some()
			&& url.username().is_empty()
			&& url.password().is_none()
			&& url.fragment().is_none()
			&& url.port() != Some(0),
		"Kimliksiz, parçacıksız HTTPS adresi gerekli."
	);
	if let Some(url::Host::Ipv4(ip)) = url.host() {
		ensure!(public_ip(IpAddr::V4(ip)), "Özel ağ hedefi kullanılamaz.");
	}
	if let Some(url::Host::Ipv6(ip)) = url.host() {
		ensure!(public_ip(IpAddr::V6(ip)), "Özel ağ hedefi kullanılamaz.");
	}
	Ok(url)
}
pub fn public_ip(ip: IpAddr) -> bool {
	match ip {
		IpAddr::V4(ip) => {
			let [a, b, c, _] = ip.octets();
			!(a == 0
				|| a == 10 || a == 127
				|| a >= 224 || (a == 100 && (64..=127).contains(&b))
				|| (a == 169 && b == 254)
				|| (a == 172 && (16..=31).contains(&b))
				|| (a == 192
					&& (b == 168 || (b == 0 && (c == 0 || c == 2)) || (b == 88 && c == 99)))
				|| (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
				|| (a == 203 && b == 0 && c == 113))
		}
		IpAddr::V6(ip) => {
			let s = ip.segments();
			// Only native global unicast; no mapped/transition/metadata/ULA/link-local addresses.
			(s[0] & 0xe000) == 0x2000
				&& !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
				&& s[0] != 0x2002
				&& !(s[0] == 0x3fff && s[1] < 0x1000)
		}
	}
}
struct Workspace(PathBuf);
impl Drop for Workspace {
	fn drop(&mut self) {
		let _ = std::fs::remove_dir_all(&self.0);
	}
}
fn quoted(value: &str) -> String {
	format!(
		"\"{}\"",
		value
			.replace('\\', "\\\\")
			.replace('"', "\\\"")
			.replace('\n', "\\n")
			.replace('\r', "\\r")
			.replace('\t', "\\t")
	)
}
async fn request(
	work: &Workspace,
	value: &str,
	method: &str,
	headers: &[(String, String)],
	body: Option<&[u8]>,
) -> Result<(u16, Vec<u8>)> {
	let url = validate_url(value)?;
	let host = url.host_str().unwrap().trim_matches(['[', ']']);
	let port = url.port_or_known_default().unwrap();
	let addresses = tokio::time::timeout(
		Duration::from_secs(5),
		tokio::net::lookup_host((host, port)),
	)
	.await
	.map_err(|_| anyhow::anyhow!("DNS zaman aşımı."))?
	.map_err(|_| anyhow::anyhow!("DNS çözümlemesi başarısız."))?
	.collect::<Vec<_>>();
	ensure!(
		!addresses.is_empty() && addresses.iter().all(|a| public_ip(a.ip())),
		"Hedef DNS sonucu genel ağda değil."
	);
	let mut config = format!(
		"url = {}\nrequest = {}\n",
		quoted(url.as_str()),
		quoted(method)
	);
	if matches!(url.host(), Some(url::Host::Domain(_))) {
		let ip = addresses[0].ip();
		let ip = match ip {
			IpAddr::V6(v) => format!("[{v}]"),
			_ => ip.to_string(),
		};
		config.push_str(&format!(
			"resolve = {}\n",
			quoted(&format!("{host}:{port}:{ip}"))
		));
	}
	for (name, value) in headers {
		config.push_str(&format!(
			"header = {}\n",
			quoted(&format!("{name}: {value}"))
		));
	}
	if let Some(body) = body {
		ensure!(body.len() <= 262144, "Yayım gövdesi çok büyük.");
		let path = work.0.join("upload");
		tokio::fs::write(&path, body).await?;
		config.push_str(&format!(
			"data-binary = {}\n",
			quoted(&format!("@{}", path.display()))
		));
	}
	let output = work.0.join("response");
	config.push_str(&format!("output = {}\n", quoted(&output.to_string_lossy())));
	let mut child = tokio::process::Command::new("/usr/bin/curl")
		.args([
			"--disable",
			"--globoff",
			"--silent",
			"--proto",
			"=https",
			"--proto-redir",
			"=https",
			"--noproxy",
			"*",
			"--max-redirs",
			"0",
			"--connect-timeout",
			"5",
			"--max-time",
			"20",
			"--max-filesize",
			"262144",
			"--tlsv1.2",
			"--write-out",
			"%{http_code}",
			"--config",
			"-",
		])
		.env_clear()
		.env("PATH", "/usr/bin:/bin")
		.env("LC_ALL", "C")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.kill_on_drop(true)
		.spawn()
		.map_err(|_| anyhow::anyhow!("HTTPS aracı başlatılamadı."))?;
	child
		.stdin
		.take()
		.unwrap()
		.write_all(config.as_bytes())
		.await?;
	let result = child.wait_with_output().await?;
	ensure!(
		result.status.success(),
		"HTTPS bağlantısı, sertifika doğrulaması veya aktarım sınırı başarısız."
	);
	let code = String::from_utf8_lossy(&result.stdout)
		.trim()
		.parse::<u16>()?;
	let data = tokio::fs::read(output).await?;
	ensure!(data.len() <= 262144, "Yanıt çok büyük.");
	Ok((code, data))
}
pub async fn payload(
	store: &Store,
	tenant: Uuid,
	farm: Uuid,
	seed: Uuid,
	prospective_url: Option<&str>,
) -> Result<String> {
	let mut tx = store.begin(tenant).await?;
	let bytes: Vec<u8> = query_scalar(
		"SELECT configuration FROM servers WHERE tenant_id=$1 AND id=$2 AND farm_id=$3",
	)
	.bind(tenant)
	.bind(seed)
	.bind(farm)
	.fetch_optional(&mut *tx)
	.await?
	.ok_or_else(|| anyhow::anyhow!("Başlangıç sunucusu artık grupta değil."))?;
	let original: serde_json::Value = serde_json::from_slice(
		&store
			.vault
			.open(&bytes, &format!("{tenant}:{seed}:config"))?,
	)?;
	let original: vpnhood_protocol::ServerToken =
		serde_json::from_value(original["publicServerToken"].clone())?;
	let target = farms::publication_target(&mut tx, tenant, farm)
		.await?
		.ok_or_else(|| anyhow::anyhow!("Uygun canlı grup sunucusu yok."))?;
	let bytes: Vec<u8> = query_scalar(
		"SELECT configuration FROM servers WHERE tenant_id=$1 AND id=$2 AND farm_id=$3",
	)
	.bind(tenant)
	.bind(target)
	.bind(farm)
	.fetch_one(&mut *tx)
	.await?;
	let config: serde_json::Value = serde_json::from_slice(
		&store
			.vault
			.open(&bytes, &format!("{tenant}:{target}:config"))?,
	)?;
	let mut token = farms::advertised_token(&mut tx, tenant, target, Some(farm), &config).await?;
	if target == seed
		&& let Some(url) = prospective_url
	{
		let urls = token.urls.get_or_insert_with(Vec::new);
		if !urls.iter().any(|u| u == url) {
			urls.push(url.to_owned())
		}
	}
	token.ct = chrono::Utc::now();
	token
		.encrypt_for(&original.sec, &crypto::random::<16>())
		.map_err(anyhow::Error::msg)
}
async fn send(
	store: &Store,
	tenant: Uuid,
	farm: Uuid,
	seed: Uuid,
	id: Uuid,
	publish_url: &str,
	encrypted: &[u8],
) -> Result<()> {
	let spec: Specification = serde_json::from_slice(
		&store
			.vault
			.open(encrypted, &format!("{tenant}:{id}:publisher"))?,
	)?;
	spec.validate()?;
	let token = payload(store, tenant, farm, seed, Some(publish_url)).await?;
	let (body, content_type) = spec.payload(&token)?;
	let mut headers = pairs(&spec.headers)?
		.into_iter()
		.map(|(n, v)| {
			(
				n,
				v.replace("{farm_token}", &token)
					.replace("{access_token}", &spec.secret),
			)
		})
		.collect::<Vec<_>>();
	headers.push(("Content-Type".into(), content_type));
	let path = std::env::temp_dir().join(format!("hoodmin-publish-{}", Uuid::new_v4()));
	use std::os::unix::fs::DirBuilderExt;
	std::fs::DirBuilder::new().mode(0o700).create(&path)?;
	let work = Workspace(path);
	let upload_url = render_url(&spec.upload_url, &spec.secret);
	let method = if spec.method == "POST" { "POST" } else { "PUT" };
	let first = request(&work, &upload_url, method, &headers, Some(&body)).await;
	let code = match first {
		Ok((code, _)) if (200..300).contains(&code) => code,
		_ if spec.method == "PUT_POST" => {
			request(&work, &upload_url, "POST", &headers, Some(&body))
				.await?
				.0
		}
		Ok((code, _)) => anyhow::bail!("Yükleme HTTP {code}."),
		Err(error) => return Err(error),
	};
	ensure!((200..300).contains(&code), "Yükleme HTTP {code}.");
	let (code, published) = request(
		&work,
		publish_url,
		"GET",
		&[("Cache-Control".into(), "no-cache".into())],
		None,
	)
	.await?;
	ensure!(code == 200, "Dosya denetimi HTTP {code}.");
	ensure!(
		Sha256::digest(&published) == Sha256::digest(token.as_bytes()),
		"Yayımlanan dosya henüz beklenen içerikle eşleşmiyor."
	);
	Ok(())
}
async fn execute(store: &Store, tenant: Uuid, id: Uuid) -> Result<()> {
	let mut tx = store.begin(tenant).await?;
	let row=query("UPDATE external_publications SET status='Running',last_attempt=now(),lease_until=now()+interval '120 seconds' WHERE tenant_id=$1 AND id=$2 AND enabled AND next_due<=now() AND (lease_until IS NULL OR lease_until<now()) RETURNING *").bind(tenant).bind(id).fetch_optional(&mut *tx).await?;
	tx.commit().await?;
	let Some(row) = row else { return Ok(()) };
	let version: Uuid = row.get("version");
	let result = tokio::time::timeout(
		Duration::from_secs(90),
		send(
			store,
			tenant,
			row.get("farm_id"),
			row.get("seed_id"),
			id,
			row.get("publish_url"),
			row.get("specification"),
		),
	)
	.await;
	let (ok, message) = match result {
		Ok(Ok(())) => (true, "Dosya doğrulandı.".to_owned()),
		Ok(Err(e)) => (false, e.to_string()),
		Err(_) => (false, "Yayım zaman aşımı.".into()),
	};
	let mut tx = store.begin(tenant).await?;
	query("UPDATE external_publications SET status=$4,message=$5,lease_until=NULL,last_success=CASE WHEN $6 THEN now() ELSE last_success END,next_due=CASE WHEN auto_minutes>0 THEN now()+auto_minutes*interval '1 minute' ELSE NULL END WHERE tenant_id=$1 AND id=$2 AND version=$3").bind(tenant).bind(id).bind(version).bind(if ok {"Succeeded"} else {"Failed"}).bind(message.chars().take(200).collect::<String>()).bind(ok).execute(&mut *tx).await?;
	tx.commit().await?;
	Ok(())
}
pub async fn worker(store: Store) {
	let mut tick = tokio::time::interval(Duration::from_secs(2));
	loop {
		tick.tick().await;
		let Ok(rows) = query("SELECT tenant_id,id FROM hoodmin_due_publications()")
			.fetch_all(&store.pool)
			.await
		else {
			continue;
		};
		for row in rows {
			if execute(&store, row.get("tenant_id"), row.get("id"))
				.await
				.is_err()
			{
				tracing::warn!("Yayım görevinin veri tabanı işlemi tamamlanamadı.");
			}
		}
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn ssrf_addresses_and_schemes_are_rejected() {
		for ip in [
			"127.0.0.1",
			"10.1.2.3",
			"169.254.169.254",
			"100.64.0.1",
			"192.168.1.1",
			"198.18.0.1",
			"::1",
			"::ffff:8.8.8.8",
			"2002:808:808::1",
			"2001:db8::1",
			"fe80::1",
			"fc00::1",
		] {
			assert!(!public_ip(ip.parse().unwrap()), "{ip}");
		}
		assert!(public_ip(std::net::Ipv4Addr::new(8, 8, 8, 8).into()));
		assert!(public_ip("2606:4700:4700::1111".parse().unwrap()));
		for u in [
			"http://example.com",
			"https://user:secret@example.com",
			"https://127.1",
			"https://[::1]/",
			"https://example.com/#fragment",
			"file:///etc/passwd",
		] {
			assert!(validate_url(u).is_err(), "{u}");
		}
	}
	#[test]
	fn templates_preserve_raw_cipher_and_boundaries() {
		let mut s = Specification {
			upload_url: "https://example.com/?key={access_token}".into(),
			method: "PUT_POST".into(),
			secret: "a/b?c".into(),
			headers: "Authorization: Bearer {access_token}".into(),
			form_data: String::new(),
			body: "{farm_token}".into(),
			content_type: "text/plain".into(),
		};
		s.validate().unwrap();
		assert_eq!(s.payload("IV.CIPHER").unwrap().0, b"IV.CIPHER");
		assert!(render_url(&s.upload_url, &s.secret).ends_with("a%2Fb%3Fc"));
		s.form_data = "payload: {farm_token}\nkey: {access_token}".into();
		assert!(s.validate().is_err());
		s.body.clear();
		s.validate().unwrap();
		let (body, ct) = s.payload("IV.CIPHER").unwrap();
		assert!(ct.starts_with("multipart/form-data; boundary="));
		assert!(
			String::from_utf8(body)
				.unwrap()
				.contains("\r\nIV.CIPHER\r\n")
		);
		s.headers = "Host: internal".into();
		assert!(s.validate().is_err());
		s.headers = "Authorization: one\nAuthorization: two".into();
		assert!(s.validate().is_err());
	}
}

#[derive(Serialize)]
pub struct Destination {
	pub id: Uuid,
	pub farm: Uuid,
	pub seed: Uuid,
	pub seed_name: String,
	pub version: Uuid,
	pub name: String,
	pub publish_url: String,
	pub enabled: bool,
	pub auto_minutes: i32,
	pub status: String,
	pub message: String,
	pub last_attempt: Option<chrono::DateTime<chrono::Utc>>,
	pub last_success: Option<chrono::DateTime<chrono::Utc>>,
}
pub fn overview(row: &sqlx_postgres::PgRow) -> Destination {
	Destination {
		id: row.get("id"),
		farm: row.get("farm_id"),
		seed: row.get("seed_id"),
		seed_name: row.get("seed_name"),
		version: row.get("version"),
		name: row.get("name"),
		publish_url: row.get("publish_url"),
		enabled: row.get("enabled"),
		auto_minutes: row.get("auto_minutes"),
		status: row.get("status"),
		message: row.get("message"),
		last_attempt: row.get("last_attempt"),
		last_success: row.get("last_success"),
	}
}

pub async fn notify_webhook(url: &str, bearer: &str, body: &[u8]) -> Result<()> {
	let path = std::env::temp_dir().join(format!("hoodmin-notify-{}", Uuid::new_v4()));
	use std::os::unix::fs::DirBuilderExt;
	std::fs::DirBuilder::new().mode(0o700).create(&path)?;
	let work = Workspace(path);
	let mut headers = vec![("Content-Type".into(), "application/json".into())];
	if !bearer.is_empty() {
		headers.push(("Authorization".into(), format!("Bearer {bearer}")));
	}
	let (code, _) = request(&work, url, "POST", &headers, Some(body)).await?;
	ensure!(
		(200..300).contains(&code),
		"Bildirim hedefi isteği kabul etmedi."
	);
	Ok(())
}
#[allow(clippy::too_many_arguments)]
pub async fn notify_smtp(
	host: &str,
	port: u16,
	username: &str,
	password: &str,
	from: &str,
	to: &str,
	event: &serde_json::Value,
) -> Result<()> {
	let addresses = tokio::time::timeout(
		Duration::from_secs(5),
		tokio::net::lookup_host((host, port)),
	)
	.await??
	.collect::<Vec<_>>();
	ensure!(
		!addresses.is_empty() && addresses.iter().all(|a| public_ip(a.ip())),
		"SMTP hedefi genel ağda değil."
	);
	let ip = match addresses[0].ip() {
		IpAddr::V6(v) => format!("[{v}]"),
		v => v.to_string(),
	};
	let path = std::env::temp_dir().join(format!("hoodmin-mail-{}", Uuid::new_v4()));
	use std::os::unix::fs::DirBuilderExt;
	std::fs::DirBuilder::new().mode(0o700).create(&path)?;
	let work = Workspace(path);
	let message = format!(
		"From: {from}\r\nTo: {to}\r\nDate: {}\r\nMessage-ID: <{}@hoodmin>\r\nSubject: Hoodmin VPN notification\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n",
		chrono::Utc::now().to_rfc2822(),
		Uuid::new_v4(),
		{
			use base64::Engine;
			let encoded =
				base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec_pretty(event)?);
			encoded
				.as_bytes()
				.chunks(76)
				.map(|c| std::str::from_utf8(c).unwrap())
				.collect::<Vec<_>>()
				.join("\r\n")
		}
	);
	let file = work.0.join("message");
	crate::crypto::write_private(&file, message.as_bytes())?;
	let config = format!(
		"url = {}\nresolve = {}\nuser = {}\nmail-from = {}\nmail-rcpt = {}\nupload-file = {}\n",
		quoted(&format!("smtps://{host}:{port}")),
		quoted(&format!("{host}:{port}:{ip}")),
		quoted(&format!("{username}:{password}")),
		quoted(from),
		quoted(to),
		quoted(&file.to_string_lossy())
	);
	let mut child = tokio::process::Command::new("/usr/bin/curl")
		.args([
			"--disable",
			"--silent",
			"--proto",
			"=smtps",
			"--noproxy",
			"*",
			"--ssl-reqd",
			"--tlsv1.2",
			"--connect-timeout",
			"5",
			"--max-time",
			"20",
			"--config",
			"-",
		])
		.env_clear()
		.env("PATH", "/usr/bin:/bin")
		.stdin(Stdio::piped())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.kill_on_drop(true)
		.spawn()?;
	child
		.stdin
		.take()
		.unwrap()
		.write_all(config.as_bytes())
		.await?;
	ensure!(child.wait().await?.success(), "SMTP aktarımı başarısız.");
	Ok(())
}
