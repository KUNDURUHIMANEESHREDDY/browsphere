// Real native Tauri + Ollama agent run against a harmless LOCAL fixture only.
// Prerequisites: isolated app with CDP port 9246, fixture HTTP server on 18764.
'use strict';
const assert=require('node:assert/strict');
const sleep=(ms)=>new Promise(resolve=>setTimeout(resolve,ms));
async function main(){
 const targets=await(await fetch('http://127.0.0.1:9246/json')).json();
 const main=targets.find(x=>x.title?.includes('Browsphere')&&x.url.includes('tauri.localhost'));
 assert.ok(main,'Native main window missing');
 const ws=new WebSocket(main.webSocketDebuggerUrl); await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});
 let serial=0; const calls=new Map();
 ws.onmessage=(e)=>{const m=JSON.parse(e.data); if(m.id&&calls.has(m.id)){const c=calls.get(m.id);calls.delete(m.id);m.error?c.reject(Error(JSON.stringify(m.error))):c.resolve(m.result||{})}};
 function cmd(method,params={}){
  const id=++serial;
  return Promise.race([
   new Promise((resolve,reject)=>{calls.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}))}),
   sleep(18000).then(()=>{if(calls.has(id)){calls.delete(id);throw Error('CDP timeout '+method)}})
  ]);
 }
 async function ev(expr){const r=await cmd('Runtime.evaluate',{expression:expr,returnByValue:true,awaitPromise:true,userGesture:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails).slice(0,650));return r.result?.value;}
 try {
  await cmd('Runtime.enable');
  const listeners=await ev(`(async()=>{
    window.__luciaGateUnlisten?.forEach(unlisten=>unlisten());
    window.__luciaGateEvents=[];
    window.__luciaGateUnlisten=[];
    for(const name of ['agent:plan','agent:stage','agent:trace','agent:approval_required','agent:approval_closed','agent:run_error','agent:verification_report','agent:run_status']){
      const unlisten=await window.__TAURI__.event.listen(name, ev => window.__luciaGateEvents.push({event:name,payload:ev.payload}));
      window.__luciaGateUnlisten.push(unlisten);
    }
    const cfg=await window.__TAURI__.core.invoke('get_router_config');
    cfg.ollama_model='qwen2.5:3b-instruct';
    cfg.prefer_local_for_simple=true;
    await window.__TAURI__.core.invoke('update_router_config',{config:cfg});
    return 'ready';
  })()`);
  assert.equal(listeners,'ready');
  const runId=await ev(`(async()=>{
    const snapshot=await window.__TAURI__.core.invoke('capture_live_snapshot');
    if(!snapshot?.origin?.startsWith('http://127.0.0.1:18764/'))throw Error('Unexpected active website');
    return await window.__TAURI__.core.invoke('start_agent_run',{
      goal:'On the current local test website, submit the sample application form.',
      snapshot,allowedDomains:['127.0.0.1'],maxSteps:14,allowLearning:false,isPrivate:true,voiceMode:false
    });
  })()`);
  assert.ok(runId,'Agent run did not start');
  console.log('RUN',runId);
  let events=[],ended=null,handled=new Set();
  for(let i=0;i<55;i++){
    await sleep(1600);
    events=(await ev('window.__luciaGateEvents.slice()'))||[];
    const freshApprovals=events.filter(x=>x.event==='agent:approval_required'&&x.payload?.run_id===runId&&!handled.has(x.payload.approval_id));
    for(const item of freshApprovals){
      if(handled.has(item.payload.approval_id))continue;
      handled.add(item.payload.approval_id);
      console.log('REJECT request',item.payload?.approval_id,item.payload?.reason);
      const outcome=await ev(`(async()=>{try{await window.__TAURI__.core.invoke('resolve_approval',{approvalId:${JSON.stringify(item.payload.approval_id)},approved:false});return 'rejected'}catch(e){return 'stale '+String(e)}})()`);
      console.log('APPROVAL_RESULT',outcome);
    }
    ended=events.find(x=>x.event==='agent:run_status'&&x.payload?.run_id===runId);
    if(ended)break;
  }
  if(!ended){
    await ev("window.__TAURI__.core.invoke('cancel_agent_run')");
    throw Error('Agent run timed out without final status');
  }
  const relevant=events.filter(x=>x.payload?.run_id===runId);
  console.log('FINAL',JSON.stringify(ended.payload));
  console.log('EVENTS',JSON.stringify(relevant.map(e=>({event:e.event,status:e.payload?.status,specialist:e.payload?.specialist,outcome:e.payload?.outcome,action:e.payload?.action?.type,message:e.payload?.message?.slice(0,170)}))));
  const snapshot=await ev("(async()=>await window.__TAURI__.core.invoke('capture_live_snapshot'))()");
  console.log('FINAL_PAGE',JSON.stringify({url:snapshot?.origin,detailsVisible:snapshot?.text_excerpt?.includes('The fixture details are visible')}));
  const verified=relevant.find(x=>x.event==='agent:verification_report');
  assert.ok(handled.size >= 1,'Submission approval must have been requested');
  assert.equal(ended.payload.status,'rejected','Rejected approval must stop the run');
  assert.ok(!snapshot?.text_excerpt?.includes('Application received. Thank you for submitting'),'Rejected application must not be submitted');
  assert.equal(verified,undefined,'Rejected task must not produce verified success');
  console.log('PASS native rejected approval stops unsafe form submission');
 }finally{ws.close()}
}
main().catch(e=>{console.error('FAIL',e.stack||e);process.exitCode=1});
