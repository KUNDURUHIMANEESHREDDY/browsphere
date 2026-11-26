'use strict';
// Native WebView2 acceptance: a short music request must enter YouTube
// without model-proposed downloads. Playback is verified separately.
const sleep=ms=>new Promise(ok=>setTimeout(ok,ms));
const port=Number(process.env.LUCIA_CDP_PORT||9398);
const goal='play any song';
async function attach(){
  let page;
  for(let k=0;k<70;k++){
    try{
      const r=await fetch('http://127.0.0.1:'+port+'/json',{signal:AbortSignal.timeout(2500)});
      page=(await r.json()).find(p=>p.type==='page'&&p.url?.includes('tauri.localhost'));
      if(page)break;
    }catch{}
    await sleep(300);
  }
  if(!page)throw Error('Native Browsphere UI was not available');
  const ws=new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no});
  let id=0;const promises=new Map();
  ws.onmessage=e=>{
    const m=JSON.parse(e.data),p=promises.get(m.id);
    if(!p)return;promises.delete(m.id);m.error?p.no(Error(JSON.stringify(m.error))):p.ok(m.result||{});
  };
  async function call(method,params={}){
    const seq=++id;
    return Promise.race([
      new Promise((ok,no)=>{promises.set(seq,{ok,no});ws.send(JSON.stringify({id:seq,method,params}));}),
      sleep(12000).then(()=>{if(promises.has(seq)){promises.delete(seq);throw Error('CDP timeout: '+method);}})
    ]);
  }
  async function evaluate(expression){
    const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});
    if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails).slice(0,350));
    return r.result?.value;
  }
  await call('Runtime.enable');
  for(let i=0;i<50;i++){
    if(await evaluate('Boolean(window.aetherBridge?.startLuciaSpecialistTask&&window.browsphereApp?.createTaskTab)').catch(()=>false))
      return {evaluate,close:()=>ws.close()};
    await sleep(350);
  }
  ws.close();throw Error('Lucia native task API unavailable');
}
async function main(){
  const {evaluate,close}=await attach();
  let tab;
  try{
    await evaluate("(async()=>{window.__musicSmoke=[];for(const kind of ['agent:trace','agent:run_error','agent:run_status'])await window.__TAURI__.event.listen(kind,e=>window.__musicSmoke.push({kind,payload:e.payload}));return true})()");
    tab=await evaluate("window.browsphereApp.createTaskTab('browsphere://start')");
    if(!tab)throw Error('No Lucia task tab');
    const runId=await evaluate("window.aetherBridge.startLuciaSpecialistTask("+JSON.stringify(goal)+",'browsphere://start','New Tab',"+JSON.stringify(tab)+")");
    if(!/^[a-f0-9-]{36}$/i.test(String(runId||'')))throw Error('No run ID');
    let verified=false,actions=[],errors=[],statuses=[];
    for(let i=0;i<115;i++){
      await sleep(1000);
      const evs=await evaluate("window.__musicSmoke.filter(e=>e.payload?.run_id==="+JSON.stringify(runId)+").slice(-80)");
      actions=evs.filter(e=>e.kind==='agent:trace').map(e=>({
        type:e.payload.action?.type,route:e.payload.model_route,
        outcome:e.payload.outcome,decision:e.payload.decision
      }));
      errors=evs.filter(e=>e.kind==='agent:run_error').map(e=>String(e.payload.message||'').slice(0,220));
      statuses=evs.filter(e=>e.kind==='agent:run_status').map(e=>e.payload.status);
      verified=actions.some(a=>a.type==='navigate'&&
        a.route?.includes('music_entry:trusted_user_goal')&&
        /confirmed/i.test(a.outcome||''));
      if(statuses.length)break;
    }
    console.log('LUCIA_MUSIC_FULL',JSON.stringify({runId,navigationVerified:verified,actions,errors,statuses}).slice(0,7000));
    if(!verified)throw Error('Lucia did not confirm first navigation to YouTube');
    if(actions.some(a=>a.type==='download'&&a.decision==='allow'))
      throw Error('Unrequested download was allowed');
    if(!statuses.includes('completed'))throw Error('Song playback was not independently verified; status='+JSON.stringify(statuses));
    console.log('PASS: play any song completed with native playback verification');
  }finally{
    try{await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')")}catch{}
    if(tab)try{await evaluate("window.browsphereApp.closeTab("+JSON.stringify(tab)+")")}catch{}
    close();
  }
}
main().catch(e=>{console.error('LUCIA_MUSIC_ENTRY_FAIL',e.stack||String(e));process.exitCode=1;});
