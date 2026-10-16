#!/usr/bin/env python3
"""Shared encrypted PFX store, node inheritance and real native TLS selection."""
from panel_support import *
import atexit
import hashlib
import socket

class Values(html.parser.HTMLParser):
	def __init__(self,body):
		super().__init__();self.values={};self.feed(body)
	def handle_starttag(self,tag,attrs):
		a=dict(attrs)
		if tag=='input' and 'name' in a:self.values[a['name']]=a.get('value','')

control=json.loads((ROOT/'work/ssh-install-qa/control.json').read_text());tenant=control['tenant'];target=control['server']
admin=Browser();assert admin.login(control['email'],control['password'])[0]==303;admin.request('/panel')
def sql(query):return subprocess.run(['runuser','-u','postgres','--','psql','-d','vpnhood_manager','-v','ON_ERROR_STOP=1','-At'],input=query,text=True,capture_output=True,check=True).stdout.strip()
old=json.loads(sql(f"SELECT json_build_object('name',name,'farm',farm_id,'configuration',encode(configuration,'base64'),'code',config_code) FROM servers WHERE tenant_id='{tenant}' AND id='{target}'"))
p=WORK/'target-before.json';p.write_text(json.dumps(old));p.chmod(0o600)
def applied():
	for _ in range(35):
		if 'Uygulandı' in admin.request(f'/manage/servers/{target}/configuration')[1]:return
		time.sleep(2)
	raise AssertionError('Native target did not apply certificate configuration')
def restore():
	farm="NULL" if old['farm'] is None else "'"+str(uuid.UUID(old['farm']))+"'"
	blob=old['configuration'].replace("'","''");code=old['code'].replace("'","''")
	sql(f"UPDATE servers SET configuration=decode('{blob}','base64'),config_code='{code}',farm_id={farm} WHERE tenant_id='{tenant}' AND id='{target}';")
	applied()
	check(True,'Gerçek hedefin önceki şifreli yapılandırması ve grup üyeliği geri yüklendi')
atexit.register(restore)
name='Sertifika grubu '+uuid.uuid4().hex[:8];assert admin.post('/manage/farms',name=name)[0]==303
farm=re.search(r'<option value="([0-9a-f-]+)"[^>]*>'+re.escape(name),admin.request('/panel/servers')[1]).group(1)
assert admin.post(f'/manage/servers/{target}',name=old['name'],farm=farm)[0]==303
code,body,_=admin.post('/manage/servers',name='DNS üyesi',host='dns-member.example.test',port=2780,farm=farm)
settings=json.loads(Fields(body).output);credential=settings['HttpAccessManager']['Authorization'][7:];member=credential.split('.')[1]
original=agent(credential,'configure',{})
# Capture only public server-token data from an operator-authorized key.
code,body,_=admin.post('/manage/tokens',server=target,name='Önce',count=1,max_clients=0,max_gib=0,profile='',expires='')
old_token=json.loads(base64.b64decode(Fields(body).output.split('vh://',1)[1].strip()))['ser']
password=WORK/'pfx-password';password.write_text(secrets.token_urlsafe(24));password.chmod(0o600)
key,cert,pfx=WORK/'group.key',WORK/'group.pem',WORK/'group.pfx'
subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(key),'-out',str(cert),'-days','2','-subj','/CN=farm-cert-test.invalid','-addext','subjectAltName=DNS:farm-cert-test.invalid'],capture_output=True,check=True);key.chmod(0o600)
subprocess.run(['openssl','pkcs12','-export','-inkey',str(key),'-in',str(cert),'-out',str(pfx),'-passout','file:'+str(password)],capture_output=True,check=True);pfx.chmod(0o600)
path=f'/manage/farms/{farm}/certificates';base=Values(admin.request(path)[1]).values['base_code']
fields=dict(base_code=base,pfx=base64.b64encode(pfx.read_bytes()).decode(),password=password.read_text(),tls_name='farm-cert-test.invalid',make_default='true',confirmed='true')
check(admin.post(path,**dict(fields,confirmed='false'))[0]==400,'Onaysız grup sertifika dağıtımı reddedildi')
viewer=Browser();viewer_email,_=account(tenant,'viewer');assert viewer.login(viewer_email)[0]==303;viewer.request('/panel')
check(viewer.post(path,**fields)[0]==403,'Gözlemci grup sertifikası dağıtamadı')
outsider=Browser();outsider_email,_=account(str(uuid.uuid4()),'admin');assert outsider.login(outsider_email)[0]==303;outsider.request('/panel')
check(outsider.post(path,**fields)[0]==404,'Başka kiracı grup sertifikası deposuna erişemedi')
check(admin.post(path,**dict(fields,password='incorrect'))[0]==400,'Grup PFX yanlış parolası reddedildi')
check(admin.post(path,**fields)[0]==303,'PFX grup deposuna kaydedilip mevcut üyelere dağıtıldı')
check(admin.post(path,**fields)[0]==409,'Eski grup sertifika formu reddedildi')
actual=agent(credential,'configure',{})
check(actual['serverSecret']==original['serverSecret'] and actual['certificates'][0]['commonName']=='farm-cert-test.invalid','Grup PFX dağıtımı üyenin sunucu sırrını korudu')
code,body,_=admin.post('/manage/tokens',server=member,name='DNS sonra',count=1,max_clients=0,max_gib=0,profile='',expires='')
dns_token=json.loads(base64.b64decode(Fields(body).output.split('vh://',1)[1].strip()))['ser']
check(dns_token['hname']=='dns-member.example.test','Grup sertifikası DNS bağlantı adresini değiştirmedi')
code,body,_=admin.post('/manage/servers',name='Yeni grup üyesi',host='10.0.3.220',port=2781,farm=farm)
new_credential=json.loads(Fields(body).output)['HttpAccessManager']['Authorization'][7:]
check(agent(new_credential,'configure',{})['certificates'][0]['rawData']==actual['certificates'][0]['rawData'],'Gruba yeni eklenen sunucu ortak PFX aldı')
new_member,new_member_credential,_=server(admin,'Sonradan katılan',2782)
check(admin.post(f'/manage/servers/{new_member}',name='Sonradan katılan',farm=farm)[0]==303,'Mevcut sunucu gruba taşındı')
check(agent(new_member_credential,'configure',{})['certificates'][0]['rawData']==actual['certificates'][0]['rawData'],'Gruba taşınan sunucu ortak PFX aldı')
applied()
context=ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT);context.check_hostname=False;context.verify_mode=ssl.CERT_NONE
expected=subprocess.run(['openssl','x509','-in',str(cert),'-outform','DER'],capture_output=True,check=True).stdout
# Compare the exact certificate, using the original VPN hostname/SNI rather than the uploaded certificate alias.
with socket.create_connection(('10.0.3.220',2550),timeout=10) as connection:
	with context.wrap_socket(connection,server_hostname=old_token['hname']) as tls:check(tls.getpeercert(binary_form=True)==expected,'Gerçek hazır sunucu eski bağlantı adıyla yeni grup sertifikasını sundu')
code,body,_=admin.post('/manage/tokens',server=target,name='Sonra',count=1,max_clients=0,max_gib=0,profile='',expires='')
new_token=json.loads(base64.b64decode(Fields(body).output.split('vh://',1)[1].strip()))['ser']
check(new_token['hname']==old_token['hname'] and base64.b64decode(new_token['ch'])==hashlib.sha1(expected).digest(),'İstemci bağlantı adı korundu ve parmak izi yeni PFX oldu')
access=WORK/'group-token.vh';access.write_text('vh://'+Fields(body).output.split('vh://',1)[1].strip());access.chmod(0o600)
probe=subprocess.run(['dotnet','run','--project',str(ROOT/'tests/ConnectionProbe'),'-c','Release','--no-build','--',str(access)],capture_output=True,text=True,timeout=75)
if probe.returncode:print(probe.stdout[-2000:],probe.stderr[-2000:])
check(probe.returncode==0,'Grup PFX sonrasında gerçek VpnHood istemcisi bağlandı')
# Renewal of the same X509 DNS name with a different UI alias must not create duplicate native selectors.
base=Values(admin.request(path)[1]).values['base_code'];fields.update(base_code=base,tls_name='certificate-alias.invalid');fields.pop('make_default')
check(admin.post(path,**fields)[0]==303,'Aynı DNS adlı PFX farklı etiketle yenilendi')
check(len(agent(credential,'configure',{})['certificates'])==1,'Native TLS seçiminde aynı DNS adı çakışmadı')
applied()
restore();atexit.unregister(restore)
print('PASS: Grup PFX deposu, üye kalıtımı, DNS adresi koruması ve gerçek TLS bağlantısı doğrulandı.')
print('Sınama çalışma dizini:',WORK)
