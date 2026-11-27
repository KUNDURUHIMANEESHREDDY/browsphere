'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const {closeObservedDetailsDialog,escapeObservedKeyboardDialog}=require('./e2e/fixture-modal-action.cjs');
const observed=[{tag:'button',label:'Close details',node_id:'dom-visible-close',is_sensitive:false}];

test('live active-dialog snapshot closes the exact observed button',()=>{
  for(const text of [
    'Active dialog: Documentation details The details dialog is open.',
    'The details dialog is open.',
    'Dialog is open'
  ]){
    assert.deepEqual(closeObservedDetailsDialog(text,observed),{
      type:'click',node_id:'dom-visible-close',element_label:'Close details',is_submit:false
    });
  }
});
test('hidden dialog and unrelated controls do not produce guessed actions',()=>{
  assert.equal(closeObservedDetailsDialog('Dialog not opened',observed),null);
  assert.equal(closeObservedDetailsDialog('Dialog closed successfully',observed),null);
  assert.equal(closeObservedDetailsDialog('Active dialog: Documentation details',[]),null);
  assert.equal(closeObservedDetailsDialog('Active dialog: Documentation details',[
    {tag:'button',label:'Close details',node_id:'private',is_sensitive:true}
  ]),null);
});

test('Escape is only returned for the actually observed keyboard-dialog role',()=>{
  const node=[{tag:'dialog',role:'dialog',label:'Keyboard dialog',node_id:'active-modal',is_sensitive:false}];
  assert.deepEqual(escapeObservedKeyboardDialog(
    'Active dialog: Keyboard-only dialog Press Escape to close this dialog.',node
  ),{type:'press_key',node_id:'active-modal',key:'Escape'});
  assert.equal(escapeObservedKeyboardDialog('Keyboard dialog not opened',node),null);
  assert.equal(escapeObservedKeyboardDialog('Active dialog: Keyboard-only dialog',[]),null);
  assert.equal(escapeObservedKeyboardDialog('Active dialog: Keyboard-only dialog',[
    {...node[0],role:'button'}
  ]),null);
});
