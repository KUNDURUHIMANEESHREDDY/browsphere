'use strict';
// Native WebView2 smoke test for Settings > AI & ML > Lucia Diagnostics.
// Run with a disposable LOCALAPPDATA directory and WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const port = Number(process.env.LUCIA_DIAG_CDP_PORT || 9277);
const artifactDir = path.join(__dirname, 'artifacts');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const failed = '11111111-1111-4111-8111-111111111111';
const completed = '22222222-2222-4222-8222-222222222222';

function installFixture() {
  const local = process.env.LUCIA_DIAG_LOCALAPPDATA;
  assert.ok(local, 'Set LUCIA_DIAG_LOCALAPPDATA to a disposable test folder');
  assert.ok(local.includes('artifacts'), 'Refusing to change real user diagnostic logs');
  const dir = path.join(local, 'BrowsphereAI', 'logs');
  fs.mkdirSync(dir, {recursive:true});
  const fixtureStart = Date.now();
  const item = (offsetSeconds, channel, event, fields) =>
    JSON.stringify({timestamp:new Date(fixtureStart + offsetSeconds * 1000).toISOString(),
      channel, event, fields});
  fs.writeFileSync(path.join(dir, 'agent.jsonl'), [
    item(0,'agent','run_started',{run_id:failed, count:6}),
    item(1,'agent','action_trace',{run_id:failed, trace_id:'33333333-3333-4333-8333-333333333333',
      step:1, action:'navigate', decision:'allow', risk:'Low', model_route:'agent_model',
      latency_ms:320, outcome:'verified', api_key:'NEVER-EXPOSE-SECRET'}),
    item(2,'agent','action_trace',{run_id:failed, trace_id:'44444444-4444-4444-8444-444444444444',
      step:2, action:'click', decision:'deny', risk:'High', model_route:'agent_model', latency_ms:24, outcome:'denied'}),
    item(2.5,'agent','tab_lifecycle',{run_id:failed,source:'user_close',kind:'intentional',status:'closed',secret:'NOT-EXPOSED'}),
    item(3,'agent','run_finished',{run_id:failed,status:'tab_closed'}),
    item(4,'agent','run_started',{run_id:completed,count:2}),
    item(5,'agent','run_finished',{run_id:completed,status:'completed'})
  ].join('\n')+'\n');
  fs.writeFileSync(path.join(dir, 'errors.jsonl'), item(3,'errors','agent_failure',
    {run_id:failed,error_kind:'agent_run'})+'\n');
}

async function main() {
  installFixture();
  let target;
  for (let i = 0; i < 60; i++) {
    try {
      const entries = await (await fetch('http://127.0.0.1:' + port + '/json')).json();
      target = entries.find(t => t.type === 'page' && t.url?.includes('tauri.localhost') && t.title?.includes('Browsphere'))
        || entries.find(t => t.type === 'page' && t.url?.includes('tauri.localhost'));
      if (target) break;
    } catch (_) {}
    await sleep(500);
  }
  assert.ok(target, 'Packaged Browsphere WebView2 chrome not detected');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.onopen=resolve; ws.onerror=reject; });
  let seq = 0;
  const pending = new Map();
  ws.onmessage = e => {
    const msg = JSON.parse(e.data);
    const slot = pending.get(msg.id);
    if (!slot) return;
    pending.delete(msg.id);
    msg.error ? slot.reject(Error(JSON.stringify(msg.error))) : slot.resolve(msg.result || {});
  };
  async function command(method, params={}) {
    const id = ++seq;
    return Promise.race([
      new Promise((resolve,reject) => { pending.set(id,{resolve,reject}); ws.send(JSON.stringify({id,method,params})); }),
      sleep(14000).then(() => { if (pending.has(id)) {pending.delete(id);throw Error('CDP timeout '+method);} })
    ]);
  }
  async function evalJS(code) {
    const r = await command('Runtime.evaluate',{expression:code,returnByValue:true,awaitPromise:true,userGesture:true});
    if(r.exceptionDetails) throw Error(JSON.stringify(r.exceptionDetails));
    return r.result?.value;
  }
  try {
    await command('Runtime.enable');
    await command('Page.enable');
    const native = await evalJS('window.aetherBridge?.isNative === true');
    assert.equal(native,true,'Native bridge must be available');
    const initial = await evalJS('(async()=>{const v=await window.aetherBridge.readLuciaDiagnostics(); return v;})()');
    assert.ok(initial.some(e => e.run_id === failed && e.action === 'click' && e.decision === 'deny'));
    assert.ok(initial.some(e => e.run_id === completed && e.status === 'completed'));
    assert.equal(JSON.stringify(initial).includes('NEVER-EXPOSE-SECRET'),false);
    assert.equal(await evalJS('!!window.LuciaDiagnosticsDashboard'),true,'Dashboard script loaded in real app');
    await evalJS("document.querySelector('[data-action=\"open-settings\"]').click()");
    await sleep(100);
    await evalJS("document.getElementById('settingsPaneBtn-ai').click()");
    await sleep(700);
    await evalJS("document.querySelector('[data-action=\"lucia-diag-filter\"][data-filter=\"all\"]').click()");
    const first = await evalJS('({open:!document.getElementById("browsphereSettingsModal").classList.contains("hidden"),heading:document.querySelector(\'[aria-label="Lucia Diagnostics"] .mac-ai-heading\')?.textContent,html:document.getElementById("luciaDiagnosticsPanel")?.innerHTML})');
    assert.equal(first.open,true);
    assert.equal(first.heading,'Lucia Diagnostics');
    assert.ok(first.html?.includes('11111111') && first.html?.includes('22222222'),'Both fixture runs visible');
    await evalJS("document.querySelector('[data-action=\"lucia-diag-filter\"][data-filter=\"issues\"]').click()");
    const issues = await evalJS('document.getElementById("luciaDiagnosticsPanel").innerHTML');
    assert.ok(issues.includes(failed.slice(0,8)));
    assert.equal(issues.includes('data-run-id="'+completed+'"'),false);
    assert.ok(issues.includes('Policy: deny'));
    assert.ok(issues.includes('Triggered by: user close'));
    assert.ok(issues.includes('Close type: intentional'));
    assert.equal(issues.includes('NOT-EXPOSED'),false);
    await evalJS("document.querySelector('[data-action=\"lucia-diag-filter\"][data-filter=\"completed\"]').click()");
    const done = await evalJS('document.getElementById("luciaDiagnosticsPanel").innerHTML');
    assert.ok(done.includes(completed));
    assert.equal(done.includes('data-run-id="'+failed+'"'),false);
    await evalJS("document.querySelector('[data-action=\"lucia-diag-filter\"][data-filter=\"issues\"]').click()");
    await evalJS("document.getElementById('luciaDiagnosticsPanel').scrollIntoView({block:'center',behavior:'instant'})");
    await sleep(350);
    fs.mkdirSync(artifactDir,{recursive:true});
    const shot = await command('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
    const file = path.join(artifactDir,'lucia-diagnostics-native.png');
    fs.writeFileSync(file,Buffer.from(shot.data,'base64'));
    console.log('PASS native dashboard: backend sanitization, modal, two runs, status filters, denied-action timeline');
    console.log('Screenshot:',file);
  } finally { ws.close(); }
}
main().catch(e => { console.error('FAIL native Lucia dashboard:',e.stack||e);process.exitCode=1; });
