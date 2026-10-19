#!/usr/bin/env python3
"""Disposable loopback-only receiver for explicitly approved Console publication QA.
No authorization values or request bodies are printed. Remove the proxy when done.
"""
import hashlib
import email.parser
import email.policy
import http.server
import json
import pathlib
import sys
import time
import urllib.parse

ROOT = pathlib.Path(sys.argv[1])


class Receiver(http.server.BaseHTTPRequestHandler):
	def log_message(self, *_):
		pass

	def handle_request(self):
		config = json.loads((ROOT / 'config.json').read_text())
		if self.path not in {config['path'], config.get('upload_path', config['path'])}:
			query = urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query)
			value = query.get('qa', [''])[0]
			rejected = dict(at=time.time(), method=self.command, status=404,
				base_matches=self.path.split('?')[0] == config['path'],
				query_keys=list(query),
				marker_matches=value == config.get('expected_marker'),
				placeholder_literal=value == '{access_token}')
			with (ROOT / 'requests.jsonl').open('a') as output:
				output.write(json.dumps(rejected) + '\n')
			self.send_error(404)
			return
		try:
			length = int(self.headers.get('Content-Length', '0'))
		except ValueError:
			self.send_error(400)
			return
		if not 0 <= length <= 65536 or self.headers.get('Transfer-Encoding'):
			self.send_error(413)
			return
		body = self.rfile.read(length)
		status = config.get(self.command, 405)
		if body:
			with (ROOT / ('body-' + str(time.time_ns()))).open('wb') as output:
				output.write(body)
			if 200 <= status < 300:
				stored = body
				if config.get('json_upload'):
					value = json.loads(body)
					stored = value.encode() if isinstance(value, str) else body
				if config.get('form_field'):
					content_type = self.headers.get('Content-Type', '')
					message = email.parser.BytesParser(policy=email.policy.default).parsebytes(
						('Content-Type: ' + content_type + '\r\nMIME-Version: 1.0\r\n\r\n').encode() + body)
					stored = next(part.get_payload(decode=True) for part in message.iter_parts()
						if part.get_param('name', header='content-disposition') == config['form_field'])
				(ROOT / 'latest').write_bytes(stored)
		record = dict(at=time.time(), method=self.command, status=status,
			length=length, sha256=hashlib.sha256(body).hexdigest(),
			content_type=self.headers.get('Content-Type'), header_names=sorted(self.headers.keys()))
		if 'expected_marker' in config:
			record['qa_marker_matches'] = self.headers.get('X-Hoodmin-QA') == config['expected_marker']
		if 'upload_path' in config:
			record['upload_path_used'] = self.path == config['upload_path']
		with (ROOT / 'requests.jsonl').open('a') as output:
			output.write(json.dumps(record) + '\n')
		response = (ROOT / 'latest').read_bytes() if self.command == 'GET' and (ROOT / 'latest').exists() else b''
		if self.command == 'GET' and 'get_body' in config:
			response = config['get_body'].encode()
		self.send_response(status)
		for name, value in config.get('response_headers', {}).items():
			self.send_header(name, value)
		self.send_header('Content-Type', 'text/plain; charset=utf-8')
		self.send_header('Content-Length', str(len(response)))
		self.end_headers()
		self.wfile.write(response)

	do_GET = handle_request
	do_PUT = handle_request
	do_POST = handle_request


http.server.ThreadingHTTPServer(('127.0.0.1', 18081), Receiver).serve_forever()
