// Actual official Stagehand v4 act() in its supported isolated Chromium,
// with direct post-action browser readback. Never run against the user's
// Windows WebView2 profile or untrusted account pages.
import {createServer} from 'node:http';
import assert from 'node:assert/strict';
import {localBrowser,Stagehand} from '@browserbasehq/stagehand';
import {createOllamaModel} from './model-provider.mjs';

const html='<!doctype html><html><head><title>Stagehand Action Fixture</title></head><body><main><button id="advance" type="button">Advance documentation</button><output id="status">Not complete</output></main><script>document.getElementById("advance").addEventListener("click",()=>document.getElementById("status").textContent="STAGEHAND_ACT_CONFIRMED")</script></body></html>';
const http=createServer((req,res)=>{
  res.writeHead(200,{'Content-Type':'text/html; charset=utf-8','Cache-Control':'no-store'});
  res.end(html);
});
let browser,stagehand;
try {
  await new Promise((resolve,reject)=>{
    http.once('error',reject);
    http.listen(0,'127.0.0.1',resolve);
  });
  browser=await localBrowser.launch({headless:true,ignoreHTTPSErrors:false,acceptDownloads:false});
  stagehand=await Stagehand.create({
    browser,
    model:createOllamaModel({model:'qwen2.5:3b-instruct'}),
    selfHeal:false,
  });
  const page=await browser.context.activePage();
  assert.ok(page,'Upstream Stagehand browser has no page');
  const source=`http://127.0.0.1:${http.address().port}/`;
  await page.goto(source);
  assert.equal(await page.url(),source,'Isolated fixture URL changed');
  const result=await stagehand.act({
    selector:'xpath=//button[@id="advance"]',
    description:'Advance documentation using the observed button',
    method:'click',
    arguments:[],
  },{timeout:20000});
  assert.equal(result?.data?.success,true,'Official Stagehand act() did not execute');
  const post=await page.evaluate(()=>document.querySelector('#status')?.textContent);
  assert.equal(post,'STAGEHAND_ACT_CONFIRMED','Stagehand action lacked independent DOM readback');
  console.log('STAGEHAND_ISOLATED_ACTION_VERIFIED '+JSON.stringify({
    upstream:'@browserbasehq/stagehand',
    actionExecuted:true,
    independentDomReadback:true,
    browser:'isolated-chromium',
    nativeWebView2Attached:false,
    realModelUsed:false,
  }));
} finally {
  try{await stagehand?.close()}catch{}
  try{await browser?.close()}catch{}
  await new Promise(resolve=>http.close(resolve));
}
