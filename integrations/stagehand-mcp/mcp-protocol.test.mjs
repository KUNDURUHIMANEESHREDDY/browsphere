import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {setTimeout as sleep} from 'node:timers/promises';
import {Client,StreamableHTTPClientTransport} from '@modelcontextprotocol/client';

async function freePort(){
  const server=createServer();
  await new Promise((ok,bad)=>server.once('error',bad).listen(0,'127.0.0.1',ok));
  const port=server.address().port;
  await new Promise(ok=>server.close(ok));
  return port;
}
async function serveFixture(){
  const server=createServer((req,res)=>{
    res.writeHead(200,{'Content-Type':'text/html; charset=utf-8'});
    res.end('<!doctype html><title>Stagehand Upstream Fixture</title><button id="research">Research</button>');
  });
  await new Promise((ok,bad)=>server.once('error',bad).listen(0,'127.0.0.1',ok));
  return server;
}
async function connectOnPort(port){
  let last;
  for(let i=0;i<35;i++){
    const client=new Client({name:'browsphere-upstream-integration-test',version:'0.1.0'});
    try{
      await client.connect(new StreamableHTTPClientTransport(new URL('http://127.0.0.1:'+port+'/mcp')));
      return client;
    }catch(e){last=e;await client.close().catch(()=>{});await sleep(150)}
  }
  throw Error('Official MCP client could not negotiate with Stagehand provider: '+String(last?.message).slice(0,200));
}
test('official MCP v2 server discovers only explicit read-only Stagehand tools and rejects unsafe sites',async()=>{
  const p=await freePort(),fixture=await serveFixture(),fixturePort=fixture.address().port;
  const child=spawn(process.execPath,['server.mjs'],{
    cwd:new URL('.',import.meta.url).pathname.replace(/^\/(?=[A-Za-z]:)/,''),
    env:{...process.env,BROWSPHERE_STAGEHAND_MCP_PORT:String(p),
      BROWSPHERE_STAGEHAND_CI_FIXTURE:'1',LUCIA_FIXTURE_PORT:String(fixturePort)},
    stdio:['ignore','ignore','pipe'],windowsHide:true
  });
  let errors='';
  child.stderr.on('data',b=>{errors=(errors+b.toString()).slice(-1200)});
  let client;
  try{
    client=await connectOnPort(p);
    const found=(await client.listTools()).tools;
    assert.deepEqual(found.map(x=>x.name).sort(),
      ['stagehand_extract','stagehand_inspect','stagehand_observe']);
    for(const tool of found){
      assert.equal(tool.annotations?.readOnlyHint,true);
      assert.equal(tool.annotations?.destructiveHint,false);
    }
    const denied=await client.callTool({name:'stagehand_inspect',arguments:{
      url:'http://169.254.169.254/latest/meta-data/',instruction:'Inspect document title'
    }});
    assert.equal(denied.isError,true,'Untrusted IP endpoint must be rejected');
    if(process.env.STAGEHAND_BROWSER_E2E==='1'){
      const observed=await client.callTool({name:'stagehand_inspect',arguments:{
        url:'http://127.0.0.1:'+fixturePort+'/',instruction:'Inspect document title'
      }});
      assert.notEqual(observed.isError,true,'Stagehand failed to launch isolated Chrome: '+errors);
      const response=JSON.parse(observed.content[0].text);
      assert.equal(response.verified,false);
      assert.equal(response.title,'Stagehand Upstream Fixture');
      assert.ok(response.url.startsWith('http://127.0.0.1:'+fixturePort+'/'));
    }
  }finally{
    if(client)await client.close().catch(()=>{});
    child.kill();
    await new Promise(ok=>fixture.close(ok));
  }
},{timeout:120000});
