'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const {pageReadiness,buildRepetitions,verifiedAgentResult}=
  require('./e2e/lucia-real-site-evidence.cjs');
const c={name:'Docs',host:'docs.python.org',path:/\/library\/asyncio\.html$/};
const source={url:'https://docs.python.org/3/',title:'Python documentation',
  controls:30,excerptLength:700};
const after={url:'https://docs.python.org/3/library/asyncio.html',
  tabUrl:'https://docs.python.org/3/library/asyncio.html',
  title:'asyncio — Asynchronous I/O',excerptLength:850};
const actions=[
  {type:'type_text',outcome:'ok',route:'Browser Agent / litellm:oc/hy4-flash-free'},
  {type:'press_key',outcome:'confirmed',route:'Browser Agent / litellm:oc/hy4-flash-free'}
];
test('live pages require matching host, meaningful controls and no error page',()=>{
  assert.equal(pageReadiness(source,c.host).ok,true);
  assert.equal(pageReadiness({...source,title:'Wikimedia Error'},c.host).reason,'website_error_page');
  assert.equal(pageReadiness({...source,url:'https://docs.python.org.evil.test/3/'},c.host).ok,false);
  assert.equal(pageReadiness({...source,controls:0},c.host).reason,'document_not_ready');
});
test('repetition case ids are unique and require bounded repeat count',()=>{
  assert.deepEqual(buildRepetitions([c],3).map(x=>x.caseId),
    ['Docs #1/3','Docs #2/3','Docs #3/3']);
  for(const n of [0,6,1.5,'abc'])assert.throws(()=>buildRepetitions([c],n));
});
test('genuine success needs completed task, matching native page, verified result, and successful model actions',()=>{
  const run={status:'completed',verified:true,actions};
  assert.equal(verifiedAgentResult(c,run,after,'oc/hy4-flash-free').passed,true);
  assert.equal(verifiedAgentResult(c,{...run,status:'failed'},after,'oc/hy4-flash-free').passed,false);
  assert.equal(verifiedAgentResult(c,{...run,verified:false},after,'oc/hy4-flash-free').passed,false);
  assert.equal(verifiedAgentResult(c,run,{...after,tabUrl:source.url},'oc/hy4-flash-free').passed,false);
  assert.equal(verifiedAgentResult(c,run,{...after,title:'Access Denied'},'oc/hy4-flash-free').passed,false);
  assert.equal(verifiedAgentResult(c,run,{...after,url:'https://docs.python.org.evil.test/3/library/asyncio.html'},'oc/hy4-flash-free').passed,false);
});
test('failed, denied, or local-only actions cannot be counted as autonomous progress',()=>{
  const run={status:'completed',verified:true,actions:[
    {type:'type_text',outcome:'failed',route:'Browser Agent / litellm:oc/hy4-flash-free'},
    {type:'click',outcome:'ok',route:'Browser Agent / ollama:some-local-model'},
    {type:'press_key',outcome:'denied',route:'Browser Agent / litellm:oc/hy4-flash-free'}
  ]};
  const result=verifiedAgentResult(c,run,after,'oc/hy4-flash-free');
  assert.equal(result.meaningfulActions,0);
  assert.equal(result.passed,false);
});
