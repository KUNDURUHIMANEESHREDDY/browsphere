'use strict';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const port=Number(process.env.LUCIA_DIAG_PORT||9397);
async function pages(){return await(await fetch('http://127.0.0.1:'+port+'/json')).json();}
async function attach(p){
 const ws=new WebSocket(p.webSocketDebuggerUrl);await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no});
 let id=0;const pending=new Map();const events=[];
 ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.no(Error(JSON.stringify(m.error))):p.ok(m.result);}}else if(m.method)events.push(m);};
 async function cmd(method,params={}){const seq=++id;return Promise.race([new Promise((ok,no)=>{pending.set(seq,{ok,no});ws.send(JSON.stringify({id:seq,method,params}));}),sleep(12000).then(()=>{if(pending.has(seq)){pending.delete(seq);throw Error('CDP timeout '+method)}})]);}
 async function ev(expression){const r=await cmd('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result?.value;}
 return {ws,cmd,ev,events};
}
(async()=>{
 let main;for(let i=0;i<60;i++){main=(await pages()).find(p=>p.type==='page'&&p.url?.includes('tauri.localhost'));if(main)break;await sleep(400);}
 if(!main)throw Error('No Tauri CDP page');
 const m=await attach(main);
 let ready=false;for(let i=0;i<40;i++){ready=await m.ev('Boolean(window.browsphereApp?.navigate)').catch(()=>false);if(ready)break;await sleep(350);}
 if(!ready)throw Error('No UI bridge');
 await m.ev("window.browsphereApp.navigate('https://www.youtube.com/watch?v=SIpKxdDEXso')");
 let video;for(let i=0;i<70;i++){video=(await pages()).find(p=>p.type==='page'&&p.url?.includes('youtube.com/watch'));if(video)break;await sleep(400);}
 if(!video)throw Error('No watch page opened');
 const v=await attach(video);await v.cmd('Network.enable');
 const expr=`(()=>({media:[...document.querySelectorAll('video')].map(x=>({paused:x.paused,ready:x.readyState,time:x.currentTime,network:x.networkState,error:x.error?.code||0,hasSrc:!!x.currentSrc})),buttons:[...document.querySelectorAll('button')].filter(x=>/play|pause/i.test(x.getAttribute('aria-label')||'')).slice(0,5).map(x=>x.getAttribute('aria-label')),visibility:document.visibilityState,ready:document.readyState}))()`;
 for(let i=0;i<5;i++){await sleep(4000);console.log('MEDIA_PROBE',i,JSON.stringify(await v.ev(expr)).slice(0,600));}
 const responses=v.events.filter(x=>x.method==='Network.responseReceived').map(x=>({type:x.params?.type,status:x.params?.response?.status,host:(()=>{try{return new URL(x.params.response.url).hostname}catch{return ''}})()}));
 const fails=v.events.filter(x=>x.method==='Network.loadingFailed').map(x=>({type:x.params?.type,error:x.params?.errorText,blocked:x.params?.blockedReason}));
 console.log('NETWORK_STATS',JSON.stringify({responses:responses.filter(x=>x.type==='Media'||/googlevideo|youtube/.test(x.host)).slice(-40),fails:fails.slice(-35)}).slice(0,7000));
 v.ws.close();m.ws.close();
})().catch(e=>{console.error('MEDIA_DIAG_FAIL',e.stack||String(e));process.exitCode=1;});
