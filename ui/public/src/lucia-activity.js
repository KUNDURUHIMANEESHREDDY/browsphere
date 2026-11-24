'use strict';
// Safe, user-facing summaries of Lucia's executed browser actions.
// This module does not expose private model reasoning, prompts, typed values,
// page excerpts, or secret-bearing URLs. Exported for deterministic UI tests.
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  if (root) root.BrowsphereLuciaActivity = api;
})(typeof window === 'object' ? window : null, function () {
  const phases = Object.freeze({
    observing: 'Reading the live page and its controls',
    observed: 'Fresh page controls captured',
    deciding: 'Choosing the next action from the observed page',
    acting: 'Applying policy checks and carrying out the action',
    checking: 'Checking whether the page actually changed',
    verifying: 'Reviewing the result against live source evidence'
  });
  const actionNames = Object.freeze({
    navigate: 'Open website',
    click: 'Click observed control',
    type_text: 'Enter text in observed field (value hidden)',
    press_key: 'Press a browser key',
    select_option: 'Choose observed menu option',
    set_checked: 'Change observed toggle',
    scroll: 'Scroll page',
    download: 'Request file download',
    complete: 'Finish specialist stage'
  });
  const outcomes = Object.freeze({
    ok: 'Executed', confirmed: 'Page change confirmed',
    failed: 'Failed', denied: 'Blocked by policy',
    rejected: 'Not approved', timed_out: 'Timed out',
    unverified: 'Not verified', stopped_or_unknown: 'Not confirmed'
  });
  function safeIndex(value) {
    return Number.isInteger(value) && value >= 0 && value < 10000 ? value + 1 : null;
  }
  function activity(payload) {
    const phase = typeof payload?.phase === 'string' ? payload.phase : '';
    if (!Object.prototype.hasOwnProperty.call(phases, phase)) return null;
    const controls = Number.isInteger(payload.controls) && payload.controls >= 0
      ? Math.min(payload.controls, 9999) : null;
    const label = phases[phase] +
      (phase === 'observed' && controls !== null ? ' (' + controls + ' controls)' : '');
    return { kind: 'phase', label, explanation: 'Live browser progress, not private model reasoning.',
      status: phase, step: safeIndex(payload.step_index) };
  }
  function trace(payload) {
    const action = typeof payload?.action?.type === 'string' ? payload.action.type : '';
    if (!Object.prototype.hasOwnProperty.call(actionNames, action)) return null;
    const outcome = typeof payload?.outcome === 'string' ? payload.outcome : '';
    const safeOutcome = Object.prototype.hasOwnProperty.call(outcomes, outcome) ? outcome : 'stopped_or_unknown';
    const route = typeof payload?.model_route === 'string' ? payload.model_route : '';
    const wasVerification = route.includes('verification:read_only');
    const wasDirectNavigation = route.includes('direct_navigation:trusted_user_goal');
    const wasMacro = route.includes('macro:');
    const wasLoopGuard = route.includes('guard:repeated_action');
    let explanation = wasVerification ?
      'An independent read-only evidence check was requested.' :
      wasDirectNavigation ? 'The website came directly from the user request, then passed policy checks.' :
      wasMacro ? 'A saved workflow action was checked against the current page and policy.' :
      'The next action was selected using observed controls and checked against browser policy.';
    if (action === 'type_text') explanation += ' Entered text is never shown in this timeline.';
    if (action === 'click' && payload.action.is_submit) explanation += ' The action may submit a page form.';
    if (action === 'complete' && !wasVerification) explanation =
      'The specialist requested completion; independent verification is still required.';
    if (wasLoopGuard) explanation = 'The loop guard stopped repeated actions without verified progress; the task is not complete.';
    const source = typeof payload.active_origin === 'string' && /^https?:\/\/[a-z0-9.-]+(?::\d+)?$/i.test(payload.active_origin)
      ? payload.active_origin.slice(0, 130) : '';
    const model = route.match(/(?:litellm|groq|ollama|bedrock):([a-z0-9_./:-]{1,64})/i);
    const ms = Number.isInteger(payload.latency_ms) && payload.latency_ms >= 0 && payload.latency_ms < 1e8
      ? payload.latency_ms : null;
    return { kind: 'action', label: wasLoopGuard ? 'Stopped by loop guard' : actionNames[action], explanation,
      status: safeOutcome, outcomeLabel: outcomes[safeOutcome],
      step: safeIndex(payload.step_index), source,
      model: model ? model[1] : (wasVerification ? 'Independent verifier' : (wasDirectNavigation ? 'Direct navigation' : 'Browser agent')),
      latency: ms === null ? '' : (ms / 1000).toFixed(1) + 's' };
  }
  function append(task, item) {
    if (!item || !task) return false;
    if (!Array.isArray(task.activityLog)) task.activityLog = [];
    task.activityLog.push(item);
    if (task.activityLog.length > 80) task.activityLog.splice(0, task.activityLog.length - 80);
    task.liveActivity = item.label;
    return true;
  }
  return Object.freeze({ activity, trace, append });
});
