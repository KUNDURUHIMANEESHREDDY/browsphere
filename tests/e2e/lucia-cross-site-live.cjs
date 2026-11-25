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
    const revision=await evalJs("window.__TAURI__.core.invoke('lucia_runtime_revision')");
    console.log('LUCIA_RUNTIME_REVISION',revision);
    if(revision!=='lucia-approved-mcp-discovery-v13.11')throw Error('Native runtime is not the expected objective-gate build');
    // An isolated test gateway may inject a controlled model failure.
    // It must be loopback HTTP and cannot redirect to a remote provider.
    const testOllama=process.env.LUCIA_TEST_OLLAMA_URL||'http://127.0.0.1:11434';
    if(!/^http:\/\/127\.0\.0\.1:\d{1,5}$/.test(testOllama))throw Error('Invalid test-only Ollama endpoint');
    const modelStatus=await fetch(testOllama+'/api/tags',{signal:AbortSignal.timeout(3000),redirect:'error'})
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
      ollama_url:testOllama,ollama_model:localModel,vision_model:visionModel,planner_model:process.env.LUCIA_TEST_PLANNER_MODEL||'qwen2.5:3b-instruct',supervisor_model:process.env.LUCIA_TEST_SUPERVISOR_MODEL||'lfm3:1.7b',domain_overrides:{}
    };
    await evalJs("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');Object.assign(c,"+
      JSON.stringify(overrides)+");await window.__TAURI__.core.invoke('update_router_config',{config:c});window.__crossSiteEvents=[];for(const name of ['agent:trace','agent:run_status','agent:run_error','agent:verification_report','agent:approval_required','agent:plan','agent:stage','agent:vision','agent:prerequisite','agent:supervisor','agent:workflow_graph','agent:objective_replan','agent:workflow_browser_step','agent:workflow_dynamic_tool','agent:objective_state','agent:skill_state','agent:model_role','agent:execution_state','agent:requirement_state','agent:evidence_acquisition','agent:catalog_state'])await window.__TAURI__.event.listen(name,e=>window.__crossSiteEvents.push({kind:name,payload:e.payload}));return true})()");
    async function nativeState(){
      // A newly created WebView2 may not have a loaded native snapshot yet.
      // Treat absence as pending, never as a verified page or a JS exception.
      return evalJs("(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return {observed:s?.origin||null,title:(s?.title||'').slice(0,110),controls:s?.interactive_nodes?.length||0,visible:window.browsphereApp.activeTab()?.url,omnibar:document.getElementById('omnibarInput')?.value}})()");
    }
    async function runTask(goal,initial='browsphere://start',timeoutSeconds=60,testApproval=null){
      const config=await evalJs("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');return {local:c.prefer_local_for_simple,cloudOnly:c.lucia_cloud_only,localModel:c.ollama_model,cloudModel:c.litellm_model}})()");
      console.log('ROUTER_BEFORE_TASK',JSON.stringify({goal:goal.slice(0,80),config}));
      const id=await evalJs('window.aetherBridge.startLuciaSpecialistTask('+JSON.stringify(goal)+','+JSON.stringify(initial)+",'Cross-site test')");
      if(!/^[a-f0-9-]{36}$/i.test(String(id||''))) throw Error('No task ID');
      let events=[], end=null, approvalRequired=false,approvalReceipts=[];
      const handledApprovals=new Set();
      for(let i=0;i<timeoutSeconds;i++){
        await sleep(1000);
        events=(await evalJs('window.__crossSiteEvents.slice(-400)'))||[];
        const mine=events.filter(e=>e.payload?.run_id===id);
        const approvals=mine.filter(e=>e.kind==='agent:approval_required'&&!handledApprovals.has(e.payload?.approval_id));
        if(approvals.length){
          for(const e of approvals){
            const a=e.payload,expected=testApproval&&approvalReceipts.length===0&&
              process.env.LOCALAPPDATA.includes('browsphere-cross-site-')&&a?.action?.type==='navigate'&&a.action.url===testApproval.url&&
              /^Cross-origin navigation to /.test(a.reason||'');
            if(!expected){approvalRequired=true;await evalJs("window.__TAURI__.core.invoke('cancel_agent_run')");break;}
            handledApprovals.add(a.approval_id);
            await evalJs("window.__TAURI__.core.invoke('resolve_approval',{approvalId:"+JSON.stringify(a.approval_id)+",approved:"+JSON.stringify(testApproval.approved)+"})");
            approvalReceipts.push({url:a.action.url,approved:testApproval.approved});
          }
          if(approvalRequired)break;
        }
        end=mine.find(e=>e.kind==='agent:run_status');
        if(end)break;
      }
      if(!end && !approvalRequired)await evalJs("window.__TAURI__.core.invoke('cancel_agent_run')");
      const mine=events.filter(e=>e.payload?.run_id===id);
      return {status:approvalRequired?'approval':(end?.payload?.status||'timeout'),
        approvalRequired,approvalReceipts, actions:mine.filter(e=>e.kind==='agent:trace')
        .map(e=>({type:e.payload?.action?.type,
          nodeId:e.payload?.action?.node_id,
          outcome:e.payload?.outcome,
          decision:e.payload?.decision,
          modelRoute:e.payload?.model_route})),
        errors:mine.filter(e=>e.kind==='agent:run_error').map(e=>String(e.payload?.message||'').slice(0,150)),
        catalogStates:mine.filter(e=>e.kind==='agent:catalog_state').map(e=>e.payload?.retailers),
        acquisitions:mine.filter(e=>e.kind==='agent:evidence_acquisition').map(e=>({status:e.payload?.status,attempt:e.payload?.attempt,missing:e.payload?.missing,candidateCounts:e.payload?.candidate_counts,scopeCurrent:e.payload?.scope_current,excerptChars:e.payload?.excerpt_chars})),
        requirements:mine.filter(e=>e.kind==='agent:requirement_state').map(e=>e.payload?.ledger),
        executionStates:mine.filter(e=>e.kind==='agent:execution_state').map(e=>({phase:e.payload?.phase,strategy:e.payload?.strategy,world:e.payload?.world,page:e.payload?.page})),
        modelRoles:mine.filter(e=>e.kind==='agent:model_role').map(e=>({role:e.payload?.role,model:e.payload?.model,status:e.payload?.status})),
        vision:mine.filter(e=>e.kind==='agent:vision').map(e=>e.payload?.status),
        visionDetails:mine.filter(e=>e.kind==='agent:vision').map(e=>({status:e.payload?.status,reason:e.payload?.reason,failure:e.payload?.failure,failureDetail:e.payload?.failure_detail,inferenceTrace:e.payload?.inference_trace,fusion:e.payload?.fusion,groundedPoints:e.payload?.grounded_points,independentGrounded:e.payload?.independent_grounded,independentProposed:e.payload?.independent_proposed,matchedControl:e.payload?.matched_control})),prerequisites:mine.filter(e=>e.kind==='agent:prerequisite').map(e=>({phase:e.payload?.phase,specialist:e.payload?.specialist,dialog:e.payload?.active_dialog})),
        skills:mine.filter(e=>e.kind==='agent:skill_state').map(e=>({phase:e.payload?.phase,diagnosis:e.payload?.diagnosis,strategy:e.payload?.recovery_strategy})),
        objectives:mine.filter(e=>e.kind==='agent:objective_state').map(e=>({revision:e.payload?.revision,required:e.payload?.required_sources,observed:e.payload?.observed_sources})),
        plan:mine.filter(e=>e.kind==='agent:plan').at(-1)?.payload?.plan||null,
        stageEvents:mine.filter(e=>e.kind==='agent:stage').map(e=>({index:e.payload?.index,status:e.payload?.status,objective:e.payload?.objective})),
        graph:mine.filter(e=>e.kind==='agent:workflow_graph').at(-1)?.payload?.graph||null,
        replans:mine.filter(e=>e.kind==='agent:objective_replan').map(e=>({stage:e.payload?.stage,strategy:e.payload?.strategy,revision:e.payload?.revision})),
        workflowBrowser:mine.filter(e=>e.kind==='agent:workflow_browser_step').map(e=>e.payload?.status),
        dynamicTools:mine.filter(e=>e.kind==='agent:workflow_dynamic_tool').map(e=>({status:e.payload?.status,index:e.payload?.index,graph:e.payload?.child_graph})),
        verificationReports:mine.filter(e=>e.kind==='agent:verification_report').length,
        report:mine.filter(e=>e.kind==='agent:verification_report').at(-1)?.payload?.report||null,
        verified:mine.some(e=>e.kind==='agent:verification_report' && e.payload?.report?.status==='evidence_checked')};
    }
    // Phase 3 cases keep expected answers in the evaluator, never in the model prompt.
    if(process.env.LUCIA_PHASE3_CASE){
      if(useCloud||!hasQwen)throw Error('Phase 3 requires installed local models');
      const key=process.env.LUCIA_PHASE3_CASE;
      if(['structured-chart-fixture','pixel-canvas-fixture','pixel-image-fixture'].includes(key)){
        const pixel=key!=='structured-chart-fixture',imageOnly=key==='pixel-image-fixture';
        // Isolated synthetic DOM in an actual native WebView2 child. This
        // does not modify a remote page/server or assert public chart coverage.
        if(!process.env.LOCALAPPDATA.includes('browsphere-cross-site-'))throw Error('Non-isolated fixture refused');
        const url='https://example.com/';
        await evalJs('window.browsphereApp.newTab('+JSON.stringify(url)+')');
        let child;
        for(let i=0;i<40;i++){
          const matches=(await targets()).filter(t=>t.type==='page'&&t.url===url);
          if(matches.length===1){child=matches[0];break;}await sleep(500);
        }
        if(!child)throw Error('Missing isolated native child WebView2');
        const ws=new WebSocket(child.webSocketDebuggerUrl);
        await new Promise((yes,no)=>{ws.onopen=yes;ws.onerror=no;});
        try{
          let seq=0;
          const call=(method,params={})=>new Promise((yes,no)=>{
            const id=++seq;const timer=setTimeout(()=>no(Error('Fixture CDP timeout')),5000);
            ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id!==id)return;clearTimeout(timer);m.error?no(Error(JSON.stringify(m.error))):yes(m.result);};
            ws.send(JSON.stringify({id,method,params}));
          });
          const fixture='<main><h1>Rainfall data</h1><p>Public synthetic rainfall example. These values are only for local browser testing.</p><figure><figcaption>Rainfall chart</figcaption><svg role="img" width="360" height="150" viewBox="0 0 360 150"><g><rect x="20" y="30" width="60" height="80"/><text x="20" y="125">Jan</text><text x="20" y="20">12 mm</text></g><g><rect x="120" y="55" width="60" height="55"/><text x="120" y="125">Feb</text><text x="120" y="45">9 mm</text></g></svg></figure><a href="/">Example source</a></main>';
          const pixels='<main><h1>Rainfall example</h1><p>Public synthetic chart for local testing.</p><figure><figcaption>Rainfall chart</figcaption><canvas width="480" height="240" style="display:block;width:480px;height:240px"></canvas></figure><a href="/">Example source</a></main>';
          const draw=()=>{const c=document.querySelector('canvas'),x=c.getContext('2d');x.fillStyle='#ffffff';x.fillRect(0,0,480,240);x.fillStyle='#182f50';x.fillRect(65,66,100,125);x.fillRect(295,98,100,93);x.fillStyle='#111111';x.font='bold 27px Arial';x.textAlign='center';x.fillText('12 mm',115,45);x.fillText('9 mm',345,78);x.fillText('Jan',115,223);x.fillText('Feb',345,223);};
          const rasterScript=pixel?';('+draw.toString()+')();'+(imageOnly?'const c=document.querySelector("canvas");const img=new Image();img.width=480;img.height=240;img.style.display="block";img.src=c.toDataURL("image/png");c.replaceWith(img);':''):'';
          const set=await call('Runtime.evaluate',{expression:'document.body.innerHTML='+JSON.stringify(pixel?pixels:fixture)+';document.title="Synthetic rainfall fixture"'+rasterScript+';true',returnByValue:true});
          if(set?.result?.value!==true)throw Error('Synthetic chart fixture injection failed');
          let ready=false;
          for(let i=0;i<20;i++){const page=await nativeState();if(page?.observed===url&&page.title==='Synthetic rainfall fixture'){ready=true;break;}await sleep(300);}
          if(!ready)throw Error('Synthetic chart not natively observed');
          const task=await runTask('Extract charts from this page',url,pixel?150:60);
          const findings=task.report?.findings||[];
          const checks=pixel?{terminal:task.status==='unverified',verified:task.verified===false,noApproval:!task.approvalRequired,
            provisional:task.report?.status==='partial_evidence'&&task.report?.summary.includes('Provisional visual chart readings')&&findings.length===0,
            exactChartPairs:task.report?.summary.includes('Jan: 12 mm')&&task.report?.summary.includes('Feb: 9 mm')&&task.report?.summary.includes(imageOnly?'(img):':'(canvas):'),
            noExternalActions:task.actions.length===0,fixtureOnly:true}:{terminal:task.status==='completed',verified:task.verified===true,noApproval:!task.approvalRequired,
            exactChartPairs:findings.length===2&&findings[0]?.quote==='Jan: 12 mm'&&findings[1]?.quote==='Feb: 9 mm'&&findings.every(f=>f.url===url),
            noExternalActions:task.actions.length===0,fixtureOnly:true};
          const pass=Object.values(checks).every(Boolean);
          console.log(pixel?'PIXEL_CHART_RESULT':'STRUCTURED_CHART_RESULT',JSON.stringify({pass,checks,task}));
          if(!pass)process.exitCode=1;
          return;
        }finally{ws.close();}
      }
      if(key==='structured-table'){
        const url='https://www.w3schools.com/html/html_tables.asp';
        await evalJs('window.browsphereApp.newTab('+JSON.stringify(url)+')');
        let ready=false;
        for(let i=0;i<40;i++){const page=await nativeState();if(page?.observed===url&&page.controls>0){ready=true;break;}await sleep(500);}
        if(!ready)throw Error('Public table fixture unavailable');
        const task=await runTask('Extract tables from this page',url,60);
        const findings=task.report?.findings||[];
        const checks={terminal:task.status==='completed',verified:task.verified===true,noApproval:!task.approvalRequired,
          exactObservedRow:findings.some(f=>f.url===url&&f.claim.includes('Company = Alfreds Futterkiste')&&f.claim.includes('Contact = Maria Anders')&&f.claim.includes('Country = Germany')),
          noExternalActions:task.actions.length===0};
        const pass=Object.values(checks).every(Boolean);
        console.log('STRUCTURED_TABLE_RESULT',JSON.stringify({pass,checks,task}));
        record('Native public table extraction',pass,{checks});
        if(!pass)process.exitCode=1;
        return;
      }

      const cases={
        'search-wikipedia':{tier:'skill',skill:'search',start:'https://en.wikipedia.org/wiki/Main_Page',
          goal:'On this Wikipedia page, search for Grace Hopper and open the corresponding article. Then report its page title.',
          path:'/wiki/Grace_Hopper',title:/Grace Hopper/i},
        'search-wiktionary':{tier:'skill',skill:'search',start:'https://en.wiktionary.org/wiki/Wiktionary:Main_Page',
          goal:'On this Wiktionary page, search for the word resilient and open the corresponding article. Then report its page title.',
          path:'/wiki/resilient',title:/resilient/i},
        'nav-rust':{tier:'skill',skill:'navigation',start:'https://doc.rust-lang.org/stable/book/index.html',
          goal:'On this Rust Book page, click the Getting Started link. Then report the page title.',
          path:'/stable/book/ch01-00-getting-started.html',title:/Getting Started/i},
        'nav-python':{tier:'skill',skill:'navigation',start:'https://docs.python.org/3/',
          goal:'On this Python documentation page, click the Tutorial link. Then report the page title.',
          path:'/3/tutorial/index.html',title:/Python Tutorial/i},
        'extract-mdn':{tier:'skill',skill:'extraction',start:'https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/isArray',
          goal:'Read the current official Array.isArray documentation and report what the method determines. Cite the observed source evidence.',
          path:'/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/isArray',fact:/array/i,extra:/determin|check|whether|true|false/i},
        'extract-python':{tier:'skill',skill:'extraction',start:'https://docs.python.org/3/library/collections.html',
          goal:'Read the current official collections documentation and explain the purpose of Counter. Cite a supporting passage.',
          path:'/3/library/collections.html',fact:/counter/i,extra:/count|tally/i},
        'compose-wiki':{tier:'composition',skill:'search-navigation-extraction',start:'https://en.wikipedia.org/wiki/Main_Page',
          goal:'On this website, search for the planet Uranus and open the corresponding article. Report its position from the Sun.',
          path:'/wiki/Uranus',fact:/seventh|7th/i,extra:/sun/i,requiresSearch:true},
        'compose-dictionary':{tier:'composition',skill:'search-navigation-extraction',start:'https://en.wiktionary.org/wiki/Wiktionary:Main_Page',
          goal:'On this website, search for the word ephemeral and open its dictionary entry. Report its English meaning.',
          path:'/wiki/ephemeral',fact:/short|brief|transient|temporary|fleeting/i,extra:/time|duration|lasting|live/i,requiresSearch:true},
        'heldout-mdn':{tier:'heldout',skill:'reference-research',start:'https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Number/isInteger',
          goal:'Read the current Number.isInteger reference. Explain what the method determines, with an exact supporting passage.',
          path:'/en-US/docs/Web/JavaScript/Reference/Global_Objects/Number/isInteger',fact:/integer/i,extra:/determin|whether|true|false/i},
        'heldout-wiki':{tier:'heldout',skill:'search-navigation-extraction',start:'https://en.wikipedia.org/wiki/Main_Page',
          goal:'On this website, search for the planet Saturn and open the corresponding article. Report its position from the Sun.',
          path:'/wiki/Saturn',fact:/sixth|6th/i,extra:/sun/i,requiresSearch:true},
      };
      // Coordinate-specific external cases: ordinary public documentation only.
      if(key==='vision-real-example'){
        const start='https://example.com/';
        await evalJs('window.browsphereApp.newTab('+JSON.stringify(start)+')');
        let before;
        for(let i=0;i<45;i++){before=await nativeState().catch(()=>null);if(before?.controls>0&&hostname(before.observed)==='example.com')break;await sleep(400);}
        if(!before?.controls||hostname(before.observed)!=='example.com'){
          console.log('VISION_REAL_RESULT',JSON.stringify({pass:false,blocked:true,reason:'Starting page unavailable',before}));process.exitCode=1;return;
        }
        const task=await runTask('Find more information about example domains using the link on this page. Report the title of the page you reach.',before.observed,100);
        const after=await nativeState().catch(()=>null);
        const checks={terminal:task.status==='completed',verified:task.verified,noApproval:!task.approvalRequired,
          exactDestination:hostname(after?.observed)==='www.iana.org'&&new URL(after.observed).pathname.replace(/\/$/,'')==='/help/example-domains',
          coordinateDispatch:task.vision.includes('coordinate_click'),
          actionTrace:task.actions.some(a=>a.type==='click'&&a.modelRoute?.includes('vision:coordinate_click')&&['ok','confirmed'].includes(a.outcome))};
        const pass=Object.values(checks).every(Boolean);
        console.log('VISION_REAL_RESULT',JSON.stringify({pass,checks,before,after,task}));if(!pass)process.exitCode=1;return;
      }
      const c=cases[key];
      if(c){
        try{
          await evalJs('window.browsphereApp.newTab('+JSON.stringify(c.start)+')');
          let before;
          for(let i=0;i<45;i++){before=await nativeState().catch(()=>null);if(before?.controls>0&&hostname(before.observed)===hostname(c.start)&&!/access denied|not found|privacy error/i.test(before.title||''))break;await sleep(400);}
          if(!before?.controls||hostname(before.observed)!==hostname(c.start)||/access denied|not found|privacy error/i.test(before.title||'')){
            console.log('PHASE3_RESULT',JSON.stringify({key,tier:c.tier,skill:c.skill,pass:false,blocked:true,reason:'Starting page unavailable',before}));process.exitCode=1;return;
          }
          const task=await runTask(c.goal,before.observed,110);
          const after=await nativeState().catch(()=>null);
          const exact=hostname(after?.observed)===hostname(c.start)&&new URL(after.observed).pathname===c.path;
          const acts=task.actions.filter(a=>['ok','confirmed'].includes(a.outcome)&&a.type!=='complete');
          const findings=task.report?.findings||[];
          const checks={terminal:task.status==='completed',verified:task.verified,noApproval:!task.approvalRequired,exactDestination:exact};
          if(c.title){checks.title=c.title.test(after?.title||'');checks.actionObserved=acts.length>0;}
          if(key==='nav-python'&&process.env.LUCIA_REQUIRE_COORDINATE==='1'){
            checks.coordinateDispatch=task.vision.includes('coordinate_click');
            checks.coordinateTrace=acts.some(a=>a.type==='click'&&a.modelRoute?.includes('vision:coordinate_click'));
          }
          if(c.fact){
            checks.fact=!!findings.some(f=>{try{return hostname(f.url)===hostname(c.start)&&new URL(f.url).pathname===c.path&&c.fact.test(f.quote||'')&&c.extra.test(f.quote||'')&&c.fact.test(f.claim||'')&&c.extra.test(f.claim||'')}catch{return false}});
          }
          if(c.requiresSearch)checks.searchAndNavigation=acts.some(a=>a.type==='type_text')&&acts.some(a=>a.type==='navigate'||a.type==='click'||a.type==='press_key');
          const pass=Object.values(checks).every(Boolean);
          console.log('PHASE3_RESULT',JSON.stringify({key,tier:c.tier,skill:c.skill,pass,checks,before,after,task}));
          if(!pass)process.exitCode=1;return;
        }catch(e){console.log('PHASE3_RESULT',JSON.stringify({key,tier:c.tier,skill:c.skill,pass:false,blocked:true,reason:String(e).slice(0,300)}));process.exitCode=1;return;}
      }
      if(key==='vision-independent'){
        const http=require('node:http');
        const requests=[];
        const server=http.createServer((req,res)=>{
          requests.push({method:req.method,path:req.url});
          if(req.method!=='GET'||!['/catalog','/guide','/favicon.ico'].includes(req.url)){res.writeHead(403);res.end();return;}
          if(req.url==='/favicon.ico'){res.writeHead(404);res.end();return;}
          res.setHeader('Content-Type','text/html; charset=utf-8');
          if(req.url==='/guide'){res.end('<!doctype html><title>Field Guide</title><main><h1>Field Guide</h1><p>This public field guide explains how plants grow using sunlight and water.</p><p>Healthy soil supplies essential nutrients for a garden.</p></main>');return;}
          // 140 rendered ordinary controls precede the target in DOM order,
          // but sit well below the viewport. The named link is visible.
          res.end('<!doctype html><title>Public library</title><style>body{font:24px Arial;color:#172038;background:#fff;padding:25px}main{padding-top:2000px}button{display:block}#target{position:fixed;left:120px;top:110px;padding:24px 65px;background:#eef4ff;color:#123b75;border:3px solid #123b75;border-radius:8px;font-size:30px}h1{position:fixed;top:15px;left:120px;font-size:34px}</style><main><h1>Public document catalog</h1>'+Array.from({length:140},(_,i)=>'<button>Unrelated option '+i+'</button>').join('')+'<a id="target" href="/guide">Field Guide</a></main>');
        });
        await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve)});
        const start='http://127.0.0.1:'+server.address().port+'/catalog';
        try{
          await evalJs('window.browsphereApp.newTab('+JSON.stringify(start)+')');
          let before;
          for(let i=0;i<40;i++){before=await nativeState().catch(()=>null);if(before?.observed===start&&before.controls>=120)break;await sleep(300);}
          const missing=await evalJs("(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return !s?.interactive_nodes?.some(n=>n.label==='Field Guide')})()");
          if(!missing||before?.observed!==start||before.controls!==120)throw Error('Target must be omitted from the initial bounded DOM observation');
          const task=await runTask('On this public document catalog page, click the Field Guide link. Then report its page title. Do not navigate to unrelated pages.',start,125);
          const after=await nativeState().catch(()=>null);
          const checks={initialTargetOmitted:missing,independentModelProposal:task.visionDetails.some(e=>e.independentProposed),nativeHitGrounding:task.visionDetails.some(e=>e.independentGrounded>0),coordinateDispatch:task.actions.some(a=>a.modelRoute?.includes('vision:coordinate_click')&&['ok','confirmed'].includes(a.outcome)),correctPage:after?.observed===start.replace('/catalog','/guide'),verifiedTask:task.status==='completed'&&task.verified,noApproval:!task.approvalRequired,readOnly:requests.every(r=>r.method==='GET')};
          const pass=Object.values(checks).every(Boolean);
          console.log('VISION_INDEPENDENT_RESULT',JSON.stringify({pass,checks,task,before,after,requests}));if(!pass)process.exitCode=1;return;
        }finally{await new Promise(r=>server.close(r));}
      }
      if(key==='vision-fixture'){
        const http=require('node:http');
        const requests=[];
        const server=http.createServer((req,res)=>{
          requests.push({method:req.method,path:req.url});
          if(req.method!=='GET'||!['/catalog','/guide','/help','/favicon.ico'].includes(req.url)){res.writeHead(403);res.end();return;}
          if(req.url==='/favicon.ico'){res.writeHead(404);res.end();return;}
          res.setHeader('Content-Type','text/html; charset=utf-8');
          const style='<style>body{font:22px Arial;padding:35px;color:#151b2e;background:#fff}a{display:inline-block;margin:15px;padding:18px 30px;border:2px solid #234c91;border-radius:8px;color:#102d60}h1{font-size:36px}</style>';
          if(req.url==='/catalog'){res.end('<!doctype html><title>Public library</title>'+style+'<h1>Public document catalog</h1><div>Read a public document about the garden or browse general help.</div><a href="/guide">Field Guide</a><a href="/help">Help</a>');return;}
          if(req.url==='/guide'){res.end('<!doctype html><title>Field Guide</title>'+style+'<main><h1>Field Guide</h1><p>A garden grows with sunlight, water and healthy soil.</p><p>The plants use sunlight to produce energy and the soil supplies essential nutrients.</p></main>');return;}
          res.end('<!doctype html><title>Help</title>'+style+'<h1>Help</h1><div>General catalog information, not the garden field guide.</div><a href="/catalog">Public catalog</a>');
        });
        await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve)});
        const start='http://127.0.0.1:'+server.address().port+'/catalog';
        try{
          await evalJs('window.browsphereApp.newTab('+JSON.stringify(start)+')');
          let before;
          for(let i=0;i<40;i++){before=await nativeState().catch(()=>null);if(before?.observed===start&&before.controls>=2)break;await sleep(300);}
          if(before?.observed!==start||before.controls<2)throw Error('Visual fixture did not load');
          const task=await runTask('Find the Field Guide in this public document catalog and report what a garden needs to grow. Cite the observed document.',start,125);
          const details=task.visionDetails||[];
          const observations=details.filter(e=>e.status==='observed'&&e.fusion);
          const after=await nativeState().catch(()=>null);
          // Validate actual structured inference and safe fusion independently
          // of the language model's final factual-task result.
          const checks={structuredObservation:observations.length>0,groundedControl:observations.some(e=>e.fusion.matched_controls>0),
            budget:details.filter(e=>e.status==='analyzing').length<=3,
            noApproval:!task.approvalRequired,readOnly:requests.every(r=>r.method==='GET'),
            correctPage:after?.observed===start.replace('/catalog','/guide'),
            verifiedTask:task.status==='completed'&&task.verified,
            fact:(task.report?.findings||[]).some(f=>f.url===start.replace('/catalog','/guide')&&/sunlight/i.test(f.quote||'')&&/water/i.test(f.quote||''))};
          const pass=Object.values(checks).every(Boolean);
          console.log('VISION_RESULT',JSON.stringify({pass,checks,task,before,after,requests}));
          if(!pass)process.exitCode=1;return;
        }finally{await new Promise(r=>server.close(r));}
      }
      if(!['delayed-link','missing-link'].includes(key))throw Error('Unknown Phase 3 case: '+key);
      // Local fault fixture. Expected target is known by label only. Never
      // ask the agent to submit, authenticate, make a purchase, or write data.
      const http=require('node:http');
      const audit=[];
      const delayed=key==='delayed-link';
      const server=http.createServer((req,res)=>{
        audit.push({method:req.method,path:req.url});
        if(req.method!=='GET'||!['/start','/guide','/other','/favicon.ico'].includes(req.url)){res.writeHead(403);res.end();return;}
        res.setHeader('content-type','text/html; charset=utf-8');
        if(req.url==='/guide'){res.end('<!doctype html><title>Field Guide</title><main><h1>Field Guide</h1><p>The public field guide describes how a garden grows with sunlight, soil and water.</p></main>');return;}
        if(req.url==='/other'){res.end('<!doctype html><title>Unrelated Index</title><main><h1>Unrelated Index</h1><p>This is not the requested guide.</p></main>');return;}
        if(req.url==='/favicon.ico'){res.writeHead(404);res.end();return;}
        res.end('<!doctype html><title>Public document library</title><main><h1>Public document library</h1><p>Read-only test catalog of public educational documents. The requested item may take time to load.</p><a href="/other">Unrelated Index</a><div id="list"></div></main>'+(delayed?'<script>setTimeout(()=>{const a=document.createElement("a");a.href="/guide";a.textContent="Field Guide";document.querySelector("#list").append(a)},4500)</script>':''));
      });
      await new Promise((ok,no)=>{server.once('error',no);server.listen(0,'127.0.0.1',ok)});
      const start='http://127.0.0.1:'+server.address().port+'/start';
      try{
        await evalJs('window.browsphereApp.newTab('+JSON.stringify(start)+')');
        let before;
        for(let i=0;i<35;i++){before=await nativeState().catch(()=>null);if(before?.observed===start&&before.controls>0)break;await sleep(300);}
        if(before?.observed!==start||!before.controls)throw Error('Local fault fixture did not load');
        const initial=await evalJs("(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return s.interactive_nodes.map(n=>n.label)})()");
        const task=await runTask('On this public document library page, click the Field Guide link. Then report its page title. Do not navigate to unrelated pages.',start,delayed?60:40);
        const after=await nativeState().catch(()=>null);
        const checks={noApproval:!task.approvalRequired,noUnexpectedWrite:audit.every(a=>a.method==='GET'),noDecoy:!audit.some(a=>a.path==='/other'),initiallyMissing:!initial.includes('Field Guide')};
        if(delayed){checks.terminal=task.status==='completed';checks.verified=task.verified;checks.destination=after?.observed===start.replace('/start','/guide');checks.actualRead=audit.some(a=>a.path==='/guide');}
        else{checks.safeStop=['failed','cancelled','unverified'].includes(task.status);checks.noFalseVerification=!task.verified;checks.noInventedDestination=!audit.some(a=>a.path==='/guide');checks.stayed=after?.observed===start;}
        const pass=Object.values(checks).every(Boolean);
        console.log('PHASE3_RESULT',JSON.stringify({key,tier:'failure',skill:'recovery',pass,checks,before,after,task,audit}));
        if(!pass)process.exitCode=1;return;
      }finally{await new Promise(r=>server.close(r));}
    }
    if(process.env.LUCIA_LEVEL_TEST){
      if(useCloud||!hasQwen)throw Error('Complex suite requires installed local models');
      const level=Number(process.env.LUCIA_LEVEL_TEST);
      const requests='https://requests.readthedocs.io/en/latest/user/quickstart/';
      const httpx='https://www.python-httpx.org/async/';
      const aiohttp='https://docs.aiohttp.org/en/stable/client_quickstart.html';
      const timeouts='https://www.python-httpx.org/advanced/timeouts/';
      const compatibility='https://www.python-httpx.org/compatibility/';
      const cases={
        2:{start:requests,goal:'Research and compare three HTTP client libraries using their official documentation: '+requests+' '+httpx+' '+aiohttp+' . Explain a useful feature and one limitation or usage caveat for each library. Cite both properties from each of the three sources.',verify:(findings)=>{
          const has=(host,re)=>findings.some(f=>hostname(f.url)===host&&re.test(f.quote||'')&&re.test(f.claim||''));
          return {requestsFeature:has('requests.readthedocs.io',/requests\.(get|post)|response object|json response|response content/i),requestsCaveat:has('requests.readthedocs.io',/timeout|raise_for_status|exception/i),httpxFeature:has('www.python-httpx.org',/asyncclient|async request|async support/i),httpxCaveat:has('www.python-httpx.org',/hot loop|close|context manager|streaming response/i),aiohttpFeature:has('docs.aiohttp.org',/clientsession|session\.get|http request/i),aiohttpCaveat:has('docs.aiohttp.org',/session per request|session per application|connection pool|close|reuse/i)};
        }},
        3:{start:'https://github.com/',goal:'Find the official psf Requests repository on GitHub. Open its README or installation documentation. Report the installation command and the supported Python version, using exact source-backed evidence from the repository or its linked official documentation.',verify:(findings,task,after)=>{
          const official=f=>hostname(f.url)==='github.com'&&new URL(f.url).pathname.startsWith('/psf/requests')||hostname(f.url)==='requests.readthedocs.io';
          return {repositoryObserved:after.observed?.startsWith('https://github.com/psf/requests')||findings.some(f=>hostname(f.url)==='github.com'&&new URL(f.url).pathname.startsWith('/psf/requests')),installation:findings.some(f=>official(f)&&/pip install requests/i.test(f.quote||'')&&/pip install requests/i.test(f.claim||'')),pythonVersion:findings.some(f=>official(f)&&/python\s*3\.\d+|requires-python/i.test(f.quote||'')&&/python\s*3\.\d+|requires-python/i.test(f.claim||''))};
        }},
        4:{start:'https://www.croma.com/',goal:'Research the Apple MacBook Air 13-inch with M4 chip, 16 GB memory and 256 GB SSD on Croma and Reliance Digital. Find matching public product listings, compare their listed Indian rupee prices and confirm that the chip, memory and storage match. Cite both product pages and their observed price evidence. If unavailable, report the limitation. Do not perform any transaction.',verify:(findings)=>{
          const forSite=host=>findings.filter(f=>hostname(f.url)===host&&new URL(f.url).pathname!=='/');const checks={};
          for(const [name,host] of [['croma','www.croma.com'],['reliance','www.reliancedigital.in']]){
            const items=forSite(host);const text=items.map(f=>f.quote).join(' ');
            checks[name+'Specs']=/macbook air/i.test(text)&&/\bM4\b/i.test(text)&&/16\s*GB/i.test(text)&&/256\s*GB/i.test(text);
            checks[name+'Price']=items.some(f=>/(\u20b9|Rs\.?|INR)\s*[\d,]{4,}/i.test(f.quote||'')&&/(\u20b9|Rs\.?|INR)\s*[\d,]{4,}/i.test(f.claim||''));
          }return checks;
        }},
        7:{start:requests,goal:'Research and reconcile these apparently conflicting claims about default HTTP request timeouts: some requests wait indefinitely, while others time out after five seconds. Compare the official sources '+requests+' '+timeouts+' '+compatibility+' . Explain which library each claim applies to, why the two statements are compatible, and cite evidence from all three pages.',verify:(findings,task)=>{
          const from=url=>findings.filter(f=>f.url===url);
          const a=from(requests),b=from(timeouts),c=from(compatibility);
          return {requestsDefault:a.some(f=>/no timeout|do not specify a timeout|no timeout is set/i.test(f.quote||'')),httpxDefault:b.some(f=>/5 seconds|five seconds/i.test(f.quote||'')),compatibility:c.some(f=>/timeout/i.test(f.quote||'')),reconciled:!!task.report?.findings?.some(f=>/requests.*httpx|httpx.*requests/i.test(f.claim||'')&&/timeout/i.test(f.claim||''))||/different librar|different default/i.test(task.report?.summary||'')};
        }},
        8:{start:'https://www.postgresql.org/docs/',goal:'Starting from the PostgreSQL documentation index, find the SQL INSERT command reference. Locate the ON CONFLICT clause and explain the difference between DO NOTHING and DO UPDATE. Cite observed source evidence for each behavior.',verify:(findings,task,after)=>{
          const official=f=>hostname(f.url)==='www.postgresql.org'&&/\/sql-insert\.html$/.test(new URL(f.url).pathname);
          return {correctReference:findings.some(official),doNothing:findings.some(f=>official(f)&&/do nothing/i.test(f.quote||'')&&/avoid|skip|not insert|not inserted/i.test(f.quote||'')),doUpdate:findings.some(f=>official(f)&&/do update/i.test(f.quote||'')&&/update/i.test(f.quote||'')),multiStep:task.actions.filter(a=>['ok','confirmed'].includes(a.outcome)&&a.type!=='complete').length>=2};
        }}
      };
      const c=cases[level];if(!c)throw Error('Unsupported level');
      await evalJs('window.browsphereApp.newTab('+JSON.stringify(c.start)+')');
      let before;
      for(let i=0;i<40;i++){before=await nativeState().catch(()=>null);if(before?.controls&&hostname(before.observed)===hostname(c.start))break;await sleep(400);}
      if(!before?.controls||hostname(before.observed)!==hostname(c.start)||/access denied|not found|privacy error/i.test(before.title||'')){
        console.log('LEVEL_RESULT',JSON.stringify({level,pass:false,blocked:true,before,reason:'Starting website unavailable'}));process.exitCode=1;return;
      }
      // Public catalog diagnostics from the isolated native browser only.
      // Do not log arbitrary input values or account/session data.
      // Opt-in inspection of public rendered controls in the disposable WebView.
      // Never access input values, cookies, storage, hidden records or APIs.
      async function rawCatalogProbe(){
        const pages=await targets();
        const page=pages.find(t=>t.type==='page'&&['www.croma.com','www.reliancedigital.in'].includes(hostname(t.url)));
        if(!page)return null;
        const ws=new WebSocket(page.webSocketDebuggerUrl);
        try{
          await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no;});
          const fn=()=>{
            const visible=e=>{const r=e.getBoundingClientRect(),s=getComputedStyle(e);return r.width>1&&r.height>1&&s.visibility==='visible'&&s.display!=='none'&&s.opacity!=='0'&&!e.closest('[aria-hidden=true],[hidden]')};
            const text=e=>(e.innerText||'').replace(/\s+/g,' ').trim().slice(0,170);
            const fields=Array.from(document.querySelectorAll('input,textarea,[contenteditable=true],[role=searchbox],[role=combobox]')).filter(visible).slice(0,25).map(e=>({tag:e.tagName,role:e.getAttribute('role'),type:e.getAttribute('type'),name:e.getAttribute('name'),id:e.id,placeholder:e.getAttribute('placeholder'),aria:e.getAttribute('aria-label'),parentRole:e.parentElement?.getAttribute('role'),parentLabel:e.closest('[role=combobox],[role=search]')?.getAttribute('aria-label'),readonly:!!e.readOnly,disabled:!!e.disabled}));
            const products=Array.from(document.querySelectorAll('a[href]')).filter(e=>visible(e)&&/macbook/i.test(e.href+' '+text(e))).slice(0,20).map(e=>({url:e.origin+e.pathname,label:text(e),alt:e.querySelector('img')?.alt}));
            const names=Array.from(document.querySelectorAll('h1,h2,h3,h4,p,span,div')).filter(e=>e.childElementCount<3&&visible(e)&&/macbook air/i.test(text(e))).slice(0,15).map(e=>({tag:e.tagName,label:text(e),className:String(e.className||'').slice(0,120),ancestor:e.closest('a[href]')?.pathname||null}));
            const txt=(document.body?.innerText||'').replace(/\s+/g,' ');
            return {url:location.origin+location.pathname,title:document.title,fields,products,names,resultHints:txt.match(/.{0,40}(?:no results|no products|products found|results for|search results|showing \d+).{0,80}/gi)?.slice(0,5)||[],bodyLength:txt.length};
          };
          const r=await Promise.race([new Promise((ok,no)=>{ws.onmessage=e=>{let m=JSON.parse(e.data);if(m.id===1)ok(m.result?.result?.value||null);};ws.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{expression:'('+fn.toString()+')()',returnByValue:true}}));}),sleep(2500).then(()=>null)]);
          return r;
        }finally{ws.close();}
      }
      const probes=[];
      let observing=true;
      const probe=(async()=>{
        while(observing&&level===4){
          const snap=await evalJs("(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return {url:s?.origin,title:s?.title,text:(s?.text_excerpt||'').slice(0,9000),nodes:(s?.interactive_nodes||[]).filter(n=>!n.is_sensitive&&(/search|macbook|apple|laptop/i.test(n.label||'')||/macbook|apple/i.test(n.href||''))).slice(0,45).map(n=>({label:n.label,tag:n.tag,role:n.role,href:n.href,search:n.search_state,submit:n.is_submit,viewport:n.bbox?.in_viewport})),structure:s?.structure}})()").catch(()=>null);
          if(snap?.url){
            if(process.env.LUCIA_CATALOG_RAW_PROBE==='1')snap.raw=await rawCatalogProbe().catch(()=>null);
            if(!probes.length||JSON.stringify(probes.at(-1))!==JSON.stringify(snap))probes.push(snap);
          }
          await sleep(900);
        }
      })();
      // Two bounded retailer windows (52 seconds each) plus authorized
      // MCP discovery and final verification need a larger outer deadline.
      // Keep the same success checks; a partial report is not a pass.
      const task=await runTask(c.goal,before.observed,level===4?180:100);
      observing=false;await probe;
      if(level===4)console.log('CATALOG_PROBES',JSON.stringify(probes.slice(0,28)));
      const after=await nativeState().catch(()=>null);
      const findings=task.report?.findings||[];
      const checks=c.verify(findings,task,after);
      const pass=task.status==='completed'&&task.verified&&!task.approvalRequired&&Object.values(checks).every(Boolean);
      console.log('LEVEL_RESULT',JSON.stringify({level,pass,checks,before,after,task}));
      if(!pass)process.exitCode=1;
      return;
    }
    if(process.env.LUCIA_COMPLEX_TEST==='1'){
      if(useCloud||!hasQwen)throw Error('Complex suite requires installed local models');
      const http=require('node:http'),fs=require('node:fs'),path=require('node:path');
      const mode=process.env.LUCIA_COMPLEX_CASE||'all';
      const wanted=name=>mode==='all'||mode===name;
      async function startAt(url){
        await evalJs('window.browsphereApp.newTab('+JSON.stringify(url)+')');
        for(let i=0;i<45;i++){const before=await nativeState();if(before?.observed===url&&before.controls>0&&!/access denied|not found|privacy error/i.test(before.title))return before;await sleep(400);}
        throw Error('Public starting page unavailable: '+url);
      }
      if(wanted('research')){
        const py='https://docs.python.org/3/tutorial/datastructures.html';
        const js='https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/map';
        try{
          await startAt(py);
          const goal='Research and compare Python list comprehensions and JavaScript Array.map using the official documentation pages '+py+' and '+js+' . Read both sources. Explain what each produces and one limitation or distinction of each. Cite exact supporting excerpts from both pages.';
          const task=await runTask(goal,py,150);
          const findings=task.report?.findings||[];
          const python=findings.some(f=>hostname(f.url)==='docs.python.org'&&/list comprehens/i.test(f.quote)&&/list/i.test(f.claim)&&/list/i.test(f.quote));
          const javascript=findings.some(f=>hostname(f.url)==='developer.mozilla.org'&&/new array/i.test(f.quote)&&/array/i.test(f.claim));
          const distinct=findings.some(f=>hostname(f.url)==='docs.python.org'&&/condition|filter|if clause|for clause/i.test(f.quote))&&findings.some(f=>hostname(f.url)==='developer.mozilla.org'&&/callback|sparse|side.effect|return/i.test(f.quote));
          const pass=task.status==='completed'&&task.verified&&!task.approvalRequired&&python&&javascript&&distinct;
          console.log('COMPLEX_RESEARCH',JSON.stringify({pass,python,javascript,distinct,task}));
          record('Cross-site technical comparison with two sourced distinctions',pass,{status:task.status,python,javascript,distinct,verified:task.verified});
        }catch(e){record('Cross-site technical comparison',false,{error:String(e)});}
      }
      if(mode==='interleave'||mode==='specialist'||mode==='specialist-dynamic'||mode==='specialist-adaptive'||mode==='specialist-link-adaptive'||mode==='entry-approved'||mode==='entry-rejected'){
        const root=path.join(process.env.LOCALAPPDATA,'BrowsphereAI');
        if(!process.env.LOCALAPPDATA.includes('browsphere-cross-site-'))throw Error('Refusing non-isolated MCP configuration');
        fs.mkdirSync(root,{recursive:true});
        const configPath=path.join(root,'mcp-servers.json');
        const audit=[];
        const entry=mode==='entry-approved'||mode==='entry-rejected',rejected=mode==='entry-rejected';
        const linkAdaptive=mode==='specialist-link-adaptive',adaptive=mode==='specialist-adaptive'||linkAdaptive,dynamic=mode==='specialist-dynamic',specialist=mode==='specialist'||dynamic||adaptive||entry;
        const excerpt=specialist?'Python is an easy to learn, powerful programming language':'This domain is for use in documentation examples';
        const lookupUrl='https://docs.python.org/3/tutorial/index.html';
        const server=http.createServer(async(req,res)=>{
          try{
            if(req.method==='DELETE'){res.writeHead(204);res.end();return;}
            if(req.method!=='POST'||req.url!=='/mcp'){res.writeHead(404);res.end();return;}
            let raw='';for await(const chunk of req){raw+=chunk;if(raw.length>32768)throw Error('oversized request');}
            const r=JSON.parse(raw);
            audit.push({method:r.method,name:r.params?.name,args:r.method==='tools/call'?r.params?.arguments:undefined});
            let result;
            if(r.method==='initialize')result={protocolVersion:'2025-11-25',capabilities:{tools:{}},serverInfo:{name:'observed-excerpt-fixture',version:'1'}};
            else if(r.method==='notifications/initialized'){res.writeHead(202);res.end();return;}
            else if(r.method==='tools/list')result={tools:[{name:'verify_excerpt',annotations:{readOnlyHint:true,destructiveHint:false},inputSchema:{type:'object',properties:{text:{type:'string',enum:linkAdaptive?[excerpt,lookupUrl]:[excerpt]}},required:['text'],additionalProperties:false}}]};
            else if(r.method==='tools/call'&&r.params?.name==='verify_excerpt'&&(r.params?.arguments?.text===excerpt||(linkAdaptive&&r.params?.arguments?.text===lookupUrl)))result={structuredContent:{value:'confirmed'},content:[{type:'text',text:'confirmed'}],isError:false};
            else throw Error('unexpected tool invocation');
            res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify({jsonrpc:'2.0',id:r.id,result}));
          }catch(e){res.writeHead(500);res.end('fixture rejected request');}
        });
        try{
          await new Promise((ok,no)=>{server.once('error',no);server.listen(0,'127.0.0.1',ok)});
          fs.writeFileSync(configPath,JSON.stringify({observed:{url:'http://127.0.0.1:'+server.address().port+'/mcp',read_only_tools:['verify_excerpt']}}));
          const source=specialist?'https://docs.python.org/3/':'https://example.com/',start=source;
          const before=await startAt(start);
          const dest='https://docs.python.org/3/tutorial/index.html';
          const expectedTitle='The Python Tutorial \u2014 '+before.title;
          const initial=entry?'https://example.com/':start;
          if(entry)await startAt(initial);
          const steps=[
            {id:'observe',action:specialist?{type:'browser_task',task:'On this Python documentation page, click the Tutorial link. Then report the page title.',url:dest,title:expectedTitle,excerpt,...(entry?{entry_url:start}:{}),...(dynamic?{dynamic_tools:[{trigger_url:dest,trigger_excerpt:excerpt,servers:['observed'],task:'Use verify_excerpt with text exactly '+JSON.stringify(excerpt),expect:{pointer:'/structuredContent/value',equals:'confirmed'}}]}:{}),...(adaptive?{adaptive_lookup:{servers:['observed'],objective:linkAdaptive?'Find the Python Tutorial link and verify its public URL using verify_excerpt text. Report the result.':'Verify the exact public Python tutorial reference using the approved verify_excerpt lookup and report the result',...(linkAdaptive?{source_url:start}:{public_context:excerpt}),expect:{pointer:'/structuredContent/value',equals:'confirmed'}}}:{})}:{type:'observe',url:source,title:'Example Domain',excerpt}},
            {id:'tool',depends_on:['observe'],action:{type:'call',server:'observed',tool:'verify_excerpt',arguments:{text:{step:'observe',pointer:'/structuredContent/excerpt'}},expect:{pointer:'/structuredContent/value',equals:'confirmed'}}},
            ...(!specialist?[{id:'browse',depends_on:['observe','tool'],action:{type:'browser',task:'Find more information about example domains using the link on this page. Report the title of the page you reach.'}}]:[])
          ];
          const task=await runTask('mcp workflow '+JSON.stringify({steps}),initial,150,entry?{url:start,approved:!rejected}:null);
          const after=await nativeState();
          const calls=audit.filter(a=>a.method==='tools/call');
          const graph=task.graph?.nodes||[];
          const receipts=graph.filter(n=>n.state==='completed').map(n=>n.receipt);
          const checks=rejected?{terminal:task.status==='failed',verified:!task.verified,noApproval:!task.approvalRequired,
            exactApproval:JSON.stringify(task.approvalReceipts)===JSON.stringify([{url:start,approved:false}]),
            noChildDispatch:task.workflowBrowser.length===0&&calls.length===0&&task.actions.length===0,
            blockedGraph:graph.length===2&&graph[0].state==='failed'&&graph[1].state==='blocked',
            unchangedSource:after?.observed===initial
          }:specialist?{terminal:task.status==='completed',verified:task.verified===true,noApproval:!task.approvalRequired,
            nativeSpecialistReceipt:JSON.stringify(task.workflowBrowser)===JSON.stringify(entry?['entry_verified','specialist_started','specialist_verified']:['specialist_started','specialist_verified']),
            ...(entry?{exactApproval:JSON.stringify(task.approvalReceipts)===JSON.stringify([{url:start,approved:true}])}:{}),
            exactExcerptTransferred:calls.length===((dynamic||adaptive)?2:1)&&calls.every((c,i)=>c.name==='verify_excerpt'&&c.args?.text===(linkAdaptive&&i===0?lookupUrl:excerpt)),
            ...((dynamic||adaptive)?{dynamicInsertion:task.dynamicTools.length===2&&task.dynamicTools[0].status===(adaptive?'adaptive_inserted':'inserted')&&task.dynamicTools[1].status===(adaptive?'adaptive_verified':'verified')&&task.dynamicTools[0].graph?.nodes?.some(n=>n.kind==='tool'&&n.state==='active')&&task.dynamicTools[1].graph?.nodes?.some(n=>n.kind==='tool'&&n.state==='completed'&&n.receipt==='tool_check')}:{ }),
            completeGraph:graph.length===2&&graph.every(n=>n.state==='completed'&&n.attempts===1)&&graph[0].receipt==='stage_gate'&&graph[1].receipt==='tool_check',
            exactDestination:after?.observed===dest&&after?.title===expectedTitle.slice(0,110),
            actionObserved:task.actions.some(a=>a.type==='click'&&['ok','confirmed'].includes(a.outcome)),
            finalReportOnce:task.verificationReports===1
          }:{terminal:task.status==='completed',verified:task.verified===true,noApproval:!task.approvalRequired,
            exactExcerptTransferred:calls.length===1&&calls[0].name==='verify_excerpt'&&calls[0].args?.text===excerpt,
            observedDependency:graph.some(n=>n.kind==='observation'&&n.state==='completed'&&n.receipt==='native_observation'),
            completeGraph:graph.length>=4&&graph.every(n=>n.state==='completed')&&['native_observation','tool_check','stage_gate','final_report'].every(r=>receipts.includes(r)),
            exactDestination:hostname(after?.observed)==='www.iana.org'&&new URL(after.observed).pathname.replace(/\/$/,'')==='/help/example-domains',
            coordinateDispatch:task.vision.includes('coordinate_click'),
            visualAction:task.actions.some(a=>a.type==='click'&&a.modelRoute?.includes('vision:coordinate_click')&&['ok','confirmed'].includes(a.outcome))};
          const pass=Object.values(checks).every(Boolean);
          console.log(entry?'CROSS_ORIGIN_BROWSER_MCP':linkAdaptive?'ADAPTIVE_LINK_MCP':adaptive?'ADAPTIVE_BROWSER_MCP':dynamic?'DYNAMIC_BROWSER_MCP':specialist?'RESUMABLE_BROWSER_SPECIALIST':'INTERLEAVED_VISION_MCP',JSON.stringify({pass,checks,task,after,audit}));
          record(specialist?'Verified browser specialist returns to MCP scheduler':'Native browser observation, exact evidence MCP transfer and visually grounded browser handoff',pass,{checks});
        }finally{await new Promise(r=>server.close(r));fs.rmSync(configPath,{force:true});}
      }
      if(mode==='alternating'){
        const root=path.join(process.env.LOCALAPPDATA,'BrowsphereAI');
        if(!process.env.LOCALAPPDATA.includes('browsphere-cross-site-'))throw Error('Refusing non-isolated MCP configuration');
        fs.mkdirSync(root,{recursive:true});
        const configPath=path.join(root,'mcp-servers.json');
        const start='https://doc.rust-lang.org/stable/book/index.html';
        const mid='https://doc.rust-lang.org/stable/book/ch01-00-getting-started.html';
        const dest='https://doc.rust-lang.org/stable/book/ch01-01-installation.html';
        const audit=[];
        async function oracle(url){
          await startAt(url);
          for(let attempt=0;attempt<3;attempt++){
            const s=await evalJs("Promise.race([window.__TAURI__.core.invoke('capture_live_snapshot'),new Promise(r=>setTimeout(()=>r(null),3500))])").catch(()=>null);
            if(s?.origin===url&&s.title&&s.interactive_nodes?.length)return s;
            await sleep(500);
          }
          throw Error('Independent page preflight failed');
        }
        function visibleLink(s,url){
          const matches=(s.interactive_nodes||[]).filter(n=>n.tag==='a'&&n.href===url&&n.bbox?.in_viewport&&!n.is_submit&&!n.is_sensitive);
          const n=matches.find(n=>n.label.trim()&&matches.filter(m=>m.label===n.label).length===1);
          if(!n)throw Error('Required unique visible link unavailable');
          return n.label.trim();
        }
        const destSnapshot=await oracle(dest);
        const midSnapshot=await oracle(mid);
        const startSnapshot=await oracle(start);
        const label1=visibleLink(startSnapshot,mid),label2=visibleLink(midSnapshot,dest);
        // Text sent by the tool is independently checked against the user's
        // approved destination and the native page; it is not a URL authority.
        const excerpt1='Getting Started',excerpt2='Installation';
        if(!midSnapshot.text_excerpt?.includes(excerpt1)||!destSnapshot.text_excerpt?.includes(excerpt2))throw Error('Expected public excerpts unavailable');
        const pairs=[['start',label1],[excerpt1,label2],[excerpt2,'done']];
        const server=http.createServer(async(req,res)=>{
          try{
            if(req.method==='DELETE'){res.writeHead(204);res.end();return;}
            if(req.method!=='POST'||req.url!=='/mcp'){res.writeHead(404);res.end();return;}
            let raw='';for await(const chunk of req){raw+=chunk;if(raw.length>32768)throw Error('oversized request');}
            const r=JSON.parse(raw);let result;
            if(r.method==='initialize')result={protocolVersion:'2025-11-25',capabilities:{tools:{}},serverInfo:{name:'alternating-fixture',version:'1'}};
            else if(r.method==='notifications/initialized'){res.writeHead(202);res.end();return;}
            else if(r.method==='tools/list')result={tools:[{name:'lookup',annotations:{readOnlyHint:true,destructiveHint:false},inputSchema:{type:'object',properties:{key:{type:'string',enum:pairs.map(p=>p[0])}},required:['key'],additionalProperties:false}}]};
            else if(r.method==='tools/call'&&r.params?.name==='lookup'){
              const pair=pairs.find(p=>p[0]===r.params?.arguments?.key);
              if(!pair)throw Error('unexpected tool argument');
              audit.push(pair[0]);
              result={structuredContent:{value:pair[1]},content:[{type:'text',text:pair[1]}],isError:false};
            }else throw Error('unexpected MCP method');
            res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify({jsonrpc:'2.0',id:r.id,result}));
          }catch(e){res.writeHead(500);res.end('fixture rejected request');}
        });
        try{
          await new Promise((ok,no)=>{server.once('error',no);server.listen(0,'127.0.0.1',ok)});
          fs.writeFileSync(configPath,JSON.stringify({fixture:{url:'http://127.0.0.1:'+server.address().port+'/mcp',read_only_tools:['lookup']}}));
          const ptr='/structuredContent/value',excerpt='/structuredContent/excerpt';
          const ref=(step,pointer)=>({step,pointer});
          const tool=(id,deps,key,equals)=>({id,depends_on:deps,action:{type:'call',server:'fixture',tool:'lookup',arguments:{key},expect:{pointer:ptr,equals}}});
          const steps=[
            tool('a',[],'start',label1),
            {id:'web1',depends_on:['a'],action:{type:'follow_link',source_url:start,label:ref('a',ptr),url:mid,title:midSnapshot.title,excerpt:excerpt1}},
            tool('b',['web1'],ref('web1',excerpt),label2),
            {id:'web2',depends_on:['b'],action:{type:'follow_link',source_url:mid,label:ref('b',ptr),url:dest,title:destSnapshot.title,excerpt:excerpt2}},
            tool('c',['web2'],ref('web2',excerpt),'done')
          ];
          const task=await runTask('mcp workflow '+JSON.stringify({steps}),start,100);
          const after=await nativeState();
          const nodes=task.graph?.nodes||[];
          const checks={terminal:task.status==='completed',verified:task.verified===true,noApproval:!task.approvalRequired,
            exactMcpOrder:JSON.stringify(audit)===JSON.stringify(pairs.map(p=>p[0])),
            alternatingGraph:nodes.length===5&&nodes.every((n,i)=>n.state==='completed'&&n.attempts===1&&n.kind===(i%2?'browser':'tool')&&n.receipt===(i%2?'stage_gate':'tool_check')),
            nativeLinkReceipts:JSON.stringify(task.workflowBrowser)===JSON.stringify(['dispatched','verified','dispatched','verified']),
            exactDestination:after?.observed===dest&&after?.title===destSnapshot.title.slice(0,110)};
          const pass=Object.values(checks).every(Boolean);
          console.log('ALTERNATING_BROWSER_MCP',JSON.stringify({pass,checks,task,after,audit}));
          record('MCP to native link to MCP to native link to MCP',pass,{checks});
        }finally{await new Promise(r=>server.close(r));fs.rmSync(configPath,{force:true});}
      }
      if(wanted('graph')||wanted('negative')||wanted('outage')||mode==='joint'){
        const root=path.join(process.env.LOCALAPPDATA,'BrowsphereAI');
        if(!process.env.LOCALAPPDATA.includes('browsphere-cross-site-'))throw Error('Refusing non-isolated MCP configuration');
        fs.mkdirSync(root,{recursive:true});
        const configPath=path.join(root,'mcp-servers.json');
        const audit=[];
        const schema=(name,key,value)=>({name,annotations:{readOnlyHint:true,destructiveHint:false},inputSchema:{type:'object',properties:{[key]:{type:'string',enum:[value]}},required:[key],additionalProperties:false}});
        function makeServer(lane){
          return http.createServer(async(req,res)=>{
            try{
              if(req.method==='DELETE'){res.writeHead(204);res.end();return;}
              if(req.method!=='POST'||req.url!=='/mcp'){res.writeHead(404);res.end();return;}
              let raw='';for await(const c of req){raw+=c;if(raw.length>32768)throw Error('request too large');}
              const r=JSON.parse(raw);let value;
              audit.push({lane,method:r.method,name:r.params?.name,args:r.method==='tools/call'?r.params?.arguments:undefined});
              if(lane==='empty'&&mode==='outage'){res.writeHead(503);res.end('Service temporarily unavailable');return;}
              if(r.method==='initialize')value={protocolVersion:'2025-11-25',capabilities:{tools:{}},serverInfo:{name:'complex-test-'+lane,version:'1'}};
              else if(r.method==='notifications/initialized'){res.writeHead(202);res.end();return;}
              else if(r.method==='tools/list')value={tools:lane==='empty'?[]:lane==='github'?[schema('repository_metadata','repository','psf/requests')]:[schema('package_metadata','package','requests')]};
              else if(r.method==='tools/call'){
                const args=r.params.arguments;
                let target;
                if(lane==='github'&&r.params.name==='repository_metadata'&&args?.repository==='psf/requests')target='https://api.github.com/repos/psf/requests';
                else if(lane==='pypi'&&r.params.name==='package_metadata'&&args?.package==='requests')target='https://pypi.org/pypi/requests/json';
                else throw Error('unapproved tool call');
                const response=await fetch(target,{headers:{'User-Agent':'Browsphere-complex-readonly-test/1.0','Accept':'application/json'},signal:AbortSignal.timeout(15000),redirect:'error'});
                if(!response.ok)throw Error('Public API status '+response.status);
                const data=await response.json();
                const name=lane==='github'?data.name:data.info?.name;
                if(typeof name!=='string'||name.length>80)throw Error('Missing package name');
                audit.push({lane,publicHttpStatus:response.status,name});
                value={content:[{type:'text',text:name}],structuredContent:{value:name},isError:false};
              }else throw Error('unsupported method');
              res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify({jsonrpc:'2.0',id:r.id,result:value}));
            }catch(e){audit.push({lane,error:String(e)});res.writeHead(500);res.end('test adapter failed');}
          });
        }
        const servers={empty:makeServer('empty'),github:makeServer('github'),pypi:makeServer('pypi')};
        try{
          await Promise.all(Object.values(servers).map(s=>new Promise((ok,no)=>{s.once('error',no);s.listen(0,'127.0.0.1',ok)})));
          fs.writeFileSync(configPath,JSON.stringify(Object.fromEntries(Object.entries(servers).map(([k,s])=>[k,{url:'http://127.0.0.1:'+s.address().port+'/mcp',read_only_tools:k==='empty'?[]:[k==='github'?'repository_metadata':'package_metadata']}]))));
          const start='https://docs.python.org/3/';
          const target='https://pypi.org/project/requests/';
          const ptr='/structuredContent/value';
          const ref=step=>({step,pointer:ptr});
          const steps=wrong=>[
            {id:'repository',action:{type:'auto',servers:mode==='joint'?['pypi','github']:['empty','github'],...(mode==='joint'?{joint_selection:true}:{}),task:'Use repository_metadata for repository psf/requests to retrieve its package name.',expect:{pointer:ptr,equals:wrong?'deliberately-incorrect':'requests'}}},
            {id:'registry',depends_on:['repository'],action:{type:'call',server:'pypi',tool:'package_metadata',arguments:{package:ref('repository')},expect:{pointer:ptr,equals:'requests'}}},
            {id:'crosscheck',depends_on:['repository','registry'],action:{type:'compare',left:ref('repository'),right:ref('registry')}},
            {id:'web',depends_on:['repository','registry','crosscheck'],action:{type:'browser',task:'Open '+target}}
          ];
          for(const negative of [false,true]){
            if(negative?!wanted('negative'):!(wanted('graph')||wanted('outage')||mode==='joint'))continue;
            const offset=audit.length;
            try{
              await startAt(start);
              const task=await runTask('mcp workflow '+JSON.stringify({steps:steps(negative)}),start,120);
              const calls=audit.slice(offset);
              const after=await nativeState();
              const github=calls.some(a=>a.lane==='github'&&a.publicHttpStatus===200&&a.name==='requests');
              const pypi=calls.some(a=>a.lane==='pypi'&&a.publicHttpStatus===200&&a.name==='requests');
              const transferred=calls.some(a=>a.lane==='pypi'&&a.method==='tools/call'&&a.args?.package==='requests');
              const emptyNoDispatch=!calls.some(a=>a.lane==='empty'&&a.method==='tools/call');
              const emptyDiscovered=calls.some(a=>a.lane==='empty'&&a.method==='tools/list');
              const firstDispatch=calls.findIndex(a=>a.method==='tools/call');
              const bothDiscoveredBeforeDispatch=firstDispatch>0&&['github','pypi'].every(l=>calls.slice(0,firstDispatch).some(a=>a.lane===l&&a.method==='tools/list'));
              const dispatched=calls.filter(a=>a.method==='tools/call').map(a=>a.lane);
              const jointSelectionVerified=bothDiscoveredBeforeDispatch&&JSON.stringify(dispatched)===JSON.stringify(['github','pypi']);
              const graph=task.graph?.nodes||[];
              const receipts=graph.filter(n=>n.state==='completed').map(n=>n.receipt);
              const fullGraph=graph.length>=5&&graph.every(n=>n.state==='completed')&&receipts.includes('comparison_check')&&receipts.includes('final_report');
              const blocked=graph.some(n=>n.state==='failed')&&graph.some(n=>n.state==='blocked');
              const pass=negative?github&&emptyNoDispatch&&!pypi&&!calls.some(a=>a.lane==='pypi'&&a.method==='tools/call')&&!task.verified&&task.status!=='completed'&&blocked&&after.observed===start&&!task.approvalRequired:
                github&&pypi&&transferred&&emptyNoDispatch&&(mode==='joint'?jointSelectionVerified:mode==='outage'?calls.some(a=>a.lane==='empty'&&a.method==='initialize'):emptyDiscovered)&&fullGraph&&after.observed===target&&task.status==='completed'&&task.verified&&!task.approvalRequired;
              console.log('COMPLEX_GRAPH',JSON.stringify({negative,pass,github,pypi,transferred,emptyNoDispatch,emptyDiscovered,fullGraph,blocked,after,task,audit:calls,...(mode==='joint'?{checks:{jointSelectionVerified,bothDiscoveredBeforeDispatch}}:{})}));
              record(negative?'Failed evidence blocks dependent external call':'Four-step MCP dependency chain and native browser verification',pass,{status:task.status,github,pypi,transferred,fullGraph,blocked,verified:task.verified});
            }catch(e){record(negative?'Dependency stop':'Hybrid DAG',false,{error:String(e),audit:audit.slice(offset)});}
          }
        }finally{await Promise.all(Object.values(servers).map(s=>new Promise(r=>s.close(r))));fs.rmSync(configPath,{force:true});}
      }
      console.log('COMPLEX_RESULT',JSON.stringify({passed:result.filter(x=>x.success).length,total:result.length,results:result}));
      if(!result.length||result.some(x=>!x.success))process.exitCode=1;
      return;
    }
    if(process.env.LUCIA_RESEARCH_CAPTURE_TEST==='1'){
      const urls=['https://en.wikipedia.org/wiki/Neptune','https://en.wiktionary.org/wiki/serendipity'];
      for(const url of urls){
        await evalJs('window.browsphereApp.newTab('+JSON.stringify(url)+')');
        let snap;
        for(let i=0;i<45;i++){
          snap=await evalJs("window.__TAURI__.core.invoke('capture_live_snapshot')");
          if(snap?.origin===url&&snap.text_excerpt?.length>500)break;
          await sleep(500);
        }
        console.log('PUBLIC_CAPTURE',JSON.stringify({url:snap?.origin,title:snap?.title,excerpt:snap?.text_excerpt?.slice(0,4800)}));
      }
      return;
    }
    if (process.env.LUCIA_FACT_RESEARCH_TEST === '1') {
      if(useCloud||!hasQwen)throw Error('Research test requires local model configuration');
      const dictionary=process.env.LUCIA_FACT_SUBJECT==='dictionary';
      const url=dictionary?'https://en.wiktionary.org/wiki/Wiktionary:Main_Page':'https://en.wikipedia.org/wiki/Main_Page';
      const finalUrl=dictionary?'https://en.wiktionary.org/wiki/serendipity':'https://en.wikipedia.org/wiki/Neptune';
      await evalJs('window.browsphereApp.newTab('+JSON.stringify(url)+')');
      let before;
      for(let i=0;i<45;i++){
        before=await nativeState();
        if(hostname(before?.observed)===hostname(url)&&before.controls>0)break;
        await sleep(400);
      }
      if(hostname(before?.observed)!==hostname(url)||!before.controls)throw Error('Research website unavailable');
      const goal=dictionary?'On this website, search for the word serendipity and open its dictionary entry. Report its English meaning.':'On this website, search for the planet Neptune and open the corresponding article. Report its position from the Sun.';
      const task=await runTask(goal,url,120);
      const after=await nativeState();
      const report=await evalJs("window.__crossSiteEvents.filter(e=>e.kind==='agent:verification_report').at(-1)?.payload?.report||null");
      const matchAnswer=s=>dictionary?(/discover|event|find|experience/i.test(s)&&/fortunate|happy|beneficial|luck/i.test(s)&&/chance|accident|unexpected|unplanned/i.test(s)):(/eighth|8th/i.test(s)&&/sun/i.test(s));
      const evidence=report?.findings?.some(f=>f.url===finalUrl&&matchAnswer(f.claim)&&matchAnswer(f.quote));
      const pass=task.status==='completed'&&task.verified&&after.observed===finalUrl&&!!evidence&&!task.approvalRequired;
      console.log('FACT_RESEARCH_RESULT',JSON.stringify({pass,task,after,report,evidence:!!evidence}));
      if(!pass)process.exitCode=1;
      return;
    }
    if (process.env.LUCIA_FILTER_TEST === '1') {
      if (useCloud || !hasQwen) throw Error('Filter test requires local model configuration');
      const http=require('node:http');
      let requests=0,unexpected=0;
      const server=http.createServer((req,res)=>{
        requests++;
        if(req.method!=='GET'||!['/','/favicon.ico'].includes(req.url)){unexpected++;res.writeHead(403);res.end();return;}
        res.setHeader('content-type','text/html; charset=utf-8');
        res.end('<!doctype html><title>Public catalog filters</title><h1>Public catalog filters</h1><p>Choose a category, format and language to filter the sample catalog. This fixture only changes local page state.</p><label for="cat">Category</label><select id="cat"><option>All</option><option>Books</option><option>Maps</option></select><label for="fmt">Format</label><select id="fmt"><option>PDF</option><option>Text</option></select><label for="lang">Language</label><select id="lang"><option>Any</option><option>English</option><option>Hindi</option></select><p id="result" aria-live="polite">Catalog results: All / PDF / Any</p><script>function update(){document.getElementById("result").textContent="Catalog results: "+["cat","fmt","lang"].map(id=>document.getElementById(id).value).join(" / ")}document.querySelectorAll("select").forEach(el=>el.addEventListener("change",update));</script>');
      });
      await new Promise((ok,no)=>{server.once('error',no);server.listen(0,'127.0.0.1',ok)});
      const url='http://127.0.0.1:'+server.address().port+'/';
      try {
        await evalJs('window.browsphereApp.newTab('+JSON.stringify(url)+')');
        let before;
        for(let i=0;i<45;i++){
          before=await nativeState();
          if(before?.observed===url&&before.controls>=3)break;
          await sleep(400);
        }
        if(before?.observed!==url||before.controls<3)throw Error('Fixture unavailable');
        const goal='Filter results only. Set "Category" to "Books". Set "Format" to "Text". Set "Language" to "English". Report the selected filters.';
        const task=await runTask(goal,url,90);
        const observed=await evalJs("(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return {url:s?.origin,excerpt:s?.text_excerpt,fields:s?.interactive_nodes?.filter(n=>n.tag==='select').map(n=>({label:n.label,state:n.search_state}))}})()");
        const expected={Category:'selected_option:Books',Format:'selected_option:Text',Language:'selected_option:English'};
        const values=observed?.fields?.length===3&&observed.fields.every(n=>expected[n.label]===n.state);
        const resultObserved=observed?.excerpt?.includes('Catalog results: Books / Text / English');
        const successful=task.actions.filter(a=>a.type==='select_option'&&['ok','confirmed'].includes(a.outcome));
        const pass=task.status==='completed'&&task.verified&&!task.approvalRequired&&successful.length===3&&values&&resultObserved&&observed.url===url&&unexpected===0;
        console.log('FILTER_RESULT',JSON.stringify({pass,task,observed,values,resultObserved,requests,unexpected}));
        if(!pass)process.exitCode=1;
      }finally{await new Promise(r=>server.close(r));}
      return;
    }
    if (process.env.LUCIA_SEMANTIC_BROWSER_TEST === '1') {
      if(useCloud||!hasQwen)throw Error('Semantic browser validation requires local models');
      const cases=[
        {name:'Ordered Rust documentation navigation',url:'https://doc.rust-lang.org/stable/book/index.html',goal:'First open the Getting Started chapter, then locate the Installation section. Use public pages only.',path:'/stable/book/ch01-01-installation.html',constraint:'Use public pages only'},
        {name:'Ordered Python documentation navigation',url:'https://docs.python.org/3/',goal:'First open the Python tutorial, then locate its chapter on data structures. Do not download any files.',path:'/3/tutorial/datastructures.html',constraint:'Do not download any files'}
      ];
      for(const t of cases){
        try{
          await evalJs('window.browsphereApp.newTab('+JSON.stringify(t.url)+')');
          let before;
          for(let i=0;i<45;i++){
            before=await nativeState();
            if(hostname(before?.observed)===hostname(t.url)&&before.controls>0)break;
            await sleep(400);
          }
          if(hostname(before?.observed)!==hostname(t.url)||!before.controls||/access denied|not found|privacy error/i.test(before.title))throw Error('Starting public page unavailable');
          const task=await runTask(t.goal,t.url,200);
          const after=await nativeState();
          const concepts=task.plan?.semantic?.concepts||[],relations=task.plan?.semantic?.relations||[];
          const objectiveNodes=concepts.filter(c=>c.kind==='objective');
          const checks={
            semanticGraph:objectiveNodes.length>=2&&relations.some(r=>r.kind==='depends_on'),
            constraint:concepts.some(c=>c.kind==='constraint'&&c.span.includes(t.constraint)),
            grounded:concepts.every(c=>t.goal.includes(c.span)),
            exactDestination:hostname(after.observed)===hostname(t.url)&&new URL(after.observed).pathname===t.path,
            terminal:task.status==='completed'&&task.verified,
            noApproval:!task.approvalRequired,
            noDownload:!task.actions.some(a=>a.type==='download'),
            realActions:task.actions.filter(a=>a.type!=='complete'&&['ok','confirmed'].includes(a.outcome)).length>=2,
            graphComplete:!!task.graph?.nodes?.length&&task.graph.nodes.every(n=>n.state==='completed')
          };
          const passed=Object.values(checks).every(Boolean);
          console.log('SEMANTIC_BROWSER_DETAIL',JSON.stringify({name:t.name,passed,checks,after,task}));
          record(t.name,passed,checks);
        }catch(e){record(t.name,false,{error:String(e)});}
      }
      console.log('SEMANTIC_BROWSER_SUMMARY',JSON.stringify({passed:result.filter(r=>r.success).length,total:result.length,results:result}));
      if(result.some(r=>!r.success))process.exitCode=1;
      return;
    }
    if (process.env.LUCIA_GENERALIZATION_TEST === '1') {
      if (useCloud || !hasQwen) throw Error('Generalization tests require a local model');
      // New targets, not the URLs from the three-task MCP benchmark.
      // Expected paths are used ONLY for independent test assertions.
      const cases=[
        {label:'Wikipedia different search subject',url:'https://en.wikipedia.org/wiki/Main_Page',goal:'On this Wikipedia page, search for Alan Turing and open the corresponding article. Then report its page title.',expected:'/wiki/Alan_Turing'},
        {label:'Wiktionary search',url:'https://en.wiktionary.org/wiki/Wiktionary:Main_Page',goal:'On this Wiktionary page, search for algorithm and open the corresponding article. Then report its page title.',expected:'/wiki/algorithm'},
        {label:'Rust Book unfamiliar documentation link',url:'https://doc.rust-lang.org/stable/book/index.html',goal:'On this Rust Book page, click the Getting Started link. Then report the page title.',expected:'/stable/book/ch01-00-getting-started.html'},
        {label:'Python open-ended documentation navigation',url:'https://docs.python.org/3/',goal:'Find the introductory tutorial for learning Python on this documentation website. Open it and report the page title.',expected:'/3/tutorial/index.html'}
      ];
      for(const t of cases.filter(t=>!process.env.LUCIA_GENERALIZATION_MATCH||t.label.toLowerCase().includes(process.env.LUCIA_GENERALIZATION_MATCH.toLowerCase()))){
        try{
          await evalJs('window.browsphereApp.newTab('+JSON.stringify(t.url)+')');
          let before;
          for(let i=0;i<45;i++){
            before=await nativeState();
            if(hostname(before?.observed)===hostname(t.url)&&before.controls>0)break;
            await sleep(400);
          }
          if(hostname(before?.observed)!==hostname(t.url)||!before.controls||/access denied|not found|privacy error/i.test(before.title))throw Error('Public starting page unavailable');
          const started=Date.now();const task=await runTask(t.goal,t.url,140);const elapsedMs=Date.now()-started;
          const after=await nativeState();
          const exact=hostname(after.observed)===hostname(t.url)&&new URL(after.observed).pathname===t.expected;
          const acts=task.actions.filter(a=>a.type!=='complete'&&['ok','confirmed'].includes(a.outcome));
          const modelActed=acts.some(a=>/ollama:/.test(a.modelRoute||''));
          const nativeActed=acts.some(a=>/prerequisite:|search_input:|search_enter:/.test(a.modelRoute||''));
          const passed=exact&&acts.length>0&&task.status==='completed'&&task.verified&&!task.approvalRequired;
          console.log('GENERALIZATION_DETAIL',JSON.stringify({label:t.label,passed,exact,modelActed,nativeActed,task,before,after}));
          record(t.label,passed,{exact,modelActed,nativeActed,status:task.status,verified:task.verified,elapsedMs,replans:task.replans,graph:task.graph});
        }catch(e){record(t.label,false,{error:String(e)});}
      }
      console.log('GENERALIZATION_RESULT',JSON.stringify({passed:result.filter(x=>x.success).length,total:result.length,results:result}));
      if(result.some(x=>!x.success))process.exitCode=1;
      return;
    }
    if (process.env.LUCIA_MCP_PUBLIC_TEST === '1') {
      if (useCloud || !hasQwen) throw Error('Public MCP tests require a local model');
      const http=require('node:http'),fs=require('node:fs'),path=require('node:path');
      const audit=[];
      // This is a test-owned MCP adapter to live, public APIs, not a third-party MCP.
      // Never forward credentials, arbitrary URLs or unreviewed tool calls.
      const definitions={
        github_repo:{name:'github_repo',annotations:{readOnlyHint:true,destructiveHint:false},inputSchema:{type:'object',properties:{repository:{type:'string',enum:['python/cpython','rust-lang/rust']}},required:['repository']}},
        wikipedia_article:{name:'wikipedia_article',annotations:{readOnlyHint:true,destructiveHint:false},inputSchema:{type:'object',properties:{title:{type:'string',enum:['Ada Lovelace','Alan Turing']}},required:['title']}}
      };
      const server=http.createServer(async(req,res)=>{
        try {
          const lane=req.url;
          if(req.method==='DELETE'){res.writeHead(204);res.end();return;}
          if(req.method!=='POST'||!['/github','/wiki','/empty'].includes(lane)){res.writeHead(404);res.end();return;}
          let raw='';for await(const chunk of req){raw+=chunk;if(raw.length>32768)throw Error('oversized request');}
          const r=JSON.parse(raw);let value;
          audit.push({lane,method:r.method,name:r.params?.name});
          if(r.method==='initialize')value={protocolVersion:'2025-11-25',capabilities:{tools:{}},serverInfo:{name:'public-api-test-adapter',version:'1'}};
          else if(r.method==='notifications/initialized'){res.writeHead(202);res.end();return;}
          else if(r.method==='tools/list')value={tools:lane==='/empty'?[]:[definitions[lane==='/github'?'github_repo':'wikipedia_article']]};
          else if(r.method==='tools/call'){
            const name=r.params?.name,args=r.params?.arguments;
            let target,expected,field;
            if(lane==='/github'&&name==='github_repo'&&definitions.github_repo.inputSchema.properties.repository.enum.includes(args?.repository)){
              target='https://api.github.com/repos/'+args.repository;expected=args.repository;field='full_name';
            }else if(lane==='/wiki'&&name==='wikipedia_article'&&definitions.wikipedia_article.inputSchema.properties.title.enum.includes(args?.title)){
              target='https://en.wikipedia.org/w/api.php?action=query&format=json&formatversion=2&prop=info&titles='+encodeURIComponent(args.title);expected=args.title;field='title';
            }else throw Error('unapproved test call');
            const response=await fetch(target,{headers:{'User-Agent':'Browsphere-Lucia-public-readonly-test/1.0','Accept':'application/json'},signal:AbortSignal.timeout(15000),redirect:'error'});
            if(!response.ok)throw Error('Public API status '+response.status);
            const data=await response.json();
            const actual=field==='full_name'?data.full_name:data.query?.pages?.[0]?.title;
            if(actual!==expected||data.query?.pages?.[0]?.missing)throw Error('Public API result mismatch');
            audit.push({lane,publicHttpStatus:response.status,actual});
            value={content:[{type:'text',text:actual}],structuredContent:{value:actual},isError:false};
          }else throw Error('unsupported method');
          res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify({jsonrpc:'2.0',id:r.id,result:value}));
        }catch(e){audit.push({error:String(e)});res.writeHead(500);res.end('Test adapter failed');}
      });
      await new Promise((ok,no)=>{server.once('error',no);server.listen(0,'127.0.0.1',ok);});
      const root=path.join(process.env.LOCALAPPDATA,'BrowsphereAI');
      if(!process.env.LOCALAPPDATA.includes('browsphere-cross-site-'))throw Error('Refusing to modify non-isolated configuration');
      fs.mkdirSync(root,{recursive:true});
      const configPath=path.join(root,'mcp-servers.json');
      const local='http://127.0.0.1:'+server.address().port;
      fs.writeFileSync(configPath,JSON.stringify({github:{url:local+'/github',read_only_tools:['github_repo']},wiki:{url:local+'/wiki',read_only_tools:['wikipedia_article']},empty:{url:local+'/empty',read_only_tools:[]}}));
      async function startAt(url) {
        await evalJs('window.browsphereApp.newTab('+JSON.stringify(url)+')');
        for(let i=0;i<40;i++){const s=await nativeState().catch(()=>null);if(s?.controls&&hostname(s.observed)===hostname(url))return s;await sleep(400);}
        throw Error('Public website unavailable: '+url);
      }
      try {
        const tests=[
          {label:'GitHub MCP public API + native browser docs',server:'github',task:'Look up repository python/cpython using github_repo with repository python/cpython',expected:'python/cpython',url:'https://docs.python.org/3/',browser:'On this Python documentation homepage, click the Tutorial link to open the Python Tutorial page. Then report the page title.',path:'/3/tutorial/index.html'},
          {label:'Wikipedia MCP public API + native browser article',server:'wiki',task:'Look up article Ada Lovelace using wikipedia_article with title Ada Lovelace',expected:'Ada Lovelace',url:'https://en.wikipedia.org/wiki/Main_Page',browser:'On this Wikipedia page, search for Ada Lovelace and open the corresponding article. Then report its page title.',path:'/wiki/Ada_Lovelace'},
          {label:'MCP no-match ? real browser fallback',server:'empty',url:'https://docs.python.org/3/',browser:'On this Python documentation homepage, click the Tutorial link to open the Python Tutorial page. Then report the page title.',path:'/3/tutorial/index.html'}
        ];
        for(const t of tests){
          const start=audit.length;
          try {
            const before=await startAt(t.url);
            const expect={pointer:'/structuredContent/value',equals:t.expected};
            const lookup=process.env.LUCIA_MCP_EXPLICIT_TEST==='1'?{type:'call',server:t.server,tool:t.server==='github'?'github_repo':'wikipedia_article',arguments:t.server==='github'?{repository:t.expected}:{title:t.expected},expect}:{type:'auto',servers:[t.server],task:t.task,expect};
            const goal=t.server==='empty'?'mcp hybrid empty '+t.browser:'mcp workflow '+JSON.stringify({steps:[{id:'lookup',action:lookup},{id:'web',depends_on:['lookup'],action:{type:'browser',task:t.browser}}]});
            const task=await runTask(goal,t.url,150);
            const after=await nativeState();
            const calls=audit.slice(start);
            const api=t.server==='empty'?calls.some(a=>a.method==='tools/list')&&!calls.some(a=>a.method==='tools/call'):calls.some(a=>a.publicHttpStatus===200&&a.actual===t.expected);
            const landed=hostname(after.observed)===hostname(t.url)&&new URL(after.observed).pathname===t.path;
            const actualActions=task.actions.filter(a=>(a.outcome==='ok'||a.outcome==='confirmed')&&a.type!=='complete');
            const modelActed=actualActions.some(a=>/ollama:/.test(a.modelRoute||''));
            const primaryActed=actualActions.some(a=>(a.modelRoute||'').includes('ollama:'+localModel));
            const nativeActed=actualActions.some(a=>/prerequisite:|search_input:|search_enter:/.test(a.modelRoute||''));
            const acted=modelActed||nativeActed;
            const pass=api&&landed&&acted&&task.verified&&task.status==='completed'&&!task.approvalRequired;
            console.log('MCP_PUBLIC_DETAIL',JSON.stringify({label:t.label,pass,api,landed,acted,modelActed,primaryActed,nativeActed,task,before,after,audit:calls}));
            record(t.label,pass,{api,landed,acted,modelActed,primaryActed,nativeActed,status:task.status,verified:task.verified});
          } catch(e) {record(t.label,false,{error:String(e),audit:audit.slice(start)});}
        }
      }finally{await new Promise(r=>server.close(r));fs.rmSync(configPath,{force:true});}
      console.log('MCP_PUBLIC_RESULT',JSON.stringify({passed:result.filter(x=>x.success).length,total:result.length,results:result}));
      if(result.some(x=>!x.success))process.exitCode=1;
      return;
    }
    if (process.env.LUCIA_TRAIN_TEST === '1') {
      if (useCloud || !hasQwen) throw Error('The train test requires the selected installed local model');
      const url='https://www.irctc.co.in/nget/train-search';
      const date=process.env.LUCIA_TRAIN_DATE || '2026-11-24';
      await evalJs('window.browsphereApp.newTab('+JSON.stringify(url)+')');
      let before;
      for(let i=0;i<45;i++){
        before=await nativeState().catch(()=>null);
        if(before?.controls>0 && hostname(before.observed).endsWith('irctc.co.in'))break;
        await sleep(500);
      }
      console.log('TRAIN_START',JSON.stringify(before));
      if(!before?.controls || !hostname(before.observed).endsWith('irctc.co.in'))throw Error('Official IRCTC search page did not load with controls');
      const goal='Test a train booking search only. Choose English if a language dialog appears. On IRCTC find trains from Bengaluru to New Delhi for '+date+' departing at or after 6 PM India time. Select the city or station suggestions and correct journey date, search public trains and report observed train names, times and any displayed availability. Do not log in, enter passenger data, book, reserve, pay, solve CAPTCHA or bypass restrictions. Stop at public search results or explain any blocker.';
      const task=await runTask(goal,url,150);
      const after=await nativeState();
      const evidence=await evalJs("(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return {excerpt:(s?.text_excerpt||'').slice(0,3500),controls:(s?.interactive_nodes||[]).filter(n=>/from|to station|journey|search|date|train|availability/i.test(n.label||'')).slice(0,12).map(n=>({label:n.label,role:n.role,tag:n.tag}))}})()");
      // Independent schedule/card parser checks complete records and the
      // report's exact source evidence. No runtime classifier is the oracle.
      const checks=require('./train-result-oracle.cjs').evaluateTrainResults(evidence.excerpt,task.report,date,'Bengaluru','New Delhi',18*60);
      checks.readOnly=!task.approvalRequired&&task.actions.every(a=>!['download','set_checked'].includes(a.type));
      const verifiedWorkflow=task.status==='completed'&&task.verified&&Object.values(checks).every(Boolean);
      checks.modelDrivenAction=task.actions.some(a=>['ok','confirmed'].includes(a.outcome)&&a.type!=='complete'&&(a.modelRoute||'').includes('ollama:'+localModel));
      const passed=verifiedWorkflow&&checks.modelDrivenAction;
      console.log('TRAIN_RESULT',JSON.stringify({passed,verifiedWorkflow,task,before,after,evidence,checks}));
      if(!passed)process.exitCode=1;
      return;
    }
    const onlyPythonTutorial=process.env.LUCIA_ONLY_PYTHON_TUTORIAL === '1';
    const cases=(process.env.LUCIA_ONLY_WIKIPEDIA === '1' || onlyPythonTutorial) ? [] : [
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
          {status:task.status,actualWebView:actual,state,actions:task.actions,errors:task.errors,verified:task.verified,vision:task.vision});
      }catch(e){record(name+' autonomous open',false,{error:String(e)});}
    }
    if((onlyPythonTutorial || process.env.LUCIA_LIVE_MULTI === '1') && !useCloud && hasQwen){
      try {
        await evalJs("window.browsphereApp.newTab('https://docs.python.org/3/')");
        let before;
        for(let i=0;i<45;i++){
          before=await nativeState();
          if(hostname(before.observed)==='docs.python.org' && before.controls>0)break;
          await sleep(400);
        }
        if(hostname(before?.observed)!=='docs.python.org' || before.controls===0 ||
            /error|not found|access denied/i.test(before?.title||''))
          throw Error('Python documentation was not loaded with live interactive controls');
        const task=await runTask('On this Python documentation homepage, click the Tutorial link to open the Python Tutorial page. Then report the page title.','https://docs.python.org/3/',135);
        const after=await nativeState();
        const destination=(()=>{try{return new URL(after.observed)}catch{return null}})();
        const modelClick=task.actions.some(a=>a.type==='click' &&
          (a.outcome==='ok'||a.outcome==='confirmed') &&
          (a.modelRoute||'').includes('ollama:'+localModel));
        console.log('PYTHON_TUTORIAL_ACTION_TRACE',JSON.stringify({status:task.status,actions:task.actions,errors:task.errors,verified:task.verified,vision:task.vision,before,after}));
        record('Python Docs local agent click Tutorial',task.status==='completed' &&
          !!destination && destination.host==='docs.python.org' &&
          destination.pathname==='/3/tutorial/index.html' &&
          task.verified && modelClick,
          {status:task.status,modelClick,verified:task.verified,vision:task.vision,before,after,actions:task.actions,errors:task.errors});
      }catch(e){record('Python Docs local agent click Tutorial',false,{error:String(e)});}
    }
    if(!onlyPythonTutorial && (useCloud || hasQwen)){
      try {
        await evalJs("window.browsphereApp.newTab('https://en.wikipedia.org/wiki/Main_Page')");
        let before;
        for(let i=0;i<40;i++){
          before=await nativeState();
          if(hostname(before.observed)==='en.wikipedia.org' && before.controls>0)break;
          await sleep(400);
        }
        if(hostname(before?.observed)!=='en.wikipedia.org' ||
            /error|not found|access denied/i.test(before?.title||''))
          throw Error('Wikipedia starting document unavailable or error page; do not test model planning against an error page');
        const searchControls=await evalJs("(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return s.interactive_nodes.filter(n=>n.label?.toLowerCase().includes('search')||n.role==='searchbox').slice(0,20).map(n=>({tag:n.tag,role:n.role,label:n.label,submit:n.is_submit,sensitive:n.is_sensitive,inViewport:n.bbox?.in_viewport,node_id:n.node_id}))})()");
        console.log('WIKIPEDIA_SEARCH_CONTROLS',JSON.stringify(searchControls));
        const task=await runTask('On this Wikipedia page, search for Ada Lovelace and open the corresponding article. Then report its page title.','https://en.wikipedia.org/wiki/Main_Page',140);
        console.log('WIKIPEDIA_ACTION_TRACE', JSON.stringify({status:task.status,actions:task.actions,errors:task.errors,verified:task.verified}).slice(0,9000));
        const after=await nativeState();
        const selectedRouteSeen=task.actions.some(action=>
          useCloud ? /(?:^|\s\/\s)(?:groq|litellm):/.test(action.modelRoute||'') :
            (action.modelRoute||'').includes('ollama:'+localModel));
        record('Wikipedia real-model search and article',task.status==='completed' &&
          hostname(after.observed)==='en.wikipedia.org' &&
          decodeURIComponent(new URL(after.observed).pathname).toLowerCase().includes('ada_lovelace') &&
          task.verified && selectedRouteSeen,
          {status:task.status,before,after,selectedRouteSeen,actions:task.actions,errors:task.errors,verified:task.verified,vision:task.vision});
      }catch(e){record('Wikipedia real-model search and article',false,{error:String(e)});}
    }else if(!onlyPythonTutorial) record('Wikipedia real-model search and article',false,{reason:'No configured live model available; no genuine model call attempted'});
  }finally{close();}
  console.log('CROSS_SITE_RESULT',JSON.stringify({passed:result.filter(x=>x.success).length,total:result.length,results:result.map(x=>({name:x.name,success:x.success}))}));
  if(result.some(x=>!x.success))process.exitCode=1;
}
main().catch(e=>{console.error('CROSS_SITE_FATAL',String(e.stack||e));process.exitCode=2;});
