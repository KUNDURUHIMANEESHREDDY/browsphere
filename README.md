# Browsphere — AI-Native Windows Desktop Browser (Tauri v2 + WebView2 + Rust)

A production-structured Windows desktop browser scaffold built with **Rust**, **Tauri v2**, and **WebView2**, featuring:
- **macOS-inspired light minimalist UI** with a **top-right AI Quick Chat popover** (no permanent right sidebar) and **non-intrusive background execution**.
- **Policy-Gated ReAct Browsing Agent** with deterministic risk classification (`low`, `medium`, `high`, `critical`), human-in-the-loop approval channels, untrusted page-data tagging, and step-by-step **execution trace logs**.
- **Hybrid Model Router** supporting local **Ollama** (e.g., `qwen3.5:4b`) and **LiteLLM** proxy/multi-provider endpoints with per-domain routing rules.
- **SQLite WAL** (`PRAGMA journal_mode = WAL;`) persistence for browsing history, downloads, agent runs, and execution traces.
- **Windows Credential Manager** (`keyring` crate on Windows) for storing API keys and secrets securely outside plain config files.
- **Intelligent Tracking Prevention (ITP)** tracker/fingerprinting blocker & native download manager.

## Quick Start (Windows)

1. Install **Rust** (`rustup`), **Node.js** (v26+ supported), and **WebView2 Runtime** (preinstalled on modern Windows 10/11).
2. Ensure **Ollama** is running locally (`ollama pull qwen3.5:4b`) or configure a **LiteLLM** endpoint.
3. Run `build-windows-installer.bat` or run `cargo tauri dev`.


## Optional AWS account connection

In the Windows app open Settings > AI & ML > Connect AWS. The button uses your existing AWS CLI v2 profile, opens the official AWS browser login when necessary, and discovers existing private Browsphere resources automatically. It saves resource identifiers locally rather than asking you to paste access keys. Bedrock inference and cloud data transfers remain off until explicitly enabled. For prerequisites, resource naming rules, permissions, and security details, see AWS_INTEGRATION.md.

---

## Project status — 22 November 2026

> **Status:** Active development / working prototype. Items marked *built* are implemented in the repository; they are **not** a claim of production readiness. Some features need external services, accounts, or additional end-to-end testing.

### What has been built

| Area | Implemented so far |
| --- | --- |
| Windows browser foundation | Rust + Tauri v2 + WebView2 desktop app, macOS-inspired interface, browsing controls, local data storage, private-browsing controls, download handling, and tracking-prevention components. |
| Lucia AI chat | Text chat with local Ollama and configurable LiteLLM-compatible endpoints; endpoint-specific provider model discovery through a **Fetch** flow, with API keys handled separately from UI preferences. |
| Agent orchestration | Model-generated task plans routed to **Research, Browser, Comparison, Form, Data, and Verification** specialists. Each specialist has task-specific instructions and allowed actions. The ReAct runner performs browser actions against observed pages rather than using canned responses. |
| Agent safeguards and verification | Sensitive/high-risk actions pass through approval policies. The verifier checks claims against observed URLs and page excerpts, rejects unsupported completion claims, and records evidence. **This checks attribution to observed pages, not independent truth of those pages.** |
| Lucia voice | Separate microphone input and opt-in voice-agent mode, dedicated voice system prompts, ElevenLabs integration with Kokoro and Windows-voice fallback paths, and simplified Lucia controls. |
| Memory, privacy, and news | SQLite WAL persistence, memory management under Privacy settings, and a scrollable news feed backed by live Hacker News items. |
| AWS integration | Optional AWS CLI-profile connection and resource discovery for Bedrock, S3, DynamoDB, Cognito, and Lambda. Account-side S3, DynamoDB, Cognito, and Lambda resources have been configured and tested in development; no automatic cloud sync or Bedrock routing is enabled by default. |
| Packaging and tests | Windows build/installer script, a GitHub Actions workflow, Rust tests, and automated WebView2/Chromium UI smoke-test scripts. |

**Verification snapshot:** The latest Rust test run completed with **51 passed, 0 failed, 4 ignored** (the ignored tests require configured local models). A separate real-local-model verifier test passed, and a previously recorded desktop E2E run passed **15/15** UI/integration checks. The updated Windows release compilation also succeeded. These are development test results, not exhaustive production certification.

### What's pending

- **End-to-end Lucia reliability:** Exercise multi-step plans with real model responses on diverse live sites, including failed navigation, ambiguous instructions, form approvals, retries, cross-site research, and final evidence-based reporting. Extend regression tests so an agent cannot claim success merely because a step was attempted.
- **Verification quality:** Broaden source coverage, failure handling, comparison tests, and checks for unsupported claims. Observed source text still may contain inaccurate information; attribution checks do not independently establish factual accuracy.
- **Voice validation:** Test ElevenLabs with a real authorized API key and quota; install/run the optional Kokoro service for offline fallback testing; recheck microphone permissions, speech behavior, and the native Windows UI after restart. Voice is push-to-talk, not a full-duplex assistant.
- **AWS completion:** Recheck Bedrock inference when the account's daily token quota permits it; validate interactive Cognito sign-in with an actual registered user, and assess MFA, token refresh, and recovery if required. Test cloud behavior without enabling surprise charges or automatic data transfers.
- **Release readiness:** Run a fresh full desktop regression and installer smoke test, validate the GitHub Actions Windows build, review privacy/security edge cases, and document supported configuration and distribution steps.

### Ongoing / next engineering task

**Priority: make Lucia's real multi-agent browser workflow dependable end to end.** The model should choose appropriate specialists dynamically, perform authorized browser actions on the actual WebView, re-observe results, and issue a final answer **only when the observed evidence supports it**. The immediate focus is expanding realistic live-model and live-browser tests around the new evidence-verification gate and correcting any failures they expose—**not** adding hardcoded task outcomes.

After that, finish native voice/AWS integration checks and release testing. This section describes the **current development priority**, not an unattended background process.

For details, see [AWS integration](AWS_INTEGRATION.md), [Voice Agent](VOICE_AGENT.md), and [Windows E2E report](tests/e2e/REPORT.md).

### Browser maturity milestone — 22 November 2026

The first **everyday-browser reliability** update is now implemented:

- **Session recovery:** ordinary open tabs, active tab and bounded per-tab URL history are saved locally and restored on launch. Closing/switching/navigating tabs updates the snapshot. Private-mode tab URLs are excluded.
- **Persistent library:** bookmarks and reading-list items survive restarts, without storing password/token-bearing URLs.
- **Real WebView2 navigation:** completed child-webview page loads synchronize link clicks/redirects into the tab's URL and navigation history. Restored tabs lazily create their WebView2 instance when selected.
- **Working reload and navigation shortcuts:** Reload now executes in the actual child webview; F5/Ctrl+R, Ctrl+Tab, Alt+Left/Right are supported when the Browsphere UI has keyboard focus.
- **Safer input and history:** the omnibar preserves HTTP and HTTPS URLs, searches ordinary text, rejects unsupported URL schemes, and avoids saving common OAuth/token URL parameters in browser history. Private navigation skips history insertion.
- **Honest UI state:** Empty Page is blank; unavailable native website rendering no longer displays simulated website/article content; navigation failures show an error; fabricated password records and nonfunctional pre-enabled extension listings were removed. The Extensions pane now explains that extension hosting is not available.

**Tests:** Run `node tests/browser-state.test.cjs` for session, bookmark, URL and privacy filtering checks, and `cargo test --workspace --offline` for the Rust test suite. Tests validate implementation paths but do not prove real-world site compatibility or full private-browsing isolation.

**Still pending before Chrome-level maturity:** full native-browser UI regression on diverse websites, engine-native back/forward state and scroll restoration, external-window/popup handling, proper isolated private browsing (cookies/storage currently share the default WebView2 profile), extension hosting, permission management, polished download UX, standards/security review (including CSP), auto-update signing/release pipeline, and performance/accessibility/reliability testing. Some keyboard shortcuts may not work while a child WebView has focus.

**Next priority:** native WebView2 navigation/permissions/private-profile integration and repeatable end-to-end desktop browsing tests. This is a development roadmap, not proof of Chrome/Edge feature parity.

## Lucia production-hardening update — 22 November 2026

The agent runner now binds clicks and typing to a fresh live DOM snapshot, uses observed control labels/sensitivity for approvals, stops on cancelled/rejected/expired approvals, blocks unsafe download URLs, and redacts sensitive action traces. Workflow macro action persistence/replay is disabled until it has a safe verified design.

The Rust regression run passed **58 tests, 0 failed, 4 ignored**, and the separate real-local-model intent, specialist-action, and evidence-verifier tests passed. An updated packaged Windows multi-step browser E2E test is **still pending**; this is not a production-ready release.

See [Lucia production gates](LUCIA_PRODUCTION_GATE.md) for exact implementation details and remaining release requirements.
