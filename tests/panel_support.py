#!/usr/bin/env python3
"""Ayrılmış test kapsayıcısında gerçek HTTPS, PostgreSQL ve .NET ile panel sınaması."""
import base64
import html.parser
import http.cookiejar
import json
import os
from pathlib import Path
import pwd
import re
import secrets
import ssl
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]
BASE = os.environ.get("VHM_TEST_ORIGIN", "https://vhm-test.oktayaktogan.com.tr")
CONTEXT = ssl.create_default_context(cafile=os.environ.get("VHM_TEST_CA"))
BINARY = os.environ.get("VHM_TEST_BINARY", "/usr/bin/hoodmin-cli")
ENV = dict(os.environ, DATABASE_URL=os.environ.get("VHM_TEST_DATABASE_URL", "postgresql:///hoodmin?host=/var/run/postgresql&user=hoodmin"), VHM_MASTER_KEY_FILE="/var/lib/hoodmin/master.key")
WORK = Path(tempfile.mkdtemp(prefix="panel-test.", dir=ROOT / "work"))
USER = pwd.getpwnam("hoodmin")
os.chown(WORK, USER.pw_uid, USER.pw_gid)
PASSWORD = secrets.token_urlsafe(24)
PASSFILE = WORK / "password"
PASSFILE.write_text(PASSWORD)
PASSFILE.chmod(0o600)
os.chown(PASSFILE, USER.pw_uid, USER.pw_gid)

class Fields(html.parser.HTMLParser):
	def __init__(self, content):
		super().__init__()
		self.csrf = None
		self.confirmation = None
		self.output = ""
		self.in_output = False
		self.feed(content)
	def handle_starttag(self, tag, attrs):
		attrs = dict(attrs)
		if tag == "input" and attrs.get("name") == "csrf":
			self.csrf = attrs.get("value")
		if tag == "input" and attrs.get("name") == "confirmation":
			self.confirmation = attrs.get("value")
		if tag == "textarea" and attrs.get("id") == "secret-output":
			self.in_output = True
	def handle_endtag(self, tag):
		if tag == "textarea":
			self.in_output = False
	def handle_data(self, data):
		if self.in_output:
			self.output += data

class NoRedirect(urllib.request.HTTPRedirectHandler):
	def redirect_request(self, req, fp, code, msg, headers, newurl):
		return None

class Browser:
	def __init__(self):
		self.jar = http.cookiejar.CookieJar()
		self.open = urllib.request.build_opener(urllib.request.HTTPSHandler(context=CONTEXT), urllib.request.HTTPCookieProcessor(self.jar), NoRedirect())
		self.csrf = None
	def request(self, path, data=None, origin=BASE, extra=None):
		headers = dict(extra or {})
		if data is not None:
			headers["Origin"] = origin
			data = urllib.parse.urlencode(data).encode()
		request = urllib.request.Request(BASE + path, data=data, headers=headers)
		try:
			response = self.open.open(request, timeout=20)
		except urllib.error.HTTPError as error:
			response = error
		body = response.read().decode()
		if response.status == 200:
			self.csrf = Fields(body).csrf or self.csrf
		return response.status, body, response.headers
	def login(self, address, password=PASSWORD):
		code, body, _ = self.request("/signin")
		assert code == 200
		return self.request("/signin", {"email": address, "password": password, "csrf": Fields(body).csrf})
	def post(self, path, **fields):
		return self.request(path, {"csrf": self.csrf, **fields})

def check(condition, message):
	if not condition:
		raise AssertionError(message)
	print("PASS:", message)

def account(tenant, role):
	address = f"{role}.{uuid.uuid4().hex[:10]}@vpnhood.test"
	result = subprocess.run(["runuser", "-u", "hoodmin", "--preserve-environment", "--", BINARY, "create-account", "--tenant", tenant, "--tenant-name", "Panel sınaması", "--email", address, "--password-file", str(PASSFILE), "--role", role], env=ENV, capture_output=True, text=True, check=True)
	identity = re.search(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", result.stdout).group()
	return address, identity

def server(browser, name, port):
	code, body, _ = browser.post("/manage/servers", name=name, host="10.0.3.220", port=port, farm="")
	check(code == 200, "Panelden sunucu ve bağlantı ayarları oluşturuldu")
	settings = json.loads(Fields(body).output)
	credential = settings["HttpAccessManager"]["Authorization"].removeprefix("Bearer ")
	return credential.split(".")[1], credential, settings

def agent(credential, path, payload):
	request = urllib.request.Request(BASE + "/api/agent/" + path, data=json.dumps(payload).encode(), headers={"Authorization": "Bearer " + credential, "Content-Type": "application/json"})
	with urllib.request.urlopen(request, context=CONTEXT, timeout=15) as response:
		return json.load(response)
