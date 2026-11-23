// Native WebView2 integration probe. Run only against the isolated test instance.
// Requires --remote-debugging-port=9246 and a local fixture on port 18764.
'use strict';
const assert = require('node:assert/strict');
const port = Number(process.env.LUCIA_CDP_PORT || 9246);
const fixture = 'http://127.0.0.1:18764/';
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function main() {
  const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
  const mainTarget = targets.find(x => x.title?.includes('Browsphere') && x.type === 'page');
  assert.ok(mainTarget, 'Main Browsphere WebView2 target must exist');
  const ws = new WebSocket(mainTarget.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  let seq=0; const pending=new Map();
  ws.onmessage = (event) => {
    const msg=JSON.parse(event.data);
    if (msg.id && pending.has(msg.id)) {
      const {resolve,reject} = pending.get(msg.id); pending.delete(msg.id);
      msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result || {});
    }
  };
  async function cmd(method, params={}) {
    const id=++seq;
    return Promise.race([
      new Promise((resolve,reject)=> {pending.set(id,{resolve,reject}); ws.send(JSON.stringify({id,method,params}));}),
      sleep(14000).then(()=>{ if(pending.has(id)){pending.delete(id);throw new Error('CDP command timed out: '+method)} })
    ]);
  }
  async function ev(expression) {
    const result=await cmd('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails).slice(0,700));
    return result.result?.value;
  }
  try {
    await cmd('Runtime.enable');
    const app = await ev('({tauri: !!window.__TAURI__?.core?.invoke, url: location.href, title:document.title})');
    assert.equal(app.tauri,true,'Tauri invoke must be exposed in main window');
    const config=await ev('(async()=>await window.__TAURI__.core.invoke("get_router_config"))()').catch(e=>({error:String(e)}));
    console.log('MAIN',JSON.stringify(app));
    console.log('ROUTER',JSON.stringify(config));
    const opened=await ev(`(async()=>{try {await window.__TAURI__.core.invoke("open_or_navigate_tab",{tabId:"lucia-native-test",url:"${fixture}",bounds:null,isPrivate:true});return "opened";}catch(e){return "ERROR "+String(e)}})()`);
    console.log('OPEN', opened);
    assert.equal(opened,'opened','Opening test website in WebView should succeed');
    await sleep(2500);
    const newer = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
    console.log('TARGETS',JSON.stringify(newer.map(x=>({title:x.title,url:x.url,type:x.type}))));
    let snapshot;
    for (let i=0;i<3;i++) {
      snapshot=await ev('(async()=>await window.__TAURI__.core.invoke("capture_live_snapshot"))()').catch(e=>({error:String(e)}));
      if(snapshot?.interactive_nodes?.length) break;
      await sleep(900);
    }
    console.log('SNAPSHOT', JSON.stringify(snapshot && {origin:snapshot.origin,title:snapshot.title,text:snapshot.text_excerpt?.slice(0,180),nodes:snapshot.interactive_nodes?.map(n=>({id:n.node_id,tag:n.tag,label:n.label,submit:n.is_submit})) ,error:snapshot.error}));
    assert.ok(snapshot && snapshot.origin?.startsWith(fixture), 'Native live DOM snapshot must be received from child WebView');
    assert.ok(snapshot.interactive_nodes?.length>=2,'Fixture interactive controls should be observed');
    const details=snapshot.interactive_nodes.find(n=>n.label.includes('Show fixture details'));
    const submit=snapshot.interactive_nodes.find(n=>n.label.includes('Submit sample application'));
    const feedback=snapshot.interactive_nodes.find(n=>n.label==='Submit feedback');
    assert.equal(details?.is_submit,false,'Ordinary button must not require form submission approval');
    assert.equal(submit?.is_submit,true,'Real submit must be recognized');
    assert.equal(submit?.is_sensitive,true,'Consequential application form must be marked sensitive');
    assert.equal(feedback?.is_submit,true,'Routine feedback submission must be recognized');
    assert.equal(feedback?.is_sensitive,false,'Routine feedback must remain eligible for autonomous submission');
    console.log('PASS native DOM snapshot, routine/critical form classification');
  } finally {ws.close();}
}
main().catch(err=>{console.error('FAIL',err?.stack||err); process.exitCode=1;});
