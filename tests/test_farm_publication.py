#!/usr/bin/env python3
"""Unavailable seed -> encrypted .NET update -> real prebuilt target connection.
The bootstrap address is intentionally unused; no user's VPN service is stopped.
"""
from panel_support import *
import atexit
import socket

class Values(html.parser.HTMLParser):
	def __init__(self,body):
		super().__init__();self.values={};self.feed(body)
	def handle_starttag(self,tag,attrs):
		a=dict(attrs)
		if tag=='input' and 'name' in a:self.values[a['name']]=a.get('value','')

control=json.loads((ROOT/'work/ssh-install-qa/control.json').read_text())
admin=Browser();assert admin.login(control['email'],control['password'])[0]==303;admin.request('/panel')
tenant=control['tenant'];target=control['server'];target_credential=control['credential']
old_query=f"SELECT json_build_object('name',name,'farm',farm_id,'paused',paused) FROM servers WHERE tenant_id='{tenant}' AND id='{target}'"
old=json.loads(subprocess.run(['runuser','-u','postgres','--','psql','-d','hoodmin','-Atqc',old_query],check=True,capture_output=True,text=True).stdout)
def restore():
	admin.request('/panel')
	fields=dict(name=old['name'],farm=old['farm'] or '')
	if old['paused']:fields['paused']='true'
	check(admin.post(f'/manage/servers/{target}',**fields)[0]==303,'Gerçek hedefin önceki grup üyeliği korundu')
atexit.register(restore)
name='Adres yenileme '+uuid.uuid4().hex[:8]
check(admin.post('/manage/farms',name=name)[0]==303,'Adres yenileme sınama grubu oluşturuldu')
body=admin.request('/panel/servers')[1]
farm=re.search(r'<option value="([0-9a-f-]+)"[^>]*>'+re.escape(name),body).group(1)
code,body,_=admin.post('/manage/servers',name='Ulaşılamayan başlangıç',host='10.0.3.220',port=2999,farm=farm)
settings=json.loads(Fields(body).output);seed_credential=settings['HttpAccessManager']['Authorization'][7:];seed=seed_credential.split('.')[1]
try:
	with socket.create_connection(('10.0.3.220',2999),timeout=2):raise AssertionError('Test bootstrap port unexpectedly serves something')
except (ConnectionRefusedError,TimeoutError):check(True,'Başlangıç VPN adresine gerçekten ulaşılamıyor')
check(admin.post(f'/manage/servers/{target}',name=old['name'],farm=farm)[0]==303,'Gerçek hazır hedef sınama grubuna alındı')
path=f'/manage/farms/{farm}/publication'
version=Values(admin.request(path)[1]).values['version']
check(admin.post(path,version=version)[0]==400,'Onaysız grup adres yayımı reddedildi')
viewer=Browser();viewer_email,_=account(tenant,'viewer');assert viewer.login(viewer_email)[0]==303;viewer.request('/panel')
check(viewer.post(path,version=version,confirmed='true')[0]==403,'Gözlemci grup adres yayımını değiştiremedi')
outsider=Browser();outsider_email,_=account(str(uuid.uuid4()),'admin');assert outsider.login(outsider_email)[0]==303;outsider.request('/panel')
check(outsider.post(path,version=version,confirmed='true')[0]==404,'Başka kiracı grup adres yayımına erişemedi')
check(admin.post(path,version=version,confirmed='true')[0]==303,'Grup adres yenilemesi panelden etkinleştirildi')
check(admin.post(path,version=version,confirmed='true')[0]==409,'Eski grup yayım formu reddedildi')
profile='Yenileme profili '+uuid.uuid4().hex[:8]
check(admin.post('/manage/profiles',name=profile,policy=json.dumps({'farmAccess':True,'maxClients':0}))[0]==303,'Grup erişim profili oluşturuldu')
body=admin.request('/manage/profiles')[1]
# Profile cards are ordered by name; locate this exact card's following form.
profile_id=re.search(re.escape(profile)+r'</h2>.*?action="/manage/profiles/([0-9a-f-]+)"',body,re.S).group(1)
code,body,_=admin.post('/manage/tokens',server=seed,name='Başlangıçtan geçiş',count=1,max_clients=0,max_gib=0,profile=profile_id,expires='')
key='vh://'+Fields(body).output.split('vh://',1)[1].strip();token=json.loads(base64.b64decode(key[5:]))
check('/f/' in token['ser']['urls'][0],'Grup anahtarı özel grup URL yolunu içerdi')
# Seed has no heartbeat. Target has a real native server heartbeat.
for _ in range(20):
	page=admin.request(f'/manage/servers/{target}/configuration')[1]
	if 'Uygulandı' in page:break
	time.sleep(2)
with urllib.request.urlopen(token['ser']['urls'][0],context=CONTEXT,timeout=15) as r:encrypted=r.read().decode()
for name,value in [('token-a.vh',key),('encrypted-farm-token',encrypted)]:
	p=WORK/name;p.write_text(value);p.chmod(0o600)
# Obtain the advertised target from a key issued to the same farm member; no secret is printed.
code,body,_=admin.post('/manage/tokens',server=target,name='Beklenen hedef',count=1,max_clients=0,max_gib=0,profile=profile_id,expires='')
expected=json.loads(base64.b64decode(Fields(body).output.split('vh://',1)[1].strip()))['ser']
p=WORK/'expected-farm-target.json';p.write_text(json.dumps(expected));p.chmod(0o600)
for project in ['ContractProbe','ConnectionProbe']:subprocess.run(['dotnet','build',str(ROOT/'tests'/project),'-c','Release','--nologo'],check=True,capture_output=True)
probe=subprocess.run(['dotnet','run','--project',str(ROOT/'tests/ContractProbe'),'-c','Release','--no-build','--','check-farm-token',str(WORK)],capture_output=True,text=True,check=True)
print(probe.stdout,end='')
connection=subprocess.run(['dotnet','run','--project',str(ROOT/'tests/ConnectionProbe'),'-c','Release','--no-build','--',str(WORK/'updated-farm-token.vh')],capture_output=True,text=True,timeout=75)
if connection.returncode:print(connection.stdout[-2000:],connection.stderr[-2000:])
check(connection.returncode==0,'Yenilenen anahtarla gerçek hazır VPN sunucusuna bağlanıldı')
print(connection.stdout,end='')
restore();atexit.unregister(restore)
print('PASS: Erişilemeyen başlangıç adresinden grup URL yenilemesiyle gerçek hedefe geçiş doğrulandı.')
print('Sınama çalışma dizini:',WORK)
