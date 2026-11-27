'use strict';
// Real Microsoft Playwright MCP + native WebView2 smoke. The runner owns a
// throwaway WebView2 profile, mock model, isolated ports, and all subprocesses.
const assert = require('node:assert/strict');
const net = require('node:net');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const cdpPort = Number(process.env.LUCIA_CDP_PORT || 9277);
const mcpPort = Number(process.env.LUCIA_MCP_PORT || 18931);
const endpoint = 'http://127.0.0.1:' + mcpPort + '/mcp';

async function portReady() {
  return new Promise(resolve => {
    const socket=net.connect(mcpPort,'127.0.0.1');
    socket.setTimeout(700);
    socket.once('connect',()=>{socket.destroy();resolve(true)});
    socket.once('timeout',()=>{socket.destroy();resolve(false)});
    socket.once('error',()=>{socket.destroy();resolve(false)});
  });
}
function decodeResponse(body, contentType, id) {
  if (!body.trim()) return null;
  if (contentType.includes('text/event-stream')) {
    const payloads=body.split(/\r?\n\r?\n/).flatMap(block=>{
      const lines=block.split(/\r?\n/).filter(line=>line.startsWith('data:'));
      if (!lines.length) return [];
      try{return [JSON.parse(lines.map(line=>line.slice(5).trimStart()).join('\n'))]}
      catch{return []}
    });
    const matching=payloads.find(value=>value.id===id);
    assert.ok(matching,'Upstream SSE result did not match the MCP request ID');
    return matching;
  }
  return JSON.parse(body);
}
class McpClient {
  constructor(){this.version='2025-11-25';this.session=null;this.id=0}
  async request(method,params,notification=false) {
    const id=++this.id;
    const msg={jsonrpc:'2.0',method,params};
    if(!notification) msg.id=id;
    const headers={
      'Content-Type':'application/json',
      'Accept':'application/json, text/event-stream',
      'MCP-Protocol-Version':this.version
    };
    if(this.session) headers['Mcp-Session-Id']=this.session;
    const response=await fetch(endpoint,{
      method:'POST',headers,body:JSON.stringify(msg),
      signal:AbortSignal.timeout(12000)
    });
    const session=response.headers.get('mcp-session-id');
    if(session) this.session=session;
    assert.ok(response.ok,'Upstream MCP HTTP '+response.status+' ('+method+')');
    if(notification) return null;
    const data=decodeResponse(await response.text(),
      response.headers.get('content-type')||'',id);
    assert.equal(data?.jsonrpc,'2.0','Invalid MCP response version');
    assert.equal(data.id,id,'MCP response ID mismatch');
    assert.equal(data.error,undefined,'Upstream MCP error ('+method+')');
    return data.result;
  }
  async init(){
    const result=await this.request('initialize',{
      protocolVersion:this.version,capabilities:{},
      clientInfo:{name:'browsphere-windows-upstream-test',version:'1'}
    });
    assert.ok(result?.capabilities?.tools,'Upstream MCP did not advertise tools');
    this.version=result.protocolVersion;
    await this.request('notifications/initialized',{},true);
  }
  async call(name,args={}){
    const result=await this.request('tools/call',{name,arguments:args});
    assert.notEqual(result.isError,true,'Upstream tool reported failure');
    return result.content?.filter(v=>v.type==='text').map(v=>v.text).join('\n')||'';
  }
}
function section(text,name){
  const marker='### '+name+'\n';
  const i=text.indexOf(marker);
  if(i<0)return null;
  const rest=text.slice(i+marker.length);
  const stop=rest.search(/\n### [A-Za-z]/);
  return stop<0?rest:rest.slice(0,stop);
}
async function main(){
  assert.equal(process.env.GITHUB_ACTIONS,'true',
    'Real CDP integration runs only in disposable Windows CI');
  let ready=false;
  for(let i=0;i<150;i++){
    if(await portReady()){ready=true;break}
    await sleep(500);
  }
  assert.ok(ready,'Pinned Microsoft Playwright MCP never opened its local port');
  const targets=await(await fetch('http://127.0.0.1:'+cdpPort+'/json')).json();
  const shell=targets.find(t=>t.type==='page'&&t.url?.includes('tauri.localhost'));
  assert.ok(shell,'Isolated native WebView2 shell not found in CDP targets');
  const client=new McpClient();
  await client.init();
  const tools=(await client.request('tools/list',{})).tools||[];
  const tool=tools.find(t=>t.name==='browser_snapshot');
  assert.ok(tool,'Real upstream server lacks browser_snapshot');
  assert.equal(tool.annotations?.readOnlyHint,true,'Snapshot is not advertised read-only');
  // The test client invokes only the read-only snapshot. No actions are
  // authorized or performed by Lucia, and the native profile has no secrets.
  const start=performance.now();
  const snapshot=await client.call('browser_snapshot',{});
  const elapsed=Math.round(performance.now()-start);
  const page=section(snapshot,'Page');
  const tree=section(snapshot,'Snapshot');
  assert.ok(page,'Real upstream MCP response missing its Page section');
  assert.ok(tree,'Real upstream MCP response missing its Snapshot section');
  const url=page.split(/\r?\n/).find(line=>line.startsWith('- Page URL: '))?.slice(12).trim();
  assert.ok(url,'Upstream Page section missing URL');
  assert.ok(targets.some(t=>t.type==='page'&&t.url===url),
    'Upstream snapshot URL not found among real native CDP targets');
  assert.ok(url.includes('tauri.localhost'),'Upstream snapshot is not the Browsphere shell');
  assert.ok(tree.includes('- '),'Accessibility snapshot is empty');
  const counts=Object.fromEntries(['button','link','textbox','searchbox','combobox','dialog']
    .map(role=>[role,tree.split('\n')
      .filter(line=>line.trimStart().startsWith('- '+role+' ')).length]));
  assert.ok(Object.values(counts).some(count=>count>0),
    'No accessibility roles in the real native shell');
  // Only safe metrics go into the CI log, never a snapshot or page contents.
  console.log('LUCIA_UPSTREAM_PLAYWRIGHT_WEBVIEW2_PASS',JSON.stringify({
    provider:'microsoft/playwright-mcp',transport:'streamable_http',
    target:'native_webview2_shell',snapshot_ms:elapsed,counts
  }));
}
main().catch(err=>{
  console.error('LUCIA_UPSTREAM_PLAYWRIGHT_WEBVIEW2_FAIL',String(err?.stack||err).slice(0,1500));
  process.exitCode=1;
});
