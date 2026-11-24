'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const activity = require('../ui/public/src/lucia-activity.js');

test('Lucia live activity shows browser phases without accepting arbitrary page text', () => {
  assert.deepEqual(activity.activity({ phase: 'observed', step_index: 0, controls: 120 }), {
    kind: 'phase',
    label: 'Fresh page controls captured (120 controls)',
    explanation: 'Live browser progress, not private model reasoning.',
    status: 'observed', step: 1
  });
  assert.equal(activity.activity({
    phase: '<script>alert("secret")</script>', message: 'private-page-text'
  }), null);
  const choosing = activity.activity({
    phase: 'deciding', step_index: 2, thought: 'do not display me', controls: 100
  });
  assert.equal(choosing.label, 'Choosing the next action from the observed page');
  assert.equal(JSON.stringify(choosing).includes('do not display me'), false);
});

test('Lucia action summaries never leak model rationale, typed values or URL query strings', () => {
  const result = activity.trace({
    step_index: 4, outcome: 'confirmed', latency_ms: 1820,
    active_origin: 'https://www.youtube.com/watch?v=private-token',
    thought: 'private chain of thought with secret',
    model_route: 'Browser Agent / litellm:oc/hy4-flash-free',
    action: { type: 'type_text', text: 'confidential query', node_id: 'n_2', is_sensitive: false }
  });
  assert.equal(result.step, 5);
  assert.equal(result.status, 'confirmed');
  assert.equal(result.model, 'oc/hy4-flash-free');
  assert.match(result.label, /value hidden/);
  assert.match(result.explanation, /never shown/);
  assert.equal(result.source, '');
  assert.equal(result.latency, '1.8s');
  const displayed = JSON.stringify(result);
  for (const token of ['confidential query', 'private chain', 'private-token', 'node_id', 'n_2']) {
    assert.equal(displayed.includes(token), false, token);
  }
});

test('Lucia shows policy and verification context without reporting a click as proven completion', () => {
  const direct = activity.trace({
    action: { type: 'navigate', url: 'https://youtube.com/watch?v=abc' }, outcome: 'ok',
    model_route: 'Browser Agent / direct_navigation:trusted_user_goal',
    active_origin: 'https://youtube.com'
  });
  assert.match(direct.explanation, /directly from the user request/);
  assert.equal(direct.source, 'https://youtube.com');
  assert.equal(direct.outcomeLabel, 'Executed');
  const completed = activity.trace({
    action: { type: 'complete', summary: 'claimed success' }, outcome: 'ok',
    model_route: 'Browser Agent / litellm:test'
  });
  assert.match(completed.explanation, /independent verification is still required/);
  const independent = activity.trace({
    action: { type: 'complete' }, outcome: 'confirmed', model_route: 'Verification Agent / verification:read_only'
  });
  assert.match(independent.explanation, /read-only evidence check/);
});

test('Lucia keeps only recent bounded activity and rejects invalid actions', () => {
  const task = {};
  for (let i = 0; i < 100; i++) {
    activity.append(task, activity.activity({ phase: 'observing', step_index: i }));
  }
  assert.equal(task.activityLog.length, 80);
  assert.equal(task.activityLog[0].step, 21);
  assert.equal(task.activityLog.at(-1).step, 100);
  assert.equal(activity.trace({
    action: { type: 'exec_shell', command: 'something-bad' }, thought: 'do not copy'
  }), null);
});

test('loop guard stop is not reported as specialist completion', () => {
  const result = activity.trace({ action: { type: 'complete' }, outcome: 'failed',
    model_route: 'ollama:test / guard:repeated_action' });
  assert.equal(result.label, 'Stopped by loop guard');
  assert.match(result.explanation, /not complete/);
});
