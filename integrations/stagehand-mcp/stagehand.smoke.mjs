// Uses the real upstream Stagehand SDK and an ephemeral browser.
// Does not exercise Lucia or the installed WebView2 executable.
import {createServer} from 'node:http';
import assert from 'node:assert/strict';
import {stagehandRead} from './stagehand-readonly.mjs';

const html='<!doctype html><html><head><title>Browsphere Upstream Smoke</title></head><body><main><h1>Stagehand fixture</h1></main></body></html>';
const http=createServer((req,res)=>{
  res.writeHead(200,{'Content-Type':'text/html; charset=utf-8','Cache-Control':'no-store'});
  res.end(html);
});
try {
  await new Promise((resolve,reject)=>{
    http.once('error',reject);
    http.listen(0,'127.0.0.1',resolve);
  });
  const port=http.address().port;
  const result=await stagehandRead({
    url:`http://127.0.0.1:${port}/fixture`,
    instruction:'Read the page title',
    mode:'inspect',
  },{ci:true,fixturePort:String(port)});
  assert.equal(result.title,'Browsphere Upstream Smoke');
  assert.equal(result.verified,false);
  assert.equal(new URL(result.url).hostname,'127.0.0.1');
  console.log('Real Stagehand isolated-browser inspection succeeded; no Lucia/WebView2 claim');
} finally {
  await new Promise((resolve)=>http.close(resolve));
}
