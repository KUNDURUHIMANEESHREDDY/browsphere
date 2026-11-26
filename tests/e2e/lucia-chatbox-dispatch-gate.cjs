'use strict';
// Actual chatbox dispatch acceptance. The final native task handoff is mocked
// so this checks chat -> native intent router -> task dispatch, not execution.
// A PASS never implies the downstream browser/MCP task was completed.
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const port=Number(process.env.LUCIA_CDP_PORT||9658);
const scenarios=[
  ['S1','Find three free Python courses suitable for beginners from different learning platforms. Check whether they provide free certificates, compare their duration and recommend the best one.'],
  ['W1','Visit the official websites of Notion, Trello and ClickUp. Compare their free plans for a five-member team and recommend the most suitable option.'],
  ['R1','Research productivity tools using official sources and write a report with citations.'],
  ['P1','Plan a Sunday outing in Bengaluru for a family of four under ₹2,000. Find three family-friendly attractions, check opening hours and ticket prices, and prepare an itinerary.'],
  ['C1','Research an upcoming conference and prepare a calendar event with verified time and venue.']
];
async function main(){
  let target;
  for(let n=0;n<60;n++){
    try{
      const pages=await (await fetch('http://127.0.0.1:'+port+'/json',{signal:AbortSignal.timeout(2500)})).json();
      target=pages.find(p=>p.type==='page'&&p.url?.includes('tauri.localhost'));
      if(target)break;
    }catch{}
    await sleep(300);
  }
  if(!target)throw Error('No native Browsphere UI exposed over CDP');
  const ws=new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject});
  const waiting=new Map();let id=0;
  ws.onmessage=e=>{const d=JSON.parse(e.data),p=waiting.get(d.id);
    if(p){waiting.delete(d.id);d.error?p.reject(Error(JSON.stringify(d.error))):p.resolve(d.result)}};
  async function evaluate(expression){
    const key=++id;
    const result=await Promise.race([
      new Promise((resolve,reject)=>{waiting.set(key,{resolve,reject});ws.send(JSON.stringify({
        id:key,method:'Runtime.evaluate',params:{expression,awaitPromise:true,returnByValue:true,userGesture:true}}))}),
      sleep(10000).then(()=>{if(waiting.has(key)){waiting.delete(key);throw Error('CDP call timeout')}})
    ]);
    if(result.exceptionDetails)throw Error(JSON.stringify(result.exceptionDetails));
    return result.result?.value;
  }
  try{
    let ok=false;
    for(let retry=0;retry<40;retry++){
      ok=await evaluate(`(()=>{
      if(!window.aetherBridge?.isNative||!window.browsphereApp?.newTab)return false;
      window.__dispatchTest={goals:[]};
      window.__dispatchTest.original=window.aetherBridge.startLuciaSpecialistTask;
      window.aetherBridge.startLuciaSpecialistTask=async function(goal){
        window.__dispatchTest.goals.push(goal);
        return '11111111-1111-4111-8111-111111111111';
      };
      return true
    })()`);
      if(ok)break;
      await sleep(200);
    }
    if(!ok)throw Error('Native Browsphere UI is unavailable');
    for(const [key,prompt] of scenarios){
      await evaluate('window.browsphereApp.newTab()');
      let entered=false;
      for(let n=0;n<30;n++){
        entered=await evaluate(`(()=>{const el=document.getElementById('newTabChatInput');
          if(!el)return false;
          el.value=${JSON.stringify(prompt)};
          document.getElementById('newTabChatForm').requestSubmit();
          return true})()`);
        if(entered)break;
        await sleep(150);
      }
      if(!entered)throw Error(key+': chatbox not found');
      let launched=false;
      for(let n=0;n<50;n++){
        await sleep(150);
        const goals=await evaluate('window.__dispatchTest.goals');
        launched=goals.includes(prompt);
        if(launched)break;
      }
      console.log('CHATBOX_DISPATCH',key,launched?'PASS':'FAIL');
      if(!launched)throw Error(key+': did not reach browser-task handoff');
    }
    console.log('CHATBOX_DISPATCH_GATE_PASSED (routing and UI dispatch only; downstream execution unverified)');
    // Separate UI-only event-order test: a completed status may arrive before
    // its independently verified report. Neither event is a live task receipt.
    const runId='11111111-1111-4111-8111-111111111111';
    await evaluate(`window.dispatchEvent(new CustomEvent('aether:agent-status',{detail:{run_id:'${runId}',status:'completed'}}));true`);
    await sleep(70);
    await evaluate(`window.dispatchEvent(new CustomEvent('aether:agent-verification',{detail:{run_id:'${runId}',report:{status:'evidence_checked',summary:'CHATBOX_RACE_VERIFIED'}}}));true`);
    await sleep(200);
    const message=await evaluate('document.body.innerText');
    if(!message.includes('CHATBOX_RACE_VERIFIED'))throw Error('Delayed verification was lost after terminal status');
    console.log('CHATBOX_EVENT_ORDER PASS (synthetic event ordering only)');
  }finally{
    try{await evaluate('(()=>{if(window.__dispatchTest?.original)window.aetherBridge.startLuciaSpecialistTask=window.__dispatchTest.original;return true})()')}catch{}
    ws.close();
  }
}
main().catch(e=>{console.error('CHATBOX_DISPATCH_FAIL',e.stack||String(e));process.exitCode=1});
