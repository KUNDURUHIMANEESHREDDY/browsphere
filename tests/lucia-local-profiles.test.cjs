'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const ui = path.join(__dirname,'../ui/public/src');
test('LFM3 and Qwen profiles are exposed while vision stays separately configurable', () => {
  const window = {};
  vm.runInNewContext(fs.readFileSync(path.join(ui,'ai-preferences.js'),'utf8'),{window,URL});
  const html = window.renderMacAiPreferences({config:{},localModels:['lfm3:1.7b','qwen3.5:0.8b','lfm3-vl:2b']});
  assert.match(html,/data-action="use-local-lucia-profile" data-model="lfm3:1.7b"/);
  assert.match(html,/data-action="use-local-lucia-profile" data-model="qwen3.5:0.8b"/);
  assert.match(html,/id="cfgVisionModel"/);
  assert.match(html,/data-action="use-local-lucia-fallback"/);
});
