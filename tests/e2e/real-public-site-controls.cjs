'use strict';
// Independent real-site DOM and navigation checks in native Browsphere WebView2.
// This does not use AI inference: report separately from Lucia autonomy.
const {pageReadiness}=require('./lucia-real-site-evidence.cjs');
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const port=Number(process.env.LUCIA_CDP_PORT || 9563);
const base='http://127.0.0.1:'+port;
const sites=[
  {name:'Python Docs',url:'https://docs.python.org/3/'},
  {name:'MDN Web Docs',url:'https://developer.mozilla.org/en-US/'},
  {name:'Wiktionary',url:'https://en.wiktionary.org/wiki/Wiktionary:Main_Page'},
  {name:'GitHub Docs',url:'https://docs.github.com/en'},
  {name:'Hacker News',url:'https://news.ycombinator.com/'}
];
class CDP {
  constructor(url){this.url=url;this.seq=0;this.pending=new Map()}
  async open(){
    this.ws=new WebSocket(this.url);
    await new Promise((ok,err)=>{this.ws.onopen=ok;this.ws.onerror=err});
    this.ws.onmessage=event=>{const m=JSON.parse(event.data);const p=this.pending.get(m.id);if(!p)return;
      this.pending.delete(m.id);m.error?p.err(Error(JSON.stringify(m.error))):p.ok(m.result||{});
    };
    await this.call('Runtime.enable');
  }
  async call(method,params={}){
    const id=++this.seq;
    const work=new Promise((ok,err)=>{this.pending.set(id,{ok,err});this.ws.send(JSON.stringify({id,method,params}))});
    try{return await Promise.race([work,sleep(12000).then(()=>{throw Error('CDP timeout: '+method)})])}
    finally{this.pending.delete(id)}
  }
  async eval(expression){
    const r=await this.call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});
    if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails).slice(0,250));
    return r.result?.value;
  }
  close(){this.ws?.close()}
}
async function targets(){const r=await fetch(base+'/json',{signal:AbortSignal.timeout(6000)});if(!r.ok)throw Error('/json HTTP '+r.status);return r.json()}
async function main(){
  const pages=await targets();
  const appPage=pages.find(p=>p.type==='page'&&p.url?.includes('tauri.localhost'));
  if(!appPage)throw Error('Native Browsphere UI missing');
  const main=new CDP(appPage.webSocketDebuggerUrl);
  await main.open();
  let child=null;
  const records=[];
  try{
    for(let i=0;i<80;i++){if(await main.eval('Boolean(window.browsphereApp?.navigate)').catch(()=>false))break;await sleep(300)}
    for(const site of sites){
      const host=new URL(site.url).hostname;
      let before=null,after=null,chosen=null,error=null;
      try{
        await main.eval('window.browsphereApp.navigate('+JSON.stringify(site.url)+')');
        let target;
        for(let n=0;n<40;n++){
          await sleep(450);
          target=(await targets()).find(p=>p.type==='page'&&p.url&&(()=>{try{return new URL(p.url).hostname===host}catch{return false}})());
          if(target)break;
        }
        if(!target)throw Error('No real destination WebView2 target appeared');
        child?.close();child=new CDP(target.webSocketDebuggerUrl);await child.open();
        const documentState='({url:location.href,title:document.title,ready:document.readyState,links:document.querySelectorAll("a[href]").length,inputs:document.querySelectorAll("input,button,select").length,body:document.body?.innerText?.length||0})';
        for(let n=0;n<45;n++){
          before=await child.eval(documentState);
          if(new URL(before.url).hostname===host&&before.title?.length>5&&before.body>100&&before.ready!=='loading')break;
          await sleep(350);
        }
        const readiness=pageReadiness({url:before.url,title:before.title,
          controls:(before.links||0)+(before.inputs||0),excerptLength:before.body},host);
        if(!readiness.ok)throw Error('SITE_UNAVAILABLE: '+readiness.reason);
        if(before.body<100)throw Error('Rendered document unavailable or empty');
        // Select an actual first-party visible link. Reject logins, downloads,
        // account operations, editor links, and any third-party destination.
        chosen=await child.eval(`(()=>{const u=location.href;
          const siteName=${JSON.stringify(site.name)};
          const excluded=/login|sign.?in|sign.?up|auth|account|register|pricing|donat|checkout|edit|upload|download|logout|subscribe|settings|mailto|javascript/i;
          const all=[...document.querySelectorAll('a[href]')];
          for(const a of all){
            if(a.download||a.target==='_blank')continue;
            if(!a.textContent?.trim()||excluded.test(a.textContent))continue;
            let url;try{url=new URL(a.getAttribute('href'),u)}catch{continue}
            if(url.hostname!==location.hostname||url.protocol!=='https:'||url.pathname===location.pathname||excluded.test(url.href))continue;
            // Use a real dictionary word, not Wiktionary's intermittently intercepted Search link.
            if(siteName==='Wiktionary' && (!url.pathname.startsWith('/wiki/')||decodeURIComponent(url.pathname).includes(':')))continue;
            // Avoid a homepage -> /news no-op on Hacker News; exercise the newest listing.
            if(siteName==='Hacker News' && url.pathname!=='/newest')continue;
            const box=a.getBoundingClientRect();const st=getComputedStyle(a);
            if(!box.width||!box.height||st.visibility==='hidden'||st.display==='none')continue;
            const label=a.textContent.trim().replace(/\\s+/g,' ').slice(0,65);
            a.click();return {url:url.href,label};
          }
          return null;
        })()`);
        if(!chosen)throw Error('No suitable public in-site link available');
        for(let n=0;n<40;n++){
          await sleep(500);
          after=await child.eval(documentState);
          if(after.url===chosen.url&&after.title?.length>5&&after.ready!=='loading'&&after.body>100)break;
        }
      }catch(e){error=String(e)}
      const navigated=!error&&after?.url===chosen?.url&&after?.body>100&&after?.title?.length>5;
      const status=navigated?'passed':String(error||'').includes('SITE_UNAVAILABLE:')?'site_unavailable':'failed';
      const record={site:site.name,passed:!!navigated,status,before,chosen,after,error};
      console.log('REAL_BROWSER_CONTROL',JSON.stringify(record));
      records.push({site:record.site,passed:record.passed,status,clicked:chosen?.label||null,error});
    }
  }finally{child?.close();main.close()}
  console.log('REAL_BROWSER_CONTROLS_SUMMARY',JSON.stringify({passed:records.filter(r=>r.passed).length,
    siteUnavailable:records.filter(r=>r.status==='site_unavailable').length,
    total:records.length,records}));
  if(records.some(r=>!r.passed))process.exitCode=1;
}
main().catch(e=>{console.error('REAL_BROWSER_CONTROLS_FATAL',String(e.stack||e));process.exitCode=2});
