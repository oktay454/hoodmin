use crate::config::Settings;
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use sha2::{Digest, Sha256};
use std::{
	fs,
	io::{Read, Write},
	path::{Path, PathBuf},
	process::{Command, Stdio},
};
#[derive(Subcommand)]
pub enum Action {
	/// Tamamlanmış yedekleri incele; yalnız --apply ile sil.
	Prune {
		#[arg(long)]
		apply: bool,
	},
	/// Saklama politikasını özel bir JSON dosyasından doğrula ve kaydet.
	Policy {
		#[arg(long)]
		file: Option<PathBuf>,
	},
	/// Özel veritabanı dökümü, yapılandırma, anahtar ve ACME dosyaları.
	Create {
		#[arg(long)]
		admin_url_file: Option<PathBuf>,
		#[arg(long, default_value = "/var/backups/hoodmin")]
		directory: PathBuf,
	},
	/// age alıcısına şifrelenmiş yedek oluştur.
	Export {
		#[arg(long)]
		admin_url_file: Option<PathBuf>,
		#[arg(long, default_value = "/etc/hoodmin/backup-recipient.txt")]
		recipient_file: PathBuf,
		#[arg(long, default_value = "/var/backups/hoodmin")]
		directory: PathBuf,
	},
	/// Sabit komutlu uzak SSH hesabından şifreli yedeği indir.
	Pull {
		#[arg(long)]
		host: String,
		#[arg(long, default_value = "root")]
		user: String,
		#[arg(long)]
		identity: PathBuf,
		#[arg(long)]
		known_hosts: PathBuf,
		#[arg(long, default_value = "/var/backups/hoodmin-remote")]
		directory: PathBuf,
	},
}
pub fn run(config: &Path, settings: &Settings, action: Action) -> Result<()> {
	let report = !matches!(action, Action::Policy { .. });
	let completed = matches!(
		action,
		Action::Create { .. } | Action::Export { .. } | Action::Pull { .. }
	);
	let result = run_inner(config, settings, action);
	if report
		&& let Err(error) = record(
			settings,
			if completed || result.is_err() {
				Some(result.is_ok())
			} else {
				None
			},
		) {
		tracing::warn!(%error,"Yedek durumu kaydedilemedi");
	}
	result
}
fn record(settings: &Settings, success: Option<bool>) -> Result<()> {
	use std::os::unix::fs::{MetadataExt, PermissionsExt};
	let status = settings
		.master_key_file
		.with_file_name("backup-status.json");
	let tmp = status.with_file_name(format!(".backup-status-{}", uuid::Uuid::new_v4()));
	let free = crate::retention::free_bytes(&settings.backup.directory).ok();
	let pressure = free.is_some_and(|free| free < settings.backup.min_free_bytes)
		|| (settings.backup.max_bytes > 0
			&& crate::retention::prune(&settings.backup, false)
				.is_ok_and(|r| r.total_bytes > settings.backup.max_bytes));
	let success = success.or_else(|| {
		fs::read(&status)
			.ok()
			.and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
			.and_then(|s| s["success"].as_bool())
	});
	let file = crate::crypto::write_private_file(
		&tmp,
		&serde_json::to_vec(
			&serde_json::json!({"success":success,"pressure":pressure,"at":chrono::Utc::now()}),
		)?,
	)?;
	let meta = fs::metadata(&settings.master_key_file)?;
	std::os::unix::fs::fchown(&file, Some(0), Some(meta.gid()))?;
	file.set_permissions(fs::Permissions::from_mode(0o640))?;
	file.sync_all()?;
	fs::rename(tmp, &status)?;
	fs::File::open(status.parent().context("Durum dizini eksik.")?)?.sync_all()?;
	Ok(())
}
fn run_inner(config: &Path, settings: &Settings, action: Action) -> Result<()> {
	crate::cli::root()?;
	let _maintenance = crate::crypto::maintenance_lock(&settings.master_key_file)?;
	match action {
		Action::Prune { apply } => println!(
			"{}",
			serde_json::to_string_pretty(&crate::retention::prune(&settings.backup, apply)?)?
		),
		Action::Policy { file } => {
			if let Some(file) = file {
				let policy: crate::retention::Policy =
					serde_json::from_str(&crate::cli::read_secret(&file)?)?;
				policy.validate()?;
				let mut updated = settings.clone();
				updated.backup = policy;
				crate::crypto::replace_key(config, &serde_json::to_vec_pretty(&updated)?)?;
			} else {
				println!("{}", serde_json::to_string_pretty(&settings.backup)?);
			}
		}
		Action::Create {
			admin_url_file,
			directory,
		} => {
			let path = create(config, settings, admin_url_file.as_deref(), &directory)?;
			automatic(settings, &directory)?;
			println!("{}", path.display());
		}
		Action::Export {
			admin_url_file,
			recipient_file,
			directory,
		} => {
			let recipient = fs::read_to_string(recipient_file)?.trim().to_owned();
			ensure!(
				recipient.starts_with("age1") && !recipient.chars().any(char::is_whitespace),
				"Açık age alıcısı geçersiz."
			);
			let path = create(config, settings, admin_url_file.as_deref(), &directory)?;
			let tmp = directory.join(format!(".export-{}.age", uuid::Uuid::new_v4()));
			let file = private_file(&tmp)?;
			let mut tar = Command::new("tar")
				.arg("-C")
				.arg(&path)
				.args(["-cf", "-", "."])
				.stdout(Stdio::piped())
				.spawn()?;
			let status = Command::new("age")
				.args(["--encrypt", "--recipient", &recipient])
				.stdin(tar.stdout.take().context("Arşiv akışı açılamadı.")?)
				.stdout(file)
				.stderr(Stdio::null())
				.status();
			let archived = tar.wait()?;
			if !status.is_ok_and(|s| s.success()) || !archived.success() {
				let _ = fs::remove_file(&tmp);
				anyhow::bail!("Şifreli yedek oluşturulamadı; age aracını ve alıcıyı denetleyin.");
			}
			fs::File::open(&tmp)?.sync_all()?;
			let archive = path.with_extension("age");
			fs::rename(&tmp, &archive)?;
			let latest = directory.join(format!(".latest-{}", uuid::Uuid::new_v4()));
			fs::hard_link(&archive, &latest)?;
			fs::rename(latest, directory.join("latest.age"))?;
			fs::File::open(&directory)?.sync_all()?;
			automatic(settings, &directory)?;
			println!("Şifreli yedek hazır.");
		}
		Action::Pull {
			host,
			user,
			identity,
			known_hosts,
			directory,
		} => {
			let host = crate::management::normalize_host(&host)?;
			ensure!(
				!user.is_empty()
					&& user
						.bytes()
						.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
					&& !user.starts_with('-'),
				"SSH kullanıcısı geçersiz."
			);
			mkdir(&directory)?;
			let path = directory.join(format!("hoodmin-{}.age", uuid::Uuid::new_v4()));
			let file = private_file(&path)?;
			let status = Command::new("ssh")
				.args([
					"-T",
					"-o",
					"BatchMode=yes",
					"-o",
					"IdentitiesOnly=yes",
					"-o",
					"StrictHostKeyChecking=yes",
					"-o",
					"ConnectTimeout=15",
					"-o",
				])
				.arg(format!("UserKnownHostsFile={}", known_hosts.display()))
				.arg("-i")
				.arg(identity)
				.arg("-l")
				.arg(user)
				.arg(host)
				.stdin(Stdio::null())
				.stdout(file)
				.stderr(Stdio::null())
				.status()?;
			if !status.success() || fs::metadata(&path)?.len() == 0 {
				let _ = fs::remove_file(path);
				anyhow::bail!(
					"Uzak yedek alınamadı; SSH kimliğini ve sabit komut yetkisini denetleyin."
				);
			}
			println!("{}", path.display());
		}
	}
	Ok(())
}
fn private_file(path: &Path) -> Result<fs::File> {
	use std::os::unix::fs::OpenOptionsExt;
	Ok(fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.mode(0o600)
		.open(path)?)
}
fn mkdir(path: &Path) -> Result<()> {
	use std::os::unix::fs::PermissionsExt;
	fs::create_dir_all(path)?;
	fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
	Ok(())
}
fn copy_tree(source: &Path, target: &Path) -> Result<()> {
	use std::os::unix::fs::symlink;
	if source.is_symlink() {
		symlink(fs::read_link(source)?, target)?;
	} else if source.is_dir() {
		fs::create_dir(target)?;
		for entry in fs::read_dir(source)? {
			let entry = entry?;
			copy_tree(&entry.path(), &target.join(entry.file_name()))?;
		}
		fs::set_permissions(target, fs::metadata(source)?.permissions())?;
	} else if source.is_file() {
		fs::copy(source, target)?;
	}
	Ok(())
}
pub(crate) fn digest(path: &Path) -> Result<String> {
	let mut file = fs::File::open(path)?;
	let mut hash = Sha256::new();
	let mut buffer = [0u8; 65536];
	loop {
		let size = file.read(&mut buffer)?;
		if size == 0 {
			break;
		}
		hash.update(&buffer[..size]);
	}
	Ok(format!("{:x}", hash.finalize()))
}
fn create(
	config: &Path,
	settings: &Settings,
	admin_file: Option<&Path>,
	directory: &Path,
) -> Result<PathBuf> {
	let app = crate::config::database_options(settings.db()?)?;
	let local = admin_file.is_none() && app.get_socket().is_some();
	let dsn = if let Some(file) = admin_file {
		crate::cli::read_secret(file)?
	} else {
		ensure!(
			local,
			"Uzak yedek için --admin-url-file gerekli; uygulama rolü kiracı yalıtımını atlayamaz."
		);
		settings.db()?.to_owned()
	};
	let options = crate::config::database_options(&dsn)?;
	ensure!(
		options.get_database() == app.get_database(),
		"Yedek bağlantısı uygulamanın veritabanını hedeflemeli."
	);
	mkdir(directory)?;
	let canonical = fs::canonicalize(directory)?;
	for source in [
		config.parent().context("Yapılandırma dizini eksik.")?,
		settings
			.master_key_file
			.parent()
			.context("Veri dizini eksik.")?,
	] {
		ensure!(
			!canonical.starts_with(fs::canonicalize(source)?),
			"Yedek dizini yapılandırma/veri dizininin altında olamaz."
		);
	}
	let path = directory.join(format!(
		"hoodmin-{}-{}",
		chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
		uuid::Uuid::new_v4()
	));
	mkdir(&path)?;
	let partial = path.join("database.dump.partial");
	let dump = private_file(&partial)?;
	let mut command = if local {
		let mut c = Command::new("runuser");
		c.args(["-u", "postgres", "--preserve-environment", "--", "pg_dump"]);
		c
	} else {
		Command::new("pg_dump")
	};
	command
		.arg("--format=custom")
		.stdout(dump)
		.stderr(Stdio::null());
	command
		.env_remove("PGSERVICE")
		.env_remove("PGSERVICEFILE")
		.env_remove("PGPASSFILE")
		.env_remove("PGOPTIONS");
	command
		.env(
			"PGHOST",
			options
				.get_socket()
				.map(|s| s.to_string_lossy().into_owned())
				.unwrap_or_else(|| options.get_host().to_owned()),
		)
		.env("PGPORT", options.get_port().to_string())
		.env(
			"PGDATABASE",
			app.get_database().context("Veritabanı adı gerekli.")?,
		)
		.env(
			"PGUSER",
			if local {
				"postgres"
			} else {
				options.get_username()
			},
		)
		.env("PGSSLMODE", if local { "disable" } else { "verify-full" });
	if let Ok(url) = url::Url::parse(&dsn) {
		if let Some(password) = url.password() {
			command.env(
				"PGPASSWORD",
				percent_encoding::percent_decode_str(password)
					.decode_utf8()?
					.as_ref(),
			);
		} else {
			command.env_remove("PGPASSWORD");
		}
		for (key, value) in url.query_pairs() {
			match key.as_ref() {
				"sslrootcert" => {
					command.env("PGSSLROOTCERT", value.as_ref());
				}
				"sslcert" => {
					command.env("PGSSLCERT", value.as_ref());
				}
				"sslkey" => {
					command.env("PGSSLKEY", value.as_ref());
				}
				_ => {}
			}
		}
	}
	let success = command
		.status()
		.context("pg_dump bulunamadı; Debian postgresql-client paketini ayrıca kurun.")?
		.success();
	ensure!(
		success,
		"Veritabanı yedeği alınamadı; yönetici yetkisi, TLS ve pg_dump sürümünü denetleyin. Eksik yedek .partial olarak kaldı."
	);
	fs::rename(partial, path.join("database.dump"))?;
	fs::copy(&settings.master_key_file, path.join("master.key"))?;
	fs::copy(config, path.join("hoodmin.json"))?;
	if let Some(parent) = config.parent() {
		copy_tree(parent, &path.join("configuration"))?;
	}
	let state = settings
		.master_key_file
		.parent()
		.context("Veri dizini geçersiz.")?;
	for (source, name) in [
		(state.join("acme"), "acme"),
		(PathBuf::from("/etc/letsencrypt"), "letsencrypt"),
	] {
		if source.exists() {
			copy_tree(&source, &path.join(name))?;
		}
	}
	let version = Command::new("dpkg-query")
		.args(["-W", "hoodmin"])
		.output()?;
	fs::write(path.join("package-version.txt"), version.stdout)?;
	let mut sums = private_file(&path.join("SHA256SUMS"))?;
	for name in [
		"database.dump",
		"master.key",
		"hoodmin.json",
		"package-version.txt",
	] {
		writeln!(sums, "{}  {name}", digest(&path.join(name))?)?;
	}
	sums.sync_all()?;
	sync_tree(&path)?;
	crate::retention::complete(&path)?;
	Ok(path)
}

fn automatic(settings: &Settings, directory: &Path) -> Result<()> {
	if settings.backup.enabled
		&& fs::canonicalize(directory)? == fs::canonicalize(&settings.backup.directory)?
	{
		let report = crate::retention::prune(&settings.backup, true)?;
		if report.pressure {
			tracing::warn!("Yedek saklama sınırları korunan kopyalar nedeniyle sağlanamadı.");
		}
		tracing::info!(
			removed = report.removed.len(),
			retained = report.retained,
			"Yedek saklama politikası uygulandı"
		);
	}
	Ok(())
}

fn sync_tree(path: &Path) -> Result<()> {
	let metadata = fs::symlink_metadata(path)?;
	if metadata.file_type().is_symlink() {
		return Ok(());
	}
	if metadata.is_dir() {
		for entry in fs::read_dir(path)? {
			sync_tree(&entry?.path())?;
		}
	}
	fs::File::open(path)?.sync_all()?;
	Ok(())
}
