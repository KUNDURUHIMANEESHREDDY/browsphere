'use strict';
const fs=require('node:fs');
const path=require('node:path');
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const ENDPOINT='http://127.0.0.1:9258/json';
async function main(){
  let targets;
  for(let i=0;i<30;i++){
    try { targets=await(await fetch(ENDPOINT)).json(); break; }
    catch { await sleep(500); }
  }
  if(!targets) throw new Error('Native Browsphere CDP port 9258 unavailable');
  const mainTarget=targets.find(t=>t.type==='page'&&t.url?.includes('tauri.localhost')&&t.title?.includes('Browsphere'))
    ||targets.find(t=>t.type==='page'&&t.url?.includes('tauri.localhost'))
    ||targets.find(t=>t.type==='page'&&t.title?.includes('Browsphere'));
  if(!mainTarget) throw Error('Main native app view missing; '+JSON.stringify(targets.map(t=>({title:t.title,url:t.url}))));
  const ws=new WebSocket(mainTarget.webSocketDebuggerUrl);
  await new Promise((res,rej)=>{ ws.onopen=res; ws.onerror=rej; });
  let next=0;
  const pending=new Map();
  ws.onmessage=e=>{
    const m=JSON.parse(e.data);
    if(m.id&&pending.has(m.id)){
      const p=pending.get(m.id);pending.delete(m.id);
      m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result||{});
    }
  };
  const cmd=(method,params={})=>new Promise((resolve,reject)=>{
    const id=++next;
    pending.set(id,{resolve,reject});
    ws.send(JSON.stringify({id,method,params}));
    setTimeout(()=>{if(pending.has(id)){pending.delete(id);reject(Error('CDP '+method+' timed out'));}},180000);
  });
  const ev=async(expression)=>{
    const ret=await cmd('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});
    if(ret.exceptionDetails) throw Error(JSON.stringify(ret.exceptionDetails));
    return ret.result?.value;
  };
  try {
    await cmd('Runtime.enable');
    const ready=await ev("({tauri:!!window.__TAURI__,title:document.title})");
    console.log('APP',ready);
    if(!ready?.tauri) throw Error('No native Tauri API on CDP target');
    await ev("window.__TAURI__.core.invoke('open_or_navigate_tab',{tabId:'brpr-e2e',url:'http://127.0.0.1:18764/',isPrivate:true})");
    let status={};
    for(let i=0;i<35;i++){
      await sleep(250);
      status=await ev("window.__TAURI__.core.invoke('get_brpr_status')");
      if(status.cached&&!status.dirty)break;
    }
    console.log('PRE_BENCH',status);
    if(!status.cached)throw Error('BRPR cache did not populate after native WebView2 navigation');
    const report=await ev("window.__TAURI__.core.invoke('run_brpr_live_benchmark',{iterations:100})");
    console.log('LIVE_BRPR_REPORT',JSON.stringify(report,null,2));
    const output=path.join(__dirname,'brpr-live-results.json');
    fs.writeFileSync(output,JSON.stringify({at:new Date().toISOString(),app:ready,status,report},null,2));
    if(report.failed>0 || report.samples.length!==100) throw Error('Incomplete BRPR live benchmark: '+report.failed+' failed');
    console.log('PASS Native WebView2 -> BRPR signal -> cache refreshed; 100 samples');
    await ev("window.__TAURI__.core.invoke('close_tab_webview',{tabId:'brpr-e2e'})");
  } finally { ws.close(); }
}
main().catch(e=>{console.error('FAIL',e.stack||e);process.exitCode=1});