'use strict';
// Real Windows WebView2 cross-site Lucia evaluation. Public sites only; isolated
// profile and no credentials, form submissions, downloads, or external writes.
// Set LUCIA_CDP_PORT, launch the current Browsphere executable separately.
const sleep = ms => new Promise(r => setTimeout(r, ms));
const port = Number(process.env.LUCIA_CDP_PORT || 9391);
const base = 'http://127.0.0.1:' + port;
async function targets() {
  const r=await fetch(base+'/json',{signal:AbortSignal.timeout(5000)});
  if(!r.ok) throw Error('CDP HTTP '+r.status);
  return r.json();
}
async function attach() {
  let page;
  for(let i=0;i<80;i++){
    page=(await targets()).find(x=>x.type==='page' && x.url?.includes('tauri.localhost'));
    if(page)break;
    await sleep(350);
  }
  if(!page)throw Error('Missing native Windows UI WebView2');
  const ws=new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((yes,no)=>{ws.onopen=yes;ws.onerror=no;});
  const pending=new Map();let seq=0;
  ws.onmessage=e=>{const m=JSON.parse(e.data),p=pending.get(m.id);
    if(!p)return;pending.delete(m.id);
    m.error?p.no(Error(JSON.stringify(m.error))):p.yes(m.result||{});
  };
  async function call(method,params={}){
    const id=++seq;
    return Promise.race([
      new Promise((yes,no)=>{pending.set(id,{yes,no});ws.send(JSON.stringify({id,method,params}));}),
      sleep(12000).then(()=>{if(pending.has(id)){pending.delete(id);throw Error('CDP timeout '+method);}})
    ]);
  }
  async function evalJs(js) {
    const r=await call('Runtime.evaluate',{expression:js,returnByValue:true,awaitPromise:true,userGesture:true});
    if(r.exceptionDetails)throw Error('JS exception '+JSON.stringify(r.exceptionDetails).slice(0,350));
    return r.result?.value;
  }
  await call('Runtime.enable');
  for(let i=0;i<90;i++) {
    if(await evalJs('Boolean(window.aetherBridge?.isNative && window.browsphereApp?.newTab)').catch(()=>false))return {evalJs,close:()=>ws.close()};
    await sleep(300);
  }
  throw Error('Native browser shell never initialized');
}
function hostname(url){try{return new URL(url).hostname}catch{return ''}}
const result=[];
function record(name,success,details){
  result.push({name,success:!!success,details});
  console.log((success?'PASS ':'FAIL ')+name+' '+JSON.stringify(details).slice(0,700));
}
async function main(){
  const {evalJs,close}=await attach();
  try{
    const modelStatus=await fetch('http://127.0.0.1:11434/api/tags',{signal:AbortSignal.timeout(3000)})
      .then(r=>r.json()).catch(()=>({models:[]}));
    const hasQwen=(modelStatus.models||[]).some(m=>['qwen2.5:3b-instruct'].includes(m.name||m.model));
    console.log('MODEL_AVAILABLE',hasQwen?'qwen2.5:3b-instruct':'none');
    await evalJs("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');c.prefer_local_for_simple=true;c.lucia_cloud_only=false;c.ollama_url='http://127.0.0.1:11434';c.ollama_model='qwen2.5:3b-instruct';c.domain_overrides={};await window.__TAURI__.core.invoke('update_router_config',{config:c});window.__crossSiteEvents=[];for(const name of ['agent:trace','agent:run_status','agent:run_error','agent:verification_report','agent:approval_required','agent:stage'])await window.__TAURI__.event.listen(name,e=>window.__crossSiteEvents.push({kind:name,payload:e.payload}));return true})()");
    async function nativeState(){
      // A newly created WebView2 may not have a loaded native snapshot yet.
      // Treat absence as pending, never as a verified page or a JS exception.
      return evalJs("(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return {observed:s?.origin||null,title:(s?.title||'').slice(0,110),controls:s?.interactive_nodes?.length||0,visible:window.browsphereApp.activeTab()?.url,omnibar:document.getElementById('omnibarInput')?.value}})()");
    }
    async function runTask(goal,initial='browsphere://start',timeoutSeconds=60){
      const config=await evalJs("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');return {local:c.prefer_local_for_simple,cloudOnly:c.lucia_cloud_only,localModel:c.ollama_model,cloudModel:c.litellm_model}})()");
      console.log('ROUTER_BEFORE_TASK',JSON.stringify({goal:goal.slice(0,80),config}));
      const id=await evalJs('window.aetherBridge.startLuciaSpecialistTask('+JSON.stringify(goal)+','+JSON.stringify(initial)+",'Cross-site test')");
      if(!/^[a-f0-9-]{36}$/i.test(String(id||''))) throw Error('No task ID');
      let events=[], end=null;
      for(let i=0;i<timeoutSeconds;i++){
        await sleep(1000);
        events=(await evalJs('window.__crossSiteEvents.slice(-400)'))||[];
        const mine=events.filter(e=>e.payload?.run_id===id);
        if(mine.some(e=>e.kind==='agent:approval_required')){await evalJs("window.__TAURI__.core.invoke('cancel_agent_run')");return {status:'approval',events:mine};}
        end=mine.find(e=>e.kind==='agent:run_status');
        if(end)break;
      }
      if(!end)await evalJs("window.__TAURI__.core.invoke('cancel_agent_run')");
      const mine=events.filter(e=>e.payload?.run_id===id);
      return {status:end?.payload?.status||'timeout',actions:mine.filter(e=>e.kind==='agent:trace')
        .map(e=>({type:e.payload?.action?.type,outcome:e.payload?.outcome,modelRoute:e.payload?.model_route})),
        errors:mine.filter(e=>e.kind==='agent:run_error').map(e=>String(e.payload?.message||'').slice(0,150)),
        verified:mine.some(e=>e.kind==='agent:verification_report' && e.payload?.report?.status==='evidence_checked')};
    }
    const cases=[
      ['Example Domain','open example.com','example.com'],
      ['GitHub','open github.com','github.com'],
      ['Wikipedia','open wikipedia.org','wikipedia.org'],
      ['Hacker News','open news.ycombinator.com','news.ycombinator.com']
    ];
    for(const [name,goal,host] of cases) {
      try{
        await evalJs("window.browsphereApp.newTab('browsphere://start')");
        const task=await runTask(goal,'browsphere://start',60);
        let state=await nativeState();
        for(let retries=0;retries<12 && !state.observed;retries++){
          await sleep(400);
          state=await nativeState();
        }
        const nativeWebViews=(await targets()).filter(x=>x.type==='page').map(x=>x.url);
        const actual=nativeWebViews.some(u=>hostname(u)===host || hostname(u)==='www.'+host);
        const matched=(hostname(state.observed)===host || hostname(state.observed)==='www.'+host) &&
          (hostname(state.visible)===host || hostname(state.visible)==='www.'+host);
        record(name+' autonomous open',task.status==='completed' && actual && matched,
          {status:task.status,actualWebView:actual,state,actions:task.actions,errors:task.errors,verified:task.verified});
      }catch(e){record(name+' autonomous open',false,{error:String(e)});}
    }
    if(hasQwen){
      try {
        await evalJs("window.browsphereApp.newTab('https://en.wikipedia.org/wiki/Main_Page')");
        let before;
        for(let i=0;i<40;i++){
          before=await nativeState();
          if(hostname(before.observed)==='en.wikipedia.org' && before.controls>0)break;
          await sleep(400);
        }
        if(hostname(before?.observed)!=='en.wikipedia.org')throw Error('Wikipedia starting page unavailable');
        const task=await runTask('On this Wikipedia page, search for Ada Lovelace and open the corresponding article. Then report its page title.','https://en.wikipedia.org/wiki/Main_Page',140);
        const after=await nativeState();
        record('Wikipedia real-model search and article',task.status==='completed' &&
          hostname(after.observed)==='en.wikipedia.org' &&
          decodeURIComponent(new URL(after.observed).pathname).toLowerCase().includes('ada_lovelace') &&
          task.verified,{status:task.status,before,after,actions:task.actions,errors:task.errors,verified:task.verified});
      }catch(e){record('Wikipedia real-model search and article',false,{error:String(e)});}
    }else record('Wikipedia real-model search and article',false,{reason:'Local Qwen unavailable; no genuine model call attempted'});
  }finally{close();}
  console.log('CROSS_SITE_RESULT',JSON.stringify({passed:result.filter(x=>x.success).length,total:result.length,results:result.map(x=>({name:x.name,success:x.success}))}));
  if(result.some(x=>!x.success))process.exitCode=1;
}
main().catch(e=>{console.error('CROSS_SITE_FATAL',String(e.stack||e));process.exitCode=2;});
