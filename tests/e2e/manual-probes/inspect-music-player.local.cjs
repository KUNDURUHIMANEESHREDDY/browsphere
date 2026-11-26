'use strict';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
(async()=>{
const pages=await(await fetch('http://127.0.0.1:9398/json')).json();
const page=pages.find(p=>p.type==='page'&&p.url?.startsWith('https://www.youtube.com/watch'));
if(!page){console.log('NO_WATCH_PAGE');return}
const ws=new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no});
let seq=0;const waits=new Map();
ws.onmessage=e=>{const m=JSON.parse(e.data),p=waits.get(m.id);if(p){waits.delete(m.id);m.error?p.no(m.error):p.ok(m.result)}};
async function ev(expression){const id=++seq;const promise=new Promise((ok,no)=>waits.set(id,{ok,no}));ws.send(JSON.stringify({id,method:'Runtime.evaluate',params:{expression,returnByValue:true}}));return (await promise).result?.value;}
const expr="(()=>({title:document.title,video:[...document.querySelectorAll('video')].map(x=>({paused:x.paused,ready:x.readyState,ended:x.ended,time:x.currentTime,muted:x.muted,network:x.networkState})),playButtons:[...document.querySelectorAll('button')].filter(x=>/play|pause/i.test(x.getAttribute('aria-label')||'')).slice(0,12).map(x=>({label:x.getAttribute('aria-label'),disabled:x.disabled}))}))()";
const first=await ev(expr);await sleep(1500);const second=await ev(expr);
console.log('MEDIA_STATE',JSON.stringify({first,second}));
ws.close();
})().catch(e=>{console.error('CDP_MEDIA_ERROR',String(e));process.exitCode=1});
