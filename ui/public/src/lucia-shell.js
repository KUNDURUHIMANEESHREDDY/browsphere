(function () {
      'use strict';

      function getInvoke() {
        if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
          return window.__TAURI__.core.invoke.bind(window.__TAURI__.core);
        }
        if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
          return function (cmd, args) { return window.__TAURI_INTERNALS__.invoke(cmd, args); };
        }
        return null;
      }

      function getListen() {
        if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.listen === 'function') {
          return window.__TAURI__.event.listen.bind(window.__TAURI__.event);
        }
        return null;
      }

      var invoke = getInvoke();
      var listen = getListen();

      var activeUrl = 'safari://start';
      var activeTitle = 'New Tab';
      var activeApprovalId = null;
      var activeVoiceRunId = null;
      var chatTurns = [];
      var isThinking = false;
      var lastAgentError = '';
      var lastVerifiedSummary = '';
      var emptyStateMarkup = document.getElementById('chatMessagesStream').innerHTML;

      function esc(str) {
        return String(str == null ? '' : str)
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;');
      }

      function applyTheme(dark) {
        var html = document.documentElement;
        html.setAttribute('data-theme', dark ? 'dark' : 'light');
        html.classList.toggle('dark', Boolean(dark));
      }

      function applyContext(title, url, isPrivate) {
        if (typeof isPrivate === 'boolean') {
          document.documentElement.setAttribute('data-private', isPrivate ? 'true' : 'false');
        }
        if (title) activeTitle = title;
        if (url) activeUrl = url;
        var pill = document.getElementById('chatContextPill');
        var context = 'Active page';
        try {
          var parsed = new URL(activeUrl);
          if (parsed.protocol === 'https:' || parsed.protocol === 'http:') context = parsed.hostname;
        } catch (_) {}
        if (pill) pill.textContent = context;
        var empty = document.getElementById('emptyHeading');
        if (empty) empty.textContent = 'Lucia';
      }

      function appendUserBubble(text) {
        var stream = document.getElementById('chatMessagesStream');
        var empty = document.getElementById('luciaEmptyState');
        if (empty) empty.remove();
        stream.classList.add('has-messages');

        var row = document.createElement('div');
        row.className = 'flex justify-end';
        row.innerHTML = '<div class="bg-[#007AFF] text-white px-3 py-2 rounded-2xl rounded-br-sm max-w-[85%] text-[12px]">' + esc(text) + '</div>';
        stream.appendChild(row);
        stream.scrollTop = stream.scrollHeight;
      }

      function appendAgentBubble(meta, bodyHtml) {
        var stream = document.getElementById('chatMessagesStream');
        var empty = document.getElementById('luciaEmptyState');
        if (empty) empty.remove();
        stream.classList.add('has-messages');

        var row = document.createElement('div');
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

      function closeApprovalBanner(id) {
        if (id && activeApprovalId !== id) return;
        activeApprovalId = null;
        document.getElementById('tauriApprovalCard')?.remove();
      }

      function showApprovalBanner(req) {
        if (!activeVoiceRunId || req.run_id !== activeVoiceRunId) return;
        closeApprovalBanner();
        activeApprovalId = req.approval_id;
        var stream = document.getElementById('chatMessagesStream');
        var existing = document.getElementById('tauriApprovalCard');
        if (existing) existing.remove();

        var card = document.createElement('div');
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

        var approvalId = req.approval_id;
        async function decide(approved) {
          if (activeApprovalId !== approvalId || !invoke) return;
          card.querySelectorAll('button').forEach(function (btn) { btn.disabled = true; });
          try {
            await invoke('resolve_approval', { approvalId: approvalId, approved: approved });
          } catch (err) {
            appendAgentBubble('Lucia · Approval expired', esc(String(err)));
          } finally {
            closeApprovalBanner(approvalId);
          }
        }
        document.getElementById('tauriApproveBtn').addEventListener('click', function () { decide(true); });
        document.getElementById('tauriRejectBtn').addEventListener('click', function () { decide(false); });
      }

      function setLuciaStatus(message) {
        var status = document.getElementById('browsphereVoiceStatus');
        if (status && window.browsphereVoice?.enabled !== true) {
          status.textContent = message;
          status.dataset.idle = message === 'Ready' ? 'true' : 'false';
        }
      }

      function speakLucia(text, model) {
        window.dispatchEvent(new CustomEvent('browsphere:assistant-text', {
          detail: { text: text, model: model || 'Lucia' }
        }));
      }

      async function submitGoal(promptText, forceTask) {
        var text = String(promptText || '').trim();
        if (!text || isThinking) return;
        if (text.length > 8000) {
          appendAgentBubble('Lucia', 'The message is too long. Please shorten it to 8,000 characters.');
          return;
        }
        appendUserBubble(text);
        if (!invoke) {
          appendAgentBubble('Lucia', 'This feature needs the Browsphere desktop app.');
          return;
        }
        isThinking = true;
        setLuciaStatus('Lucia is thinking…');
        try {
          // The configured LLM decides whether this is conversation or an
          // explicit browser task. If classification is unavailable, prefer
          // a harmless chat request, not an unrequested browser action.
          var intent = { mode: 'chat' };
          if (forceTask === true) {
            // Explicit Run Task should stay a browser-agent request.
            intent = { mode: 'browser_task' };
          } else {
            try {
              intent = await invoke('route_lucia_intent', { message: text });
            } catch (routeError) {
              console.warn('Lucia intent model unavailable; using chat mode:', routeError);
            }
          }

          if (intent && intent.mode === 'browser_task') {
            var snapshot = null;
            try { snapshot = await invoke('capture_live_snapshot'); } catch (_) {}
            if (!snapshot) {
              snapshot = {
                origin: activeUrl, title: activeTitle,
                text_excerpt: '', interactive_nodes: []
              };
            }
            lastAgentError = '';
            lastVerifiedSummary = '';
            activeVoiceRunId = await invoke('start_agent_run', {
              goal: text,
              snapshot: snapshot,
              allowedDomains: [],
              maxSteps: 26,
              allowLearning: false,
              isPrivate: document.documentElement.getAttribute('data-private') === 'true',
              voiceMode: window.browsphereVoice?.enabled === true
            });
            setLuciaStatus('Lucia is working on your task…');
          } else {
            var messages = chatTurns.concat([{ role: 'user', content: text }]).slice(-19);
            var result = await invoke('ask_main_tab_ai', {
              messages: messages,
              voiceMode: window.browsphereVoice?.enabled === true
            });
            if (!result || !result.answer || !result.answer.trim()) {
              throw new Error('The selected model returned an empty answer.');
            }
            var answer = result.answer.trim();
            chatTurns.push({ role: 'user', content: text }, { role: 'assistant', content: answer });
            if (chatTurns.length > 18) chatTurns.splice(0, 2);
            appendAgentBubble('Lucia · ' + (result.model || 'Configured model'),
              esc(answer).replace(/\r?\n/g, '<div style="height:5px"></div>'));
            speakLucia(answer, result.model);
            setLuciaStatus('Ready');
          }
        } catch (err) {
          var explanation = String(err);
          appendAgentBubble('Lucia · Request failed', esc(explanation));
          setLuciaStatus('Request failed');
        } finally {
          isThinking = false;
        }
      }

      function clearMessages() {
        var stream = document.getElementById('chatMessagesStream');
        stream.innerHTML = emptyStateMarkup;
        stream.classList.remove('has-messages');
        var heading = document.getElementById('emptyHeading');
        if (heading) heading.textContent = 'Lucia';
        if (activeVoiceRunId && invoke) {
          invoke('cancel_agent_run').catch(function () {});
        }
        closeApprovalBanner();
        activeVoiceRunId = null;
        chatTurns = [];
        lastAgentError = '';
        lastVerifiedSummary = '';
        setLuciaStatus('Ready');
      }

      // Header drag via native Tauri window drag
      document.getElementById('aiChatHeader').addEventListener('mousedown', function (e) {
        if (e.button !== 0) return;
        if (e.target.closest('button')) return;
        if (invoke) invoke('ai_chat_start_dragging').catch(function () {});
      });

      document.getElementById('btnClose').addEventListener('click', function () {
        if (invoke) invoke('ai_chat_close').catch(function () {});
      });
      document.getElementById('btnDock').addEventListener('click', function () {
        if (invoke) invoke('dock_ai_chat_window').catch(function () {});
      });
      document.getElementById('btnNew').addEventListener('click', clearMessages);
      document.getElementById('btnMinimize').addEventListener('click', function () {
        if (invoke) invoke('ai_chat_close').catch(function () {});
      });

      document.getElementById('chatComposerForm').addEventListener('submit', function (e) {
        e.preventDefault();
        var inp = document.getElementById('chatPromptInput');
        var val = inp.value.trim();
        if (!val) return;
        inp.value = '';
        submitGoal(val);
      });

      document.addEventListener('keydown', function (e) {
        if (e.key === 'Escape' && invoke) {
          invoke('ai_chat_close').catch(function () {});
        }
      });

      try {
        var boot = window.__AETHER_CHAT_INIT__;
        if (boot) {
          if (typeof boot.dark === 'boolean') applyTheme(boot.dark);
          if (boot.title || boot.url) applyContext(boot.title || '', boot.url || '', boot.is_private);
        }
      } catch (_) {}

      if (listen) {
        listen('ai-chat:context', function (ev) {
          var p = ev.payload || {};
          applyContext(p.title, p.url, p.is_private);
          if (typeof p.dark === 'boolean') applyTheme(p.dark);
        });
        listen('ai-chat:theme', function (ev) {
          var p = ev.payload || {};
          if (typeof p.dark === 'boolean') applyTheme(p.dark);
        });
        listen('ai-chat:prompt', function (ev) {
          var p = ev.payload || {};
          if (p.prompt) submitGoal(p.prompt, p.force_task === true);
        });
        listen('agent:plan', function (ev) {
          var p = ev.payload || {};
          var steps = p.plan && p.plan.stages || [];
          if (!steps.length) return;
          appendAgentBubble('Lucia · Task Plan', '<ol class="list-decimal pl-4 space-y-1">' +
            steps.map(function (step) {
              return '<li><strong>' + esc(step.specialist) + '</strong> — ' +
                esc(step.objective) + '</li>';
            }).join('') + '</ol>');
        });
        listen('agent:stage', function (ev) {
          var step = ev.payload || {};
          if (!step.specialist || (activeVoiceRunId && step.run_id !== activeVoiceRunId)) return;
          if (step.status === 'completed' && /Verification Agent/i.test(step.specialist)) {
            lastVerifiedSummary = String(step.summary || '').slice(0, 1500);
          }
          appendAgentBubble('Lucia · ' + step.specialist,
            step.status === 'completed'
              ? 'Stage completed after observing ' + esc(step.evidence_count || 0) + ' sources.'
              : 'Working on ' + esc(step.objective || 'assigned task'));
        });
        listen('agent:verification_report', function (ev) {
          var payload = ev.payload || {};
          if (activeVoiceRunId && payload.run_id !== activeVoiceRunId) return;
          if (payload.report?.status === 'evidence_checked') {
            lastVerifiedSummary = String(payload.report.summary || '').slice(0, 7000);
            appendAgentBubble('Lucia · Sources checked',
              esc(payload.report.findings.length) +
              ' findings matched to captured webpage excerpts. Sources appear in the result.');
          }
        });
        listen('agent:run_error', function (ev) {
          var failure = ev.payload || {};
          if (activeVoiceRunId && failure.run_id !== activeVoiceRunId) return;
          lastAgentError = String(failure.message || 'The browser task could not run.');
        });
        listen('agent:run_status', function (ev) {
          var result = ev.payload || {};
          if (!activeVoiceRunId || result.run_id !== activeVoiceRunId) return;
          closeApprovalBanner();
          activeVoiceRunId = null;
          setLuciaStatus('Ready');
          if (result.status === 'completed') {
            var completedText = lastVerifiedSummary ||
              'The browser steps finished, but I do not have a final verification summary.';
            appendAgentBubble('Lucia · Task result', esc(completedText));
            speakLucia(completedText);
          } else if (result.status === 'cancelled') {
            appendAgentBubble('Lucia', 'The browser task was stopped.');
          } else {
            var problem = lastAgentError || (
              result.status === 'max_steps'
                ? 'The task reached its step limit without enough verified evidence.'
                : 'The browser could not complete all the requested steps.'
            );
            appendAgentBubble('Lucia · Task not completed', esc(problem));
            speakLucia('I could not complete the browser task. ' + problem);
          }
          lastAgentError = '';
          lastVerifiedSummary = '';
        });
        listen('agent:trace', function (ev) {
          var t = ev.payload;
          if (!t || (activeVoiceRunId && t.run_id !== activeVoiceRunId)) return;
          if (/^(FAILED|DENIED|HALTED|REJECTED)/i.test(t.outcome || '')) {
            lastAgentError = String(t.outcome).slice(0, 360);
          }
          appendAgentBubble(
            t.model_route + ' · Step ' + t.step_index + ' · Risk: ' + t.risk,
            esc(t.thought) + '<div class="text-[10.5px] font-mono opacity-65 mt-1">' + esc(t.outcome) + '</div>'
          );
        });
        listen('agent:approval_required', function (ev) {
          if (ev.payload) showApprovalBanner(ev.payload);
        });
        listen('agent:approval_closed', function (ev) {
          var closed = ev.payload || {};
          closeApprovalBanner(closed.approval_id);
        });
        listen('agent:macro_saved', function (ev) {
          var m = ev.payload;
          if (m && m.goal_display) {
            appendAgentBubble('Macro Cached (SQLite WAL)', 'Saved deterministic workflow for <strong>' + esc(m.origin_host) + '</strong> (' + ((m.steps && m.steps.length) || 0) + ' steps).');
          }
        });
      }
    })();
