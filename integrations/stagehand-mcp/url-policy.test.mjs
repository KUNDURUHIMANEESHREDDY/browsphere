import test from 'node:test';
import assert from 'node:assert/strict';
import {allowedResearchUrl} from './url-policy.mjs';
import {createOllamaModel} from './model-provider.mjs';
test('explicit public source allowlist blocks private networks, credentials and redirects',()=>{
  assert.equal(allowedResearchUrl('https://www.w3.org/WAI/'),'https://www.w3.org/WAI/');
  assert.equal(allowedResearchUrl('https://web.dev/'),'https://web.dev/');
  for(const url of [
    'http://www.w3.org','http://localhost:18888/mcp','http://127.0.0.1:18888/mcp',
    'http://169.254.169.254/latest/meta-data/','https://user:pass@www.w3.org/',
    'https://evil.example/','file:///etc/passwd','https://www.w3.org.evil.example/'
  ]) assert.throws(()=>allowedResearchUrl(url));
  assert.equal(allowedResearchUrl('http://127.0.0.1:18818/generic-search.html',
    {ci:true,fixturePort:'18818'}),'http://127.0.0.1:18818/generic-search.html');
  assert.throws(()=>allowedResearchUrl('http://127.0.0.1:18819/',{ci:true,fixturePort:'18818'}));
});
test('upstream Stagehand Ollama callback uses loopback and structured schema',async()=>{
  let called;
  const generate=createOllamaModel({model:'qwen2.5:3b-instruct',request:async(...args)=>{
    called=args;
    return {ok:true,json:async()=>({message:{content:'{"answer":"ok"}'}})}
  }}).generate;
  const result=await generate({messages:[{role:'user',content:{type:'text',text:'test'}}],
    responseFormat:{type:'json_schema',schema:{type:'object',properties:{answer:{type:'string'}}}}});
  assert.equal(result.structuredContent.answer,'ok');
  assert.equal(called[0],'http://127.0.0.1:11434/api/chat');
  assert.equal(JSON.parse(called[1].body).stream,false);
  await assert.rejects(generate({messages:[{role:'user',content:{type:'image',data:'sensitive'}}],
    responseFormat:{type:'json_schema',schema:{type:'object'}}}));
});
