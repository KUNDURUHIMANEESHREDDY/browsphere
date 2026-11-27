'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const {traceRouteMatchesModel: matches}=require('./model-route-contract.cjs');

test('namespaced model IDs and exact provider labels are preserved',()=>{
  assert.equal(matches('Browser Agent / ollama:openbmb/minicpm5:latest / supervisor_recovery',
    'ollama','openbmb/minicpm5:latest'),true);
  assert.equal(matches('Browser Agent / ollama:openbmb/minicpm5:latest-stale',
    'ollama','openbmb/minicpm5:latest'),false);
  assert.equal(matches('Browser Agent / ollama:custom-model / fresh_dom_replan',
    'ollama','custom-model'),true);
  assert.equal(matches('Browser Agent / litellm:custom-model',
    'ollama','custom-model'),false);
  assert.equal(matches('Browser Agent / litellm:custom-model',
    'litellm','custom-model'),true);
  assert.equal(matches('Browser Agent / recovery:ranked_live_alternative',
    'ollama','custom-model'),false);
});
