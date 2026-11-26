'use strict';
const sleep=(ms)=>new Promise(r=>setTimeout(r,ms));
const base='http://127.0.0.1:9397';
const goal="Search Google for the best free note-taking apps for Windows. Open the first relevant result and report its title.";
async function main(){
 let target;
 for(let i=0;i<60;i++){try{target=(await(await fetch(base+'/json',{signal:AbortSignal.timeout(3000)})).json()).find(x=>x.type==='page'&&x.url.includes('tauri.localhost'));if(target)break}catch{}await sleep(500)}
 if(!target)throw Error('Native UI CDP unavailable');
 const ws=new WebSocket(target.webSocketDebuggerUrl);
 await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no});
 let n=0;const waits=new Map();
 ws.onmessage=evt=>{const x=JSON.parse(evt.data),p=waits.get(x.id);if(p){waits.delete(x.id);x.error?p.no(Error(JSON.stringify(x.error))):p.ok(x.result||{})}};
 function send(method,params={}){return Promise.race([new Promise((ok,no)=>{const id=++n;waits.set(id,{ok,no});ws.send(JSON.stringify({id,method,params}))}),sleep(15000).then(()=>{throw Error('CDP method timeout '+method)})])}
 const ev=async expression=>{const r=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result?.value};
 try {
 await send('Runtime.enable');
 for(let i=0;i<60;i++){if(await ev('Boolean(window.aetherBridge?.startLuciaSpecialistTask&&window.browsphereApp?.createTaskTab)').catch(()=>false))break;await sleep(500)}
 if(!await ev('Boolean(window.browsphereApp?.createTaskTab)'))throw Error('App not ready');
 await ev("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');c.prefer_local_for_simple=true;c.lucia_cloud_only=false;c.ollama_url='http://127.0.0.1:11434';c.ollama_model='qwen3.5:0.8b';await window.__TAURI__.core.invoke('update_router_config',{config:c});window.__smoke=[];for(const name of ['agent:trace','agent:run_status','agent:run_error','agent:activity','agent:observation','agent:stage'])await window.__TAURI__.event.listen(name,e=>window.__smoke.push({kind:name,payload:e.payload}));return true})()");
 await ev("window.browsphereApp.newTab('https://www.google.com/')");
 let home;
 for(let i=0;i<40;i++){home=await ev("(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return {url:s?.origin||'',title:s?.title||'',controls:s?.interactive_nodes?.length||0}})()").catch(()=>null);if(home?.url?.includes('google.com')&&home?.controls>0)break;await sleep(600)}
 console.log('GOOGLE_HOME',JSON.stringify(home));
 const tab=await ev("window.browsphereApp.createTaskTab('https://www.google.com/')");
 if(!tab)throw Error('No task tab created');
 const id=await ev("window.aetherBridge.startLuciaSpecialistTask("+JSON.stringify(goal)+",'https://www.google.com/','Google',"+JSON.stringify(tab)+")");
 console.log('TASK_STARTED',JSON.stringify({id,tab}));
 let result={};
 for(let i=0;i<95;i++){
  await sleep(1200);
  const evs=await ev('window.__smoke.filter(x=>x.payload?.run_id==='+JSON.stringify(id)+').slice(-140)');
  const trace=evs.filter(e=>e.kind==='agent:trace').map(e=>({action:e.payload?.action?.type,route:e.payload?.model_route,outcome:e.payload?.outcome,decision:e.payload?.decision}));
  const events=evs.filter(e=>e.kind==='agent:run_error'||e.kind==='agent:run_status').map(e=>({kind:e.kind,payload:e.payload}));
  const snapshot=await ev("(async()=>{const s=await window.__TAURI__.core.invoke('capture_tab_snapshot',{tabId:"+JSON.stringify(tab)+"});return {url:s?.origin||'',title:(s?.title||'').slice(0,100),text_length:s?.text_excerpt?.length||0,controls:s?.interactive_nodes?.length||0}})()").catch(e=>({error:String(e)}));
  result={trace,events,snapshot};
  if(trace.some(t=>t.route?.includes('google_research:exact_user_query'))){console.log('GOOGLE_QUERY',JSON.stringify({trace:trace.filter(x=>x.route?.includes('google_research:exact_user_query')),snapshot}));const links=await ev("(async()=>{const s=await window.__TAURI__.core.invoke('capture_tab_snapshot',{tabId:"+JSON.stringify(tab)+"});return (s?.interactive_nodes||[]).filter(n=>n.href).slice(0,65).map(n=>({tag:n.tag,role:n.role,label:n.label,href:n.href,submit:n.is_submit,sensitive:n.is_sensitive}))})()");console.log('RESULT_LINKS',JSON.stringify(links).slice(0,15000));}
  if(events.some(x=>x.kind==='agent:run_status')||trace.some(t=>t.route?.includes('google_research:exact_user_query')&&(/verified|confirmed/i.test(t.outcome||''))))break;
 }
 console.log('GOOGLE_SMOKE',JSON.stringify(result).slice(0,11000));
 try{await ev("window.__TAURI__.core.invoke('cancel_agent_run')")}catch{}
 }finally{ws.close()}
}
main().catch(e=>{console.error('SMOKE_ERROR',e.stack||String(e));process.exitCode=1});
