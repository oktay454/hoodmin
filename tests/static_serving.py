"""Exercise shipped Nginx examples only inside the disposable runtime suite."""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time
import urllib.error
import urllib.request


def exercise(root, work, origin, config, proxy, processes, server, log, context, cli, api, request, wait_ready, run, check):
	web = Path('/usr/share/hoodmin/web')
	copy = Path('/usr/share/hoodmin/qa-frontend/web')
	shutil.copytree(web, copy)
	for path in web.rglob('*'):
		if path.is_file():
			check(path.read_bytes() == (copy / path.relative_to(web)).read_bytes(), 'Copied frontend file: ' + str(path.relative_to(web)))
	# The installed default serves the exact same bytes as direct Nginx.
	shell = (web / 'index.html').read_text()
	for route in ['/', '/servers', '/servers/', '/access-keys', '/farms', '/account', '/servers/50331fb8-2282-467b-b6a5-fc8a1fea9c71/configuration']:
		check(request(route) == (200, shell), 'Built-in deep link: ' + route)
	for route in ['/panel', '/panel/servers', '/manage/farms', '/missing', '/assets/missing.js', '/assets/locales/missing.json', '/api/manage/missing']:
		code, body = request(route)
		check(code == 404 and '<html' not in body.lower(), 'Built-in missing path stays non-HTML: ' + route)
	cli('config', 'web', '--serve-static', 'false')
	cli('config', 'set', '--database-url-file', work / 'app.url', '--public-origin', origin, '--listen', '127.0.0.1:3108')
	check(json.loads(cli('config', 'show').stdout)['web']['serve_static'] is False, 'CLI reports API-only mode')
	# Restart here to test both startup and the absence of embedded static fallbacks.
	def stop_server():
		os.killpg(server.pid, signal.SIGTERM)
		server.wait(timeout=15)
	def start_server():
		return subprocess.Popen(['runuser', '-u', 'hoodmin', '--', '/usr/bin/hoodmin', '--config', str(config), '--listen', '127.0.0.1:3108'], stdout=log, stderr=log, start_new_session=True)
	stop_server()
	server = start_server(); processes.append(server)
	wait_ready()
	for route in ['/', '/signin', '/assets/spa.js', '/assets/app.css', '/assets/locales/tr.json']:
		check(request(route)[0] == 404, 'API-only daemon does not serve static: ' + route)
	api('whoami')

	def template(name):
		text = (root / 'deploy' / name).read_text()
		# The HTTP redirect/ACME block is independent of static/API serving.
		if 'listen 80;' in text:
			text = text[:text.index('server {')] + text[text.index('server {', text.index('server {') + 8):]
		text = text.replace('listen 443 ssl;', 'listen 127.0.0.1:3459 ssl;')
		text = text.replace('/etc/letsencrypt/live/hoodmin.example.com/fullchain.pem', f'"{work}/tls/server.crt"')
		text = text.replace('/etc/letsencrypt/live/hoodmin.example.com/privkey.pem', f'"{work}/tls/server.key"')
		text = text.replace('hoodmin.example.com', 'localhost')
		return text.replace('127.0.0.1:3080', '127.0.0.1:3108')

	def replace_proxy(text):
		n = work / 'nginx.conf'
		n.write_text(f'pid {work}/nginx.pid;\nerror_log {work}/nginx.log;\nevents {{}}\nhttp {{\ninclude /etc/nginx/mime.types;\naccess_log off;\n' + text + '\n}\n')
		run(['nginx', '-t', '-c', n])
		if proxy.poll() is None:
			os.killpg(proxy.pid, signal.SIGTERM); proxy.wait(timeout=15)
		p = subprocess.Popen(['nginx', '-c', str(n), '-g', 'daemon off;'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
		processes.append(p)
		for _ in range(50):
			try:
				if request('/')[0] == 200: return p
			except OSError: pass
			time.sleep(.1)
		raise AssertionError('Nginx frontend did not start')

	for mode in ['local', 'remote']:
		if mode == 'local':
			text = template('nginx-hoodmin-static.conf')
		else:
			text = template('nginx-hoodmin-static-remote.conf').replace('/usr/share/hoodmin/web', str(copy))
			text = text.replace('proxy_pass https://hoodmin-backend.example.com;', 'proxy_pass https://127.0.0.1:3460;').replace('proxy_ssl_name hoodmin-backend.example.com;', 'proxy_ssl_name localhost;').replace('/etc/ssl/certs/ca-certificates.crt', f'"{work}/tls/ca.crt"')
			backend = template('nginx-hoodmin-backend.conf').replace('listen 127.0.0.1:3459 ssl;', 'listen 127.0.0.1:3460 ssl;').replace('192.0.2.10', '127.0.0.1').replace('/etc/letsencrypt/live/hoodmin-backend.example.com/fullchain.pem', f'"{work}/tls/server.crt"').replace('/etc/letsencrypt/live/hoodmin-backend.example.com/privkey.pem', f'"{work}/tls/server.key"')
			text += '\n' + backend
		proxy = replace_proxy(text); wait_ready()
		for route in ['/', '/servers', '/servers/', '/access-keys', '/account', '/servers/50331fb8-2282-467b-b6a5-fc8a1fea9c71/credential']:
			check(request(route) == (200, shell), mode + ' Nginx deep link: ' + route)
		for path in ['assets/spa.js', 'assets/app.css', 'assets/locales/tr.json', 'assets/locales/en.json', 'assets/locales/ar.json']:
			check(request('/' + path) == (200, (web / path).read_text()), mode + ' Nginx static bytes: ' + path)
		for route in ['/panel', '/manage/farms', '/missing', '/assets/missing.js', '/api/manage/missing', '/s/not/a-token', '/f/not/a/token']:
			code, body = request(route)
			check(code in [400,404] and 'id="app"' not in body, mode + ' does not fallback dynamic/missing routes to SPA: ' + route)
		with urllib.request.urlopen(urllib.request.Request(origin + '/servers', method='HEAD'), context=context) as response:
			check(response.read() == b'' and response.headers.get_content_type() == 'text/html' and "script-src 'self'" in response.headers['Content-Security-Policy'] and response.headers['Cache-Control'] == 'no-store', mode + ' HEAD, MIME, CSP and cache policy')
		api('whoami')
		stop_server()
		check(request('/servers') == (200, shell) and request('/assets/spa.js')[0] == 200 and request('/api/manage/me')[0] == 502, mode + ' static UI survives daemon stop; API returns 502')
		server = start_server(); processes.append(server); wait_ready(); api('whoami')
		if mode == 'remote':
			# The frontend copy works without the package's original static directory.
			web.rename(web.with_name('web.hidden'))
			try: check(request('/servers') == (200,shell), 'Remote frontend depends only on its copied directory')
			finally: web.with_name('web.hidden').rename(web)
			proxy = replace_proxy(text.replace('proxy_ssl_name localhost;', 'proxy_ssl_name wrong.example.invalid;'))
			check(request('/api/manage/me')[0] == 502, 'Remote HTTPS rejects backend hostname mismatch')
			proxy = replace_proxy(text);wait_ready()
	# Leave the real remote/static topology active for browser QA.
	return server, proxy
