'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');

test('OpenRouter preset selects an explicitly free model and official HTTPS endpoint', () => {
  const root = path.join(__dirname, '..');
  const code = fs.readFileSync(path.join(root, 'ui/public/src/ai-preferences.js'), 'utf8');
  const window = {};
  vm.runInNewContext(code, {window}, {filename: 'ai-preferences.js'});
  const [endpoint, model] = Array.from(window.browsphereCloudPresets.openrouter);
  assert.equal(endpoint, 'https://openrouter.ai/api/v1/chat/completions');
  assert.equal(model, 'inclusionai/ling-3.2-flash:free');
  assert.equal(model.endsWith(':free'), true);
  const html = window.renderMacAiPreferences({
    config: {
      ollama_url: 'http://127.0.0.1:11434',
      ollama_model: 'test-local',
      litellm_url: endpoint,
      litellm_model: model,
      prefer_local_for_simple: false,
      lucia_cloud_only: true,
      cloud_model_catalogs: {}
    },
    localModels: [],
    hasKey: false,
    aws: {}
  });
  assert.match(html, /Cloud provider/);
  assert.match(html, /inclusionai\/ling-3\.2-flash:free/);
  assert.match(html, /No API key saved for this endpoint/);
});

test('native OpenRouter real-site suite keeps model-derived browser actions separate from live evidence', () => {
  const root = path.join(__dirname, '..');
  const runner = fs.readFileSync(path.join(root, 'tests/e2e/lucia-real-sites-openrouter.cjs'), 'utf8');
  const wrapper = fs.readFileSync(path.join(root, 'tests/e2e/run-lucia-real-sites-openrouter.ps1'), 'utf8');
  assert.match(runner, /verifiedAgentResult\(c,run,after,model\)/);
  assert.match(runner, /pageReadiness\(before,c\.host\)/);
  assert.match(runner, /buildRepetitions\(selected,process\.env\.LUCIA_REAL_REPEATS/);
  assert.match(runner, /browserUrlSynchronized/);
  assert.match(runner, /meaningfulActions:evaluation\.meaningfulActions/);
  assert.match(runner, /site_unavailable/);
  assert.match(wrapper, /LUCIA_REAL_REPEATS/);
  assert.match(wrapper, /ValidateRange\(1,5\)/);
  assert.match(runner, /provider_rate_limited/);
  assert.match(runner, /name:'YouTube music'/);
  assert.match(runner, /requiresPlayback:true/);
  assert.match(runner, /youtubePlaybackEvidence\(after\?\.url\)/);
  assert.match(runner, /!c\.requiresPlayback\|\|media\?\.playing===true/);
  assert.match(runner, /provider_auth_failed/);
  assert.match(wrapper, /https:\/\/openrouter\.ai\/api\/v1\/chat\/completions/);
  assert.match(wrapper, /MISSING_OPENROUTER_KEY/);
  assert.match(wrapper, /structured_outputs/);
  assert.match(wrapper, /Write-Warning 'Model does not advertise structured outputs/);
  assert.match(wrapper, /if \(-not \$structured\)/);
  assert.match(wrapper, /pricing\.prompt/);
  assert.match(wrapper, /pricing\.completion/);
  assert.match(wrapper, /ONLY_FREE_MODELS/);
  assert.match(wrapper, /openrouter\/free/);
  assert.match(wrapper, /[.]EndsWith\(':free'/);
  assert.match(wrapper, /browsphere-openrouter-real-sites/);
  assert.doesNotMatch(wrapper, /9router\.com|20128/);
});
