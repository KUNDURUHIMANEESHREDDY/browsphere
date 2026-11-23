/* Lucia's local, metadata-only diagnostics dashboard. No remote analytics. */
(function () {
  'use strict';

  var records = [];
  var currentRunId = '';
  var statusFilter = 'all';
  var loaded = false;
  var pending = false;
  var failure = '';
  var requestVersion = 0;

  function escapeHtml(value) {
    return String(value == null ? '' : value)
      .replace(/&/g, '&amp;').replace(/</g, '&lt;')
      .replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
  }
  function timestamp(value) {
    var date = new Date(value);
    return Number.isNaN(date.getTime()) ? 'Unknown time' : date.toLocaleString();
  }
  function summarize(events) {
    var grouped = new Map();
    (Array.isArray(events) ? events : []).forEach(function (event) {
      if (!event || !/^[a-f0-9-]{36}$/i.test(event.run_id || '')) return;
      var run = grouped.get(event.run_id);
      if (!run) {
        run = { id: event.run_id, events: [], started: '', latest: '', status: 'incomplete', steps: 0, failures: 0 };
        grouped.set(event.run_id, run);
      }
      run.events.push(event);
      if (!run.latest || event.timestamp > run.latest) run.latest = event.timestamp;
      if (event.event === 'run_started') run.started = event.timestamp;
      if (event.event === 'action_trace') run.steps += 1;
      if (event.event === 'run_finished') run.status = event.status || 'unknown';
      if (event.event === 'agent_failure') run.failures += 1;
    });
    return Array.from(grouped.values()).map(function (run) {
      run.events.sort(function (a, b) { return a.timestamp.localeCompare(b.timestamp); });
      // The final run status wins even when rotated files were read in different groups.
      run.events.forEach(function (event) {
        if (event.event === 'run_finished') run.status = event.status || 'unknown';
      });
      return run;
    }).sort(function (a, b) { return b.latest.localeCompare(a.latest); }).slice(0, 100);
  }
  function label(status) {
    return {
      completed: 'Completed', complete: 'Completed', success: 'Completed', replayed: 'Replayed',
      failed: 'Failed', halted: 'Halted', unverified: 'Unverified', rejected: 'Rejected', cancelled: 'Cancelled',
      canceled: 'Cancelled', stopped: 'Stopped',
      incomplete: 'No completion recorded', running: 'Running'
    }[status] || 'Unknown';
  }
  function isIssue(run) {
    return !['completed', 'complete', 'success', 'replayed'].includes(run.status);
  }
  function failureHint(run) {
    if (!isIssue(run)) return 'Run completed. The timeline shows the recorded actions and decisions.';
    var events = run.events;
    if (events.some(function (e) { return e.outcome === 'approval_rejected'; }) || run.status === 'rejected')
      return 'An action was rejected during approval. The run stopped without continuing.';
    if (events.some(function (e) { return e.outcome === 'approval_timeout'; }))
      return 'The approval request timed out before the action was allowed.';
    if (events.some(function (e) { return e.outcome === 'unverified'; }) || run.status === 'unverified')
      return 'Lucia could not verify a browser action or its final result. Completion was not confirmed.';
    if (events.some(function (e) { return e.decision === 'deny'; }))
      return 'One or more actions were denied by policy. Review the denied steps below.';
    if (events.some(function (e) { return e.outcome === 'timeout'; }))
      return 'An action timed out. Check the final recorded action below.';
    if (events.some(function (e) { return e.outcome === 'failed' || e.outcome === 'failure' || e.outcome === 'error'; }))
      return 'An action reported failure. The detailed error was not stored for privacy.';
    if (run.status === 'incomplete')
      return 'No completion event is recorded. The run may still be active or may have been interrupted.';
    return 'This run did not complete successfully. The precise cause is not available in privacy-safe diagnostic metadata.';
  }
  function pill(value) {
    var s = String(value || 'unknown');
    var tone = /failed|halted|error|denied|deny/i.test(s) ? 'text-rose-600 dark:text-rose-400 bg-rose-500/10'
      : /completed|success|allow|verified/i.test(s) ? 'text-emerald-700 dark:text-emerald-400 bg-emerald-500/10'
      : 'text-[#64748B] dark:text-[#9CA3AF] bg-black/5 dark:bg-white/10';
    return '<span class="rounded-md px-1.5 py-0.5 text-[10px] font-medium ' + tone + '">' + escapeHtml(s) + '</span>';
  }
  function renderInner() {
    var runs = summarize(records);
    var shown = runs.filter(function (run) {
      return statusFilter === 'all' || (statusFilter === 'issues' ? isIssue(run) : !isIssue(run));
    });
    if (!shown.some(function (run) { return run.id === currentRunId; })) currentRunId = shown[0] ? shown[0].id : '';
    var selected = shown.find(function (run) { return run.id === currentRunId; });
    var controls = '<div class="flex items-center flex-wrap gap-2 mb-3">' +
      ['all', 'issues', 'completed'].map(function (key) {
        var title = { all:'All runs', issues:'Needs attention', completed:'Completed' }[key];
        return '<button type="button" data-action="lucia-diag-filter" data-filter="' + key + '" aria-pressed="' +
          (statusFilter === key) + '" class="mac-ai-btn ' +
          (statusFilter === key ? '!bg-[#007AFF] !text-white' : '') + '">' + title + '</button>';
      }).join('') +
      '<button type="button" data-action="lucia-diag-refresh" class="mac-ai-btn ml-auto"' +
      (pending ? ' disabled' : '') + '>' + (pending ? 'Loading...' : 'Refresh') + '</button></div>';
    if (failure) return controls + '<p role="alert" class="text-rose-600 text-[12px]">' + escapeHtml(failure) + '</p>';
    if (!loaded) return controls + '<p class="text-[12px] opacity-60" role="status">' +
      (pending ? 'Reading local diagnostics...' : 'Select Refresh to load local runs.') + '</p>';
    if (!shown.length) return controls + '<p class="text-[12px] opacity-60">No ' +
      (statusFilter === 'all' ? '' : escapeHtml(statusFilter) + ' ') +
      'Lucia runs were found in retained logs.</p>';

    var sidebar = shown.map(function (run) {
      return '<button type="button" data-action="lucia-diag-select" data-run-id="' + escapeHtml(run.id) +
        '" aria-pressed="' + (run.id === currentRunId) +
        '" class="w-full text-left rounded-lg border px-2.5 py-2 mb-1.5 transition ' +
        (run.id === currentRunId ? 'border-[#007AFF] bg-[#007AFF]/[0.07]' :
          'border-black/10 dark:border-white/10 hover:bg-black/5 dark:hover:bg-white/5') + '">' +
        '<div class="flex justify-between items-center gap-2 mb-1">' + pill(label(run.status)) +
        '<span class="text-[10px] opacity-60">' + run.steps + ' actions</span></div>' +
        '<div class="font-mono text-[11px] truncate">' + escapeHtml(run.id.slice(0, 8)) + '</div>' +
        '<div class="text-[10px] opacity-60 mt-0.5">' + escapeHtml(timestamp(run.latest)) + '</div></button>';
    }).join('');
    var selectedInfo = '';
    if (selected) {
      var events = selected.events.slice(-150);
      selectedInfo = '<div class="min-w-0 flex-1">' +
        '<div class="flex items-center gap-2 flex-wrap mb-2">' + pill(label(selected.status)) +
        '<span class="text-[11px] opacity-60">' + selected.steps + ' actions</span></div>' +
        '<div class="text-[11px] font-mono break-all opacity-70 mb-1">Run ' + escapeHtml(selected.id) + '</div>' +
        '<div class="text-[11px] opacity-65 mb-3">First recorded: ' +
        escapeHtml(timestamp(selected.started || selected.events[0].timestamp)) +
        '</div><p class="text-[12px] leading-relaxed rounded-lg bg-black/[0.035] dark:bg-white/[0.06] px-3 py-2 mb-3">' +
        escapeHtml(failureHint(selected)) + '</p>' +
        '<h4 class="text-[11px] font-semibold tracking-wide mb-2">ACTION TIMELINE</h4>' +
        '<div class="space-y-2 max-h-[360px] overflow-y-auto" aria-label="Lucia action timeline">' +
        (events.length < selected.events.length ? '<p class="text-[10px] opacity-60">Displaying latest 150 events.</p>' : '') +
        events.map(function (ev) {
          var title = ev.event === 'run_started' ? 'Run started' :
            ev.event === 'run_finished' ? 'Run finished' :
            ev.event === 'agent_failure' ? 'Failure recorded' : (ev.action || 'Action');
          var detail = [
            ev.step != null ? 'Step ' + ev.step : '',
            ev.decision ? 'Policy: ' + ev.decision : '',
            ev.risk ? 'Risk: ' + ev.risk : '',
            ev.model_route ? 'Route: ' + ev.model_route : '',
            ev.outcome ? 'Outcome: ' + ev.outcome : '',
            ev.latency_ms != null ? ev.latency_ms + ' ms' : ''
          ].filter(Boolean).join(' · ');
          return '<div class="border-l-2 border-[#007AFF]/30 pl-3 py-1">' +
            '<div class="flex flex-wrap items-center gap-2">' +
            '<span class="text-[12px] font-medium capitalize">' + escapeHtml(title.replace(/_/g, ' ')) + '</span>' +
            (ev.status ? pill(ev.status) : '') +
            '<span class="text-[10px] opacity-55 ml-auto">' + escapeHtml(timestamp(ev.timestamp)) + '</span></div>' +
            (detail ? '<div class="text-[11px] opacity-65 mt-1">' + escapeHtml(detail) + '</div>' : '') +
            (ev.trace_id ? '<div class="text-[10px] font-mono opacity-45 mt-1">Trace ' + escapeHtml(ev.trace_id.slice(0, 8)) + '</div>' : '') +
            '</div>';
        }).join('') + '</div></div>';
    }
    return controls + '<div class="grid grid-cols-1 md:grid-cols-[205px_minmax(0,1fr)] gap-3">' +
      '<div class="max-h-[440px] overflow-y-auto" aria-label="Recent Lucia runs">' + sidebar + '</div>' +
      '<div class="min-w-0 border border-black/10 dark:border-white/10 rounded-xl p-3">' + selectedInfo +
      '</div></div><p class="text-[10.5px] opacity-55 mt-3">Local metadata only; no prompts, page text or URLs. ' +
      'Old runs may be incomplete after log rotation.</p>';
  }
  function render() {
    return '<section class="mac-ai-group" aria-label="Lucia Diagnostics"><h3 class="mac-ai-heading">Lucia Diagnostics</h3>' +
      '<p class="mac-ai-group-note">Inspect agent runs, policy decisions, and execution failures without opening log files.</p>' +
      '<div class="mac-ai-surface p-3"><div id="luciaDiagnosticsPanel" aria-live="polite">' +
      renderInner() + '</div></div></section>';
  }
  function paint() {
    var node = document.getElementById('luciaDiagnosticsPanel');
    if (node) node.innerHTML = renderInner();
  }
  async function refresh() {
    var request = ++requestVersion;
    pending = true;
    failure = '';
    paint();
    try {
      if (!window.aetherBridge || !window.aetherBridge.isNative ||
          typeof window.aetherBridge.readLuciaDiagnostics !== 'function') {
        throw new Error('Diagnostics require the Browsphere Windows app.');
      }
      var data = await window.aetherBridge.readLuciaDiagnostics();
      if (request !== requestVersion) return;
      records = Array.isArray(data) ? data : [];
      loaded = true;
    } catch (err) {
      if (request !== requestVersion) return;
      failure = 'Could not read local Lucia diagnostics.';
    } finally {
      if (request === requestVersion) { pending = false; paint(); }
    }
  }
  window.LuciaDiagnosticsDashboard = {
    render: render,
    refresh: refresh,
    select: function (runId) { currentRunId = runId; paint(); },
    filter: function (key) {
      if (['all', 'issues', 'completed'].includes(key)) statusFilter = key;
      paint();
    },
    // Pure summarizer available for unit tests, no raw-data persistence.
    summarize: summarize,
    failureHint: failureHint
  };
})();
