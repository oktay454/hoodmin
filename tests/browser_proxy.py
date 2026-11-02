#!/usr/bin/python3
"""Loopback-only browser fixture for the disposable runtime QA installation.
Validates the QA certificate and hostname. Test cookies are isolated from production.
Requires an active test_runtime_cli.py session. No production targets are accepted.
"""
import http.client, http.server, json, socket, ssl
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
TLS=ssl.create_default_context(cafile=str(ROOT/'work/browser-qa/ca.crt'))
class QA(http.client.HTTPSConnection):
	def __init__(self):super().__init__('yapayzeka',3459,context=TLS,timeout=25)
	def connect(self):self.sock=TLS.wrap_socket(socket.create_connection(('10.0.3.236',3459),timeout=25),server_hostname='yapayzeka')
class Handler(http.server.BaseHTTPRequestHandler):
	def log_message(self,*args):pass
	def do_GET(self):self.handle_request()
	def do_POST(self):self.handle_request()
	def handle_request(self):
		if self.path.startswith('/api/'):
			connection=QA();headers={'Origin':'https://yapayzeka:3459','Accept':'application/json'}
			for name in ['Content-Type','Cookie']:
				if name in self.headers:headers[name]=self.headers[name].replace('hoodmin_qa_session=','__Host-vhm_session=').replace('hoodmin_qa_login=','__Host-vhm_login=')
			try:
				connection.request(self.command,self.path,body=self.rfile.read(int(self.headers.get('Content-Length','0'))),headers=headers);r=connection.getresponse();body=r.read()
				self.send_response(r.status)
				self.send_header('Content-Type',r.getheader('Content-Type','application/json'))
				for key,value in r.getheaders():
					if key.lower()=='set-cookie':self.send_header(key,value.replace('__Host-vhm_session=','hoodmin_qa_session=').replace('__Host-vhm_login=','hoodmin_qa_login=').replace('; Secure',''))
				self.end_headers();self.wfile.write(body)
			except (OSError,http.client.HTTPException):self.send_response(503);self.send_header('Content-Type','application/json');self.end_headers();self.wfile.write(b'{"error":"QA unavailable"}')
			finally:connection.close()
		else:
			path=self.path.split('?')[0]
			if path.startswith('/assets/locales/'):
				name=path.rsplit('/',1)[1]
				if name not in ['tr.json','en.json','ar.json']:self.send_error(404);return
				file=ROOT/'crates/manager/locales'/name;kind='application/json'
			elif path in ['/assets/spa.js','/assets/app.css']:
				file=ROOT/'crates/manager/assets'/path.rsplit('/',1)[1];kind='text/javascript' if path.endswith('.js') else 'text/css'
			else:file=ROOT/'crates/manager/assets/index.html';kind='text/html'
			self.send_response(200);self.send_header('Content-Type',kind+'; charset=utf-8');self.end_headers();self.wfile.write(file.read_bytes())
http.server.ThreadingHTTPServer(('127.0.0.1',33480),Handler).serve_forever()
