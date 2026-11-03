#!/usr/bin/python3
"""Disposable Debian QA only: real 0.4 -> current dpkg/systemd upgrade and recovery.
Requires existing PostgreSQL 17 tools, OpenSSL and both DEBs; never targets production.
"""
from pathlib import Path
import fcntl,hashlib,http.client,http.cookies,json,os,pwd,secrets,shutil,subprocess,sys,tempfile,time
ROOT=Path(__file__).resolve().parents[1]
OLD=Path(sys.argv[1]);NEW=Path(sys.argv[2]);CLUSTER='hoodmin-upgrade-qa';PORT='55461';HTTP=3110
CONFIG=Path('/etc/hoodmin/hoodmin.json');POLICY=Path('/etc/hoodmin/upgrade.json');STATE=Path('/var/lib/hoodmin-upgrade/state.json')
BACKUPS=Path('/var/backups/hoodmin-upgrade-qa');OWNER=Path('/root/hoodmin-first-owner.json')
CLEAN=[Path('/etc/hoodmin'),Path('/var/lib/hoodmin'),Path('/var/lib/hoodmin-upgrade'),BACKUPS,OWNER]
assert all(not p.exists() for p in CLEAN)
assert subprocess.run(['dpkg-query','-W','-f=${Status}','hoodmin'],capture_output=True).stdout.strip()!=b'install ok installed'
try:pwd.getpwnam('hoodmin');raise AssertionError('Existing system account')
except KeyError:pass
WORK=Path(tempfile.mkdtemp(prefix='hoodmin-upgrade-qa.',dir='/root'));WORK.chmod(0o700)
created=False;holder=None

def check(ok,text):
	assert ok,text
	print('PASS:',text,flush=True)
def run(*args,input=None,ok=True):
	p=subprocess.run([str(x) for x in args],input=input,capture_output=True)
	if ok:assert p.returncode==0,(str(args[0]),p.stderr[-1600:].decode(errors='replace'))
	return p

def private(path,text):path.write_text(text);path.chmod(0o600);return path

def sql(text):return run('runuser','-u','postgres','--','psql','-X','-qAt','-p',PORT,'-d','hoodmin','-v','ON_ERROR_STOP=1',input=text.encode()).stdout.decode().strip()
def cli(*args,ok=True):return run('hoodmin-cli',*args,ok=ok)
def phase():return json.loads(STATE.read_text())['phase']
def wait_phase(want,limit=50):
	for _ in range(limit):
		if STATE.exists() and phase()==want:return
		time.sleep(.25)
	raise AssertionError('Phase not reached: '+want+'; actual '+(phase() if STATE.exists() else 'missing'))
def request(path,data=None,cookie=''):
	c=http.client.HTTPConnection('127.0.0.1',HTTP,timeout=5);headers={'Origin':'https://upgrade-qa.example.invalid','Cookie':cookie,'Accept':'application/json'}
	if data is not None:headers['Content-Type']='application/json'
	c.request('GET' if data is None else 'POST',path,body=None if data is None else json.dumps(data).encode(),headers=headers);r=c.getresponse();body=r.read();code=r.status;headers=r.getheaders();c.close();return code,body,headers

def wait_ready():
	for _ in range(80):
		try:
			if request('/readyz')[0]==200:return
		except OSError:pass
		time.sleep(.25)
	raise AssertionError('HTTP never became ready')

def cookie(headers,name):
	for key,value in headers:
		if key.lower()=='set-cookie':
			parsed=http.cookies.SimpleCookie();parsed.load(value)
			if name in parsed:return name+'='+parsed[name].value
	raise AssertionError('Missing QA cookie')

def login():
	owner=json.loads(OWNER.read_text());code,body,headers=request('/api/manage/login');assert code==200
	nonce=json.loads(body)['csrf'];code,_,headers=request('/api/manage/login',{'email':owner['email'],'password':owner['password'],'csrf':nonce},cookie(headers,'__Host-vhm_login'));assert code in [200,202]
	session=cookie(headers,'__Host-vhm_session');code,body,_=request('/api/manage/me',cookie=session);assert code==200
	return session,json.loads(body)['csrf']
def api(path,data=None):
	if data is not None:data={**data,'csrf':csrf}
	code,body,_=request('/api/manage'+path,data,session);assert code in [200,202],(path,code)
	return json.loads(body)
def backups():return sorted(p for p in BACKUPS.glob('hoodmin-*') if (p/'hoodmin-backup.json').exists()) if BACKUPS.exists() else []
def live_dumps():
	for p in Path('/proc').glob('[0-9]*/stat'):
		try:
			stat=p.read_text()
			if '(pg_dump)' in stat and stat.split(') ',1)[1][0]!='Z':return True
		except OSError:pass
	return False
def policy(admin):
	private(POLICY,json.dumps({'admin_url_file':str(admin) if admin else None,'backup_directory':str(BACKUPS),'attempt_timeout_seconds':60,'retry_seconds':1}))
def restart_worker():
	# Fault injection intentionally starts more often than the normal systemd rate limit.
	run('systemctl','reset-failed','hoodmin-upgrade');run('systemctl','restart','hoodmin-upgrade')
try:
	run('dpkg','--force-confold','-i',OLD)
	check(run('systemctl','is-active','--quiet','hoodmin',ok=False).returncode!=0,'Fresh package does not automatically enable/start the panel')
	run('pg_createcluster','17',CLUSTER,'--port',PORT);created=True
	ca=WORK/'db-ca.crt';run('openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','1','-subj','/CN=Hoodmin upgrade QA CA','-keyout',WORK/'ca.key','-out',ca)
	run('openssl','req','-new','-newkey','rsa:2048','-nodes','-subj','/CN=localhost','-keyout',WORK/'db.key','-out',WORK/'db.csr')
	(WORK/'ext').write_text('subjectAltName=DNS:localhost\n');run('openssl','x509','-req','-days','1','-in',WORK/'db.csr','-CA',ca,'-CAkey',WORK/'ca.key','-CAcreateserial','-extfile',WORK/'ext','-out',WORK/'db.crt')
	pg=Path('/var/lib/postgresql/17')/CLUSTER
	for file in ['db.key','db.crt']:
		shutil.copy2(WORK/file,pg/file);run('chown','postgres:postgres',pg/file);(pg/file).chmod(0o600)
	Path('/etc/postgresql/17',CLUSTER,'conf.d/upgrade-qa.conf').write_text("ssl=on\nssl_cert_file='db.crt'\nssl_key_file='db.key'\n")
	run('pg_ctlcluster','--skip-systemctl-redirect','17',CLUSTER,'start')
	password=secrets.token_urlsafe(28);app_password=secrets.token_urlsafe(28)
	run('runuser','-u','postgres','--','psql','-X','-q','-p',PORT,'-d','postgres','-v','ON_ERROR_STOP=1',input=f"ALTER ROLE postgres PASSWORD '{password}';".encode())
	shutil.copy2(ca,CONFIG.parent/'qa-ca.crt');(CONFIG.parent/'qa-ca.crt').chmod(0o644)
	tail=f'?sslmode=verify-full&sslrootcert={CONFIG.parent}/qa-ca.crt'
	app=private(WORK/'app.url',f'postgresql://hoodmin:{app_password}@localhost:{PORT}/hoodmin{tail}')
	admin=private(WORK/'admin.url',f'postgresql://postgres:{password}@localhost:{PORT}/postgres{tail}')
	admin_target=private(WORK/'upgrade-admin.url',admin.read_text().replace('/postgres?','/hoodmin?'))
	cli('config','set','--database-url-file',app,'--public-origin','https://upgrade-qa.example.invalid','--listen',f'127.0.0.1:{HTTP}')
	cli('database','init','--admin-url-file',admin)
	cli('owner-init','--email','upgrade-qa@example.invalid')
	run('systemctl','enable','--now','hoodmin');wait_ready();session,csrf=login()
	run('systemctl','disable','hoodmin')
	sid=api('/servers',{'name':'Preserved server','host':'vpn.example.invalid','port':443,'farm':''})['id']
	api('/tokens',{'name':'Preserved key','server':sid,'count':1,'max_clients':1,'max_gib':1,'expires':''})
	tid=api('/tokens')['items'][0]['id'];key=api('/tokens/'+tid+'/key')['value']
	master=Path('/var/lib/hoodmin/master.key').read_bytes();before=json.loads(CONFIG.read_text())
	check(sql('SELECT count(*) FROM _hoodmin_migrations')=='13','Old package has the actual 0.4 schema')
	policy(admin_target)
	run('pg_ctlcluster','--skip-systemctl-redirect','17',CLUSTER,'stop')
	start=time.monotonic();run('env','DEBIAN_FRONTEND=noninteractive','dpkg','--force-confold','-i',NEW);elapsed=time.monotonic()-start
	check(elapsed<50,'Real noninteractive dpkg upgrade completes while remote database is offline: '+str(round(elapsed,1))+'s')
	wait_phase('waiting_database')
	for _ in range(40):
		try:
			if request('/healthz')[0]==200:break
		except OSError:pass
		time.sleep(.1)
	check(request('/')[0]==200 and request('/readyz')[0]==503,'HTTP shell stays available; API readiness is held during migration')
	check(run('systemctl','is-active','--quiet','hoodmin').returncode==0,'Previously running panel is restarted automatically')
	check(run('systemctl','is-enabled','--quiet','hoodmin',ok=False).returncode!=0,'Disabled-but-running panel resumes without changing its enable preference')
	check(Path('/var/lib/hoodmin/master.key').read_bytes()==master and json.loads(CONFIG.read_text())==before,'Package preserves exact master key and customized configuration')
	check(not backups(),'Database outage creates no false completed backup')
	run('pg_ctlcluster','--skip-systemctl-redirect','17',CLUSTER,'start');wait_phase('ready');wait_ready()
	check(sql('SELECT count(*) FROM _hoodmin_migrations')=='14','Worker migrates automatically when verified remote TLS connection returns')
	session,csrf=login();check(api('/servers')['items'][0]['id']==sid and api('/tokens/'+tid+'/key')['value']==key,'Accounts, server UUID and VPN access-key secret survive real upgrade')
	check(len(backups())==1 and (backups()[0]/'hoodmin-backup.json').exists(),'Exactly one complete migration backup is retained')
	proof=json.loads((backups()[0]/'upgrade-provenance.json').read_text());check(proof['from_version'].startswith('0.4.0') and proof['target_version']=='0.5.0','Backup identifies the source and target releases')
	applied=sql("SELECT applied_at FROM _hoodmin_migrations WHERE name='0014_upgrade.sql'")
	run('systemctl','restart','hoodmin');wait_phase('ready');wait_ready()
	check(len(backups())==1 and sql("SELECT applied_at FROM _hoodmin_migrations WHERE name='0014_upgrade.sql'")==applied,'Restart checks again without repeating completed migrations/backups')
	# Local maintenance lock: no marker-file deletion is required to recover.
	lock=Path('/var/lib/hoodmin/master.maintenance.lock').open('rb');fcntl.flock(lock,fcntl.LOCK_EX)
	cli('upgrade','--request');restart_worker();time.sleep(1.5)
	check(phase()=='pending','Concurrent local maintenance defers upgrade without corrupting state')
	lock.close();wait_phase('ready');wait_ready()
	# Restart after a killed process; reuse completed backup while the same plan is pending.
	run('systemctl','stop','hoodmin-upgrade')
	sql("DELETE FROM _hoodmin_migrations WHERE name='0014_upgrade.sql'")
	env=dict(os.environ,PGAPPNAME='hoodmin-upgrade-test-lock')
	holder=subprocess.Popen(['runuser','-u','postgres','--preserve-environment','--','psql','-X','-qAt','-p',PORT,'-d','hoodmin'],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,env=env)
	holder.stdin.write(b'SELECT pg_advisory_lock(684663002); SELECT pg_sleep(120);\n');holder.stdin.close()
	for _ in range(30):
		if sql("SELECT count(*) FROM pg_locks l JOIN pg_stat_activity a USING(pid) WHERE a.application_name='hoodmin-upgrade-test-lock' AND l.locktype='advisory' AND l.granted")=='1':break
		time.sleep(.1)
	cli('upgrade','--request');restart_worker();wait_phase('waiting_database');count=len(backups());time.sleep(2)
	check(count==2 and len(backups())==count,'Database lock defers apply and reuses a verified backup across retries')
	run('systemctl','kill','--signal=KILL','--kill-whom=all','hoodmin-upgrade')
	sql("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='hoodmin-upgrade-test-lock'")
	holder.wait(timeout=8);holder=None
	restart_worker();wait_phase('ready');wait_ready();check(len(backups())==count,'Killed worker resumes safely without duplicate backup or stale lock')
	# Wrong metadata is a real failure, not a false successful deferred operation.
	good=sql("SELECT checksum FROM _hoodmin_migrations WHERE name='0014_upgrade.sql'")
	sql("UPDATE _hoodmin_migrations SET checksum='wrong' WHERE name='0014_upgrade.sql'")
	cli('upgrade','--request');restart_worker();wait_phase('failed')
	check(cli('upgrade','--wait','1',ok=False).returncode!=0,'Corrupted migration history is visible and fails the package wait command')
	broken=run('env','DEBIAN_FRONTEND=noninteractive','dpkg','--force-confold','-i',NEW,ok=False)
	check(broken.returncode!=0,'Permanent migration failure is not reported as successful package configuration')
	for _ in range(40):
		try:
			if request('/healthz')[0]==200:break
		except OSError:pass
		time.sleep(.1)
	check(request('/readyz')[0]==503 and request('/')[0]==200,'Permanent package failure still resumes the HTTP shell with readiness closed')
	sql("UPDATE _hoodmin_migrations SET checksum='"+good+"' WHERE name='0014_upgrade.sql'");restart_worker();wait_phase('ready');wait_ready()
	run('env','DEBIAN_FRONTEND=noninteractive','dpkg','--configure','hoodmin');wait_phase('ready');wait_ready()
	# Remote administrator credentials remain protected from the web account.
	check(run('runuser','-u','hoodmin','--','cat',POLICY,ok=False).returncode!=0,'Web service cannot read upgrade administrator policy')
	check(run('runuser','-u','hoodmin','--','hoodmin-cli','upgrade',ok=False).returncode!=0,'Web account cannot invoke the privileged coordinator')
	policy(None);cli('upgrade','--request');restart_worker();wait_phase('needs_admin')
	cli('upgrade','--configure','--admin-url-file',admin_target);restart_worker();wait_phase('ready');wait_ready()
	# A hung pg_dump must time out together with its descendants, without a completed marker.
	run('systemctl','stop','hoodmin-upgrade')
	sql("DELETE FROM _hoodmin_migrations WHERE name='0014_upgrade.sql'")
	holder=subprocess.Popen(['runuser','-u','postgres','--preserve-environment','--','psql','-X','-qAt','-p',PORT,'-d','hoodmin'],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,env=env)
	holder.stdin.write(b'BEGIN; LOCK TABLE audit_events IN ACCESS EXCLUSIVE MODE; SELECT pg_sleep(120);\n');holder.stdin.close()
	for _ in range(30):
		if sql("SELECT count(*) FROM pg_locks l JOIN pg_stat_activity a USING(pid) WHERE a.application_name='hoodmin-upgrade-test-lock' AND l.mode='AccessExclusiveLock' AND l.granted")=='1':break
		time.sleep(.1)
	p=json.loads(POLICY.read_text());p['attempt_timeout_seconds']=1;p['retry_seconds']=5;private(POLICY,json.dumps(p))
	cli('upgrade','--request');restart_worker();wait_phase('waiting_database',100)
	run('systemctl','stop','hoodmin-upgrade');time.sleep(.25)
	check(len(backups())==count and not live_dumps(),'Timed-out backup kills pg_dump descendants and never marks an incomplete dump complete')
	sql("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='hoodmin-upgrade-test-lock'");holder.wait(timeout=8);holder=None
	policy(admin_target);restart_worker();wait_phase('ready');wait_ready()
	check(len(backups())==count+1,'Upgrade recovers after the blocked backup becomes available');count=len(backups())
	# Exercise the native local-peer worker, including a real migration and backup.
	local=private(WORK/'local.url',f'postgresql:///hoodmin?host=/var/run/postgresql&port={PORT}&user=hoodmin')
	cli('config','set','--database-url-file',local,'--public-origin','https://upgrade-qa.example.invalid','--listen',f'127.0.0.1:{HTTP}');policy(None)
	sql("DELETE FROM _hoodmin_migrations WHERE name='0014_upgrade.sql'; REVOKE ALL ON _hoodmin_migrations FROM hoodmin;")
	cli('upgrade','--request');restart_worker();wait_phase('ready');wait_ready()
	check(len(backups())==count+1,'Local peer migration and protected backup use native Rust/PostgreSQL workers')
	# Administrator-stopped services stay stopped during package reconfiguration.
	run('systemctl','stop','hoodmin')
	run('env','DEBIAN_FRONTEND=noninteractive','dpkg','--force-confold','-i',NEW)
	check(run('systemctl','is-active','--quiet','hoodmin',ok=False).returncode!=0,'Package reconfiguration preserves deliberately stopped service')
	run('systemctl','start','hoodmin');wait_phase('ready');wait_ready()
	logs=run('journalctl','-u','hoodmin-upgrade','--since','1 hour ago','--no-pager').stdout.decode(errors='replace')
	check(password not in logs and app_password not in logs,'Coordinator logs do not disclose database passwords')
	print('PASS: Real package upgrade, deferred recovery, locking, failure and restart tests complete',flush=True)
finally:
	if holder:
		run('runuser','-u','postgres','--','psql','-X','-qAt','-p',PORT,'-d','postgres',input=b"SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='hoodmin-upgrade-test-lock'",ok=False)
		holder.terminate();holder.wait(timeout=5)
	run('systemctl','stop','hoodmin-upgrade','hoodmin',ok=False)
	run('dpkg','--purge','hoodmin',ok=False)
	if created:
		run('pg_ctlcluster','--skip-systemctl-redirect','17',CLUSTER,'stop',ok=False);run('pg_dropcluster','17',CLUSTER,ok=False)
	for p in CLEAN:
		if p.is_dir():shutil.rmtree(p)
		else:p.unlink(missing_ok=True)
	try:pwd.getpwnam('hoodmin')
	except KeyError:pass
	else:run('userdel','hoodmin',ok=False);run('groupdel','hoodmin',ok=False)
	run('systemctl','daemon-reload',ok=False)
	shutil.rmtree(WORK)
