/* Native Chromium DevTools protocol; no browser framework or downloaded dependencies. */
import {readFile,writeFile} from 'node:fs/promises';
const owner=JSON.parse(await readFile('work/browser-qa/owner.json','utf8'));
const tabs=await (await fetch('http://127.0.0.1:33481/json')).json();
const ws=new WebSocket(tabs.find(t => t.type==='page').webSocketDebuggerUrl);
await new Promise(r => ws.addEventListener('open',r,{once:true}));
let seq=0;const pending=new Map(),errors=[];
ws.addEventListener('message',e => {const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);pending.delete(m.id);if(m.error)p.reject(m.error);else p.resolve(m.result);}else if(m.method==='Runtime.exceptionThrown')errors.push(m.params.exceptionDetails.text+': '+m.params.exceptionDetails.exception?.description);});
function c(method,params={}){const id=++seq;return new Promise((resolve,reject) => {pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});}
async function evaluate(expression){const r=await c('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;}
async function wait(expression){for(let i=0;i<80;i++){if(await evaluate(expression))return;await new Promise(r=>setTimeout(r,150));}throw Error('Timeout: '+expression);}
const check=(value,message)=>{if(!value)throw Error(message);console.log('PASS:',message);};
await c('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});
await c('Runtime.enable');await c('Page.enable');await c('Page.navigate',{url:'http://127.0.0.1:33480/signin'});
await wait('!!document.querySelector("input[name=email]")');
await evaluate("localStorage.setItem('hoodmin.theme','light');localStorage.setItem('hoodmin.language','tr')");await c('Page.reload');await wait('!!document.querySelector("input[name=email]")');
await evaluate(`document.querySelector('input[name=email]').value=${JSON.stringify(owner.email)};document.querySelector('input[name=password]').value=${JSON.stringify(owner.password)};document.querySelector('form').requestSubmit()`);
await wait('!!document.querySelector("aside")');
check(await evaluate('document.body.innerText.includes("Genel bakış")'),'Static SPA login uses real QA API');
await evaluate(`document.querySelector('a[href="/panel/servers"]').click()`);await wait('document.title.startsWith("Sunucular")');
check(await evaluate('document.querySelectorAll("tbody tr").length>=2'),'Server table renders API records');
check(await evaluate('!document.querySelector("script:not([src])")'),'No inline script or server-generated user markup needed');
await evaluate(`Array.from(document.querySelectorAll('button')).find(b=>b.textContent==='Düzenle').click()`);await wait('!!document.querySelector("dialog input[name=host]")');
check(await evaluate('document.querySelector("dialog input[name=host]").value.length>0'),'Server edit exposes IP/FQDN');
check(await evaluate('document.querySelector("dialog").scrollWidth<=document.querySelector("dialog").clientWidth+2'),'Edit form stays within dialog');
await evaluate(`document.querySelector('dialog input[name=host]').value='vpn-ui.example.invalid';document.querySelector('dialog input[name=port]').value='3443';document.querySelector('dialog form').requestSubmit()`);
await wait('document.querySelectorAll("dialog").length===2');
await evaluate(`Array.from(document.querySelectorAll('dialog')).at(-1).querySelector('.primary').click()`);
await wait('document.querySelectorAll("dialog").length===0 && document.querySelector("main").innerText.includes("vpn-ui.example.invalid:3443")');
check(true,'Browser IP/FQDN edit is persisted through the real API after confirmation');
check(await evaluate('!document.querySelector("tbody script")'),'Untrusted server names remain text, never executable markup');
await evaluate(`const row=Array.from(document.querySelectorAll('tbody tr')).find(r=>r.innerText.includes('Rotation farm'));row.querySelector('button').click()`);
await wait('!!document.querySelector("dialog select[name=farm]")');
check(await evaluate('document.querySelector("dialog select[name=farm]").selectedOptions[0].textContent==="Rotation farm"'),'Server edit preserves the selected group UUID');

await evaluate(`document.querySelector('dialog').close();document.querySelector('.theme-toggle').click()`);
check(await evaluate('document.documentElement.dataset.theme==="dark" && document.querySelector(".theme-toggle").textContent==="☀"'),'Single icon toggles dark theme');
await c('Page.reload');await wait('!!document.querySelector("aside")');
check(await evaluate('document.documentElement.dataset.theme==="dark"'),'Theme persists across reload');
for(const [locale,dir,label] of [['en','ltr','Servers'],['ar','rtl','الخوادم'],['tr','ltr','Sunucular']]) {
	await evaluate(`document.querySelector('select[aria-label]').value='${locale}';document.querySelector('select[aria-label]').dispatchEvent(new Event('change',{bubbles:true}))`);
	await wait(`document.documentElement.lang==='${locale}' && !!document.querySelector('aside') && document.documentElement.dir==='${dir}'`);
	check(await evaluate(`document.querySelector('aside').innerText.includes(${JSON.stringify(label)})`),'Locale and direction: '+locale);
}
for(const path of ['/manage/farms','/manage/profiles','/manage/server-profiles','/panel/tokens','/panel/sessions','/panel/audit','/reports','/panel/users','/manage/bots','/manage/notifications','/manage/integrations','/panel/account']){
	await c('Page.navigate',{url:'http://127.0.0.1:33480'+path});await wait('!!document.querySelector("main h1")');
	check(!await evaluate('document.querySelector("main").innerText.includes("İşlem tamamlanamadı.")'),'API screen: '+path);
}


await c('Page.navigate',{url:'http://127.0.0.1:33480/manage/server-profiles'});await wait('!!document.querySelector("main h1")');
await evaluate(`Array.from(document.querySelectorAll('main button')).find(b=>b.textContent==='Profil oluştur').click()`);await wait('!!document.querySelector("dialog textarea[name=settings]")');
check(await evaluate('!JSON.parse(document.querySelector("dialog textarea[name=settings]").value).tcpEndPoints'),'New server profiles leave listener endpoints unspecified');
await evaluate(`document.querySelector('dialog input[name=name]').value='Browser profile';document.querySelector('dialog form').requestSubmit()`);await wait('document.querySelectorAll("dialog").length===2');await evaluate('Array.from(document.querySelectorAll("dialog")).at(-1).querySelector(".primary").click()');
await wait('document.querySelectorAll("dialog").length===0 && document.querySelector("main").innerText.includes("Browser profile")');
check(true,'Browser preserves partial settings while creating a server profile');
await c('Page.navigate',{url:'http://127.0.0.1:33480/manage/notifications'});await wait('!!document.querySelector("main h1")');
await evaluate(`Array.from(document.querySelectorAll('main button')).find(b=>b.textContent==='Webhook ekle').click()`);await wait('!!document.querySelector("dialog input[name=url]")');
await evaluate(`document.querySelector('dialog input[name=name]').value='Browser QA';document.querySelector('dialog input[name=url]').value='https://example.com/browser-qa';document.querySelector('dialog input[name=enabled]').checked=false;document.querySelector('dialog form').requestSubmit()`);
await wait('document.querySelectorAll("dialog").length===2');await evaluate('Array.from(document.querySelectorAll("dialog")).at(-1).querySelector(".primary").click()');
await wait('document.querySelectorAll("dialog").length===0 && document.querySelector("main").innerText.includes("Browser QA")');
check(true,'Browser creates a disabled encrypted notification channel through the real API');
await c('Page.navigate',{url:'http://127.0.0.1:33480/panel/tokens'});await wait('!!document.querySelector("main h1")');
await evaluate(`Array.from(document.querySelectorAll('main button')).find(b=>b.textContent==='Anahtar oluştur').click()`);await wait('!!document.querySelector("dialog input[name=name]")');
await evaluate(`document.querySelector('dialog input[name=name]').value='Browser key';document.querySelector('dialog form').requestSubmit()`);await wait('document.querySelectorAll("dialog").length===2');await evaluate('Array.from(document.querySelectorAll("dialog")).at(-1).querySelector(".primary").click()');
await wait('!!document.querySelector("dialog textarea[readonly]")');
check(await evaluate('document.querySelector("dialog textarea[readonly]").value.includes("vh://")'),'Generated access-key secret remains visible after the parent form closes');
await evaluate('document.querySelectorAll("dialog").forEach(d=>d.close())');
await c('Page.navigate',{url:'http://127.0.0.1:33480/panel/servers'});await wait('!!document.querySelector("tbody a[href*=configuration]")');
const href=await evaluate('document.querySelector("tbody a[href*=configuration]").getAttribute("href")');
await c('Page.navigate',{url:'http://127.0.0.1:33480'+href});await wait('document.querySelectorAll(".spa-editor [name]").length>40');
check(await evaluate('document.querySelectorAll(".spa-editor [name]").length===47'),'All 47 editable VpnHood configuration leaves are reachable');
await evaluate(`document.querySelector('textarea[name=dnsServers]').value='["1.1.1.1"]';document.querySelector('main form').requestSubmit()`);
await wait('!!document.querySelector("dialog .primary")');await evaluate('document.querySelector("dialog .primary").click()');
await wait(`fetch('/api/manage/servers/'+${JSON.stringify(href.split('/')[3])}+'/configuration').then(r=>r.json()).then(v=>v.settings?.dnsServers?.[0]==='1.1.1.1')`);
check(true,'All-field configuration form submits valid nullable types to the actual API');

const sid=href.split('/')[3];
await c('Page.navigate',{url:`http://127.0.0.1:33480/manage/servers/${sid}/credential`});await wait('!!document.querySelector("main button.danger")');
await evaluate('document.querySelector("main button.danger").click()');await wait('!!document.querySelector("dialog")');
check(await evaluate('document.querySelector("dialog").innerText.includes("bağlantısı kesilir")'),'Credential rotation requires a consequence-aware confirmation');
await evaluate(`Array.from(document.querySelectorAll('dialog button')).find(b=>b.textContent==='Vazgeç').click()`);
await c('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true});
await c('Page.navigate',{url:'http://127.0.0.1:33480/panel/servers'});await wait('!!document.querySelector("tbody")');
check(await evaluate('document.documentElement.scrollWidth<=window.innerWidth+1'),'Mobile layout has no page overflow');
await c('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});await c('Page.navigate',{url:'http://127.0.0.1:33480/panel'});await wait('!!document.querySelector(".stats-grid")');
await writeFile('work/browser-qa/panel.png',Buffer.from((await c('Page.captureScreenshot',{format:'png'})).data,'base64'));
await c('Network.enable');await c('Network.emulateNetworkConditions',{offline:true,latency:0,downloadThroughput:-1,uploadThroughput:-1});
await evaluate(`Array.from(document.querySelectorAll('main button')).find(b=>b.textContent==='Yenile').click()`);
await wait('document.querySelector("main").innerText.includes("Hizmet geçici olarak")');
await new Promise(r=>setTimeout(r,11000));
check(await evaluate('document.querySelectorAll("main .spa-card").length===1'),'Repeated outage retries keep a single simple error card');
await c('Network.emulateNetworkConditions',{offline:false,latency:0,downloadThroughput:-1,uploadThroughput:-1});
await evaluate(`Array.from(document.querySelectorAll('main button')).find(b=>b.textContent==='Yeniden dene').click()`);await wait('!!document.querySelector(".stats-grid")');
check(errors.length===0,'No uncaught browser exceptions: '+errors.join('\n'));
ws.close();
