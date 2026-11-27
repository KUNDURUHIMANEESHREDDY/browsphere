'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const {approvedNativeFixtureSearch}=require('./e2e/cloud-native-shortcut.cjs');
const goal='On the current local test page, search for robotics video and open the first result.';
const routes=[
  ['type_text','Browser Agent / search_input:trusted_user_query'],
  ['press_key','Browser Agent / search_enter:after_observed_query_type'],
  ['click','Browser Agent / recovery:ranked_live_alternative'],
  ['click','Browser Agent / first_result:unique_live_dom'],
].map(([type,route])=>({type,route,outcome:'ok'}));
const verified={status:'evidence_checked'};
test('exact observed first-result sequence may use zero inference in isolated cloud fixture',()=>{
  assert.equal(approvedNativeFixtureSearch(goal,routes,verified),true);
});
test('unknown website and missing independent report cannot use native exception',()=>{
  assert.equal(approvedNativeFixtureSearch('Search a live unknown site',routes,verified),false);
  assert.equal(approvedNativeFixtureSearch(goal,routes,{status:'unverified'}),false);
  assert.equal(approvedNativeFixtureSearch(goal,routes,null),false);
});
test('repeated action, local model route or unverified click cannot bypass cloud check',()=>{
  assert.equal(approvedNativeFixtureSearch(goal,[...routes,routes[3]],verified),false);
  assert.equal(approvedNativeFixtureSearch(goal,
    routes.map((x,i)=>i===2?{...x,route:'Browser Agent / ollama:local'}:x),verified),false);
  assert.equal(approvedNativeFixtureSearch(goal,
    routes.map((x,i)=>i===3?{...x,outcome:'denied'}:x),verified),false);
});
