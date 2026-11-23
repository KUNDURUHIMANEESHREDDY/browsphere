'use strict';
// Native Windows regression: closing Lucia's assigned background tab stops only
// that task and reports a meaningful reason. Requires the isolated mock harness.
const assert = require('node:assert/strict');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const port = process.env.LUCIA_CDP_PORT || '9272';

async function main() {
  const targets = await (await fetch('http://127.0.0.1:' + port + '/json')).json();
  const target = targets.find(t => t.type === 'page' && t.url?.includes('tauri.localhost'));
  assert.ok(target?.webSocketDebuggerUrl, 'Isolated Browsphere chrome WebView missing');
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let id = 0;
  const pending = new Map();
  socket.onmessage = event => {
    const result = JSON.parse(event.data);
    const task = pending.get(result.id);
    if (!task) return;
    pending.delete(result.id);
    result.error ? task.reject(Error(JSON.stringify(result.error))) : task.resolve(result.result || {});
  };
  async function evaluate(expression) {
    const key = ++id;
    const answer = await Promise.race([
      new Promise((resolve, reject) => {
        pending.set(key, {resolve, reject});
        socket.send(JSON.stringify({id:key, method:'Runtime.evaluate', params:{
          expression, returnByValue:true, awaitPromise:true, userGesture:true
        }}));
      }),
      sleep(16000).then(() => {
        if (pending.has(key)) { pending.delete(key); throw Error('CDP evaluation timed out'); }
      })
    ]);
    if (answer.exceptionDetails) throw Error(JSON.stringify(answer.exceptionDetails));
    return answer.result?.value;
  }
  try {
    let shellReady = false;
    for (let i = 0; i < 60; i++) {
      shellReady = await evaluate('Boolean(window.browsphereApp?.newTab && window.aetherBridge?.isNative)').catch(() => false);
      if (shellReady) break;
      await sleep(300);
    }
    assert.ok(shellReady, 'Native Browsphere UI never became ready');
    await evaluate(`(async () => {
      window.__tabCloseEvents = [];
      for (const name of ['agent:plan','agent:run_status','agent:run_error']) {
        await window.__TAURI__.event.listen(name, event =>
          window.__tabCloseEvents.push({name, payload:event.payload}));
      }
      await window.__TAURI__.core.invoke('set_agent_paused', {paused:true});
      window.browsphereApp.newTab('browsphere://start');
      const input = document.getElementById('newTabChatInput');
      if (!input) throw Error('Missing Lucia new-tab composer');
      input.value = 'open youtube';
      document.querySelector('[data-action="nt-run-agent"]').click();
      return true;
    })()`);
    let taskTab, runId;
    for (let i = 0; i < 60; i++) {
      const view = await evaluate(`({
        taskTab:document.querySelector('.main-chat-agent-progress [data-action="nt-view-task-tab"]')?.dataset.tabId,
        plan:window.__tabCloseEvents.find(event=>event.name==='agent:plan')?.payload?.run_id,
        status:window.__tabCloseEvents.find(event=>event.name==='agent:run_status')?.payload?.status
      })`);
      taskTab = view.taskTab;
      runId = view.plan;
      assert.ok(!view.status, 'Task unexpectedly completed while paused');
      if (taskTab && runId) break;
      await sleep(300);
    }
    assert.match(taskTab || '', /^tab-lucia-/, 'Dedicated Lucia task tab was not created');
    assert.ok(runId, 'Agent did not start before lifecycle check');
    const chatId = await evaluate('window.browsphereApp.activeTab().id');
    await evaluate("window.browsphereApp.newTab('browsphere://start')");
    const foregroundId = await evaluate('window.browsphereApp.activeTab().id');
    assert.notEqual(chatId, foregroundId, 'Failed to switch user to a separate tab');
    assert.notEqual(taskTab, foregroundId, 'Assigned background tab became foreground');
    // Navigate the unrelated foreground tab, switch to the task tab and back,
    // then verify that neither ordinary navigation nor focus transfer closed it.
    await evaluate("window.browsphereApp.navigate('http://127.0.0.1:18764/recovery.html')");
    await sleep(400);
    await evaluate('window.browsphereApp.switchTab(' + JSON.stringify(taskTab) + ')');
    await evaluate('window.browsphereApp.switchTab(' + JSON.stringify(foregroundId) + ')');
    await sleep(350);
    const stillPending = await evaluate(`window.__tabCloseEvents.some(event =>
      event.name==='agent:run_status' && event.payload?.run_id===${JSON.stringify(runId)})`);
    assert.equal(stillPending, false, 'Switching or navigating unrelated tabs stopped the paused agent');
    assert.equal(await evaluate(`window.__tabCloseEvents.some(event =>
      event.name==='agent:run_error' && event.payload?.run_id===${JSON.stringify(runId)})`), false,
      'Ordinary navigation emitted an agent close or failure error');
    const clickResult = await evaluate(`(() => {
      const close = document.querySelector('[data-action="close-tab"][data-tab-id="${taskTab}"]');
      if (!close) throw Error('Lucia task-tab close button missing');
      close.click();
      return true;
    })()`);
    assert.equal(clickResult, true);
    let finalStatus;
    for (let i = 0; i < 60; i++) {
      finalStatus = await evaluate(`window.__tabCloseEvents.find(event =>
        event.name==='agent:run_status' && event.payload?.run_id===${JSON.stringify(runId)})?.payload?.status`);
      if (finalStatus) break;
      await sleep(300);
    }
    assert.equal(finalStatus, 'tab_closed', 'Closing the assigned tab did not stop its task explicitly');
    const closeSource = await evaluate(`window.__tabCloseEvents.find(event =>
      event.name==='agent:run_error' && event.payload?.run_id===${JSON.stringify(runId)} &&
      event.payload?.source==='user_close')?.payload?.source`);
    assert.equal(closeSource, 'user_close', 'Native task closure did not identify the user close operation');
    assert.equal(await evaluate('window.browsphereApp.activeTab().id'), foregroundId,
      'Closing task tab unexpectedly moved the foreground browser tab');
    await evaluate(`document.querySelector('[data-action="switch-tab"][data-tab-id="${chatId}"]').click()`);
    await sleep(350);
    const message = await evaluate('document.getElementById("mainChatMessages")?.innerText || ""');
    assert.match(message, /(?:Lucia stopped because its assigned task tab was closed|Task stopped because its assigned browser tab was closed)/i);
    console.log('PASS: unrelated tab switch kept task alive; closing its assigned tab cancelled it with a clear chat result');
  } finally {
    await evaluate("window.__TAURI__.core.invoke('set_agent_paused', {paused:false})").catch(() => {});
    socket.close();
  }
}
main().catch(error => { console.error('FAIL', error.stack || error); process.exitCode = 1; });
