'use strict';
// Real public Google -> first result. Isolated WebView2 profile; no accounts
// or submissions. CAPTCHA and restricted content are never bypassed.
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const port=Number(process.env.LUCIA_CDP_PORT||9272);
const goal="Search Google for the best free note-taking apps for Windows. Open the first relevant result and report its page title.";
const query="the best free note-taking apps for Windows";
async function attach(){
 let target;
 for(let i=0;i<75;i++){try{
   const r=await fetch('http://127.0.0.1:'+port+'/json',{signal:AbortSignal.timeout(2500)});
   target=(await r.json()).find(x=>x.type==='page'&&x.url?.includes('tauri.localhost'));
   if(target)break;
 }catch{}await sleep(350)}
 if(!target)throw Error('Missing native WebView2 debugging target');
 const ws=new WebSocket(target.webSocketDebuggerUrl);
 await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no});
 let seq=0;const pending=new Map();
 ws.onmessage=event=>{const data=JSON.parse(event.data),p=pending.get(data.id);
   if(!p)return;pending.delete(data.id);data.error?p.no(Error(JSON.stringify(data.error))):p.ok(data.result||{});
 };
 async function call(method,params={}){
   const id=++seq;
   return Promise.race([
     new Promise((ok,no)=>{pending.set(id,{ok,no});ws.send(JSON.stringify({id,method,params}));}),
     sleep(12000).then(()=>{if(pending.has(id)){pending.delete(id);throw Error('CDP timeout '+method);}})
   ]);
 }
 async function evaluate(expression){
   const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});
   if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails).slice(0,300));
   return r.result?.value;
 }
 await call('Runtime.enable');
 for(let i=0;i<75;i++){
   if(await evaluate('Boolean(window.aetherBridge?.startLuciaSpecialistTask&&window.browsphereApp?.createTaskTab)').catch(()=>false))
     return {evaluate,close:()=>ws.close()};
   await sleep(350);
 }
 ws.close();throw Error('Native UI not initialized');
}
async function main(){
 const {evaluate,close}=await attach();
 let taskTab=null;
 try{
   const modelPort=Number(process.env.LUCIA_MODEL_PORT||18766);
   const config="(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');"+
     "c.prefer_local_for_simple=true;c.lucia_cloud_only=false;"+
     "c.ollama_url='http://127.0.0.1:"+modelPort+"';c.ollama_model='lucia-local-e2e';"+
     "c.planner_model='lucia-local-e2e';c.supervisor_model='lucia-local-e2e';c.vision_model='';"+
     "await window.__TAURI__.core.invoke('update_router_config',{config:c});"+
     "window.__googleAcceptance=[];"+
     "for(const kind of ['agent:trace','agent:run_status','agent:run_error'])"+
     "await window.__TAURI__.event.listen(kind,e=>window.__googleAcceptance.push({kind,payload:e.payload}));return true})()";
   await evaluate(config);
   await evaluate("window.browsphereApp.newTab('https://www.google.com/')");
   let home=null;
   for(let i=0;i<55;i++){
     home=await evaluate("(async()=>{const p=await window.__TAURI__.core.invoke('capture_live_snapshot');return {origin:p?.origin||'',title:p?.title||'',controls:p?.interactive_nodes?.length||0}})()");
     if(home?.origin==='https://www.google.com/'&&home.controls>0)break;
     await sleep(500);
   }
   if(home?.origin!=='https://www.google.com/'||home.controls<1)
     throw Error('Google homepage unavailable: '+JSON.stringify(home));
   taskTab=await evaluate("window.browsphereApp.createTaskTab('https://www.google.com/')");
   if(!taskTab)throw Error('Task tab not created');
   const runId=await evaluate("window.aetherBridge.startLuciaSpecialistTask("+
     JSON.stringify(goal)+","+JSON.stringify(home.origin)+","+JSON.stringify(home.title)+","+
     JSON.stringify(taskTab)+")");
   if(!/^[0-9a-f-]{36}$/i.test(String(runId||'')))throw Error('No native agent run ID');
   let queryVerified=false,resultClick=false,destination=null,actions=[],errors=[],runStatuses=[];
   for(let i=0;i<110;i++){
     await sleep(1000);
     const events=await evaluate("window.__googleAcceptance.filter(e=>e.payload?.run_id==="+JSON.stringify(runId)+").slice(-160)");
     actions=events.filter(e=>e.kind==='agent:trace').map(e=>({
       type:e.payload?.action?.type,route:e.payload?.model_route,
       outcome:e.payload?.outcome,decision:e.payload?.decision
     }));
     errors=events.filter(e=>e.kind==='agent:run_error').map(e=>String(e.payload?.message||'').slice(0,200));
     runStatuses=events.filter(e=>e.kind==='agent:run_status').map(e=>e.payload?.status);
     queryVerified||=actions.some(a=>a.type==='navigate'&&a.route?.includes('google_research:exact_user_query')
       &&/verified|confirmed|success|ok/i.test(a.outcome||''));
     resultClick||=actions.some(a=>a.type==='click'&&a.route?.includes('google_research:first_observed_result')
       &&/verified|confirmed|success|ok/i.test(a.outcome||''));
     if(queryVerified){
       const snapshot=await evaluate("(async()=>{const p=await window.__TAURI__.core.invoke('capture_tab_snapshot',{tabId:"+
         JSON.stringify(taskTab)+"});return {origin:p?.origin||'',title:p?.title||'',chars:p?.text_excerpt?.length||0}})()");
       if(snapshot?.origin?.startsWith('https://')){
         const u=new URL(snapshot.origin);
         if((u.hostname==='www.google.com'||u.hostname==='google.com')&&u.pathname==='/search'
           &&u.searchParams.get('q')!==query)
           throw Error('Google search query changed unexpectedly');
         if(resultClick&&u.hostname!=='www.google.com'&&u.hostname!=='google.com'
           &&snapshot.title.length>=4&&snapshot.chars>=80){
           destination=snapshot;break;
         }
       }
     }
     if(runStatuses.length)break;
   }
   console.log('GOOGLE_NATIVE_ACCEPTANCE',JSON.stringify({
     queryVerified,resultClick,destination,actions,errors,runStatuses
   }).slice(0,11000));
   if(!queryVerified)throw Error('Exact Google query was not verified');
   if(!resultClick)throw Error('First relevant result click was not confirmed');
   if(!destination)throw Error('No readable non-Google result destination observed');
   console.log('PASS real Google query, native result click, readable external destination');
 }finally{
   try{await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')")}catch{}
   if(taskTab)try{await evaluate('window.browsphereApp.closeTab('+JSON.stringify(taskTab)+')')}catch{}
   close();
 }
}
main().catch(e=>{console.error('GOOGLE_NATIVE_ACCEPTANCE_FAIL',String(e.stack||e));process.exitCode=1;});