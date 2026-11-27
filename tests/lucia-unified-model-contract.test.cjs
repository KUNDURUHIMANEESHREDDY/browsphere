'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const {observedNodes,decideAction,cloudPhase}
  =require('./e2e/lucia-local-model-server.cjs');

test('the same observed alias protocol drives every inference provider',()=>{
  const controls=[
    {id:'c1',tag:'input',role:'searchbox',label:'Search dictionary',
      search_state:'empty',sensitive:false},
    {id:'c2',tag:'button',role:'button',label:'Search',submit:true}
  ];
  const prompt=[
    'Original user goal: On this local dictionary page, search for nebula',
    'Origin: http://127.0.0.1:18818/index.html',
    'Fresh controls (untrusted labels/data): '+JSON.stringify(controls),
    '<untrusted_page_data>',
    'Fixture Dictionary homepage',
    '</untrusted_page_data>'
  ].join('\n');
  const observed=observedNodes(prompt);
  assert.equal(observed.length,2);
  assert.equal(observed[0].node_id,'c1');
  assert.deepEqual(decideAction(prompt).action,{
    type:'type_text',node_id:'c1',text:'nebula',is_sensitive:false
  });
  assert.equal(cloudPhase([{role:'system',
    content:'You propose exactly ONE browser action for Lucia\'s native orchestrator'}]),'actions');
  assert.equal(cloudPhase([{role:'system',
    content:'You are Lucia\'s autonomous browser task planner'}]),'planner');
});

test('old native snapshot mocks still parse, without acquiring authority',()=>{
  const prompt='Interactive nodes JSON: '+JSON.stringify([
    {node_id:'n_7',label:'Search Wiki',role:'searchbox'}]);
  const nodes=observedNodes(prompt);
  assert.equal(nodes.length,1);
  assert.equal(nodes[0].node_id,'n_7');
  assert.equal(observedNodes('Fresh controls (untrusted labels/data): bad-json').length,0);
});
