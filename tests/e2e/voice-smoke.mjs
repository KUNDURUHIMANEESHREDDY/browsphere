import fs from 'node:fs';
const base='C:\\Users\\himan\\Downloads\\Browsphere\\tests\\e2e\\';
const logs=[], pending=new Map();
let seq=0,ws;
async function cmd(method,params={}){
 const id=++seq;
 return new Promise((resolve,reject)=>{
  pending.set(id,{resolve,reject});
  ws.send(JSON.stringify({id,method,params}));
  setTimeout(()=>{if(pending.has(id)){pending.delete(id);reject(new Error(method+' timed out'))}},14000);
 });
}
async function evalFn(fn){
 const v=await cmd('Runtime.evaluate',{expression:'('+fn.toString()+')()',returnByValue:true,awaitPromise:true});
 if(v.exceptionDetails)throw Error(JSON.stringify(v.exceptionDetails));
 return v.result?.value;
}
function pass(name,result,details=''){logs.push({name,passed:!!result,details});console.log((result?'PASS':'FAIL')+' '+name+' '+JSON.stringify(details).slice(0,280));}
async function shot(name){let a=await cmd('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});fs.writeFileSync(base+'artifacts\\'+name+'.png',Buffer.from(a.data,'base64'));}
async function run(){
 const targets=await(await fetch('http://127.0.0.1:9228/json')).json();
 const page=targets.find(t=>t.title?.includes('Browsphere')) || targets.find(t=>t.type==='page');
 if(!page)throw Error('Webview target missing');
 ws=new WebSocket(page.webSocketDebuggerUrl);await new Promise((res,rej)=>{ws.onopen=res;ws.onerror=rej;});
 ws.onmessage=e=>{
 const m=JSON.parse(e.data);
 if(pending.has(m.id)){let p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(m.error):p.resolve(m.result||{});}
 };
 await cmd('Runtime.enable');await cmd('Page.enable');
 await new Promise(r=>setTimeout(r,900));
 const head=await evalFn(()=>({voice:!!window.browsphereVoice,native:!!window.__TAURI__,main:!!document.getElementById('newTabChatForm'),talk:document.querySelectorAll('[data-browsphere-voice-talk]').length,mode:document.querySelectorAll('[data-browsphere-voice-mode]').length}));
 pass('Voice agent module loaded',head.voice&&head.native,head);
 pass('Talk controls next to chat',head.talk>0&&head.mode>0,head);
 const cfg=await evalFn(async()=>{let c=await window.__TAURI__.core.invoke('get_voice_config');return {provider:c.provider,allow_cloud_tts:c.allow_cloud_tts,speak_replies:c.speak_replies,voice_id:c.voice_id};});
 pass('ElevenLabs config loads',cfg.provider==='elevenlabs'&&!!cfg.voice_id,cfg);
 pass('Cloud voice opt-in is off by default',cfg.allow_cloud_tts===false,cfg);
 const saved=await evalFn(async()=>await window.__TAURI__.core.invoke('has_voice_api_key'));
 pass('Secure key storage command available',typeof saved==='boolean',{saved});
 const before=await evalFn(()=>{let b=document.querySelector('[data-browsphere-voice-mode]');const v=b?.getAttribute('aria-pressed');b?.click();let after=b?.getAttribute('aria-pressed');b?.click();return{v,after,restored:b?.getAttribute('aria-pressed')}});
 pass('Voice toggle works',before.after==='true'&&before.restored==='false',before);
 const panel=await evalFn(async()=>{document.querySelector('[aria-label="Voice settings"]')?.click();await new Promise(r=>setTimeout(r,350));const d=document.getElementById('browsphereVoiceSettings');return {open:d?.style.display==='flex',cloudCheckbox:!!document.getElementById('bvConsent'),keyInputType:document.getElementById('bvKey')?.type,keyStatus:document.getElementById('bvKeyStatus')?.textContent};});
 pass('Voice settings dialog opens',panel.open&&panel.cloudCheckbox&&panel.keyInputType==='password',panel);
 await shot('04-voice-settings');
 const nativeFailure=await evalFn(async()=>{try{await window.__TAURI__.core.invoke('synthesize_voice',{text:'',isPrivate:true});return false}catch(e){return String(e).includes('Speech input')}});
 pass('Speech input validation rejects empty text',nativeFailure);
 await evalFn(()=>{document.querySelector('#bvClose')?.click();});
 await shot('05-voice-chat');
 const results={date:new Date().toISOString(),checks:logs};
 fs.writeFileSync(base+'artifacts\\voice-results.json',JSON.stringify(results,null,2));
 console.log('SUMMARY',logs.filter(x=>x.passed).length,'/',logs.length);
 ws.close();process.exitCode=logs.every(x=>x.passed)?0:1;
}
run().catch(e=>{console.error('VOICE_TEST_ERROR',e.stack||e);if(ws)ws.close();process.exitCode=2;});
