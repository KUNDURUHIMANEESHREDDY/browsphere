import fs from 'node:fs';
let seq=0;let pending=new Map();let socket;
const delay=t=>new Promise(r=>setTimeout(r,t));
async function getTarget(urlContains){
 const list=await(await fetch('http://127.0.0.1:9228/json')).json();
 return list.find(t=>t.url?.includes(urlContains));
}
async function connect(target){
 const ws=new WebSocket(target.webSocketDebuggerUrl);
 await new Promise((res,rej)=>{ws.onopen=res;ws.onerror=rej});
 ws.onmessage=e=>{
  const m=JSON.parse(e.data);
  if(m.id&&pending.has(m.id)){let p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(m.error):p.resolve(m.result||{});}
 };
 socket=ws;return ws;
}
async function cmd(method,params={}){
 const id=++seq;
 return new Promise((resolve,reject)=>{pending.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}));setTimeout(()=>{if(pending.has(id)){pending.delete(id);reject(Error('timeout '+method));}},14000);});
}
async function runFn(fn){
 const r=await cmd('Runtime.evaluate',{expression:'('+fn.toString()+')()',returnByValue:true,awaitPromise:true});
 if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));
 return r.result?.value;
}
const checks=[];
function pass(label,ok,detail){checks.push({label,ok,detail});console.log((ok?'PASS ':'FAIL ')+label+' '+JSON.stringify(detail));}
async function main(){
 let target=await getTarget('/');
 if(!target)throw Error('main target missing');
 await connect(target);await cmd('Runtime.enable');
 const opened=await runFn(async()=>await window.__TAURI__.core.invoke('toggle_ai_chat_window',{forceOpen:true,title:'Voice Test',url:'https://docs.rs',dark:false,initialPrompt:null,isPrivate:true}));
 pass('Open floating Lucia window',opened);
 socket.close();pending=new Map();
 await delay(800);
 target=await getTarget('ai-chat.html');
 if(!target)throw Error('Floating window target not found');
 await connect(target);await cmd('Runtime.enable');
 const view=await runFn(()=>({voiceLoaded:!!window.browsphereVoice,talk:!!document.querySelector('[data-browsphere-voice-talk]'),private:document.documentElement.getAttribute('data-private'),textChat:!!document.getElementById('chatComposerForm')}));
 pass('Voice and chat share Lucia popup',view.voiceLoaded&&view.talk&&view.textChat,view);
 pass('Private mode inherited into popup',view.private==='true',view);
 await runFn(async()=>await window.__TAURI__.core.invoke('ai_chat_close'));
 socket.close();
 fs.writeFileSync('C:\\Users\\himan\\Downloads\\Browsphere\\tests\\e2e\\artifacts\\voice-popup-results.json',JSON.stringify({checks,at:new Date().toISOString()},null,2));
 console.log('SUMMARY',checks.filter(x=>x.ok).length,'/',checks.length);
 process.exitCode=checks.every(x=>x.ok)?0:1;
}
main().catch(e=>{console.error('POPUP_TEST_ERROR',e.stack||String(e));socket?.close();process.exitCode=2;});
