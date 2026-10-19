#!/usr/bin/env python3
"""Five-role boundary and JSON bot API through the same production handlers."""
from panel_support import *

TENANT,OTHER=str(uuid.uuid4()),str(uuid.uuid4())
people={}
for role in ['owner','admin','operator','token_writer','viewer']:
	email,id=account(TENANT,role);browser=Browser();assert browser.login(email)[0]==303;browser.request('/panel');people[role]=(browser,id)
other_email,_=account(OTHER,'admin');other=Browser();assert other.login(other_email)[0]==303;other.request('/panel')
owner,owner_id=people['owner'];admin,admin_id=people['admin'];writer,_=people['token_writer'];viewer,_=people['viewer']
sid,credential,_=server(admin,'API sınaması',2770)
check(owner.post(f'/manage/users/{owner_id}',role='viewer',active='true')[0]==400,'Son sahip rolünün kaldırılması reddedildi')
check(admin.post(f'/manage/users/{owner_id}',role='viewer',active='true')[0]==403,'Yönetici sahip rolünü değiştiremedi')
check(admin.post(f'/manage/users/{admin_id}',role='owner',active='true')[0]==403,'Yönetici kendini sahip yapamadı')
check(writer.post('/manage/servers',name='Denied',host='10.0.3.220',port=2771,farm='')[0]==403,'Anahtar yazarı sunucu oluşturamadı')
check(writer.post('/manage/profiles',name='Yazar profili',policy=json.dumps({'maxClients':1}))[0]==303,'Anahtar yazarı erişim profili oluşturdu')
check(writer.post('/manage/tokens',server=sid,name='Yazar anahtarı',count=1,max_clients=1,max_gib=0,profile='',expires='')[0]==200,'Anahtar yazarı VPN anahtarı oluşturdu')
check(viewer.post('/manage/bots',name='Denied',role='viewer',lifetime_days=1,confirmed='true')[0]==403,'Gözlemci bot oluşturamadı')

def bot(role):
	code,body,_=admin.post('/manage/bots',name='API '+role,role=role,lifetime_days=1,confirmed='true')
	check(code==200,'Bot oluşturuldu: '+role)
	settings=json.loads(Fields(body).output)
	p=WORK/(role+'-bot.json');p.write_text(json.dumps(settings));p.chmod(0o600)
	return settings

def api(settings,path,body=None):
	headers={'Authorization':settings['authorization']}
	data=None
	if body is not None:data=json.dumps(body).encode();headers['Content-Type']='application/json'
	request=urllib.request.Request(BASE+'/api/manage'+path,headers=headers,data=data)
	try:response=urllib.request.urlopen(request,context=CONTEXT,timeout=20)
	except urllib.error.HTTPError as e:response=e
	text=response.read().decode()
	try:text=json.loads(text)
	except json.JSONDecodeError:pass
	return response.status,text,response.headers

read=bot('viewer');write=bot('token_writer');operator=bot('operator');supervisor=bot('admin')
check(api(read,'/servers')[0]==200,'Okuyucu bot JSON sunucu listesini okudu')
check(api(read,'/tokens',dict(server=sid,name='Denied',count=1,max_clients=1,max_gib=0,profile='',expires=''))[0]==403,'Okuyucu bot anahtar oluşturamadı')
check(api(write,'/servers',dict(name='Denied',host='10.0.3.220',port=2772,farm=''))[0]==403,'Anahtar yazarı bot sunucu oluşturamadı')
code,result,_=api(write,'/tokens',dict(server=sid,name='Bot anahtarı',count=2,max_clients=0,max_gib=0,profile='',expires=''))
check(code==200 and len(result['keys'])==2,'Bot aynı panel işleviyle toplu anahtar üretti')
token_id=result['keys'][0]['id']
check(api(read,f'/tokens/{token_id}/key')[0]==403,'Okuyucu bot erişim sırrını alamadı')
check(api(write,f'/tokens/{token_id}/key')[0]==200,'Anahtar yazarı bot erişim anahtarını alabildi')
code,created,_=api(operator,'/servers',dict(name='Bot sunucusu',host='10.0.3.220',port=2773,farm=''))
check(code==200 and created['settings']['HttpAccessManager']['Authorization'].startswith('Bearer '),'Operatör bot sunucu ve bağlantı kimliği oluşturdu')
new=created['id'];code,config,_=api(operator,f'/servers/{new}/configuration')
check(code==200 and 'serverSecret' not in config['settings'] and 'certificates' not in config['settings'],'JSON gelişmiş ayar yanıtı korunan alanları göstermedi')
check(api(operator,f'/servers/{new}/configuration',dict(base_code=config['base_code'],settings={'serverSecret':'replace'},confirmed=True))[0]==400,'API geçersiz ayarda hata durumunu korudu')
check(api(operator,f'/servers/{new}/configuration',dict(base_code=config['base_code'],settings={'tcpEndPoints':['0.0.0.0:2773'],'Session':{'syncInterval':5}},confirmed=True))[0]==202,'JSON nesne ayarı panelin aynı doğrulayıcısıyla uygulandı')
check(api(operator,f'/servers/{new}/configuration',dict(base_code=config['base_code'],settings={'Session':{}},confirmed=True))[0]==409,'API eski yapılandırma formunu reddetti')
check(api(read,'/users')[0]==403 and api(supervisor,'/users')[0]==200,'Bot ekip okuma yetkisi rolüne göre sınırlandı')
check(owner.post(f"/manage/users/{supervisor['id']}",role='owner',active='true')[0]==400,'Bot insan üye yolundan sahip rolü alamadı')
check(other.post(f"/manage/bots/{write['id']}/revoke",confirmed='true')[0]==404,'Başka kiracı botu iptal edemedi')
check(admin.post(f"/manage/bots/{write['id']}/revoke",confirmed='true')[0]==303,'Bot iptal edildi')
check(api(write,'/servers')[0]==401,'İptal edilen anahtar sonraki istekte reddedildi')
subprocess.run(['runuser','-u','postgres','--','psql','-d','hoodmin','-v','ON_ERROR_STOP=1','-c',f"UPDATE bots SET expires_at=now()-interval '1 second' WHERE tenant_id='{TENANT}' AND id='{read['id']}'"],check=True,capture_output=True)
check(api(read,'/servers')[0]==401,'Süresi dolmuş bot anahtarı reddedildi')
check(api(dict(authorization='Bearer invalid'),'/servers')[0]==401,'Bozuk bot kimliği reddedildi')
check(api(supervisor,'/servers')[1]['items'] and all(row['id']!=sid or 'configuration' not in row for row in api(supervisor,'/servers')[1]['items']),'API listesi şifreli sunucu sırlarını taşımadı')
print('PASS: Beş rol, bot yetkileri, JSON API, iptal ve süre sonu doğrulandı.')
