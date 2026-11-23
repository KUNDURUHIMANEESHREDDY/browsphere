'use strict';
// Real Windows Tauri/WebView2 end-to-end test with a LOOPBACK model simulator.
// No live public sites, credentials, paid API endpoints or consent bypasses.
const assert = require('node:assert/strict');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const port = Number(process.env.LUCIA_CDP_PORT || 9272);
const base = 'http://127.0.0.1:18764';

async function connect() {
  let target;
  for (let attempt = 0; attempt < 40; attempt++) {
    const targets = await (await fetch('http://127.0.0.1:' + port + '/json')).json();
    target = targets.find(t => t.type === 'page' && t.url?.includes('tauri.localhost'));
    if (target) break;
    await sleep(350);
  }
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

    // 2. Generic website: heavy navigation chrome hides real controls unless
    // the DOM reader prioritizes main content; search works only on Enter.
    const docsInitial = await openLocal('/generic-search.html');
    const docs = await run(
      'Search this documentation website for systems, press Enter, and open the API reference.',
      docsInitial
    );
    assert.ok(docs.actions.some(a=>a.type==='type_text' &&
      ['ok','confirmed'].includes(a.outcome)),'Generic docs query not typed');
    assert.ok(docs.actions.some(a=>a.type==='press_key' &&
      ['ok','confirmed'].includes(a.outcome)),'Generic docs Enter action not executed');
    assert.ok(docs.actions.some(a=>a.type==='click' &&
      ['ok','confirmed'].includes(a.outcome)),'Generic docs result link not clicked');
    const docsAfter = await evaluate(snapshotExpression);
    assert.ok(docsAfter?.origin?.includes('/generic-search.html?doc=api'),
      'Generic docs result URL not observed');
    assert.ok(docsAfter?.text_excerpt?.includes('GENERIC_DOCS_CONFIRMED'),
      'Generic docs confirmation not observed');
    console.log('PASS native generic website with 180 navigation controls and Enter-only search');

    // 3. Genuine failed primary click -> model recovers with fallback -> verified.
    const recoveryInitial=await openLocal('/recovery.html');
    const recovery=await run(
      'On the local recovery test page, click Start recovery test. After it fails, use the recovery fallback.',
      recoveryInitial
    );
    const failedAt=recovery.actions.findIndex(x=>x.type==='click' && x.outcome==='failed');
    assert.ok(failedAt>=0,'Test did not observe a genuine failed click');
    assert.ok(recovery.actions.slice(failedAt+1).some(x=>x.type==='click'
      && ['ok','confirmed'].includes(x.outcome)),'No successful fallback click');
    const finalVerification=recovery.actions.filter(x=>x.type==='complete'
      && x.route?.includes('Verification Agent')).at(-1);
    assert.equal(finalVerification?.outcome,'ok',
      'Verifier audit trace must report success even when its summary mentions a previous failed click');
    const afterRecovery=await evaluate(snapshotExpression);
    assert.ok(afterRecovery.text_excerpt.includes('Recovery complete: fallback button succeeded'),
      'Recovery confirmation not visible');
    console.log('PASS native failed action, autonomous fallback, and independent verification');

    // 3. A fresh task-owned background WebView must finish while the user
    // switches to another unrelated tab. Its verified result must not
    // navigate or mutate the foreground tab.
    const backgroundInitial = await openLocal('/multistep.html');
    const taskId = await evaluate('window.browsphereApp.createTaskTab(' +
      JSON.stringify(backgroundInitial.origin) + ')');
    assert.match(taskId, /^tab-lucia-/);
    const backgroundGoal = 'On the current local test page, search for robotics video and open the first result.';
    const backgroundRun = await evaluate(
      'window.aetherBridge.startLuciaSpecialistTask(' +
      JSON.stringify(backgroundGoal) + ',' +
      JSON.stringify(backgroundInitial.origin) + ',' +
      JSON.stringify(backgroundInitial.title) + ',' + JSON.stringify(taskId) + ')');
    assert.ok(backgroundRun, 'Background Lucia run did not start');
    await evaluate("window.browsphereApp.newTab('browsphere://start')");
    const browsingTab = await evaluate('window.browsphereApp.activeTab()');
    assert.notEqual(browsingTab.id, taskId, 'Browsing tab should differ from task tab');
    let backgroundTerminal;
    for (let i = 0; i < 80; i++) {
      await sleep(700);
      const events = await evaluate(eventsExpression) || [];
      backgroundTerminal = events.find(e => e.event === 'agent:run_status' &&
        e.payload?.run_id === backgroundRun);
      if (backgroundTerminal) break;
    }
    assert.ok(backgroundTerminal, 'Background task never reached a terminal state');
    assert.equal(backgroundTerminal.payload?.status, 'completed',
      'The background multi-step task failed');
    const backgroundSnapshot = await evaluate(
      "window.__TAURI__.core.invoke('capture_tab_snapshot',{tabId:" +
      JSON.stringify(taskId) + "})");
    assert.ok(backgroundSnapshot?.origin?.includes('/multistep.html?video=robotics-demo'),
      'Background task did not open the first video result');
    assert.ok(backgroundSnapshot.text_excerpt.includes('VIDEO_PAGE_CONFIRMED'),
      'Background target page lacked verification evidence');
    assert.equal((await evaluate('window.browsphereApp.activeTab()')).id, browsingTab.id,
      'Task unexpectedly changed the foreground tab');
    console.log('PASS native task-owned tab continues during unrelated browsing');

    // 4. Native close must cancel the matching task, including when paused.
    await evaluate("window.__TAURI__.core.invoke('set_agent_paused',{paused:true})");
    try {
      const closingTab = await evaluate(
        "window.browsphereApp.createTaskTab('about:blank')");
      const closingRun = await evaluate(
        "window.aetherBridge.startLuciaSpecialistTask(" +
          JSON.stringify('On this local test page, find the recovery control.') +
          ",'browsphere://start','New Tab'," + JSON.stringify(closingTab) + ")");
      assert.ok(closingRun, 'Closing test run did not start');
      // The user is still browsing elsewhere; close the task target by ID.
      await evaluate('window.browsphereApp.closeTab(' + JSON.stringify(closingTab) + ')');
      let closedTerminal, closedError;
      for (let i=0; i<45; i++) {
        await sleep(350);
        const events = await evaluate(eventsExpression) || [];
        closedError = events.find(e => e.event === 'agent:run_error' &&
          e.payload?.run_id === closingRun &&
          /assigned browser tab was closed/.test(e.payload?.message || ''));
        closedTerminal = events.find(e => e.event === 'agent:run_status' &&
          e.payload?.run_id === closingRun);
        if (closedTerminal && closedError) break;
      }
      assert.ok(closedError, 'Closing the assigned tab did not report the cause');
      assert.equal(closedTerminal?.payload?.status, 'cancelled',
        'Closing a task tab must stop the run');
      console.log('PASS closing the background task tab cancels with a clear message');
    } finally {
      await evaluate("window.__TAURI__.core.invoke('set_agent_paused',{paused:false})");
    }

    // 5. The user-facing View task tab control must open the assigned tab.
    await evaluate("window.browsphereApp.newTab('browsphere://start')");
    await evaluate("window.__TAURI__.core.invoke('set_agent_paused',{paused:true})");
    try {
      const chatReady = await evaluate("Boolean(document.getElementById('newTabChatInput'))");
      assert.ok(chatReady, 'Main Lucia composer unavailable');
      await evaluate("(()=>{const f=document.getElementById('newTabChatInput');" +
        "f.value='On this blank local test page, inspect available controls.';" +
        `document.querySelector('[data-action="nt-run-agent"]').click();return true})()`);
      let viewTabId;
      for (let i=0;i<45;i++) {
        await sleep(250);
        viewTabId = await evaluate(
          `document.querySelector('[data-action="nt-view-task-tab"]')?.dataset.tabId || null`);
        if (viewTabId) break;
      }
      assert.match(viewTabId || '', /^tab-lucia-/,
        'View task tab button not shown in the Lucia conversation');
      await evaluate(`document.querySelector('[data-action="nt-view-task-tab"]').click()`);
      const opened = await evaluate('window.browsphereApp.activeTab()');
      assert.equal(opened.id, viewTabId, 'View task tab did not select its target');
      await evaluate('window.browsphereApp.closeTab(' + JSON.stringify(viewTabId) + ')');
      console.log('PASS task progress button opens assigned WebView2 tab');
    } finally {
      await evaluate("window.__TAURI__.core.invoke('set_agent_paused',{paused:false})");
    }
  } finally {
    ws.close();
  }
}

main().catch(err=>{console.error('FAIL',err.stack||err);process.exitCode=1});
