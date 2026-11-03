use crate::config::{DEFAULT_FILE, Settings};
use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use std::io::Write;
use std::{
	path::{Path, PathBuf},
	process::{Command, Stdio},
};

#[derive(Parser)]
#[command(
	name = "hoodmin-cli",
	version,
	about = "Hoodmin kurulum, bakım ve VPN yönetim aracı"
)]
struct Args {
	#[arg(long,global=true,default_value=DEFAULT_FILE)]
	config: PathBuf,
	#[arg(long, global = true, env = "DATABASE_URL", hide = true)]
	database_url: Option<String>,
	#[arg(long, global = true, env = "VHM_MASTER_KEY_FILE", hide = true)]
	master_key_file: Option<PathBuf>,
	#[command(subcommand)]
	command: Action,
}
#[derive(Subcommand)]
enum Action {
	/// Anahtar halkasını incele veya veritabanını yeni anahtarla yeniden şifrele.
	Keys {
		#[arg(long, hide = true)]
		peer_helper: bool,
		#[command(subcommand)]
		command: crate::rotation::Action,
	},
	/// Sunucu bağlantı ve HTTPS ayarları.
	Config {
		#[command(subcommand)]
		command: ConfigAction,
	},
	/// Yerel veya uzak PostgreSQL hazırlama ve şema geçişleri.
	Database {
		#[command(subcommand)]
		command: crate::cli_database::Action,
	},
	/// İlk sahip hesabını oluştur; parolayı yalnız root dosyasına kaydet.
	OwnerInit {
		#[arg(long)]
		email: String,
	},
	/// HTTPS üzerinden insan hesabı veya bot ile tüm yönetim işlemleri.
	Api {
		#[command(flatten)]
		options: crate::cli_http::Options,
	},
	/// Veritabanı, anahtar ve sertifikaları yedekle.
	Backup {
		#[command(subcommand)]
		command: crate::cli_backup::Action,
	},
	/// Yerel systemd hizmetini yönet.
	Service {
		#[arg(value_parser=["start","stop","restart","status","enable","disable"])]
		action: String,
	},
	#[command(flatten)]
	Local(crate::legacy::Command),
}
#[derive(Subcommand)]
enum ConfigAction {
	/// Statik arayüz sunumunu veritabanından bağımsız yapılandır.
	Web {
		#[arg(long, action = clap::ArgAction::Set, required = true)]
		serve_static: bool,
		#[arg(long)]
		directory: Option<PathBuf>,
	},
	/// Parola ve bağlantı sırlarını göstermeden ayar özetini yazdır.
	Show,
	/// Bağlantı dizesini özel dosyadan al, ayarları doğrula ve kaydet.
	Set {
		#[arg(long)]
		database_url_file: PathBuf,
		#[arg(long)]
		public_origin: String,
		#[arg(long, default_value = "127.0.0.1:3080")]
		listen: std::net::SocketAddr,
		#[arg(long, default_value = "/var/lib/hoodmin/master.key")]
		key_file: PathBuf,
	},
}
pub async fn run() -> Result<()> {
	let args = Args::parse();
	tracing_subscriber::fmt()
		.with_writer(std::io::stderr)
		.with_env_filter("hoodmin=info")
		.init();
	if let Action::Api { options } = args.command {
		return crate::cli_http::run(options).await;
	}
	if let Action::Config { command } = args.command {
		return configure(&args.config, command);
	}
	if let Action::Keys {
		peer_helper: true,
		command,
	} = args.command
	{
		use std::io::Read;
		let mut text = String::new();
		std::io::stdin().take(65536).read_to_string(&mut text)?;
		let settings: Settings = serde_json::from_str(&text)?;
		settings.validate()?;
		return crate::rotation::run(&settings, command, true).await;
	}
	let mut settings = Settings::load(&args.config).unwrap_or_default();
	if let Some(value) = args.database_url {
		settings.database_url = Some(value);
	}
	if let Some(value) = args.master_key_file {
		settings.master_key_file = value;
	}
	let dsn = settings.database_url.clone();
	let result = match args.command {
		Action::Keys { command, .. } => crate::rotation::run(&settings, command, false).await,
		Action::Database { command } => crate::cli_database::run(&settings, command).await,
		Action::OwnerInit { email } => owner_init(&args.config, &settings, &email),
		Action::Backup { command } => crate::cli_backup::run(&args.config, &settings, command),
		Action::Service { action } => {
			if action != "status" {
				root()?;
			}
			let status = Command::new("systemctl")
				.args([&action, "hoodmin.service"])
				.status()?;
			ensure!(status.success(), "systemd işlemi başarısız.");
			Ok(())
		}
		Action::Local(command) => {
			crate::legacy::run(
				command,
				settings.db()?.to_owned(),
				settings.master_key_file.clone(),
			)
			.await
		}
		_ => unreachable!(),
	};
	result.map_err(|e| anyhow::anyhow!(crate::config::redact(&e, dsn.as_deref())))
}
pub fn root() -> Result<()> {
	let output = Command::new("id").arg("-u").output()?;
	ensure!(output.stdout == b"0\n", "Bu işlem root gerektirir.");
	Ok(())
}
pub fn read_secret(path: &Path) -> Result<String> {
	use std::os::unix::fs::PermissionsExt;
	ensure!(
		std::fs::metadata(path)?.permissions().mode() & 0o077 == 0,
		"Sır dosyası 0600 olmalı."
	);
	let text = std::fs::read_to_string(path)?;
	ensure!(text.len() <= 65536, "Sır dosyası çok büyük.");
	Ok(text.trim_end_matches(['\r', '\n']).to_owned())
}
fn configure(path: &Path, command: ConfigAction) -> Result<()> {
	match command {
		ConfigAction::Web {
			serve_static,
			directory,
		} => {
			root()?;
			let mut s = Settings::load(path)?;
			s.web.serve_static = serve_static;
			if let Some(directory) = directory {
				s.web.directory = directory;
			}
			s.validate()?;
			let parent = path.parent().context("Yapılandırma yolu geçersiz.")?;
			let tmp = parent.join(format!(".hoodmin-{}.json", uuid::Uuid::new_v4()));
			crate::crypto::write_private(&tmp, &serde_json::to_vec_pretty(&s)?)?;
			chown(&tmp, "root:hoodmin")?;
			use std::os::unix::fs::PermissionsExt;
			std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o640))?;
			std::fs::rename(tmp, path)?;
			println!("Statik arayüz sunumu kaydedildi. Nginx yapılandırmasını eşleştirin.");
		}
		ConfigAction::Show => {
			let s = Settings::load(path)?;
			let endpoint = s
				.database_url
				.as_deref()
				.map(crate::config::database_options)
				.transpose()?;
			println!(
				"{}",
				serde_json::to_string_pretty(
					&serde_json::json!({"listen":s.listen,"public_origin":s.public_origin,"master_key_file":s.master_key_file,"web":s.web,"database":endpoint.as_ref().map(|o|serde_json::json!({"host":o.get_host(),"port":o.get_port(),"database":o.get_database(),"user":o.get_username(),"socket":o.get_socket(),"tls":format!("{:?}",o.get_ssl_mode())}))})
				)?
			);
		}
		ConfigAction::Set {
			database_url_file,
			public_origin,
			listen,
			key_file,
		} => {
			root()?;
			let s = Settings {
				database_url: Some(read_secret(&database_url_file)?),
				master_key_file: key_file,
				public_origin,
				listen,
				backup: Settings::load(path).unwrap_or_default().backup,
				web: Settings::load(path).unwrap_or_default().web,
			};
			s.validate()?;
			if let Ok(previous) = Settings::load(path) {
				ensure!(
					previous.database_url.is_none()
						|| previous.master_key_file == s.master_key_file,
					"Mevcut kurulumun ana anahtar yolu bu komutla değiştirilemez."
				);
			}
			if !s.master_key_file.exists() {
				crate::crypto::write_private(&s.master_key_file, &crate::crypto::random::<32>())?;
				chown(&s.master_key_file, "hoodmin:hoodmin")?;
			}
			let parent = path.parent().context("Yapılandırma yolu geçersiz.")?;
			std::fs::create_dir_all(parent)?;
			let tmp = parent.join(format!(".hoodmin-{}.json", uuid::Uuid::new_v4()));
			crate::crypto::write_private(&tmp, &serde_json::to_vec_pretty(&s)?)?;
			chown(&tmp, "root:hoodmin")?;
			use std::os::unix::fs::PermissionsExt;
			std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o640))?;
			std::fs::rename(&tmp, path)?;
			println!(
				"Yapılandırma kaydedildi. Veritabanı için hoodmin-cli database init, ilk hesap için hoodmin-cli owner-init kullanın."
			);
		}
	}
	Ok(())
}
pub fn chown(path: &Path, owner: &str) -> Result<()> {
	ensure!(
		Command::new("chown")
			.arg(owner)
			.arg(path)
			.status()?
			.success(),
		"Dosya sahipliği ayarlanamadı."
	);
	Ok(())
}
fn owner_init(config: &Path, settings: &Settings, email: &str) -> Result<()> {
	root()?;
	crate::auth::email(email)?;
	settings.validate()?;
	settings.db()?;
	let output = Path::new("/root/hoodmin-first-owner.json");
	ensure!(
		!output.exists(),
		"İlk sahip dosyası zaten var; mevcut hesap değiştirilmedi."
	);
	let tenant = uuid::Uuid::new_v4();
	let password = crate::auth::nonce();
	let file = settings
		.master_key_file
		.with_file_name(format!(".bootstrap-{}", uuid::Uuid::new_v4()));
	crate::crypto::write_private(&file, password.as_bytes())?;
	chown(&file, "hoodmin:hoodmin")?;
	// Exclusive output reserves the initial bootstrap; credentials never enter process arguments.
	crate::crypto::write_private(
		output,
		&serde_json::to_vec_pretty(
			&serde_json::json!({"email":email,"password":password,"tenant":tenant,"origin":settings.public_origin}),
		)?,
	)?;
	let result = Command::new("runuser")
		.args(["-u", "hoodmin", "--"])
		.arg(std::env::current_exe()?)
		.arg("--config")
		.arg(config)
		.args([
			"create-account",
			"--first",
			"--tenant",
			&tenant.to_string(),
			"--tenant-name",
			"Hoodmin",
			"--email",
			email,
			"--password-file",
		])
		.arg(&file)
		.args(["--role", "owner"])
		.output();
	let _ = std::fs::remove_file(file);
	if !result.as_ref().is_ok_and(|r| r.status.success()) {
		let _ = std::fs::remove_file(output);
		anyhow::bail!(
			"İlk sahip oluşturulamadı; veritabanı bağlantısını ve şema kurulumunu kontrol edin."
		);
	}
	println!("İlk sahip oluşturuldu: /root/hoodmin-first-owner.json (0600).");
	Ok(())
}
/// Run a fixed local helper as postgres; private settings travel only through stdin.
pub fn local_bootstrap(settings: &Settings) -> Result<()> {
	root()?;
	let options = crate::config::database_options(settings.db()?)?;
	ensure!(
		options.get_socket().is_some(),
		"--local yalnız Unix soketi içindir."
	);
	let mut child=Command::new("runuser").args(["-u","postgres","--"]).arg(std::env::current_exe()?)
		.args(["database","init","--internal-local"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().context("Yerel PostgreSQL kullanıcısı bulunamadı; PostgreSQL'i ayrıca kurun veya uzak yönetici bağlantısını seçin.")?;
	child
		.stdin
		.take()
		.context("Standart girdi açılamadı.")?
		.write_all(&serde_json::to_vec(settings)?)?;
	let result = child.wait_with_output()?;
	ensure!(
		result.status.success(),
		"Yerel veritabanı hazırlanamadı: {}",
		crate::config::redact(
			&anyhow::anyhow!(String::from_utf8_lossy(&result.stderr).to_string()),
			settings.database_url.as_deref()
		)
	);
	println!("Veritabanı ve şema hazır.");
	Ok(())
}
