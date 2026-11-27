# Upstream-only browser action and benchmark adapters

The aim is to **reuse existing repositories, not implement a new browser,
CDP agent, LLM, MCP transport, or benchmarking framework**.

## Sources and scope

| Upstream | Exact component reused | Where | What this does NOT prove |
| --- | --- | --- | --- |
| Browserbase Stagehand 4.5.0 (MIT) | `localBrowser.connect`, `Stagehand.create`, `Stagehand.act(Action)` | `integrations/stagehand-mcp/webview2-native-actions.ci.mjs` | Real-model autonomy or deployment-safe CDP |
| Microsoft Playwright (Apache-2.0) | Existing `playwright-core` CI CDP inspection of installed WebView2 | `tests/interop` and native Windows runner | Browser replacement readiness |
| Ollama (MIT) | Native `/api/generate` structured output and timing counters | Existing router + numeric CI diagnostics | Fast enough Qwen without timed-out task evidence |
| ServiceNow BrowserGym 0.14.3 (Apache-2.0) | BrowserGym `gymnasium` environment, Chromium observation and action API | `tests/upstream/browsergym_smoke.py` | BrowserGym tasks are not automatically Lucia or WebView2 tasks |
| Browser Use 1.0.9 (MIT) | Its local `Browser('chromium')` CDP browser engine | `tests/upstream/browser_use_smoke.py` | No model agent is run or connected to a user's real browser |

The upstream benchmark runners are intentionally **separate, isolated
comparators**, not production dependencies. Browser Use 1.x can execute
model-written Python, so never connect it to a user's WebView2 session
or execute untrusted website tasks in the normal Tauri process.

## Real Stagehand WebView2 action acceptance

The installed-Windows fixture gate creates a disposable CDP-enabled
WebView2 profile and opens `generic-search.html` via Browsphere's
existing native tab creation. The Stagehand SDK then attaches to that
CI-only browser and performs three actual action-engine operations:

1. Fill the visible search field using Stagehand `act({method:'fill'})`.
2. Press Enter using Stagehand `act({method:'press'})`.
3. Click the observed API reference using Stagehand `act({method:'click'})`.

After **each** action, the runner invokes Browsphere's independent
native `capture_live_snapshot` command and requires the expected
live DOM state, plus the final `?doc=api` URL and confirmation
marker. The `data.success` flag returned by Stagehand alone is not
sufficient. This test never visits a live public website or an account.
Stagehand `selfHeal` is disabled to avoid model-directed retry in this
fixture gate.

The previously existing Stagehand MCP research tools stay **read-only**.
Mutating actions are NOT exposed through the regular user-facing MCP
server, as the native MCP client only approves explicitly configured
read-only tools. BrowserGem/Browser Use don't bypass this restriction.

## Reproducibility and strict gates

- Pinned Stagehand JS dependencies stay in the experimental
  `integrations/stagehand-mcp` module; no production installer dependency.
- BrowserGym and Browser Use are installed in **separate ephemeral CI jobs**,
  with exact upstream versions and no cloud keys or user profiles.
- A green Stagehand fixture or BrowserGym smoke test does **not** change
  the baseline real-Qwen unfamiliar-site success metric. Only actual
  native Lucia tasks on unseen live sites, with independent evidence and
  model-driven browser actions, may change it.
- Before production: inspect upstream supply chain, pin and commit
  full transitive dependency locks, measure startup/memory/p50-p95-p99,
  secure Chrome/WebView2 CDP, test install/uninstall/updater, run
  repeated real-model tasks, and verify Stagehand/Playwright coexistence
  without affecting private tabs.

See `docs/integrations/CPU_QWEN_STAGEHAND_NATIVE.md` for the earlier
real-Qwen timeout and privacy-preserving model observability work.
