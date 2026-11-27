// Official Stagehand v4 action engine on a DISPOSABLE Windows CI WebView2
// fixture, followed by independent native Browsphere DOM snapshot readback.
// This is NOT enabled for end-user profiles, real sites, or regular builds.
// Stagehand itself executes the actions: no custom CDP/browser actuator.
import assert from 'node:assert/strict';
import {localBrowser,Stagehand} from '@browserbasehq/stagehand';
import {createOllamaModel} from './model-provider.mjs';

if (process.env.GITHUB_ACTIONS !== 'true' ||
    process.env.BROWSPHERE_STAGEHAND_WEBVIEW2_CI !== '1') {
  throw Error('Official Stagehand mutating actions require ephemeral Windows CI');
}
const cdpPort=Number(process.env.LUCIA_CDP_PORT);
const fixturePort=Number(process.env.LUCIA_FIXTURE_PORT);
if (!Number.isInteger(cdpPort)||cdpPort<9200||cdpPort>9900||
    !Number.isInteger(fixturePort)||fixturePort<1024||fixturePort>65535) {
  throw Error('CI-specific CDP and fixture ports must be supplied');
}
const fixtureUrl=`http://127.0.0.1:${fixturePort}/generic-search.html`;
let stagehand;
try {
  const browser=await localBrowser.connect({cdpUrl:`http://127.0.0.1:${cdpPort}`});
  const pages=await browser.context.pages();
  const entries=await Promise.all(pages.map(async page=>({page,url:await page.url()})));
  const shell=entries.find(x=>x.url.startsWith('http://tauri.localhost') ||
    x.url.startsWith('https://tauri.localhost'));
  const target=entries.find(x=>x.url===fixtureUrl);
  assert.ok(shell,'The actual installed Tauri shell must be available');
  assert.ok(target,'Lucia must open the live fixture tab before Stagehand action');
  // Scope every mutation to the single disposable fixture page. Never call
  // act() on tauri.localhost, user profiles, account pages, or external URLs.
  await browser.context.setActivePage(target.page);
  stagehand=await Stagehand.create({
    browser,
    model:createOllamaModel({model:'qwen2.5:3b-instruct'}),
    selfHeal:false,
  });
  const current=await browser.context.activePage();
  assert.equal(await current?.url(),fixtureUrl,'Wrong Stagehand action target');
  async function nativeState() {
    return shell.page.evaluate(async()=>window.__TAURI__.core.invoke('capture_live_snapshot'));
  }
  const original=await nativeState();
  assert.equal(original?.origin,fixtureUrl,'Native snapshot was not the fixture tab');
  const actions=[
    {
      name:'fill-search',
      selector:'xpath=//input[@id="docs-query"]',
      method:'fill',
      args:['systems'],
      expected:'Typed query: systems',
    },
    {
      name:'submit-search',
      selector:'xpath=//input[@id="docs-query"]',
      method:'press',
      args:['Enter'],
      expected:'Search results for: systems',
    },
    {
      name:'open-result',
      selector:'xpath=//a[normalize-space()="Open API reference"]',
      method:'click',
      args:[],
      expected:'GENERIC_DOCS_CONFIRMED',
    },
  ];
  for(const step of actions) {
    assert.equal(await current.url(),step.name==='open-result'?fixtureUrl:fixtureUrl,
      'Target unexpectedly navigated away before action');
    const result=await stagehand.act({
      selector:step.selector,
      description:step.name,
      method:step.method,
      arguments:step.args,
    },{timeout:15000});
    assert.equal(result?.data?.success,true,
      'Official Stagehand action failed: '+step.name);
    // Only the installed native app can certify the actual task tab state.
    // Stagehand's success flag alone is NOT evidence of action completion.
    let snapshot;
    for(let tries=0;tries<15;tries++){
      snapshot=await nativeState();
      if(snapshot?.origin.startsWith(fixtureUrl)&&
          snapshot?.text_excerpt?.includes(step.expected))break;
      await new Promise(resolve=>setTimeout(resolve,250));
    }
    assert.ok(snapshot?.origin.startsWith(fixtureUrl),
      'Native WebView2 task tab changed origin unexpectedly');
    assert.ok(snapshot.text_excerpt.includes(step.expected),
      'Native WebView2 readback does not confirm '+step.name);
  }
  const final=await nativeState();
  assert.ok(final.origin.includes('?doc=api'),'Observed result URL was not updated');
  assert.ok(final.text_excerpt.includes('GENERIC_DOCS_CONFIRMED'));
  console.log('STAGEHAND_NATIVE_ACTIONS_VERIFIED '+JSON.stringify({
    upstream:'@browserbasehq/stagehand',
    installedWebView2:true,
    actions:actions.length,
    independentlyReadBack:true,
    fixtureOnly:true,
    liveSiteTaskComplete:false,
    genuineModelUsed:false
  }));
} finally {
  // Only disconnect the official Stagehand client; installed-Browsphere
  // lifecycle and its ephemeral WebView2 profile belong to the CI harness.
  try{await stagehand?.close()}catch{}
}
