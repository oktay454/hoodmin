/* Static client. Session secrets stay in HttpOnly cookies; CSRF lives only in memory. */
const root = document.querySelector('#app');
let user, main, dictionary = {}, lang = preference('language', 'tr'), theme = preference('theme', 'light');
let navigation = 0, schemaCache, retryTimer, requests = new AbortController();
function preference(key, fallback) { try { return localStorage.getItem(`hoodmin.${key}`) || fallback; } catch { return fallback; } }
function remember(key, value) { try { localStorage.setItem(`hoodmin.${key}`, value); } catch { /* preferences are optional */ } }
const t = key => dictionary[key] || key;
function n(tag, attrs = {}, ...children) {
	const node = document.createElement(tag);
	for (const [key, value] of Object.entries(attrs)) {
		if (key.startsWith('on')) node.addEventListener(key.slice(2), value);
		else if (key === 'class') node.className = value;
		else if (value !== undefined && value !== null && value !== false) node.setAttribute(key, value === true ? '' : String(value));
	}
	for (const child of children.flat(Infinity)) if (child !== null && child !== undefined && child !== false) node.append(child instanceof Node ? child : document.createTextNode(String(child)));
	return node;
}
const b = (label, action, cls = '') => n('button', {type:'button', class:`button ${cls}`, onclick:() => safe(action)}, t(label));
const link = (label, path, cls = '') => n('a', {href:path, class:cls}, t(label));
const pretty = value => JSON.stringify(value, null, '\t');
const box = (title, ...content) => n('section', {class:'card spa-card'}, n('h2', {}, t(title)), content);
const actions = (...items) => n('div', {class:'spa-actions'}, items);
function help(text) { const button=b('?', () => modal(t('Yardım'), n('p', {}, t(text))), 'help-trigger');button.setAttribute('aria-label',t('Yardım'));return button; }
function notice(text, error = false) {
	const node = n('div', {class:`notice ${error ? 'error' : ''}`, role:error ? 'alert' : 'status'}, t(text));
	(main || root).prepend(node); return node;
}
async function safe(action) { try { await action(); } catch (error) { if(error.name==='AbortError')return; if (error.status === 401) return go('/signin'); notice(error.message || 'İşlem tamamlanamadı.', true); } }
async function api(path, data, options = {}) {
	const headers = {Accept:'application/json'};
	if (data !== undefined) headers['Content-Type'] = 'application/json';
	const response = await fetch(`/api/manage${path}`, {method:data === undefined ? 'GET' : 'POST', credentials:'same-origin', cache:'no-store', headers, body:data === undefined ? undefined : JSON.stringify({...data, csrf:options.csrf ?? user?.csrf}), signal:options.signal || (data === undefined ? requests.signal : undefined)});
	let body; try { body = await response.json(); } catch { body = {error:'Hizmet geçici olarak kullanılamıyor. Lütfen daha sonra yeniden deneyin.'}; }
	if (data === undefined && requests.signal.aborted) throw new DOMException('Aborted','AbortError');
	if (!response.ok) throw Object.assign(new Error(t(body.error || 'İşlem tamamlanamadı.')), {status:response.status});
	return body;
}
async function schema() { return schemaCache ||= await api('/schema'); }
function modal(title, content) {
	const dialog = n('dialog', {class:'spa-modal'}, n('header', {}, n('h2', {}, title), b('Kapat', () => dialog.close())), content);
	dialog.addEventListener('close', () => dialog.remove()); document.body.append(dialog); dialog.showModal(); return dialog;
}
function confirm(message) {
	return new Promise(resolve => {
		let answered = false;
		const dialog = modal(t('İşlemi onaylayın'), n('p', {}, t(message)));
		dialog.append(actions(b('Vazgeç', () => dialog.close()), b('Onayla', () => { answered = true; dialog.close(); }, 'primary')));
		dialog.addEventListener('close', () => resolve(answered), {once:true});
	});
}
function secret(value) {
	const area = n('textarea', {readonly:true, rows:14, dir:'ltr', spellcheck:false}); area.value = typeof value === 'string' ? value : pretty(value);
	modal(t('Gizli bilgi'), n('div', {}, n('p', {}, t('Bu bilgiyi güvenli saklayın; yalnız yetkili kişilerle paylaşın.')), area, b('Kopyala', () => navigator.clipboard.writeText(area.value))));
}
function field(label, name, type = 'text', value = '', options) {
	let input;
	if (type === 'select' || type === 'select-json') {
		input = n('select', {name}, (options || []).map(([v,l]) => n('option', {value:v, selected:String(value ?? '') === String(v)}, t(l))));
	} else if (type === 'textarea' || type === 'json') {
		input = n('textarea', {name, rows:type === 'json' ? 7 : 4, dir:'ltr', spellcheck:false}); input.value = type === 'json' ? pretty(value) : value ?? '';
	} else {
		input = n('input', {name, type:type === 'optional-number' ? 'number' : type === 'nullable-text' ? 'text' : type, autocomplete:type === 'password' ? 'new-password' : 'off'});
		if (type === 'checkbox') input.checked = Boolean(value); else input.value = value ?? '';
		if (type === 'number' || type === 'optional-number') input.step = 'any';
	}
	input.dataset.kind = type;
	const wrapper = n('label', {class:`spa-field ${type === 'checkbox' ? 'spa-check' : ''}`}, n('span', {}, t(label)), input);
	return wrapper;
}
function values(form) {
	const result = {};
	for (const input of form.querySelectorAll('[name]')) {
		const kind = input.dataset.kind;
		result[input.name] = kind === 'checkbox' ? input.checked : kind === 'number' ? Number(input.value) : kind === 'optional-number' ? (input.value === '' ? null : Number(input.value)) : (kind === 'json' || kind === 'select-json') ? JSON.parse(input.value) : kind === 'nullable-text' ? (input.value === '' ? null : input.value) : input.type === 'datetime-local' && input.value ? new Date(input.value).toISOString() : input.value;
	}
	return result;
}
function form(title, fields, submit, options = {}) {
	const output = n('div', {class:'spa-form-message', role:'status'});
	const button = n('button', {type:'submit', class:'button primary'}, t(options.label || 'Kaydet'));
	const f = n('form', {class:'spa-form', onsubmit:async event => {
		event.preventDefault(); output.replaceChildren(); button.disabled = true;
		try {
			const data = values(f);
			if (options.confirm && !await confirm(options.confirm)) return;
			const result = await submit(data);
			for (const input of f.querySelectorAll('input[type=password]')) input.value = '';
			if (result?.value !== undefined) secret(result.value);
			if (result?.keys) secret(result.keys);
			if (result?.settings?.HttpAccessManager) secret({appsettings:result.settings, Linux:result.commands?.[0], Windows:result.commands?.[1]});
			output.append(n('p', {}, t('İşlem tamamlandı.')));
			if (options.done) await options.done(result);
		} catch (error) { output.append(n('p', {class:'error', role:'alert'}, t(error.message))); }
		finally { button.disabled = false; }
	}}, fields, actions(button), output);
	if(options.readonly) for(const input of f.querySelectorAll('input,select,textarea,button[type=submit]')) input.disabled=true;
	return box(title, f);
}
function editDialog(title, fields, endpoint, extra = {}, message = 'Değişiklikleri uygulamak istiyor musunuz?') {
	let dialog;
	const content = form(title, fields, data => api(endpoint, {...data, ...extra, confirmed:true}), {confirm:message, done:() => {dialog.close(); return render();}});
	dialog = modal(t(title), content);
}
function editor(value, prefix = '') {
	const group = n('fieldset', {class:'spa-editor'});
	for (const [key, v] of Object.entries(value || {})) {
		const path = prefix ? `${prefix}.${key}` : key;
		if (v && typeof v === 'object' && !Array.isArray(v)) group.append(n('details', {open:true}, n('summary', {}, key), editor(v, path)));
		else group.append(field(key, path, typeof v === 'boolean' ? 'checkbox' : typeof v === 'number' ? 'number' : (Array.isArray(v) || v === null) ? 'json' : 'text', v));
	}
	return group;
}
function objectValues(flat) {
	const result = Object.create(null);
	for (const [path, value] of Object.entries(flat)) {
		const parts = path.split('.'); if(parts.some(p => ['__proto__','prototype','constructor'].includes(p))) throw new Error('Invalid field'); let target = result;
		for (const key of parts.slice(0,-1)) target = target[key] ||= {};
		target[parts.at(-1)] = value;
	}
	return result;
}
function table(headers, rows) {
	return n('div', {class:'table-wrap'}, n('table', {}, n('thead', {}, n('tr', {}, headers.map(h => n('th', {}, t(h))))), n('tbody', {}, rows.length ? rows.map(row => n('tr', {}, row.map(cell => n('td', {}, cell ?? '—')))) : n('tr', {}, n('td', {colspan:headers.length, class:'muted'}, t('Kayıt yok.'))))));
}
function page(title, ...children) { main.append(n('div', {class:'section-heading'}, n('h1', {}, t(title)), b('Yenile', render)), n('div',{class:'spa-page'},children)); document.title = `${t(title)} · Hoodmin`; }
const admin = () => ['owner','admin'].includes(user?.role);
const write = () => admin() || user?.role === 'operator';
const tokensWrite = () => write() || user?.role === 'token_writer';
const roleLabel=role => t(({owner:'Sahip',admin:'Yönetici',operator:'Operatör',token_writer:'Erişim anahtarı yazarı',viewer:'Salt okunur'})[role] || role);
const roleOptions = ['owner','admin','operator','token_writer','viewer'].map(v => [v,roleLabel(v)]);
async function changeLanguage(value) { lang = ['tr','en','ar'].includes(value) ? value : 'tr'; remember('language',lang); dictionary = await (await fetch(`/assets/locales/${lang}.json`)).json(); document.documentElement.lang = lang; document.documentElement.dir = lang === 'ar' ? 'rtl' : 'ltr'; }
function appearance() { document.documentElement.dataset.theme = theme; }
function controls() {
	const select = n('select', {'aria-label':t('Dil'), onchange:event => safe(async () => {await changeLanguage(event.target.value); await render();})}, [['tr','Türkçe'],['en','English'],['ar','العربية']].map(([code,label]) => n('option', {value:code, selected:lang === code},label)));
	const toggle = b(theme === 'dark' ? '☀' : '☾', () => {theme = theme === 'dark' ? 'light' : 'dark'; remember('theme',theme); appearance(); toggle.textContent = theme === 'dark' ? '☀' : '☾'; toggle.setAttribute('aria-label',t(theme === 'dark' ? 'Açık tema' : 'Koyu tema'));}, 'theme-toggle');
	toggle.setAttribute('aria-label',t(theme === 'dark' ? 'Açık tema' : 'Koyu tema'));
	return actions(select,toggle);
}
function shell() {
	main = n('main', {class:'content spa-content', id:'main'});
	const selected=location.pathname.startsWith('/manage/servers/') ? '/panel/servers' : location.pathname.startsWith('/manage/farms/') ? '/manage/farms' : location.pathname==='/' ? '/panel' : location.pathname;
	const nav = [['Genel bakış','/panel'],['Sunucular','/panel/servers'],['Sunucu grupları','/manage/farms'],['Erişim anahtarları','/panel/tokens'],['Erişim profilleri','/manage/profiles'],['Sunucu profilleri','/manage/server-profiles'],['VPN oturumları','/panel/sessions'],['Raporlar','/reports'],['Denetim kaydı','/panel/audit'],...(admin() ? [['Kullanıcılar','/panel/users'],['Botlar','/manage/bots'],['Bildirimler','/manage/notifications'],['Entegrasyonlar','/manage/integrations']] : [])];
	root.replaceChildren(n('div', {class:'app-shell'}, link('İçeriğe geç','#main','skip-link'), n('aside', {class:'sidebar'}, n('div', {class:'brand'}, n('span',{class:'brand-mark'},'HM'), n('strong', {},'Hoodmin'), n('small', {},t('VPN yönetimi'))), b('Menü',() => {const sidebar=document.querySelector('.sidebar');const open=sidebar.classList.toggle('spa-nav-open');sidebar.querySelector('.menu-toggle').setAttribute('aria-expanded',String(open));},'menu-toggle'), n('div', {class:'workspace'}, n('strong',{},user.tenant_name), n('small',{},t('Çalışma alanı'))), n('nav', {},nav.map(([label,path]) => {const a=link(label,path,selected === path ? 'active' : '');a.prepend(icon(path.split('/').at(-1)));return a;}))), n('div', {class:'main-shell'}, n('header', {class:'topbar'}, n('strong', {}, 'Hoodmin · VPN'), n('div', {class:'profile'}, controls(), n('a',{href:'/panel/account',class:'account-link'},icon('account'),n('span',{},n('strong',{},t('Hesabım')),n('bdi',{dir:'ltr'},user.email))), n('span', {class:'badge'},roleLabel(user.role)), b('Çıkış', async () => {await api('/logout',{});user = undefined;go('/signin');}))), main)));
}
function go(path) { document.querySelectorAll('dialog').forEach(d => d.close()); history.pushState({},'',path);window.scrollTo(0,0); return render(); }
document.addEventListener('input',event => {if(main?.contains(event.target))main.dataset.dirty='true';});
document.addEventListener('click', event => {
	const a = event.target.closest('a');
	if (!a || event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || a.download || a.target || a.hash) return;
	const url = new URL(a.href); if (url.origin !== location.origin || url.pathname.startsWith('/api/')) return;
	event.preventDefault(); go(url.pathname + url.search);
});
window.addEventListener('popstate', () => render());
async function signin() {
	user = undefined;
	const nonce = await api('/login');
	main = n('main', {class:'spa-login'}); root.replaceChildren(main);
	main.append(n('header', {},n('h1',{},'Hoodmin'),n('p',{},t('VPN yönetimi')),controls()),form('Oturum aç',[field('E-posta','email','email'),field('Parola','password','password')],async data => {await api('/login',data,{csrf:nonce.csrf});await go('/panel');},{label:'Oturum aç'}));
}
async function overview() {
	const data = await api('/dashboard');
	page('Genel bakış',n('div',{class:'stats-grid'},Object.entries(data.stats).map(([k,v]) => box(({servers:'Sunucular',online:'Çevrimiçi',tokens:'Erişim anahtarları',activeSessions:'Aktif oturumlar',traffic:'Trafik'})[k] || k,n('strong',{class:'spa-metric'},v)))),box('VPN yönetimi',n('div',{class:'spa-grid'},[['Sunucular','/panel/servers'],['Erişim anahtarları','/panel/tokens'],['Raporlar','/reports']].map(([title,url]) => link(title,url,'action-tile')))));
}
async function servers() {
	const params = new URLSearchParams(location.search), data = await api('/panel/servers?' + params);
	const search = n('input',{type:'search',placeholder:t('Sunucu ara'),value:params.get('q') || '',maxlength:120,'aria-label':t('Sunucu ara')});
	const toolbar = actions(n('form',{onsubmit:event => {event.preventDefault();go('/panel/servers?q=' + encodeURIComponent(search.value));}},search,n('button',{type:'submit',class:'button'},t('Ara'))));
	if (write()) toolbar.append(b('Sunucu ekle', () => editDialog('Sunucu ekle',[field('Sunucu adı','name'),field('VPN adresi (DNS veya IP)','host'),field('TCP portu','port','number',443),field('Sunucu grubu','farm','select','',[['','Grupsuz'],...data.farms.map(f => [f.id,f.name])])],'/servers')),help('Her sunucu yalnızca kendi çalışma alanınıza aittir.'));
	page('Sunucular',toolbar,box('Sunucular',table(['Sunucu','Grup','Durum','CPU','Bellek','Oturum','İşlemler'],data.servers.map(s => [n('div',{},n('strong',{},s.name),n('small',{},s.host ? `${s.host}:${s.port}` : ''),n('small',{},t(s.seen))),s.group || t('Grupsuz'),n('span',{class:`badge ${s.online ? 'online' : ''}`},t(s.status)),s.cpu,s.memory,s.sessions,actions(write() && b('Düzenle', () => editDialog('Sunucuyu düzenle',[field('Sunucu adı','name','text',s.name),field('VPN adresi (DNS veya IP)','host','text',s.host),field('TCP portu','port','number',s.port),field('Sunucu grubu','farm','select',s.farm || '',[['','Grupsuz'],...data.farms.map(f => [f.id,f.name])]),field('Yeni oturumları durdur (bakım)','paused','checkbox',s.paused)],`/servers/${s.id}`,{},'Adres veya port değişikliği sunucunun sonraki bildiriminde uygulanır. Daha önce paylaşılan erişim anahtarlarındaki adres kendiliğinden güncellenmez.')),link('Yapılandırma',`/manage/servers/${s.id}/configuration`),link('AutoSign',`/manage/servers/${s.id}/autosign`))]))),actions(data.page > 1 && link('Önceki',`/panel/servers?q=${encodeURIComponent(data.query)}&p=${data.page-1}`),data.more && link('Sonraki',`/panel/servers?q=${encodeURIComponent(data.query)}&p=${data.page+1}`)));
}
function serverNav(id) { return actions(...[['Yapılandırma','configuration'],['Yönlendirme','routing'],['Sertifikalar','certificates'],['AutoSign','autosign'],['Kurulum','installation'],['Bağlantı kimliğini yenile','credential']].filter(([,part]) => write() || ['configuration','routing','autosign'].includes(part)).map(([label,part]) => link(label,`/manage/servers/${id}/${part}`,'spa-tab'))); }
async function serverDetail(id, kind) {
	const path = `/servers/${id}/${kind}`, data = await api(path);
	if(kind === 'configuration') await schema();
	page(data.name || 'Sunucu',serverNav(id));
	if (kind === 'configuration') {
		const applied=data.status?.configCode===data.base_code&&!data.status?.configError;
		main.append(box('Uygulama durumu',n('span',{class:'badge '+(applied ? 'online' : '')},t(applied ? 'Uygulandı' : data.status?.configError ? 'Yapılandırma hatası' : 'Sunucunun uygulaması bekleniyor')),n('p',{},t('Kaydedilen sürüm')+': ',n('code',{},data.base_code))));
		main.append(form('Yapılandırma',[help('Bu ayarlar sunucunun sonraki bildiriminde uygulanır. Yanlış ağ ayarları VPN erişimini kesebilir.'),configEditor(data.settings)],flat => api(path,{base_code:data.base_code,settings:objectValues(flat),confirmed:true}),{readonly:!write(),confirm:'Sunucu yapılandırmasını değiştirmeyi onaylayın.',done:render}),box('Sunucu durumu',n('pre',{},pretty(data.status))),form('Şifreli sunucu adresi yayımla',[],() => api(`/servers/${id}/publish`,{}),{readonly:!write(),confirm:'Şifreli sunucu adresi yayımlansın mı?',done:render}));
	} else if (kind === 'routing') {
		main.append(form('Yönlendirme',[editor({policy:data.policy,advertisement:data.advertisement}),help('Yönlendirme, grup seçimi ve reklam koşulları VPN istemcilerinin bağlanabileceği sunucuları belirler.')],flat => api(path,{base_code:data.base_code,...objectValues(flat),confirmed:true}),{readonly:!write(),confirm:'Yönlendirme değişikliğini onaylayın.',done:render}));
	} else if (kind === 'certificates') {
		certificateForm(path,data);
		main.append(form('HTTP-01 doğrulama',[field('Token','token'),field('Key authorization','key_authorization')],flat => api(`/servers/${id}/challenge`,{...flat,confirmed:true}),{confirm:'HTTP-01 doğrulamasını hazırlamak istiyor musunuz?'}));
	} else if (kind === 'autosign') {
		const o = data.order || {};
		main.append(box('AutoSign',table(['Alan','Değer'],[['Alan adı',o.domain],['Durum',t(o.status || 'Kayıt yok.')],['Son deneme',o.last_attempt],['Son başarı',o.last_success],['Son geçerlilik',o.expires_at],['Sonraki deneme',o.next_due],['Açıklama',o.message]].map(([key,value])=>[t(key),value])),help('AutoSign sertifikayı CA üzerinden alır ve VPN sunucusuna uygular. DNS ve dış TCP 80 doğrulama yolu hazırlanmış olmalı.')));
		if (admin()) main.append(form('AutoSign ayarları',[field('Alan adı','domain','text',o.domain),field('E-posta','email','email',o.email || user.email),field('Test CA kullan','staging','checkbox',o.staging ?? true),field('Etkin','enabled','checkbox',o.enabled ?? true),field('CA hizmet koşullarını kabul ediyorum','agree_tos','checkbox',false)],flat => api(path,{...flat,version:o.version || null,confirmed:true}),{confirm:'Alan adı ve e-posta CA ile paylaşılacak. Sertifika işlemini onaylıyor musunuz?',done:render}));
	} else if (kind === 'credential') {
		main.append(box('Bağlantı kimliğini yenile',n('p',{},t('Kimlik yenilenince mevcut sunucu bağlantısı kesilir. Yeni bağlantı ayarını sunucuya uygulamanız gerekir.')),b('Bağlantı kimliğini yenile',async () => {if (!await confirm('Kimlik yenilenince mevcut sunucu bağlantısı kesilir. Yeni bağlantı ayarını sunucuya uygulamanız gerekir.')) return; const response = await api(path,{confirmation:data.confirmation,confirmed:true});secret(response.value || response);await render();},'danger')));
	} else if (kind === 'installation') {
		main.append(form('SSH ile kurulum',[field('SSH adresi','host'),field('SSH portu','port','number',22),field('SSH kullanıcısı','username','text','root'),field('SSH SHA256 parmak izi','fingerprint'),field('Parola','password','password'),field('SSH özel anahtar','private_key','textarea'),field('sudo parolası','sudo_password','password'),field('Bağlantı kimliği (varsa)','credential','password'),help('Resmî VpnHood kurucusu hedef sunucuda çalıştırılır. Parola veya SSH anahtarından birini girin. Parmak izini güvenilir bir kanaldan doğrulayın.')],flat => api(path,{...flat,confirmed:true}),{confirm:'SSH üzerinden resmî VpnHood kurulumu hedef sunucuyu değiştirecek. Onaylıyor musunuz?',done:render}),box('Kurulum işleri',table(['Tarih','Durum','Günlük'],data.jobs.map(job => [job.time,t(job.status),n('pre',{},job.log)]))));
	}
}
function certificateForm(path,data) {
	main.append(box('Sertifikalar',table(['TLS adı','Son geçerlilik','Parmak izi'],data.certificates)));
	if (write()) main.append(form('PFX yükle',[field('PFX (Base64)','pfx','textarea'),field('PFX parolası','password','password'),field('TLS adı','tls_name'),field('Varsayılan yap','make_default','checkbox',true)],flat => api(path,{...flat,base_code:data.base_code,confirmed:true}),{confirm:'Sertifikayı değiştirmeyi ve VPN sunucularına uygulamayı onaylayın.',done:render}));
}
async function listPage(endpoint){const p=new URLSearchParams(location.search).get('p') || 0;return api(endpoint+'?p='+encodeURIComponent(p));}
function pager(data){main.append(actions(data.page>0&&link('Önceki',location.pathname+'?p='+(data.page-1)),data.hasMore&&link('Sonraki',location.pathname+'?p='+(data.page+1))));}
async function farms() {
	const data = await listPage('/farms');
	page('Sunucu grupları',write() && b('Grup oluştur', () => editDialog('Grup oluştur',[field('Grup adı','name')],'/farms')),box('Sunucu grupları',table(['Ad','İşlemler'],data.items.map(f => [f.name,actions(link('Sunucu profili',`/manage/farms/${f.id}/server-profile`),link('Sertifikalar',`/manage/farms/${f.id}/certificates`),link('Harici yayım',`/manage/farms/${f.id}/destinations`),write() && b('Şifreli adresi yayımla',async () => {if (await confirm('Grubun şifreli sunucu adreslerini yayımlamayı onaylayın.')) {await api(`/farms/${f.id}/publication`,{version:f.version,confirmed:true});notice('İşlem tamamlandı.');}}))]))));
	pager(data);
}
async function farmDetail(id,kind) {
	const path = `/farms/${id}/${kind}`, data = await api(path);
	page('Sunucu grubu',actions(link('Sunucu grupları','/manage/farms')));
	if (kind === 'certificates') return certificateForm(path,data);
	if (kind === 'server-profile') {
		main.append(box('Sunucu profili',n('p',{},`${t('Üye sunucu sayısı')}: ${data.members}`)));
		if (write()) main.append(form('Gruba sunucu profili ata',[field('Sunucu profili','profile','select',data.profile || '',[['','Bağımsız'],...data.profiles.map(p => [p.id,p.name])])],flat => api(path,{...flat,version:data.version,profile_version:data.profiles.find(p => p.id === flat.profile)?.version || null,confirmed:true}),{confirm:'Bağlı sunucuların ayarları değişecek. Onaylıyor musunuz?',done:render}));
	} else if (kind === 'destinations') {
		if (!admin()) {main.append(box('Harici yayım',table(['Ad','Durum'],data.items.map(d => [d.name,t(d.status)]))));return;}
		const members = (await api('/panel/servers')).servers.filter(s => s.farm === id);
		main.append(b('Yayım hedefi ekle', () => publicationDialog(id,{},members)),box('Harici yayım',table(['Ad','Durum','Son deneme','İşlemler'],data.items.map(d => [d.name,n('div',{},t(d.status),n('small',{},t(d.message))),d.last_attempt,actions(b('Düzenle',() => publicationDialog(id,d,members)),b('Şimdi yayımla',async () => {if (await confirm('Şifreli adres belirtecini dış hedefe göndermek istiyor musunuz?')) {await api(`/publications/${d.id}/push`,{version:d.version,confirmed:true});await render();}}),b('Kaldır',async () => {if (await confirm('Yayım hedefini kaldırmak istiyor musunuz?')) {await api(`/publications/${d.id}/remove`,{version:d.version,confirmed:true});await render();}},'danger'))]))),form('Tüm hedeflere yayımla',[],() => api(`/farms/${id}/push`,{confirmed:true}),{confirm:'Tüm etkin yayım hedeflerine gönderim yapılacak. Onaylıyor musunuz?',done:render}));
	}
}
function publicationDialog(farm,d,members) {
	editDialog('Yayım hedefi',[field('Ad','name','text',d.name),field('Kaynak sunucu','seed','select',d.seed,members.map(s => [s.id,s.name])),field('Yayımlanan adres','publish_url','url',d.publish_url),field('Etkin','enabled','checkbox',d.enabled ?? true),field('Yayım aralığı (dakika)','auto_minutes','number',d.auto_minutes || 0),field('Yükleme ayarlarını değiştir','replace_settings','checkbox',!d.id),field('Yükleme URL','upload_url','url'),field('Yöntem','method','select','PUT_POST',[['PUT_POST','PUT_POST'],['PUT','PUT'],['POST','POST']]),field('Erişim belirteci','secret','password'),field('Başlıklar','headers','textarea'),field('Form-Data','form_data','textarea'),field('Gövde','body','textarea','{farm_token}'),field('İçerik türü','content_type','text','text/plain'),help('0 yalnız elle; 5–1440 dakika düzenli yayım. Panel yeniden başlasa da sıradaki işler korunur.'),help('Kimlikler listede gösterilmez. Mevcut ayarları korumak için yükleme ayarlarını değiştir seçeneğini kapalı bırakın.')],d.id ? `/publications/${d.id}` : `/farms/${farm}/destinations`,{version:d.version || null},'Dış yayım ayarlarını değiştirmeyi onaylayın.');
}
async function tokenList() {
	const data = await api('/tokens?p=' + (new URLSearchParams(location.search).get('p') || 0));
	const panel = await api('/panel/tokens');
	const names = new Map(panel.servers.map(s => [s.id,s.name]));
	const selected = new Set();
	page('Erişim anahtarları');
	if (tokensWrite()) main.append(b('Anahtar oluştur',() => editDialog('Anahtar oluştur',[field('Ad','name'),field('Sunucu','server','select','',panel.servers.map(s => [s.id,s.name])),field('Erişim profili','profile','select','',[['','Özel'],...panel.profiles.map(p => [p.id,p.name])]),field('Adet','count','number',1),field('Cihaz sınırı','max_clients','number',1),field('Kota (GiB)','max_gib','number',0),field('Son kullanım','expires','datetime-local')],'/tokens')));
	main.append(box('Erişim anahtarları',table(['Seç','Ad','Sunucu','Kullanım','Durum','İşlemler'],data.items.map(k => [n('input',{type:'checkbox','aria-label':`${t('Seç')}: ${k.name}`,onchange:event => event.target.checked ? selected.add(k.id) : selected.delete(k.id)}),n('div',{},k.name,n('small',{},k.group_name)),names.get(k.server_id) || k.server_id,`${k.sent} / ${k.received}`,t(k.revoked ? 'İptal edildi' : 'Etkin'),actions(tokensWrite() && b('Anahtarı göster',async () => secret((await api(`/tokens/${k.id}/key`)).value)),tokensWrite() && b('İndir',() => download(`/tokens/${k.id}/download`,'access-key.vhkey')),tokensWrite() && b('Düzenle',() => tokenDialog(k)),tokensWrite() && b('Politika',async () => {const defaults = (await schema()).policy;let dialog;dialog=modal(t('Erişim politikası'),form('Erişim politikası',[editor({...defaults,...k.policy})],flat => api(`/tokens/${k.id}/policy`,{policy:objectValues(flat),confirmed:true}),{confirm:'Mevcut VPN oturumlarını etkileyebilecek politika değişikliğini onaylayın.',done:() => {dialog.close();return render();}}));}),tokensWrite() && b('Erişim kodu',async () => {if (await confirm('Erişim kodu oluşturmayı onaylayın.')) secret((await api(`/tokens/${k.id}/access-code`,{confirmed:true})).value);}))]))));
	if (tokensWrite()) main.append(form('Toplu işlemler',[field('İşlem','action','select','revoke',[['revoke','İptal et'],['enable','Yeniden etkinleştir'],['quota','Kota değiştir'],['expiry','Son kullanım']]),field('Kota (GiB)','max_gib','number',0),field('Cihaz sınırı','max_clients','number',1),field('Son kullanım','expires','datetime-local')],flat => api('/tokens/bulk',{...flat,ids:[...selected].join('\n')}),{confirm:'Seçili erişim anahtarlarına toplu işlem uygulanacak. Onaylıyor musunuz?',done:render}));
	main.append(actions(data.page > 0 && link('Önceki',`/panel/tokens?p=${data.page-1}`),data.hasMore && link('Sonraki',`/panel/tokens?p=${data.page+1}`)));
}
function tokenDialog(k) { editDialog('Anahtar bilgileri',['name','group_name','description','customer_id','order_id','support_id'].map(key => field(({name:'Ad',group_name:'Grup',description:'Açıklama',customer_id:'Müşteri kimliği',order_id:'Sipariş kimliği',support_id:'Destek kimliği'})[key],key,key === 'description' ? 'textarea' : 'text',k[key])),`/tokens/${k.id}/details`); }
async function download(path,name) {
	const response=await fetch(`/api/manage${path}`,{credentials:'same-origin',headers:{Accept:'application/json'}});
	if (!response.ok) throw new Error(t('İndirme başarısız.'));
	const url=URL.createObjectURL(await response.blob());const a=n('a',{href:url,download:name});a.click();setTimeout(() => URL.revokeObjectURL(url),1000);
}
async function profiles(serverProfiles = false) {
	const endpoint = serverProfiles ? '/server-profiles' : '/profiles', title = serverProfiles ? 'Sunucu profilleri' : 'Erişim profilleri';
	const data = await listPage(endpoint);
	const canEdit=serverProfiles ? write() : tokensWrite();
	async function edit(p = {}) {
		const defaults = await schema(), fields = [field('Ad','name','text',p.name)];
		if (serverProfiles) fields.push(n('div',{},field('Ayarlar (JSON)','settings','json',p.settings || {Session:{},Tracking:{},NetFilter:{}}),help('Gruba bağlı profil kaydedildiğinde değişiklikler tüm grup üyelerine uygulanır. Bağlı olmayan profiller yalnız seçilerek uygulanır. Dinleyici, ağ ve kapasite değişebilir; sertifika, kimlik ve duyurulan adres korunur.'),editor(p.management || defaults.management,'management')));
		else fields.push(editor({policy:{...defaults.policy,...p.policy}}),field('Mevcut anahtarlara uygula','apply_existing','checkbox',false));
		let dialog;
		dialog = modal(t(title),form(title,fields,flat => {
			const name=flat.name, apply_existing=flat.apply_existing;delete flat.name;delete flat.apply_existing;
			return api(endpoint + (p.id ? `/${p.id}` : ''),{name,...objectValues(flat),...(serverProfiles ? {version:p.version || null} : {apply_existing}),confirmed:true});
		},{confirm:'Profil değişikliğini ve bağlı VPN kaynaklarına etkisini onaylayın.',done:() => {dialog.close();return render();}}));
	}
	page(title,canEdit && b('Profil oluştur',() => edit()),box(title,table(['Ad','İşlemler'],data.items.map(p => [p.name,actions(canEdit && b('Düzenle',() => edit(p)),serverProfiles && write() && b('Sunuculara uygula',async () => {const servers=(await api('/panel/servers')).servers;editDialog('Sunuculara uygula',[field('Sunucu kimlikleri','servers','textarea',''),n('details',{},n('summary',{},t('Sunucu kimlikleri')),table(['Sunucu','Kimlik'],servers.map(s=>[s.name,s.id])))],`/server-profiles/${p.id}/apply`,{version:p.version},'Seçili sunucuların yapılandırması değişecek. Onaylıyor musunuz?');}))]))));
	pager(data);
}
async function sessions() {
	const data = await api('/panel/sessions');
	page('VPN oturumları',box('VPN oturumları',table(['Anahtar','Sunucu','Oluşturuldu','Durum','İşlemler'],data.sessions.map(s => [s.token,s.server,s.created,t(s.closed ? 'Kapalı' : 'Açık'),tokensWrite() && !s.closed && b('Oturumu sonlandır',async () => {if(await confirm('VPN oturumunu sonlandırmak istiyor musunuz?')) {await api(`/sessions/${s.id}/close`,{});await render();}},'danger')]))));
}
async function users() {
	const data=await listPage('/users');
	page('Kullanıcılar',b('Kullanıcı ekle',() => editDialog('Kullanıcı ekle',[field('E-posta','email','email'),field('Parola','password','password'),field('Rol','role','select','viewer',roleOptions.filter(([r])=>user.role==='owner'||r!=='owner').map(([v])=>[v,roleLabel(v)]))],'/users')),box('Kullanıcılar',table(['E-posta','Rol','Durum','İşlemler'],data.items.map(u => [u.email,roleLabel(u.role),t(u.active ? 'Etkin' : 'Devre dışı'),b('Düzenle',() => editDialog('Kullanıcı',[field('Rol','role','select',u.role,roleOptions.map(([v])=>[v,roleLabel(v)])),field('Etkin','active','checkbox',u.active)],`/users/${u.id}`))]))));
	pager(data);
}
async function bots() {
	const data=await listPage('/bots');
	page('Botlar',b('Bot oluştur',() => editDialog('Bot oluştur',[field('Ad','name'),field('Rol','role','select','viewer',roleOptions.filter(([r]) => r !== 'owner').map(([v])=>[v,roleLabel(v)])),field('Geçerlilik (gün)','lifetime_days','number',30)],'/bots')),box('Botlar',table(['Ad','Rol','Son kullanım','İşlemler'],data.items.map(bot => [bot.name,roleLabel(bot.role),bot.expires_at,!bot.revoked && b('İptal et',async () => {if(await confirm('Bot erişimini iptal etmeyi onaylayın.')){await api(`/bots/${bot.id}/revoke`,{confirmed:true});await render();}},'danger')]))));
	pager(data);
}
async function notifications() {
	const data=await api('/notifications');
	function edit(channel = {},kind = 'webhook') {
		const target=channel.target || {}, fields=[field('Ad','name','text',channel.name),field('Etkin','enabled','checkbox',channel.enabled ?? true),field('Hedefi değiştir','replace','checkbox',!channel.id)];
		kind=target.kind || kind;
		if(kind === 'webhook') fields.push(field('HTTPS adresi','url','url'),field('Bearer belirteci','bearer','password'));
		else fields.push(field('SMTP sunucusu','host','text',target.host),field('SMTP portu','port','number',target.port || 465),field('Kullanıcı adı','username'),field('Parola','password','password'),field('Gönderen','from','email',target.from),field('Alıcı','to','email',target.to),help('SMTP bağlantısı başlangıçtan itibaren TLS kullanır (SMTPS). Hedef genel ağda olmalı ve sertifikası doğrulanmalıdır.'));
		let dialog;
		dialog=modal(t('Bildirim kanalı'),form('Bildirim kanalı',fields,flat => {const{name,enabled,replace,...target}=flat;return api(`/notifications/${channel.id || crypto.randomUUID()}`,{name,enabled,version:channel.version || null,target:replace ? {kind,...target} : null,confirmed:true});},{confirm:'Bildirim hedefini kaydetmeyi onaylayın.',done:() => {dialog.close();return render();}}));
	}
	page('Bildirimler',actions(b('Webhook ekle',() => edit()),b('E-posta ekle',() => edit({},'smtp')),help('Sunucu çevrimdışı/iyileşme, AutoSign ve harici yayım hata/iyileşme olayları gönderilir. Başarısız gönderimler kalıcı kuyrukta en fazla sekiz kez denenir.')),box('Bildirim kanalları',table(['Ad','Tür','Durum','İşlemler'],data.channels.map(c => [c.name,c.target.kind,t(c.enabled ? 'Etkin' : 'Devre dışı'),actions(b('Düzenle',() => edit(c)),c.enabled && b('Sına',async () => {if(await confirm('Hedefe sınama bildirimi gönderilsin mi?')){await api(`/notifications/${c.id}/test`,{confirmed:true});await render();}}))]))),box('Son gönderimler',table(['Tarih','Durum','Deneme','Olay'],data.deliveries.map(d => [d.created_at,t(d.status),d.attempts,n('div',{},d.event.resource,n('small',{},t(d.event.state)),n('small',{},t(d.message)))]))));
}
async function account() {
	page('Hesabım',box('Hesabım',n('p',{},user.email),n('p',{},`${user.tenant_name} · ${user.role}`)),form('Parola değiştir',[field('Mevcut parola','current_password','password'),field('Yeni parola','new_password','password')],flat => api('/password',flat),{confirm:'Parolanızı değiştirmek istiyor musunuz?',done:() => go('/signin')}));
	if(admin()) main.append(form('Çalışma alanı',[field('Ad','name','text',user.tenant_name)],flat => api('/tenant',flat),{done:render}));
}
async function audit() {const data=await api('/panel/audit');page('Denetim kaydı',box('Denetim kaydı',table(['Tarih','Kullanıcı','İşlem','Kaynak'],data.audit.map(a => [a.time,a.email,t(a.action),a.resource]))));}
async function reports() {
	const params=new URLSearchParams(location.search), data=await api('/reports?' + params);
	page('Raporlar',form('Filtre',[field('Sunucu','server','select',params.get('server') || '',[['','Tümü'],...data.servers.map(s => Array.isArray(s) ? [s[0],s[1]] : [s.id,s.name])]),field('Saat','hours','select',data.hours,[1,24,48,72,168,744].map(v => [v,String(v)]))],flat => go('/reports?' + new URLSearchParams(flat)),{label:'Göster'}),b('CSV indir',() => download('/usage.csv','hoodmin-usage.csv')));
	for(const [title,key,scale] of [['CPU','cpuUsage',1],['Bellek','availableMemory',1/1073741824],['Oturum','sessionCount',1],['Aktarım hızı','tunnelSpeed',8/1e6]]) {
		const points=data.samples.map((s,i) => {const raw=key === 'tunnelSpeed' ? (typeof s.status[key]?.sent==='number'&&typeof s.status[key]?.received==='number' ? s.status[key].sent+s.status[key].received : null) : s.status[key];return [i,typeof raw==='number' ? raw*scale : NaN];}).filter(([,v]) => Number.isFinite(v) && v>=0 && v<=1e15);
		const peak=Math.max(0,...points.map(([,v]) => v));
		const svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.setAttribute('viewBox','0 0 600 160');svg.setAttribute('role','img');svg.setAttribute('aria-label',t(title));svg.classList.add('spa-chart');
		const line=document.createElementNS(svg.namespaceURI,'polyline');line.setAttribute('points',points.map(([i,v]) => `${10+580*i/Math.max(1,data.samples.length-1)},${140-125*v/Math.max(1,peak)}`).join(' '));svg.append(line);
		main.append(box(title,svg,n('small',{},`${t('En yüksek')}: ${peak.toFixed(2)}`)));
	}
	main.append(box('Ülkeler',table(['Ülke','Kullanılan cihazlar'],data.countries)),box('Kullanım',n('p',{},data.range)));
}
async function integrations() {const data=await api('/integrations');page('Entegrasyonlar',box('Ödüllü reklam doğrulaması',n('p',{},t(data.enabled ? 'Etkin' : 'Devre dışı')),help('Ödül doğrulama kimliğini yalnız güvenilir sunucuyla paylaşın. İstemci uygulamasına koymayın.'),b('Bağlantıyı yenile',async () => {if(await confirm('Mevcut doğrulama kimliği geçersiz olacak. Onaylıyor musunuz?')) secret((await api('/integrations',{confirmed:true})).value);},'danger')));}
function refreshLater(){retryTimer=setTimeout(() => {if(!document.querySelector('dialog')&&!main.dataset.dirty)render();else refreshLater();},30000);}
async function render() {
	const serial=++navigation;requests.abort();requests=new AbortController();clearTimeout(retryTimer);
	try {
		const current=location.pathname;
		if(current === '/signin') return await signin();
		const me=await api('/me');if(serial!==navigation)return;user=me;shell();
		const route=current.split('/').filter(Boolean);
		if(current==='/'||current==='/panel') await overview();
		else if(current==='/panel/servers') await servers();
		else if(current==='/panel/tokens') await tokenList();
		else if(current==='/panel/sessions') await sessions();
		else if(current==='/panel/users'&&admin()) await users();
		else if(current==='/panel/audit') await audit();
		else if(current==='/panel/account') await account();
		else if(current==='/manage/farms') await farms();
		else if(current==='/manage/profiles') await profiles();
		else if(current==='/manage/server-profiles') await profiles(true);
		else if(current==='/manage/bots'&&admin()) await bots();
		else if(current==='/manage/notifications'&&admin()) await notifications();
		else if(current==='/manage/integrations'&&admin()) await integrations();
		else if(route[0]==='manage'&&route[1]==='servers'&&route.length===4) await serverDetail(route[2],route[3]);
		else if(route[0]==='manage'&&route[1]==='farms'&&route.length===4) await farmDetail(route[2],route[3]);
		else if(current==='/reports') await reports();
		else page('Sayfa bulunamadı.',link('Genel bakış','/panel'));
		if(['/panel','/panel/servers','/panel/sessions','/manage/notifications'].includes(current)) refreshLater();
	} catch(error) {
		if(serial!==navigation)return;
		if(error.status===401 && location.pathname!=='/signin') return go('/signin');
		if(!main?.isConnected){main=n('main',{class:'spa-loading'});root.replaceChildren(main);}
		main.replaceChildren(box('İşlem tamamlanamadı.',n('p',{role:'alert'},t(error.status===503||!error.status ? 'Hizmet geçici olarak kullanılamıyor. Lütfen daha sonra yeniden deneyin.' : error.message)),b('Yeniden dene',render)));
		if(error.status===503||!error.status)retryTimer=setTimeout(render,10000);
	}
}


function configEditor(value,prefix='') {
	const container=n('div',{class:'spa-editor'});
	for(const [className,section] of [['ServerConfig',''],['SessionOptions','Session'],['TrackingOptions','Tracking'],['NetFilterOptions','NetFilter']]) {
		const group=n('details',{open:true},n('summary',{},section || t('Genel ayarlar'))), fields=n('div',{class:'spa-editor'});
		for(const f of schemaCache.catalog.classes[className].fields) {
			if(['serverSecret','configCode','certificates','Session','Tracking','NetFilter'].includes(f.name)) continue;
			const path=[prefix,section,f.name].filter(Boolean).join('.'), v=(section ? value[section]?.[f.name] : value[f.name]) ?? null;
			const kind=f.type.replace(/\?$/,'');let item;
			if(kind === 'bool') item=field(f.name,path,'select-json',JSON.stringify(v),[['null','Varsayılan'],['true','Evet'],['false','Hayır']]);
			else if(['int','long'].includes(kind)) item=field(f.name,path,'optional-number',v);
			else if(['string','TimeSpan'].includes(kind)) item=field(f.name,path,'nullable-text',v);
			else item=field(f.name,path,'json',v);
			item.append(n('small',{class:'muted'},f.type));fields.append(item);
		}
		group.append(fields);container.append(group);
	}
	container.append(help('Session oturum sınırlarını, NetFilter ağ erişim kurallarını, Tracking trafik izleme alanlarını belirler. null varsayılanı kullanır. Süreler gün.saat:dakika:saniye; tamponlar send/receive bayt değerleridir. Dinleme uçları IP:port biçimindedir. Bu ayarlar VPN sunucusunda uygulanır; istemcilere duyurulan genel adres Sunucular ekranından değiştirilir.'));
	return container;
}

function icon(name) {
	const paths={account:'M20 21a8 8 0 0 0-16 0 M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8',servers:'M3 3h18v7H3z M3 14h18v7H3z M6 6h.01 M6 17h.01 M10 6h8 M10 17h8',tokens:'M15 3a6 6 0 1 1-4.8 9.6L3 20v1h4v-3h3v-3l1.4-1.4 M16 7h.01',sessions:'M2 12h4l3-8 6 16 3-8h4',audit:'M12 3 3 7v6c0 4 5 7 9 9 4-2 9-5 9-9V7z M8 12l3 3 5-6',notifications:'M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9 M9 21h6',reports:'M4 20V10 M12 20V4 M20 20v-7',users:'M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8 M20 8v6 M17 11h6'};
	const svg=document.createElementNS('http://www.w3.org/2000/svg','svg');for(const[k,v]of Object.entries({class:'ui-icon',viewBox:'0 0 24 24',fill:'none',stroke:'currentColor','stroke-width':'1.7','stroke-linecap':'round','stroke-linejoin':'round','aria-hidden':'true'}))svg.setAttribute(k,v);
	const path=document.createElementNS(svg.namespaceURI,'path');path.setAttribute('d',paths[name]||'M3 11 12 3l9 8 M5 10v11h14V10 M9 21v-7h6v7');svg.append(path);return svg;
}

appearance();try { await changeLanguage(lang); } catch { dictionary = {}; } await render();
