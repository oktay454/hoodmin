#!/usr/bin/env python3
"""Exercise new protocol paths through HTTPS and PostgreSQL, with independent tenants."""
from panel_support import *

class FormValues(html.parser.HTMLParser):
	def __init__(self, body):
		super().__init__(convert_charrefs=True)
		self.values = {}
		self.current = None
		self.feed(body)
	def handle_starttag(self, tag, attrs):
		attrs = dict(attrs)
		if tag == "input" and "name" in attrs:
			self.values[attrs["name"]] = attrs.get("value", "")
		if tag == "textarea":
			self.current = attrs.get("name")
			self.values[self.current] = ""
	def handle_data(self, data):
		if self.current: self.values[self.current] += data
	def handle_endtag(self, tag):
		if tag == "textarea": self.current = None

def private(name, content):
	p = WORK / name
	p.write_text(content)
	p.chmod(0o600)
	os.chown(p, USER.pw_uid, USER.pw_gid)

TENANT, OTHER = str(uuid.uuid4()), str(uuid.uuid4())
admin_email,_ = account(TENANT,"admin")
viewer_email,_ = account(TENANT,"viewer")
other_email,_ = account(OTHER,"admin")
admin,viewer,other = Browser(),Browser(),Browser()
for browser,email in [(admin,admin_email),(viewer,viewer_email),(other,other_email)]:
	check(browser.login(email)[0] == 303, "Genişletilmiş sözleşme sınama hesabı giriş yaptı")
	browser.request("/panel")
server_id,credential,settings = server(admin,"Sözleşme genişletmesi",2444)
path = f"/manage/servers/{server_id}/configuration"
check(other.request(path)[0] == 404, "Başka kiracı gelişmiş ayarlara erişemedi")
code,body,_ = admin.request(path)
check(code == 200 and "serverSecret" not in FormValues(body).values["settings"], "Gelişmiş düzenleyicide sunucu sırları yer almıyor")
original = agent(credential,"configure",{})
values = FormValues(body).values
base_code = values["base_code"]
config = json.loads(values["settings"])
config.update({"tcpEndPoints":["0.0.0.0:2444","[::]:2444"],"udpEndPoints":["0.0.0.0:2445"],"quicEndPoints":[],"dnsServers":["1.1.1.1","8.8.8.8"],"updateStatusInterval":"00:00:05","logAnonymizer":True,"minCompletionPortThreads":None,"maxCompletionPortThreads":None,"swapMemorySizeMb":None,"addListenerIpsToNetwork":None,"tcpCongestionControl":None,"enableHttp01Challenge":False})
config["Session"] = {"timeout":"00:30:00","udpTimeout":"00:01:00","tcpTimeout":"00:15:00","icmpTimeout":"00:00:30","syncInterval":"00:00:05","syncCacheSize":1024,"maxPacketChannelCount":8,"maxUdpClientCount":500,"maxIcmpClientCount":20,"tcpKernelBufferSize":{"send":65536,"receive":65536},"streamProxyBufferSize":{"send":8192,"receive":8192},"udpProxyBufferSize":{"send":8192,"receive":8192},"udpChannelBufferSize":{"send":8192,"receive":8192},"tcpConnectTimeout":"00:00:30","channelIdleTimeout":"00:00:40","maxTcpConnectWaitCount":500,"maxTcpChannelCount":1000,"netScanLimit":100,"netScanTimeout":"00:01:00","allowTcpProxy":True,"allowTcpPacket":True}
config["Tracking"] = {key:False for key in ["trackClientIp","trackLocalPort","trackDestinationIp","trackDestinationPort","trackUdp","trackTcp","trackIcmp"]}
config["NetFilter"] = {"networkIsolation":True,"includeLocalNetwork":False,"vpnAdapterIncludeIpRanges":["0.0.0.0/0","::/0"],"vpnAdapterExcludeIpRanges":["198.51.100.0/24"],"includeIpRanges":["0.0.0.0/0","::/0"],"excludeIpRanges":["203.0.113.1-203.0.113.3"],"blockIpV6":False}
check(viewer.post(path,base_code=base_code,settings=json.dumps(config),confirmed="true")[0] == 403, "Salt okunur kullanıcı gelişmiş ayarları değiştiremedi")
check(admin.post(path,base_code=base_code,settings=json.dumps(config))[0] == 400, "Gelişmiş ayarlar etkisi onaylanmadan değişmedi")
check(admin.post(path,base_code=base_code,settings=json.dumps(config),confirmed="true")[0] == 303, "Tüm 47 düzenlenebilir ayar panelden kaydedildi")
actual = agent(credential,"configure",{})
check(actual["configCode"] != original["configCode"] and actual["serverSecret"] == original["serverSecret"] and actual["certificates"] == original["certificates"], "Yeni sürüm üretildi ve sunucu kimliği korundu")
check(all(actual[key] == value for key,value in config.items()), "Bütün ayarlar agent yapılandırmasına kayıpsız taşındı")
check(admin.post(path,base_code=base_code,settings=json.dumps(config),confirmed="true")[0] == 409, "Eski gelişmiş ayar formu yeni sürümü ezemedi")
new_code = actual["configCode"]
for invalid in [{"Session":{"useUdpProxy2":True}}, {"serverSecret":"replace"}, {"NetFilter":{"excludeIpRanges":["::/129"]}}]:
	check(admin.post(path,base_code=new_code,settings=json.dumps(invalid),confirmed="true")[0] == 400, "Geçersiz veya uygulanmayan ayar reddedildi")
check(agent(credential,"configure",{}) == actual, "Reddedilen ayarlar kayıtlı yapılandırmayı değiştirmedi")
agent(credential,"status",{"configCode":new_code,"configError":None,"endPointStatuses":[{"protocol":"Tcp","endPoint":"0.0.0.0:2444","error":None}]})
agent(credential,"status",{"configCode":new_code,"endPointStatuses":None})
check("Uygulandı" in admin.request(path)[1] and "0.0.0.0:2444" in admin.request(path)[1], "Sunucunun sürüm onayı ve tek seferlik dinleyici sonucu gösterildi")
policy = {"maxClients":1,"maxTotalTraffic":100000,"maxCycleTraffic":4096,"cycleSeconds":60,"maxUploadMbps":2,"maxDownloadMbps":3,"lifetimeDays":2,"deviceLifeSpan":0,"isPremium":True,"isPublic":False,"clientPolicies":[{"ccs":["*"],"n":0}]}
check(admin.post("/manage/profiles",name="Hız ve dönem",policy=json.dumps(policy))[0] == 303, "Hız ve dönem sınırları olan erişim profili oluşturuldu")
profile_page = admin.request("/manage/profiles")[1]
profile_id = re.search(r'action="/manage/profiles/([0-9a-f-]+)"',profile_page).group(1)
check(other.post(f"/manage/profiles/{profile_id}",name="No",policy=json.dumps(policy))[0] == 404, "Başka kiracı erişim profilini değiştiremedi")
changed = dict(policy,isPublic=True)
check(admin.post(f"/manage/profiles/{profile_id}",name="No",policy=json.dumps(changed))[0] == 400, "Profilin açık/özel türünün değiştirilmesi reddedildi")
code,body,_ = admin.post("/manage/tokens",server=server_id,name="Hız sınaması",count=1,max_clients=3,max_gib=0,profile=profile_id,expires="")
key = Fields(body).output.split("vh://",1)[1].strip()
token = json.loads(base64.b64decode(key))
check(code == 200 and token["cpols"][0]["ccs"] == ["*"], "Profil ile oluşturulan anahtara istemci politikaları eklendi")
private("server-a.credential",credential)
private("token-a.vh","vh://"+key)
private("expected-settings.json",json.dumps(config))
subprocess.run(["dotnet","build",str(ROOT/"tests/ContractProbe"),"-c","Release","--nologo"],check=True,capture_output=True)
session = subprocess.run(["dotnet","run","--project",str(ROOT/"tests/ContractProbe"),"-c","Release","--no-build","--","one-session",str(WORK)],check=True,capture_output=True,text=True)
session_id = int(session.stdout.strip())
request = urllib.request.Request(BASE+f"/api/agent/sessions/{session_id}",headers={"Authorization":"Bearer "+credential})
with urllib.request.urlopen(request,context=CONTEXT) as r:response=json.load(r)
check(response["accessInfo"]["maxSpeedMbps"] == {"sent":2,"received":3} and response["accessInfo"]["maxCycleTraffic"] == 4096, "Hız ve dönem kotası oturumun gerçek yanıtına işlendi")
check(response["accessInfo"]["expirationTime"] is not None and response["accessInfo"]["maxDeviceCount"] == 1, "İlk kullanımdan başlayan süre ve profil cihaz kotası uygulandı")
private("session-response.json",json.dumps(response))
check(admin.post(f"/manage/servers/{server_id}/publish")[0] == 303, "Şifreli sunucu adresi yayımı açıldı")
code,body,_ = admin.request(f"/manage/tokens/{token['tid']}/key")
updated_key = Fields(body).output
updated_token = json.loads(base64.b64decode(updated_key.removeprefix("vh://")))
url = updated_token["ser"]["urls"][0]
with urllib.request.urlopen(url,context=CONTEXT) as r:encrypted=r.read().decode()
check("." in encrypted and "serverSecret" not in encrypted and "hname" not in encrypted, "Genel adres güncelleme kaynağı yalnız şifreli belirteç sundu")
private("token-a.vh",updated_key)
private("encrypted-server-token",encrypted)
probe = subprocess.run(["dotnet","run","--project",str(ROOT/"tests/ContractProbe"),"-c","Release","--no-build","--","check-extended",str(WORK)],check=True,capture_output=True,text=True)
print(probe.stdout,end="")
result = agent(credential,f"sessions/{session_id}/usage",{"sent":2048,"received":2048})
check(result["errorCode"] == "AccessTrafficOverflow", "Dönem kotası dolunca erişim engellendi")
# Fast-forward only this disposable token's cycle, keeping its total usage.
subprocess.run(["runuser","-u","postgres","--","psql","-d","hoodmin","-v","ON_ERROR_STOP=1","-c",f"UPDATE access_tokens SET cycle_started_at=now()-interval '61 seconds' WHERE tenant_id='{TENANT}' AND id='{token['tid']}'"],check=True,capture_output=True)
request = urllib.request.Request(BASE+f"/api/agent/sessions/{session_id}",headers={"Authorization":"Bearer "+credential})
with urllib.request.urlopen(request,context=CONTEXT) as r:response=json.load(r)
check(response["errorCode"] == "Ok" and response["accessUsage"]["cycleTraffic"] == {"sent":0,"received":0} and response["accessUsage"]["totalTraffic"] == {"sent":2048,"received":2048}, "Yeni dönemde yalnız dönem sayacı sıfırlandı; toplam trafik korundu")
private("server-settings.json",json.dumps(settings))
if not os.environ.get("VHM_TEST_ISOLATED"):
	(ROOT/"work/latest-contract-features").write_text(str(WORK))
check(admin.request("/panel/tokens")[0] == 200, "Yeni özelliklerden sonra panel çalışıyor")
print("PASS: Genişletilmiş yönetim sözleşmesi sınaması tamamlandı.")
print("Sınama çalışma dizini:",WORK)
