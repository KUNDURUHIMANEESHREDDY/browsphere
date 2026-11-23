// End-to-end Windows WebView2 + local Qwen recovery test.
// Primary click fails deliberately; Lucia must re-observe and click the fallback.
// Run only against an isolated Browsphere profile and loopback fixture.
'use strict';
const assert=require('node:assert/strict');
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const url='http://127.0.0.1:18764/recovery.html';
async function main(){
  const targets=await(await fetch('http://127.0.0.1:9246/json')).json();
  const main=targets.find(t=>t.type==='page'&&t.url.includes('tauri.localhost'));
  assert.ok(main,'Isolated Browsphere main WebView2 target not found');
  const ws=new WebSocket(main.webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
  let serial=0;const pending=new Map();
  ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id&&pending.has(m.id)){const x=pending.get(m.id);pending.delete(m.id);m.error?x.reject(Error(JSON.stringify(m.error))):x.resolve(m.result||{});}};
  async function cmd(method,params={}){
    const id=++serial;
    return Promise.race([new Promise((resolve,reject)=>{pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));}),
      sleep(16000).then(()=>{if(pending.has(id)){pending.delete(id);throw Error('CDP timeout '+method);}})]);
  }
  async function ev(expression){
    const r=await cmd('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});
    if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails).slice(0,500));
    return r.result?.value;
  }
  try{
    await cmd('Runtime.enable');
    await ev(`(async()=>{
      window.__recoveryUnlisten?.forEach(fn=>fn());
      window.__recoveryEvents=[];
      window.__recoveryUnlisten=[];
      for(const name of ['agent:plan','agent:stage','agent:trace','agent:approval_required','agent:run_status','agent:run_error','agent:verification_report']){
        window.__recoveryUnlisten.push(await window.__TAURI__.event.listen(name,event=>
          window.__recoveryEvents.push({event:name,payload:event.payload})));
      }
      const config=await window.__TAURI__.core.invoke('get_router_config');
      config.ollama_model='qwen2.5:3b-instruct';config.prefer_local_for_simple=true;
      await window.__TAURI__.core.invoke('update_router_config',{config});
      await window.__TAURI__.core.invoke('open_or_navigate_tab',{
        tabId:'lucia-failure-recovery',url:${JSON.stringify(url)},bounds:null,isPrivate:true
      });
      return 'configured';
    })()`);
    await sleep(1700);
    const initial=await ev("(async()=>await window.__TAURI__.core.invoke('capture_live_snapshot'))()");
    assert.equal(initial.origin,url,'Expected safe local recovery fixture');
    assert.ok(initial.interactive_nodes.some(n=>n.label==='Start recovery test'));
    console.log('INITIAL',JSON.stringify({origin:initial.origin,nodes:initial.interactive_nodes.map(n=>n.label)}));
    const runId=await ev(`window.__TAURI__.core.invoke('start_agent_run',{
      goal:'On the current local recovery test page, click Start recovery test. It is intentionally broken. After its click fails, re-observe and click Use recovery fallback to reveal the recovery complete confirmation. Do not navigate away.',
      snapshot:${JSON.stringify(initial)},allowedDomains:['127.0.0.1'],maxSteps:16,
      allowLearning:false,isPrivate:true,voiceMode:false
    })`);
    assert.ok(runId,'Agent did not start');
    console.log('RUN',runId);
    let all=[],ended;
    for(let i=0;i<85;i++){
      await sleep(1500);
      all=await ev('window.__recoveryEvents.slice()')||[];
      const approvals=all.filter(x=>x.event==='agent:approval_required'&&x.payload?.run_id===runId);
      for(const item of approvals){
        console.log('UNEXPECTED_APPROVAL',item.payload?.reason);
        try{await ev(`window.__TAURI__.core.invoke('resolve_approval',{approvalId:${JSON.stringify(item.payload.approval_id)},approved:false})`);}catch{}
      }
      ended=all.find(x=>x.event==='agent:run_status'&&x.payload?.run_id===runId);
      if(ended)break;
    }
    if(!ended){
      await ev("window.__TAURI__.core.invoke('cancel_agent_run')");
      throw Error('Agent exceeded recovery test deadline');
    }
    const relevant=all.filter(x=>x.payload?.run_id===runId);
    const traces=relevant.filter(x=>x.event==='agent:trace');
    const end=await ev("(async()=>await window.__TAURI__.core.invoke('capture_live_snapshot'))()");
    const verified=relevant.find(x=>x.event==='agent:verification_report');
    console.log('FINAL',JSON.stringify(ended.payload));
    console.log('TRACES',JSON.stringify(traces.map(t=>({action:t.payload?.action?.type,outcome:t.payload?.outcome,route:t.payload?.model_route}))));
    console.log('STAGES',JSON.stringify(relevant.filter(x=>x.event==='agent:stage').map(x=>({specialist:x.payload?.specialist,status:x.payload?.status}))));
    console.log('PAGE',JSON.stringify({origin:end?.origin,success:end?.text_excerpt?.includes('Recovery complete: fallback button succeeded')}));
    const failure=traces.findIndex(t=>t.payload?.action?.type==='click'&&t.payload?.outcome==='failed');
    const nextSuccess=traces.findIndex((t,i)=>i>failure&&t.payload?.action?.type==='click'&&['ok','confirmed'].includes(t.payload?.outcome));
    assert.ok(failure>=0,'No genuine primary click failure was observed');
    assert.ok(nextSuccess>failure,'Lucia did not recover with a successful fallback click');
    assert.equal(traces.filter(t=>t.payload?.action?.type==='click'&&['ok','confirmed'].includes(t.payload?.outcome)).length,1,
      'Disabled fallback must not generate a second successful click acknowledgement');
    assert.ok(!end.interactive_nodes.some(n=>n.label==='Use recovery fallback'),
      'A disabled fallback control must not remain actionable');
    assert.ok(end?.text_excerpt?.includes('Recovery complete: fallback button succeeded'),'Fallback page state not reached');
    assert.equal(ended.payload?.status,'completed','Recovery run did not finish');
    assert.equal(verified?.payload?.report?.status,'evidence_checked','No source-backed verification report');
    console.log('PASS real LLM failure → re-observe → successful fallback → grounded verification');
  }finally{ws.close();}
}
main().catch(err=>{console.error('FAIL',err.stack||err);process.exitCode=1;});
