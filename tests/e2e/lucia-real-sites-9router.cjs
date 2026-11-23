'use strict';
// Real hosted-model browsing tests across distinct public websites in native WebView2.
// No accounts, purchases, personal data, downloads, or consequential submissions.
// Requires an already running isolated Browsphere instance with LUCIA_CDP_PORT.
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const cdport=Number(process.env.LUCIA_CDP_PORT || 9553);
const base='http://127.0.0.1:'+cdport;
const cases=[
  {name:'Python documentation',start:'https://docs.python.org/3/',
   goal:'On this Python documentation page, search for asyncio and open the corresponding reference article. Then report its page title.',
   host:'docs.python.org',path:/\/library\/asyncio(?:\.html)?\/?$/i},
  {name:'MDN Web Docs',start:'https://developer.mozilla.org/en-US/',
   goal:'On this MDN website, search for Array and open the corresponding JavaScript reference article. Then report its page title.',
   host:'developer.mozilla.org',path:/\/JavaScript\/Reference\/Global_Objects\/Array\/?$/i},
  {name:'Wiktionary',start:'https://en.wiktionary.org/wiki/Wiktionary:Main_Page',
   goal:'On this Wiktionary page, search for photosynthesis and open the corresponding article. Then report its page title.',
   host:'en.wiktionary.org',path:/\/wiki\/photosynthesis\/?$/i},
  {name:'GitHub Docs',start:'https://docs.github.com/en',
   goal:'On this GitHub Docs page, search for About repositories and open the corresponding article. Then report its page title.',
   host:'docs.github.com',path:/\/about-repositories\/?$/i}
];
function safeHost(url){try{return new URL(url).host}catch{return ''}}
async function targets(){
  const r=await fetch(base+'/json',{signal:AbortSignal.timeout(6000)});
  if(!r.ok)throw Error('CDP /json HTTP '+r.status);
  return r.json();
}
async function attach(){
  let page;
  for(let i=0;i<75;i++){
    page=(await targets()).find(x=>x.type==='page' && x.url?.includes('tauri.localhost'));
    if(page)break;
    await sleep(400);
  }
  if(!page)throw Error('Missing native WebView2 app page');
  const ws=new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((ok,err)=>{ws.onopen=ok;ws.onerror=err});
  let seq=0;const pending=new Map();
  ws.onmessage=e=>{const m=JSON.parse(e.data),p=pending.get(m.id);if(!p)return;
    pending.delete(m.id);m.error?p.err(Error(JSON.stringify(m.error))):p.ok(m.result||{});
  };
  async function call(method,params={}){
    const id=++seq;
    const p=new Promise((ok,err)=>{pending.set(id,{ok,err});ws.send(JSON.stringify({id,method,params}));});
    try{return await Promise.race([p,sleep(16000).then(()=>{throw Error('CDP timeout '+method)})])}
    finally{pending.delete(id)}
  }
  async function evaluate(expression){
    const res=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});
    if(res.exceptionDetails)throw Error('Native UI evaluation failed: '+JSON.stringify(res.exceptionDetails).slice(0,350));
    return res.result?.value;
  }
  await call('Runtime.enable');
  for(let i=0;i<80;i++){
    if(await evaluate('Boolean(window.aetherBridge?.isNative && window.browsphereApp?.newTab)').catch(()=>false))
      return {evaluate,close:()=>ws.close()};
    await sleep(350);
  }
  throw Error('Native WebView2 UI not initialized');
}
async function main(){
  const {evaluate,close}=await attach();
  const endpoint=process.env.LUCIA_TEST_CLOUD_ENDPOINT;
  const model=process.env.LUCIA_TEST_CLOUD_MODEL;
  if(!endpoint || !model)throw Error('Missing model routing environment');
  if(model!=='oc/hy4-flash-free')throw Error('Wrong provider model for this test');
  const config={prefer_local_for_simple:false,lucia_cloud_only:true,
    litellm_url:endpoint,litellm_model:model,domain_overrides:{}};
  try{
    await evaluate("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');Object.assign(c,"+JSON.stringify(config)+");await window.__TAURI__.core.invoke('update_router_config',{config:c});window.__siteEvents=[];for(const n of ['agent:trace','agent:run_status','agent:run_error','agent:verification_report','agent:approval_required','agent:stage'])await window.__TAURI__.event.listen(n,e=>window.__siteEvents.push({kind:n,payload:e.payload}));return true})()");
    async function state(){
      return evaluate("(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return {url:s?.origin||'',title:(s?.title||'').slice(0,150),controls:s?.interactive_nodes?.length||0,excerptLength:s?.text_excerpt?.length||0,tabUrl:window.browsphereApp.activeTab()?.url}})()");
    }
    async function liveTask(c,timeoutSeconds=130){
      const id=await evaluate('window.aetherBridge.startLuciaSpecialistTask('+JSON.stringify(c.goal)+','+JSON.stringify(c.start)+",'Real public website')");
      if(!/^[a-f\d-]{36}$/i.test(String(id||'')))throw Error('No valid task UUID');
      console.log('SITE_TASK_STARTED',JSON.stringify({name:c.name,runId:id}));
      let seen=[],terminal=null,approval=false;
      for(let t=0;t<timeoutSeconds;t++){
        await sleep(1000);
        seen=(await evaluate('window.__siteEvents.slice(-500)')) || [];
        const own=seen.filter(e=>e.payload?.run_id===id);
        if(t>0 && t%20===0)console.log('SITE_TASK_HEARTBEAT',JSON.stringify({name:c.name,second:t,events:own.slice(-6).map(e=>e.kind),allStatusEvents:seen.filter(e=>e.kind==='agent:run_status').slice(-3).map(e=>({run_id:e.payload?.run_id,status:e.payload?.status}))}));
        if(own.some(e=>e.kind==='agent:approval_required')){
          approval=true;await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')");break;
        }
        terminal=own.find(e=>e.kind==='agent:run_status');
        if(terminal)break;
      }
      if(!terminal&&!approval)await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')");
      const own=seen.filter(e=>e.payload?.run_id===id);
      return {status:approval?'approval':terminal?.payload?.status||'timeout',
        actions:own.filter(e=>e.kind==='agent:trace').map(e=>({
          type:e.payload?.action?.type,outcome:e.payload?.outcome,
          route:e.payload?.model_route
        })),
        errors:own.filter(e=>e.kind==='agent:run_error').map(e=>String(e.payload?.message||'').slice(0,200)),
        verified:own.some(e=>e.kind==='agent:verification_report'&&e.payload?.report?.status==='evidence_checked'),
        stages:own.filter(e=>e.kind==='agent:stage').map(e=>({name:e.payload?.specialist,status:e.payload?.status}))
      };
    }
    const results=[];
    const selected=process.env.LUCIA_REAL_SITE ? cases.filter(x=>x.name.toLowerCase().includes(process.env.LUCIA_REAL_SITE.toLowerCase())) : cases;
    if(selected.length===0)throw Error('LUCIA_REAL_SITE did not match any case');
    for(const c of selected){
      let before=null,after=null,run=null,error=null;
      try{
        await evaluate('window.browsphereApp.newTab('+JSON.stringify(c.start)+')');
        for(let i=0;i<60;i++){
          before=await state();
          if(safeHost(before.url)===c.host&&before.controls>0&&before.excerptLength>=50)break;
          await sleep(500);
        }
        if(safeHost(before?.url)!==c.host||before.excerptLength<50||/error|not found|access denied/i.test(before.title||''))
          throw Error('Starting native page did not load as expected: '+JSON.stringify(before));
        console.log('LIVE_SITE_START',JSON.stringify({name:c.name,before}));
        run=await liveTask(c);
        after=await state();
      }catch(e){error=String(e);}
      const parsed=(()=>{try{return new URL(after?.url)}catch{return null}})();
      const observedDestination=!!parsed&&parsed.host===c.host&&c.path.test(parsed.pathname);
      const modelUsed=run?.actions?.some(x=>(x.route||'').includes('litellm:oc/hy4-flash-free'))||false;
      const meaningfulActions=run?.actions?.filter(x=>['click','type_text','press_key','select_option'].includes(x.type)).length||0;
      const passed=!error&&run?.status==='completed'&&run.verified&&observedDestination&&modelUsed&&meaningfulActions>=2;
      const record={name:c.name,passed,before,after,status:run?.status||'not-started',
        verified:run?.verified||false,observedDestination,modelUsed,meaningfulActions,
        actions:run?.actions||[],errors:run?.errors||[],error,stages:run?.stages||[]};
      console.log('REAL_SITE_RESULT',JSON.stringify(record));
      results.push({name:c.name,passed,status:record.status,observedDestination,modelUsed,verified:record.verified,error});
    }
    console.log('REAL_SITES_SUMMARY',JSON.stringify({passed:results.filter(x=>x.passed).length,total:results.length,results}));
    if(results.some(x=>!x.passed))process.exitCode=1;
  }finally{close()}
}
main().catch(e=>{console.error('REAL_SITES_FATAL',String(e.stack||e));process.exitCode=2});
