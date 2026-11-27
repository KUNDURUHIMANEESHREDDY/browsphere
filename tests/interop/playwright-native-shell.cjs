'use strict';

// Upstream Playwright WebView2 CDP attach. Never visits a third-party page,
// reads user profiles, or starts a new browser. Restricted to disposable CI.
const assert=require('node:assert/strict');
const {chromium}=require('playwright-core');
const {localCdpEndpoint}=require('./playwright-ci-policy.cjs');
const endpoint=localCdpEndpoint(process.env.LUCIA_CDP_PORT,process.env.GITHUB_ACTIONS);
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
(async()=>{
  const browser=await chromium.connectOverCDP(endpoint,{timeout:20000});
  try {
    let target;
    for(let i=0;i<40;i++){
      target=browser.contexts().flatMap(c=>c.pages())
        .find(p=>p.url().includes('tauri.localhost'));
      if(target && await target.locator('body').count().catch(()=>0))break;
      await delay(250);
    }
    assert.ok(target,'No real Tauri WebView2 shell observed');
    const ready=await target.evaluate(()=>Boolean(
      window.__TAURI__?.core?.invoke && window.aetherBridge?.startLuciaSpecialistTask
    ));
    assert.equal(ready,true,'Native shell not ready for Lucia chatbox');
    console.log('PLAYWRIGHT_NATIVE_SHELL_PASS '+JSON.stringify({
      attached:true,tauri_shell:true,lucia_task_bridge:true,
      adapter:'microsoft/playwright-core'
    }));
  } finally {
    await browser.close(); // Disconnect CDP client, never close the application.
  }
})().catch(error=>{console.error('PLAYWRIGHT_NATIVE_SHELL_FAILED '+error.message);process.exitCode=1;});
