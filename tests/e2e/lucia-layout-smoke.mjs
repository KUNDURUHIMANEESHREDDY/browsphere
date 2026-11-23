import fs from 'node:fs';
const root = 'C:\\Users\\himan\\Downloads\\Browsphere\\tests\\e2e\\artifacts\\';
let socket,seq=0;const pend=new Map();
async function cmd(method,params={}){let id=++seq;return new Promise((res,rej)=>{pend.set(id,{res,rej});socket.send(JSON.stringify({id,method,params}));setTimeout(()=>{if(pend.has(id)){pend.delete(id);rej(new Error('Timeout: '+method));}},10000);});}
async function expr(fn) {const r=await cmd('Runtime.evaluate',{expression:'('+fn.toString()+')()',returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw new Error(JSON.stringify(r.exceptionDetails));return r.result?.value;}
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const outcomes=[];
function check(label,cond,details){outcomes.push({label,passed:!!cond,details});console.log((cond?'PASS':'FAIL')+' '+label+' '+JSON.stringify(details).slice(0,300));}
async function run(){
 const targets=await(await fetch('http://127.0.0.1:9231/json')).json();
 const t=targets.find(x=>x.url?.endsWith('/ui/public/ai-chat.html')||x.url?.includes('ai-chat.html'))||targets.find(x=>x.type==='page');
 if(!t)throw new Error('No Edge page target');
 socket=new WebSocket(t.webSocketDebuggerUrl);
 await new Promise((res,rej)=>{socket.onopen=res;socket.onerror=rej;});
 socket.onmessage=e=>{const m=JSON.parse(e.data);if(m.id&&pend.has(m.id)){const c=pend.get(m.id);pend.delete(m.id);m.error?c.rej(m.error):c.res(m.result||{});}};
 await cmd('Runtime.enable');await cmd('Page.enable');
 await cmd('Emulation.setDeviceMetricsOverride',{width:396,height:540,deviceScaleFactor:1,mobile:false});
 await sleep(800);
 const state=await expr(()=>{
  const rect=id=>{const r=document.getElementById(id)?.getBoundingClientRect();return r?{x:Math.round(r.x),right:Math.round(r.right),width:Math.round(r.width),y:Math.round(r.y)}:null;};
  return {innerWidth,clientWidth:document.documentElement.clientWidth,bodyScrollWidth:document.body.scrollWidth,
    shell:rect('aiChatHeader'),title:document.getElementById('emptyHeading')?.textContent,
    tagline:document.querySelector('.lucia-welcome p')?.textContent,
    send:rect('btnSend'),newThread:rect('btnNew'),mic:rect('btnVoice'),
    composer:rect('chatComposerForm'),orb:document.querySelector('.lucia-orb')?.getBoundingClientRect().x,
    chips:document.querySelectorAll('[data-chip]').length,
    hasPause:!!document.getElementById('btnPause'),hasStop:!!document.getElementById('btnStop'),
    voiceHandler:!!window.browsphereVoice,
    aria:document.getElementById('btnVoice')?.getAttribute('aria-label'),
    agentButton:rect('btnVoiceAgent'),voiceOrb:rect('btnVoiceOrb')
  };
 });
 check('Lucia heading and new caption',state.title==='Lucia'&&state.tagline==='Your AI browsing partner.',state);
 check('No old controls or suggested chips',!state.hasPause&&!state.hasStop&&state.chips===0,state);
 check('Mic is dedicated to dictating and sending a query',
   state.mic&&state.aria==='Dictate and send a query'&&state.voiceHandler,state);
 check('Separate Voice Agent waveform and orb buttons',
   state.agentButton&&state.voiceOrb,state);
 check('Mic, send and new-thread buttons fit popup',state.send?.right<=396&&state.newThread?.right<=396&&state.mic?.right<=396,state);
 check('No horizontal overflow',state.bodyScrollWidth<=state.clientWidth,{scroll:state.bodyScrollWidth,client:state.clientWidth});
 const mode=await expr(()=>{
   const header=document.getElementById('btnVoiceAgent');
   const orb=document.getElementById('btnVoiceOrb');
   const mic=document.getElementById('btnVoice');
   const micPressedBefore=mic.hasAttribute('aria-pressed');
   const initial=window.browsphereVoice.enabled;
   header.click();
   const afterHeader=window.browsphereVoice.enabled;
   const waveformPressed=header.getAttribute('aria-pressed');
   const orbPressed=orb.getAttribute('aria-pressed');
   orb.click();
   return {initial,afterHeader,afterOrb:window.browsphereVoice.enabled,
      waveformPressed,orbPressed,micHasModeState:mic.hasAttribute('aria-pressed'),micPressedBefore};
 });
 check('Waveform activates Voice Agent independently from microphone',
   mode.initial===false&&mode.afterHeader===true&&mode.waveformPressed==='true'&&
   mode.orbPressed==='true'&&mode.afterOrb===false&&!mode.micHasModeState,mode);
 const gear=await expr(async()=>{document.getElementById('btnVoiceSettings').click();await new Promise(r=>setTimeout(r,80));return{visible:document.getElementById('browsphereVoiceSettings')?.style.display,hasKey:!!document.getElementById('bvKey')};});
 check('Voice settings remain reachable',gear.visible==='flex'&&gear.hasKey,gear);
 await expr(()=>document.getElementById('bvClose')?.click());
 const shot=await cmd('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
 fs.writeFileSync(root+'lucia-final-396.png',Buffer.from(shot.data,'base64'));
 fs.writeFileSync(root+'lucia-layout-results.json',JSON.stringify({state,outcomes},null,2));
 console.log('SUMMARY '+outcomes.filter(o=>o.passed).length+'/'+outcomes.length);
 socket.close();
 process.exitCode=outcomes.every(o=>o.passed)?0:1;
}
run().catch(e=>{console.error('LUCIA_UI_TEST_ERROR',e.stack||e);socket?.close();process.exitCode=2;});
