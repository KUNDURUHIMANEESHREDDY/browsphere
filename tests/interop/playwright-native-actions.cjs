'use strict';
// Microsoft Playwright acting on ONE explicitly selected disposable WebView2
// fixture tab, with an independent Tauri native DOM readback after each action.
// Never use this harness on a user profile or use CDP as Lucia's action engine.
const assert=require('node:assert/strict');
const {chromium}=require('playwright-core');
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const cdp=Number(process.env.LUCIA_CDP_PORT);
const fixture=Number(process.env.LUCIA_FIXTURE_PORT);
if(process.env.GITHUB_ACTIONS!=='true'||
  !Number.isInteger(cdp)||cdp<9200||cdp>9900||
  !Number.isInteger(fixture)||fixture<1024||fixture>65535){
  throw Error('Installed WebView2 actions require a disposable CI profile');
}
const source=`http://127.0.0.1:${fixture}/generic-search.html`;
// Earlier MCP and Playwright smokes open more than one tab at 'source'.
// An arbitrary contexts().pages().find(p=>p.url()===source) may drive an
// INACTIVE copy and falsely test the wrong native tab. Create and bind a
// dedicated URL; it is still the identical local page and action workflow.
const dedicated=`${source}?upstream-playwright-action-test=1`;
(async()=>{
  const browser=await chromium.connectOverCDP(`http://127.0.0.1:${cdp}`);
  try{
    const shell=browser.contexts().flatMap(c=>c.pages())
      .find(p=>p.url().includes('tauri.localhost'));
    assert.ok(shell,'Native Tauri shell is missing');
    assert.ok(await shell.evaluate(()=>Boolean(window.browsphereApp?.newTab)),
      'The actual native browser tab manager is unavailable');
    const nativeRead=()=>shell.evaluate(()=>
      window.__TAURI__.core.invoke('capture_live_snapshot'));
    await shell.evaluate(url=>window.browsphereApp.newTab(url),dedicated);
    let task=null,initial=null;
    for(let i=0;i<60;i++){
      const pages=browser.contexts().flatMap(c=>c.pages());
      task=pages.find(p=>p.url()===dedicated)||null;
      initial=await nativeRead().catch(()=>null);
      if(task&&initial?.origin===dedicated&&
        initial.interactive_nodes?.some(n=>n.label==='Search documentation'))break;
      await sleep(200);
    }
    assert.ok(task,'Dedicated native WebView2 fixture target did not attach');
    assert.equal(initial?.origin,dedicated,
      'The shell native snapshot must show the SAME task tab as Playwright');
    assert.ok(initial.interactive_nodes.some(n=>n.label==='Search documentation'),
      'The native snapshot does not contain the expected observed field');
    const cases=[
      {name:'type',action:()=>task.locator('#docs-query').fill('systems'),
        match:'Typed query: systems', expectedUrl:dedicated},
      {name:'submit',action:()=>task.locator('#docs-query').press('Enter'),
        match:'Search results for: systems', expectedUrl:dedicated},
      {name:'open',action:()=>task.getByRole('link',{name:'Open API reference'}).click(),
        match:'GENERIC_DOCS_CONFIRMED',expectedUrl:`${source}?doc=api`}
    ];
    for(const step of cases){
      assert.equal(await nativeRead().then(r=>r.origin),dedicated,
        'Test tab was switched before an action; refusing to mutate another page');
      assert.equal(task.url(),dedicated,'Playwright target switched before action');
      await step.action();
      let readback=null;
      for(let i=0;i<35;i++){
        readback=await nativeRead().catch(()=>null);
        if(readback?.origin===step.expectedUrl&&
          readback.text_excerpt?.includes(step.match))break;
        await sleep(200);
      }
      // Diagnose only the known disposable fixture's state, not page content,
      // secrets or user URLs. Both sides are independently observed.
      if(readback?.origin!==step.expectedUrl||
          !readback?.text_excerpt?.includes(step.match)){
        const playwrightText=await task.locator('#status').textContent().catch(()=>null);
        console.error('PLAYWRIGHT_NATIVE_READBACK_DIAG '+JSON.stringify({
          step:step.name,
          playwrightFixtureStatusMatches:playwrightText?.includes(step.match)===true,
          playwrightFixtureUrlMatches:task.url()===step.expectedUrl,
          nativeFixtureUrlMatches:readback?.origin===step.expectedUrl,
          nativeExcerptMatches:readback?.text_excerpt?.includes(step.match)===true,
          nativeExcerptLength:readback?.text_excerpt?.length||0,
          nativeControlCount:readback?.interactive_nodes?.length||0
        }));
      }
      assert.equal(readback?.origin,step.expectedUrl,
        'Native snapshot is not the same exact test tab and route');
      assert.ok(readback.text_excerpt?.includes(step.match),
        'Official Playwright '+step.name+' failed independent native DOM verification');
    }
    const result=await nativeRead();
    assert.equal(result.origin,`${source}?doc=api`);
    assert.ok(result.text_excerpt.includes('GENERIC_DOCS_CONFIRMED'));
    console.log('PLAYWRIGHT_NATIVE_ACTIONS_VERIFIED '+JSON.stringify({
      upstream:'microsoft/playwright-core',nativeWebView2:true,
      independentlyReadBack:true,actions:cases.length,
      realQwenUsed:false,publicWebsiteTaskCompleted:false
    }));
  }finally{await browser.close();}
})().catch(e=>{console.error('PLAYWRIGHT_NATIVE_ACTIONS_FAILED '+e.message);process.exitCode=1;});
