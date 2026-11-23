'use strict';
// Real Windows Tauri/WebView2 end-to-end test with a LOOPBACK model simulator.
// No live public sites, credentials, paid API endpoints or consent bypasses.
const assert = require('node:assert/strict');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const port = Number(process.env.LUCIA_CDP_PORT || 9272);
const base = 'http://127.0.0.1:18764';

async function connect() {
  const targets = await (await fetch('http://127.0.0.1:' + port + '/json')).json();
  const target = targets.find(t => t.type === 'page' && t.url?.includes('tauri.localhost'));
  assert.ok(target, 'Native Browsphere main WebView2 target missing');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((ok,bad) => { ws.onopen = ok; ws.onerror = bad; });
  let seq = 0;
  const pending = new Map();
  ws.onmessage = event => {
    const result = JSON.parse(event.data);
    const task = pending.get(result.id);
    if (!task) return;
    pending.delete(result.id);
    result.error ? task.reject(Error(JSON.stringify(result.error)))
      : task.resolve(result.result || {});
  };
  async function command(method, params = {}) {
    const id = ++seq;
    return Promise.race([
      new Promise((resolve,reject) => {
        pending.set(id,{resolve,reject});
        ws.send(JSON.stringify({id,method,params}));
      }),
      sleep(18000).then(() => {
        if (pending.has(id)) {
          pending.delete(id);
          throw Error('CDP command timed out: ' + method);
        }
      })
    ]);
  }
  async function evaluate(expression) {
    const response = await command('Runtime.evaluate',{
      expression, returnByValue:true, awaitPromise:true, userGesture:true
    });
    if (response.exceptionDetails) {
      throw Error('Evaluation failed: ' + JSON.stringify(response.exceptionDetails).slice(0,600));
    }
    return response.result?.value;
  }
  return {ws,command,evaluate};
}

async function main() {
  assert.equal((await (await fetch('http://127.0.0.1:18766/health')).json()).ok,true,
    'Loopback test model is not running');
  assert.equal((await (await fetch(base + '/health')).json()).ok,true,
    'Local webpage fixture is not running');
  const {ws,command,evaluate} = await connect();
  const snapshotExpression = "window.__TAURI__.core.invoke('capture_live_snapshot')";
  const eventsExpression = "window.__luciaNativeEvents.slice(-300)";
  try {
    await command('Runtime.enable');
    await evaluate("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');c.ollama_url='http://127.0.0.1:18766';c.ollama_model='lucia-local-e2e';c.prefer_local_for_simple=true;c.domain_overrides={};await window.__TAURI__.core.invoke('update_router_config',{config:c});window.__luciaNativeEvents=[];for(const name of ['agent:plan','agent:stage','agent:trace','agent:run_status','agent:run_error','agent:verification_report','agent:approval_required'])await window.__TAURI__.event.listen(name,e=>window.__luciaNativeEvents.push({event:name,payload:e.payload}));return true})()");
    async function openLocal(path) {
      const url = base + path;
      await evaluate('window.browsphereApp.newTab(' + JSON.stringify(url) + ')');
      for (let i=0;i<40;i++) {
        await sleep(350);
        const observed = await evaluate(snapshotExpression).catch(() => null);
        if (observed?.origin === url) {
          assert.ok(observed.interactive_nodes?.length > 0, 'Page has no observed native controls');
          return observed;
        }
      }
      throw Error('Fixture navigation not observed in native WebView: ' + url);
    }

    async function run(goal, initial, limit=60) {
      // Exercise the real UI-to-Rust bridge, including run-to-tab registration.
      const id = await evaluate('window.aetherBridge.startLuciaSpecialistTask(' +
        JSON.stringify(goal) + ',' + JSON.stringify(initial.origin) + ',' +
        JSON.stringify(initial.title) + ')');
      assert.ok(id,'Native agent run failed to start');
      let terminal, events = [];
      for(let i=0;i<limit;i++){
        await sleep(750);
        events = await evaluate(eventsExpression) || [];
        const approval = events.find(x=>x.event==='agent:approval_required'
          && x.payload?.run_id===id);
        if(approval){
          await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')");
          throw Error('Unexpected approval required on isolated harmless test fixture');
        }
        terminal=events.find(x=>x.event==='agent:run_status'
          && x.payload?.run_id===id);
        if(terminal)break;
      }
      if(!terminal){
        await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')");
        throw Error('Lucia task timed out');
      }
      const relevant=events.filter(x=>x.payload?.run_id===id);
      const actions=relevant.filter(x=>x.event==='agent:trace').map(x=>({
        type:x.payload?.action?.type,
        outcome:x.payload?.outcome,
        route:x.payload?.model_route
      }));
      const report=relevant.find(x=>x.event==='agent:verification_report')?.payload?.report;
      const errors=relevant.filter(x=>x.event==='agent:run_error')
        .map(x=>x.payload?.message);
      console.log('AGENT_RUN',JSON.stringify({
        status:terminal.payload?.status,actions,reportStatus:report?.status,errors
      }));
      assert.equal(terminal.payload?.status,'completed','Agent run did not finish');
      assert.equal(report?.status,'evidence_checked','Missing independent evidence');
      return {actions,report};
    }

    // 1. Search text -> submit local search -> click result -> SPA URL -> verified.
    const initial=await openLocal('/multistep.html');
    const search=await run(
      'On the current local test page, search for robotics video and open the first result.',
      initial
    );
    assert.ok(search.actions.some(x=>x.type==='type_text'
      && ['ok','confirmed'].includes(x.outcome)), 'Search typing not executed');
    assert.ok(search.actions.filter(x=>x.type==='click'
      && ['ok','confirmed'].includes(x.outcome)).length>=2,
      'Search control + result were not clicked');
    const after=await evaluate(snapshotExpression);
    assert.ok(after.origin.includes('/multistep.html?video=robotics-demo'),
      'No real observed SPA video-result URL');
    assert.ok(after.text_excerpt.includes('VIDEO_PAGE_CONFIRMED'),
      'No visible result confirmation');
    // The latest completed DOM observation must update the actual UI address bar.
    let synced=false;
    for(let i=0;i<15;i++){
      const tab=await evaluate('window.browsphereApp.activeTab()');
      if(tab?.url===after.origin){synced=true;break;}
      await sleep(150);
    }
    assert.ok(synced,'Visible browser URL was not synchronized with SPA navigation');
    console.log('PASS native multi-step typing, search, result click, SPA URL sync and evidence review');

    // 2. Genuine failed primary click -> model recovers with fallback -> verified.
    const recoveryInitial=await openLocal('/recovery.html');
    const recovery=await run(
      'On the local recovery test page, click Start recovery test. After it fails, use the recovery fallback.',
      recoveryInitial
    );
    const failedAt=recovery.actions.findIndex(x=>x.type==='click' && x.outcome==='failed');
    assert.ok(failedAt>=0,'Test did not observe a genuine failed click');
    assert.ok(recovery.actions.slice(failedAt+1).some(x=>x.type==='click'
      && ['ok','confirmed'].includes(x.outcome)),'No successful fallback click');
    const afterRecovery=await evaluate(snapshotExpression);
    assert.ok(afterRecovery.text_excerpt.includes('Recovery complete: fallback button succeeded'),
      'Recovery confirmation not visible');
    console.log('PASS native failed action, autonomous fallback, and independent verification');
  } finally {
    ws.close();
  }
}

main().catch(err=>{console.error('FAIL',err.stack||err);process.exitCode=1});
