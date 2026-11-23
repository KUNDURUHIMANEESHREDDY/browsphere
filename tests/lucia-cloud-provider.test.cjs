'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const http = require('node:http');
const {runLocalModel,providerStats,cloudPhase} =
  require('./e2e/lucia-local-model-server.cjs');

async function withServer(fn) {
  const server=http.createServer(runLocalModel);
  await new Promise((resolve,reject)=>{
    server.once('error',reject);
    server.listen(0,'127.0.0.1',resolve);
  });
  const base='http://127.0.0.1:'+server.address().port;
  try { await fn(base); } finally { await new Promise(resolve=>server.close(resolve)); }
}
async function completion(base,model,system='You are a browser agent working under Lucia',user='Interactive nodes JSON: []') {
  return fetch(base+'/v1/chat/completions',{
    method:'POST',
    headers:{'content-type':'application/json'},
    body:JSON.stringify({model,messages:[
      {role:'system',content:system},
      {role:'user',content:user}
    ]})
  });
}

test('the OpenAI-compatible cloud model catalog includes an explicit test ID',async()=>{
  await withServer(async base=>{
    const resp=await fetch(base+'/v1/models');
    assert.equal(resp.status,200);
    const data=await resp.json();
    assert.ok(data.data.some(m=>m.id==='lucia-cloud-e2e'));
  });
});

test('cloud browser-action requests respond using choices[0].message.content',async()=>{
  await withServer(async base=>{
    const resp=await completion(base,'lucia-cloud-e2e');
    assert.equal(resp.status,200);
    const body=await resp.json();
    assert.equal(body.model,'lucia-cloud-e2e');
    const parsed=JSON.parse(body.choices[0].message.content);
    assert.equal(typeof parsed.thought,'string');
    assert.equal(typeof parsed.action.type,'string');
  });
});

test('planner and verifier use the same cloud endpoint and JSON response envelope',async()=>{
  await withServer(async base=>{
    const p=await completion(base,'lucia-cloud-e2e',
      "You are Lucia's autonomous browser task planner");
    const plan=JSON.parse((await p.json()).choices[0].message.content);
    assert.equal(plan.stages[0].specialist,'browser');
    const v=await completion(base,'lucia-cloud-e2e',
      "You are Lucia's independent, READ-ONLY evidence-review specialist",
      JSON.stringify({observed_web_pages_untrusted:[]}));
    const report=JSON.parse((await v.json()).choices[0].message.content);
    assert.equal(report.status,'insufficient');
  });
  assert.equal(cloudPhase([{role:'system',content:'You are Lucia\'s autonomous browser task planner'}]),'planner');
  assert.equal(cloudPhase([{role:'system',content:'You are Lucia\'s independent, READ-ONLY evidence-review specialist'}]),'verifier');
  assert.ok(providerStats.planner>0 && providerStats.actions>0 && providerStats.verifier>0);
});

test('cloud endpoint rejects unavailable models with explicit statuses, not chat fallback',async()=>{
  await withServer(async base=>{
    for (const [model,expected] of [
      ['lucia-cloud-reject-401',401],
      ['lucia-cloud-limit-429',429],
      ['lucia-cloud-unavailable-503',503],
      ['unlisted-model',404]
    ]) {
      const resp=await completion(base,model);
      assert.equal(resp.status,expected,'Incorrect status for '+model);
      const body=await resp.json();
      assert.ok(body.error?.code,'Missing failure code for '+model);
      assert.ok(!body.choices,'Failed provider must not include an action');
    }
    const malformed=await completion(base,'lucia-cloud-invalid-json');
    assert.equal(malformed.status,200);
    assert.equal((await malformed.json()).choices[0].message.content,'not a browser action');
  });
});
