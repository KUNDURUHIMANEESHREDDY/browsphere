(function () {
  'use strict';

  function isAutomaticMemoryAllowed() {
    try { return window.localStorage.getItem('browsphere-auto-memory') !== 'off'; }
    catch (_) { return true; }
  }

  function getTauriCore() {
    if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
      return window.__TAURI__.core.invoke.bind(window.__TAURI__.core);
    }
    if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
      return (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
    }
    return null;
  }

  function getTauriListen() {
    if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.listen === 'function') {
      return window.__TAURI__.event.listen.bind(window.__TAURI__.event);
    }
    return null;
  }

  const invoke = getTauriCore();
  const hasTauri = Boolean(invoke);
  const listen = getTauriListen();

  // Capture unexpected UI failures without uploading or storing stack traces,
  // full error messages, URLs, prompts, or app state.
  if (hasTauri && invoke) {
    const report = (kind) => invoke('record_ui_diagnostic', { kind }).catch(() => {});
    window.addEventListener('error', () => report('js_error'));
    window.addEventListener('unhandledrejection', () => report('unhandled_rejection'));
  }

  let activeApprovalId = null;
  let agentPaused = false;
  let currentTabId = 'tab-1';
  const loadedExternalTabs = new Set();
  let currentTabIsExternal = false;
  // Bind each agent run to the tab the user actually launched it from.
  // This prevents an asynchronous SPA observation from updating another tab.
  const agentRunTabs = new Map();

  if (hasTauri) {
    document.body.classList.remove('p-2', 'sm:p-4', 'md:p-6');
    document.body.classList.add('p-0');
    const win = document.getElementById('safariWindow');
    if (win) {
      win.classList.remove('max-w-[1400px]', 'h-[93vh]', 'rounded-[16px]');
      win.classList.add('max-w-full', 'h-screen', 'rounded-none');
    }
  }

  function esc(str) {
    return String(str ?? '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function emitUiEvent(name, detail) {
    window.dispatchEvent(new CustomEvent(name, { detail }));
  }

  function isFullModalOpen() {
    const modals = ['tabOverviewModal', 'safariSettingsModal'];
    for (const id of modals) {
      const el = document.getElementById(id);
      if (el && !el.classList.contains('hidden')) return true;
    }
    return false;
  }

  function getOpenPopoverRect(id) {
    const el = document.getElementById(id);
    if (!el || el.classList.contains('popover-closed') || el.classList.contains('drawer-closed') || el.classList.contains('hidden')) {
      return null;
    }
    const r = el.getBoundingClientRect();
    if (r.width < 10 || r.height < 10) return null;
    return r;
  }

  function computeCanvasBounds() {
    const canvas = document.getElementById('readingCanvas');
    if (!canvas) {
      return {
        x: 256,
        y: 52,
        width: Math.max(320, window.innerWidth - 256),
        height: Math.max(240, window.innerHeight - 52),
        hidden: true
      };
    }
    const rect = canvas.getBoundingClientRect();
    if (!currentTabIsExternal || isFullModalOpen() || rect.width < 40 || rect.height < 40) {
      return {
        x: Math.round(rect.left),
        y: Math.round(rect.top),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
        hidden: true
      };
    }

    let x = rect.left;
    let y = rect.top;
    let right = rect.right;
    let bottom = rect.bottom;

    // Top-anchored dropdowns: shift live webpage down below the dropdown instead of hiding it
    const topOverlays = ['omnibarDropdown', 'privacyPopover'];
    for (const id of topOverlays) {
      const r = getOpenPopoverRect(id);
      if (r && r.bottom > y && r.top < bottom) {
        y = Math.max(y, r.bottom + 8);
      }
    }

    // Side-anchored popovers / AI Quick Chat: carve out horizontal space so live webpage stays visible beside AI Chat
    const sideOverlays = ['downloadsPopover', 'sharePopover', 'newTabHistoryDrawer'];
    for (const id of sideOverlays) {
      const r = getOpenPopoverRect(id);
      if (!r) continue;
      // Check if it overlaps the current canvas region
      if (r.right <= x || r.left >= right || r.bottom <= y || r.top >= bottom) continue;

      const leftSpace = r.left - x - 10;
      const rightSpace = right - r.right - 10;

      if (leftSpace >= rightSpace) {
        right = Math.min(right, r.left - 10);
      } else {
        x = Math.max(x, r.right + 10);
      }
    }

    const width = Math.round(right - x);
    const height = Math.round(bottom - y);
    const hidden = width < 120 || height < 120;

    return {
      x: Math.round(x),
      y: Math.round(y),
      width: Math.max(0, width),
      height: Math.max(0, height),
      hidden
    };
  }

  let syncRaf = null;
  function scheduleBoundsSync() {
    if (!invoke) return;
    if (syncRaf) cancelAnimationFrame(syncRaf);
    syncRaf = requestAnimationFrame(() => {
      syncRaf = null;
      const bounds = computeCanvasBounds();
      invoke('sync_tab_bounds', { bounds }).catch(() => {});
    });
  }

  function appendAgentBubble(meta, bodyHtml) {
    const stream = document.getElementById('chatMessagesStream');
    if (!stream) return;
    const row = document.createElement('div');
    row.className = 'flex items-start gap-2';
    row.innerHTML =
      '<span class="w-5 h-5 rounded-full bg-gradient-to-tr from-[#007AFF] to-[#AF52DE] text-white flex items-center justify-center shrink-0 mt-0.5 text-[9px] font-bold">AI</span>' +
      '<div class="bg-black/[0.04] dark:bg-white/[0.07] px-3 py-2 rounded-2xl rounded-tl-sm text-[12px] max-w-[88%] space-y-1">' +
        '<div class="text-[10px] font-mono opacity-55">' + esc(meta) + '</div>' +
        '<div>' + bodyHtml + '</div>' +
      '</div>';
    stream.appendChild(row);
    stream.scrollTop = stream.scrollHeight;
  }

  function closeApprovalBanner(approvalId) {
    if (approvalId && approvalId !== activeApprovalId) return;
    activeApprovalId = null;
    document.getElementById('tauriApprovalCard')?.remove();
  }

  function showApprovalBanner(req) {
    const stream = document.getElementById('chatMessagesStream');
    if (!stream) return;
    closeApprovalBanner();
    activeApprovalId = req.approval_id;
    const existing = document.getElementById('tauriApprovalCard');
    if (existing) existing.remove();

    const card = document.createElement('div');
    card.id = 'tauriApprovalCard';
    card.className = 'p-2.5 rounded-xl border border-amber-400/50 bg-amber-500/10 text-[11.5px] space-y-2';
    card.innerHTML =
      '<div class="font-semibold text-amber-600 dark:text-amber-400">Policy Gate · Approval Required (' + esc(req.risk) + ')</div>' +
      '<div class="opacity-90">' + esc(req.reason) + '</div>' +
      '<div class="flex items-center gap-2">' +
        '<button type="button" id="tauriApproveBtn" class="px-2.5 py-1 rounded-lg bg-emerald-600 text-white text-[11px] font-semibold hover:brightness-110">Approve</button>' +
        '<button type="button" id="tauriRejectBtn" class="px-2.5 py-1 rounded-lg bg-rose-600 text-white text-[11px] font-semibold hover:brightness-110">Reject</button>' +
      '</div>';
    stream.appendChild(card);
    stream.scrollTop = stream.scrollHeight;

    const approvalId = req.approval_id;
    async function decide(approved) {
      if (!invoke || activeApprovalId !== approvalId) return;
      card.querySelectorAll('button').forEach(btn => { btn.disabled = true; });
      try {
        await invoke('resolve_approval', { approvalId, approved });
      } catch (err) {
        appendAgentBubble('Lucia · Approval expired', esc(String(err)));
      } finally {
        closeApprovalBanner(approvalId);
      }
    }
    document.getElementById('tauriApproveBtn').addEventListener('click', () => decide(true));
    document.getElementById('tauriRejectBtn').addEventListener('click', () => decide(false));
  }

  async function reconcileAgentPage(runId) {
    const key = String(runId || '');
    const tabId = agentRunTabs.get(key);
    if (!invoke || !tabId || tabId !== currentTabId) return;
    try {
      // WebView2 PageLoadEvent does not necessarily fire for history.pushState
      // transitions in dynamic SPAs. Only a fresh native DOM observation can
      // synchronize the address bar here; never trust the model's claimed URL.
      const observed = await invoke('capture_live_snapshot');
      if (agentRunTabs.get(key) !== tabId || tabId !== currentTabId) return;
      const url = observed?.origin || '';
      if (!/^https?:\/\//i.test(url)) return;
      const currentUiUrl = window.browsphereApp?.activeTab?.()?.url;
      if (currentUiUrl === url) return;
      currentTabIsExternal = true;
      loadedExternalTabs.add(tabId);
      scheduleBoundsSync();
      emitUiEvent('aether:page-loaded', { tabId, url });
    } catch (_) {
      // The next real page event or agent observation can repair the view.
    }
  }

  window.aetherBridge = {
    isNative: hasTauri,

    async onNavigate(tabId, url, title) {
      if (!invoke) return;
      currentTabId = tabId || currentTabId;
      const isExternal = url.startsWith('http://') || url.startsWith('https://');
      currentTabIsExternal = isExternal;
      const bounds = computeCanvasBounds();

      if (isExternal) {
        try {
          await invoke('open_or_navigate_tab', {
            tabId: currentTabId, url, bounds,
            isPrivate: document.documentElement.getAttribute('data-private') === 'true'
          });
          loadedExternalTabs.add(currentTabId);
        } catch (err) {
          console.warn('Child WebView2 nav error:', err);
          emitUiEvent('aether:browser-error', {
            tabId: currentTabId, url, message: String(err)
          });
        }
      } else {
        try {
          await invoke('switch_tab_webview', { tabId: currentTabId, isExternal: false, bounds });
          if (document.documentElement.getAttribute('data-private') !== 'true') {
            await invoke('record_navigation', { url, title: title || url, isPrivate: false });
          }
        } catch (_) {}
      }
      emitUiEvent('aether:history-updated');
    },

    async onSwitchTab(tabId, url) {
      if (!invoke) return;
      currentTabId = tabId || currentTabId;
      const isExternal = Boolean(url && (url.startsWith('http://') || url.startsWith('https://')));
      currentTabIsExternal = isExternal;
      const bounds = computeCanvasBounds();
      try {
        if (isExternal && !loadedExternalTabs.has(currentTabId)) {
          await invoke('open_or_navigate_tab', {
            tabId: currentTabId, url, bounds,
            isPrivate: document.documentElement.getAttribute('data-private') === 'true'
          });
          loadedExternalTabs.add(currentTabId);
        } else {
          await invoke('switch_tab_webview', { tabId: currentTabId, isExternal, bounds });
        }
      } catch (err) {
        console.warn('Could not switch browser tab:', err);
        emitUiEvent('aether:browser-error', {
          tabId: currentTabId, url, message: String(err)
        });
      }
    },

    async reloadTab(tabId) {
      if (!invoke) throw new Error('Native browser runtime not available');
      await invoke('reload_tab_webview', { tabId });
    },

    async recordCompletedNavigation(url, title) {
      if (!invoke || document.documentElement.getAttribute('data-private') === 'true') return;
      await invoke('record_navigation', { url, title, isPrivate: false });
      emitUiEvent('aether:history-updated');
    },

    async onCloseTab(tabId) {
      if (!invoke || !tabId) return;
      try {
        await invoke('close_tab_webview', { tabId });
        loadedExternalTabs.delete(tabId);
      } catch (_) {}
    },

    syncBounds() {
      scheduleBoundsSync();
    },

    async setTheme(dark) {
      if (!invoke) return;
      try {
        await invoke('set_app_theme', { dark: Boolean(dark) });
      } catch (_) {}
    },

    async toggleAiChatWindow(forceOpen, title, url, dark, initialPrompt, forceTask = false) {
      if (!invoke) return false;
      try {
        return await invoke('toggle_ai_chat_window', {
          forceOpen: typeof forceOpen === 'boolean' ? forceOpen : null,
          title: title || null,
          url: url || null,
          dark: typeof dark === 'boolean' ? dark : null,
          initialPrompt: initialPrompt || null,
          forceTask: Boolean(forceTask),
          isPrivate: document.documentElement.getAttribute('data-private') === 'true'
        });
      } catch (err) {
        console.warn('AI Chat window error:', err);
        return false;
      }
    },

    async syncAiChatContext(title, url, dark) {
      if (!invoke) return;
      try {
        await invoke('sync_ai_chat_context', {
          title: title || 'Active Page',
          url: url || 'safari://start',
          dark: Boolean(dark),
          isPrivate: document.documentElement.getAttribute('data-private') === 'true'
        });
      } catch (_) {}
    },

    async fetchUrlPreview(url) {
      if (!invoke || !url) return null;
      try {
        return await invoke('fetch_url_preview', { url });
      } catch (_) {
        return null;
      }
    },

    async routeLuciaIntent(message) {
      if (!invoke) return {mode:'chat'};
      return invoke('route_lucia_intent', { message });
    },

    async startLuciaSpecialistTask(goal, currentUrl, pageTitle) {
      if (!invoke) throw new Error('Specialist tasks require the Browsphere Windows application');
      let snapshot = null;
      try { snapshot = await invoke('capture_live_snapshot'); } catch (_) {}
      if (!snapshot) {
        snapshot = {
          origin: currentUrl || 'browsphere://start',
          title: pageTitle || 'New Tab',
          text_excerpt: '',
          interactive_nodes: []
        };
      }
      const launchTabId = currentTabId;
      const runId = await invoke('start_agent_run', {
        goal,
        snapshot,
        allowedDomains: [],
        maxSteps: 26,
        allowLearning: document.documentElement.getAttribute('data-private') !== 'true' && isAutomaticMemoryAllowed(),
        isPrivate: document.documentElement.getAttribute('data-private') === 'true',
        voiceMode: window.browsphereVoice?.enabled === true
      });
      agentRunTabs.set(String(runId), launchTabId);
      // Capture an initial navigation even if it raced ahead of the launch reply.
      reconcileAgentPage(runId).catch(() => {});
      return runId;
    },

    async resolveLuciaTaskApproval(approvalId, approved) {
      if (!invoke) throw new Error('Requires the Browsphere Windows application');
      return invoke('resolve_approval', { approvalId, approved });
    },

    async runAgentGoal(goal, currentUrl, pageTitle) {
      if (!invoke) return false;
      let snapshot = null;
      try {
        snapshot = await invoke('capture_live_snapshot');
      } catch (_) {}
      if (!snapshot) {
        snapshot = {
          origin: currentUrl,
          title: pageTitle || currentUrl,
          text_excerpt: (document.getElementById('pageViewport')?.innerText || '').slice(0, 1800),
          interactive_nodes: []
        };
      }
      try {
        await invoke('start_agent_run', {
          goal,
          snapshot,
          allowedDomains: [],
          maxSteps: 12,
          allowLearning: document.documentElement.getAttribute('data-private') !== 'true' && isAutomaticMemoryAllowed(),
          isPrivate: document.documentElement.getAttribute('data-private') === 'true',
          voiceMode: window.browsphereVoice?.enabled === true
        });
        emitUiEvent('aether:traces-updated');
        return true;
      } catch (err) {
        appendAgentBubble('Rust Agent Error', esc(err));
        return false;
      }
    },

    async togglePause() {
      if (!invoke) return false;
      agentPaused = !agentPaused;
      await invoke('set_agent_paused', { paused: agentPaused });
      return agentPaused;
    },

    async cancelRun() {
      if (!invoke) return;
      agentPaused = false;
      await invoke('cancel_agent_run');
      emitUiEvent('aether:traces-updated');
    },

    async getHistory(limit = 50) {
      if (!invoke) return [];
      return invoke('get_history', { limit });
    },

    async clearHistory() {
      if (!invoke) return;
      await invoke('clear_history');
      emitUiEvent('aether:history-updated');
    },

    async listMemories(query = null) {
      if (!invoke) return [];
      return invoke('list_memories', { query });
    },

    async deleteMemory(id) {
      if (!invoke) return false;
      const deleted = await invoke('delete_memory', { id });
      emitUiEvent('aether:memories-updated');
      return deleted;
    },

    async clearMemories() {
      if (!invoke) return 0;
      const removed = await invoke('clear_memories');
      emitUiEvent('aether:memories-updated');
      return removed;
    },

    async getDownloads(limit = 30) {
      if (!invoke) return [];
      return invoke('get_downloads', { limit });
    },

    async startDownload(url, filename) {
      if (!invoke) return null;
      const rec = await invoke('start_streaming_download', { url, filename });
      emitUiEvent('aether:downloads-updated', rec);
      return rec;
    },

    async cancelDownload(downloadId) {
      if (!invoke) return false;
      const ok = await invoke('cancel_download', { downloadId });
      emitUiEvent('aether:downloads-updated');
      return ok;
    },

    async clearDownloads() {
      if (!invoke) return;
      await invoke('clear_downloads');
      emitUiEvent('aether:downloads-updated');
    },

    async getItpStats() {
      if (!invoke) return null;
      return invoke('get_itp_stats');
    },

    async getItpBreakdown(limit = 8) {
      if (!invoke) return [];
      return invoke('get_itp_breakdown', { limit });
    },

    async loadAwsConfig() {
      if (!invoke) return null;
      return invoke('get_aws_config');
    },

    async saveAwsConfig(config) {
      if (!invoke) throw new Error('AWS settings require the desktop application');
      return invoke('update_aws_config', { config });
    },

    async connectAwsAccount(profile) {
      if (!invoke) throw new Error('AWS connection requires the Windows app');
      return invoke('aws_connect_account', { profile });
    },

    async testAwsConnection() {
      if (!invoke) throw new Error('Desktop application required');
      return invoke('aws_test_connection');
    },

    async backupAwsS3() {
      if (!invoke) throw new Error('Desktop application required');
      return invoke('aws_backup_s3');
    },

    async restoreAwsS3() {
      if (!invoke) throw new Error('Desktop application required');
      return invoke('aws_restore_s3');
    },

    async pushAwsDynamoDb() {
      if (!invoke) throw new Error('Desktop application required');
      return invoke('aws_sync_dynamodb');
    },

    async pullAwsDynamoDb() {
      if (!invoke) throw new Error('Desktop application required');
      return invoke('aws_pull_dynamodb');
    },

    async cognitoSignIn(username, password) {
      if (!invoke) throw new Error('Desktop application required');
      return invoke('aws_cognito_sign_in', { username, password });
    },

    async cognitoSignOut() {
      if (!invoke) throw new Error('Desktop application required');
      return invoke('aws_cognito_sign_out');
    },

    async cognitoSession() {
      if (!invoke) return null;
      return invoke('aws_cognito_session');
    },

    async invokeAwsLambda(payload) {
      if (!invoke) throw new Error('Desktop application required');
      return invoke('aws_invoke_lambda', { payload });
    },

    async openDiagnosticsFolder() {
      if (!invoke) throw new Error('Diagnostics require the desktop application');
      return invoke('open_diagnostics_folder');
    },

    async exportDiagnostics() {
      if (!invoke) throw new Error('Diagnostics export requires the desktop application');
      return invoke('export_diagnostics');
    },

    async loadRouterConfig() {
      if (!invoke) return null;
      return invoke('get_router_config');
    },

    async saveRouterConfig(cfg) {
      if (!invoke) return;
      await invoke('update_router_config', { config: cfg });
    },

    async listLocalModels() {
      if (!invoke) return [];
      return invoke('list_local_models');
    },

    async askMainTabAi(messages, voiceMode = false) {
      if (!invoke) throw new Error('AI chat requires the Browsphere desktop app');
      return invoke('ask_main_tab_ai', { messages, voiceMode: Boolean(voiceMode) });
    },

    async fetchCloudModels() {
      if (!invoke) throw new Error('Model fetching requires the Browsphere Windows application');
      return invoke('fetch_cloud_models');
    },

    async testModelConnection(backend = null) {
      if (!invoke) throw new Error('Model connection testing requires the Windows application');
      return invoke('test_model_connection', { backend });
    },

    async removeModelApiKey() {
      if (!invoke) throw new Error('Windows application required');
      return invoke('remove_model_api_key');
    },

    async hasLiteLlmKey() {
      if (!invoke) return false;
      return invoke('has_litellm_api_key');
    },

    async listSavedCredentials() {
      if (!invoke) return [];
      return invoke('list_saved_credentials');
    },

    async saveCredential(site, username, password) {
      if (!invoke) return;
      await invoke('save_credential', { site, username, password });
    },

    async getCredentialSecret(site, username) {
      if (!invoke) return null;
      return invoke('get_credential_secret', { site, username });
    },

    async deleteCredential(site, username) {
      if (!invoke) return;
      await invoke('delete_credential', { site, username });
    },

    async autofillActiveTab(site = null, username = null) {
      if (!invoke) return null;
      return invoke('autofill_active_tab', { site, username });
    },

    async speakText(text, rate = 0) {
      if (!invoke) return false;
      return invoke('speak_text', { text, rate });
    },

    async stopSpeaking() {
      if (!invoke) return;
      await invoke('stop_speaking').catch(() => {});
    },

    async listenSpeechOnce(timeoutSecs = 8) {
      if (!invoke) return null;
      return invoke('listen_speech_once', { timeoutSecs });
    },

    async runPolicyLoadTest(iterations = 240, concurrency = 8) {
      if (!invoke) return null;
      return invoke('run_policy_load_test', { iterations, concurrency });
    },

    async checkForUpdates() {
      if (!invoke) return null;
      return invoke('check_for_updates');
    },

    async getRecentRuns(limit = 20) {
      if (!invoke) return [];
      return invoke('get_recent_runs', { limit });
    },

    async getTracesFiltered(runId, risk, limit = 60) {
      if (!invoke) return [];
      return invoke('get_traces_filtered', {
        runId: runId && runId !== 'all' ? runId : null,
        risk: risk && risk !== 'all' ? risk : null,
        limit
      });
    },

    async listWorkflowMacros(limit = 30) {
      if (!invoke) return [];
      return invoke('list_workflow_macros', { limit });
    },

    async deleteWorkflowMacro(macroId) {
      if (!invoke) return;
      await invoke('delete_workflow_macro', { macroId });
      emitUiEvent('aether:macros-updated');
    },

    async replayWorkflowMacro(macroId, currentUrl, pageTitle) {
      if (!invoke) return false;
      let snapshot = null;
      try {
        snapshot = await invoke('capture_live_snapshot');
      } catch (_) {}
      if (!snapshot) {
        snapshot = {
          origin: currentUrl || '',
          title: pageTitle || currentUrl || 'Untitled',
          text_excerpt: (document.getElementById('pageViewport')?.innerText || '').slice(0, 1800),
          interactive_nodes: []
        };
      }
      await invoke('replay_workflow_macro', {
        macroId,
        snapshot,
        allowedDomains: []
      });
      emitUiEvent('aether:traces-updated');
      emitUiEvent('aether:macros-updated');
      return true;
    },

    async listAutomations(limit = 40) {
      if (!invoke) return [];
      return invoke('list_automations', { limit });
    },

    async saveAutomationJob(jobSpec) {
      if (!invoke) return null;
      const saved = await invoke('save_automation_job', {
        id: jobSpec.id || null,
        name: jobSpec.name,
        trigger: jobSpec.trigger || 'manual',
        intervalSecs: jobSpec.interval_secs || null,
        enabled: jobSpec.enabled !== false,
        steps: jobSpec.steps || []
      });
      emitUiEvent('aether:automations-updated', saved);
      return saved;
    },

    async toggleAutomationJob(id, enabled) {
      if (!invoke) return;
      await invoke('toggle_automation_job', { id, enabled });
      emitUiEvent('aether:automations-updated');
    },

    async deleteAutomationJob(id) {
      if (!invoke) return;
      await invoke('delete_automation_job', { id });
      emitUiEvent('aether:automations-updated');
    },

    async runAutomationNow(id) {
      if (!invoke) return null;
      const res = await invoke('run_automation_now', { id });
      emitUiEvent('aether:automations-updated');
      emitUiEvent('aether:traces-updated');
      return res;
    },

    async stopAutomationJob(id) {
      if (!invoke) return false;
      const ok = await invoke('stop_automation_job', { id });
      emitUiEvent('aether:automations-updated');
      return ok;
    }
  };

  if (listen) {
    listen('agent:trace', (event) => {
      const t = event.payload;
      if (!t) return;
      reconcileAgentPage(t.run_id).catch(() => {});
      emitUiEvent('aether:agent-trace', t);
      appendAgentBubble(
        `${t.model_route} · Step ${t.step_index} · Risk: ${t.risk}`,
        `${esc(t.thought)} <div class="text-[10.5px] font-mono opacity-65 mt-1">${esc(t.outcome)}</div>`
      );
      emitUiEvent('aether:traces-updated', t);
    });

    listen('agent:run_status', (event) => {
      closeApprovalBanner();
      const runId = String(event.payload?.run_id || '');
      reconcileAgentPage(runId)
        .catch(() => {})
        .finally(() => agentRunTabs.delete(runId));
      emitUiEvent('aether:agent-status', event.payload);
      emitUiEvent('aether:traces-updated', event.payload);
      emitUiEvent('aether:macros-updated', event.payload);
    });

    listen('agent:memory_saved', (event) => {
      emitUiEvent('aether:memories-updated', event.payload || {});
    });

    listen('agent:macro_saved', (event) => {
      const m = event.payload;
      if (m && m.goal_display) {
        appendAgentBubble('Macro Cached (SQLite WAL)', `Saved deterministic workflow for <strong>${esc(m.origin_host)}</strong> (${(m.steps || []).length} steps).`);
      }
      emitUiEvent('aether:macros-updated', m);
    });

    listen('automation:step', (event) => {
      const s = event.payload;
      if (!s) return;
      appendAgentBubble(
        `Automation · ${s.step_kind} · Risk: ${s.risk}`,
        `${esc(s.detail)} <div class="text-[10px] font-mono opacity-60 mt-0.5">${s.ok ? 'OK' : 'FAILED'}</div>`
      );
      emitUiEvent('aether:traces-updated', s);
    });

    listen('automation:status', (event) => {
      const st = event.payload || {};
      if (st.name && st.status) {
        appendAgentBubble('Automation Run', `<strong>${esc(st.name)}</strong> finished with status: <code>${esc(st.status)}</code>.`);
      }
      emitUiEvent('aether:automations-updated', st);
      emitUiEvent('aether:traces-updated', st);
    });

    listen('agent:plan', (event) => {
      emitUiEvent('aether:agent-plan', event.payload || {});
    });

    listen('agent:stage', (event) => {
      emitUiEvent('aether:agent-stage', event.payload || {});
    });

    listen('agent:run_error', (event) => {
      emitUiEvent('aether:agent-error', event.payload || {});
    });

    listen('agent:verification_report', (event) => {
      emitUiEvent('aether:agent-verification', event.payload || {});
    });

    listen('agent:approval_required', (event) => {
      if (event.payload) {
        emitUiEvent('aether:agent-approval', event.payload);
        // Legacy docked agent still supports approval while main-tab tasks
        // render their own approval surface inside the conversation.
        showApprovalBanner(event.payload);
      }
    });
    listen('agent:approval_closed', (event) => {
      const closed = event.payload || {};
      closeApprovalBanner(closed.approval_id);
      emitUiEvent('aether:agent-approval-closed', closed);
    });

    listen('itp:blocked', (event) => {
      emitUiEvent('aether:itp-updated', event.payload || {});
    });

    listen('browser:page-loaded', (event) => {
      const detail = event.payload || {};
      // A Lucia action may create/navigate a native child without going
      // through onNavigate(). Make that tab visible and keep geometry in sync
      // before the browser shell updates its URL and history.
      if (detail.tabId === currentTabId && /^https?:\/\//i.test(detail.url || '')) {
        currentTabIsExternal = true;
        loadedExternalTabs.add(detail.tabId);
        scheduleBoundsSync();
      }
      emitUiEvent('aether:page-loaded', detail);
    });
    listen('browser:popup-blocked', (event) => {
      emitUiEvent('aether:popup-blocked', event.payload || {});
    });
    listen('browser:permission-denied', (event) => {
      emitUiEvent('aether:site-permission-denied', event.payload || {});
    });

    listen('download:progress', (event) => {
      emitUiEvent('aether:download-progress', event.payload);
      emitUiEvent('aether:downloads-updated', event.payload);
    });

    listen('download:completed', (event) => {
      const d = event.payload;
      if (d && d.filename) {
        appendAgentBubble('Native Download', `Completed: <strong>${esc(d.filename)}</strong>`);
      }
      emitUiEvent('aether:download-progress', d);
      emitUiEvent('aether:downloads-updated', d);
    });

    listen('download:failed', (event) => {
      emitUiEvent('aether:download-progress', event.payload);
      emitUiEvent('aether:downloads-updated', event.payload);
    });

    listen('ai-chat:open-url', (event) => {
      if (event.payload && event.payload.url) {
        emitUiEvent('aether:open-url-from-chat', event.payload);
      }
    });
  }


  // Wire frameless window controls + automatic ResizeObserver / MutationObserver for child WebView2 geometry & z-order
  if (hasTauri && invoke) {
    const setupNativeShell = () => {
      const controls = document.querySelector('[aria-label="Window Controls"]');
      if (controls) {
        const btns = controls.querySelectorAll('button');
        if (btns[0]) {
          btns[0].addEventListener('click', (e) => {
            e.preventDefault();
            e.stopImmediatePropagation();
            invoke('window_close').catch(() => {});
          }, true);
        }
        if (btns[1]) {
          btns[1].addEventListener('click', (e) => {
            e.preventDefault();
            e.stopImmediatePropagation();
            invoke('window_minimize').catch(() => {});
          }, true);
        }
        if (btns[2]) {
          btns[2].addEventListener('click', (e) => {
            e.preventDefault();
            e.stopImmediatePropagation();
            invoke('window_toggle_maximize').catch(() => {});
          }, true);
        }
      }

      const toolbar = document.querySelector('header.macos-toolbar');
      if (toolbar) {
        toolbar.addEventListener('mousedown', (e) => {
          if (e.button !== 0) return;
          if (e.target.closest('button, input, a, select, textarea, form, .omnibar-shell, #omnibarDropdown')) {
            return;
          }
          if (e.detail === 2) {
            invoke('window_toggle_maximize').catch(() => {});
          } else {
            invoke('window_start_dragging').catch(() => {});
          }
        });
      }

      const canvas = document.getElementById('readingCanvas');
      if (canvas && typeof ResizeObserver !== 'undefined') {
        const ro = new ResizeObserver(() => scheduleBoundsSync());
        ro.observe(canvas);
      }

      const overlayIds = [
        'aiChatPopover',
        'privacyPopover',
        'downloadsPopover',
        'sharePopover',
        'omnibarDropdown',
        'tabOverviewModal',
        'safariSettingsModal',
        'newTabHistoryDrawer',
        'leftSidebar'
      ];
      if (typeof MutationObserver !== 'undefined') {
        const mo = new MutationObserver(() => scheduleBoundsSync());
        for (const id of overlayIds) {
          const el = document.getElementById(id);
          if (el) mo.observe(el, { attributes: true, attributeFilter: ['class', 'style'] });
        }
      }

      window.addEventListener('resize', scheduleBoundsSync);
    };

    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', setupNativeShell);
    } else {
      setupNativeShell();
    }
  }
})();
