(async()=>{
 const port=Number(process.argv[2]||9399), tabs=await(await fetch('http://127.0.0.1:'+port+'/json')).json();
 const tab=tabs.find(x=>x.type==='page'&&x.url.startsWith('https://www.bing.com/search'));
 if(!tab){console.log('No Bing result page');return}
 const ws=new WebSocket(tab.webSocketDebuggerUrl);
 await new Promise((yes,no)=>{ws.onopen=yes;ws.onerror=no});
 const js=`JSON.stringify({url:location.href,organic:Array.from(document.querySelectorAll('li.b_algo h2 a')).slice(0,12).map(a=>{const u=new URL(a.href);const token=u.searchParams.get('u');let host=null;try{if(token?.startsWith('a1')){const b=token.slice(2).replace(/-/g,'+').replace(/_/g,'/');host=new URL(atob(b.padEnd(Math.ceil(b.length/4)*4,'='))).hostname}}catch{}return {label:a.innerText.slice(0,65),path:u.pathname,host,tokenPrefix:token?.slice(0,2)||null}})})`;
 ws.onmessage=e=>{const r=JSON.parse(e.data); if(r.id!==1)return;console.log(r.result?.result?.value||JSON.stringify(r.error));ws.close()};
 ws.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{expression:js,returnByValue:true}}));
 setTimeout(()=>ws.close(),6000).unref();
})().catch(console.error)