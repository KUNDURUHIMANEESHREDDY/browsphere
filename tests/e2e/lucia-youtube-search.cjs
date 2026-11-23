'use strict';
// Manual native Windows E2E regression. Launch an isolated Browsphere with
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9272.
// Requires a running local Ollama qwen2.5:3b-instruct model.
// Searches a public YouTube page; never approves account/consent actions.
const assert = require('node:assert/strict');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const port = process.env.LUCIA_CDP_PORT || '9272';

async function main() {
  const targets = await (await fetch('http://127.0.0.1:' + port + '/json')).json();
  const target = targets.find(t => t.type === 'page' && t.url?.includes('tauri.localhost'));
  assert.ok(target, 'Native Browsphere main window missing');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  let id = 0;
  const pending = new Map();
  ws.onmessage = e => {
    const message = JSON.parse(e.data);
    const task = pending.get(message.id);
    if (!task) return;
    pending.delete(message.id);
    message.error ? task.reject(Error(JSON.stringify(message.error))) : task.resolve(message.result || {});
  };
  async function command(method, params = {}) {
    const seq = ++id;
    return Promise.race([
      new Promise((resolve, reject) => {
        pending.set(seq, {resolve, reject});
        ws.send(JSON.stringify({id:seq, method, params}));
      }),
      sleep(18000).then(() => {
        if (pending.has(seq)) {
          pending.delete(seq);
          throw Error('CDP timed out: ' + method);
        }
      })
    ]);
  }
  async function evaluate(expression) {
    const response = await command('Runtime.evaluate', {
      expression, returnByValue: true, awaitPromise: true, userGesture: true
    });
    if (response.exceptionDetails) throw Error(JSON.stringify(response.exceptionDetails));
    return response.result?.value;
  }
  async function terminal(runId, count = 100) {
    for (let i = 0; i < count; i++) {
      await sleep(1500);
      const all = await evaluate('window.__luciaYoutubeEvents.slice(-200)') || [];
      const approval = all.find(e => e.event === 'agent:approval_required' && e.payload?.run_id === runId);
      if (approval) {
        await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')");
        throw Error('Unexpected approval required; no external consent was given');
      }
      const status = all.find(e => e.event === 'agent:run_status' && e.payload?.run_id === runId);
      if (status) return {status: status.payload.status, events: all};
    }
    await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')");
    throw Error('Lucia task did not reach a terminal status');
  }
  const compactSnapshot = "(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return {origin:s?.origin,title:s?.title,searchControls:(s?.interactive_nodes||[]).filter(n=>/search/i.test(n.label)).map(n=>({id:n.node_id,label:n.label,tag:n.tag,role:n.role})).slice(0,10)}})()";
  try {
    await command('Runtime.enable');
    await evaluate("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');c.prefer_local_for_simple=true;c.ollama_url='http://127.0.0.1:11434';c.ollama_model='qwen2.5:3b-instruct';await window.__TAURI__.core.invoke('update_router_config',{config:c});window.__luciaYoutubeEvents=[];for(const event of ['agent:trace','agent:stage','agent:run_status','agent:run_error','agent:verification_report','agent:approval_required'])await window.__TAURI__.event.listen(event,e=>window.__luciaYoutubeEvents.push({event,payload:e.payload}));window.browsphereApp.newTab('browsphere://start');return true})()");
    await sleep(400);
    const openRun = await evaluate("window.aetherBridge.startLuciaSpecialistTask('open youtube','browsphere://start','New Tab')");
    const opened = await terminal(openRun, 35);
    assert.equal(opened.status, 'completed', 'Lucia did not open YouTube');
    await sleep(2500);
    const before = await evaluate(compactSnapshot);
    const visibleBefore = await evaluate('window.browsphereApp.activeTab()');
    console.log('BEFORE', JSON.stringify({snapshot: before, tab: visibleBefore?.url}));
    assert.match(before?.origin || '', /^https:\/\/(www\.)?youtube\.com\//);
    const goal = 'Search YouTube for NASA Artemis mission and open the first video result, then report the observed video page.';
    const taskTabId = await evaluate("window.browsphereApp.createTaskTab('https://www.youtube.com/')");
    const runId = await evaluate("window.aetherBridge.startLuciaSpecialistTask(" +
      JSON.stringify(goal) + ",'https://www.youtube.com/','YouTube'," + JSON.stringify(taskTabId) + ")");
    // Do not bring the task-owned tab to the foreground: prove that its
    // native WebView2 keeps working independently of everyday browsing.
    await evaluate("window.browsphereApp.newTab('browsphere://start')");
    const foregroundTab = await evaluate('window.browsphereApp.activeTab()');
    assert.notEqual(foregroundTab?.id, taskTabId, 'Lucia unexpectedly stole browser focus');
    const result = await terminal(runId);
    const after = await evaluate("(async()=>{const s=await window.__TAURI__.core.invoke('capture_tab_snapshot',{tabId:" +
      JSON.stringify(taskTabId) +
      "});return {origin:s?.origin,title:s?.title,searchControls:(s?.interactive_nodes||[]).filter(n=>/search/i.test(n.label)).slice(0,10)}})()")
      .catch(err => ({error:String(err)}));
    const visibleAfter = await evaluate('window.browsphereApp.activeTab()');
    assert.equal(visibleAfter?.id, foregroundTab?.id,
      'Browsing foreground tab changed during background YouTube task');
    const traces = result.events.filter(e=>e.event==='agent:trace' && e.payload?.run_id===runId);
    const reports = result.events.filter(e=>e.event==='agent:verification_report' && e.payload?.run_id===runId);
    const actions = traces.map(e=>({
      type:e.payload?.action?.type, nodeId:e.payload?.action?.node_id,
      origin:e.payload?.active_origin, decision:e.payload?.decision,
      outcome:e.payload?.outcome, route:e.payload?.model_route
    }));
    console.log('RESULT', JSON.stringify({
      status:result.status, snapshot:after, tab:visibleAfter?.url,
      actions, verificationReports:reports.length
    }));
    assert.equal(result.status, 'completed', 'Lucia did not finish the multi-step task');
    assert.ok(reports.length, 'No independent evidence-backed verification report');
    // A real search can be performed by entering text or by navigating to a
    // valid YouTube results URL. Verify the *observed results*, not one input method.
    const verifiedSearch = reports.some(e => (e.payload?.report?.findings || []).some(f => {
      try {
        const page = new URL(f.url);
        return page.protocol === 'https:' && page.hostname === 'www.youtube.com'
          && page.pathname === '/results'
          && (page.searchParams.get('search_query') || '').trim().toLowerCase() === 'nasa artemis mission';
      } catch { return false; }
    }));
    assert.ok(verifiedSearch, 'No independently verified YouTube search-results evidence');
    assert.ok(actions.some(a=>a.type==='click' && ['ok','confirmed'].includes(a.outcome)),
      'No successful video-result click');
    assert.match(after?.origin || '', /^https:\/\/(www\.)?youtube\.com\/watch/,
      'No observed YouTube watch page');
    console.log('PASS: Background Lucia searched, clicked first YouTube result, and verified observed watch page without changing foreground tab');
  } finally {
    ws.close();
  }
}
main().catch(err => { console.error('FAIL', err.stack || err); process.exitCode = 1; });
