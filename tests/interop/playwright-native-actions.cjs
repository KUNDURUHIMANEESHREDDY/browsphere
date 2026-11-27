'use strict';
// Official Microsoft Playwright action engine against an isolated *installed*
// Browsphere WebView2 fixture; verifies results using a fresh native Tauri
// DOM snapshot. No custom CDP actuator, real user data or internet pages.
const assert=require('node:assert/strict');
const {chromium}=require('playwright-core');
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const cdp=Number(process.env.LUCIA_CDP_PORT);
const fixture=Number(process.env.LUCIA_FIXTURE_PORT);
if (process.env.GITHUB_ACTIONS!=='true' ||
    !Number.isInteger(cdp)||cdp<9200||cdp>9900 ||
    !Number.isInteger(fixture)||fixture<1024||fixture>65535) {
  throw Error('Installed WebView2 actions are allowed only in disposable CI');
}
const source=`http://127.0.0.1:${fixture}/generic-search.html`;
(async()=>{
 const browser=await chromium.connectOverCDP(`http://127.0.0.1:${cdp}`);
 try {
   const pages=browser.contexts().flatMap(c=>c.pages());
   const shell=pages.find(p=>p.url().includes('tauri.localhost'));
   const task=pages.find(p=>p.url()===source);
   assert.ok(shell,'Native Tauri shell is missing');
   assert.ok(task,'Lucia must have opened the source fixture in a real WebView2 tab');
   const nativeRead=()=>shell.evaluate(()=>window.__TAURI__.core.invoke('capture_live_snapshot'));
   const before=await nativeRead();
   assert.equal(before?.origin,source,'Native task tab is not the observed fixture');
   const cases=[
     {name:'type',action:()=>task.locator('#docs-query').fill('systems'),match:'Typed query: systems'},
     {name:'submit',action:()=>task.locator('#docs-query').press('Enter'),match:'Search results for: systems'},
     {name:'open',action:()=>task.getByRole('link',{name:'Open API reference'}).click(),match:'GENERIC_DOCS_CONFIRMED'},
   ];
   for(const test of cases) {
     assert.ok(task.url().startsWith(source),'External navigation or wrong tab refused');
     await test.action();
     let readback;
     for(let i=0;i<18;i++){
       readback=await nativeRead();
       if(readback?.origin?.startsWith(source) && readback?.text_excerpt?.includes(test.match))break;
       await sleep(250);
     }
     assert.ok(readback?.origin?.startsWith(source),'Native readback origin changed');
     assert.ok(readback?.text_excerpt?.includes(test.match),
       'Official Playwright '+test.name+' failed native independent verification');
   }
   const result=await nativeRead();
   assert.ok(result.origin.includes('?doc=api'),'Expected API route absent');
   assert.ok(result.text_excerpt.includes('GENERIC_DOCS_CONFIRMED'),'Native final evidence missing');
   console.log('PLAYWRIGHT_NATIVE_ACTIONS_VERIFIED '+JSON.stringify({
     upstream:'microsoft/playwright-core',
     nativeWebView2:true,
     independentlyReadBack:true,
     actions:cases.length,
     realQwenUsed:false,
     publicWebsiteTaskCompleted:false
   }));
 } finally {await browser.close();}
})().catch(e=>{console.error('PLAYWRIGHT_NATIVE_ACTIONS_FAILED '+e.message);process.exitCode=1;});
