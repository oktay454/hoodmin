#!/usr/bin/env python3
"""Run only against the named disposable vpnhood container, with temporary credentials."""
from panel_support import *
import sys

QA=ROOT/"work/ssh-install-qa"
mode=sys.argv[1] if len(sys.argv)>1 else "key"
control_path=QA/"control.json"
if control_path.exists():
	control=json.loads(control_path.read_text())
else:
	tenant=str(uuid.uuid4());email,_=account(tenant,"admin");browser=Browser();assert browser.login(email)[0]==303;browser.request("/panel")
	server_id,credential,settings=server(browser,"Resmi SSH kurulum sınaması",2550)
	control={"email":email,"password":PASSWORD,"server":server_id,"credential":credential,"tenant":tenant}
	control_path.write_text(json.dumps(control));control_path.chmod(0o600)
admin=Browser();assert admin.login(control["email"],control["password"])[0]==303;admin.request("/panel")
server_id=control["server"]
path=f"/manage/servers/{server_id}/installation"
fingerprint=(QA/"host-fingerprint").read_text().split()[1]
key=(QA/"id_ed25519").read_text()
password=(QA/"password").read_text()
fields={"host":"10.0.3.220","port":22 if mode=="key" else 22522,"username":"root" if mode=="key" else "hoodmin-install-qa","fingerprint":fingerprint,"private_key":key if mode=="key" else "","password":password if mode=="password" else "","sudo_password":password if mode=="password" else "","credential":"","confirmed":"true"}

def latest_job():
	body=admin.request(path)[1]
	return re.search(r'data-install-job="(/manage/install-jobs/[0-9a-f-]+)"',body).group(1)

def wait_job(job):
	for _ in range(140):
		code,body,_=admin.request(job);assert code==200
		state=json.loads(body)
		if state["status"] in ["Succeeded","Failed"]:return state
		time.sleep(3)
	raise AssertionError("SSH job did not finish")

if mode=="key":
	wrong=dict(fields,fingerprint="SHA256:"+base64.b64encode(bytes(32)).decode().rstrip('='))
	check(admin.post(path,**wrong)[0]==303,"Uyuşmayan parmak izi için ayrı sınama işi açıldı")
	result=wait_job(latest_job())
	check(result["status"]=="Failed" and "eşleşmiyor" in result["log"],"Yanlış SSH sunucu anahtarına bağlantı kurulmadı")
check(admin.post(path,**fields)[0]==303,"SSH kurulum işi sıraya alındı: "+mode)
job=latest_job();result=wait_job(job)
if result["status"]!="Succeeded":
	# Logs have already been redacted by the backend; never print input credentials here.
	print(result["log"][-5000:])
check(result["status"]=="Succeeded","Resmi hazır kurulum SSH üzerinden tamamlandı: "+mode)
check(key not in result["log"] and password not in result["log"] and control["credential"] not in result["log"],"İş günlüğüne SSH/agent sırları yazılmadı")
check(not (Path('/run/vpnhood-manager')/job.rsplit('/',1)[1]).exists(),"SSH kimlik dosyaları tamamlanan işten sonra silindi")
for _ in range(20):
	page=admin.request(f"/manage/servers/{server_id}/configuration")[1]
	if "Uygulandı" in page:break
	time.sleep(2)
check("Uygulandı" in page,"Kurulan hazır sunucu Hoodmin yapılandırmasını uyguladığını bildirdi")
print("PASS: Gerçek SSH kurulum sınaması tamamlandı: "+mode)
