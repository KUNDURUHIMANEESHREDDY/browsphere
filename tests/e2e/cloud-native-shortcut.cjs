'use strict';
// Strict CI fixture exception: a native, DOM-bound first-result workflow can
// complete without an inference call in explicit cloud-only mode. No local model
// is used, and all task completion evidence still comes from WebView2/Rust.
// This exception does NOT authorize native shortcuts for arbitrary websites.
const GOAL='On the current local test page, search for robotics video and open the first result.';
const EXPECTED=[
  ['type_text','Browser Agent / search_input:trusted_user_query'],
  ['press_key','Browser Agent / search_enter:after_observed_query_type'],
  ['click','Browser Agent / recovery:ranked_live_alternative'],
  ['click','Browser Agent / first_result:unique_live_dom'],
];
function approvedNativeFixtureSearch(goal, actions, report) {
  return goal===GOAL && report?.status==='evidence_checked' &&
    Array.isArray(actions) && actions.length===EXPECTED.length &&
    actions.every((action,i)=>
      action?.type===EXPECTED[i][0] &&
      typeof action.route==='string' &&
      action.route.startsWith(EXPECTED[i][1]) &&
      (action.outcome==='ok'||action.outcome==='confirmed'));
}
module.exports={approvedNativeFixtureSearch};
