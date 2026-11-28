'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const {decideAction}=require('./e2e/lucia-local-model-server.cjs');

function withObservation(nodes,page){
  return [
    'Original user goal: On this test page select Engineering, enable examples, choose Intermediate, enable Show advanced, open details and close the dialog.',
    'Fresh controls (untrusted labels/data): '+JSON.stringify(nodes),
    '<untrusted_page_data>',
    page,
    '</untrusted_page_data>'
  ].join('\n');
}
const details=[{id:'node-details-7',label:'Open details',role:'button'}];
test('local mocked form action uses the live alias when its later status is clipped',()=>{
  const action=decideAction(withObservation(details,
    'Selected category: Engineering. Examples enabled. Mode: Intermediate. Note drafted. Advanced is enabled.')).action;
  assert.equal(action.type,'click');
  assert.equal(action.node_id,'node-details-7');
  assert.equal(action.element_label,'Open details');
  assert.equal(action.is_submit,false);
});
test('missing observed control never authorizes an invented open-details click',()=>{
  const action=decideAction(withObservation([],'Advanced is enabled')).action;
  assert.notEqual(action.type,'click');
});
test('already closed dialog must not be reopened',()=>{
  const action=decideAction(withObservation(details,'Advanced is enabled. Dialog closed successfully.')).action;
  assert.notEqual(action.type,'click');
});
test('completion still requires the actual observed success marker',()=>{
  const action=decideAction(withObservation([],'CONTROLS_FLOW_CONFIRMED')).action;
  assert.equal(action.type,'complete');
});

function keyboardObservation(nodes,page){
  return [
    'Original user goal: Open keyboard dialog, then close it using Escape.',
    'Fresh controls (untrusted labels/data): '+JSON.stringify(nodes),
    '<untrusted_page_data>',page,'</untrusted_page_data>'
  ].join('\n');
}
test('clipped keyboard-dialog status still chooses its observed opener',()=>{
  const nodes=[{id:'open-keyboard',label:'Open keyboard dialog',tag:'button',role:'button'}];
  const action=decideAction(keyboardObservation(nodes,'Browser Controls Lab.')).action;
  assert.equal(action.type,'click');
  assert.equal(action.node_id,'open-keyboard');
  assert.equal(action.is_submit,false);
});
test('keyboard-dialog test never invents a missing opener',()=>{
  const action=decideAction(keyboardObservation([],'Browser Controls Lab.')).action;
  assert.notEqual(action.type,'click');
});
test('Escape uses the observed active modal rather than a guessed node',()=>{
  const nodes=[{id:'keyboard-dialog-node',label:'Keyboard dialog',role:'dialog',tag:'dialog'}];
  const action=decideAction(keyboardObservation(nodes,'Active dialog: Keyboard dialog.')).action;
  assert.equal(action.type,'press_key');
  assert.equal(action.node_id,'keyboard-dialog-node');
  assert.equal(action.key,'Escape');
});
test('completed keyboard dialog is not reopened',()=>{
  const nodes=[{id:'open-keyboard',label:'Open keyboard dialog',tag:'button',role:'button'}];
  const action=decideAction(keyboardObservation(nodes,'KEYBOARD_DIALOG_CLOSED')).action;
  assert.equal(action.type,'complete');
});
