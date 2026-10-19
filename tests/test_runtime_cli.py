#!/usr/bin/python3
"""Fresh, isolated QA only: native CLI, verified PostgreSQL TLS and HTTP recovery.
Never run on a configured panel. Requires existing Debian QA tools and certificates.
"""
import hashlib
import json
import os
from pathlib import Path
import pwd
import secrets
import shutil
import signal
import ssl
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
CLUSTER = 'hoodmin-runtime-qa'
PORT = '55440'
ORIGIN = 'https://yapayzeka:3459'
CONFIG = Path('/etc/hoodmin/hoodmin.json')
OWNER = Path('/root/hoodmin-first-owner.json')
PATHS = [Path('/etc/hoodmin'), Path('/var/lib/hoodmin'), Path('/usr/share/hoodmin')]
BINARIES = [Path('/usr/bin/hoodmin'), Path('/usr/bin/hoodmin-cli')]
assert Path('/etc/vpnhood-manager/tls/ca.crt').exists(), 'Dedicated QA certificates required'
assert all(not p.exists() for p in [*PATHS, *BINARIES, OWNER]), 'Use an unused QA installation'
assert not Path('/etc/postgresql/17', CLUSTER).exists()
try:
	pwd.getpwnam('hoodmin')
	raise AssertionError('Test system account already exists')
except KeyError:
	pass
WORK = Path(tempfile.mkdtemp(prefix='runtime-cli.', dir=ROOT / 'work'))
WORK.chmod(0o700)
CONTEXT = ssl.create_default_context(cafile='/etc/vpnhood-manager/tls/ca.crt')
ENV = dict(os.environ, SSL_CERT_FILE='/etc/vpnhood-manager/tls/ca.crt')
ENV.pop('DATABASE_URL', None)
ENV.pop('VHM_MASTER_KEY_FILE', None)
LOG = WORK / 'service.log'
PROCESSES = []
CREATED = False

def check(value, message):
	assert value, message
	print('PASS:', message, flush=True)

def run(args, expected=0, **kwargs):
	result = subprocess.run([str(a) for a in args], text=True, capture_output=True, **kwargs)
	assert (result.returncode == 0) == (expected == 0), (args[0:3], result.stderr[-1500:])
	return result

def private(path, text):
	path.write_text(text)
	path.chmod(0o600)
	return path

def cli(*args, expected=0):
	return run([BINARIES[1], '--config', CONFIG, *args], expected=expected, env=ENV)

def request(path, data=None, headers=None):
	try:
		response = urllib.request.urlopen(urllib.request.Request(ORIGIN + path, data=data, headers=headers or {}), context=CONTEXT, timeout=8)
	except urllib.error.HTTPError as error:
		response = error
	with response:
		return response.status, response.read().decode()

def wait_ready(ready=True):
	for _ in range(35):
		try:
			code, _ = request('/readyz')
			if (code == 200) == ready:
				return
		except OSError:
			pass
		time.sleep(.5)
	raise AssertionError('Readiness transition timed out')

def api(*args, expected=0, bot=None):
	prefix = ['api', '--origin', ORIGIN, '--session-file', str(WORK / ('bot-session.json' if bot else 'session.json'))]
	if bot:
		prefix += ['--token-file', str(bot)]
	return cli(*prefix, *args, expected=expected)

def sql(text):
	return run(['runuser', '-u', 'postgres', '--', 'psql', '-X', '-p', PORT, '-d', 'postgres', '-v', 'ON_ERROR_STOP=1'], input=text).stdout

try:
	for path in PATHS:
		path.mkdir(mode=0o755)
	shutil.copytree(ROOT / 'migrations', PATHS[2] / 'migrations')
	for binary in BINARIES:
		shutil.copy2(ROOT / 'target/release' / binary.name, binary)
	shutil.copy2(ROOT / 'deploy/hoodmin.json', CONFIG)
	run(['sh', ROOT / 'debian/hoodmin.postinst', 'configure', ''])
	check('postgresql' not in next(l for l in (ROOT / 'debian/control').read_text().splitlines() if l.startswith('Depends:')), 'No PostgreSQL server/client package dependency')
	log = LOG.open('wb')
	server = subprocess.Popen(['runuser', '-u', 'hoodmin', '--', str(BINARIES[0]), '--config', str(CONFIG), '--listen', '127.0.0.1:3108'], stdout=log, stderr=log, start_new_session=True)
	PROCESSES.append(server)
	nginx = WORK / 'nginx.conf'
	nginx.write_text(f'''pid {WORK}/nginx.pid;
error_log {WORK}/nginx.log;
events {{}}
http {{
	access_log off;
	server {{
		listen 3459 ssl;
		server_name yapayzeka;
		ssl_certificate /etc/vpnhood-manager/tls/manager.crt;
		ssl_certificate_key /etc/vpnhood-manager/tls/manager.key;
		location / {{
			proxy_pass http://127.0.0.1:3108;
			proxy_http_version 1.1;
			proxy_set_header Connection "";
			proxy_buffering off;
			proxy_read_timeout 60s;
			proxy_set_header Host $host;
			proxy_set_header X-Real-IP $remote_addr;
			proxy_set_header X-Forwarded-For $remote_addr;
		}}
	}}
}}
''')
	run(['nginx', '-t', '-c', nginx])
	proxy = subprocess.Popen(['nginx', '-c', str(nginx), '-g', 'daemon off;'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
	PROCESSES.append(proxy)
	wait_ready(False)
	check(request('/healthz')[0] == 200 and request('/signin')[0] == 503 and request('/assets/app.css')[0] == 200, 'Unconfigured service stays alive and serves a simple static error')
	check('database_url' not in request('/signin')[1] and '/var/' not in request('/signin')[1], 'Browser error does not disclose configuration details')

	run(['pg_createcluster', '17', CLUSTER, '--port', PORT])
	CREATED = True
	ca = WORK / 'db-ca.crt'
	run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1', '-subj', '/CN=Hoodmin QA CA', '-keyout', WORK / 'db-ca.key', '-out', ca])
	run(['openssl', 'req', '-new', '-newkey', 'rsa:2048', '-nodes', '-subj', '/CN=localhost', '-keyout', WORK / 'db.key', '-out', WORK / 'db.csr'])
	(WORK / 'extensions').write_text('subjectAltName=DNS:localhost\n')
	run(['openssl', 'x509', '-req', '-days', '1', '-in', WORK / 'db.csr', '-CA', ca, '-CAkey', WORK / 'db-ca.key', '-CAcreateserial', '-extfile', WORK / 'extensions', '-out', WORK / 'db.crt'])
	pgdata = Path('/var/lib/postgresql/17') / CLUSTER
	for name in ['db.key', 'db.crt']:
		shutil.copy2(WORK / name, pgdata / name)
		run(['chown', 'postgres:postgres', pgdata / name])
		(pgdata / name).chmod(0o600)
	Path('/etc/postgresql/17', CLUSTER, 'conf.d/hoodmin-qa.conf').write_text(f"ssl=on\nssl_cert_file='{pgdata}/db.crt'\nssl_key_file='{pgdata}/db.key'\n")
	run(['pg_ctlcluster', '--skip-systemctl-redirect', '17', CLUSTER, 'start'])
	admin_password, app_password = secrets.token_urlsafe(24), secrets.token_urlsafe(24)
	sql(f"ALTER ROLE postgres PASSWORD '{admin_password}';")
	shutil.copy2(ca, PATHS[0] / 'db-ca.crt')
	(PATHS[0] / 'db-ca.crt').chmod(0o644)
	query = f'?sslmode=verify-full&sslrootcert={PATHS[0]}/db-ca.crt'
	app_url = f'postgresql://hoodmin:{app_password}@localhost:{PORT}/hoodmin{query}'
	admin_url = f'postgresql://postgres:{admin_password}@localhost:{PORT}/postgres{query}'
	app_file = private(WORK / 'app.url', app_url)
	admin_file = private(WORK / 'admin.url', admin_url)
	cli('config', 'set', '--database-url-file', app_file, '--public-origin', ORIGIN, '--listen', '127.0.0.1:3108')
	check(app_password not in cli('config', 'show').stdout, 'CLI configuration output redacts the database password')
	cli('database', 'init', '--admin-url-file', admin_file)
	cli('database', 'init', '--admin-url-file', admin_file)
	cli('database', 'check')
	local_config = json.loads(CONFIG.read_text())
	local_config['database_url'] = f'postgresql:///hoodmin_local?host=/var/run/postgresql&port={PORT}&user=hoodmin'
	local_file = private(PATHS[0] / 'local-test.json', json.dumps(local_config))
	run(['chown', 'root:hoodmin', local_file])
	local_file.chmod(0o640)
	run([BINARIES[1], '--config', local_file, 'database', 'init', '--local'], env=ENV)
	run(['runuser', '-u', 'hoodmin', '--', BINARIES[1], '--config', local_file, 'database', 'check'], env=ENV)
	check(True, 'Local Unix socket bootstrap and peer role also work through Rust CLI')
	check(True, 'Verified remote PostgreSQL TLS, role/database creation and idempotent migrations')
	cli('owner-init', '--email', 'runtime-qa@example.invalid')
	owner = json.loads(OWNER.read_text())
	password_file = private(WORK / 'owner-password', owner['password'])
	wait_ready()
	check(server.poll() is None and request('/signin')[0] == 200, 'Service recovers automatically after configuration and database preparation')

	api('login', '--email', owner['email'], '--password-file', password_file)
	check(json.loads(api('whoami').stdout)['role'] == 'owner', 'Native CLI human login preserves owner role')
	form = private(WORK / 'server.json', json.dumps(dict(name='Runtime QA', host='vpn.example.invalid', port=443, farm='')))
	created = json.loads(api('post', '/servers', '--data-file', form).stdout)
	sid = created['id']
	check(len(json.loads(api('get', '/servers').stdout)['items']) == 1, 'CLI uses shared server creation/list API')
	for path in ['/dashboard', '/panel/audit', '/panel/account', '/reports?hours=24', f'/servers/{sid}/configuration', f'/servers/{sid}/routing', f'/servers/{sid}/certificates', f'/servers/{sid}/installation']:
		json.loads(api('get', path).stdout)
	check(True, 'Dashboard, audit, account, report and server detail operations return JSON')
	session = json.loads((WORK / 'session.json').read_text())
	code, _ = request('/api/manage/tenant', json.dumps({'name': 'Blocked', 'csrf': session['csrf']}).encode(), {'Content-Type': 'application/json', 'Accept': 'application/json', 'Cookie': '__Host-vhm_session=' + session['token'], 'Origin': 'https://invalid.example'})
	check(code == 403, 'Human JSON API rejects wrong Origin')
	code, _ = request('/api/manage/tenant', json.dumps({'name': 'Blocked'}).encode(), {'Content-Type': 'application/json', 'Accept': 'application/json', 'Cookie': '__Host-vhm_session=' + session['token'], 'Origin': ORIGIN})
	check(code == 403, 'Human JSON API rejects missing CSRF')
	bot_form = private(WORK / 'bot.json', json.dumps(dict(name='Runtime viewer', role='viewer', lifetime_days=1, confirmed=True)))
	bot = json.loads(api('post', '/bots', '--data-file', bot_form).stdout)
	bot_file = private(WORK / 'bot-token', bot['value'])
	api('get', '/servers', bot=bot_file)
	api('post', '/servers', '--data-file', form, bot=bot_file, expected=1)
	check(True, 'Bot CLI access respects the existing read-only role')

	bad = private(WORK / 'bad.url', app_url.replace('sslmode=verify-full', 'sslmode=disable'))
	before = CONFIG.read_bytes()
	cli('config', 'set', '--database-url-file', bad, '--public-origin', ORIGIN, expected=1)
	check(CONFIG.read_bytes() == before, 'Unverified TCP configuration is rejected without replacing settings')
	wrong = json.loads(CONFIG.read_text())
	wrong['database_url'] = app_url.replace('@localhost:', '@127.0.0.1:')
	bad_config = private(WORK / 'wrong-name.json', json.dumps(wrong))
	run([BINARIES[1], '--config', bad_config, 'database', 'check'], expected=1, env=ENV)
	check(True, 'Wrong database certificate hostname is rejected')

	original_pid = server.pid
	run(['pg_ctlcluster', '--skip-systemctl-redirect', '17', CLUSTER, 'stop'])
	wait_ready(False)
	check(server.poll() is None and server.pid == original_pid and request('/healthz')[0] == 200 and request('/signin')[0] == 503, 'Database outage does not terminate/restart HTTP service')
	run(['pg_ctlcluster', '--skip-systemctl-redirect', '17', CLUSTER, 'start'])
	wait_ready()
	api('get', '/servers')
	check(True, 'Same process and human session recover after database restart')
	regression_env = dict(ENV, VHM_TEST_ISOLATED='1', VHM_TEST_ORIGIN=ORIGIN, VHM_TEST_CA='/etc/vpnhood-manager/tls/ca.crt', VHM_TEST_DATABASE_URL=app_url, VHM_TEST_AGENT_ORIGIN='http://127.0.0.1:3108', PGPORT=PORT)
	for script in ([] if os.environ.get('HOODMIN_TEST_SKIP_REGRESSION') else ['test_preferences.py', 'test_panel.py', 'test_team_bots.py', 'test_contract_features.py', 'test_profiles_reports.py']):
		result = subprocess.run(['python3', str(ROOT / 'tests' / script)], env=regression_env, capture_output=True, text=True)
		if result.returncode:
			print(result.stdout[-2500:], result.stderr[-2000:], flush=True)
		check(result.returncode == 0, 'Existing isolated regression: ' + script)
	check(app_password not in LOG.read_text() and admin_password not in LOG.read_text(), 'Server log retains diagnostics without database passwords')

	backup_admin = private(WORK / 'backup-admin.url', admin_url.replace('/postgres?', '/hoodmin?'))
	backup = Path(cli('backup', 'create', '--admin-url-file', backup_admin, '--directory', WORK / 'backups').stdout.strip())
	check((backup / 'database.dump').stat().st_size > 0 and (backup / 'configuration/db-ca.crt').exists() and (backup / 'master.key').read_bytes() == Path('/var/lib/hoodmin/master.key').read_bytes(), 'Remote native CLI backup includes database, config CA and exact master key')
	identity = WORK / 'recovery.agekey'
	run(['age-keygen', '-o', identity])
	identity.chmod(0o600)
	recipient = run(['age-keygen', '-y', identity]).stdout.strip()
	recipient_file = private(WORK / 'recipient.txt', recipient)
	cli('backup', 'export', '--admin-url-file', backup_admin, '--recipient-file', recipient_file, '--directory', WORK / 'backups')
	archive = WORK / 'backup.tar'
	run(['age', '--decrypt', '--identity', identity, '--output', archive, WORK / 'backups/latest.age'])
	archive.chmod(0o600)
	import tarfile
	with tarfile.open(archive) as exported:
		check(exported.extractfile('./master.key').read() == Path('/var/lib/hoodmin/master.key').read_bytes() and exported.extractfile('./database.dump').read(5) == b'PGDMP', 'Native CLI encrypted export decrypts to the exact key and valid PostgreSQL dump')
	api('logout')
	check(not (WORK / 'session.json').exists(), 'CLI logout revokes and removes its human session')
finally:
	for process in reversed(PROCESSES):
		if process.poll() is None:
			os.killpg(process.pid, signal.SIGTERM)
			try:
				process.wait(timeout=15)
			except subprocess.TimeoutExpired:
				os.killpg(process.pid, signal.SIGKILL)
				process.wait()
	if CREATED:
		subprocess.run(['pg_ctlcluster', '--skip-systemctl-redirect', '17', CLUSTER, 'stop'], capture_output=True)
		run(['pg_dropcluster', '17', CLUSTER])
		custom = Path('/etc/postgresql/17', CLUSTER, 'conf.d/hoodmin-qa.conf')
		custom.unlink(missing_ok=True)
		if custom.parent.exists():
			custom.parent.rmdir()
		if custom.parent.parent.exists():
			custom.parent.parent.rmdir()
	for path in [OWNER, *BINARIES]:
		path.unlink(missing_ok=True)
	for path in PATHS:
		if path.exists():
			shutil.rmtree(path)
	try:
		pwd.getpwnam('hoodmin')
	except KeyError:
		pass
	else:
		run(['userdel', 'hoodmin'])
		subprocess.run(['groupdel', 'hoodmin'], capture_output=True)
	shutil.rmtree(WORK)
print('PASS: Runtime/CLI isolated integration complete; existing services unchanged.')
