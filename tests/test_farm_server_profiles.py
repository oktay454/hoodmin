#!/usr/bin/env python3
"""Old QA: atomic group inheritance and updates, isolation, real native acknowledgement."""
from panel_support import *
import atexit

class Inputs(html.parser.HTMLParser):
	def __init__(self,body):
		super().__init__();self.values={};self.feed(body)
	def handle_starttag(self,tag,attrs):
		a=dict(attrs)
		if tag=='input' and 'name' in a:self.values[a['name']]=a.get('value','')

def sql(query):
	return subprocess.run(['runuser','-u','postgres','--','psql','-X','-v','ON_ERROR_STOP=1','-d','vpnhood_manager','-Atqc',query],text=True,capture_output=True,check=True).stdout.strip()
def version(browser,path):
	return Inputs(browser.request(path)[1]).values['version']
def native(credential):
	return agent(credential,'configure',{})
def register(browser,name,farm):
	code,body,_=browser.post('/manage/servers',name=name,host='10.0.3.220',port=2550,farm=farm)
	assert code==200,(code,body[:80])
	credential=json.loads(Fields(body).output)['HttpAccessManager']['Authorization'][7:]
	return credential.split('.')[1],credential

tenant=str(uuid.uuid4())
admin_email,_=account(tenant,'admin');viewer_email,_=account(tenant,'viewer');other_email,_=account(str(uuid.uuid4()),'admin')
admin,viewer,other=Browser(),Browser(),Browser()
for b,e in [(admin,admin_email),(viewer,viewer_email),(other,other_email)]:
	assert b.login(e)[0]==303;b.request('/panel')
def farm(name):
	assert admin.post('/manage/farms',name=name)[0]==303
	return sql(f"SELECT id FROM farms WHERE tenant_id='{tenant}' AND name='{name}'")
farm_a=farm('Profil grubu A');farm_b=farm('Profil grubu B')
a,ca=register(admin,'Mevcut üye',farm_a);b,cb=register(admin,'Taşınacak üye','')
old_a=native(ca);old_b=native(cb)
settings={'tcpEndPoints':['0.0.0.0:2550'],'updateStatusInterval':5,'Session':{'syncInterval':5},'Tracking':{},'NetFilter':{}}
policy={'power':3,'location':'TR/Istanbul'}
def create_profile(name,settings=settings):
	assert admin.post('/manage/server-profiles',name=name,settings=json.dumps(settings),management=json.dumps(policy))[0]==303
	return sql(f"SELECT id FROM server_profiles WHERE tenant_id='{tenant}' AND name='{name}'")
profile=create_profile('Otomatik profil')
profile_path='/manage/server-profiles/'+profile
profile_version=sql(f"SELECT version FROM server_profiles WHERE tenant_id='{tenant}' AND id='{profile}'")
path='/manage/farms/'+farm_a+'/server-profile'
fields=dict(version=version(admin,path),profile=profile,profile_version=profile_version)
check(admin.post(path,**fields)[0]==400,'Onaysız grup profil ataması reddedildi')
check(viewer.post(path,**fields,confirmed='true')[0]==403,'Gözlemci grup profil atamasını değiştiremedi')
check(other.post(path,**fields,confirmed='true')[0]==404,'Başka kiracı grup profil atamasını değiştiremedi')
invalid=dict(fields,profile=str(uuid.uuid4()))
check(admin.post(path,**invalid,confirmed='true')[0]==404,'Başka kiracı veya bilinmeyen profil atanamadı')
check(native(ca)==old_a,'Reddedilen işlemler üyeye dokunmadı')
check(admin.post(path,**fields,confirmed='true')[0]==303,'Gruba profil atanıp mevcut üyeye uygulandı')
check(admin.post(path,**fields,confirmed='true')[0]==409,'Eski grup sürümü yeni atamayı ezemedi')
now=native(ca)
check(all(now[k]==v for k,v in settings.items()),'Mevcut üye grup profilini aldı')
check(all(now[k]==old_a[k] for k in ['serverSecret','certificates']),'Sır ve sertifika korundu')
check(native(cb)==old_b,'Grup dışındaki sunucu değişmedi')
new,cn=register(admin,'Yeni üye',farm_a)
check(all(native(cn)[k]==v for k,v in settings.items()),'Yeni kayıt sırasında güncel grup profili otomatik uygulandı')
check(admin.post('/manage/servers/'+b,name='Taşınan üye',farm=farm_a)[0]==303,'Mevcut sunucu profilli gruba taşındı')
check(all(native(cb)[k]==v for k,v in settings.items()),'Taşınan üye güncel profili aldı')
path_b='/manage/farms/'+farm_b+'/server-profile'
assert admin.post(path_b,version=version(admin,path_b),profile=profile,profile_version=profile_version,confirmed='true')[0]==303
d,cd=register(admin,'Diğer grup üyesi',farm_b)
before=[native(c) for c in [ca,cb,cn,cd]]
changed=dict(settings,updateStatusInterval=7)
edit=dict(name='Güncel otomatik profil',version=profile_version,settings=json.dumps(changed),management=json.dumps(dict(policy,power=7)))
check(admin.post(profile_path,**edit)[0]==400,'Bağlı profil değişikliği için toplu etki onayı istendi')
check(all(native(c)==old for c,old in zip([ca,cb,cn,cd],before)),'Onaysız değişiklik hiçbir grubu kısmen değiştirmedi')
# Corrupt only one disposable synthetic fixture, after earlier sorted members.
broken=max([a,b,new,d])
encrypted=sql(f"SELECT encode(configuration,'hex') FROM servers WHERE tenant_id='{tenant}' AND id='{broken}'")
codes=json.loads(sql(f"SELECT json_object_agg(id,config_code) FROM servers WHERE tenant_id='{tenant}'"))
try:
	sql(f"UPDATE servers SET configuration=decode('00','hex') WHERE tenant_id='{tenant}' AND id='{broken}'")
	check(admin.post(profile_path,**edit,confirmed='true')[0]==503,'Bir üyenin şifreli ayarı bozuksa toplu işlem başarısız oldu')
	check(json.loads(sql(f"SELECT json_object_agg(id,config_code) FROM servers WHERE tenant_id='{tenant}'"))==codes,'Hata durumunda daha önce işlenen üyelerin sürümleri de geri alındı')
	check(sql(f"SELECT version FROM server_profiles WHERE tenant_id='{tenant}' AND id='{profile}'")==profile_version,'Başarısız dağıtım profil sürümünü değiştirmedi')
finally:
	sql(f"UPDATE servers SET configuration=decode('{encrypted}','hex') WHERE tenant_id='{tenant}' AND id='{broken}'")
check(admin.post(profile_path,**edit,confirmed='true')[0]==303,'Tek profil değişikliği iki gruba ve dört üyeye yayıldı')
check(all(native(c)['updateStatusInterval']==7 for c in [ca,cb,cn,cd]),'Çevrimdışı üyeler kalıcı güncel yapılandırmayı agent yolundan aldı')
check(sql(f"SELECT count(*) FROM servers WHERE tenant_id='{tenant}' AND farm_id IN ('{farm_a}','{farm_b}') AND management->>'power'='7.0'")=='4','Yönetim kapasite politikası da tüm üyelere uygulandı')
check(admin.post(profile_path,**edit,confirmed='true')[0]==409,'Eski profil sürümü dağıtımı ezemedi')
check(admin.post(path,version=version(admin,path),profile='',confirmed='true')[0]==303,'Profil ataması kaldırıldı')
retained=[native(c) for c in [ca,cb,cn]]
new_version=sql(f"SELECT version FROM server_profiles WHERE tenant_id='{tenant}' AND id='{profile}'")
edit.update(version=new_version,settings=json.dumps(dict(changed,updateStatusInterval=9)))
assert admin.post(profile_path,**edit,confirmed='true')[0]==303
check(all(native(c)==old for c,old in zip([ca,cb,cn],retained)),'Atamadan ayrılan grup son ayarlarını korudu ve sonraki yayımı almadı')
check(native(cd)['updateStatusInterval']==9,'Bağlı kalan grup sonraki güncellemeyi aldı')
# Stale profile revision and language return path.
stale=dict(version=version(admin,path),profile=profile,profile_version=profile_version,confirmed='true')
check(admin.post(path,**stale)[0]==409,'Atama eski profil içeriğini sessizce uygulamadı')
for lang in ['en','ar','tr']:
	code,_,headers=admin.request('/language?lang='+lang+'&return_to='+urllib.parse.quote(path,safe=''))
	check(code==303 and headers['Location']==path,'Dil değişimi grup profili sayfasında kaldı: '+lang)

# API and concurrent new membership use the same tenant serialization.
from concurrent.futures import ThreadPoolExecutor
edit.update(version=sql(f"SELECT version FROM server_profiles WHERE tenant_id='{tenant}' AND id='{profile}'"),settings=json.dumps(dict(changed,updateStatusInterval=11)))
parallel=Browser();assert parallel.login(admin_email)[0]==303;parallel.request('/panel')
with ThreadPoolExecutor(max_workers=2) as pool:
	update=pool.submit(admin.post,profile_path,**edit,confirmed='true')
	creation=pool.submit(register,parallel,'Eşzamanlı yeni üye',farm_b)
	assert update.result()[0]==303
	parallel_id,parallel_credential=creation.result()
check(native(parallel_credential)['updateStatusInterval']==11 and native(cd)['updateStatusInterval']==11,'Eşzamanlı profil kaydı ve yeni üye ekleme güncel tek sürümde birleşti')
def api(bot,path,body=None):
	headers={'Authorization':bot['authorization']};data=None
	if body is not None:data=json.dumps(body).encode();headers['Content-Type']='application/json'
	req=urllib.request.Request(BASE+'/api/manage'+path,data=data,headers=headers)
	try:response=urllib.request.urlopen(req,context=CONTEXT,timeout=20)
	except urllib.error.HTTPError as error:response=error
	return response.status,json.loads(response.read())
bots=[]
try:
	for role in ['viewer','operator']:
		admin.request('/panel')
		code,body,_=admin.post('/manage/bots',name='Grup profil API '+role,role=role,lifetime_days=1,confirmed='true')
		assert code==200;bots.append(json.loads(Fields(body).output))
	code,data=api(bots[0],'/farms/'+farm_b+'/server-profile')
	check(code==200 and data['profile']==profile and data['members']==2,'Bot API atanmış profili ve üye sayısını okudu')
	fields={'version':data['version'],'profile':'','confirmed':True}
	check(api(bots[0],'/farms/'+farm_b+'/server-profile',fields)[0]==403,'Okuyucu bot profil atamasını değiştiremedi')
	check(api(bots[1],'/farms/'+farm_b+'/server-profile',fields)[0]==202,'Operatör bot aynı onaylı atama yolunu kullandı')
finally:
	for bot in bots:
		admin.request('/panel');admin.post('/manage/bots/'+bot['id']+'/revoke',confirmed='true')

# Real prebuilt native server: restore encrypted config and membership even on failure.
control=json.loads((ROOT/'work/ssh-install-qa/control.json').read_text())
owner=Browser();assert owner.login(control['email'],control['password'])[0]==303;owner.request('/panel')
target=control['server'];real_tenant=control['tenant']
backup=json.loads(sql(f"SELECT row_to_json(s) FROM (SELECT name,farm_id,paused,configuration,config_code,management FROM servers WHERE tenant_id='{real_tenant}' AND id='{target}') s"))
backup_file=WORK/'native-backup.json';backup_file.write_text(json.dumps(backup));backup_file.chmod(0o600)
def restore():
	# Original encrypted bytes and their matching code restore the exact old server configuration.
	def quote(s):return "'"+s.replace("'","''")+"'"
	farm_sql=quote(backup['farm_id']) if backup['farm_id'] else 'NULL'
	sql(f"UPDATE servers SET name={quote(backup['name'])},farm_id={farm_sql},paused={'true' if backup['paused'] else 'false'},configuration={quote(backup['configuration'])}::bytea,config_code={quote(backup['config_code'])},management={quote(json.dumps(backup['management']))}::jsonb WHERE tenant_id='{real_tenant}' AND id='{target}'")
atexit.register(restore)
real_config=native(control['credential'])
real_settings={k:v for k,v in real_config.items() if k not in ['serverSecret','configCode','certificates','publicServerToken']}
real_settings['updateStatusInterval']=5
real_name='Gerçek profil '+uuid.uuid4().hex[:8]
assert owner.post('/manage/farms',name=real_name)[0]==303
real_farm=sql(f"SELECT id FROM farms WHERE tenant_id='{real_tenant}' AND name='{real_name}'")
assert owner.post('/manage/server-profiles',name=real_name,settings=json.dumps(real_settings),management=json.dumps(backup['management']))[0]==303
real_profile=sql(f"SELECT id FROM server_profiles WHERE tenant_id='{real_tenant}' AND name='{real_name}'")
real_version=sql(f"SELECT version FROM server_profiles WHERE tenant_id='{real_tenant}' AND id='{real_profile}'")
rp='/manage/farms/'+real_farm+'/server-profile'
assert owner.post(rp,version=version(owner,rp),profile=real_profile,profile_version=real_version,confirmed='true')[0]==303
assert owner.post('/manage/servers/'+target,name=backup['name'],farm=real_farm)[0]==303
expected=sql(f"SELECT config_code FROM servers WHERE tenant_id='{real_tenant}' AND id='{target}'")
for _ in range(40):
	ack=sql(f"SELECT status->>'configCode' FROM servers WHERE tenant_id='{real_tenant}' AND id='{target}'")
	if ack==expected:break
	time.sleep(1)
check(ack==expected,'Hazır gerçek VpnHood sunucusu gruptan aldığı sürümü uygulayıp onayladı')
restore();atexit.unregister(restore)
print('PASS: Grup profili atama, yeni/taşınan üye, iki gruba otomatik yayım, yalıtım ve gerçek native sürüm onayı tamamlandı.')
print('Private QA evidence directory:',WORK)
