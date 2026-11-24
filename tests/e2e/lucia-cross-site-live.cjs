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
      JSON.stringify(overrides)+");await window.__TAURI__.core.invoke('update_router_config',{config:c});window.__crossSiteEvents=[];for(const name of ['agent:trace','agent:run_status','agent:run_error','agent:verification_report','agent:approval_required','agent:stage','agent:vision','agent:prerequisite','agent:supervisor'])await window.__TAURI__.event.listen(name,e=>window.__crossSiteEvents.push({kind:name,payload:e.payload}));return true})()");
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
      let events=[], end=null, approvalRequired=false;
      for(let i=0;i<timeoutSeconds;i++){
        await sleep(1000);
        events=(await evalJs('window.__crossSiteEvents.slice(-400)'))||[];
        const mine=events.filter(e=>e.payload?.run_id===id);
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
      return {status:approvalRequired?'approval':(end?.payload?.status||'timeout'),
        approvalRequired, actions:mine.filter(e=>e.kind==='agent:trace')
        .map(e=>({type:e.payload?.action?.type,
          nodeId:e.payload?.action?.node_id,
          outcome:e.payload?.outcome,
          decision:e.payload?.decision,
          modelRoute:e.payload?.model_route})),
        errors:mine.filter(e=>e.kind==='agent:run_error').map(e=>String(e.payload?.message||'').slice(0,150)),
        vision:mine.filter(e=>e.kind==='agent:vision').map(e=>e.payload?.status),prerequisites:mine.filter(e=>e.kind==='agent:prerequisite').map(e=>({phase:e.payload?.phase,specialist:e.payload?.specialist,dialog:e.payload?.active_dialog})),
        verified:mine.some(e=>e.kind==='agent:verification_report' && e.payload?.report?.status==='evidence_checked')};
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
            const acted=task.actions.some(a=>(a.outcome==='ok'||a.outcome==='confirmed')&&a.type!=='complete'&&(a.modelRoute||'').includes('ollama:'+localModel));
            const pass=api&&landed&&acted&&task.verified&&task.status==='completed'&&!task.approvalRequired;
            console.log('MCP_PUBLIC_DETAIL',JSON.stringify({label:t.label,pass,api,landed,acted,task,before,after,audit:calls}));
            record(t.label,pass,{api,landed,acted,status:task.status,verified:task.verified});
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
      const hasTrainList=/train list|trains found|train number|check availability|no trains found|no train found/i.test(evidence.excerpt);
      const routeMatches=/bengaluru|bangalore|sbc|smvt|ypr/i.test(evidence.excerpt)&&/delhi|ndls|nzm/i.test(evidence.excerpt);
      const dateMatches=evidence.excerpt.includes(date)||/24[\s/.-]+nov|24[\s/.-]+11[\s/.-]+2026/i.test(evidence.excerpt);
      const passed=task.status==='completed'&&task.verified&&hasTrainList&&routeMatches&&dateMatches&&task.actions.some(a=>(a.modelRoute||'').includes('ollama:'+localModel));
      console.log('TRAIN_RESULT',JSON.stringify({passed,task,before,after,evidence,checks:{hasTrainList,routeMatches,dateMatches}}));
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
