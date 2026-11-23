'use strict';
// 9Router Cloud Models preset regression. No provider credentials or traffic.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '../..');
const window = {};
vm.runInNewContext(
  fs.readFileSync(path.join(root, 'ui/public/src/ai-preferences.js'), 'utf8'),
  { window, URL },
  { filename: 'ai-preferences.js' }
);
const presets = window.browsphereCloudPresets;
assert.ok(presets && Object.isFrozen(presets), 'Shared provider presets were not loaded');
assert.deepEqual(Array.from(presets.ninerouter_local), [
  'http://127.0.0.1:20128/v1/chat/completions', '9router/select-model'
]);
assert.deepEqual(Array.from(presets.ninerouter_cloud), [
  'https://9router.com/v1/chat/completions', '9router/select-model'
]);
const render = endpoint => window.renderMacAiPreferences({
  config: {
    litellm_url: endpoint, litellm_model: '9router/select-model',
    prefer_local_for_simple: false, lucia_cloud_only: true
  },
  localModels: [], hasKey: false
});
const localHtml = render(presets.ninerouter_local[0]);
const cloudHtml = render(presets.ninerouter_cloud[0]);
assert.match(localHtml, /<option value="ninerouter_local" selected>9Router \(Local gateway\)<\/option>/);
assert.match(cloudHtml, /<option value="ninerouter_cloud" selected>9Router \(Cloud\)<\/option>/);
assert.match(localHtml, /id="cfgLiteLlmUrl"[^>]*http:\/\/127\.0\.0\.1:20128\/v1\/chat\/completions/);
assert.match(cloudHtml, /id="cfgLiteLlmUrl"[^>]*https:\/\/9router\.com\/v1\/chat\/completions/);
assert.ok(localHtml.includes('Fetch Models'));
assert.ok(localHtml.includes('Test Lucia reasoning'));
assert.ok(localHtml.includes('cfgLuciaCloudOnly'));
assert.ok(localHtml.includes('No model is selected yet'));
assert.ok(localHtml.includes('Fetch Models, then choose a model'));
assert.ok(!localHtml.includes('id="cfgLiteLlmModel" type="text" value="9router/select-model"'));
assert.ok(!localHtml.includes('cc/claude-sonnet-4-6'));
assert.ok(render('https://9router.com.evil.test/v1/chat/completions')
  .includes('<option value="custom" selected>Custom / LiteLLM</option>'),
  'Do not treat a lookalike domain as 9Router Cloud');
assert.ok(render('https://example.com/v1/chat/completions')
  .includes('<option value="custom" selected>Custom / LiteLLM</option>'));
const browserShell = fs.readFileSync(path.join(root, 'ui/public/src/browser-shell.js'), 'utf8');
assert.ok(browserShell.includes('window.browsphereCloudPresets'),
  'The provider selection handler must use the shared preset registry');
console.log('PASS 9Router local/cloud presets, secure detection, settings rendering, model discovery UI');
