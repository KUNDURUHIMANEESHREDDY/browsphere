'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const {observedNodes,decideAction,verifyEvidence,chat} =
  require('./e2e/lucia-local-model-server.cjs');

function prompt(text, nodes = []) {
  return 'Original user goal: search for robotics video\n' +
    'Current specialist: Browser Agent\n' + text +
    '\nInteractive nodes JSON: ' + JSON.stringify(nodes);
}
const search = [
  {node_id:'live-input',tag:'input',role:'searchbox',label:'Search',
    is_sensitive:false,is_submit:false},
  {node_id:'live-button',tag:'button',role:'button',label:'Search',
    is_sensitive:false,is_submit:false}
];
const confirmation =
  'VIDEO_PAGE_CONFIRMED. Robotics video opened successfully after searching and selecting the first result.';
const recovered =
  'Recovery complete: fallback button succeeded after the first click failed.';

test('mock planner proposes real browser specialist followed by Rust verifier', () => {
  const result = chat({messages:[
    {role:'system',content:'You are Lucia\'s autonomous browser task planner.'},
    {role:'user',content:'Search for robotics video'}
  ]});
  const proposal = JSON.parse(result.message.content);
  assert.equal(proposal.stages.length,1);
  assert.equal(proposal.stages[0].specialist,'browser');
});

test('search action sequence reacts to observed controls and page state', () => {
  assert.equal(observedNodes(prompt('',search)).length,2);
  const type = decideAction(prompt('No query entered yet.',search)).action;
  assert.equal(type.type,'type_text');
  assert.equal(type.text,'robotics video');
  assert.equal(type.node_id,'invalid-search-input-id',
    'Fixture deliberately exercises live-DOM repair');
  const click = decideAction(prompt('Query: robotics video',search)).action;
  assert.equal(click.type,'click');
  assert.equal(click.element_label,'Search');
  const result = decideAction(prompt('Search results loaded',[
    {node_id:'fresh-result',label:'Open robotics video',tag:'button',role:'button'}
  ])).action;
  assert.equal(result.type,'click');
  assert.equal(result.node_id,'fresh-result');
  const complete = decideAction(prompt(confirmation,[])).action;
  assert.equal(complete.type,'complete');
});

test('compact local prompt reads current DOM controls even with instructions after the JSON', () => {
  const makePrompt = visible =>
    'Choose ONE next action using only observed DOM node_id values.\n' +
    'Original user goal: search for robotics video\n' +
    'Current specialist: Browser Agent\n' +
    '<untrusted_page_data>\n' + visible + '\n</untrusted_page_data>\n' +
    'Freshly observed controls JSON: ' + JSON.stringify(search) + '\n' +
    'Use type_text for observed text fields, click for observed buttons. Do not guess node IDs.';
  const initial = makePrompt('Search the local robotics catalog.');
  assert.deepEqual(observedNodes(initial),search);
  const typed = decideAction(initial).action;
  assert.equal(typed.type,'type_text');
  assert.equal(typed.text,'robotics video');
  const afterTyping = decideAction(makePrompt('Query: robotics video')).action;
  assert.equal(afterTyping.type,'click');
  assert.equal(afterTyping.element_label,'Search');
  assert.equal(afterTyping.node_id,'invalid-search-button-id');
});

test('compact page excerpt excludes stale history and rejects invalid control data', () => {
  const stale = 'Previous action outcomes: VIDEO_PAGE_CONFIRMED from old page\n' +
    '<untrusted_page_data>\nThe new page is a blank search interface.\n</untrusted_page_data>\n' +
    'Freshly observed controls JSON: ' + JSON.stringify(search) + '\n' +
    'Do not claim success without observed evidence.';
  assert.equal(decideAction(stale).action.type,'type_text');
  assert.deepEqual(observedNodes('Freshly observed controls JSON: not-json\nContinue'),[]);
  assert.deepEqual(observedNodes('No current controls provided'),[]);
  assert.deepEqual(observedNodes('Freshly observed controls JSON: ' +
    JSON.stringify([{label:'Search'}]) + '\nContinue'),[]);
});

test('recovery action sequence handles a failed primary click', () => {
  const primary = decideAction(prompt('Recovery waiting',[
    {node_id:'primary',label:'Start recovery test',tag:'button'}
  ])).action;
  assert.equal(primary.node_id,'primary');
  const fallback = decideAction(prompt('Primary failed',[
    {node_id:'fallback',label:'Use recovery fallback',tag:'button'}
  ])).action;
  assert.equal(fallback.node_id,'fallback');
  assert.equal(decideAction(prompt(recovered)).action.type,'complete');
});

test('verifier cites only observed success and refuses missing proof', () => {
  const sample = {original_goal:'Search for robotics video',observed_web_pages_untrusted:[
    {url:'http://127.0.0.1:18764/multistep.html?video=robotics-demo',
      title:'Robotics demo',excerpt:confirmation}
  ]};
  const verified=verifyEvidence(JSON.stringify(sample));
  assert.equal(verified.status,'verified');
  assert.equal(verified.findings[0].quote,confirmation);
  assert.equal(verified.findings[0].url,sample.observed_web_pages_untrusted[0].url);
  assert.equal(verifyEvidence(JSON.stringify({...sample,observed_web_pages_untrusted:[]})).status,'insufficient');
  assert.equal(verifyEvidence(JSON.stringify({
    observed_web_pages_untrusted:[{
      url:'http://127.0.0.1:18764/recovery.html',excerpt:recovered
    }]
  })).status,'verified');
});
