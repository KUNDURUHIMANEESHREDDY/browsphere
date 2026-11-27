'use strict';
// Read-only acceptance check for a native WebView2 CDP screencast frame stream.
// No screenshot or frame bytes are returned to this Node client.
const assert=require('node:assert/strict');
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const port=Number(process.env.LUCIA_CDP_PORT||9290);
async function main(){
  assert.equal(process.env.GITHUB_ACTIONS,'true','Live vision test requires disposable Windows CI');
  let targets;
  for(let i=0;i<50;i++){
    try {targets=await(await fetch('http://127.0.0.1:'+port+'/json')).json();break}
    catch {await sleep(350)}
  }
  assert.ok(targets,'Native WebView2 CDP endpoint missing');
  const shell=targets.find(t=>t.type==='page'&&t.url?.includes('tauri.localhost')&&t.title?.includes('Browsphere'))
      ||targets.find(t=>t.type==='page'&&t.url?.includes('tauri.localhost'));
  assert.ok(shell?.webSocketDebuggerUrl,'Native Browsphere app shell missing');
  const ws=new WebSocket(shell.webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject});
  let id=0;const pending=new Map();
  ws.onmessage=e=>{
    const value=JSON.parse(e.data);
    if(value.id&&pending.has(value.id)){
      const req=pending.get(value.id);pending.delete(value.id);
      value.error?req.reject(Error('CDP returned an error')):req.resolve(value.result||{});
    }
  };
  const cdp=(method,params={})=>new Promise((resolve,reject)=>{
    const seq=++id;
    pending.set(seq,{resolve,reject});
    ws.send(JSON.stringify({id:seq,method,params}));
    setTimeout(()=>{
      if(pending.has(seq)){pending.delete(seq);reject(Error('Native CDP test command timed out'))}
    },20000);
  });
  const evalMain=async expr=>{
    const out=await cdp('Runtime.evaluate',{
      expression:expr,returnByValue:true,awaitPromise:true,userGesture:true
    });
    assert.equal(out.exceptionDetails,undefined,'Trusted app command failed');
    return out.result?.value;
  };
  try{
    await cdp('Runtime.enable');
    const ready=await evalMain("({tauri:!!window.__TAURI__,title:document.title})");
    assert.equal(ready.tauri,true,'Target is not native Browsphere app shell');
    await evalMain("window.__TAURI__.core.invoke('open_or_navigate_tab',{tabId:'live-visual-fixture',url:'http://127.0.0.1:18764/',isPrivate:true})");
    let status={};
    for(let i=0;i<60;i++){
      await sleep(150);
      status=await evalMain("window.__TAURI__.core.invoke('get_brpr_status')");
      if(status?.cached&&!status?.dirty)break;
    }
    assert.ok(status?.cached,'BRPR never observed isolated visual fixture');
    const resultData=await evalMain("window.__TAURI__.core.invoke('run_live_visual_smoke')");
    assert.equal(resultData.changed,true,'Live CDP frames did not reflect a DOM visual change');
    assert.equal(resultData.brpr_signaled,true,'BRPR did not signal that visual change');
    assert.ok(resultData.first_frame_chars>1000&&resultData.second_frame_chars>1000,
      'Live frames were unexpectedly empty');
    console.log('LUCIA_LIVE_VISUAL_STREAM_PASS',JSON.stringify({
      native_webview2:true,real_cdp_screencast:true,brpr_change_detected:true,
      frames_changed:true,first_frame_ms:resultData.first_frame_ms,
      second_frame_ms:resultData.second_frame_ms
    }));
    await evalMain("window.__TAURI__.core.invoke('close_tab_webview',{tabId:'live-visual-fixture'})");
  }finally{ws.close()}
}
main().catch(err=>{
  console.error('LUCIA_LIVE_VISUAL_STREAM_FAIL',String(err?.stack||err).slice(0,1600));
  process.exitCode=1;
});
