'use strict';
// Live read-only acceptance: open the exact Wikipedia article named in the
// user's request, verify native page title + URL, and require a source receipt.
// Uses a disposable Browsphere profile; never visits accounts or downloads.
const sleep=ms=>new Promise(ok=>setTimeout(ok,ms));
const port=Number(process.env.LUCIA_CDP_PORT||9399);
const goal='Open Wikipedia, find the Ada Lovelace article, and report its title and source URL. Verify the final webpage before finishing.';
const target='https://en.wikipedia.org/wiki/Ada_Lovelace';
async function attach(){
  let page;
  for(let t=0;t<75;t++){
    try{
      const r=await fetch('http://127.0.0.1:'+port+'/json',{signal:AbortSignal.timeout(2500)});
      page=(await r.json()).find(p=>p.type==='page'&&p.url?.includes('tauri.localhost'));
      if(page)break;
    }catch{}
    await sleep(350);
  }
  if(!page)throw Error('Browsphere native UI CDP not available');
  const ws=new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no});
  let id=0;const pending=new Map();
  ws.onmessage=e=>{const msg=JSON.parse(e.data),p=pending.get(msg.id);
    if(!p)return;pending.delete(msg.id);
    msg.error?p.reject(Error(JSON.stringify(msg.error))):p.resolve(msg.result||{});
  };
  const call=(method,params={})=>new Promise((resolve,reject)=>{
    const n=++id;pending.set(n,{resolve,reject});ws.send(JSON.stringify({id:n,method,params}));
    setTimeout(()=>{if(pending.has(n)){pending.delete(n);reject(Error('CDP timeout '+method));}},12000);
  });
  const evaluate=async expression=>{
    const result=await call('Runtime.evaluate',{
      expression,returnByValue:true,awaitPromise:true,userGesture:true});
    if(result.exceptionDetails)throw Error('Native UI evaluation error: '+JSON.stringify(result.exceptionDetails).slice(0,280));
    return result.result?.value;
  };
  await call('Runtime.enable');
  for(let t=0;t<70;t++){
    if(await evaluate("Boolean(window.aetherBridge?.startLuciaSpecialistTask && window.browsphereApp?.createTaskTab)").catch(()=>false))
      return {evaluate,close:()=>ws.close()};
    await sleep(350);
  }
  ws.close();throw Error('Browsphere UI did not initialize');
}
async function main(){
  const {evaluate,close}=await attach();
  let taskTab;
  try{
    await evaluate("(async()=>{window.__wikiAcceptance=[];for(const kind of ['agent:trace','agent:verification_report','agent:run_error','agent:run_status','agent:run_completed'])await window.__TAURI__.event.listen(kind,e=>window.__wikiAcceptance.push({kind,payload:e.payload}));return true})()");
    taskTab=await evaluate("window.browsphereApp.createTaskTab('browsphere://start')");
    if(!taskTab)throw Error('Failed to create dedicated Lucia task tab');
    const runId=await evaluate("window.aetherBridge.startLuciaSpecialistTask("+
      JSON.stringify(goal)+",'browsphere://start','New Tab',"+JSON.stringify(taskTab)+")");
    if(!/^[a-f0-9-]{36}$/i.test(String(runId||'')))throw Error('No native run ID');
    let report=null,complete=false,status=null,errors=[],actions=[];
    for(let t=0;t<100;t++){
      await sleep(900);
      const events=await evaluate("window.__wikiAcceptance.filter(e=>e.payload?.run_id==="+JSON.stringify(runId)+").slice(-90)");
      const terminal=events.find(e=>e.kind==='agent:run_status');
      const result=events.find(e=>e.kind==='agent:verification_report');
      const finished=events.find(e=>e.kind==='agent:run_completed');
      if(result?.payload?.report)report=result.payload.report;
      if(finished)complete=true;
      if(terminal)status=terminal.payload.status;
      errors=events.filter(e=>e.kind==='agent:run_error').map(e=>String(e.payload.message||'').slice(0,230));
      actions=events.filter(e=>e.kind==='agent:trace').map(e=>({
        action:e.payload?.action?.type,route:e.payload?.model_route,
        outcome:e.payload?.outcome,decision:e.payload?.decision
      }));
      if(complete||status)break;
    }
    const navigated=actions.some(a=>a.action==='navigate'&&
      /confirmed/i.test(a.outcome||''));
    const verified=report?.status==='evidence_checked'&&
      report.summary?.includes('Ada Lovelace')&&
      report.summary?.includes(target)&&
      report.findings?.some(f=>f.url===target&&
        f.quote?.includes('Ada Lovelace')&&f.quote?.includes('Wikipedia'));
    console.log('LUCIA_WIKIPEDIA_ACCEPTANCE',JSON.stringify({
      runId,completed:complete,status,navigated,verified,report,
      actions:actions.slice(-15),errors}).slice(0,12500));
    if(!navigated)throw Error('No verified native navigation to Wikipedia article');
    if(!verified)throw Error('Native article title URL and source evidence not verified');
    if(status&&status!=='completed')throw Error('Browser workflow ended with status '+status);
    if(!complete&&!status)throw Error('Wikipedia task did not complete');
    console.log('PASS Wikipedia Ada Lovelace article title and URL independently verified');
  }finally{
    try{await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')")}catch{}
    if(taskTab)try{await evaluate("window.browsphereApp.closeTab("+JSON.stringify(taskTab)+")")}catch{}
    close();
  }
}
main().catch(e=>{console.error('LUCIA_WIKIPEDIA_ACCEPTANCE_FAIL',e.stack||String(e));process.exitCode=1;});