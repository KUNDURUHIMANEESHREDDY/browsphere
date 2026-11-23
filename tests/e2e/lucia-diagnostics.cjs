'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

async function main() {
  const script = fs.readFileSync(path.join(__dirname, '../../ui/public/src/lucia-diagnostics.js'), 'utf8');
  const panel = { innerHTML: '' };
  const runA = '11111111-1111-4111-8111-111111111111';
  const runB = '22222222-2222-4222-8222-222222222222';
  const fixture = [
    { timestamp: '2026-11-23T10:00:00Z', run_id: runA, event: 'run_started' },
    { timestamp: '2026-11-23T10:00:01Z', run_id: runA, event: 'action_trace', step: 1,
      action: 'navigate', decision: 'allow', risk: 'Low', latency_ms: 250 },
    { timestamp: '2026-11-23T10:00:02Z', run_id: runA, event: 'action_trace', step: 2,
      action: 'click', decision: 'deny', latency_ms: 100 },
    { timestamp: '2026-11-23T10:00:03Z', run_id: runA, event: 'run_finished', status: 'halted' },
    { timestamp: '2026-11-23T10:00:04Z', run_id: runB, event: 'run_started' },
    { timestamp: '2026-11-23T10:00:05Z', run_id: runB, event: 'run_finished', status: 'completed' }
  ];
  const window = { aetherBridge: { isNative: true, readLuciaDiagnostics: async () => fixture } };
  const context = { window, document: { getElementById: () => panel },
    Date, Map, Array, String, Number };
  vm.runInNewContext(script, context, { filename: 'lucia-diagnostics.js' });
  const dashboard = window.LuciaDiagnosticsDashboard;

  assert.equal(dashboard.summarize(fixture).length, 2);
  assert.equal(dashboard.summarize(fixture).find(r => r.id === runA).steps, 2);
  assert.match(dashboard.failureHint(dashboard.summarize(fixture).find(r => r.id === runA)), /denied by policy/);
  assert.match(dashboard.render(), /Lucia Diagnostics/);
  await dashboard.refresh();
  dashboard.select(runA);
  assert.match(panel.innerHTML, /ACTION TIMELINE/);
  assert.match(panel.innerHTML, /navigate/);
  assert.match(panel.innerHTML, /Policy: deny/);
  assert.match(panel.innerHTML, /No completion recorded|Halted/);
  dashboard.filter('completed');
  assert.match(panel.innerHTML, /22222222/);
  assert.doesNotMatch(panel.innerHTML, /11111111/);
  dashboard.filter('issues');
  assert.match(panel.innerHTML, /11111111/);
  assert.doesNotMatch(panel.innerHTML, /22222222/);

  const hostile = fixture.concat([{ timestamp: '2026-11-23T10:00:06Z', run_id: runA,
    event: 'action_trace', action: '<img src=x onerror=alert(1)>' }]);
  window.aetherBridge.readLuciaDiagnostics = async () => hostile;
  await dashboard.refresh();
  assert.doesNotMatch(panel.innerHTML, /<img src=x/);
  assert.match(panel.innerHTML, /&lt;img/);
  window.aetherBridge.readLuciaDiagnostics = async () => { throw Error('secret API key'); };
  await dashboard.refresh();
  assert.match(panel.innerHTML, /Could not read local Lucia diagnostics/);
  assert.doesNotMatch(panel.innerHTML, /secret API key/);
  console.log('Lucia dashboard tests passed (grouping, filters, failure, XSS, error privacy)');
}

main().catch(err => { console.error(err); process.exitCode = 1; });
