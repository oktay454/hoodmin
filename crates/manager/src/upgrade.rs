//! Root-only upgrade coordinator. The HTTP daemon only reads sanitized readiness state.
use crate::{config::Settings, schema};
use anyhow::{Context, Result, ensure};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx_core::{
	connection::Connection, executor::Executor, query::query, query_scalar::query_scalar, row::Row,
};
use sqlx_postgres::{PgConnectOptions, PgConnection};
use std::os::unix::process::CommandExt;
use std::{
	collections::BTreeMap,
	fs,
	io::Read,
	os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
	path::{Path, PathBuf},
	process::Stdio,
	time::Duration,
};
use tokio::io::AsyncWriteExt;

const DIRECTORY: &str = "/var/lib/hoodmin-upgrade";
const STATE: &str = "/var/lib/hoodmin-upgrade/state.json";
const POLICY: &str = "/etc/hoodmin/upgrade.json";
const VERSION: &str = env!("CARGO_PKG_VERSION");
#[derive(Args, Default)]
pub struct Options {
	#[arg(long)]
	pub watch: bool,
	#[arg(long)]
	pub request: bool,
	#[arg(long, requires = "request")]
	pub from_version: Option<String>,
	#[arg(long, value_parser=clap::value_parser!(u64).range(0..=60))]
	pub wait: Option<u64>,
	#[arg(long)]
	pub status: bool,
	#[arg(long, requires = "admin_url_file")]
	pub configure: bool,
	#[arg(long, requires = "configure")]
	pub admin_url_file: Option<PathBuf>,
	#[arg(long, hide = true)]
	pub internal: Option<Phase>,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum Phase {
	Probe,
	Apply,
	Backup,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct Policy {
	admin_url_file: Option<PathBuf>,
	backup_directory: PathBuf,
	attempt_timeout_seconds: u64,
	retry_seconds: u64,
}
impl Default for Policy {
	fn default() -> Self {
		Self {
			admin_url_file: None,
			backup_directory: "/var/backups/hoodmin-upgrade".into(),
			attempt_timeout_seconds: 300,
			retry_seconds: 30,
		}
	}
}
#[derive(Clone, Serialize, Deserialize)]
struct State {
	target: String,
	from_version: Option<String>,
	catalog: String,
	fingerprint: Option<String>,
	phase: String,
	updated_at: String,
	backup: Option<Checkpoint>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Checkpoint {
	path: PathBuf,
	signature: String,
}
fn fingerprint(s: &Settings) -> Option<String> {
	s.database_url.as_ref().map(|dsn| {
		format!(
			"{:x}",
			Sha256::digest(format!("{dsn}\n{}", s.master_key_file.display()).as_bytes())
		)
	})
}
fn state(s: &Settings, phase: &str) -> State {
	State {
		target: VERSION.into(),
		from_version: read_state()
			.ok()
			.filter(|st| st.target == VERSION)
			.and_then(|st| st.from_version),
		catalog: schema::digest(),
		fingerprint: fingerprint(s),
		phase: phase.into(),
		updated_at: chrono::Utc::now().to_rfc3339(),
		backup: if phase == "ready" {
			None
		} else {
			read_state()
				.ok()
				.filter(|st| {
					st.target == VERSION
						&& st.catalog == schema::digest()
						&& st.fingerprint == fingerprint(s)
				})
				.and_then(|st| st.backup)
		},
	}
}
fn read_state() -> Result<State> {
	Ok(serde_json::from_slice(&fs::read(STATE)?)?)
}
pub fn problem(s: &Settings) -> Option<&'static str> {
	if s.database_url.is_none() {
		return Some("Kurulumun tamamlanması bekleniyor.");
	}
	match read_state() {
		Ok(st)
			if st.target == VERSION
				&& st.catalog == schema::digest()
				&& st.fingerprint == fingerprint(s)
				&& st.phase == "ready" =>
		{
			None
		}
		Ok(st) if st.phase == "waiting_database" => Some("Veritabanı bağlantısı bekleniyor."),
		Ok(st) if st.phase == "needs_admin" => {
			Some("Yükseltme için yönetici yapılandırması bekleniyor.")
		}
		Ok(st) if st.phase == "failed" => {
			Some("Yükseltme tamamlanamadı. Lütfen yöneticiye başvurun.")
		}
		_ => Some("Yükseltme tamamlanıyor. Lütfen bekleyin."),
	}
}
fn protected(path: &Path) -> Result<Vec<u8>> {
	let mut f = fs::OpenOptions::new()
		.read(true)
		.custom_flags(0o400000)
		.open(path)?;
	let m = f.metadata()?;
	ensure!(
		m.is_file()
			&& m.uid() == 0
			&& m.nlink() == 1
			&& m.permissions().mode() & 0o077 == 0
			&& m.len() <= 65536,
		"Yönetici dosyası root sahipliğinde, normal dosya ve 0600 olmalı."
	);
	let mut bytes = Vec::new();
	f.read_to_end(&mut bytes)?;
	Ok(bytes)
}
fn settings(path: &Path) -> Result<Settings> {
	let m = fs::symlink_metadata(path)?;
	ensure!(
		m.is_file()
			&& !m.file_type().is_symlink()
			&& m.uid() == 0
			&& m.permissions().mode() & 0o022 == 0,
		"Yükseltme yapılandırması root tarafından korunmalı."
	);
	let s = Settings::load(path)?;
	s.validate()?;
	Ok(s)
}
fn group() -> Result<u32> {
	let output = std::process::Command::new("id")
		.args(["-g", "hoodmin"])
		.output()?;
	Ok(std::str::from_utf8(&output.stdout)?.trim().parse()?)
}
fn directory() -> Result<()> {
	fs::create_dir_all(DIRECTORY)?;
	let m = fs::symlink_metadata(DIRECTORY)?;
	ensure!(
		m.is_dir()
			&& !m.file_type().is_symlink()
			&& m.uid() == 0
			&& m.permissions().mode() & 0o022 == 0,
		"Yükseltme durum dizini root tarafından korunmalı."
	);
	std::os::unix::fs::chown(DIRECTORY, Some(0), Some(group()?))?;
	fs::set_permissions(DIRECTORY, fs::Permissions::from_mode(0o750))?;
	Ok(())
}
fn write_state(value: &State) -> Result<()> {
	directory()?;
	let tmp = Path::new(DIRECTORY).join(format!(".state-{}", uuid::Uuid::new_v4()));
	let f = crate::crypto::write_private_file(&tmp, &serde_json::to_vec(value)?)?;
	std::os::unix::fs::fchown(&f, Some(0), Some(group()?))?;
	f.set_permissions(fs::Permissions::from_mode(0o640))?;
	f.sync_all()?;
	fs::rename(tmp, STATE)?;
	fs::File::open(DIRECTORY)?.sync_all()?;
	Ok(())
}
fn policy() -> Result<Policy> {
	let p = if Path::new(POLICY).exists() {
		serde_json::from_slice(&protected(Path::new(POLICY))?)?
	} else {
		Policy::default()
	};
	ensure!(
		p.backup_directory.is_absolute()
			&& (1..=3600).contains(&p.attempt_timeout_seconds)
			&& (1..=300).contains(&p.retry_seconds),
		"Yükseltme politikası geçersiz."
	);
	Ok(p)
}
fn admin(s: &Settings, p: &Policy) -> Result<Option<String>> {
	let app = crate::config::database_options(s.db()?)?;
	if let Some(file) = &p.admin_url_file {
		let text = String::from_utf8(protected(file)?)?.trim().to_owned();
		let db = crate::config::database_options(&text)?;
		ensure!(
			db.get_database() == app.get_database()
				&& db.get_host() == app.get_host()
				&& db.get_port() == app.get_port()
				&& db.get_socket() == app.get_socket(),
			"Yükseltme bağlantısı aynı sunucu ve veritabanını hedeflemeli."
		);
		Ok(Some(text))
	} else {
		ensure!(
			app.get_socket().is_some(),
			"Uzak veritabanı için yükseltme yönetici dosyasını yapılandırın."
		);
		Ok(None)
	}
}
#[derive(Serialize, Deserialize)]
struct Input {
	from_version: Option<String>,
	config: PathBuf,
	settings: Settings,
	admin_url: Option<String>,
	backup_directory: PathBuf,
	signature: Option<String>,
}
#[derive(Default, Serialize, Deserialize)]
struct Output {
	pending: Vec<String>,
	signature: String,
	retry: bool,
	error: Option<String>,
}
fn retryable(e: &anyhow::Error) -> bool {
	e.chain().any(|cause| {
		if let Some(db) = cause.downcast_ref::<sqlx_core::Error>() {
			match db {
				sqlx_core::Error::Io(_) | sqlx_core::Error::PoolTimedOut => true,
				sqlx_core::Error::Database(e) => e.code().is_some_and(|code| {
					code.starts_with("08")
						|| matches!(code.as_ref(), "57P01" | "57P02" | "57P03" | "55P03")
				}),
				_ => false,
			}
		} else {
			false
		}
	})
}
async fn worker(phase: Phase) -> Result<()> {
	let who = std::process::Command::new("id").arg("-un").output()?;
	let who = String::from_utf8(who.stdout)?;
	ensure!(
		matches!(who.trim(), "root" | "postgres"),
		"İç yükseltme yardımcısı yalnız yönetici hesabında çalışabilir."
	);
	let mut text = String::new();
	std::io::stdin().take(262144).read_to_string(&mut text)?;
	let input: Input = serde_json::from_str(&text)?;
	let result = work(phase, &input).await;
	let output = match result {
		Ok(output) => output,
		Err(error) => {
			let clean = crate::config::redact(&error, input.settings.database_url.as_deref());
			let clean = crate::config::redact(&anyhow::anyhow!(clean), input.admin_url.as_deref());
			Output {
				retry: retryable(&error),
				error: Some(clean),
				..Output::default()
			}
		}
	};
	println!("{}", serde_json::to_string(&output)?);
	Ok(())
}
async fn work(phase: Phase, input: &Input) -> Result<Output> {
	if matches!(phase, Phase::Backup) {
		crate::cli::root()?;
		let admin_file = if let Some(url) = &input.admin_url {
			let path = Path::new(DIRECTORY).join(format!(".backup-url-{}", uuid::Uuid::new_v4()));
			crate::crypto::write_private(&path, url.as_bytes())?;
			Some(path)
		} else {
			None
		};
		let provenance = serde_json::json!({"from_version":input.from_version,"target_version":VERSION,"target_catalog":schema::digest(),"before_schema":input.signature});
		let result = crate::cli_backup::create(
			&input.config,
			&input.settings,
			admin_file.as_deref(),
			&input.backup_directory,
			Some(&provenance),
		);
		if let Some(file) = admin_file {
			fs::remove_file(file)?;
		}
		let path = result?;
		crate::retention::verify(&path)?;
		let mut dump = fs::File::open(path.join("database.dump"))?;
		let mut magic = [0; 5];
		dump.read_exact(&mut magic)?;
		ensure!(&magic == b"PGDMP", "Yedek dökümü geçersiz.");
		ensure!(
			fs::read(path.join("master.key"))? == fs::read(&input.settings.master_key_file)?,
			"Yedek anahtarı uyuşmuyor."
		);
		return Ok(Output {
			signature: path.to_string_lossy().into_owned(),
			..Output::default()
		});
	}
	let app = crate::config::database_options(input.settings.db()?)?;
	let options = if let Some(url) = &input.admin_url {
		crate::config::database_options(url)?
	} else {
		ensure!(app.get_socket().is_some(), "Yerel soket gerekli.");
		PgConnectOptions::new()
			.host(&app.get_socket().unwrap().to_string_lossy())
			.port(app.get_port())
			.username("postgres")
	}
	.database(app.get_database().context("Veritabanı adı gerekli.")?);
	let mut connection = PgConnection::connect_with(&options).await?;
	connection
		.execute("SET statement_timeout='240s'; SET lock_timeout='3s'")
		.await?;
	let bypass: bool =
		query_scalar("SELECT rolsuper OR rolbypassrls FROM pg_roles WHERE rolname=current_user")
			.fetch_one(&mut connection)
			.await?;
	ensure!(bypass, "Şema yöneticisinin bakım yetkisi eksik.");
	let role = query("SELECT rolsuper,rolbypassrls FROM pg_roles WHERE rolname=$1")
		.bind(app.get_username())
		.fetch_one(&mut connection)
		.await?;
	ensure!(
		!role.get::<bool, _>("rolsuper") && !role.get::<bool, _>("rolbypassrls"),
		"Uygulama rolü yönetici olamaz."
	);
	let held: bool = query_scalar("SELECT pg_try_advisory_lock(684663001)")
		.fetch_one(&mut connection)
		.await?;
	if !held {
		return Ok(Output {
			retry: true,
			error: Some("Başka şema bakım işlemi çalışıyor.".into()),
			..Output::default()
		});
	}
	let rows = query("SELECT name,checksum FROM _hoodmin_migrations ORDER BY name")
		.fetch_all(&mut connection)
		.await?;
	let applied: BTreeMap<String, String> = rows
		.into_iter()
		.map(|r| (r.get("name"), r.get("checksum")))
		.collect();
	ensure!(
		!applied.is_empty(),
		"Önce açık kurulum işlemini tamamlayın; yükseltme boş şema oluşturmaz."
	);
	let pending = schema::pending(&applied)?;
	let permission: bool =
		query_scalar("SELECT has_table_privilege($1,'_hoodmin_migrations','SELECT')")
			.bind(app.get_username())
			.fetch_one(&mut connection)
			.await?;
	let mut pending = pending;
	if !permission && pending.is_empty() {
		pending.push("application_permissions".into());
	}
	let signature = format!("{:x}", Sha256::digest(serde_json::to_vec(&applied)?));
	if matches!(phase, Phase::Apply) && !pending.is_empty() {
		ensure!(
			input.signature.as_deref() == Some(&signature),
			"Şema deneme sırasında değişti; yükseltmeyi yeniden deneyin."
		);
		let held: bool = query_scalar("SELECT pg_try_advisory_lock(684663002)")
			.fetch_one(&mut connection)
			.await?;
		if !held {
			return Ok(Output {
				retry: true,
				error: Some("Başka anahtar/veri bakım işlemi çalışıyor.".into()),
				..Output::default()
			});
		}
		let actual: BTreeMap<_, _> = fs::read_dir("/usr/share/hoodmin/migrations")?
			.map(|entry| {
				let path = entry?.path();
				let name = path
					.file_name()
					.context("Geçiş adı eksik.")?
					.to_string_lossy()
					.into_owned();
				let hash = format!("{:x}", Sha256::digest(fs::read(path)?));
				Ok((name, hash))
			})
			.collect::<Result<_>>()?;
		ensure!(
			actual == schema::hashes(),
			"Kurulu geçiş dosyaları paket kataloğuyla uyuşmuyor."
		);
		crate::cli_database::apply_locked(
			&mut connection,
			&crate::cli_database::identifier(app.get_username())?,
			&crate::cli_database::identifier(app.get_database().unwrap())?,
		)
		.await?;
	}
	Ok(Output {
		pending,
		signature,
		..Output::default()
	})
}
// Every child gets its own process group. Timeout also terminates pg_dump descendants.
async fn child(phase: Phase, input: &Input, seconds: u64) -> Result<Output> {
	let mut command = tokio::process::Command::new(
		if input.admin_url.is_none() && !matches!(phase, Phase::Backup) {
			"runuser".into()
		} else {
			std::env::current_exe()?
		},
	);
	if input.admin_url.is_none() && !matches!(phase, Phase::Backup) {
		command
			.args(["-u", "postgres", "--"])
			.arg(std::env::current_exe()?);
	}
	command.args([
		"upgrade",
		"--internal",
		match phase {
			Phase::Probe => "probe",
			Phase::Apply => "apply",
			Phase::Backup => "backup",
		},
	]);
	command
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.kill_on_drop(true);
	command.as_std_mut().process_group(0);
	let mut process = command.spawn()?;
	let pid = process.id().context("Yardımcı süreç başlatılamadı.")?;
	let mut stdin = process.stdin.take().unwrap();
	let mut stdout = process.stdout.take().unwrap();
	let mut stderr = process.stderr.take().unwrap();
	let result = tokio::time::timeout(Duration::from_secs(seconds), async {
		stdin.write_all(&serde_json::to_vec(input)?).await?;
		drop(stdin);
		Ok::<_, anyhow::Error>(process.wait().await?)
	})
	.await;
	let status = match result {
		Ok(status) => status?,
		Err(_) => {
			unsafe extern "C" {
				fn kill(pid: i32, signal: i32) -> i32;
			}
			// SAFETY: only the process group created above, still held by our unreaped child.
			unsafe {
				kill(-(pid as i32), 9);
			}
			let _ = process.wait().await;
			return Ok(Output {
				retry: true,
				error: Some("Yükseltme denemesi zaman aşımına uğradı.".into()),
				..Output::default()
			});
		}
	};
	let mut bytes = Vec::new();
	tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut bytes).await?;
	let mut err = Vec::new();
	tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut err).await?;
	if !status.success() {
		let error = anyhow::anyhow!(
			String::from_utf8_lossy(&err)
				.chars()
				.take(2048)
				.collect::<String>()
		);
		let clean = crate::config::redact(&error, input.settings.database_url.as_deref());
		let clean = crate::config::redact(&anyhow::anyhow!(clean), input.admin_url.as_deref());
		anyhow::bail!("Yükseltme yardımcısı başarısız: {clean}");
	}
	Ok(serde_json::from_slice(&bytes)?)
}
fn try_lock(s: &Settings) -> Result<Option<fs::File>> {
	let path = s.master_key_file.with_extension("maintenance.lock");
	let f = fs::OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(false)
		.mode(0o600)
		.custom_flags(0o400000)
		.open(path)?;
	ensure!(
		f.metadata()?.is_file() && f.metadata()?.nlink() == 1,
		"Bakım kilidi normal, tek bağlantılı dosya olmalı."
	);
	let meta = fs::metadata(&s.master_key_file)?;
	if f.metadata()?.uid() != meta.uid() {
		std::os::unix::fs::fchown(&f, Some(meta.uid()), Some(meta.gid()))?;
	}
	match f.try_lock() {
		Ok(()) => Ok(Some(f)),
		Err(std::fs::TryLockError::WouldBlock) => Ok(None),
		Err(error) => Err(error.into()),
	}
}
async fn attempt(path: &Path, p: &Policy) -> Result<(Settings, &'static str)> {
	let s = settings(path)?;
	if s.database_url.is_none() {
		write_state(&state(&s, "needs_setup"))?;
		return Ok((s, "needs_setup"));
	}
	let Some(_lock) = try_lock(&s)? else {
		return Ok((s, "busy"));
	};
	// Filesystem maintenance uses only root-owned configuration; never create/replace a lost key.
	s.vault()?;
	std::os::unix::fs::chown(path, Some(0), Some(group()?))?;
	fs::set_permissions(path, fs::Permissions::from_mode(0o640))?;
	let admin_url = match admin(&s, p) {
		Ok(admin) => admin,
		Err(error) => {
			tracing::error!(%error,"Yükseltme yönetici bağlantısı hazırlanamadı");
			write_state(&state(&s, "needs_admin"))?;
			return Ok((s, "needs_admin"));
		}
	};
	let mut input = Input {
		from_version: read_state().ok().and_then(|st| st.from_version),
		config: path.to_owned(),
		settings: s.clone(),
		admin_url,
		backup_directory: p.backup_directory.clone(),
		signature: None,
	};
	let plan = child(Phase::Probe, &input, 10).await?;
	if let Some(error) = plan.error {
		tracing::error!(detail=%error,"Yükseltme ön denetimi tamamlanamadı");
		let phase = if plan.retry {
			"waiting_database"
		} else {
			"failed"
		};
		write_state(&state(&s, phase))?;
		return Ok((s, phase));
	}
	if !plan.pending.is_empty() {
		write_state(&state(&s, "running"))?;
		input.signature = Some(plan.signature.clone());
		let cached = read_state().ok().and_then(|st| st.backup).filter(|saved| {
			saved.signature == plan.signature
				&& saved.path.starts_with(&p.backup_directory)
				&& crate::retention::verify(&saved.path).is_ok()
				&& fs::read(saved.path.join("hoodmin.json")).ok() == fs::read(path).ok()
				&& fs::read(saved.path.join("master.key")).ok() == fs::read(&s.master_key_file).ok()
		});
		if cached.is_none() {
			let backup = child(Phase::Backup, &input, p.attempt_timeout_seconds).await?;
			if let Some(error) = backup.error {
				tracing::error!(detail=%error,"Yükseltme yedeği oluşturulamadı");
				let phase = if backup.retry {
					"waiting_database"
				} else {
					"failed"
				};
				write_state(&state(&s, phase))?;
				return Ok((s, phase));
			}
			let mut st = state(&s, "running");
			st.backup = Some(Checkpoint {
				path: backup.signature.into(),
				signature: plan.signature.clone(),
			});
			write_state(&st)?;
		}
		input.signature = Some(plan.signature);
		let applied = child(Phase::Apply, &input, p.attempt_timeout_seconds).await?;
		if let Some(error) = applied.error {
			tracing::error!(detail=%error,"Yükseltme uygulanamadı");
			let phase = if applied.retry {
				"waiting_database"
			} else {
				"failed"
			};
			write_state(&state(&s, phase))?;
			return Ok((s, phase));
		}
		let verified = child(Phase::Probe, &input, 10).await?;
		ensure!(
			verified.error.is_none() && verified.pending.is_empty(),
			"Yükseltme sonucu doğrulanamadı."
		);
	}
	write_state(&state(&s, "ready"))?;
	Ok((s, "ready"))
}
pub async fn run(path: &Path, options: Options) -> Result<()> {
	if let Some(phase) = options.internal {
		return worker(phase).await;
	}
	crate::cli::root()?;
	ensure!(
		[
			options.watch,
			options.request,
			options.wait.is_some(),
			options.status,
			options.configure
		]
		.into_iter()
		.filter(|b| *b)
		.count() <= 1,
		"Tek yükseltme çalışma biçimi seçin."
	);
	if options.status {
		println!("{}", serde_json::to_string_pretty(&read_state()?)?);
		return Ok(());
	}
	if options.configure {
		let s = settings(path)?;
		let mut p = policy()?;
		p.admin_url_file = options.admin_url_file;
		admin(&s, &p)?;
		let tmp =
			Path::new(POLICY).with_file_name(format!(".upgrade-{}.json", uuid::Uuid::new_v4()));
		crate::crypto::write_private(&tmp, &serde_json::to_vec_pretty(&p)?)?;
		fs::rename(tmp, POLICY)?;
		println!("Yükseltme yönetici dosyası kaydedildi.");
		return Ok(());
	}
	if options.request {
		let s = settings(path)?;
		let mut st = state(
			&s,
			if s.database_url.is_none() {
				"needs_setup"
			} else {
				"pending"
			},
		);
		if options.from_version.is_some() {
			st.from_version = options.from_version;
		}
		write_state(&st)?;
		return Ok(());
	}
	if let Some(seconds) = options.wait {
		for _ in 0..seconds {
			if let Ok(st) = read_state() {
				if matches!(st.phase.as_str(), "ready" | "needs_setup") {
					return Ok(());
				}
				ensure!(
					st.phase != "failed",
					"Yükseltme başarısız; hoodmin-upgrade.service günlüğünü kontrol edin."
				);
			}
			tokio::time::sleep(Duration::from_secs(1)).await;
		}
		println!("Paket kuruldu; ertelenen yükseltme hizmet tarafından tamamlanacak.");
		return Ok(());
	}
	loop {
		let p = match policy() {
			Ok(p) => p,
			Err(error) => {
				if let Ok(s) = settings(path) {
					write_state(&state(&s, "failed"))?;
				}
				return Err(error);
			}
		};
		let result = attempt(path, &p).await;
		let phase = match result {
			Ok((_, phase)) => phase,
			Err(error) => {
				tracing::error!(%error,"Yükseltme tamamlanamadı");
				if let Ok(s) = settings(path) {
					write_state(&state(&s, "failed"))?;
				}
				"failed"
			}
		};
		tracing::info!(phase, "Yükseltme durumu");
		if phase == "ready" {
			return Ok(());
		}
		if !options.watch {
			ensure!(
				matches!(
					phase,
					"waiting_database" | "needs_setup" | "needs_admin" | "busy"
				),
				"Yükseltme başarısız; hizmet günlüğünü kontrol edin."
			);
			return Ok(());
		}
		tokio::time::sleep(Duration::from_secs(p.retry_seconds)).await;
	}
}
