#!/usr/bin/env python3
"""Old QA: AutoSign roles, RLS, versions, leased hooks and cleanup; no public CA call."""
from panel_support import *
from concurrent.futures import ThreadPoolExecutor
class Inputs(html.parser.HTMLParser):
	def __init__(self,body):
		super().__init__();self.values={};self.feed(body)
	def handle_starttag(self,tag,attrs):
		a=dict(attrs)
		if tag=='input' and 'name' in a:self.values[a['name']]=a.get('value','')
def sql(q):
	return subprocess.run(['runuser','-u','postgres','--','psql','-X','-Atqc',q,'hoodmin'],check=True,capture_output=True,text=True).stdout.strip()
tenant=str(uuid.uuid4());email,_=account(tenant,'admin');reader,_=account(tenant,'viewer');outsider,_=account(str(uuid.uuid4()),'admin')
admin,viewer,other=Browser(),Browser(),Browser()
for b,e in [(admin,email),(viewer,reader),(other,outsider)]:
	assert b.login(e)[0]==303;b.request('/panel')
sid,credential,_=server(admin,'AutoSign sınama',2999);path='/manage/servers/'+sid+'/autosign'
fields=dict(domain='vpn.example.com',email='vpn@example.com',staging='true',confirmed='true')
assert viewer.request(path)[0]==200
check(viewer.post(path,**fields)[0]==403,'Gözlemci AutoSign ayarını değiştiremedi')
check(other.post(path,**fields)[0]==404,'Başka kiracı AutoSign kaydına ulaşamadı')
check(admin.post(path,**dict(fields,confirmed='false'))[0]==400,'Etki onayı olmadan AutoSign kaydedilmedi')
check(admin.post(path,**fields,enabled='true')[0]==400,'CA koşulu onayı olmadan dış talep oluşturulmadı')
for invalid in ['127.0.0.1','*.example.com','example.com/arg','localhost']:
	check(admin.post(path,**dict(fields,domain=invalid))[0]==400,'Geçersiz HTTP-01 alan adı reddedildi')
check(admin.post(path,**fields)[0]==303,'Kapalı AutoSign kaydı panelden oluşturuldu')
data=json.loads(sql(f"SELECT json_build_object('id',id,'version',version,'status',status) FROM acme_orders WHERE tenant_id='{tenant}' AND server_id='{sid}'"))
check(data['status']=='Disabled','Kapalı kayıtta CA isteği zamanlanmadı')
check(admin.post(path,**fields,version=data['version'])[0]==303,'Güncel sürümle kayıt değiştirildi')
check(admin.post(path,**fields,version=data['version'])[0]==409,'Eski AutoSign formu reddedildi')
job=data['id']
sql(f"UPDATE acme_orders SET lease_until=now()+interval '20 minutes' WHERE tenant_id='{tenant}' AND id='{job}'")
version=Inputs(admin.request(path)[1]).values['version']
check(admin.post(path,**fields,version=version)[0]==409,'Çalışan ACME işi sırasında ayarlar değiştirilemedi')
old=agent(credential,'configure',{})
token=secrets.token_urlsafe(24);proof=token+'.'+secrets.token_urlsafe(32)
env=dict(ENV,CERTBOT_DOMAIN='vpn.example.com',CERTBOT_TOKEN=token,CERTBOT_VALIDATION=proof)
def hook(cleanup=False,scope=tenant):
	command=['runuser','-u','hoodmin','--preserve-environment','--',BINARY,'acme-hook','--tenant',scope,'--job',job]
	if cleanup:command+=['--cleanup']
	return subprocess.run(command,env=env,capture_output=True,text=True,timeout=140)
def await_hook(cleanup=False):
	with ThreadPoolExecutor(max_workers=1) as pool:
		future=pool.submit(hook,cleanup)
		for _ in range(60):
			config=agent(credential,'configure',{})
			agent(credential,'status',dict(configCode=config['configCode'],configError=None))
			if future.done():break
			time.sleep(.2)
		result=future.result()
		assert result.returncode==0,result.stderr[-400:]
await_hook()
current=agent(credential,'configure',{})
check(current['enableHttp01Challenge'] is True,'Auth kancası tenant içindeki HTTP-01 dinleyicisini açtı')
check(sql(f"SELECT key_authorization FROM acme_challenges WHERE tenant_id='{tenant}' AND server_id='{sid}' AND token='{token}'")==proof,'Doğrulama yanıtı doğru sunucu ve kiracıya kaydedildi')
check(hook(scope=str(uuid.uuid4())).returncode!=0,'Yabancı kiracıdaki kanca görev sırrına ulaşamadı')
await_hook(True)
check(agent(credential,'configure',{})['enableHttp01Challenge']==old.get('enableHttp01Challenge',False),'Cleanup önceki HTTP-01 durumunu geri yükledi')
check(sql(f"SELECT count(*) FROM acme_challenges WHERE tenant_id='{tenant}' AND server_id='{sid}' AND token='{token}'")=='0','Yalnız bu görevin geçici doğrulama kaydı kaldırıldı')
sql(f"UPDATE acme_orders SET lease_until=NULL WHERE tenant_id='{tenant}' AND id='{job}'")
for lang in ['en','ar','tr']:
	code,body,headers=admin.request('/language?'+urllib.parse.urlencode(dict(lang=lang,return_to=path)))
	check(code==303 and headers['Location']==path,'AutoSign dil dönüş yolu korundu: '+lang)
print('PASS: AutoSign roller, RLS, sürüm, CA onayı ve gerçek agent kancası/temizleme tamamlandı.')
