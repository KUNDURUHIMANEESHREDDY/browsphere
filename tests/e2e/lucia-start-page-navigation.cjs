'use strict';
// End-to-end regression for Lucia navigating from the ACTUAL Browsphere start page.
// Unlike lucia-direct-navigation.cjs, this test does not create a child WebView
// as a precondition. Launch a disposable native Browsphere with WebView2 CDP.
const assert = require('node:assert/strict');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const port = process.env.BROWSER_CDP_PORT || '9272';
async function main() {
  const endpoint = 'http://127.0.0.1:' + port + '/json';
  const pages = await (await fetch(endpoint)).json();
  const chrome = pages.find(p => p.type === 'page' && p.url?.includes('tauri.localhost') && p.title?.includes('Browsphere'))
    || pages.find(p => p.type === 'page' && p.url?.includes('tauri.localhost'));
  assert.ok(chrome, 'Native Browsphere chrome not found');
  const ws = new WebSocket(chrome.webSocketDebuggerUrl);
  await new Promise((ok, bad) => { ws.onopen = ok; ws.onerror = bad; });
  let serial = 0;
  const pending = new Map();
  ws.onmessage = event => {
    const m = JSON.parse(event.data);
    if (!pending.has(m.id)) return;
    const p = pending.get(m.id); pending.delete(m.id);
    m.error ? p.reject(Error(JSON.stringify(m.error))) : p.resolve(m.result || {});
  };
  function command(method, params = {}) {
    const id = ++serial;
    return Promise.race([
      new Promise((resolve, reject) => {
        pending.set(id, {resolve, reject});
        ws.send(JSON.stringify({id, method, params}));
      }),
      sleep(15000).then(() => { if (pending.has(id)) { pending.delete(id); throw Error('Timed out: ' + method); } })
    ]);
  }
  async function evaluate(expression) {
    const result = await command('Runtime.evaluate', {expression, returnByValue:true, awaitPromise:true, userGesture:true});
    if (result.exceptionDetails) throw Error(JSON.stringify(result.exceptionDetails));
    return result.result?.value;
  }
  try {
    await command('Runtime.enable');
    assert.equal(await evaluate('window.aetherBridge?.isNative'), true);
    const intent = await evaluate("window.__TAURI__.core.invoke('route_lucia_intent',{message:'open youtube'})");
    assert.equal(intent?.mode, 'browser_task', 'Explicit navigation should not depend on intent model');
    await evaluate("window.browsphereApp.newTab('browsphere://start')");
    await sleep(350);
    const before = await evaluate('window.browsphereApp.activeTab()');
    assert.equal(before.url, 'browsphere://start', 'Need to start from the actual in-app New Tab page');
    // The old preflight rejected this goal before returning a run ID.
    // Exercise startup independently of the configured planner and its response.
    const genericRun = await evaluate("window.aetherBridge.startLuciaSpecialistTask('Search YouTube for robotics videos','browsphere://start','New Tab')");
    try {
      assert.match(genericRun, /^[0-9a-f-]{36}$/i, 'A multi-step request should start on New Tab');
      const blank = await evaluate("window.__TAURI__.core.invoke('capture_live_snapshot')");
      assert.equal(blank?.origin, 'about:blank', 'Startup should attach an empty native document');
      assert.equal(blank.interactive_nodes.length, 0);
    } finally {
      await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')");
    }
    await evaluate("window.browsphereApp.newTab('browsphere://start')");
    await sleep(350);
    const output = await evaluate("(async()=>{window.__luciaStartEvents=[];window.__luciaStartUnlisten=await window.__TAURI__.event.listen('agent:run_status',ev=>window.__luciaStartEvents.push(ev.payload));return await window.aetherBridge.startLuciaSpecialistTask('open youtube','browsphere://start','New Tab')})()");
    assert.ok(output, 'Lucia task did not start');
    let outcome;
    for (let i=0; i<30; i++) {
      await sleep(900);
      const results = await evaluate('window.__luciaStartEvents');
      outcome = results.find(x => x.run_id === output);
      if (outcome) break;
    }
    const state = await evaluate("({tab:window.browsphereApp.activeTab(),address:document.getElementById('omnibarInput')?.value,native:window.aetherBridge.isNative})");
    const snapshot = await evaluate("window.__TAURI__.core.invoke('capture_live_snapshot')");
    console.log('LUCIA_START_PAGE_RESULT',JSON.stringify({status:outcome?.status,state,observedUrl:snapshot?.origin,observedTitle:snapshot?.title}));
    assert.equal(outcome?.status, 'completed', 'Lucia must finish with evidence');
    assert.match(snapshot?.origin || '', /^https:\/\/(www\.)?youtube\.com\//);
    assert.match(state.tab.url, /^https:\/\/(www\.)?youtube\.com\//, 'Visible app tab must navigate');
    assert.match(state.address, /youtube\.com/, 'Visible address bar must update');
    console.log('PASS: Lucia routed navigation and actually opened YouTube from New Tab');
  } finally { ws.close(); }
}
main().catch(e => { console.error('FAIL',e.stack || e); process.exitCode = 1; });
