'use strict';
// Real Windows WebView2 + Ollama smoke for the opt-in local Lucia recovery button.
// Run only against an isolated Browsphere profile with LUCIA_CDP_PORT set.
const assert = require('node:assert/strict');
const sleep = ms => new Promise(r => setTimeout(r, ms));
const port = process.env.LUCIA_CDP_PORT || '9273';

async function main() {
  const targets = await (await fetch('http://127.0.0.1:' + port + '/json')).json();
  const target = targets.find(t => t.type === 'page' && t.url?.includes('tauri.localhost'));
  assert.ok(target, 'Native Browsphere main WebView2 missing');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {ws.onopen = resolve; ws.onerror = reject;});
  let serial = 0;
  const pending = new Map();
  ws.onmessage = e => {
    const m = JSON.parse(e.data);
    const waiter = pending.get(m.id);
    if (!waiter) return;
    pending.delete(m.id);
    m.error ? waiter.reject(Error(JSON.stringify(m.error))) : waiter.resolve(m.result || {});
  };
  async function command(method, params={}) {
    const id = ++serial;
    return Promise.race([
      new Promise((resolve, reject) => {
        pending.set(id, {resolve, reject});
        ws.send(JSON.stringify({id, method, params}));
      }),
      sleep(15000).then(() => {
        if (pending.has(id)) {pending.delete(id); throw Error('CDP timeout: ' + method);}
      })
    ]);
  }
  async function evaluate(expression) {
    const result = await command('Runtime.evaluate', {
      expression, returnByValue: true, awaitPromise: true, userGesture: true
    });
    if (result.exceptionDetails) throw Error(JSON.stringify(result.exceptionDetails).slice(0,700));
    return result.result?.value;
  }
  try {
    await command('Runtime.enable');
    let ready = false;
    for (let i=0; i<60; i++) {
      ready = await evaluate('Boolean(window.browsphereApp?.openSettings && window.aetherBridge?.isNative)').catch(()=>false);
      if (ready) break;
      await sleep(350);
    }
    assert.ok(ready, 'Browser shell was never ready');

    // The temporary profile has no user credentials. Simulate cloud-only state
    // to verify the button explicitly switches back to a real local model.
    const prior = await evaluate(`(async()=>{
      const c=await window.__TAURI__.core.invoke('get_router_config');
      c.prefer_local_for_simple=false;c.lucia_cloud_only=true;
      await window.__TAURI__.core.invoke('update_router_config',{config:c});
      return {cloudOnly:c.lucia_cloud_only};
    })()`);
    assert.equal(prior.cloudOnly, true);
    await evaluate("window.browsphereApp.openSettings('ai')");
    let found = false;
    for (let i=0;i<50;i++) {
      found = await evaluate('Boolean(document.querySelector(\'[data-action="use-local-lucia-fallback"]\'))').catch(()=>false);
      if (found) break;
      await sleep(350);
    }
    assert.ok(found, 'Use local model for Lucia button not rendered');
    await evaluate('document.querySelector(\'[data-action="use-local-lucia-fallback"]\').click()');

    let result;
    for (let i=0;i<100;i++) {
      await sleep(500);
      result=await evaluate(`(async()=>{
        const c=await window.__TAURI__.core.invoke('get_router_config');
        return {local:c.prefer_local_for_simple,cloudOnly:c.lucia_cloud_only,
          model:c.ollama_model,
          status:document.getElementById('cfgConnectionStatus')?.textContent || ''};
      })()`);
      if (result.status.startsWith('Connected:') || result.status.startsWith('Connection failed:')) break;
    }
    console.log('LOCAL_FALLBACK', JSON.stringify(result));
    assert.ok(result.status.startsWith('Connected:'), 'Local Lucia action validation failed');
    assert.equal(result.local, true, 'Local preference was not saved');
    assert.equal(result.cloudOnly, false, 'Cloud-only Lucia setting was not turned off');
    assert.equal(result.model, 'qwen2.5:3b-instruct', 'Verified installed model was not selected');
    console.log('PASS: AI & ML enabled a tested local Lucia model from cloud-only state');
  } finally {ws.close();}
}
main().catch(e => {console.error('FAIL', e.stack || e); process.exitCode=1;});
