#!/usr/bin/env python3
"""Görsel QA için gerçek SSR sayfalarının sır içermeyen yerel görüntüsünü alır."""
import html.parser
import http.cookiejar
from pathlib import Path
import re
import ssl
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
BASE = "https://yapayzeka:3443"
CONTEXT = ssl.create_default_context(cafile="/etc/vpnhood-manager/tls/ca.crt")
INFO = dict(line.split(": ", 1) for line in (ROOT / "work/panel-admin.txt").read_text().splitlines())


class NoRedirect(urllib.request.HTTPRedirectHandler):
	def redirect_request(self, *args):
		return None


class Fields(html.parser.HTMLParser):
	def __init__(self, text):
		super().__init__()
		self.csrf = None
		self.feed(text)

	def handle_starttag(self, tag, attrs):
		attrs = dict(attrs)
		if tag == "input" and attrs.get("name") == "csrf":
			self.csrf = attrs.get("value")


JAR = http.cookiejar.CookieJar()
OPENER = urllib.request.build_opener(
	urllib.request.HTTPSHandler(context=CONTEXT),
	urllib.request.HTTPCookieProcessor(JAR),
	NoRedirect(),
)


def request(path, data=None):
	headers = {}
	if data is not None:
		headers["Origin"] = BASE
		data = urllib.parse.urlencode(data).encode()
	try:
		response = OPENER.open(urllib.request.Request(BASE + path, data=data, headers=headers), timeout=15)
	except urllib.error.HTTPError as error:
		response = error
	return response.status, response.read().decode()


def save(path, text):
	csrf = Fields(text).csrf
	if csrf:
		text = text.replace(csrf, "preview-only")
	text = re.sub(r'<script[^>]*src="/assets/app.js"[^>]*></script>', "", text)
	file = ROOT / "work/preview" / path.strip("/") / "index.html"
	file.parent.mkdir(parents=True, exist_ok=True)
	file.write_text(text)


code, signin = request("/signin")
assert code == 200
save("signin", signin)
code, _ = request("/signin", {"email": INFO["E-posta"], "password": INFO["Parola"], "csrf": Fields(signin).csrf})
assert code == 303
for path in ["panel", "panel/servers", "panel/tokens", "panel/sessions", "panel/users", "panel/audit", "panel/account"]:
	code, text = request("/" + path)
	assert code == 200
	save(path, text)
assets = ROOT / "work/preview/assets"
assets.mkdir(parents=True, exist_ok=True)
(assets / "app.css").write_text((ROOT / "crates/manager/assets/app.css").read_text())
print("Görsel QA sayfaları work/preview altında hazırlandı; parola/oturum çerezi kopyalanmadı.")
