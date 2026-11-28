'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const root=path.resolve(__dirname,'..');
const script=fs.readFileSync(path.join(root,'tests/e2e/run-lucia-real-qwen.ps1'),'utf8');
const workflow=fs.readFileSync(path.join(root,'.github/workflows/lucia-real-qwen.yml'),'utf8');

test('real-site runner default is a model the admission gate actually allows',()=>{
  const model=script.match(/\[string\]\$Model\s*=\s*'([^']+)'/)?.[1];
  const only=script.match(/if\s*\(\$Model\s*-ne\s*'([^']+)'\)/)?.[1];
  assert.equal(model,'qwen3.5:0.8b');
  assert.equal(model,only);
});
test('workflow dispatch uses the real-model runner in unfamiliar-site mode',()=>{
  assert.match(workflow,/workflow_dispatch:/);
  assert.match(workflow,/npm ci --prefix tests\/interop --ignore-scripts/);
  assert.match(workflow,/npm test --prefix tests\/interop/);
  assert.match(workflow,/run-lucia-real-qwen\.ps1\s+-UnfamiliarSites/);
  assert.match(script,/run-lucia-unfamiliar-hosted\.ps1/);
  assert.match(script,/\$probe\.done/);
});

test('genuine Qwen JSON preflight disables thinking and budgets response tokens',()=>{
  const probe=script;
  assert.match(probe,/think=\$false/);
  const budget=Number(probe.match(/num_predict=(\d+);num_ctx=3072/)?.[1]);
  assert.ok(budget>=96,'Preflight must reserve enough tokens for a complete answer');
  assert.match(probe,/format='json'/);
  assert.match(probe,/IsNullOrWhiteSpace\(\[string\]\$probe\.response\)/);
  assert.match(probe,/ConvertFrom-Json -ErrorAction Stop/);
  assert.match(probe,/\$preflightAction\.type -ne 'scroll'/);
  assert.match(probe,/\$preflightAction\.delta_y -ne 150/);
});
