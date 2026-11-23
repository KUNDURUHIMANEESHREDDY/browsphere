import fs from 'node:fs';
import path from 'node:path';
const ROOT = 'C:\\Users\\himan\\Downloads\\Browsphere';
const ART = path.join(ROOT, 'tests', 'e2e', 'artifacts');
fs.mkdirSync(ART,{recursive:true});
const tests=[], errors=[], calls=new Map();
let counter=0, ws=null;
const pause=(ms)=>new Promise(r=>setTimeout(r,ms));
function record(name,passed,detail='') {
 tests.push({name,passed,detail:String(detail).slice(0,260)});
 console.log((passed?'PASS':'FAIL')+' '+name+' '+String(detail).slice(0,160));
}
async function cmd(method,params={}) {
 const id=++counter;
 return await new Promise((res,rej)=>{
  calls.set(id,{res,rej});ws.send(JSON.stringify({id,method,params}));
  setTimeout(()=>{if(calls.has(id)){calls.delete(id);rej(new Error('CDP '+method+' timeout'));}},18000);
 });
}
async function ev(fn,...args) {
 const expression='('+fn.toString()+')('+args.map(x=>JSON.stringify(x)).join(',')+')';
 const r=await cmd('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});
 if(r.exceptionDetails)throw new Error(JSON.stringify(r.exceptionDetails).slice(0,500));
 return r.result?.value;
}
async function shot(name) {
 const r=await cmd('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
 if(r.data)fs.writeFileSync(path.join(ART,name+'.png'),Buffer.from(r.data,'base64'));
}
async function main(){
 const endpoints=await(await fetch('http://127.0.0.1:9228/json')).json();
 const target=endpoints.find(x=>x.title?.includes('Browsphere'))||endpoints.find(x=>x.type==='page');
 if(!target)throw Error('Browsphere CDP target missing');
 ws=new WebSocket(target.webSocketDebuggerUrl);
 await new Promise((res,rej)=>{ws.onopen=res;ws.onerror=rej;});
 ws.onmessage=e=>{
  const m=JSON.parse(e.data);
  if(m.id&&calls.has(m.id)){
    const c=calls.get(m.id);calls.delete(m.id);
    m.error?c.rej(new Error(JSON.stringify(m.error))):c.res(m.result||{});
  } else if(m.method==='Runtime.exceptionThrown'){
    errors.push(m.params?.exceptionDetails?.text||'Runtime.exceptionThrown');
  } else if(m.method==='Log.entryAdded'&&m.params?.entry?.level==='error'){
    errors.push(m.params.entry.text);
  }
 };
 await cmd('Runtime.enable');await cmd('Page.enable');await cmd('Log.enable');
 await cmd('Page.reload',{ignoreCache:true});
 await pause(1200);
 const info=await ev(()=>({title:document.title,native:!!window.aetherBridge?.isNative,
   tauri:!!window.__TAURI__,news:!!document.getElementById('newTabNewsSection'),
   noMemoryButton:!document.getElementById('browsphereMemoryButton'),
   canvas:!!document.getElementById('readingCanvas')}));
 record('Start page renders',info.news&&info.canvas&&info.title.includes('Browsphere'),JSON.stringify(info));
 record('Native Tauri bridge ready',info.tauri&&info.native);
 record('Memory toolbar button removed',info.noMemoryButton);
 await shot('01-start');
 const mem=await ev(async()=>{
   try{let v=await window.aetherBridge.listMemories();return {ok:Array.isArray(v),count:v?.length};}
   catch(e){return {ok:false,error:String(e)};}
 });
 record('Native memory list command',mem.ok,mem.ok?mem.count+' items':mem.error);
 const history=await ev(async()=>{
   try{return Array.isArray(await window.aetherBridge.getHistory(1));}
   catch(e){return String(e);}
 });
 record('Native history read command',history===true,String(history));
 const settings=await ev(()=>{
   document.querySelector('button[data-action="open-settings"][data-pane="general"]')?.click();
   document.getElementById('settingsPaneBtn-privacy')?.click();
   return {list:!!document.getElementById('privacyMemoryList'),toggle:!!document.querySelector('[data-setting-key="autoMemoryEnabled"]')};
 });
 record('Settings Privacy UI renders',settings.list&&settings.toggle,JSON.stringify(settings));
 await pause(1300);
 const list=await ev(()=>{
   let e=document.getElementById('privacyMemoryList');
   return {exists:!!e,loading:e?.textContent.includes('Loading saved memories'),length:e?.textContent.length};
 });
 record('Memory list resolves from SQLite',list.exists&&!list.loading,JSON.stringify(list));
 const fixture=await ev(async()=>{
   const rows=await window.aetherBridge.listMemories();
   const id='browsphere-e2e-memory-fixture';
   const present=rows.some(x=>x.id===id);
   const button=document.querySelector('[data-action="delete-learned-memory"][data-memory-id="'+id+'"]');
   if(button)button.click();
   return {present,button:!!button};
 });
 record('Temporary memory appears in Privacy UI',fixture.present&&fixture.button,JSON.stringify(fixture));
 await pause(900);
 const afterDelete=await ev(async()=>{
   const rows=await window.aetherBridge.listMemories();
   return !rows.some(x=>x.id==='browsphere-e2e-memory-fixture');
 });
 record('Delete memory through Privacy UI',afterDelete);
 await shot('02-privacy');
 const toggle=await ev(()=>{
   const e=document.querySelector('[data-setting-key="autoMemoryEnabled"]');
   if(!e)return null;
   const originallyChecked=e.checked,previous=localStorage.getItem('browsphere-auto-memory');
   const current=()=>document.querySelector('[data-setting-key="autoMemoryEnabled"]');
   if(current().checked)current().click();
   const off=localStorage.getItem('browsphere-auto-memory');
   if(!current().checked)current().click();
   const on=localStorage.getItem('browsphere-auto-memory');
   // Restore the default user setting. The app initially had learning enabled.
   if(!current().checked)current().click();
   if(previous===null)localStorage.removeItem('browsphere-auto-memory');
   else localStorage.setItem('browsphere-auto-memory',previous);
   return {off,on,originallyChecked};
 });
 record('Memory opt-out can be toggled',toggle?.off==='off'&&toggle?.on==='on',JSON.stringify(toggle));
 const bridgeUsesStorage = await ev(()=>window.aetherBridge.runAgentGoal.toString().includes('isAutomaticMemoryAllowed()'));
 record('Agent launch uses memory setting',bridgeUsesStorage);
 await ev(()=>{
   let modal=document.getElementById('browsphereSettingsModal');
   modal?.querySelector('[data-action="close-settings"]')?.click();
 });
 const privateState=await ev(()=>{
   let e=document.getElementById('privateBrowsingToggleBtn');
   if(!e)return null;
   let initial=document.documentElement.getAttribute('data-private');
   if(initial!=='true')e.click();
   let during=document.documentElement.getAttribute('data-private');
   if(initial!=='true')e.click();
   return {initial,during,restored:document.documentElement.getAttribute('data-private')};
 });
 record('Private Browsing toggle',privateState?.during==='true'&&(privateState.initial==='true'?privateState.restored==='true':privateState.restored!=='true'),JSON.stringify(privateState));
 const scroll=await ev(()=>{
   let c=document.getElementById('readingCanvas');
   c.scrollTop=c.scrollHeight;
   c.dispatchEvent(new Event('scroll'));
   return c.scrollTop;
 });
 record('Scroll to news',scroll>80,'scrollTop='+scroll);
 await pause(14000);
 const news=await ev(()=>{
   let s=document.getElementById('newTabNewsSection');
   return {links:s?.querySelectorAll('button[data-action="navigate"][data-url^="https://"],button[data-action="navigate"][data-url^="http://"]').length||0,
    live:s?.innerText.includes('LIVE')||false,
    error:s?.innerText.toLowerCase().includes('could not load')||false,
    preview:s?.innerText.slice(0,220)||''};
 });
 record('Live headlines load after scrolling',news.links>0 && !news.error && news.preview.includes('Hacker News'),JSON.stringify(news));
 await shot('03-news');
 record('No uncaught JS exceptions',errors.length===0,errors.slice(0,3).join(' | '));
 const report={timestamp:new Date().toISOString(),target:{title:target.title,url:target.url},tests,errors,news};
 fs.writeFileSync(path.join(ART,'results.json'),JSON.stringify(report,null,2));
 console.log('SUMMARY: '+tests.filter(x=>x.passed).length+'/'+tests.length+' checks passed');
 ws.close();
 process.exitCode=tests.every(x=>x.passed)?0:1;
}
main().catch(e=>{console.error('E2E_FATAL',e.stack||String(e));if(ws)ws.close();process.exitCode=2;});
