import {createServer} from 'node:http';
import {McpServer,createMcpHandler} from '@modelcontextprotocol/server';
import {localhostHostValidation,localhostOriginValidation,toNodeHandler} from '@modelcontextprotocol/node';
import {z} from 'zod/v4';
import {stagehandRead} from './stagehand-readonly.mjs';

if (process.env.BROWSPHERE_STAGEHAND_ENABLE !== '1') {
  throw Error('Stagehand MCP requires explicit BROWSPHERE_STAGEHAND_ENABLE=1 opt-in');
}
const port=Number(process.env.BROWSPHERE_STAGEHAND_MCP_PORT||18888);
if (!Number.isInteger(port)||port<1024||port>65535) throw Error('Invalid loopback MCP port');
const ci=process.env.GITHUB_ACTIONS==='true' && process.env.BROWSPHERE_STAGEHAND_CI_FIXTURE==='1';
const fixturePort=ci?process.env.LUCIA_FIXTURE_PORT:'';

function register(server,name,mode){
  server.registerTool(name,{
    description:mode==='inspect'?'Read public page title and URL with upstream Stagehand':
      mode==='observe'?'Propose read-only structured actions using upstream Stagehand observe()':
      'Extract page facts with upstream Stagehand extract(); every quote requires independent verification',
    inputSchema:z.object({
      url:z.string().min(11).max(2048),
      instruction:z.string().min(4).max(240),
    }),
    annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:true}
  },async({url,instruction})=>{
    try{
      const result=await stagehandRead({url,instruction,mode,model:process.env.BROWSPHERE_STAGEHAND_OLLAMA_MODEL},
        {ci,fixturePort});
      return {content:[{type:'text',text:JSON.stringify(result).slice(0,16000)}]};
    }catch(err){
      // Never echo prompt text, credentials, paths or URL back to a page.
      return {content:[{type:'text',text:'Stagehand capability not available or source unverified'}],isError:true};
    }
  });
}
const handler=createMcpHandler(()=>{
  const server=new McpServer({name:'browsphere-stagehand-upstream',version:'0.1.0'});
  register(server,'stagehand_inspect','inspect');
  register(server,'stagehand_observe','observe');
  register(server,'stagehand_extract','extract');
  return server;
});
const asNode=toNodeHandler(handler),goodHost=localhostHostValidation(),goodOrigin=localhostOriginValidation();
const http=createServer((req,res)=>{
  if(req.url!=='/mcp'||!goodHost(req,res)||!goodOrigin(req,res)){
    if(!res.headersSent){res.statusCode=404;res.end();}
    return;
  }
  void asNode(req,res);
});
http.listen(port,'127.0.0.1',()=>{console.error('Stagehand upstream MCP ready on loopback only')});
process.on('SIGINT',async()=>{await handler.close();http.close()});
