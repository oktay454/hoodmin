"""Disposable QA-only public-address transport fixture. Always remove trust/DNS/address changes."""
import base64, http.server, json, socketserver, ssl, threading
from pathlib import Path
HOST='hoodmin-notify-qa.example.test'
ADDRESS='93.184.216.34'
class NotificationFixture:
	def __init__(self,work,run):self.work=work;self.run=run;self.servers=[];self.added=False;self.trusted=False;self.hosted=False
	def start(self):
		run=self.run;work=self.work
		assert HOST not in Path('/etc/hosts').read_text()
		self.hosts=Path('/etc/hosts').read_text()
		run(['ip','addr','add',ADDRESS+'/32','dev','lo']);self.added=True
		with Path('/etc/hosts').open('a') as file:file.write(f'\n{ADDRESS} {HOST} # hoodmin-notification-qa\n')
		self.hosted=True
		run(['openssl','req','-new','-newkey','rsa:2048','-nodes','-subj','/CN='+HOST,'-keyout',work/'notify.key','-out',work/'notify.csr'])
		(work/'notify.ext').write_text('subjectAltName=DNS:'+HOST+'\n')
		run(['openssl','x509','-req','-days','1','-in',work/'notify.csr','-CA',work/'db-ca.crt','-CAkey',work/'db-ca.key','-CAcreateserial','-extfile',work/'notify.ext','-out',work/'notify.crt'])
		self.ca=Path('/usr/local/share/ca-certificates/hoodmin-notification-qa.crt');assert not self.ca.exists();self.ca.write_bytes((work/'db-ca.crt').read_bytes());self.trusted=True;run(['update-ca-certificates'])
		context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(work/'notify.crt',work/'notify.key')
		class HTTP(http.server.BaseHTTPRequestHandler):
			def log_message(self,*args):pass
			def do_POST(self):
				data=self.rfile.read(int(self.headers['Content-Length']))
				json.loads(data)
				with (work/'webhook-received.jsonl').open('ab') as f:f.write(data+b'\n')
				self.send_response(204);self.send_header('Content-Length','0');self.end_headers()
		class SMTP(socketserver.StreamRequestHandler):
			def handle(self):
				self.connection.settimeout(25);self.wfile.write(b'220 hoodmin QA\r\n');authenticated=False
				while line:=self.rfile.readline(65536):
					verb=line.split(b' ',1)[0].strip().upper()
					if verb==b'EHLO':self.wfile.write(b'250-hoodmin\r\n250 AUTH PLAIN\r\n')
					elif verb==b'AUTH':
						parts=line.strip().split()
						if len(parts)==2:self.wfile.write(b'334 \r\n');encoded=self.rfile.readline(65536).strip()
						else:encoded=parts[-1]
						authenticated=base64.b64decode(encoded) in [b'\x00qa\x00qa-smtp-password',b'qa\x00qa\x00qa-smtp-password'];self.wfile.write(b'235 Authenticated\r\n' if authenticated else b'535 Rejected\r\n')
					elif verb in [b'MAIL',b'RCPT'] and authenticated:self.wfile.write(b'250 OK\r\n')
					elif verb==b'DATA' and authenticated:
						self.wfile.write(b'354 Continue\r\n');body=b''
						while (part:=self.rfile.readline(65536))!=b'.\r\n':
							if not part:break
							body+=part
						(work/'smtp-received.eml').write_bytes(body);self.wfile.write(b'250 Accepted\r\n')
					elif verb==b'QUIT':self.wfile.write(b'221 Bye\r\n');break
					else:self.wfile.write(b'500 Unexpected\r\n')
		for port,handler in [(55441,HTTP),(55442,SMTP)]:
			server=socketserver.ThreadingTCPServer((ADDRESS,port),handler);server.daemon_threads=True;server.socket=context.wrap_socket(server.socket,server_side=True);self.servers.append(server);threading.Thread(target=server.serve_forever,daemon=True).start()
	def close(self):
		for server in self.servers:server.shutdown();server.server_close()
		if self.trusted:self.ca.unlink(missing_ok=True);self.run(['update-ca-certificates'])
		if self.hosted:Path('/etc/hosts').write_text(Path('/etc/hosts').read_text().replace(f'\n{ADDRESS} {HOST} # hoodmin-notification-qa\n',''))
		if self.added:self.run(['ip','addr','del',ADDRESS+'/32','dev','lo'])
