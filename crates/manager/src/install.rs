use crate::{auth, crypto, store::Store};
use anyhow::{Context, Result, ensure};
use base64::{
	Engine,
	engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx_core::query::query;
use std::{os::unix::fs::PermissionsExt, path::PathBuf, process::Stdio, time::Duration};
use tokio::{
	io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
	process::Command,
};
use uuid::Uuid;

pub const LINUX_INSTALLER: &str =
	"https://github.com/vpnhood/VpnHood.App.Server/releases/latest/download/VpnHoodServer-linux.sh";
pub const WINDOWS_INSTALLER: &str = "https://github.com/vpnhood/VpnHood.App.Server/releases/latest/download/VpnHoodServer-win-x64.ps1";
pub fn settings(origin: &str, credential: &str) -> Value {
	json!({"HttpAccessManager":{"BaseUrl":origin,"Authorization":format!("Bearer {credential}")},"ManagementSecret":STANDARD.encode(crypto::random::<16>()),"AllowAnonymousTracker":false,"LogLevel":"Information"})
}
fn shell_quote(value: &str) -> String {
	format!("'{}'", value.replace('\'', "'\"'\"'"))
}
fn ps_quote(value: &str) -> String {
	format!("'{}'", value.replace('\'', "''"))
}
pub fn commands(settings: &Value) -> (String, String) {
	let origin = settings["HttpAccessManager"]["BaseUrl"]
		.as_str()
		.unwrap_or_default();
	let authorization = settings["HttpAccessManager"]["Authorization"]
		.as_str()
		.unwrap_or_default();
	let secret = settings["ManagementSecret"].as_str().unwrap_or_default();
	let linux = format!(
		"sudo bash -c {}",
		shell_quote(&format!(
			"bash <(wget -qO- {LINUX_INSTALLER}) -managementSecret {} -httpBaseUrl {} -httpAuthorization {}",
			shell_quote(secret),
			shell_quote(origin),
			shell_quote(authorization)
		))
	);
	let windows = format!(
		"[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; & ([ScriptBlock]::Create((Invoke-WebRequest -UseBasicParsing {}).Content)) -managementSecret {} -httpBaseUrl {} -httpAuthorization {}",
		ps_quote(WINDOWS_INSTALLER),
		ps_quote(secret),
		ps_quote(origin),
		ps_quote(authorization)
	);
	(linux, windows)
}
pub struct SshInstall {
	pub host: String,
	pub port: u16,
	pub username: String,
	pub fingerprint: String,
	pub password: String,
	pub private_key: String,
	pub sudo_password: String,
	pub credential: String,
	pub origin: String,
}
impl SshInstall {
	pub fn validate(&self) -> Result<()> {
		crate::management::normalize_host(&self.host)?;
		ensure!(
			self.port > 0
				&& (1..=64).contains(&self.username.len())
				&& !self.username.starts_with('-')
				&& self
					.username
					.bytes()
					.all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)),
			"SSH kullanıcı/port geçersiz"
		);
		ensure!(
			self.fingerprint.starts_with("SHA256:")
				&& STANDARD_NO_PAD
					.decode(&self.fingerprint[7..])
					.is_ok_and(|v| v.len() == 32),
			"SHA256 sunucu anahtarı parmak izi gerekli"
		);
		ensure!(
			(!self.password.is_empty()) != (!self.private_key.is_empty()),
			"Parola veya özel anahtardan biri gerekli"
		);
		ensure!(
			self.password.len() <= 1024
				&& self.sudo_password.len() <= 1024
				&& !self.password.contains(['\r', '\n'])
				&& !self.sudo_password.contains(['\r', '\n'])
				&& self.private_key.len() <= 65536,
			"SSH kimlik bilgisi boyutu/biçimi geçersiz"
		);
		Ok(())
	}
}
async fn capture(mut reader: impl AsyncRead + Unpin) -> Result<Vec<u8>> {
	let mut output = Vec::new();
	let mut buffer = [0; 8192];
	loop {
		let n = reader.read(&mut buffer).await?;
		if n == 0 {
			break;
		}
		let remaining = 65536usize.saturating_sub(output.len());
		output.extend_from_slice(&buffer[..n.min(remaining)]);
	}
	Ok(output)
}
async fn execute(
	mut command: Command,
	input: &[u8],
	timeout: Duration,
) -> Result<(bool, String, String)> {
	command
		.kill_on_drop(true)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	tokio::time::timeout(timeout, async move {
		let mut child = command.spawn()?;
		let mut stdin = child.stdin.take().context("SSH stdin")?;
		let stdout = child.stdout.take().context("SSH stdout")?;
		let stderr = child.stderr.take().context("SSH stderr")?;
		let (write, out, err) = tokio::join!(
			async {
				stdin.write_all(input).await?;
				stdin.shutdown().await?;
				drop(stdin);
				Ok::<_, anyhow::Error>(())
			},
			capture(stdout),
			capture(stderr)
		);
		write?;
		let success = child.wait().await?.success();
		Ok::<_, anyhow::Error>((
			success,
			String::from_utf8_lossy(&out?).into_owned(),
			String::from_utf8_lossy(&err?).into_owned(),
		))
	})
	.await
	.context("SSH işleminin süresi doldu")?
}
async fn run_job(job: Uuid, parameters: &SshInstall, path: &std::path::Path) -> Result<String> {
	let mut scan = Command::new("/usr/bin/ssh-keyscan");
	scan.args([
		"-T",
		"5",
		"-p",
		&parameters.port.to_string(),
		"--",
		&parameters.host,
	]);
	let (_, keys, _) = execute(scan, b"", Duration::from_secs(15)).await?;
	let key = keys
		.lines()
		.find(|line| {
			let parts = line.split_whitespace().collect::<Vec<_>>();
			parts.len() == 3
				&& STANDARD.decode(parts[2]).ok().is_some_and(|v| {
					auth::same_token(
						&format!("SHA256:{}", STANDARD_NO_PAD.encode(Sha256::digest(v))),
						&parameters.fingerprint,
					)
				})
		})
		.context("SSH sunucu anahtarı parmak izi eşleşmiyor; bağlantı kurulmadı")?;
	let known_hosts = path.join("known_hosts");
	crypto::write_private(&known_hosts, format!("{key}\n").as_bytes())?;
	let mut ssh = Command::new("/usr/bin/ssh");
	ssh.args([
		"-T",
		"-p",
		&parameters.port.to_string(),
		"-l",
		&parameters.username,
		"-o",
		"ConnectTimeout=10",
		"-o",
		"StrictHostKeyChecking=yes",
		"-o",
		"GlobalKnownHostsFile=/dev/null",
		"-o",
		"UpdateHostKeys=no",
		"-o",
		"NumberOfPasswordPrompts=1",
		"-o",
		"IdentityAgent=none",
		"-o",
		"IdentitiesOnly=yes",
		"-o",
		"ServerAliveInterval=15",
		"-o",
		"ServerAliveCountMax=3",
	]);
	ssh.arg("-o")
		.arg(format!("UserKnownHostsFile={}", known_hosts.display()));
	if !parameters.private_key.is_empty() {
		let key = path.join("identity");
		crypto::write_private(&key, parameters.private_key.as_bytes())?;
		ssh.args([
			"-o",
			"BatchMode=yes",
			"-o",
			"PasswordAuthentication=no",
			"-i",
		])
		.arg(key);
	} else {
		let password = path.join("password");
		crypto::write_private(&password, parameters.password.as_bytes())?;
		let helper = path.join("askpass");
		crypto::write_private(
			&helper,
			b"#!/bin/sh\nexec /bin/cat -- \"${HOODMIN_SSH_PASSWORD_FILE}\"\n",
		)?;
		std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700))?;
		ssh.env("SSH_ASKPASS", helper)
			.env("SSH_ASKPASS_REQUIRE", "force")
			.env("DISPLAY", ":hoodmin")
			.env("HOODMIN_SSH_PASSWORD_FILE", password)
			.args([
				"-o",
				"PubkeyAuthentication=no",
				"-o",
				"PreferredAuthentications=password,keyboard-interactive",
			]);
	}
	let remote = if parameters.username == "root" {
		"bash -s"
	} else if parameters.sudo_password.is_empty() {
		"sudo -n bash -s"
	} else {
		"sudo -S -p '' bash -s"
	};
	ssh.arg("--").arg(&parameters.host).arg(remote);
	let config =
		serde_json::to_string_pretty(&settings(&parameters.origin, &parameters.credential))?;
	let delimiter = format!("HOODMIN_{}", job.simple());
	let script = format!(
		r#"set -euo pipefail
umask 077
if [ "$(id -u)" -ne 0 ]
then
	printf '%s\n' 'Root access is required.' >&2
	exit 1
fi
if ! command -v wget >/dev/null
then
	apt-get update
	DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates wget
fi
install -d -m 0700 /opt/VpnHoodServer/storage
if [ -f /opt/VpnHoodServer/storage/appsettings.json ]
then
	install -m 0600 /opt/VpnHoodServer/storage/appsettings.json "/opt/VpnHoodServer/storage/appsettings.json.hoodmin-$(date +%Y%m%d-%H%M%S)"
fi
cat > /opt/VpnHoodServer/storage/appsettings.json.hoodmin-new <<'{delimiter}'
{config}
{delimiter}
chmod 0600 /opt/VpnHoodServer/storage/appsettings.json.hoodmin-new
mv /opt/VpnHoodServer/storage/appsettings.json.hoodmin-new /opt/VpnHoodServer/storage/appsettings.json
HOODMIN_KURULUM_BETIGI="$(mktemp)"
trap 'rm -f "${{HOODMIN_KURULUM_BETIGI}}"' EXIT
wget --https-only --timeout=60 --tries=3 -qO "${{HOODMIN_KURULUM_BETIGI}}" '{LINUX_INSTALLER}'
bash "${{HOODMIN_KURULUM_BETIGI}}" -q
systemctl enable VpnHoodServer
systemctl restart VpnHoodServer
systemctl is-active --quiet VpnHoodServer
"#
	);
	let input = if parameters.username != "root" && !parameters.sudo_password.is_empty() {
		format!("{}\n{script}", parameters.sudo_password)
	} else {
		script
	};
	let (success, stdout, stderr) =
		execute(ssh, input.as_bytes(), Duration::from_secs(360)).await?;
	let mut log = format!("{stdout}\n{stderr}");
	for secret in [
		&parameters.password,
		&parameters.private_key,
		&parameters.sudo_password,
		&parameters.credential,
	] {
		if !secret.is_empty() {
			log = log.replace(secret, "[gizlendi]");
		}
	}
	let log = log
		.lines()
		.map(|line| {
			if [
				"authorization",
				"secret",
				"password",
				"private key",
				"vh://",
			]
			.iter()
			.any(|word| line.to_ascii_lowercase().contains(word))
			{
				"[hassas satır gizlendi]"
			} else {
				line
			}
		})
		.collect::<Vec<_>>()
		.join("\n");
	ensure!(success, "SSH/kurulum başarısız. {log}");
	Ok(log)
}
pub async fn launch(store: Store, tenant: Uuid, job: Uuid, parameters: SshInstall) {
	static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
	let _slot = match SLOTS.acquire().await {
		Ok(v) => v,
		Err(_) => return,
	};
	let path = PathBuf::from("/run/hoodmin").join(job.to_string());
	let result=async{
		let mut tx=store.begin(tenant).await?;let updated=query("UPDATE install_jobs SET status='Running',started_at=now() WHERE tenant_id=$1 AND id=$2 AND status='Queued'").bind(tenant).bind(job).execute(&mut *tx).await?;ensure!(updated.rows_affected()==1,"İş artık sırada değil");tx.commit().await?;
		std::fs::create_dir(&path)?;std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o700))?;
		run_job(job,&parameters,&path).await
	}.await;
	let _ = std::fs::remove_dir_all(&path);
	let (status, log) = match result {
		Ok(log) => ("Succeeded", log),
		Err(error) => ("Failed", error.to_string()),
	};
	if let Ok(mut tx) = store.begin(tenant).await {
		let updated = query(
			"UPDATE install_jobs SET status=$3,log=$4,finished_at=now() WHERE tenant_id=$1 AND id=$2",
		)
		.bind(tenant)
		.bind(job)
		.bind(status)
		.bind(log)
		.execute(&mut *tx)
		.await;
		if updated.is_ok() {
			let _ = tx.commit().await;
		}
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn generated_commands_quote_shell_and_powershell_values() {
		assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
		assert_eq!(ps_quote("a'b"), "'a''b'");
		let config = settings("https://manager.example", "tenant.server.secret");
		let (linux, windows) = commands(&config);
		assert!(linux.contains(LINUX_INSTALLER) && windows.contains(WINDOWS_INSTALLER));
		assert_eq!(config["AllowAnonymousTracker"], false);
	}
}
