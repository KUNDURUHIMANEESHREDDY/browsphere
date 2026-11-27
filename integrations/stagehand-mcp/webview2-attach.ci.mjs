// CI-only upstream CDP-attach compatibility experiment. Reuse Stagehand's
// official localBrowser.connect() instead of implementing another CDP client.
// This never sends an action, changes navigation, or accesses user profiles.
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
let stagehand;
try {
  const browser=await localBrowser.connect({cdpUrl});
  const pages=await browser.context.pages();
  assert.ok(pages.length > 0,'Upstream Stagehand could not enumerate WebView2 pages');
  // Never assume the active page is a browser task: WebView2 also has a
  // privileged tauri.localhost UI. Only verify read-only connectivity.
  const urls=await Promise.all(pages.map(page=>page.url()));
  const shellIndex=urls.findIndex(url=>String(url).includes('tauri.localhost'));
  assert.ok(shellIndex>=0,'Stagehand did not see installed Tauri/WebView2 shell');
  stagehand=await Stagehand.create({
    browser,
    model:createOllamaModel({model:'qwen2.5:3b-instruct'}),
  });
  const page=pages[shellIndex];
  const title=await page.title();
  assert.equal(typeof title,'string');
  console.log('UPSTREAM_STAGEHAND_WEBVIEW2_ATTACH_PASS '+JSON.stringify({
    installed_webview2:true,
    attached_via_official_stagehand:true,
    page_count:pages.length,
    read_only:true,
    action_or_extraction_tested:false,
    completed_task:false,
  }));
} finally {
  // The browser is owned by the installed-app harness. Do not close its
  // WebView2 process or profile from the Stagehand client.
  try { await stagehand?.close(); } catch {}
}
