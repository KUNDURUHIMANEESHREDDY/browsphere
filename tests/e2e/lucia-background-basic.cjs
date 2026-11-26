'use strict';
// Native isolated WebView2 background-tab acceptance: destination, verification,
// and foreground focus must all agree. No accounts or external submissions.
const assert=require('node:assert/strict');
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
(async()=>{
 const port=process.env.LUCIA_CDP_PORT||'9273';
 const pages=await(await fetch('http://127.0.0.1:'+port+'/json')).json();
 const tab=pages.find(p=>p.type==='page'&&p.url?.includes('tauri.localhost'));
 assert.ok(tab,'Native app not found');
 const ws=new WebSocket(tab.webSocketDebuggerUrl);
 await new Promise((ok,err)=>{ws.onopen=ok;ws.onerror=err;});
 let seq=0;const pending=new Map();
 ws.onmessage=e=>{let x=JSON.parse(e.data),p=pending.get(x.id);if(!p)return;
  pending.delete(x.id);x.error?p.reject(Error(JSON.stringify(x.error))):p.resolve(x.result||{});};
 const call=(method,params={})=>new Promise((resolve,reject)=>{
  const id=++seq;const timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP timeout'))},15000);
  pending.set(id,{resolve:v=>{clearTimeout(timer);resolve(v)},reject:e=>{clearTimeout(timer);reject(e)}});
  ws.send(JSON.stringify({id,method,params}));
 });
 const evaluate=async expression=>{
  const r=await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true,userGesture:true});
  if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));
  return r.result?.value;
 };
 try{
  await call('Runtime.enable');
  for(let i=0;i<70;i++){
   if(await evaluate('Boolean(window.aetherBridge?.startLuciaSpecialistTask&&window.browsphereApp?.createTaskTab)').catch(()=>false))break;
   if(i===69)throw Error('Native UI not ready');
   await sleep(350);
  }
  await evaluate("(async()=>{window.__bgEvents=[];for(const n of ['agent:run_status','agent:run_error','agent:verification_report','agent:approval_required'])await window.__TAURI__.event.listen(n,e=>window.__bgEvents.push({name:n,payload:e.payload}));return true})()");
  await evaluate("window.browsphereApp.newTab('browsphere://start')");
  const before=await evaluate('window.browsphereApp.activeTab()');
  const taskId=await evaluate("window.browsphereApp.createTaskTab('about:blank')");
  const runId=await evaluate("window.aetherBridge.startLuciaSpecialistTask('open wikipedia.org','about:blank','Background site task',"+JSON.stringify(taskId)+")");
  const foreground=await evaluate('window.browsphereApp.activeTab()');
  assert.equal(foreground.id,before.id,'Background task stole foreground focus');
  let status=null,verified=false;
  for(let i=0;i<100;i++){
   await sleep(500);
   const events=await evaluate('window.__bgEvents.slice(-200)');
   if(events.some(e=>e.name==='agent:approval_required'&&e.payload?.run_id===runId))
    throw Error('Unexpected approval requested');
   verified=events.some(e=>e.name==='agent:verification_report'&&e.payload?.run_id===runId);
   const end=events.find(e=>e.name==='agent:run_status'&&e.payload?.run_id===runId);
   if(end){status=end.payload.status;break}
  }
  const final=await evaluate('window.browsphereApp.activeTab()');
  const page=await evaluate("(async()=>{let s=await window.__TAURI__.core.invoke('capture_tab_snapshot',{tabId:"+JSON.stringify(taskId)+"});return {origin:s.origin,title:s.title}})()");
  console.log('BACKGROUND_RESULT',JSON.stringify({status,verified,foregroundUnchanged:before.id===final.id,taskPage:page.origin,title:page.title}));
  assert.equal(status,'completed');
  assert.ok(verified,'Independent verifier did not report');
  assert.equal(before.id,final.id);
  assert.match(page.origin,/^https:\/\/(www\.)?wikipedia\.org\//);
  console.log('BACKGROUND_BASIC_PASSED');
 }finally{ws.close()}
})().catch(e=>{console.error('BACKGROUND_BASIC_FAILED',e.stack||String(e));process.exitCode=1});
