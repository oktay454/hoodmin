#!/usr/bin/env python3
"""Reusable server templates and bounded telemetry through real tenant HTTPS."""
from panel_support import *

class Inputs(html.parser.HTMLParser):
	def __init__(self,body):
		super().__init__();self.values={};self.feed(body)
	def handle_starttag(self,tag,attrs):
		a=dict(attrs)
		if tag=='input' and 'name' in a:self.values[a['name']]=a.get('value','')

first,second=str(uuid.uuid4()),str(uuid.uuid4())
admin_email,_=account(first,'admin');viewer_email,_=account(first,'viewer');other_email,_=account(second,'admin')
admin,viewer,other=Browser(),Browser(),Browser()
for b,e in [(admin,admin_email),(viewer,viewer_email),(other,other_email)]:assert b.login(e)[0]==303;b.request('/panel')
a,ca,_=server(admin,'Profil A',2660);b,cb,_=server(admin,'Profil B',2661);c,cc,_=server(other,'Başka kiracı',2662)
old=agent(ca,'configure',{})
settings={'tcpEndPoints':['0.0.0.0:2660'],'updateStatusInterval':120,'NetFilter':{'includeLocalNetwork':False,'blockIpV6':True},'Session':{'syncInterval':5}}
management={'maxNormalSessions':12,'maxPremiumSessions':24,'location':'TR/Istanbul','power':2}
form={'name':'Sınama sunucu profili','settings':json.dumps(settings),'management':json.dumps(management)}
check(viewer.post('/manage/server-profiles',**form)[0]==403,'Gözlemci sunucu profili oluşturamadı')
check(admin.post('/manage/server-profiles',**form)[0]==303,'Yeniden kullanılabilir sunucu profili oluşturuldu')
body=admin.request('/manage/server-profiles')[1]
id=re.search(r'action="/manage/server-profiles/([0-9a-f-]+)"',body).group(1)
version=Inputs(body).values['version']
path=f'/manage/server-profiles/{id}'
check(other.post(path,**dict(form,version=version))[0]==404,'Başka kiracı sunucu profilini değiştiremedi')
apply=dict(version=version,servers=a+' '+c,confirmed='true')
check(admin.post(path+'/apply',**apply)[0]==404,'Başka kiracı sunucusu içeren toplu uygulama reddedildi')
check(agent(ca,'configure',{})==old,'Geçersiz toplu işlem kendi sunucusunu da değiştirmedi')
apply['servers']=a+' '+b
check(admin.post(path+'/apply',**apply)[0]==303,'Profil iki sunucuya tek işlemde uygulandı')
now=agent(ca,'configure',{});now_b=agent(cb,'configure',{})
check(all(now[k]==v and now_b[k]==v for k,v in settings.items()),'Profil ayarları iki agent yapılandırmasına geçti')
check(now['serverSecret']==old['serverSecret'] and now['certificates']==old['certificates'],'Profil uygulaması kimlik ve sertifikayı korudu')
check(admin.post(path,**dict(form,version=version))[0]==303,'Profil sürümlenerek güncellendi')
check(admin.post(path+'/apply',**apply)[0]==409,'Eski profil uygulama formu reddedildi')
check(admin.post('/manage/server-profiles',**dict(form,name='Geçersiz',settings='{"serverSecret":"overwrite"}'))[0]==400,'Profil içinde sunucu sırrını değiştirmek reddedildi')
agent(ca,'status',{'cpuUsage':25,'availableMemory':1073741824,'sessionCount':3,'tunnelSpeed':{'sent':125000,'received':125000}})
for who in [admin,viewer]:
	code,body,_=who.request('/reports?hours=24&server='+a)
	check(code==200 and '<polyline' in body and '25.00 %' in body and '2.00 Mbps' in body,'Gerçek durum örneği grafiklerde doğru birimlerle gösterildi')
check(other.request('/reports?hours=24&server='+a)[0]==404,'Başka kiracının raporu okunamadı')
check(admin.request('/reports?hours=0')[0]==400,'Desteklenmeyen rapor dönemi reddedildi')
check(admin.request('/reports?hours=744&server=')[0]==200,'Boş sunucu filtresiyle 31 günlük rapor çalıştı')
query=f"SELECT heartbeat_seconds FROM servers WHERE tenant_id='{first}' AND id='{a}'"
value=subprocess.run(['runuser','-u','postgres','--','psql','-d','hoodmin','-Atqc',query],check=True,capture_output=True,text=True).stdout.strip()
check(value=='360','Çevrimiçi toleransı ayarlanan 120 saniyenin üç katına uyarlandı')
code,body,_=admin.post('/manage/tokens',server=a,name='Bilgi sınaması',count=1,max_clients=1,max_gib=0,profile='',expires='')
key='vh://'+Fields(body).output.split('vh://',1)[1].strip()
token=json.loads(base64.b64decode(key.removeprefix('vh://')));token_id=token['tid']
details=dict(name='Güncel anahtar',group_name='Satış',description='Açıklama <script>test</script>',customer_id='customer-1',order_id='order-2',support_id='support-3')
check(admin.post(f'/manage/tokens/{token_id}/details',**details)[0]==303,'Anahtar müşteri, sipariş ve destek bilgileri düzenlendi')
check(other.post(f'/manage/tokens/{token_id}/details',**details)[0]==404,'Başka kiracı anahtar bilgilerini değiştiremedi')
code,download,headers=admin.request(f'/manage/tokens/{token_id}/download')
export=json.loads(base64.b64decode(download.removeprefix('vh://')))
check(code==200 and 'attachment;' in headers['Content-Disposition'] and export['sid']=='support-3' and export['name']=='Güncel anahtar','Anahtar indirmesi güncel ad ve destek kodunu içerdi')
check(viewer.request(f'/manage/tokens/{token_id}/download')[0]==403,'Salt okunur kullanıcı VPN sırrını indiremedi')
check('&lt;script&gt;' in admin.request(f'/manage/tokens/{token_id}/policy')[1],'Anahtar açıklaması HTML kodu çalıştırmadan gösterildi')
print('PASS: Sunucu profilleri, raporlar ve anahtar bilgileri HTTPS/PostgreSQL ile doğrulandı.')
