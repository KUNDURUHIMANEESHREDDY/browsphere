(async()=>{
 const port=Number(process.argv[2]||9397);
 const tabs=await (await fetch('http://127.0.0.1:'+port+'/json')).json();
 const main=tabs.find(x=>x.url.includes('tauri.localhost'));
 if(!main){console.log('Native shell unavailable');return}
 const ws=new WebSocket(main.webSocketDebuggerUrl);
 await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject});
 let req=1;
 const cmd={id:req,method:'Runtime.evaluate',params:{expression:"JSON.stringify({active:window.browsphereApp?.activeTab?.()?.url,events:(window.__crossSiteEvents||[]).slice(-20).map(e=>({kind:e.kind,status:e.payload?.status,decision:e.payload?.decision,outcome:e.payload?.outcome,action:e.payload?.action?.type,stage:e.payload?.index,reason:e.payload?.reason,run_id:e.payload?.run_id}))})",returnByValue:true}};
 ws.send(JSON.stringify(cmd));
 ws.onmessage=e=>{let res=JSON.parse(e.data);if(res.id!==req)return;console.log(res.result?.result?.value||JSON.stringify(res.error));ws.close()};
 setTimeout(()=>ws.close(),5000).unref();
})().catch(e=>{console.error(String(e));process.exitCode=1});