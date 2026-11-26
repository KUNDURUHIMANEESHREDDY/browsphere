'use strict';
// End-to-end smoke: enter a natural-language instruction into the actual
// home chatbox, then require a finished native media-verification result.
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const port = Number(process.env.LUCIA_CDP_PORT || 9398);
async function main() {
  let target;
  for (let i = 0; i < 90; i++) {
    try {
      const pages = await (await fetch('http://127.0.0.1:' + port + '/json')).json();
      target = pages.find(p => p.type === 'page' && p.url?.includes('tauri.localhost'));
      if (target) break;
    } catch {}
    await sleep(400);
  }
  if (!target) throw Error('No native app CDP tab');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((ok, no) => { ws.onopen = ok; ws.onerror = no; });
  const waiting = new Map(); let seq = 0;
  ws.onmessage = e => {
    const d = JSON.parse(e.data), p = waiting.get(d.id);
    if (p) { waiting.delete(d.id); d.error ? p.no(Error(JSON.stringify(d.error))) : p.ok(d.result); }
  };
  async function evaluate(expression) {
    const id = ++seq;
    const response = await Promise.race([
      new Promise((ok,no) => { waiting.set(id, {ok,no}); ws.send(JSON.stringify({id,method:'Runtime.evaluate',params:{expression,awaitPromise:true,returnByValue:true,userGesture:true}})); }),
      sleep(20000).then(() => { if (waiting.has(id)) { waiting.delete(id); throw Error('CDP evaluation timed out'); }})
    ]);
    if (response.exceptionDetails) throw Error(JSON.stringify(response.exceptionDetails));
    return response.result?.value;
  }
  let taskTab;
  try {
    let bridgeReady=false;
    for(let i=0;i<70;i++){
      bridgeReady=await evaluate("Boolean(window.aetherBridge?.startLuciaSpecialistTask && window.__TAURI__?.core?.invoke && document.getElementById('newTabChatInput'))").catch(()=>false);
      if(bridgeReady)break;
      await sleep(350);
    }
    if(!bridgeReady)throw Error('Lucia frontend bridge/chatbox did not become ready');
    await evaluate(`(async()=> {
      window.__chatPlayback={ launches:[],events:[] };
      const orig=window.aetherBridge.startLuciaSpecialistTask;
      window.aetherBridge.startLuciaSpecialistTask=function(...args) {
        const entry={goal:args[0],tab:args[3],runId:null};
        window.__chatPlayback.launches.push(entry);
        return Promise.resolve(orig.apply(this,args)).then(id=>{entry.runId=id;return id});
      };
      for (const kind of ['agent:trace','agent:run_error','agent:run_status','agent:verification_report'])
        await window.__TAURI__.event.listen(kind,e=>window.__chatPlayback.events.push({kind,payload:e.payload}));
      const c=await window.__TAURI__.core.invoke('get_router_config');
      c.prefer_local_for_simple=true;c.lucia_cloud_only=false;
      c.ollama_url='http://127.0.0.1:11434';
      c.ollama_model='qwen2.5:3b-instruct';c.planner_model='qwen2.5:3b-instruct';
      c.supervisor_model='lfm3:1.7b';c.vision_model='lfm3-vl:2b';
      await window.__TAURI__.core.invoke('update_router_config',{config:c});
      return true;
    })()`);
    let entered = false;
    for(let i=0;i<35;i++){
      entered=await evaluate(`(()=>{const el=document.getElementById('newTabChatInput');if(!el)return false;el.value='Plan a Sunday outing in Bengaluru for a family of four under INR 2000. Find three family-friendly attractions, check their opening hours and ticket prices, and prepare an itinerary.';el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',code:'Enter',bubbles:true,cancelable:true}));return true})()`);
      if(entered)break;
      await sleep(350);
    }
    if(!entered) throw Error('Chatbox input not found');
    let lastCount=0, finalStatus=null, runId=null;
    for (let i=0; i<255; i++) {
      await sleep(1000);
      const data=await evaluate(`({launches:window.__chatPlayback.launches,events:window.__chatPlayback.events.slice(-90)})`);
      if (data.launches.length) { runId=data.launches[0].runId;taskTab=data.launches[0].tab; }
      const evs=data.events.filter(e=>e.payload?.run_id===runId);
      const traces=evs.filter(e=>e.kind==='agent:trace');
      if (traces.length>lastCount) {
        lastCount=traces.length;
        console.log('CHATBOX_PROGRESS',JSON.stringify(traces.slice(-2).map(e=>({action:e.payload.action?.type,outcome:e.payload.outcome,decision:e.payload.decision}))).slice(0,650));
      }
      const ended=evs.find(e=>e.kind==='agent:run_status');
      if(ended){finalStatus=ended.payload.status;break;}
    }
    const final=await evaluate('window.__chatPlayback');
    const evs=final.events.filter(e=>e.payload?.run_id===runId);
    const failures=evs.filter(e=>e.kind==='agent:run_error').map(e=>e.payload.message);
    const verified=evs.filter(e=>e.kind==='agent:verification_report'&&e.payload?.report?.status==='evidence_checked').at(-1)?.payload?.report;
    const supported=new Set((verified?.findings||[]).map(f=>{
      try{const host=new URL(f.url).hostname;return (host.startsWith('www.')?host.slice(4):host).split('.')[0]}catch{return ''}
    }));
    const sourceCheck=(verified?.findings||[]).length>=3 && supported.size>=3;
    const recommendationCheck=/itinerary/i.test(verified?.summary||'') && /total/i.test(verified?.summary||'') && /(?:₹|INR|Rs[.])/i.test(verified?.summary||'');
    console.log('CHATBOX_P1_RESULT',JSON.stringify({launches:final.launches,runId,status:finalStatus,failures,traces:evs.filter(e=>e.kind==='agent:trace').map(e=>({action:e.payload.action?.type,decision:e.payload.decision,outcome:e.payload.outcome}))}).slice(0,11000));
    if(final.launches.length!==1 || final.launches[0].goal!=='Plan a Sunday outing in Bengaluru for a family of four under INR 2000. Find three family-friendly attractions, check their opening hours and ticket prices, and prepare an itinerary.' || !runId) throw Error('Chatbox did not dispatch exact request to Lucia');
    if(finalStatus!=='completed') throw Error('Native P1 outing was not independently verified, status='+finalStatus);
    if(!verified||!sourceCheck||!recommendationCheck) throw Error('No source-backed P1 itinerary report with three supported attractions');
    console.log('P1_VERIFICATION_RECEIPT',JSON.stringify({status:verified.status,findings:verified.findings.length,vendors:[...supported],recommendation:verified.summary.slice(0,170)}));
    console.log('PASS: chatbox P1 outing task completed and verified');
  } finally {
    try {await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')")} catch {}
    if(taskTab) try {await evaluate('window.browsphereApp.closeTab('+JSON.stringify(taskTab)+')')} catch {}
    ws.close();
  }
}
main().catch(e=>{console.error('CHATBOX_P1_FAIL',e.stack||String(e));process.exitCode=1;});
