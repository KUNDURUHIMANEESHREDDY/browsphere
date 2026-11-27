'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

test('existing AI settings expose read-only model discovery without trusting status HTML', () => {
  const context = { window: {}, URL };
  const source = fs.readFileSync(path.join(__dirname, '../ui/public/src/ai-preferences.js'), 'utf8');
  vm.runInNewContext(source, context);
  const html = context.window.renderMacAiPreferences({
    config: {
      ollama_url:'http://127.0.0.1:11434',
      ollama_model:'custom-local',
      litellm_url:'https://api.example.org/v1/chat/completions',
      litellm_model:'selected-cloud',
      cloud_model_catalogs:{},
      domain_overrides:{}
    },
    localModels:['custom-local'],
    localCapabilitiesStatus:'<img src=x onerror=alert(1)>'
  });
  assert.match(html, /data-action="inspect-ai-capabilities"/);
  assert.match(html, /id="cfgLocalCapabilitiesStatus"/);
  assert.match(html, /Structured JSON and real task success:|Provider-declared text, vision, tools and thinking/);
  assert.ok(!html.includes('<img src=x onerror=alert(1)>'), 'status must be escaped');
  assert.ok(html.includes('&lt;img src=x onerror=alert(1)&gt;'));
  assert.match(html, /data-action="save-router-settings"/, 'preserve existing settings');
});
