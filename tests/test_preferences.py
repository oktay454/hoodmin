#!/usr/bin/env python3
"""Genel HTTPS adresinde kimliksiz tema/dil tercihi sözleşmesini sınar."""
import http.cookiejar
import os
import urllib.error
import urllib.parse
import urllib.request

BASE = os.environ.get("VHM_TEST_ORIGIN", "https://vhm-test.oktayaktogan.com.tr")
class NoRedirect(urllib.request.HTTPRedirectHandler):
	def redirect_request(self, *args):
		return None
JAR = http.cookiejar.CookieJar()
OPENER = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(JAR), NoRedirect())
def request(path, headers=None):
	try:
		response = OPENER.open(urllib.request.Request(BASE + path, headers=headers or {}), timeout=15)
	except urllib.error.HTTPError as response:
		return response.status, response.read().decode(), response.headers
	with response:
		return response.status, response.read().decode(), response.headers

def check(condition, message):
	assert condition, message
	print("PASS:", message, flush=True)

code, html, _ = request("/signin")
check(code == 200 and 'data-theme="light"' in html, "Varsayılan açık tema sunucudan oluşturuldu")
for theme in ["dark", "light"]:
	code, _, headers = request("/theme?theme=" + theme + "&return_to=%2Fsignin")
	cookie = headers.get("Set-Cookie", "")
	check(code == 303 and headers.get("Location") == "/signin", "JavaScript olmadan tema tercihi: " + theme)
	check(all(part in cookie for part in ["__Host-vhm_theme=" + theme, "Secure", "HttpOnly", "SameSite=Lax", "Path=/", "Max-Age=31536000"]), "Tercih çerezi güvenli ve kalıcı: " + theme)
	code, html, _ = request("/signin")
	check(code == 200 and f'data-theme="{theme}"' in html and f'name="color-scheme" content="{theme}"' in html, "Yeni istekte tercih ve yerel kontrollerin teması korundu: " + theme)
code, body, headers = request("/theme?theme=dark&return_to=%2Fsignin", {"Accept": "application/json"})
check(code == 204 and not body and headers.get("Cache-Control") == "no-store", "Anlık tema değişimi yönlendirme ve sayfa gövdesi göndermedi")
code, html, _ = request("/signin")
check(code == 200 and 'data-theme="dark"' in html, "Anlık değişimin tercihi yeni istekte korundu")
code, _, headers = request("/theme?theme=light&return_to=https%3A%2F%2Fevil.invalid")
check(code == 303 and headers.get("Location") == "/panel", "Dış adrese yönlendirme engellendi")
code, _, headers = request("/theme?theme=dark&return_to=%2Fpanel%3Fq%3Dtest")
check(code == 303 and headers.get("Location") == "/panel?q=test", "Geçerli iç yol ve arama korundu")
for language, direction, error in [("en", "ltr", "Unsupported theme."), ("ar", "rtl", "المظهر غير مدعوم."), ("tr", "ltr", "Desteklenmeyen tema.")]:
	request("/language?lang=" + language + "&return_to=%2Fsignin")
	code, html, _ = request("/signin")
	check(code == 200 and 'data-theme="dark"' in html and f'lang="{language}"' in html and f'dir="{direction}"' in html, "Dil değişimi temayı koruyor: " + language)
	code, html, headers = request("/theme?theme=neon")
	check(code == 400 and error in html and not headers.get("Set-Cookie"), "Geçersiz tercih çerezi değiştirmeden seçilen dilde reddedildi: " + language)
code, html, _ = request("/signin", {"Cookie": '__Host-vhm_theme=dark" onload="invalid'})
check(code == 200 and 'data-theme="light"' in html and 'onload="invalid' not in html, "Geçersiz tema çerezi HTML niteliğine taşınmadı")
print("PASS: Tema/dil HTTPS sınamaları tamamlandı.")
