// CI-only upstream Stagehand browser-attachment probe. Do not duplicate CDP.
// Upstream localBrowser.connect() works on an installed WebView2 test profile.
// Full Stagehand.create() is an INDEPENDENT experimental extension handshake:
// https://github.com/browserbase/stagehand/issues/2782 tracks its 60s timeout.
// This probe never visits user sites, sends an action, or reads private data.
import assert from 'node:assert/strict';
import {localBrowser, Stagehand} from '@browserbasehq/stagehand';
import {createOllamaModel} from './model-provider.mjs';

if (process.env.GITHUB_ACTIONS !== 'true' ||
    process.env.BROWSPHERE_STAGEHAND_WEBVIEW2_CI !== '1') {
  throw Error('Stagehand/WebView2 attach is only allowed in isolated Windows CI');
}
const port=Number(process.env.LUCIA_CDP_PORT);
if (!Number.isInteger(port) || port < 9200 || port > 9900) {
  throw Error('A dedicated ephemeral WebView2 loopback CDP port is required');
}
const cdpUrl=`http://127.0.0.1:${port}`;
const requireFullInit=process.env.BROWSPHERE_STAGEHAND_INIT_PROBE==='1';
let browser,stagehand;
try {
  browser=await localBrowser.connect({cdpUrl});
  const pages=await browser.context.pages();
  assert.ok(pages.length > 0,'Upstream Stagehand browser client did not enumerate WebView2 pages');
  // Only metadata on Browsphere's trusted UI shell; no action on user tabs.
  const urls=await Promise.all(pages.map(page=>page.url()));
  const shellIndex=urls.findIndex(url=>String(url).includes('tauri.localhost'));
  assert.ok(shellIndex>=0,'Upstream Stagehand browser client did not find installed WebView2 shell');
  const title=await pages[shellIndex].title();
  assert.equal(typeof title,'string');

  // The exact upstream browser connector and read-only page client worked.
  // This does NOT imply Stagehand.observe(), extract() or act() works inside
  // native WebView2. Those are separately tested on isolated Chromium via MCP.
  console.log('UPSTREAM_STAGEHAND_WEBVIEW2_CONNECT_PASS '+JSON.stringify({
    installed_webview2:true,
    official_localBrowser_connect:true,
    native_shell_readable:true,
    page_count:pages.length,
    full_stagehand_initialized:false,
    task_completed:false,
  }));
  if(requireFullInit){
    // Hard failure if requested: never turn an unsuccessful extension startup
    // into a green full-Stagehand acceptance claim.
    console.log('UPSTREAM_STAGEHAND_WEBVIEW2_FULL_INIT_PROBE_START');
    stagehand=await Stagehand.create({
      browser,
      model:createOllamaModel({model:'qwen2.5:3b-instruct'}),
    });
    console.log('UPSTREAM_STAGEHAND_WEBVIEW2_FULL_INIT_PASS');
  }
} finally {
  // Official Stagehand SDK v4.4+ browser.disconnect() releases the client
  // without sending Browser.close to the *installed* WebView2 application.
  try{await stagehand?.close();}catch{}
  try{await browser?.disconnect();}catch{}
}
