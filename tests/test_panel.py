#!/usr/bin/env python3
from panel_support import *

for attempt in range(20):
	try:
		with urllib.request.urlopen(BASE + "/healthz", context=CONTEXT, timeout=3) as ready:
			if ready.status == 200:
				break
	except (urllib.error.URLError, urllib.error.HTTPError):
		if attempt == 19:
			raise
		time.sleep(0.25)

anonymous = Browser()
check(anonymous.request("/panel")[0] == 303, "Kimliksiz panel erişimi giriş sayfasına yönlendirildi")
TENANT_A, TENANT_B = str(uuid.uuid4()), str(uuid.uuid4())
admin_email, admin_id = account(TENANT_A, "admin")
viewer_email, viewer_id = account(TENANT_A, "viewer")
operator_email, _ = account(TENANT_A, "operator")
other_email, _ = account(TENANT_B, "admin")
admin, viewer, operator, other = Browser(), Browser(), Browser(), Browser()
check(admin.login(admin_email, "wrong-password")[0] == 401, "Yanlış parola reddedildi")
code, _, headers = admin.login(admin_email)
check(code == 303, "Doğru parola ile panel oturumu açıldı")
cookie_headers = " ".join(headers.get_all("Set-Cookie"))
check(all(flag in cookie_headers for flag in ["__Host-vhm_session", "Secure", "HttpOnly", "SameSite=Strict"]), "Güvenli oturum çerezi bayrakları uygulandı")
for browser, address in [(viewer, viewer_email), (operator, operator_email), (other, other_email)]:
	check(browser.login(address)[0] == 303, "Kiracı rolüyle giriş başarılı")
	check(browser.request("/panel")[0] == 200, "Yetkili panel sayfası oluşturuldu")
code, body, headers = admin.request("/panel")
check(code == 200 and "Genel bakış" in body, "Leptos genel bakış ekranı çalışıyor")
check(headers.get("Cache-Control") == "no-store" and "frame-ancestors 'none'" in headers.get("Content-Security-Policy", ""), "Panel önbellek ve içerik güvenliği başlıkları uygulandı")
check(admin.request("/manage/farms", {"csrf": "wrong", "name": "No"})[0] == 403, "Yanlış CSRF kanıtı reddedildi")
check(admin.request("/manage/farms", {"csrf": admin.csrf, "name": "No"}, origin="https://attacker.invalid")[0] == 403, "Yabancı Origin reddedildi")
check(viewer.post("/manage/farms", name="No")[0] == 403, "Salt okunur rolün yazması engellendi")
check(operator.request("/panel/users")[0] == 403, "Operatör kullanıcı yönetimini açamadı")
check(operator.post("/manage/users", email="x@vpnhood.test", password=PASSWORD, role="admin")[0] == 403, "Operatör yönetici hesabı oluşturamadı")
check(admin.post(f"/manage/users/{admin_id}", role="viewer", active="false")[0] == 400, "Son yönetici devre dışı bırakılamadı")
check(operator.post("/manage/farms", name="QA bölge")[0] == 303, "Operatör sunucu grubu oluşturabildi")
check(admin.post("/manage/tenant", name="QA çalışma alanım")[0] == 303, "Yönetici çalışma alanı adını güncelleyebildi")
check(operator.post("/manage/tenant", name="No")[0] == 403, "Operatör çalışma alanı ayarını değiştiremedi")
check(admin.post("/manage/farms", name="<script>alert(1)</script>")[0] == 303, "HTML içeren ad veri olarak kaydedildi")
code, body, _ = admin.request("/panel/servers")
check("<script>alert(1)</script>" not in body and "&lt;script&gt;" in body, "Kayıtlı HTML adı güvenle kaçırıldı")
server_a, credential_a, settings = server(admin, "QA kendi sunucum", 2444)
server_b, _, _ = server(other, "QA başka kiracının sunucusu", 2445)
check(admin.post(f"/manage/servers/{server_b}", name="No", farm="")[0] == 404, "Başka kiracının sunucusu düzenlenemedi")
check("QA başka kiracının sunucusu" not in admin.request("/panel/servers")[1], "Başka kiracının sunucusu listelenmedi")
check(admin.request(f"/manage/servers/{server_b}/credential")[0] == 404, "Başka kiracının kimlik onayı açılamadı")
check(viewer.request(f"/manage/servers/{server_a}/credential")[0] == 403, "Salt okunur rol kimlik onayını açamadı")
check(admin.post("/manage/tokens", server=server_b, name="No", count=1, max_clients=1, max_gib=1, expires="")[0] == 404, "Başka kiracının sunucusuna anahtar üretilemedi")
code, body, _ = admin.post("/manage/tokens", server=server_a, name="=1+1", count=3, max_clients=2, max_gib=1, expires="")
keys = re.findall(r"vh://[A-Za-z0-9+/=]+", Fields(body).output)
check(code == 200 and len(keys) == 3, "Üç erişim anahtarı tek işlemde üretildi")
tokens = [json.loads(base64.b64decode(key.removeprefix("vh://"))) for key in keys]
ids = [token["tid"] for token in tokens]
check(admin.post("/manage/tokens/bulk", ids=ids[0] + " " + str(uuid.uuid4()), action="revoke")[0] == 404, "Geçersiz kimlik içeren toplu işlem bütünüyle reddedildi")
check(all(token["v"] == 4 and len(base64.b64decode(token["sec"])) == 16 for token in tokens), "Üretilen anahtarların VpnHood biçimi doğru")
check(admin.post("/manage/tokens/bulk", ids=" ".join(ids), action="quota", max_gib=2, max_clients=3)[0] == 303, "Toplu kota güncellemesi uygulandı")
check(other.post("/manage/tokens/bulk", ids=ids[0], action="revoke")[0] == 404, "Başka kiracının anahtarı iptal edilemedi")
check(admin.post("/manage/tokens/bulk", ids=" ".join(ids[1:]), action="revoke")[0] == 303, "Toplu anahtar iptali uygulandı")
check(admin.post("/manage/tokens/bulk", ids=ids[0], action="expiry", expires="2030-01-01T00:00:00Z")[0] == 303, "Anahtar süresi topluca güncellendi")
check(admin.post("/manage/tokens/bulk", ids=ids[0], action="expiry", expires="invalid")[0] == 400, "Geçersiz son kullanma zamanı reddedildi")
check(admin.post("/manage/tokens/bulk", ids=ids[0], action="expiry", expires="")[0] == 303, "Anahtar tekrar süresiz yapılabildi")
check(admin.post("/manage/tokens", server=server_a, name="Paged", count=100, max_clients=1, max_gib=0, expires="")[0] == 200, "Yüz anahtar tek işlemde üretilebildi")
first_page = admin.request("/panel/tokens")[1]
second_page = admin.request("/panel/tokens?p=2")[1]
check(first_page.count('class="token-select"') == 100 and second_page.count('class="token-select"') == 3, "Yüzü aşan anahtarlar sayfalanarak görüntülendi")
check(admin.request("/panel/tokens?q=Paged")[1].count('class="token-select"') == 100, "Anahtar araması veri tabanında uygulandı")
code, csv, _ = admin.request("/export/usage.csv")
check(code == 200 and "\"'=1+1" in csv, "Kullanım CSV'si formül enjeksiyonunu etkisizleştirdi")
for name in ["servers", "tokens", "sessions", "users", "audit", "account"]:
	check(admin.request("/panel/" + name)[0] == 200, "Panel sayfası hazır: " + name)
code, audit, _ = admin.request("/panel/audit")
check("Toplu anahtar işlemi" in audit and PASSWORD not in audit and credential_a not in audit, "Denetim kaydı işlem içeriyor, sır içermiyor")
check(admin.post(f"/manage/users/{viewer_id}", role="viewer")[0] == 303, "Yönetici kullanıcıyı devre dışı bırakabildi")
check(viewer.request("/panel")[0] == 303, "Devre dışı kullanıcının mevcut oturumu engellendi")
check(operator.post("/manage/password", current_password="wrong", new_password=secrets.token_urlsafe(24))[0] == 400, "Parola değişimi mevcut parolayı doğruladı")
NEW_PASSWORD = secrets.token_urlsafe(24)
check(operator.post("/manage/password", current_password=PASSWORD, new_password=NEW_PASSWORD)[0] == 303, "Parola değişimi tamamlandı")
check(operator.request("/panel")[0] == 303, "Parola değişiminde eski oturum kapatıldı")
operator = Browser()
check(operator.login(operator_email, NEW_PASSWORD)[0] == 303, "Yeni parola ile giriş yapılabildi")
operator.request("/panel")
check(operator.post("/logout")[0] == 303 and operator.request("/panel")[0] == 303, "Çıkış sunucu oturumunu geçersizleştirdi")

# Dil tercihi ve yönlendirme güvenliği.
for language, direction, heading in [("en", "ltr", "Servers"), ("ar", "rtl", "الخوادم"), ("tr", "ltr", "Sunucular")]:
	code, _, headers = admin.request("/language?lang=" + language + "&return_to=%2Fpanel%2Fservers%3Fq%3DQA")
	check(code == 303 and headers["Location"] == "/panel/servers?q=QA", "Dil değişimi aramayı ve yolu koruyor: " + language)
	code, html, _ = admin.request("/panel/servers")
	check(code == 200 and f'lang="{language}"' in html and f'dir="{direction}"' in html and heading in html, "Sunucu tarafı dil ve yön doğru: " + language)
	check("QA kendi sunucum" in html, "Kullanıcı verisi çevrilmedi: " + language)
check(admin.request("/language?lang=de")[0] == 400, "Desteklenmeyen dil reddedildi")
code, _, headers = admin.request("/language?lang=tr&return_to=https%3A%2F%2Fevil.invalid")
check(code == 303 and headers["Location"] == "/panel", "Dil tercihi açık yönlendirmeye izin vermiyor")

# .NET tarafında gerçek SessionRequestEx üretip bildirim yolunu sınar.
for filename, content in [("server-a.credential", credential_a), ("token-a.vh", keys[0])]:
	file = WORK / filename
	file.write_text(content)
	file.chmod(0o600)
# Adres düzenlemesi yalnız bu ayrılmış kiracının sınama sunucusuna uygulanır.
before_config = agent(credential_a, "configure", {})
before_code = agent(credential_a, "status", {})["configCode"]
check(other.post(f"/manage/servers/{server_a}", name="No", host="203.0.113.55", port=8443, farm="")[0] == 404, "Başka kiracının sunucu adresi değiştirilemedi")
check(admin.request(f"/manage/servers/{server_a}", {"csrf":"invalid", "name":"QA", "host":"203.0.113.55", "port":8443})[0] == 403, "Adres düzenlemesi CSRF denetiminden geçiyor")
for host, port, expected in [("203.0.113.55",8443,["203.0.113.55:8443"]),("2001:db8::55",9443,["[2001:db8::55]:9443"]),("vpn.endpoint.test",8443,[])]:
	check(admin.post(f"/manage/servers/{server_a}", name="QA kendi sunucum", host=host, port=port, farm="")[0] == 303, "Sunucu adresi ve portu düzenlendi: " + host)
	config = agent(credential_a, "configure", {})
	code = agent(credential_a, "status", {})["configCode"]
	check(code != before_code and config["configCode"] == code, "Yeni yapılandırma sürümü agent bildirimine taşındı")
	check(config["tcpEndPoints"] == [f"0.0.0.0:{port}",f"[::]:{port}"], "TCP dinleme portları güncellendi")
	check(config["serverSecret"] == before_config["serverSecret"] and config["certificates"] == before_config["certificates"], "Sunucu sırrı ve sertifikası korundu")
	before_code = code
check('value="vpn.endpoint.test"' in admin.request("/panel/servers")[1], "Düzenleme formu kayıtlı FQDN ile dolduruldu")
check(admin.post(f"/manage/servers/{server_a}", name="QA kendi sunucum", host="vpn.endpoint.test", port=8443, farm="")[0] == 303 and agent(credential_a,"status",{})["configCode"] == before_code, "Değişmeyen adres gereksiz yapılandırma sürümü üretmedi")
for invalid_host, invalid_port in [("https://bad.example/path",8443),("999.0.0.1",8443),("vpn.endpoint.test",0)]:
	check(admin.post(f"/manage/servers/{server_a}", name="QA kendi sunucum", host=invalid_host, port=invalid_port, farm="")[0] == 400, "Geçersiz adres/port reddedildi")
	check(agent(credential_a,"configure",{}) == config, "Geçersiz düzenleme yapılandırmayı değiştirmedi")
code, body, _ = admin.post("/manage/tokens", server=server_a, name="Adres sınaması", count=1, max_clients=1, max_gib=0, expires="")
updated_key = json.loads(base64.b64decode(Fields(body).output.split("vh://",1)[1].strip()))
check(code == 200 and updated_key["ser"]["hname"] == "vpn.endpoint.test" and updated_key["ser"]["hport"] == 8443, "Yeni erişim anahtarı güncellenen FQDN ve portu taşıyor")
check(updated_key["ser"]["sec"] == tokens[0]["ser"]["sec"] and updated_key["ser"]["ch"] == tokens[0]["ser"]["ch"], "Yeni adres için sunucu sırrı ve sertifika parmak izi korundu")

configuration_check = subprocess.run(["dotnet", "run", "--project", str(ROOT / "tests/ContractProbe"), "-c", "Release", "--no-build", "--", "check-config", str(WORK)], capture_output=True, text=True, check=True)
check("PASS:" in configuration_check.stdout, "Panelin sertifika ve yapılandırması gerçek .NET tipleriyle doğrulandı")
result = subprocess.run(["dotnet", "run", "--project", str(ROOT / "tests/ContractProbe"), "-c", "Release", "--no-build", "--", "one-session", str(WORK)], capture_output=True, text=True, check=True)
session_id = int(result.stdout.strip())
check(other.post(f"/manage/sessions/{session_id}/close")[0] == 404, "Başka kiracının VPN oturumu kapatılamadı")
check(admin.post(f"/manage/sessions/{session_id}/close")[0] == 303, "Panelden gerçek .NET oturumu kapatıldı")
command = agent(credential_a, "status", {})
check(command["sessionResponses"][str(session_id)]["errorCode"] == "SessionClosed", "Kapatma kararı sunucuya durum yanıtında iletildi")
check(str(session_id) in agent(credential_a, "status", {})["sessionResponses"], "Karar onay alınana kadar tekrar iletildi")
agent(credential_a, "sessions/usages", [{"sessionId": session_id, "errorCode": "SessionClosed"}])
check(str(session_id) not in agent(credential_a, "status", {})["sessionResponses"], "Kapanış bildirimi alınınca bekleyen karar temizlendi")
check(admin.post(f"/manage/servers/{server_a}", name="QA kendi sunucum", host="10.0.3.220", port=2444, farm="")[0] == 303, "Sınama adresi başlangıç değerine geri getirildi")
check(admin.post(f"/manage/servers/{server_a}", name="QA kendi sunucum", farm="", paused="true")[0] == 303, "Sunucu bakım moduna alındı")
result = subprocess.run(["dotnet", "run", "--project", str(ROOT / "tests/ContractProbe"), "-c", "Release", "--no-build", "--", "one-session", str(WORK)], capture_output=True, text=True)
check(result.returncode != 0, "Bakım modunda yeni VPN oturumu reddedildi")
admin.post(f"/manage/servers/{server_a}", name="QA kendi sunucum", farm="")

# SSE ve yalnız eklenebilir denetim kayıtları.
request = urllib.request.Request(BASE + "/events")
with admin.open.open(request, timeout=20) as response:
	lines = [response.readline().decode() for _ in range(2)]
	check(any("stats" in line for line in lines), "Canlı SSE göstergeleri HTTPS üzerinden iletildi")
result = subprocess.run(["runuser", "-u", "hoodmin", "--", "psql", "-d", "vpnhood_manager", "-c", "DELETE FROM audit_events"], capture_output=True, text=True)
check(result.returncode != 0, "Uygulama veri tabanı rolü denetim kayıtlarını silemedi")

check(admin.post(f"/manage/servers/{server_a}/credential")[0] == 400, "Onaysız doğrudan kimlik yenileme reddedildi")
check("configCode" in agent(credential_a, "status", {}), "Reddedilen işlem eski kimliği korudu")
code, confirmation_page, _ = admin.request(f"/manage/servers/{server_a}/credential")
check(code == 200 and "Eski bağlantı kimliği hemen geçersizleşir." in confirmation_page, "Kimlik onayı etkiyi ve hedefi açıklıyor")
proof = Fields(confirmation_page).confirmation
check(bool(proof), "Güncel kimliğe bağlı onay kanıtı oluşturuldu")
check(admin.post(f"/manage/servers/{server_a}/credential", confirmed="true", confirmation="invalid")[0] == 409, "Geçersiz onay kanıtı reddedildi")
check("configCode" in agent(credential_a, "status", {}), "Geçersiz kanıt eski kimliği korudu")
code, body, _ = admin.post(f"/manage/servers/{server_a}/credential", confirmed="true", confirmation=proof)
check(code == 200, "Açık onayla sunucu bağlantı kimliği yenilenebildi")
settings = json.loads(Fields(body).output)
new_credential = settings["HttpAccessManager"]["Authorization"].removeprefix("Bearer ")
try:
	agent(credential_a, "status", {})
	raise AssertionError("Eski sunucu kimliği kabul edildi")
except urllib.error.HTTPError as error:
	check(error.code == 401, "Önceki sunucu kimliği anında geçersizleşti")
check("configCode" in agent(new_credential, "status", {}), "Yeni sunucu kimliği geçerli")
check(admin.post(f"/manage/servers/{server_a}/credential", confirmed="true", confirmation=proof)[0] == 409, "Aynı onay formuyla ikinci yenileme engellendi")
check("configCode" in agent(new_credential, "status", {}), "Tekrarlanan gönderim yeni kimliği bozmadı")
(WORK / "server-a.credential").write_text(new_credential)
# Gerçek uzak sunucu sınamasında kullanılacak özel dosyalar Git dışında kalır.
(WORK / "server-settings.json").write_text(json.dumps(settings))
(WORK / "server-settings.json").chmod(0o600)
(ROOT / "work/latest-panel-test").write_text(str(WORK))
# Ayrı hesapla giriş sınırını doğrular; mevcut panel hesaplarını kilitlemez.
throttle_email, _ = account(TENANT_B, "viewer")
throttled = Browser()
for _ in range(20):
	check(throttled.login(throttle_email, "wrong-password")[0] == 401, "Sınır içindeki yanlış giriş reddedildi")
check(throttled.login(throttle_email, "wrong-password")[0] == 429, "Yirmi giriş denemesinden sonra hesap hız sınırı uygulandı")
check(throttled.login(throttle_email)[0] == 429, "Hız sınırı doğru parola ile de aşılamadı")
print("PASS: Panel uçtan uca sınamaları tamamlandı.")
print("Sınama çalışma dizini:", WORK)
