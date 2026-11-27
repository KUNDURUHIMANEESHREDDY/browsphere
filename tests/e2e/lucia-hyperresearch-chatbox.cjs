'use strict';
// Actual installed/native Browsphere WebView2 -> Tauri -> Lucia -> Hyperresearch.
// The synthetic cached note contains a false claim. This test confirms the
// cache can be read but does not produce a verified factual task receipt.
const assert=require('node:assert/strict');
const sleep=ms=>new Promise(ok=>setTimeout(ok,ms));
const port=Number(process.env.LUCIA_CDP_PORT||9292);

async function attach(){
  let page;
  for(let t=0;t<65;t++){
    const result=await fetch('http://127.0.0.1:'+port+'/json').then(r=>r.json()).catch(()=>[]);
    page=result.find(p=>p.type==='page'&&p.url?.includes('tauri.localhost'));
    if(page)break;
    await sleep(300);
  }
  assert.ok(page,'No native WebView2 browsing UI');
  const ws=new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no});
  let seq=0;const pending=new Map();
  ws.onmessage=e=>{
    const msg=JSON.parse(e.data),p=pending.get(msg.id);
    if(!p)return;pending.delete(msg.id);
    msg.error?p.reject(Error(JSON.stringify(msg.error))):p.resolve(msg.result||{});
  };
  const call=(method,params={})=>new Promise((resolve,reject)=>{
    const id=++seq;pending.set(id,{resolve,reject});
    ws.send(JSON.stringify({id,method,params}));
    setTimeout(()=>{if(pending.has(id)){
      pending.delete(id);reject(Error('CDP timeout '+method));
    }},14000);
  });
  const evalJs=async expression=>{
    const result=await call('Runtime.evaluate',{
      expression,returnByValue:true,awaitPromise:true,userGesture:true
    });
    if(result.exceptionDetails)throw Error(JSON.stringify(result.exceptionDetails).slice(0,300));
    return result.result?.value;
  };
  await call('Runtime.enable');
  for(let t=0;t<70;t++){
    if(await evalJs('Boolean(window.aetherBridge?.startLuciaSpecialistTask && window.browsphereApp?.createTaskTab)').catch(()=>false)){
      return {evalJs,close:()=>ws.close()};
    }
    await sleep(300);
  }
  ws.close();throw Error('Native Browsphere UI did not initialize');
}
async function main(){
  const {evalJs,close}=await attach();
  let taskTab;
  try{
    await evalJs("(async()=>{window.__hprAcceptance=[];for(const name of ['agent:stage','agent:run_completed','agent:run_status','agent:run_error','agent:verification_report'])await window.__TAURI__.event.listen(name,e=>window.__hprAcceptance.push({name,payload:e.payload}));return true})()");
    taskTab=await evalJs("window.browsphereApp.createTaskTab('browsphere://start')");
    assert.ok(taskTab,'Failed to allocate Lucia task tab');
    const goal='mcp call hyperresearch/search_notes {"query":"Ada Lovelace","limit":2}';
    const id=await evalJs("window.aetherBridge.startLuciaSpecialistTask("+
      JSON.stringify(goal)+",'browsphere://start','New Tab',"+JSON.stringify(taskTab)+")");
    assert.match(String(id||''),/^[a-f0-9-]{36}$/i);
    let events=[];
    for(let t=0;t<70;t++){
      await sleep(350);
      // agent:run_completed carries a summary string rather than run_id.
      // Include it explicitly so a false completion cannot escape detection.
      events=await evalJs("window.__hprAcceptance.filter(e=>e.name==='agent:run_completed'||e.payload?.run_id==="+JSON.stringify(id)+").slice(-100)")||[];
      if(events.some(e=>e.name==='agent:run_status'))break;
    }
    const status=events.find(e=>e.name==='agent:run_status')?.payload?.status;
    const errors=events.filter(e=>e.name==='agent:run_error')
      .map(e=>String(e.payload?.message||''));
    const text=errors.join('\n');
    const stages=events.filter(e=>e.name==='agent:stage').map(e=>e.payload?.status);
    const hasVerifiedReport=events.some(e=>e.name==='agent:verification_report');
    const falselyCompleted=events.some(e=>e.name==='agent:run_completed');
    const checks={
      unverifiedStatus:status==='unverified',
      realCachedResult:text.includes('cached_unverified') && text.includes('Ada Lovelace'),
      returnIsNotVerification:text.includes('not independently verified'),
      noVerifiedReport:!hasVerifiedReport,
      noFalseCompletion:!falselyCompleted,
      unverifiedStage:stages.includes('unverified'),
    };
    console.log('LUCIA_HYPERRESEARCH_WEBVIEW_ACCEPTANCE',JSON.stringify({
      checks,status,errors:errors.map(x=>x.slice(0,250)),stages
    }));
    assert.ok(Object.values(checks).every(Boolean),'Cached notes must not complete a factual research task');
    console.log('PASS Native WebView2 chatbox retrieval is explicitly unverified');
  }finally{
    try{await evalJs("window.__TAURI__.core.invoke('cancel_agent_run')")}catch{}
    if(taskTab)try{await evalJs("window.browsphereApp.closeTab("+JSON.stringify(taskTab)+")")}catch{}
    close();
  }
}
main().catch(e=>{console.error('LUCIA_HYPERRESEARCH_WEBVIEW_FAIL',e.stack||e);process.exitCode=1});
