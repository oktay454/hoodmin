#!/usr/bin/env python3
from panel_support import *

class Values(html.parser.HTMLParser):
	def __init__(self,body):
		super().__init__(convert_charrefs=True);self.values={};self.current=None;self.feed(body)
	def handle_starttag(self,tag,attrs):
		attrs=dict(attrs)
		if tag=="input" and attrs.get("name"):self.values[attrs["name"]]=attrs.get("value","")
		if tag=="textarea":self.current=attrs.get("name");self.values[self.current]=""
	def handle_data(self,data):
		if self.current:self.values[self.current]+=data
	def handle_endtag(self,tag):
		if tag=="textarea":self.current=None

def request_for(key):
	p=WORK/"token-a.vh";p.write_text(key);p.chmod(0o600)
	result=subprocess.run(["dotnet","run","--project",str(ROOT/"tests/ContractProbe"),"-c","Release","--no-build","--","make-request",str(WORK)],check=True,capture_output=True,text=True)
	return json.loads((WORK/result.stdout.strip()).read_text())

def issue(server,profile=""):
	code,body,_=admin.post("/manage/tokens",server=server,profile=profile,name="Grup sınaması",count=1,max_clients=10,max_gib=0,expires="")
	check(code==200,"Grup sınaması anahtarı oluşturuldu")
	key="vh://"+Fields(body).output.split("vh://",1)[1].strip()
	return key,json.loads(base64.b64decode(key[5:]))

def routing(server,**changes):
	path=f"/manage/servers/{server}/routing";code,body,_=admin.request(path);assert code==200
	v=Values(body).values;policy=json.loads(v["policy"]);policy.update(changes)
	check(admin.post(path,base_code=v["base_code"],policy=json.dumps(policy),advertisement=v["advertisement"],confirmed="true")[0]==303,"Sunucu yönlendirme politikası kaydedildi")

TENANT=str(uuid.uuid4());email,_=account(TENANT,"admin");admin=Browser();assert admin.login(email)[0]==303;admin.request("/panel")
admin.post("/manage/farms",name="Yönlendirme grubu")
body=admin.request("/panel/servers")[1]
farm=re.search(r'<option value="([0-9a-f-]+)"[^>]*>Yönlendirme grubu',body).group(1)
servers=[]
for name,port in [("Grup A",2544),("Grup B",2545)]:
	code,body,_=admin.post("/manage/servers",name=name,host="10.0.3.220",port=port,farm=farm)
	settings=json.loads(Fields(body).output);cred=settings["HttpAccessManager"]["Authorization"][7:];servers.append((cred.split('.')[1],cred))
(a,ca),(b,cb)=servers
c,cc,_=server(admin,"Grup dışı",2546)
for cred in [ca,cb,cc]:agent(cred,"configure",{"logicalCoreCount":2});agent(cred,"status",{"sessionCount":0})
routing(a,location="TR/Istanbul",power=1)
routing(b,location="DE/Frankfurt",power=4,pushTokenToClients=True)
profile={"farmAccess":True,"maxClients":10,"isPremium":True}
check(admin.post("/manage/profiles",name="Grup erişimi",policy=json.dumps(profile))[0]==303,"Açık grup yetkisi olan profil oluşturuldu")
profile_id=re.search(r'action="/manage/profiles/([0-9a-f-]+)"',admin.request("/manage/profiles")[1]).group(1)
key,token=issue(a,profile_id)
request=request_for(key);request["serverLocation"]="DE/*"
redirect=agent(ca,"sessions",request)
check(redirect["errorCode"]=="RedirectHost" and redirect["redirectServerTokens"][0]["hport"]==2545,"Konum isteği başka grup üyesine doğru sertifika/adresle yönlendirildi")
request["allowRedirect"]=False
session=agent(cb,"sessions",request);sid=session["sessionId"]
check(session["errorCode"]=="Ok" and session["serverLocation"]=="DE/Frankfurt" and session.get("accessKey","").startswith("vh://"),"Yönlendirilen üye aynı grup anahtarıyla oturum açtı ve güncel anahtarı iletti")
check(agent(cc,"sessions",request)["errorCode"]=="AccessError","Aynı kiracıdaki grup dışı sunucu aynı anahtarı kullanamadı")
result=agent(cb,f"sessions/{sid}/usage",{"sent":111,"received":222})
check(result["accessUsage"]["totalTraffic"]=={"sent":111,"received":222},"Grup üyesinin kullanımı ortak anahtar sayacına işlendi")
routing(b,maxPremiumSessions=1)
new_request=request_for(key);new_request["allowRedirect"]=False;new_request["serverLocation"]="DE/*"
check(agent(cb,"sessions",new_request)["errorCode"]=="NoServerAvailable","Sunucunun premium oturum kapasitesi uygulandı")
agent(cb,f"sessions/{sid}/usage?closeSession=true",{"sent":0,"received":0})
check(agent(cb,"sessions",new_request)["errorCode"]=="Ok","Oturum kapanınca sunucu kapasitesi serbest kaldı")
routing(b,maxPremiumSessions=0)
premium_key,premium_token=issue(b)
code,body,_=admin.post(f"/manage/tokens/{premium_token['tid']}/access-code",confirmed="true");access_code=Fields(body).output
req=request_for(key);req["accessCode"]=access_code;req["allowRedirect"]=False
upgraded=agent(cb,"sessions",req)
check(code==200 and upgraded["errorCode"]=="Ok" and upgraded["accessInfo"]["isPremium"],"Geçerli erişim koduyla hedef anahtarın erişimi uygulandı")
req["accessCode"]="invalid-code"
check(agent(cb,"sessions",req)["errorCode"]=="AccessCodeRejected","Sahte erişim kodu reddedildi")
# A provider's verified callback is necessary; client AdData is not proof.
check(admin.post("/manage/integrations",confirmed="true")[0]==200,"Doğrulanmış ödül entegrasyonu oluşturuldu")
# Rotate once in this disposable fixture to capture the one-time credential.
code,body,_=admin.post("/manage/integrations",confirmed="true");hook=json.loads(Fields(body).output)
reward_policy={"maxClients":10,"isPremium":False,"rewardedSeconds":600}
check(admin.post(f"/manage/tokens/{premium_token['tid']}/policy",policy=json.dumps(reward_policy),confirmed="true")[0]==303,"Ödüllü planın politikası kaydedildi")
req=request_for(premium_key);req["planId"]="NormalByRewardedAd";req["allowRedirect"]=False
reward=agent(cb,"sessions",req);reward_id=reward["sessionId"]
check(reward["errorCode"]=="Ok" and reward["adRequirement"]=="Rewarded","Ödüllü plan kısa doğrulama süresiyle başlatıldı")
ad_data=f"sid:{reward_id};ad:{uuid.uuid4()}"
callback=urllib.request.Request(hook["url"],data=json.dumps({"sessionId":reward_id,"adData":ad_data}).encode(),headers={"Authorization":hook["authorization"],"Content-Type":"application/json"})
with urllib.request.urlopen(callback,context=CONTEXT) as response:check(response.status==204,"Güvenilir sağlayıcının doğrulaması alındı")
result=agent(cb,"sessions/usages",[{"sessionId":reward_id,"adData":ad_data}])[str(reward_id)]
check(result["errorCode"]=="Ok" and result["accessUsage"]["expirationTime"]>reward["accessUsage"]["expirationTime"],"Doğrulanmış reklam ödülü oturum süresini uzattı")
expiry=result["accessUsage"]["expirationTime"]
check(agent(cb,"sessions/usages",[{"sessionId":reward_id,"adData":ad_data}])[str(reward_id)]["accessUsage"]["expirationTime"]==expiry,"Aynı reklam verisinin tekrarı ikinci ödül vermedi")
req=request_for(premium_key);req["planId"]="NormalByRewardedAd";req["allowRedirect"]=False
unverified=agent(cb,"sessions",req);uid=unverified["sessionId"]
result=agent(cb,"sessions/usages",[{"sessionId":uid,"adData":f"sid:{uid};ad:{uuid.uuid4()}"}])[str(uid)]
check(result["errorCode"]=="RewardedAdRejected","Doğrulaması olmayan istemci reklam bildirimi reddedildi")
print("PASS: Grup yönlendirmesi, erişim kodu ve ödül doğrulaması sınandı.")
