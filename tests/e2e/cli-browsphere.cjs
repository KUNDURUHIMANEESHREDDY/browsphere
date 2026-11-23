#!/usr/bin/env node
'use strict';
// CLI-Anything-style developer harness; independent implementation.
// Controls only the isolated Browsphere/WebView2 test environment via loopback.
// It does not provide remote device access or bypass browser approval rules.
const {spawnSync} = require('node:child_process');
const path = require('node:path');
const args = process.argv.slice(2);
const json = args.includes('--json');
const action = args.find(x=>!x.startsWith('--')) || 'help';
const cdpPort = Number(process.env.LUCIA_CDP_PORT || 9272);
const mockPort = Number(process.env.LUCIA_MODEL_PORT || 18766);
const fixturePort = Number(process.env.LUCIA_FIXTURE_PORT || 18764);

function output(data, ok=true) {
  if(json) console.log(JSON.stringify({ok, ...data}));
  else console.log(data.message || JSON.stringify(data,null,2));
  if(!ok) process.exitCode = 1;
}
async function request(url) {
  try {
    const r=await fetch(url,{signal:AbortSignal.timeout(3000)});
    return r.ok ? {ok:true, value:await r.json()} : {ok:false,code:r.status};
  } catch(e) { return {ok:false,error:String(e?.cause?.code || e.message || e)}; }
}
async function snapshot() {
  const targets=await request('http://127.0.0.1:'+cdpPort+'/json');
  const target=targets.value?.find?.(t=>t.type==='page' && t.url?.includes('tauri.localhost'));
  if(!target?.webSocketDebuggerUrl) throw Error('Native Browsphere testing window is not available');
  const ws=new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
  try {
    return await new Promise((resolve,reject)=>{
      const timeout=setTimeout(()=>reject(Error('Native CDP observation timed out')),6500);
      ws.onmessage=e=>{
        const m=JSON.parse(e.data);
        if(m.id!==1)return;
        clearTimeout(timeout);
        if(m.error || m.result?.exceptionDetails)return reject(Error('Native observation rejected'));
        resolve(m.result?.result?.value);
      };
      // Only trusted Tauri chrome receives the command. Do not export page
      // excerpts, form contents, cookies or personal data in CLI output.
      ws.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{
        expression:"(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return {origin:s.origin,title:s.title,interactiveControls:s.interactive_nodes.length}})()",
        returnByValue:true,awaitPromise:true
      }}));
    });
  } finally { ws.close(); }
}
async function main() {
  if(action==='help')return output({message:
    'Usage: node tests/e2e/cli-browsphere.cjs [--json] doctor|snapshot|test. Local isolated Windows test instance only.'});
  if(action==='doctor'){
    const [cdp,model,fixture]=await Promise.all([
      request('http://127.0.0.1:'+cdpPort+'/json'),
      request('http://127.0.0.1:'+mockPort+'/health'),
      request('http://127.0.0.1:'+fixturePort+'/health')]);
    const browser=Boolean(cdp.value?.some?.(t=>t.type==='page'&&t.url?.includes('tauri.localhost')));
    const ok=browser&&model.ok&&fixture.ok;
    return output({browserConnected:browser,modelConnected:model.ok,fixtureConnected:fixture.ok,
      message:ok?'Native Browsphere test instance and mock dependencies are ready':'Native test dependencies are not ready'},ok);
  }
  if(action==='snapshot'){
    const result=await snapshot();
    return output({snapshot:result,message:JSON.stringify(result)});
  }
  if(action==='test'){
    const program=path.join(__dirname,'lucia-multistep-mock.cjs');
    const result=spawnSync(process.execPath,[program],{
      encoding:'utf8',timeout:240000,env:process.env,windowsHide:true
    });
    return output({exitCode:result.status,stdout:(result.stdout||'').slice(-13000),
      stderr:(result.stderr||'').slice(-8000),error:result.error?.message||null,
      message:result.status===0?'Native multi-step integration PASS':'Native multi-step integration FAILED'},result.status===0);
  }
  throw Error('Unknown CLI command: '+action);
}
main().catch(e=>output({message:String(e.message||e)},false));
