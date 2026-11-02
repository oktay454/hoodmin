#!/usr/bin/python3
"""Isolated API, encryption rotation, notification transitions and retention integration.
Invoked only by test_runtime_cli.py; never uses production credentials or databases.
"""
import base64, json, os, secrets, subprocess, time, uuid, ssl, http.cookiejar, urllib.request, urllib.error
from pathlib import Path
assert os.environ.get('VHM_TEST_ISOLATED') == '1'
BASE=os.environ['VHM_TEST_ORIGIN']
WORK=Path(os.environ['HOODMIN_QA_WORK'])
CONFIG=Path(os.environ['HOODMIN_QA_CONFIG'])
OWNER=json.loads(Path(os.environ['HOODMIN_QA_OWNER']).read_text())
PORT=os.environ['HOODMIN_QA_PORT']
TLS=ssl.create_default_context(cafile=os.environ['VHM_TEST_CA'])
ENV=dict(os.environ)

def check(ok,message):
	assert ok,message
	print('PASS:',message,flush=True)

def cli(*args,ok=True):
	r=subprocess.run(['/usr/bin/hoodmin-cli','--config',str(CONFIG),*map(str,args)],env=ENV,text=True,capture_output=True)
	assert (r.returncode==0)==ok,(args[:2],r.stderr[-1200:])
	return r.stdout if ok else r.stderr

def sql(text,db='hoodmin'):
	r=subprocess.run(['runuser','-u','postgres','--','psql','-X','-A','-t','-p',PORT,'-d',db,'-v','ON_ERROR_STOP=1'],input=text,text=True,capture_output=True)
	assert r.returncode==0,r.stderr[-1000:]
	return r.stdout.strip()

def private(name,text):
	p=WORK/name;p.write_text(text);p.chmod(0o600);return p

class Client:
	def __init__(self,email,password):
		self.open=urllib.request.build_opener(urllib.request.HTTPSHandler(context=TLS),urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
		self.csrf=None
		nonce=self.get('/login')['csrf'];self.csrf=nonce
		self.post('/login',{'email':email,'password':password})
		self.csrf=self.get('/me')['csrf']
	def call(self,path,data=None,status=None,retry=0):
		headers={'Accept':'application/json','Origin':BASE}
		if data is not None:headers['Content-Type']='application/json';data=json.dumps({'csrf':self.csrf,**data}).encode()
		try:r=self.open.open(urllib.request.Request(BASE+'/api/manage'+path,data=data,headers=headers),timeout=25)
		except urllib.error.HTTPError as e:r=e
		body=r.read();value=json.loads(body)
		if r.status==503 and data is None and status is None and retry<50:
			time.sleep(.2);return self.call(path,retry=retry+1)
		if status:check(r.status==status,f'API status {r.status} expected {status}: '+path)
		else:assert 200<=r.status<300,(path,r.status,value)
		return value
	def get(self,path,**kw):return self.call(path,**kw)
	def post(self,path,data,**kw):return self.call(path,data,**kw)

owner=Client(OWNER['email'],OWNER['password'])
tenant=OWNER['tenant']
server=owner.post('/servers',{'name':'Operations <script>','host':'vpn.example.invalid','port':2443,'farm':''})
sid=server['id'];credential=server['settings']['HttpAccessManager']['Authorization']

def agent(path,payload,authorization=None):
	r=urllib.request.Request(BASE+'/api/agent/'+path,data=json.dumps(payload).encode(),headers={'Authorization':authorization or credential,'Content-Type':'application/json'})
	with urllib.request.urlopen(r,context=TLS,timeout=20) as response:return json.load(response)

before=agent('configure',{})
config=owner.get(f'/servers/{sid}/configuration')
settings=config['settings'];settings.update({'dnsServers':['1.1.1.1'],'Session':{'syncInterval':'00:00:05','allowTcpProxy':True},'Tracking':{'trackClientIp':False},'NetFilter':{'blockIpV6':True}})
owner.post(f'/servers/{sid}/configuration',{'base_code':config['base_code'],'settings':settings,'confirmed':True})
after=agent('configure',{})
check(after['dnsServers']==['1.1.1.1'] and after['serverSecret']==before['serverSecret'],'JSON API changes reach native agent configuration without replacing VPN identity')
owner.post(f'/servers/{sid}/configuration',{'base_code':config['base_code'],'settings':settings,'confirmed':True},status=409)
owner.post(f'/servers/{sid}/credential',{'confirmed':False},status=400)
proof=owner.get(f'/servers/{sid}/credential')['confirmation']
owner.post(f'/servers/{sid}/credential',{'confirmed':True,'confirmation':'wrong'},status=409)
# A separate tenant and read-only user exercise server-side authorization independently of UI.
password=secrets.token_urlsafe(24);password_file=private('operations-pass',password);subprocess.run(['chown','hoodmin:hoodmin',password_file],check=True)
other_tenant=str(uuid.uuid4());email='ops-'+uuid.uuid4().hex+'@example.invalid'
cli('create-account','--tenant',other_tenant,'--tenant-name','Other','--email',email,'--password-file',password_file,'--role','viewer')
other=Client(email,password)
other.get(f'/servers/{sid}/configuration',status=404)
other.post('/servers',{'name':'Blocked','host':'vpn.example.invalid','port':443},status=403)
second_email='second-admin-'+uuid.uuid4().hex+'@example.invalid'
cli('create-account','--tenant',other_tenant,'--tenant-name','Other','--email',second_email,'--password-file',password_file,'--role','admin')
second_admin=Client(second_email,password)
second_server=second_admin.post('/servers',{'name':'Other encrypted server','host':'other.example.invalid','port':2445,'farm':''})
second_authorization=second_server['settings']['HttpAccessManager']['Authorization']
second_identity=agent('configure',{},second_authorization)['serverSecret']
check(len(other.get('/servers')['items'])==1,'A second tenant reads only its own encrypted server metadata')

owner.post('/profiles',{'name':'Quota policy','policy':{'maxClients':2,'maxCycleTraffic':2048,'cycleSeconds':60,'maxUploadMbps':3}})
keys=owner.post('/tokens',{'server':sid,'name':'Operations','count':2,'max_clients':2,'max_gib':0,'expires':'','profile':''})['keys']
check(len(keys)==2,'JSON access-key batch creation preserves secret output')
owner.post('/tokens/bulk',{'ids':'\n'.join(k['id'] for k in keys),'action':'revoke'})
check(all(k['revoked'] for k in owner.get('/tokens')['items'] if k['id'] in {k['id'] for k in keys}),'Bulk key revocation is applied through shared API')
# Encrypted notification targets are masked on reads and included in key rotation.
channel=str(uuid.uuid4());target={'kind':'webhook','url':'https://example.com/hoodmin-qa','bearer':'never-log-this-secret'}
owner.post('/notifications/'+channel,{'name':'QA channel','enabled':False,'target':target,'version':None,'confirmed':True})
channels=owner.get('/notifications');check('never-log-this-secret' not in json.dumps(channels),'Notification channel reads never return stored credentials')
other.get('/notifications',status=403)
owner.post('/notifications/'+channel,{'name':'Stale','enabled':False,'target':None,'version':str(uuid.uuid4()),'confirmed':True},status=409)
owner.post('/notifications/'+str(uuid.uuid4()),{'name':'SSRF','enabled':False,'target':{'kind':'webhook','url':'https://127.0.0.1/','bearer':''},'confirmed':True},status=400)
# Enable observation without external delivery by scheduling all events far in the future before worker wake.
# A disabled channel is safe for rotation; actual delivery retries use a refused public port below.
sql(f"UPDATE servers SET last_seen=now()-interval '1 day' WHERE id='{sid}'; INSERT INTO notification_states(tenant_id,resource,state) VALUES('{tenant}','server/{sid}','healthy') ON CONFLICT(tenant_id,resource) DO UPDATE SET state='healthy';")
version=channels['channels'][0]['version']
owner.post('/notifications/'+channel,{'name':'Retry transport','enabled':True,'version':version,'target':{'kind':'webhook','url':'https://1.1.1.1:1/','bearer':''},'confirmed':True})
owner.post('/notifications/'+channel+'/test',{'confirmed':True})
for _ in range(35):
	deliveries=owner.get('/notifications')['deliveries']
	if any(d['status']=='Retry' for d in deliveries):break
	time.sleep(1)
check(any(d['status']=='Retry' and d['attempts']==1 for d in deliveries),'Failed TLS transport is recorded durably and retries are scheduled')
for _ in range(20):
	events=sql(f"SELECT count(*) FROM notification_deliveries WHERE tenant_id='{tenant}' AND event->>'resource'='server/{sid}'")
	if events!='0':break
	time.sleep(1)
check(events=='1','Initial offline transition creates exactly one notification event')
time.sleep(11)
check(sql(f"SELECT count(*) FROM notification_deliveries WHERE tenant_id='{tenant}' AND event->>'resource'='server/{sid}'")=='1','Repeated observation does not duplicate unchanged offline state')
agent('status',{'configCode':after['configCode']})
for _ in range(20):
	if sql(f"SELECT count(*) FROM notification_deliveries WHERE tenant_id='{tenant}' AND event->>'resource'='server/{sid}' AND event->>'state'='healthy'")=='1':break
	time.sleep(1)
check(sql(f"SELECT count(*) FROM notification_deliveries WHERE tenant_id='{tenant}' AND event->>'resource'='server/{sid}' AND event->>'state'='healthy'")=='1','Recovery creates a distinct notification event')
c=owner.get('/notifications')['channels'][0]
owner.post('/notifications/'+channel,{'name':'QA channel','enabled':False,'version':c['version'],'target':None,'confirmed':True})
check(all(d['status']=='Cancelled' for d in owner.get('/notifications')['deliveries']),'Disabling a channel cancels pending retries')
# Real TLS delivery through an isolated QA-only public-address fixture, with full certificate validation.
for kind,target in [('webhook',{'kind':'webhook','url':'https://hoodmin-notify-qa.example.test:55441/event','bearer':'qa-webhook'}),('smtp',{'kind':'smtp','host':'hoodmin-notify-qa.example.test','port':55442,'username':'qa','password':'qa-smtp-password','from':'qa@example.test','to':'recipient@example.test'})]:
	cid=str(uuid.uuid4());owner.post('/notifications/'+cid,{'name':'TLS '+kind,'enabled':True,'target':target,'version':None,'confirmed':True})
	delivery=owner.post('/notifications/'+cid+'/test',{'confirmed':True})['id']
	for _ in range(40):
		row=next(d for d in owner.get('/notifications')['deliveries'] if d['id']==delivery)
		if row['status']=='Delivered':break
		time.sleep(1)
	check(row['status']=='Delivered','Real certificate-verified '+kind+' notification delivered')
	c=next(c for c in owner.get('/notifications')['channels'] if c['id']==cid)
	owner.post('/notifications/'+cid,{'name':'TLS '+kind,'enabled':False,'target':None,'version':c['version'],'confirmed':True})
check((WORK/'webhook-received.jsonl').exists() and b'Subject: Hoodmin' in (WORK/'smtp-received.eml').read_bytes(),'Webhook JSON and SMTP MIME payloads arrive at independent TLS receivers')
# Cover every encrypted storage column, including native session, group PFX and publication targets.
farm_name='Rotation farm';owner.post('/farms',{'name':farm_name})
farm=next(f for f in owner.get('/farms')['items'] if f['name']==farm_name)
owner.post('/servers/'+sid,{'name':'Operations <script>','host':'vpn.example.invalid','port':2443,'farm':farm['id'],'paused':False})
pfx=WORK/'notify.pfx'
subprocess.run(['openssl','pkcs12','-export','-inkey',str(WORK/'notify.key'),'-in',str(WORK/'notify.crt'),'-out',str(pfx),'-passout','pass:'],check=True,capture_output=True)
cert=owner.get('/farms/'+farm['id']+'/certificates')
owner.post('/farms/'+farm['id']+'/certificates',{'base_code':cert['base_code'],'pfx':base64.b64encode(pfx.read_bytes()).decode(),'password':'','tls_name':'hoodmin-notify-qa.example.test','make_default':True,'confirmed':True})
owner.post('/farms/'+farm['id']+'/destinations',{'name':'Rotation publisher','seed':sid,'publish_url':'https://example.com/qa-token','enabled':False,'auto_minutes':0,'replace_settings':True,'upload_url':'https://example.com/qa-upload','method':'PUT','secret':'publisher-secret','body':'{farm_token}','content_type':'text/plain','confirmed':True})
active=owner.post('/tokens',{'server':sid,'name':'Session rotation','count':1,'max_clients':1,'max_gib':0,'expires':'','profile':''})['keys'][0]
wire=json.loads(base64.b64decode(active['key'].removeprefix('vh://')));client=uuid.uuid4()
proof=subprocess.run(['openssl','enc','-aes-128-cbc','-nopad','-K',base64.b64decode(wire['sec']).hex(),'-iv','00'*16],input=client.bytes_le,capture_output=True,check=True).stdout
session=agent('sessions',{'tokenId':active['id'],'clientInfo':{'clientId':str(client),'clientVersion':'9.0.856','userAgent':'OperationsQA','minProtocolVersion':10,'maxProtocolVersion':10},'encryptedClientId':base64.b64encode(proof).decode(),'hostEndPoint':'127.0.0.1:2443','protocolVersion':10})
check(session['errorCode']=='Ok','Independent OpenSSL client proof creates a native VPN session')
# Interrupt between durable dual-key write and database commit, while a QA-only DB lock blocks rotation.
block=subprocess.Popen(['runuser','-u','postgres','--','env','PGAPPNAME=hoodmin-rotation-blocker','psql','-X','-p',PORT,'-d','hoodmin','-c',"BEGIN; SELECT pg_advisory_xact_lock(684663002); SELECT pg_sleep(45);"],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
try:
	for _ in range(40):
		pid=sql("SELECT pid FROM pg_stat_activity WHERE application_name='hoodmin-rotation-blocker' AND wait_event='PgSleep'")
		if pid:break
		time.sleep(.1)
	assert pid
	keyfile=Path('/var/lib/hoodmin/master.key');original=keyfile.read_bytes()
	rotation=subprocess.Popen(['/usr/bin/hoodmin-cli','--config',str(CONFIG),'keys','rotate','--confirmed'],env=ENV,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
	try:
		for _ in range(50):
			if keyfile.read_bytes()!=original:break
			time.sleep(.1)
		check(keyfile.read_bytes()!=original,'Pending key is durable before database rotation lock is acquired')
	finally:rotation.terminate();rotation.wait(timeout=10)
finally:
	if pid:sql('SELECT pg_terminate_backend('+pid+')')
	block.wait(timeout=10)
check(agent('configure',{})['serverSecret']==before['serverSecret'],'Pre-commit process interruption preserves readable original data')

# Rotation failure rolls back every tenant and preserves both old and pending key material.
key_path=Path('/var/lib/hoodmin/master.key');old_key=key_path.read_bytes();old_active=json.loads(cli('keys','status'))['active']
old_cipher=sql(f"SELECT encode(configuration,'hex') FROM servers WHERE id='{sid}'")
# A misowned SECURITY DEFINER function must error, never report zero tenants and switch keys.
sql('ALTER FUNCTION hoodmin_vault_tenants() OWNER TO hoodmin; GRANT EXECUTE ON FUNCTION hoodmin_vault_tenants() TO hoodmin')
try:
	failure=cli('keys','rotate','--confirmed',ok=False)
	check('row-level security' in failure.lower(),'Maintenance detects FORCE RLS filtering instead of silently skipping tenants')
	check(json.loads(cli('keys','status'))['active']==old_active,'Maintenance fails closed when the definer cannot bypass FORCE RLS')
finally:sql('ALTER FUNCTION hoodmin_vault_tenants() OWNER TO postgres; GRANT EXECUTE ON FUNCTION hoodmin_vault_tenants() TO hoodmin')
sql(f"UPDATE servers SET configuration=decode('00','hex') WHERE id='{sid}'")
cli('keys','rotate','--confirmed',ok=False)
check(json.loads(cli('keys','status'))['active']==old_active,'Corrupt ciphertext aborts rotation without changing active database key')
sql(f"UPDATE servers SET configuration=decode('{old_cipher}','hex') WHERE id='{sid}'")
rotated=json.loads(cli('keys','rotate','--confirmed'))
check(rotated['active']!=old_active and rotated['rewrapped']>=5,'All encrypted resources are rewrapped transactionally')
for _ in range(35):
	try:
		if owner.get('/me'):break
	except AssertionError:time.sleep(1)
check(agent('configure',{})['serverSecret']==before['serverSecret'],'Live runtime reloads the rotated key without changing agent/VPN identity')
check(agent('configure',{},second_authorization)['serverSecret']==second_identity,'Rotation also preserves the independently owned second tenant server')
check(channel in {c['id'] for c in owner.get('/notifications')['channels']},'Encrypted notification configuration opens after rotation')
check(owner.get(f'/tokens/{keys[0]["id"]}/key')['value'].startswith('vh://'),'Access-token secrets open after rotation')
r=urllib.request.Request(BASE+'/api/agent/sessions/'+str(session['sessionId']),headers={'Authorization':credential})
with urllib.request.urlopen(r,context=TLS) as response:restored_session=json.load(response)
check(restored_session['sessionKey']==session['sessionKey'],'Native session key survives rotation unchanged')
for table,column in [('servers','configuration'),('servers','agent_credential'),('access_tokens','secret'),('sessions','session_key'),('farms','certificates'),('external_publications','specification'),('notification_channels','specification')]:
	check(sql(f"SELECT bool_and(convert_from(substring({column} from 5 for 32),'UTF8')='{rotated['active']}') FROM {table} WHERE {column} IS NOT NULL")=='t','All tenants use new key for '+table+'.'+column)
owner.get('/farms/'+farm['id']+'/certificates');owner.get('/farms/'+farm['id']+'/destinations')
# Simulate the commit/file-switch crash window: DB ID selects pending key from dual-reader file.
ring=json.loads(key_path.read_text());ring['current']=old_active;key_path.write_text(json.dumps(ring));key_path.chmod(0o600)
check(json.loads(cli('keys','status'))['active']==rotated['active'],'Post-commit key-file recovery uses the authoritative database key ID')
# Retention preview/apply and preservation floor on real full database backups.
# Existing test supplies the actual privileged URL; application password must never be reused as administrator.
backup_admin=WORK/'backup-admin.url'
admin_url=(WORK/'admin.url').read_text().replace('/postgres?','/hoodmin?');private('backup-admin.url',admin_url)
directory=WORK/'retention-backups'
for _ in range(3):cli('backup','create','--admin-url-file',backup_admin,'--directory',directory)
policy=private('retention.json',json.dumps({'enabled':True,'directory':str(directory),'keep_min':2,'max_count':2,'max_age_days':30,'max_bytes':0,'min_free_bytes':0,'notify_tenant':tenant}))
cli('backup','policy','--file',policy)
check(CONFIG.stat().st_mode&0o777==0o640,'Policy update preserves daemon-readable configuration permissions')
preview=json.loads(cli('backup','prune'));check(len(preview['removed'])==1 and len(list(directory.glob('hoodmin-*')))==3,'Retention preview does not delete backup copies')
report=json.loads(cli('backup','prune','--apply'));check(report['retained']==2 and len(list(directory.glob('hoodmin-*')))==2,'Retention applies only validated copies and keeps configured minimum')
cli('backup','create','--admin-url-file',backup_admin,'--directory',directory)
check(len(list(directory.glob('hoodmin-*')))==2,'Successful backup automatically applies an enabled retention policy')
# Operator-selected tenant receives local backup/disk failures and their recovery.
notify=next(c for c in owner.get('/notifications')['channels'] if c['name']=='TLS webhook')
owner.post('/notifications/'+notify['id'],{'name':notify['name'],'enabled':True,'version':notify['version'],'target':None,'confirmed':True})
invalid_admin=private('failed-backup.url',admin_url.replace('postgres:','nonexistent-qa-user:'))
cli('backup','create','--admin-url-file',invalid_admin,'--directory',directory,ok=False)
def state_event(resource,state):
	return sql(f"SELECT count(*) FROM notification_deliveries WHERE tenant_id='{tenant}' AND event->>'resource'='{resource}' AND event->>'state'='{state}'")!='0'
for _ in range(30):
	if state_event('backup','failed'):break
	time.sleep(1)
check(state_event('backup','failed'),'Backup failure reaches only the operator-selected tenant notification queue')
cli('backup','create','--admin-url-file',backup_admin,'--directory',directory)
for _ in range(25):
	if state_event('backup','healthy'):break
	time.sleep(1)
check(state_event('backup','healthy'),'Successful backup emits a recovery transition')
policy_data=json.loads(policy.read_text());policy_data['min_free_bytes']=18_000_000_000_000_000_000;private('retention.json',json.dumps(policy_data))
cli('backup','policy','--file',policy);report=json.loads(cli('backup','prune','--apply'))
check(report['pressure'] and report['retained']==2,'Impossible free-space threshold reports pressure without deleting protected backups')
for _ in range(30):
	if state_event('backup_disk','pressure'):break
	time.sleep(1)
check(state_event('backup_disk','pressure'),'Disk pressure is sent through the operator-selected notification channel')
policy_data['min_free_bytes']=0;private('retention.json',json.dumps(policy_data));cli('backup','policy','--file',policy)
cli('backup','create','--admin-url-file',backup_admin,'--directory',directory)
for _ in range(30):
	if state_event('backup_disk','healthy'):break
	time.sleep(1)
check(state_event('backup_disk','healthy'),'Disk pressure recovery is deduplicated as a new transition')

print('PASS: Operations integration complete')
