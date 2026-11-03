/* Local static UI with simulated 503 API responses; no account or production data. */
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile,mkdir} from 'node:fs/promises';
import {spawn} from 'node:child_process';
const base=new URL('../crates/manager/web/',import.meta.url),debug=33483;
const phases=['Kurulumun tamamlanması bekleniyor.','Veritabanı bağlantısı bekleniyor.','Yükseltme için yönetici yapılandırması bekleniyor.','Yükseltme tamamlanamadı. Lütfen yöneticiye başvurun.','Yükseltme tamamlanıyor. Lütfen bekleyin.'];
let phase=phases[0],browser,ws;const pending=new Map();let seq=0;const errors=[];
const server=createServer(async(req,res)=>{
	if(req.url.startsWith('/api/')){res.writeHead(503,{'Content-Type':'application/json','Retry-After':'5'});res.end(JSON.stringify({code:'upgrade_pending',error:phase}));return;}
	const file=req.url.startsWith('/assets/')?req.url.slice(1):'index.html';
	if(file.includes('..')){res.writeHead(404);res.end();return;}
	try {const data=await readFile(new URL(file,base));res.writeHead(200,{'Content-Type':file.endsWith('.js')?'text/javascript':file.endsWith('.json')?'application/json':file.endsWith('.css')?'text/css':'text/html'});res.end(data);}catch{res.writeHead(404);res.end();}
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const origin='http://127.0.0.1:'+server.address().port;
function c(method,params={}){const id=++seq;return new Promise((resolve,reject)=>{pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});}
async function evaluate(expression){const r=await c('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.text);return r.result.value;}
async function wait(expression){for(let i=0;i<80;i++){if(await evaluate(expression))return;await new Promise(r=>setTimeout(r,100));}throw Error('Timeout: '+expression);}
try {
	await mkdir('work/upgrade-qa/browser',{recursive:true});
	browser=spawn('chromium',['--headless','--disable-gpu','--no-first-run','--remote-debugging-port='+debug,'--user-data-dir='+process.cwd()+'/work/upgrade-qa/browser','about:blank'],{stdio:'ignore'});
	let tabs;for(let i=0;i<80;i++){try{tabs=await(await fetch(`http://127.0.0.1:${debug}/json`)).json();break;}catch{await new Promise(r=>setTimeout(r,100));}}
	assert(tabs,'Chromium DevTools must be available');
	ws=new WebSocket(tabs.find(t=>t.type==='page').webSocketDebuggerUrl);await new Promise(r=>ws.addEventListener('open',r,{once:true}));
	ws.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);pending.delete(m.id);if(m.error)p.reject(m.error);else p.resolve(m.result);}else if(m.method==='Runtime.exceptionThrown')errors.push(m.params.exceptionDetails.text);});
	await c('Runtime.enable');await c('Page.enable');await c('Page.navigate',{url:origin});await wait('!!document.querySelector("main [role=alert]")');
	for(const locale of ['tr','en','ar']) {
		const dictionary=JSON.parse(await readFile(new URL('assets/locales/'+locale+'.json',base)));
		for(const message of phases) {
			phase=message;await evaluate(`localStorage.setItem('hoodmin.language',${JSON.stringify(locale)})`);await c('Page.reload');
			await wait(`document.documentElement.lang===${JSON.stringify(locale)} && document.querySelector('main [role=alert]')?.textContent===${JSON.stringify(dictionary[message])}`);
			assert.equal(await evaluate('document.documentElement.dir'),locale==='ar'?'rtl':'ltr');
			assert.equal(await evaluate('document.querySelectorAll("main .spa-card").length'),1);
		}
		console.log('PASS: Five upgrade states, one simple error card and correct direction: '+locale);
	}
	assert.deepEqual(errors,[]);console.log('PASS: Upgrade UI has no uncaught browser exceptions');
} finally {
	if(ws)ws.close();if(browser){browser.kill('SIGTERM');await new Promise(r=>browser.once('exit',r));}server.close();
}
