'use strict';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function main(){
 const response=await fetch('http://127.0.0.1:9272/json');
 const pages=(await response.json()).filter(p=>p.type==='page');
 const page=pages.find(p=>p.url?.includes('tauri.localhost')&&p.title?.includes('Browsphere'))
    ||pages.find(p=>p.url?.includes('tauri.localhost'));
 if(!page)throw Error('Native Browsphere chrome not found '+JSON.stringify(pages.map(p=>p.url)));
 const ws=new WebSocket(page.webSocketDebuggerUrl);
 await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
 let serial=0;const pending=new Map();
 ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id&&pending.has(m.id)){const p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result)}};
 async function cmd(method,params={}){
  const id=++serial;
  const promise=new Promise((resolve,reject)=>{pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});
  return Promise.race([promise,sleep(140000).then(()=>{if(pending.has(id)){pending.delete(id);throw Error(method+' timeout')}})]);
 }
 async function ev(expression){
  const r=await cmd('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});
  if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails).slice(0,1000));
  return r.result?.value;
 }
 try {
 await cmd('Runtime.enable');
 const conf=await ev("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');return {model:c.litellm_model,host:new URL(c.litellm_url).hostname,cloud:!c.prefer_local_for_simple}})()");
 console.log('CONFIG',JSON.stringify(conf));
 await ev("window.__TAURI__.core.invoke('open_or_navigate_tab',{tabId:'lucia-direct-navigation-e2e',url:'http://127.0.0.1:18764/',isPrivate:true})");
 await sleep(1000);
 const snapshot=await ev("window.__TAURI__.core.invoke('capture_live_snapshot')");
 if(!snapshot?.origin?.startsWith('http://127.0.0.1:18764/'))throw Error('Unexpected fixture origin '+snapshot?.origin);
 console.log('FIXTURE',snapshot.origin);
 await ev("(async()=>{window.__navTestEvents=[];window.__navTestUnlisten=[];for(const name of ['agent:plan','agent:stage','agent:trace','agent:approval_required','agent:verification_report','agent:run_error','agent:run_status']){window.__navTestUnlisten.push(await window.__TAURI__.event.listen(name,e=>{const p=e.payload||{};window.__navTestEvents.push({name,run_id:p.run_id,status:p.status,reason:p.reason,message:p.message,report:p.report,evidence_count:p.evidence_count,index:p.index})}))}return true})()");
 const args={goal:'open youtube',snapshot,allowedDomains:[],maxSteps:12,allowLearning:false,isPrivate:true,voiceMode:false};
 const run=await ev("window.__TAURI__.core.invoke('start_agent_run',"+JSON.stringify(args)+")");
 console.log('RUN_ID',run);
 let completed=null,events=[];
 for(let i=0;i<45;i++){
  await sleep(2000);
  events=await ev('window.__navTestEvents.slice(-80)');
  if(events.some(e=>e.name==='agent:approval_required'&&e.run_id===run)){
    console.log('UNEXPECTED_APPROVAL',JSON.stringify(events.filter(e=>e.name==='agent:approval_required')));
    await ev("window.__TAURI__.core.invoke('cancel_agent_run')");
    break;
  }
  completed=events.find(e=>e.name==='agent:run_status'&&e.run_id===run);
  if(completed)break;
 }
 if(!completed){
  console.log('NO_TERMINAL_STATUS');
  await ev("window.__TAURI__.core.invoke('cancel_agent_run')");
 }
 const after=await ev("window.__TAURI__.core.invoke('capture_live_snapshot')");
 console.log('RESULT',JSON.stringify({status:completed?.status,url:after?.origin,title:after?.title,visibleTextLength:after?.text_excerpt?.length,approvedCount:events.filter(e=>e.name==='agent:approval_required').length,
  verified:events.some(e=>e.name==='agent:verification_report'&&e.run_id===run),stages:events.filter(e=>e.name==='agent:stage').map(e=>({status:e.status,index:e.index,evidence_count:e.evidence_count})),errors:events.filter(e=>e.name==='agent:run_error').map(e=>e.message)}));
 if(completed?.status!=='completed')process.exitCode=2;
 }finally{ws.close();}
}
main().catch(e=>{console.error('FAIL',e.stack||e);process.exitCode=1;});