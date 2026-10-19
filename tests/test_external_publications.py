#!/usr/bin/env python3
"""Dedicated old QA only: real public HTTPS publisher, RLS/roles, restart and .NET client."""
from panel_support import *
import atexit
import shutil

class Values(html.parser.HTMLParser):
	def __init__(self,body):
		super().__init__();self.values={};self.feed(body)
	def handle_starttag(self,tag,attrs):
		a=dict(attrs)
		if tag=='input' and 'name' in a:self.values[a['name']]=a.get('value','')

def sql(query):
	return subprocess.run(['runuser','-u','postgres','--','psql','-X','-d','hoodmin','-Atqc',query],capture_output=True,text=True,check=True).stdout.strip()

control=json.loads((ROOT/'work/ssh-install-qa/control.json').read_text())
admin=Browser();assert admin.login(control['email'],control['password'])[0]==303;admin.request('/panel')
tenant=control['tenant'];target=control['server']
old=json.loads(sql(f"SELECT json_build_object('name',name,'farm',farm_id,'paused',paused) FROM servers WHERE tenant_id='{tenant}' AND id='{target}'"))
nginx=Path('/etc/nginx/conf.d/vpnhood-manager-test.conf');before=nginx.read_bytes()
receiver=Path('/run/hoodmin-publication-sink')
receiver_script=Path('/run/hoodmin-publication-sink.py')
receiver_unit='hoodmin-publication-qa-'+uuid.uuid4().hex[:8]
path='/__hoodmin_publish_qa/'+str(uuid.uuid4());publish=BASE+path
secret='qa-'+secrets.token_urlsafe(24)
ids=[]
bots=[]
issued_tokens=[]
farm=None

def configure(**changes):
	p=receiver/'config.json';config=json.loads(p.read_text());config.update(changes);p.write_text(json.dumps(config))

def row(id):
	return json.loads(sql(f"SELECT row_to_json(p) FROM (SELECT id,version,status,message,last_success,next_due,lease_until,enabled FROM external_publications WHERE tenant_id='{tenant}' AND id='{id}') p"))

def wait(id,status='Succeeded'):
	for _ in range(45):
		r=row(id)
		if r['status'] in ('Succeeded','Failed'):
			assert r['status']==status,r
			return r
		time.sleep(1)
	raise AssertionError('Publisher did not finish')

def cleanup():
	if issued_tokens:
		values=','.join("'"+str(uuid.UUID(t))+"'" for t in issued_tokens)
		sql(f"UPDATE access_tokens SET revoked=true WHERE tenant_id='{tenant}' AND id IN ({values})")
	for bot in bots:
		admin.request('/panel');admin.post('/manage/bots/'+bot+'/revoke',confirmed='true')
	if farm:
		sql(f"DELETE FROM external_publications WHERE tenant_id='{tenant}' AND farm_id='{farm}'")
	try:
		admin.request('/panel');fields=dict(name=old['name'],farm=old['farm'] or '')
		if old['paused']:fields['paused']='true'
		admin.post('/manage/servers/'+target,**fields)
	finally:
		nginx.write_bytes(before)
		subprocess.run(['nginx','-t'],check=True,capture_output=True)
		subprocess.run(['systemctl','reload','nginx'],check=True)
		subprocess.run(['systemctl','stop',receiver_unit],capture_output=True)
		shutil.rmtree(receiver,ignore_errors=True)
		receiver_script.unlink(missing_ok=True)
atexit.register(cleanup)

assert not receiver.exists()
shutil.copyfile(ROOT/'tests/publication_sink.py',receiver_script)
receiver_script.chmod(0o644)
receiver.mkdir(mode=0o700);os.chown(receiver,65534,65534)
config=dict(path=path,GET=200,PUT=405,POST=201,expected_marker=secret)
p=receiver/'config.json';p.write_text(json.dumps(config));p.chmod(0o600);os.chown(p,65534,65534)
route='\tlocation ^~ /__hoodmin_publish_qa/ {\n\t\tclient_max_body_size 512k;\n\t\tproxy_pass http://127.0.0.1:18081;\n\t\tproxy_next_upstream off;\n\t\taccess_log off;\n\t}\n'
assert b'__hoodmin_publish_qa' not in before
nginx.write_text(before.decode().replace('\tlocation /events {',route+'\tlocation /events {',1))
subprocess.run(['nginx','-t'],check=True,capture_output=True)
subprocess.run(['systemd-run','--unit='+receiver_unit,'--property=User=nobody','--property=Group=nogroup','--property=NoNewPrivileges=yes','--property=ProtectSystem=strict','--property=ProtectHome=yes','--property=ReadWritePaths='+str(receiver),'--property=RuntimeMaxSec=1800','/usr/bin/python3',str(receiver_script),str(receiver)],check=True,capture_output=True)
subprocess.run(['systemctl','reload','nginx'],check=True)

name='Harici yayım '+uuid.uuid4().hex[:8]
check(admin.post('/manage/farms',name=name)[0]==303,'Yalıtılmış yayım sınama grubu oluşturuldu')
farm=re.search(r'<option value="([0-9a-f-]+)"[^>]*>'+re.escape(name),admin.request('/panel/servers')[1]).group(1)
assert admin.post('/manage/servers/'+target,name=old['name'],farm=farm)[0]==303
page='/manage/farms/'+farm+'/publication'
assert admin.post(page,version=Values(admin.request(page)[1]).values['version'],confirmed='true')[0]==303
_,body,_=admin.post('/manage/servers',name='Yayım başlangıcı',host='10.0.3.220',port=2999,farm=farm)
settings=json.loads(Fields(body).output);seed=settings['HttpAccessManager']['Authorization'][7:].split('.')[1]
profile_name='Harici profil '+uuid.uuid4().hex[:8]
assert admin.post('/manage/profiles',name=profile_name,policy=json.dumps({'farmAccess':True,'maxClients':0}))[0]==303
profile=re.search(re.escape(profile_name)+r'</h2>.*?action="/manage/profiles/([0-9a-f-]+)"',admin.request('/manage/profiles')[1],re.S).group(1)
def key(server,name):
	code,body,_=admin.post('/manage/tokens',server=server,name=name,count=1,max_clients=0,max_gib=0,profile=profile,expires='')
	assert code==200
	value='vh://'+Fields(body).output.split('vh://',1)[1].strip()
	issued_tokens.append(json.loads(base64.b64decode(value[5:]))['tid'])
	return value
original=key(seed,'Harici başlangıç')
(WORK/'token-a.vh').write_text(original)
endpoint='/manage/farms/'+farm+'/destinations'
form=dict(name='Ham yayım',seed=seed,publish_url=publish,upload_url=publish,enabled='true',auto_minutes=5,replace_settings='true',method='PUT_POST',secret=secret,headers='X-Hoodmin-QA: {access_token}',form_data='',body='{farm_token}',content_type='text/plain; charset=utf-8')
check(admin.post(endpoint,**form)[0]==400,'Harici yayım açık onay olmadan oluşturulamadı')
viewer=Browser();email,_=account(tenant,'viewer');assert viewer.login(email)[0]==303;viewer.request('/panel')
check(viewer.post(endpoint,**form,confirmed='true')[0]==403,'Gözlemci yayım hedefi oluşturamadı')
outsider=Browser();email,_=account(str(uuid.uuid4()),'admin');assert outsider.login(email)[0]==303;outsider.request('/panel')
check(outsider.post(endpoint,**form,confirmed='true')[0]==404,'Başka kiracı grubuna yayım hedefi eklenemedi')
other_tenant=str(uuid.uuid4())
assert subprocess.run(['runuser','-u','hoodmin','--','psql','-X','-d','hoodmin','-Atqc',f"SET app.tenant_id='{other_tenant}'; SELECT count(*) FROM external_publications WHERE tenant_id='{tenant}';"],capture_output=True,text=True,check=True).stdout.strip()=='0'
check(True,'Uygulama DB rolü başka kiracının yayım satırlarını RLS üzerinden okuyamadı')
for bad_url in ['https://127.0.0.1/','https://169.254.169.254/','http://example.com/','https://user:pass@example.com/','https://[::1]/']:
	invalid={**form,'upload_url':bad_url}
	check(admin.post(endpoint,**invalid,confirmed='true')[0]==400,'SSRF/kimlikli/HTTP yükleme hedefi reddedildi')
assert admin.post(endpoint,**form,confirmed='true')[0]==303
id=sql(f"SELECT id FROM external_publications WHERE tenant_id='{tenant}' AND farm_id='{farm}' ORDER BY id LIMIT 1");ids.append(id)
wait(id)
check(True,'Gerçek genel HTTPS: ham PUT başarısızlığından sonra POST ve dosya doğrulaması geçti')
check(admin.post(endpoint,**{**form,'name':'Çakışan dosya','seed':target},confirmed='true')[0]==400,'Farklı başlangıç sırlarının aynı yayım dosyasını bozması engellendi')
records=[json.loads(s) for s in (receiver/'requests.jsonl').read_text().splitlines()]
uploads=[r for r in records if r['method'] in ['PUT','POST']]
check([r['method'] for r in uploads[:2]]==['PUT','POST'] and uploads[0]['sha256']==uploads[1]['sha256'],'PUT/POST geri dönüşü aynı ham gövdeyi gönderdi')
check(uploads[0]['content_type']=='text/plain; charset=utf-8' and uploads[0]['qa_marker_matches'],'Gövdeyle uyumlu içerik türü ve şifreli saklanan kimlik şablonu kullanıldı')
check(all(not r['qa_marker_matches'] for r in records if r['method']=='GET'),'Yükleme kimliği genel dosya GET isteğine taşınmadı')
check(secret not in admin.request('/manage/publications/'+id)[1],'Düzenleyici kayıtlı kimliği geri göstermedi')

for lang,word,direction in [('en','External address publishing','ltr'),('ar','نشر العنوان الخارجي','rtl')]:
	code,_,headers=admin.request('/language?'+urllib.parse.urlencode(dict(lang=lang,return_to=endpoint)))
	check(code==303 and headers['Location']==endpoint,'Dil değişimi yayım listesinde kaldı: '+lang)
	body=admin.request(endpoint)[1]
	check(word in body and 'dir="'+direction+'"' in body,'Yayım sayfasında çeviri ve metin yönü doğrulandı: '+lang)
admin.request('/language?'+urllib.parse.urlencode(dict(lang='tr',return_to=endpoint)))
code,_,headers=admin.request('/theme?'+urllib.parse.urlencode(dict(theme='dark',return_to='/manage/publications/'+id)))
check(code==303 and headers['Location']=='/manage/publications/'+id,'Tema değişimi yayım düzenleyicisinde kaldı')


# JSON API uses the same checks and never serializes the encrypted specification.
def bot(role):
	admin.request('/panel');code,body,_=admin.post('/manage/bots',name='Yayım API '+role,role=role,lifetime_days=1,confirmed='true');assert code==200
	settings=json.loads(Fields(body).output);bots.append(settings['id']);return settings

def api(settings,path,body=None):
	headers={'Authorization':settings['authorization']}
	data=None
	if body is not None:data=json.dumps(body).encode();headers['Content-Type']='application/json'
	request=urllib.request.Request(BASE+'/api/manage'+path,headers=headers,data=data)
	try:response=urllib.request.urlopen(request,context=CONTEXT,timeout=20)
	except urllib.error.HTTPError as e:response=e
	return response.status,json.loads(response.read())

read=bot('viewer');write=bot('admin')
code,data=api(read,'/farms/'+farm+'/destinations')
check(code==200 and secret not in json.dumps(data) and all('specification' not in r for r in data['items']),'JSON listesi yayıncı sırrını veya şifreli ayarları döndürmedi')
api_form={**form,'enabled':False,'replace_settings':True,'auto_minutes':0,'confirmed':True,'name':'API kapalı hedef'}
check(api(read,'/farms/'+farm+'/destinations',api_form)[0]==403,'Okuyucu bot yayım hedefi oluşturamadı')
code,created=api(write,'/farms/'+farm+'/destinations',api_form);assert code==202
api_id=created['location'].rsplit('/',1)[-1]
check(row(api_id)['status']=='Disabled','Yönetici bot panelle aynı doğrulayıcıdan hedef oluşturdu')
check(api(read,'/publications/'+api_id+'/remove',{'version':row(api_id)['version'],'confirmed':True})[0]==403,'Okuyucu bot yayım hedefini kaldıramadı')
check(api(write,'/publications/'+api_id+'/remove',{'version':row(api_id)['version'],'confirmed':True})[0]==202,'Yönetici bot yayım hedefini kaldırdı')

check(sql(f"SELECT position(convert_to('{secret}','UTF8') in specification)=0 FROM external_publications WHERE tenant_id='{tenant}' AND id='{id}'")=='t','Veritabanında yayıncı kimliği açık metin olarak saklanmadı')
new_key=json.loads(base64.b64decode(key(seed,'Yayımlanmış anahtar')[5:]))
check(publish in new_key['ser']['urls'],'Doğrulanan harici adres yalnız ilgili başlangıç sunucusunun yeni anahtarına eklendi')
other=json.loads(base64.b64decode(key(target,'Hedef anahtarı')[5:]))
check(publish not in other['ser']['urls'],'Başka başlangıç sunucusuna yanlış sırla şifreli URL eklenmedi')
(WORK/'expected-farm-target.json').write_text(json.dumps(other['ser']))
with urllib.request.urlopen(publish,context=CONTEXT,timeout=20) as response:encrypted=response.read()
(WORK/'encrypted-farm-token').write_bytes(encrypted)
for project in ['ContractProbe','ConnectionProbe']:subprocess.run(['dotnet','build',str(ROOT/'tests'/project),'-c','Release','--nologo'],check=True,capture_output=True)
result=subprocess.run(['dotnet','run','--project',str(ROOT/'tests/ContractProbe'),'-c','Release','--no-build','--','check-farm-token',str(WORK)],check=True,capture_output=True,text=True);print(result.stdout,end='')
result=subprocess.run(['dotnet','run','--project',str(ROOT/'tests/ConnectionProbe'),'-c','Release','--no-build','--',str(WORK/'updated-farm-token.vh')],capture_output=True,text=True,timeout=75)
check(result.returncode==0,'Harici dosyadan çözümlenen adresle gerçek hazır VPN sunucusuna bağlanıldı')

configure(PUT=201)
cursor=len((receiver/'requests.jsonl').read_text().splitlines())
version=row(id)['version'];admin.request('/panel')
check(admin.post('/manage/publications/'+id+'/push',version=version)[0]==400,'Elle yayım onaysız başlatılamadı')
check(viewer.post('/manage/publications/'+id+'/push',version=version,confirmed='true')[0]==403,'Gözlemci elle yayım başlatamadı')
assert admin.post('/manage/publications/'+id+'/push',version=version,confirmed='true')[0]==303
wait(id)
recent=[json.loads(s) for s in (receiver/'requests.jsonl').read_text().splitlines()[cursor:]]
check([r['method'] for r in recent if r['method'] in ['PUT','POST']]==['PUT'],'Başarılı PUT sonrası POST gönderilmedi')

# Edit without replace_settings preserves the sealed credentials; stale versions conflict.
update=dict(name='Ham yayım düzenlendi',seed=seed,publish_url=publish,enabled='true',auto_minutes=5,version=version,confirmed='true')
assert admin.post('/manage/publications/'+id,**update)[0]==303
check(admin.post('/manage/publications/'+id,**update)[0]==409,'Eski sürümlü düzenleme reddedildi')
wait(id)
# Force the scheduled due time in this disposable QA record, then restart the real service.
sql(f"UPDATE external_publications SET next_due=now(),lease_until=now()+interval '2 seconds',status='Running' WHERE tenant_id='{tenant}' AND id='{id}'")
subprocess.run(['systemctl','restart','vpnhood-manager'],check=True)
wait(id)
check(True,'Servis yeniden başladıktan sonra süresi dolan iş kirası ve otomatik yayım toparlandı')
# Multipart upload: independent destination, no JSON wrapping.
configure(form_field='payload',upload_path=path+'?qa='+secret)
multipart={**form,'name':'Multipart yayım','upload_url':publish+'?qa={access_token}','method':'POST','body':'','form_data':'payload: {farm_token}\nkey: {access_token}','auto_minutes':0}
admin.request('/panel');assert admin.post(endpoint,**multipart,confirmed='true')[0]==303
form_id=sql(f"SELECT id FROM external_publications WHERE tenant_id='{tenant}' AND farm_id='{farm}' AND name='Multipart yayım'");ids.append(form_id);wait(form_id)
check(True,'Multipart POST, alan yer tutucuları ve yayımlanan dosyanın byte eşitliği doğrulandı')
latest=[json.loads(s) for s in (receiver/'requests.jsonl').read_text().splitlines() if json.loads(s)['method']=='POST'][-1]
check(latest['upload_path_used'],'Yükleme URL yer tutucusu güvenli URL kodlamasıyla değiştirildi')
# DNS name resolves locally: must never connect, and cannot become an advertised URL.
bad={**form,'name':'DNS SSRF sınaması','upload_url':'https://localhost/blocked','auto_minutes':0}
assert admin.post(endpoint,**bad,confirmed='true')[0]==303
bad_id=sql(f"SELECT id FROM external_publications WHERE tenant_id='{tenant}' AND farm_id='{farm}' AND name='DNS SSRF sınaması'");ids.append(bad_id)
check('DNS' in wait(bad_id,'Failed')['message'],'DNS üzerinden özel ağ hedefine bağlantı engellendi')
configure(form_field='',PUT=302,POST=302,response_headers={'Location':'https://127.0.0.1:3080/never'})
redirect={**form,'name':'Yönlendirme sınaması','auto_minutes':0}
assert admin.post(endpoint,**redirect,confirmed='true')[0]==303
redirect_id=sql(f"SELECT id FROM external_publications WHERE tenant_id='{tenant}' AND farm_id='{farm}' AND name='Yönlendirme sınaması'");ids.append(redirect_id)
check('302' in wait(redirect_id,'Failed')['message'],'HTTP yönlendirmesi izlenmedi; iç hedefe kimlik taşınmadı')
# Disable the original automatic task before the receiver is removed.
r=row(id);admin.request('/panel');assert admin.post('/manage/publications/'+id,name='Ham yayım',seed=seed,publish_url=publish,auto_minutes=0,version=r['version'],confirmed='true')[0]==303
check(row(id)['status']=='Disabled' and row(id)['next_due'] is None,'Yayım kapatılınca zamanlama durdu')
for ident in ids:
	r=row(ident)
	check(admin.post('/manage/publications/'+ident+'/remove',version=r['version'],confirmed='true')[0]==303,'Yayım kaydı ve şifreli kimlik onaylı kaldırıldı')
(WORK/'publication-proof.json').write_text(json.dumps(dict(farm=farm,server=target,tokens=issued_tokens,checks='HTTPS/native-client/multipart/API/SSRF/restart passed')))
cleanup();atexit.unregister(cleanup)
print('PASS: Harici yayıncı HTTPS, gerçek VPN bağlantısı, multipart, RLS, SSRF, sürüm/onay ve yeniden başlatma sınamaları tamamlandı.')
print('Private QA evidence directory:',WORK)
