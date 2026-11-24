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
    const localModel=process.env.LUCIA_TEST_LOCAL_MODEL?.trim() || 'qwen2.5:3b-instruct';
    const hasQwen=(modelStatus.models||[]).some(m=>(m.name||m.model)===localModel);
    const visionModel=process.env.LUCIA_TEST_VISION_MODEL?.trim() || '';
    const cloudEndpoint=process.env.LUCIA_TEST_CLOUD_ENDPOINT?.trim();
    const cloudModel=process.env.LUCIA_TEST_CLOUD_MODEL?.trim();
    if (Boolean(cloudEndpoint)!==Boolean(cloudModel)) throw Error('Cloud testing requires both endpoint and model');
    const useCloud=Boolean(cloudEndpoint && cloudModel);
    console.log('MODEL_ROUTE',useCloud ? 'cloud:'+new URL(cloudEndpoint).hostname :
      (hasQwen ? 'local:'+localModel : 'local:unavailable'));
    const overrides=useCloud ? {
      prefer_local_for_simple:false,lucia_cloud_only:true,
      litellm_url:cloudEndpoint,litellm_model:cloudModel,domain_overrides:{}
    } : {
      prefer_local_for_simple:true,lucia_cloud_only:false,
      ollama_url:'http://127.0.0.1:11434',ollama_model:localModel,vision_model:visionModel,domain_overrides:{}
    };
    await evalJs("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');Object.assign(c,"+
      JSON.stringify(overrides)+");await window.__TAURI__.core.invoke('update_router_config',{config:c});window.__crossSiteEvents=[];for(const name of ['agent:trace','agent:run_status','agent:run_error','agent:verification_report','agent:approval_required','agent:stage','agent:vision','agent:prerequisite','agent:supervisor','agent:orchestration','agent:workflow_graph'])await window.__TAURI__.event.listen(name,e=>window.__crossSiteEvents.push({kind:name,payload:e.payload}));return true})()");
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
      let events=[], end=null, approvalRequired=false,lastLogged=0;
      for(let i=0;i<timeoutSeconds;i++){
        await sleep(1000);
        events=(await evalJs('window.__crossSiteEvents.slice(-400)'))||[];
        const mine=events.filter(e=>e.payload?.run_id===id);
        for(const e of mine.slice(lastLogged)){console.log("LIVE_EVENT",JSON.stringify({kind:e.kind,status:e.payload?.status,action:e.payload?.action?.type,route:e.payload?.model_route,outcome:e.payload?.outcome,message:e.payload?.message,reason:e.payload?.reason,phase:e.payload?.phase}).slice(0,350));}lastLogged=mine.length;
        if(mine.some(e=>e.kind==='agent:approval_required')){
          approvalRequired=true;
          await evalJs("window.__TAURI__.core.invoke('cancel_agent_run')");
          break;
        }
        end=mine.find(e=>e.kind==='agent:run_status');
        if(end)break;
      }
      if(!end && !approvalRequired)await evalJs("window.__TAURI__.core.invoke('cancel_agent_run')");
      const mine=events.filter(e=>e.payload?.run_id===id);
      return {id,checkpointResumable:mine.filter(e=>e.kind==='agent:workflow_graph').at(-1)?.payload?.checkpoint_resumable===true,status:approvalRequired?'approval':(end?.payload?.status||'timeout'),
        approvalRequired, actions:mine.filter(e=>e.kind==='agent:trace')
        .map(e=>({type:e.payload?.action?.type,
          nodeId:e.payload?.action?.node_id,
          outcome:e.payload?.outcome,
          decision:e.payload?.decision,
          modelRoute:e.payload?.model_route})),
        errors:mine.filter(e=>e.kind==='agent:run_error').map(e=>String(e.payload?.message||'').slice(0,150)),
        vision:mine.filter(e=>e.kind==='agent:vision').map(e=>e.payload?.status),prerequisites:mine.filter(e=>e.kind==='agent:prerequisite').map(e=>({phase:e.payload?.phase,specialist:e.payload?.specialist,dialog:e.payload?.active_dialog})),
        graph:mine.filter(e=>e.kind==="agent:workflow_graph").at(-1)?.payload?.graph,report:mine.find(e=>e.kind==='agent:verification_report')?.payload?.report,verified:mine.some(e=>e.kind==='agent:verification_report' && e.payload?.report?.status==='evidence_checked')};
    }
    const cases=[
      {label:'MDN documentation navigation',url:'https://developer.mozilla.org/en-US/docs/Web/JavaScript',goal:'On this MDN JavaScript documentation page, open the JavaScript Guide using its visible link. Then report the page title.',test:(u,t,r)=>u.hostname==='developer.mozilla.org'&&/\/JavaScript\/Guide\/?$/i.test(u.pathname)&&/guide/i.test(t)},
      {label:'Python tutorial navigation',url:'https://docs.python.org/3/',goal:'On this Python documentation homepage, click the Tutorial link to open the Python Tutorial page. Then report the page title.',test:(u,t,r)=>u.hostname==='docs.python.org'&&/\/tutorial\//.test(u.pathname)&&/Python Tutorial/i.test(t)},
      {label:'Wiktionary factual lookup',url:'https://en.wiktionary.org/wiki/Wiktionary:Main_Page',goal:'On this Wiktionary page, search for the word serendipity and open its dictionary entry. Report the page title and the English meaning.',test:(u,t,r)=>u.hostname==='en.wiktionary.org'&&u.pathname.toLowerCase()==='/wiki/serendipity'&&/serendipity/i.test(t)&&/fortunate|unexpected|chance|accident/i.test(JSON.stringify(r||{}))},
      {label:'Wikipedia multi-fact research',url:'https://en.wikipedia.org/wiki/Main_Page',goal:'On Wikipedia, search for the planet Neptune, open the relevant article, and report its position from the Sun and whether it is a gas giant or ice giant. Use the observed article only.',test:(u,t,r)=>u.hostname==='en.wikipedia.org'&&u.pathname==='/wiki/Neptune'&&/eighth|8th/i.test(JSON.stringify(r||{}))&&/ice giant/i.test(JSON.stringify(r||{}))}
    ];
    const fs=require('node:fs'),path=require('node:path');
    if(!process.env.LOCALAPPDATA.includes('browsphere-checkpoint-profile-'))throw Error('Isolated profile required');
    const checkpointFixture=path.join(process.env.LOCALAPPDATA,'checkpoint-live-test.json');
    async function openPage(url){
      await evalJs('window.browsphereApp.newTab('+JSON.stringify(url)+')');
      for(let i=0;i<40;i++){
        const p=await nativeState().catch(()=>null);
        if(p?.controls&&p.observed&&hostname(p.observed)===hostname(url))return p;
        await sleep(400);
      }
      throw Error('Native destination unavailable');
    }
    if(process.env.LUCIA_CHECKPOINT_RESTART==='1'){
      const c=JSON.parse(fs.readFileSync(checkpointFixture,'utf8'));
      const before=await openPage(c.destination);
      const task=await runTask('resume checkpoint '+c.id+' '+c.goal,before.observed,25);
      const after=await nativeState();
      const pass=task.status==='completed'&&task.verified&&task.actions.length===0&&after.observed===c.destination&&task.graph?.nodes?.every(n=>n.state==='completed'&&n.receipt);
      record('Restarted browser checkpoint recovery',pass,{status:task.status,verified:task.verified,actions:task.actions.length,graph:task.graph,errors:task.errors});
      const wrong=await openPage('https://example.org/');
      const negative=await runTask('resume checkpoint '+c.id+' '+c.goal,wrong.observed,20);
      record('Changed destination refused without actions',negative.status!=='completed'&&!negative.verified&&negative.actions.length===0,{status:negative.status,errors:negative.errors});
      console.log('CHECKPOINT_RESTART_RESULT',JSON.stringify(result));
      if(result.some(x=>!x.success))process.exitCode=1;
      return;
    }
    for(const c of cases){
      try{
        const before=await openPage(c.url);
        console.log('GENERAL_START',JSON.stringify({name:c.label,before,goal:c.goal}));
        if(/access denied|not found|error/i.test(before.title))throw Error('Public page unavailable');
        const task=await runTask(c.goal,before.observed,40);
        const after=await nativeState();
        const landed=!!after?.observed&&c.test(new URL(after.observed),after.title,task.report);
        const acted=task.actions.some(a=>['click','type_text','press_key','select_option','set_checked'].includes(a.type)&&['ok','confirmed'].includes(a.outcome));
        const pass=task.status==='completed'&&task.verified&&acted&&landed&&!task.approvalRequired&&task.graph?.nodes?.length>=2&&task.graph.nodes.every(n=>n.state==='completed'&&n.receipt);
        console.log('GENERAL_DETAIL',JSON.stringify({name:c.label,pass,landed,acted,task,after}));
        record(c.label,pass,{status:task.status,landed,acted,verified:task.verified,approval:task.approvalRequired,checkpointResumable:task.checkpointResumable});
        if(pass&&task.checkpointResumable&&!fs.existsSync(checkpointFixture)){
          fs.writeFileSync(checkpointFixture,JSON.stringify({id:task.id,goal:c.goal,destination:after.observed}));
        }
      }catch(e){record(c.label,false,{error:String(e)});}
    }
    console.log('GENERAL_RESULT',JSON.stringify({passed:result.filter(x=>x.success).length,total:result.length,results:result}));
    if(result.some(x=>!x.success))process.exitCode=1;
  }finally{close();}
}
main().catch(e=>{console.error('CHECKPOINT_LIVE_FATAL',String(e.stack||e));process.exitCode=2;});
