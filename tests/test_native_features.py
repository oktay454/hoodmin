#!/usr/bin/env python3
"""Native ready-release server: certificate pin, HTTP01 relay and actual payloads.
Uses only the isolated server created by test_install_jobs; never the user's node.
"""
from panel_support import *
import socketserver
import threading
import atexit
import sys

class Values(html.parser.HTMLParser):
	def __init__(self, body):
		super().__init__(convert_charrefs=True)
		self.values={};self.current=None;self.feed(body)
	def handle_starttag(self, tag, attrs):
		a=dict(attrs)
		if tag=='input' and 'name' in a:self.values[a['name']]=a.get('value','')
		if tag=='textarea':self.current=a.get('name');self.values[self.current]=''
	def handle_data(self,data):
		if self.current:self.values[self.current]+=data
	def handle_endtag(self,tag):
		if tag=='textarea':self.current=None

control=json.loads((ROOT/'work/ssh-install-qa/control.json').read_text())
admin=Browser();assert admin.login(control['email'],control['password'])[0]==303
sid=control['server'];credential=control['credential'];path=f'/manage/servers/{sid}'

def values(suffix):return Values(admin.request(path+suffix)[1]).values

def applied():
	for _ in range(40):
		if 'Uygulandı' in admin.request(path+'/configuration')[1]:return
		time.sleep(2)
	raise AssertionError('Native server did not acknowledge configuration')

def configure(config):
	form=values('/configuration')
	check(admin.post(path+'/configuration',base_code=form['base_code'],settings=json.dumps(config),confirmed='true')[0]==303,'Yerli sunucu ayarı panelden kaydedildi')
	applied()

initial=json.loads(values('/configuration')['settings'])
( WORK/'before-native-settings.json').write_text(json.dumps(initial))
(WORK/'before-native-settings.json').chmod(0o600)
config=dict(initial)
config.update(tcpEndPoints=['0.0.0.0:2550'],udpEndPoints=['0.0.0.0:2551'],quicEndPoints=['0.0.0.0:2552'],updateStatusInterval='00:00:05')
config['Session']={**config.get('Session',{}),'syncInterval':'00:00:05','syncCacheSize':1024,'allowTcpProxy':True}
config['NetFilter']={**config.get('NetFilter',{}),'includeLocalNetwork':True,'includeIpRanges':['10.0.3.236/32'],'excludeIpRanges':['203.0.113.0/24'],'blockIpV6':False}
configure(config)
def cleanup_native():
	cleanup=dict(config);cleanup['enableHttp01Challenge']=False
	cleanup['NetFilter']={**config['NetFilter'],'includeLocalNetwork':False,'includeIpRanges':['0.0.0.0/0','::/0'],'excludeIpRanges':[]}
	configure(cleanup)
atexit.register(cleanup_native)

if '--traffic-only' not in sys.argv:
	# Password is a generated fixture, passed through a private file, not process argv.
	pfx_password=secrets.token_urlsafe(24)
	password_file=WORK/'pfx-password';password_file.write_text(pfx_password);password_file.chmod(0o600)
	key=WORK/'cert-key.pem';cert=WORK/'cert.pem';pfx=WORK/'cert.pfx'
	subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(key),'-out',str(cert),'-days','2','-subj','/CN=hoodmin-cert-test.invalid','-addext','subjectAltName=DNS:hoodmin-cert-test.invalid'],check=True,capture_output=True)
	key.chmod(0o600)
	subprocess.run(['openssl','pkcs12','-export','-inkey',str(key),'-in',str(cert),'-out',str(pfx),'-passout','file:'+str(password_file)],check=True,capture_output=True)
	pfx.chmod(0o600)
	encoded=base64.b64encode(pfx.read_bytes()).decode()
	base=values('/certificates')['base_code']
	fields=dict(base_code=base,pfx=encoded,password='incorrect-password',tls_name='hoodmin-cert-test.invalid',make_default='true',confirmed='true')
	check(admin.post(path+'/certificates',**fields)[0]==400,'Yanlış PFX parolası reddedildi')
	check(values('/certificates')['base_code']==base,'Reddedilen PFX sunucu sürümünü değiştirmedi')
	fields['password']=pfx_password
	check(admin.post(path+'/certificates',**fields)[0]==303,'Parolalı PFX panelden yüklenip varsayılan yapıldı')
	check(admin.post(path+'/certificates',**fields)[0]==409,'Eski PFX formu yeni sertifikayı ezemedi')
	applied()
	expected=subprocess.run(['openssl','x509','-in',str(cert),'-outform','DER'],check=True,capture_output=True).stdout
	# Explicit pin verification for the private self-signed test fixture.
	pin_context=ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT);pin_context.check_hostname=False;pin_context.verify_mode=ssl.CERT_NONE
	import socket
	with socket.create_connection(('10.0.3.220',2550),timeout=10) as connection:
		with pin_context.wrap_socket(connection,server_hostname='hoodmin-cert-test.invalid') as tls:
			check(tls.getpeercert(binary_form=True)==expected,'Hazır sunucu yeni PFX sertifikasını gerçekten TLS üzerinde sundu')

	challenge=secrets.token_urlsafe(24);proof=challenge+'.'+secrets.token_urlsafe(32)
	check(admin.post(path+'/challenge',token=challenge,key_authorization=proof,confirmed='true')[0]==303,'HTTP-01 doğrulaması panelden hazırlandı')
	applied()
	with urllib.request.urlopen('http://10.0.3.220/.well-known/acme-challenge/'+challenge,timeout=15) as response:
		check(response.read().decode()==proof,'Hazır VPN sunucusu HTTP-01 yanıtını Hoodmin üzerinden aldı')
	try:
		urllib.request.urlopen('http://10.0.3.220/.well-known/acme-challenge/'+secrets.token_urlsafe(24),timeout=10)
	except urllib.error.HTTPError as e:check(e.code==404,'Bilinmeyen HTTP-01 tokeni 404 döndürdü')
	else:raise AssertionError('Unknown challenge unexpectedly succeeded')

policy={'maxClients':0,'maxUploadMbps':0,'maxDownloadMbps':1,'maxTotalTraffic':0,'maxCycleTraffic':0,'isPremium':True}
check(admin.post('/manage/profiles',name='Gerçek aktarım 1 Mbps '+uuid.uuid4().hex[:8],policy=json.dumps(policy))[0]==303,'Sınırsız cihaz ve 1 Mbps indirme profili oluşturuldu')
profile=re.findall(r'action="/manage/profiles/([0-9a-f-]+)"',admin.request('/manage/profiles')[1])[0]
code,body,_=admin.post('/manage/tokens',server=sid,name='Gerçek aktarım',count=1,max_clients=0,max_gib=0,profile=profile,expires='')
check(code==200,'Sınırsız cihaz anahtarı üretildi')
access=WORK/'native-token.vh';access.write_text('vh://'+Fields(body).output.split('vh://',1)[1].strip());access.chmod(0o600)

contacts=[]
class Payload(socketserver.BaseRequestHandler):
	def handle(self):
		if self.client_address[0]!='10.0.3.220':return
		contacts.append(1)
		self.request.settimeout(20)
		if self.request.recv(1)!=b'\x01':return
		try:self.request.sendall(b'Z'*(12*1024*1024))
		except (BrokenPipeError,ConnectionResetError):pass
class Server(socketserver.ThreadingTCPServer):
	allow_reuse_address=True;daemon_threads=True

try:
	with Server(('10.0.3.236',18080),Payload) as payload:
		thread=threading.Thread(target=payload.serve_forever,daemon=True);thread.start()
		for mode,ip,count in [('tcp','10.0.3.236',12*1024*1024),('quic','10.0.3.236',256*1024),('blocked','10.0.3.236',0)]:
			if mode=='blocked':
				denied=dict(config);denied['NetFilter']={**config['NetFilter'],'excludeIpRanges':['10.0.3.236/32']}
				configure(denied)
				before=len(contacts)
			probe=subprocess.run(['dotnet','run','--project',str(ROOT/'tests/ConnectionProbe'),'-c','Release','--no-build','--',str(access),mode,ip,'18080',str(count)],capture_output=True,text=True,timeout=85)
			if probe.returncode:print(probe.stdout[-3000:],probe.stderr[-3000:])
			check(probe.returncode==0,'Gerçek VPN kanal sınaması: '+mode)
			print(probe.stdout,end='')
			if mode=='blocked':check(len(contacts)==before,'Engellenen hedefe sunucudan bağlantı kurulmadı')
		payload.shutdown()
finally:
	# Do not leave the disposable VPN node exposing HTTP80 or private destinations.
	cleanup_native()
	atexit.unregister(cleanup_native)
print('PASS: '+('TCP hız, QUIC veri ve ağ filtresi' if '--traffic-only' in sys.argv else 'PFX, HTTP-01, TCP hız, QUIC veri ve ağ filtresi')+' gerçek hazır sunucuda doğrulandı.')
print('Sınama çalışma dizini:',WORK)
