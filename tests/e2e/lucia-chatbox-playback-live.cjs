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
    console.log('CHATBOX_STAGE bridge_ready');
    await evaluate(`(async()=> {
      window.__chatPlayback={ launches:[],events:[] };
      const orig=window.aetherBridge.startLuciaSpecialistTask;
      window.aetherBridge.startLuciaSpecialistTask=function(...args) {
        const entry={goal:args[0],tab:args[3],runId:null};
        window.__chatPlayback.launches.push(entry);
        return Promise.resolve(orig.apply(this,args)).then(id=>{entry.runId=id;return id});
      };
      for (const kind of ['agent:trace','agent:run_error','agent:run_status'])
        await window.__TAURI__.event.listen(kind,e=>{
          // Keep the live acceptance probe bounded. Full native snapshots can
          // be enormous; retaining all of them in the UI can itself stall CDP.
          const p=e.payload||{};
          const payload={run_id:p.run_id,status:p.status,message:p.message,
            outcome:p.outcome,decision:p.decision,action:{type:p.action?.type}};
          const events=window.__chatPlayback.events;
          events.push({kind,payload});
          if(events.length>300)events.splice(0,events.length-300);
        });
      const c=await window.__TAURI__.core.invoke('get_router_config');
      c.prefer_local_for_simple=true;c.lucia_cloud_only=false;
      c.ollama_url='http://127.0.0.1:11434';
      c.ollama_model='qwen3.5:0.8b';c.planner_model='qwen2.5:3b-instruct';
      c.supervisor_model='lfm3:1.7b';c.vision_model='lfm3-vl:2b';
      await window.__TAURI__.core.invoke('update_router_config',{config:c});
      return true;
    })()`);
    console.log('CHATBOX_STAGE diagnostics_registered');
    let entered = false;
    for(let i=0;i<35;i++){
      entered=await evaluate(`(()=>{const el=document.getElementById('newTabChatInput');if(!el)return false;el.value='play any song';el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',code:'Enter',bubbles:true,cancelable:true}));return true})()`);
      if(entered)break;
      await sleep(350);
    }
    if(!entered) throw Error('Chatbox input not found');
    console.log('CHATBOX_STAGE input_submitted');
    let lastCount=0, finalStatus=null, runId=null, revealed=false;
    for (let i=0; i<145; i++) {
      await sleep(1000);
      const data=await evaluate(`({launches:window.__chatPlayback.launches,events:window.__chatPlayback.events.slice(-90)})`);
      if (data.launches.length) { runId=data.launches[0].runId;taskTab=data.launches[0].tab; }
      if(i===12&&!data.launches.length)throw Error('Chatbox did not dispatch a task within 13 seconds');
      if(i===30&&!runId)throw Error('Native task admission did not return a run ID within 31 seconds');
      const evs=data.events.filter(e=>e.payload?.run_id===runId);
      const traces=evs.filter(e=>e.kind==='agent:trace');
      // Diagnostic only: test whether hidden WebView2 throttles media loading.
      if(!revealed && process.env.LUCIA_TEST_REVEAL_TASK_TAB==='1' && taskTab &&
          traces.filter(e=>e.payload.action?.type==='navigate' && e.payload.outcome==='confirmed').length>=2){
        await evaluate('window.browsphereApp.switchTab('+JSON.stringify(taskTab)+')');
        revealed=true;
        console.log('REVEALED_TASK_TAB_FOR_MEDIA_DIAGNOSIS');
      }
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
    console.log('CHATBOX_PLAYBACK_RESULT',JSON.stringify({launches:final.launches,runId,status:finalStatus,failures,traces:evs.filter(e=>e.kind==='agent:trace').map(e=>({action:e.payload.action?.type,decision:e.payload.decision,outcome:e.payload.outcome}))}).slice(0,11000));
    if(final.launches.length!==1 || final.launches[0].goal!=='play any song' || !runId) throw Error('Chatbox did not dispatch exact request to Lucia');
    if(finalStatus!=='completed') throw Error('Native playback was not independently verified, status='+finalStatus);
    console.log('PASS: chatbox submitted and live playback verified');
  } finally {
    try {await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')")} catch {}
    if(taskTab) try {await evaluate('window.browsphereApp.closeTab('+JSON.stringify(taskTab)+')')} catch {}
    ws.close();
  }
}
main().catch(e=>{console.error('CHATBOX_PLAYBACK_FAIL',e.stack||String(e));process.exitCode=1;});
