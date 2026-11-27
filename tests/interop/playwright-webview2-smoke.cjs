'use strict';

// Uses Microsoft's Playwright implementation, not a homegrown CDP client.
// Read-only attach to the actual Tauri/WebView2 browser executable that the
// NSIS installer placed on a separate, disposable Windows CI runner.
const assert = require('node:assert/strict');
const {chromium} = require('playwright-core');
const {localCdpEndpoint} = require('./playwright-ci-policy.cjs');

const endpoint = localCdpEndpoint(process.env.LUCIA_CDP_PORT, process.env.GITHUB_ACTIONS);
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

(async () => {
  const browser = await chromium.connectOverCDP(endpoint, {timeout: 20000});
  try {
    let page;
    for (let attempt = 0; attempt < 35; attempt++) {
      // No interaction with ordinary pages or other browser installations:
      // select only Browsphere's own Tauri shell target.
      page = browser.contexts().flatMap(context => context.pages())
        .find(candidate => candidate.url().includes('tauri.localhost'));
      if (page && await page.evaluate(() =>
        Boolean(window.browsphereApp?.newTab && window.aetherBridge?.isNative)
      ).catch(() => false)) break;
      page = null;
      await delay(250);
    }
    assert.ok(page, 'Playwright could not attach to the actual native Tauri shell');
    assert.ok(await page.locator('body').count(), 'Native WebView2 DOM missing');

    // The Tauri shell starts before any document tab is necessarily active.
    // Open only the loopback fixture already started by the Windows harness;
    // then verify readback from that child WebView, not an empty home view.
    const fixturePort = Number(process.env.LUCIA_FIXTURE_PORT);
    assert.ok(Number.isInteger(fixturePort) && fixturePort >= 1024 &&
      fixturePort <= 65535, 'Isolated fixture port missing');
    const fixtureUrl = 'http://127.0.0.1:' + fixturePort + '/generic-search.html';
    await page.evaluate(url => window.browsphereApp.newTab(url), fixtureUrl);
    let snapshot = null;
    for (let attempt = 0; attempt < 50; attempt++) {
      snapshot = await page.evaluate(() =>
        window.__TAURI__.core.invoke('capture_live_snapshot')
      ).catch(() => null);
      if (snapshot?.origin === fixtureUrl &&
          Array.isArray(snapshot?.interactive_nodes) &&
          snapshot.interactive_nodes.length > 0) break;
      await delay(300);
    }
    assert.equal(snapshot?.origin, fixtureUrl,
      'Native readback did not reach the observed isolated fixture URL');
    assert.ok(Array.isArray(snapshot.interactive_nodes) &&
      snapshot.interactive_nodes.length > 0,
      'Observed fixture contains no native interactive controls');
    // Report only readiness classifications, never URL, DOM text or secrets.
    console.log('PLAYWRIGHT_WEBVIEW2_REUSE_PASS ' + JSON.stringify({
      native_shell: true,
      live_snapshot: true,
      controls_observed: snapshot.interactive_nodes.length,
      cdp_library: 'microsoft/playwright-core'
    }));
  } finally {
    // A CDP client disconnect; no page closures, user-profile writes or
    // browser-process termination.
    await browser.close();
  }
})().catch(error => {
  console.error('PLAYWRIGHT_WEBVIEW2_REUSE_FAILED: ' + error.message);
  process.exitCode = 1;
});
