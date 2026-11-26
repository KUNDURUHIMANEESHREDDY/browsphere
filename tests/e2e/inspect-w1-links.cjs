(async()=>{
 const port=Number(process.argv[2]||9398);
 const tabs=await (await fetch('http://127.0.0.1:'+port+'/json')).json();
 const main=tabs.find(x=>x.url.includes('tauri.localhost'));
 const ws=new WebSocket(main.webSocketDebuggerUrl);
 await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no});
 ws.onmessage=e=>{const r=JSON.parse(e.data); if(r.id!==1)return;
 console.log(r.result?.result?.value||JSON.stringify(r.error));ws.close()};
 ws.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{expression:`(async()=>{const s=await window.__TAURI__.core.invoke('capture_live_snapshot');return JSON.stringify({origin:s?.origin,title:s?.title,chars:s?.text_excerpt?.length,hasFree:s?.text_excerpt?.toLowerCase().includes("free"),hasPlan:s?.text_excerpt?.toLowerCase().includes("plan"),sample:s?.text_excerpt?.slice(0,180),links:(s?.interactive_nodes||[]).filter(n=>n.href).slice(0,100).map(n=>({label:n.label?.slice(0,100),href:n.href,role:n.role}))})})()`,awaitPromise:true,returnByValue:true}}));
 setTimeout(()=>ws.close(),6000).unref();
})().catch(console.error)