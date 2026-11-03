use anyhow::{Context, Result, ensure};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Args)]
pub struct Options {
	#[arg(long)]
	origin: Option<String>,
	#[arg(long)]
	session_file: Option<PathBuf>,
	#[arg(long)]
	token_file: Option<PathBuf>,
	#[command(subcommand)]
	command: Action,
}
#[derive(Subcommand)]
enum Action {
	/// İnsan hesabıyla giriş; oturum dosyası 0600 olarak saklanır.
	Login {
		#[arg(long)]
		email: String,
		#[arg(long)]
		password_file: PathBuf,
	},
	/// API yolunu oku; örnek: get /servers veya get /views/audit.
	Get {
		path: String,
		#[arg(long)]
		output: Option<PathBuf>,
	},
	/// JSON dosyasını gönder; mevcut sunucu onay/sürüm kuralları aynen uygulanır.
	Post {
		path: String,
		#[arg(long)]
		data_file: PathBuf,
		#[arg(long)]
		output: Option<PathBuf>,
	},
	/// Oturumu sunucuda ve yerel dosyada kapat.
	Logout,
	/// Oturumun rol ve kiracı bilgisini göster.
	Whoami,
}
#[derive(Serialize, Deserialize)]
struct Session {
	origin: String,
	token: String,
	csrf: String,
}
fn default_session() -> PathBuf {
	std::env::var_os("HOME")
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from("."))
		.join(".config/hoodmin/session.json")
}
fn endpoint(origin: &str, path: &str) -> Result<url::Url> {
	crate::config::validate_origin(origin)?;
	ensure!(
		path.starts_with('/') && !path.starts_with("//") && !path.contains('#'),
		"API yolu / ile başlamalı."
	);
	let url = url::Url::parse(origin)?.join(&format!("/api/manage{path}"))?;
	ensure!(url.path().starts_with("/api/manage/"), "API yolu geçersiz.");
	Ok(url)
}
fn cookie(response: &reqwest::Response, name: &str) -> Option<String> {
	response
		.headers()
		.get_all(reqwest::header::SET_COOKIE)
		.iter()
		.filter_map(|v| v.to_str().ok())
		.find_map(|v| {
			v.split(';')
				.next()?
				.strip_prefix(&format!("{name}="))
				.map(str::to_owned)
		})
}
pub async fn run(options: Options) -> Result<()> {
	let file = options.session_file.unwrap_or_else(default_session);
	let session = if options.token_file.is_none() && file.exists() {
		Some(serde_json::from_str::<Session>(&crate::cli::read_secret(
			&file,
		)?)?)
	} else {
		None
	};
	let origin = options
		.origin
		.as_deref()
		.or_else(|| session.as_ref().map(|s| s.origin.as_str()))
		.context("--origin HTTPS_ADRESI gerekli.")?;
	crate::config::validate_origin(origin)?;
	if let Some(s) = &session {
		ensure!(
			s.origin == origin,
			"Oturum başka bir adrese ait; ayrı --session-file kullanın."
		);
	}
	let client = reqwest::Client::builder()
		.https_only(true)
		.redirect(reqwest::redirect::Policy::none())
		.connect_timeout(std::time::Duration::from_secs(5))
		.timeout(std::time::Duration::from_secs(30))
		.build()?;
	if let Action::Login {
		email,
		password_file,
	} = &options.command
	{
		ensure!(
			options.token_file.is_none(),
			"Girişte bot anahtarı kullanılmaz."
		);
		let response = client.get(endpoint(origin, "/login")?).send().await?;
		ensure!(
			response.status().is_success(),
			"Giriş başlatılamadı: HTTP {}",
			response.status()
		);
		let nonce =
			cookie(&response, crate::auth::LOGIN_COOKIE).context("Giriş çerezi alınamadı.")?;
		let body: Value = response.json().await?;
		let password = crate::cli::read_secret(password_file)?;
		let response = client
			.post(endpoint(origin, "/login")?)
			.header("Origin", origin)
			.header("Accept", "application/json")
			.header("Cookie", format!("{}={nonce}", crate::auth::LOGIN_COOKIE))
			.json(&json!({"email":email,"password":password,"csrf":body["csrf"]}))
			.send()
			.await?;
		ensure!(
			response.status().is_success(),
			"Giriş reddedildi: HTTP {}",
			response.status()
		);
		let token = cookie(&response, crate::auth::COOKIE)
			.filter(|s| !s.is_empty())
			.context("Oturum oluşturulamadı.")?;
		let me: Value = client
			.get(endpoint(origin, "/me")?)
			.header("Accept", "application/json")
			.header("Cookie", format!("{}={token}", crate::auth::COOKIE))
			.send()
			.await?
			.error_for_status()?
			.json()
			.await?;
		let session = Session {
			origin: origin.to_owned(),
			token,
			csrf: me["csrf"].as_str().context("CSRF alınamadı.")?.to_owned(),
		};
		let parent = file.parent().context("Oturum yolu geçersiz.")?;
		if !parent.exists() {
			std::fs::create_dir_all(parent)?;
			use std::os::unix::fs::PermissionsExt;
			std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
		}
		if file.exists() {
			std::fs::remove_file(&file)?;
		}
		crate::crypto::write_private(&file, &serde_json::to_vec(&session)?)?;
		println!("Oturum açıldı; oturum bilgileri özel dosyaya kaydedildi.");
		return Ok(());
	}
	let token = options
		.token_file
		.as_ref()
		.map(|p| bot_token(&crate::cli::read_secret(p)?))
		.transpose()?;
	ensure!(
		token.is_some() || session.is_some(),
		"Önce api login veya --token-file kullanın."
	);
	let (method, path, mut body, output) = match &options.command {
		Action::Get { path, output } => {
			(reqwest::Method::GET, path.as_str(), None, output.as_ref())
		}
		Action::Post {
			path,
			data_file,
			output,
		} => {
			let bytes = std::fs::read(data_file)?;
			ensure!(bytes.len() <= 512 * 1024, "JSON dosyası çok büyük.");
			(
				reqwest::Method::POST,
				path.as_str(),
				Some(serde_json::from_slice::<Value>(&bytes)?),
				output.as_ref(),
			)
		}
		Action::Logout => (reqwest::Method::POST, "/logout", Some(json!({})), None),
		Action::Whoami => (reqwest::Method::GET, "/me", None, None),
		_ => unreachable!(),
	};
	let mut request = client
		.request(method, endpoint(origin, path)?)
		.header("Accept", "application/json");
	if let Some(token) = token {
		request = request.bearer_auth(token);
	} else if let Some(s) = &session {
		request = request.header("Cookie", format!("{}={}", crate::auth::COOKIE, s.token));
		if let Some(body) = body.as_mut() {
			ensure!(body.is_object(), "JSON gövdesi nesne olmalı.");
			body["csrf"] = s.csrf.clone().into();
			request = request.header("Origin", origin);
		}
	}
	if let Some(body) = body {
		request = request.json(&body);
	}
	let response = request.send().await?;
	let status = response.status();
	ensure!(
		status.is_success(),
		"API isteği reddedildi: HTTP {}. Yetki, sürüm, onay ve sunucu günlüğünü kontrol edin.",
		status
	);
	let bytes = response.bytes().await?;
	if matches!(options.command, Action::Logout) {
		if file.exists() {
			std::fs::remove_file(&file)?;
		}
		println!("Oturum kapatıldı.");
	} else if let Some(path) = output {
		crate::crypto::write_private(path, &bytes)?;
		println!("Yanıt özel dosyaya kaydedildi.");
	} else {
		use std::io::Write;
		std::io::stdout().write_all(&bytes)?;
		println!();
	}
	Ok(())
}

fn bot_token(text: &str) -> Result<String> {
	let value = if text.trim_start().starts_with('{') {
		let mut object: Value =
			serde_json::from_str(text).context("Bot dosyası JSON biçimi geçersiz.")?;
		if let Some(inner) = object.get("value").and_then(Value::as_str) {
			object = serde_json::from_str(inner).context("Bot kimliği JSON biçimi geçersiz.")?;
		}
		object
			.get("authorization")
			.and_then(Value::as_str)
			.context("Bot dosyasında authorization yok.")?
			.to_owned()
	} else {
		text.to_owned()
	};
	let token = value.strip_prefix("Bearer ").unwrap_or(&value);
	ensure!(
		token.starts_with("hmb.")
			&& token.len() <= 200
			&& token
				.bytes()
				.all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_'),
		"Bot dosyası hmb anahtarı veya panelin JSON kimlik çıktısı olmalı."
	);
	Ok(token.to_owned())
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn bot_files_accept_panel_output_without_header_injection() {
		assert_eq!(
			bot_token(r#"{"authorization":"Bearer hmb.a.b.c"}"#).unwrap(),
			"hmb.a.b.c"
		);
		assert_eq!(bot_token("hmb.a.b.c").unwrap(), "hmb.a.b.c");
		assert!(bot_token("hmb.a.b.c\r\nHost: evil").is_err());
	}
}
