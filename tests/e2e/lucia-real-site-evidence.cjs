'use strict';
// Pure evidence rules for real-public-site Lucia tests. No model calls or
// filesystem/browser access; keep the criteria independent from the agent.
const SITE_ERROR_TITLE = /\b(?:Wikimedia Error|Access Denied|Access Forbidden|Too Many Requests|Service Unavailable|Bad Gateway|Cloudflare Error|Internal Server Error)\b/i;
function originOf(url) { try { return new URL(url); } catch { return null; } }
function pageReadiness(state, host) {
  const origin=originOf(state?.url);
  if (!origin || origin.hostname !== host) return {ok:false,reason:'wrong_or_missing_document'};
  if (SITE_ERROR_TITLE.test(state?.title || '')) return {ok:false,reason:'website_error_page'};
  if ((state?.controls || 0)<1 || (state?.excerptLength || 0)<50 ||
      !String(state?.title || '').trim())
    return {ok:false,reason:'document_not_ready'};
  return {ok:true,reason:null};
}
function buildRepetitions(cases, requested) {
  const repeats=Number(requested ?? 1);
  if (!Number.isInteger(repeats) || repeats<1 || repeats>5) {
    throw Error('LUCIA_REAL_REPEATS must be an integer between 1 and 5');
  }
  return cases.flatMap(c=>Array.from({length:repeats},(_,i)=>({
    ...c, attempt:i+1,caseId:c.name+' #'+(i+1)+'/'+repeats
  })));
}
const ACTIONS = new Set(['click','type_text','press_key','select_option','set_checked']);
const CONFIRMED = new Set(['ok','confirmed']);
function verifiedAgentResult(c,run,after,model) {
  const url=originOf(after?.url);
  const observedDestination=!!url && url.hostname===c.host &&
    c.path.test(url.pathname) && !SITE_ERROR_TITLE.test(after?.title || '') &&
    (after?.excerptLength||0)>=50;
  const modelUsed=(run?.actions||[]).some(x=>(x.route||'').includes('litellm:'+model));
  const meaningfulActions=(run?.actions||[]).filter(x=>
    ACTIONS.has(x.type) && CONFIRMED.has(x.outcome) &&
    (x.route||'').includes('litellm:'+model)).length;
  const browserUrlSynchronized=!!after?.tabUrl && !!after?.url &&
    after.tabUrl === after.url;
  const verified=run?.verified===true;
  return {observedDestination,modelUsed,meaningfulActions,
    browserUrlSynchronized,verified,
    passed:run?.status==='completed' && verified && observedDestination &&
      modelUsed && meaningfulActions>=2 && browserUrlSynchronized};
}
module.exports={pageReadiness,buildRepetitions,verifiedAgentResult};
