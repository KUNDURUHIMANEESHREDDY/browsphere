'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const {localCdpEndpoint} = require('./playwright-ci-policy.cjs');

test('reuses Playwright only for bounded loopback CI endpoints', () => {
  assert.equal(localCdpEndpoint('9317','true'),'http://127.0.0.1:9317');
  for(const invalid of ['0','1023','65536','1.5','-1','n/a','',undefined]){
    assert.throws(() => localCdpEndpoint(invalid,'true'));
  }
  assert.throws(() => localCdpEndpoint('9317',undefined));
  assert.throws(() => localCdpEndpoint('9317','false'));
});
