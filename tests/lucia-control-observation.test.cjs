'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const model = require('./e2e/lucia-local-model-server.cjs');

const pagePrompt = (excerpt, nodes, oldEvidence = 'KEYBOARD_DIALOG_CLOSED') =>
  [
    'Original user goal: Open keyboard dialog, then close it using Escape.',
    'Prior live-source evidence (stale):',
    '<untrusted_page_data>' + oldEvidence + '</untrusted_page_data>',
    '<untrusted_page_data origin="http://127.0.0.1:18764">',
    excerpt,
    '</untrusted_page_data>',
    'Interactive nodes JSON: ' + JSON.stringify(nodes)
  ].join('\n');

test('decisions ignore stale old evidence when current dialog is open', () => {
  const prompt = pagePrompt('Keyboard dialog is open',
    [{node_id:'modal',label:'Keyboard dialog'},{node_id:'open',label:'Open keyboard dialog'}]);
  assert.equal(model.currentPageText(prompt),'Keyboard dialog is open');
  assert.deepEqual(model.decideAction(prompt).action,
    {type:'press_key',node_id:'modal',key:'Escape'});
});

test('wait for dialog close event instead of reopening the same modal', () => {
  const prompt = pagePrompt('Keyboard dialog is open',
    [{node_id:'open',label:'Open keyboard dialog'}]);
  assert.equal(model.decideAction(prompt).action.type,'scroll');
});

test('completion requires current-page closed confirmation', () => {
  const prompt = pagePrompt('KEYBOARD_DIALOG_CLOSED. Native dialog dismissed by Escape.',
    [{node_id:'open',label:'Open keyboard dialog'}],
    'Keyboard dialog is open');
  assert.equal(model.decideAction(prompt).action.type,'complete');
});
