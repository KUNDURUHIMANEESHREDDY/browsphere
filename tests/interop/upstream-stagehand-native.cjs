'use strict';
const {chromium}=require('playwright-core');
const assert=require('node:assert/strict');
const delay=ms=>new Promise(ok=>setTimeout(ok,ms));
const cdpPort=Number(process.env.LUCIA_CDP_PORT);
const fixturePort=Number(process.env.LUCIA_FIXTURE_PORT);
if(process.env.GITHUB_ACTIONS!=='true'||!Number.isInteger(cdpPort)||!Number.isInteger(fixturePort)){
  throw Error('Stagehand native adapter is restricted to isolated Windows CI browser profiles');
}
(async()=>{
 const browser=await chromium.connectOverCDP('http://127.0.0.1:'+cdpPort);
 try{
  const page=browser.contexts().flatMap(x=>x.pages()).find(x=>x.url().includes('tauri.localhost'));
  assert.ok(page,'Installed Browsphere Tauri WebView2 shell missing');
  const modelPort=Number(process.env.LUCIA_MODEL_PORT);
  assert.ok(Number.isInteger(modelPort)&&modelPort>=1024&&modelPort<=65535,
    'Isolated model service port missing');
  // The existing native chatbox requires an explicit healthy model route
  // before dispatching *any* task, even an MCP-only read. Use the same
  // deterministic CI loopback model as Lucia's already-verified E2E runner.
  // Do not disable or weaken the runtime's readiness admission check.
  await page.evaluate(async port=>{
    const cfg=await window.__TAURI__.core.invoke('get_router_config');
    cfg.ollama_url='http://127.0.0.1:'+port;
    cfg.ollama_model='lucia-local-e2e';
    cfg.prefer_local_for_simple=true;
    cfg.lucia_cloud_only=false;
    cfg.domain_overrides={};
    await window.__TAURI__.core.invoke('update_router_config',{config:cfg});
  },modelPort);
  await page.evaluate(async()=>{
    window.__upstreamMcpEvents=[];
    for(const name of ['agent:run_status','agent:run_error','agent:run_completed'])
      await window.__TAURI__.event.listen(name,e=>window.__upstreamMcpEvents.push({type:name,payload:e.payload}));
  });
  const fixture='http://127.0.0.1:'+fixturePort+'/generic-search.html';
  await page.evaluate(url=>window.browsphereApp.newTab(url),fixture);
  let initial;
  for(let i=0;i<45;i++){
    initial=await page.evaluate(()=>window.__TAURI__.core.invoke('capture_live_snapshot')).catch(()=>null);
    if(initial?.origin===fixture)break;
    await delay(350);
  }
  assert.equal(initial?.origin,fixture,'Native browser source not observed before MCP');
  async function run(goal,need){
    await page.evaluate(()=>{window.__upstreamMcpEvents=[]});
    const runId=await page.evaluate(({goal,origin,title})=>
      window.aetherBridge.startLuciaSpecialistTask(goal,origin,title),
      {goal,origin:initial.origin,title:initial.title});
    assert.ok(runId,'Native Lucia task ID missing');
    let terminal,summary,err;
    for(let i=0;i<100;i++){
      await delay(350);
      const events=await page.evaluate(()=>window.__upstreamMcpEvents);
      terminal=events.find(e=>e.type==='agent:run_status'&&e.payload?.run_id===runId);
      summary=events.find(e=>e.type==='agent:run_completed')?.payload;
      err=events.find(e=>e.type==='agent:run_error'&&e.payload?.run_id===runId)?.payload?.message;
      if(terminal)break;
    }
    assert.equal(terminal?.payload?.status,'completed','Native MCP task failed: '+String(err||'no terminal status').slice(0,240));
    assert.ok(String(summary).includes(need),'Upstream MCP capability missing from native response');
    return summary;
  }
  await run('mcp list stagehand','stagehand_inspect');
  await run('mcp call stagehand/stagehand_inspect '+JSON.stringify({
    url:fixture,instruction:'Read the page title'
  }),'Stagehand');
  console.log('LUCIA_UPSTREAM_MCP_NATIVE_PASS '+JSON.stringify({
    installed_webview2:true,official_mcp_tools:true,
    external_stagehand_inspection:true,verified_task_completion:false
  }));
 }finally{await browser.close();}
})().catch(e=>{console.error('LUCIA_UPSTREAM_MCP_NATIVE_FAILED '+e.message);process.exitCode=1;});
