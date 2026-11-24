'use strict';
const port=Number(process.argv[2]);
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const assert=(value,message)=>{if(!value)throw Error(message)};
async function main(){
    let page;
    for(let i=0;i<100;i++){
        try{
            const pages=await fetch('http://127.0.0.1:'+port+'/json',{signal:AbortSignal.timeout(1000)}).then(r=>r.json());
            page=pages.find(p=>p.type==='page'&&p.url.includes('tauri.localhost'));
            if(page)break;
        }catch{}
        await pause(300);
    }
    assert(page,'Native shell not found');
    const ws=new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no});
    let seq=0;
    const pending=new Map();
    ws.onmessage=e=>{const m=JSON.parse(e.data),p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.no(Error(JSON.stringify(m.error))):p.ok(m.result)}};
    async function evalJs(expression){
        const id=++seq;
        const result=await Promise.race([new Promise((ok,no)=>{pending.set(id,{ok,no});ws.send(JSON.stringify({id,method:'Runtime.evaluate',params:{expression,returnByValue:true,awaitPromise:true}}))}),pause(120000).then(()=>{if(pending.has(id)){pending.delete(id);throw Error('CDP timeout')}})]);
        if(result.exceptionDetails)throw Error(JSON.stringify(result.exceptionDetails));
        return result.result?.value;
    }
    try{
        let ready=false;
        for(let i=0;i<50;i++){if(await evalJs('Boolean(window.__TAURI__?.core?.invoke)')){ready=true;break}await pause(200)}
        assert(ready,'Native bridge missing');
        const config=await evalJs("window.__TAURI__.core.invoke('get_router_config')");
        assert(config.ollama_url.startsWith('http://127.0.0.1:')&&!config.ollama_url.endsWith(':11434'),'Private endpoint not selected');
        assert(config.ollama_model==='qwen3.5:0.8b'&&config.supervisor_model==='lfm3:1.7b'&&config.vision_model==='lfm3-vl:2b','Unexpected role config');
        assert(config.prefer_local_for_simple&&!config.lucia_cloud_only,'Local mode disabled');
        console.log('PASS: native app startup and role routing '+config.ollama_url);
        const models=await evalJs("window.__TAURI__.core.invoke('list_local_models')");
        for(const model of ['qwen3.5:0.8b','lfm3:1.7b','lfm3-vl:2b'])assert(models.includes(model),'Missing '+model);
        console.log('PASS: native router discovered all three models');
        const reply=await evalJs("window.__TAURI__.core.invoke('ask_main_tab_ai',{messages:[{role:'user',content:'Respond with exactly READY.'}],voiceMode:false})");
        assert(reply.model==='qwen3.5:0.8b'&&/READY/i.test(reply.answer),'Unexpected native inference: '+JSON.stringify(reply));
        console.log('PASS: native app generated '+reply.answer);
        console.log('ENDPOINT='+config.ollama_url);
        try{await evalJs("window.__TAURI__.core.invoke('window_close')")}catch{}
    }finally{ws.close()}
}
main().then(()=>process.exit(0),e=>{console.error(e);process.exit(1)});
