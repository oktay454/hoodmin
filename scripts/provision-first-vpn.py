#!/usr/bin/python3
"""One-time deployment helper, run as root inside the new Hoodmin container."""
import html.parser
import http.cookiejar
import json
import os
from pathlib import Path
import urllib.request
import urllib.parse

class NoRedirect(urllib.request.HTTPRedirectHandler):
	def redirect_request(self,*args):return None
class Fields(html.parser.HTMLParser):
	def __init__(self,body):super().__init__(convert_charrefs=True);self.csrf='';self.output='';self.inside=False;self.feed(body)
	def handle_starttag(self,tag,attrs):
		a=dict(attrs)
		if tag=='input' and a.get('name')=='csrf':self.csrf=a.get('value','')
		if tag=='textarea' and a.get('id')=='secret-output':self.inside=True
	def handle_endtag(self,tag):
		if tag=='textarea':self.inside=False
	def handle_data(self,data):
		if self.inside:self.output+=data

assert os.geteuid()==0
owner=json.loads(Path('/root/hoodmin-first-owner.json').read_text());origin=owner['origin']
state_path=Path('/root/hoodmin-first-vpn.json')
state=json.loads(state_path.read_text()) if state_path.exists() else {}
jar=http.cookiejar.CookieJar();client=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar),NoRedirect())
def save():
	fd=os.open(state_path,os.O_CREAT|os.O_WRONLY|os.O_TRUNC,0o600)
	with os.fdopen(fd,'w') as f:json.dump(state,f,ensure_ascii=False,indent='\t')
def request(path,data=None):
	req=urllib.request.Request(origin+path,data=None if data is None else urllib.parse.urlencode(data).encode(),headers={'Origin':origin})
	try:response=client.open(req,timeout=20)
	except urllib.error.HTTPError as error:
		if error.code not in (302,303):raise SystemExit(f'Provisioning request failed: {path}, HTTP {error.code}') from None
		response=error
	return response.status,response.read().decode()
csrf=Fields(request('/signin')[1]).csrf
assert request('/signin',dict(csrf=csrf,email=owner['email'],password=owner['password']))[0]==303
csrf=Fields(request('/panel')[1]).csrf
if 'server' not in state:
	code,body=request('/manage/servers',dict(csrf=csrf,name='vh01.oktayaktogan.com.tr',host='vh01.oktayaktogan.com.tr',port=443,farm=''))
	assert code==200
	settings=json.loads(Fields(body).output)
	state.update(settings=settings,server=settings['HttpAccessManager']['Authorization'].split('.')[1]);save()
if 'bot' not in state:
	csrf=Fields(request('/manage/bots')[1]).csrf
	code,body=request('/manage/bots',dict(csrf=csrf,name='ACME vh01',role='operator',lifetime_days=365,confirmed='true'))
	assert code==200
	state['bot']=json.loads(Fields(body).output);save()
config=Path('/etc/hoodmin/acme/vh01.json');config.parent.mkdir(mode=0o700,exist_ok=True)
fd=os.open(config,os.O_WRONLY|os.O_CREAT|os.O_TRUNC,0o600)
with os.fdopen(fd,'w') as f:json.dump(dict(origin=origin,domain='vh01.oktayaktogan.com.tr',server=state['server'],authorization=state['bot']['authorization'],bot_expires_at=state['bot']['expiresAt']),f,indent='\t')
print('First VPN server and one-year operator bot created. Private deployment state saved; no credentials printed.')
