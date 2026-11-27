'use strict';
// Deterministic, fail-closed SECOND-pass semantic reviewer for isolated local
// WebView2 integration fixtures. This is not a production model or fact oracle.
// Rust still validates original WebView evidence, citations and exact quotes.
const FIXTURES = [
  {goal:/search this documentation website for systems.*open the api reference/i,
   marker:'GENERIC_DOCS_CONFIRMED', subject:/api reference.*documentation search/i},
  {goal:/load delayed documentation.*open the documentation result/i,
   marker:'DELAYED_DOCS_CONFIRMED', subject:/documentation.*delayed/i},
  {goal:/select engineering.*enable examples.*intermediate.*show advanced/i,
   marker:'CONTROLS_FLOW_CONFIRMED', subject:/dropdown|checkbox|dialog|controls/i},
  {goal:/open keyboard dialog.*close it using escape/i,
   marker:'KEYBOARD_DIALOG_CLOSED', subject:/dialog.*closed/i},
  {goal:/search for robotics video.*open the first result/i,
   marker:'VIDEO_PAGE_CONFIRMED', subject:/robotics video.*opened/i},
  {goal:/click start recovery test.*use the recovery fallback/i,
   marker:'Recovery complete: fallback button succeeded', subject:/recovery complete/i},
];

function reviewFixtureEvidence(payload) {
  const incomplete = why => ({
    verdict:'insufficient', relevant_ids:[], missing:[why],
  });
  let input;
  try { input = typeof payload === 'string' ? JSON.parse(payload) : payload; }
  catch { return incomplete('Malformed evidence review payload'); }
  if (!input || typeof input !== 'object') return incomplete('Missing evidence');
  const goal = input.original_goal;
  const findings = input.findings_untrusted;
  if (typeof goal !== 'string' || !Array.isArray(findings) ||
      findings.length !== 1 || input.comparison === true) {
    return incomplete('Unsupported fixture objective or finding count');
  }
  const rule = FIXTURES.find(item => item.goal.test(goal));
  if (!rule) return incomplete('No approved synthetic fixture objective');
  const [f] = findings;
  if (!f || f.id !== 'f1' || typeof f.url !== 'string' ||
      !/^http:\/\/127\.0\.0\.1:\d+\//.test(f.url) ||
      typeof f.claim !== 'string' || typeof f.quote !== 'string' ||
      !f.quote.includes(rule.marker) || !rule.subject.test(f.claim)) {
    return incomplete('Finding does not support the specified fixture objective');
  }
  return {verdict:'supported',relevant_ids:['f1'],missing:[]};
}
module.exports = {reviewFixtureEvidence};
