# Browsphere — AI-Native Windows Desktop Browser (Tauri v2 + WebView2 + Rust)

A production-structured Windows desktop browser scaffold built with **Rust**, **Tauri v2**, and **WebView2**, featuring:
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

**Tests:** Run `node tests/browser-state.test.cjs` for session, bookmark, URL and privacy filtering checks, and `cargo test --workspace --offline` for the Rust test suite. Tests validate implementation paths but do not prove real-world site compatibility; a separate desktop smoke test now verifies cookie and localStorage separation between ordinary and private WebView2 sessions.

**Still pending before Chrome-level maturity:** full native-browser UI regression on diverse websites, engine-native back/forward state and scroll restoration, user-controlled popup and per-site permission handling (currently denied by default), extension hosting, polished download UX, a broader privacy/security audit, signed auto-update releases, and performance/accessibility/reliability testing. Some keyboard shortcuts may not work while a child WebView has focus.

**Next priority:** native WebView2 navigation/permissions/private-profile integration and repeatable end-to-end desktop browsing tests. This is a development roadmap, not proof of Chrome/Edge feature parity.

## Lucia production-hardening update — 22 November 2026

The agent runner now binds clicks and typing to a fresh live DOM snapshot, uses observed control labels/sensitivity for approvals, stops on cancelled/rejected/expired approvals, blocks unsafe download URLs, and redacts sensitive action traces. Workflow macro action persistence/replay is disabled until it has a safe verified design.

The expanded Lucia backend regression run passed **59 tests, 0 failed, 4 ignored**. A real native Windows WebView2 integration test and one end-to-end Ollama/Qwen Browser → Verification workflow both passed against a harmless local website. Separate desktop tests confirmed that rejected and cancelled form approvals never submit the form. An unsigned Windows NSIS installer was also packaged successfully. These limited passes do **not** certify multi-site reliability, installer installation/upgrade, code signing, or production readiness.

**Autonomous forms update:** Lucia's Form Agent can submit routine low-risk forms (such as feedback) without waiting for approval. The action policy still asks for confirmation before consequential actions, payment/account changes, applications, or submission of sensitive data. Confirmed submissions advance to the independent verifier without clicking submit a second time; ambiguous submission outcomes stop safely rather than retrying.

See [Lucia production gates](LUCIA_PRODUCTION_GATE.md) for exact implementation details and remaining release requirements.

## Navigation, privacy, and security hardening — 22 November 2026

This milestone fixes the confirmed address-bar regression and adds native private browsing isolation in the Windows application.

- **Address-bar reliability:** Both active-tab renders and Settings refresh use the same URL formatter. Full-address mode preserves `https://` or `http://` and the complete path; compact mode shows the host. Explicit navigation and tab changes refresh the display even if the address input previously had focus.
- **Private WebView2 profiles:** Private mode opens a separate set of private tabs using Tauri's `.incognito(true)` WebView2 option. Closing private mode closes those native WebViews and restores the regular tab group. Native history and download-history recording remain disabled for private tabs.
- **Default-deny sensitive site capabilities:** External webpages cannot silently obtain permission for camera, microphone, location, notifications, or other WebView2 permission requests. Popups are blocked and reported in the UI until controlled per-site permission and popup workflows are developed. These protections can prevent some sign-in or video-call flows until a permission UI exists.
- **Content Security Policy:** The main browser and Lucia CSS/JavaScript have been moved into versioned local assets, allowing a CSP that blocks inline JavaScript and third-party scripts. Inline CSS remains allowed for existing UI styling. The CSP applies to app pages; external websites retain their own web security policies.
- **Release updates:** The unsigned auto-updater check has been disabled instead of configuring a dummy signing key. Signed automatic updates must be enabled only after the publisher configures a real public verification key, a securely managed signing secret, and a validated release manifest.

**Validation:** JavaScript browser-state regression tests passed; **58 Rust tests passed** and 4 optional local-model tests were ignored. A separate isolated Windows WebView2 smoke session passed **12/12** navigation/CSP/private-profile checks plus **6/6** additional cookie, popup, and Lucia checks. Both the normal browser and Lucia blocked injected inline scripts; private views could not access normal cookies or localStorage while normal-site data remained available after private mode ended. Development builds compiled successfully.

**Outstanding release gates:** Signed Windows installer/update pipeline; per-origin permissions and controlled popup workflows; production certificate and CSP audit; multi-site cookie/SSO compatibility tests; browser-native back/forward, shortcut handling in child WebViews, and full cross-site desktop regression. These tests are not a proof of Chrome equivalence or comprehensive browser security.

## Site permissions, popup consent, and signed release preparation — 22 November 2026

The browser now has an explicit **lock icon in the address bar** for the current HTTPS site. The site panel shows Camera, Microphone, Location, and Notifications as **Blocked** by default. The user can grant/revoke each permission for that exact top-level HTTPS origin; normal-profile grants are stored in `BrowsphereAI/site-permissions.json`. Insecure origins and other high-risk permission types cannot be granted from these controls. Private tabs remain deny-only and never inherit persistent grants.

External website popups are denied by default. A popup request presents its actual destination and requires the user to select **Open once in new tab**. Browsphere opens the approved link in a normal browser tab rather than automatically creating an uncontrolled external window. **Block** closes the request. This is intentionally conservative: OAuth and SSO websites that require a genuine popup may need additional handling.

**Validation:** `cargo test --workspace --offline` passed **61 tests**, 0 failed, with 4 optional live-model tests ignored. The separately launched Windows WebView2 compatibility suite (`node tests/e2e/browser-compat.mjs`) passed **17/17 checks** across Example Domain, GitHub, Wikipedia, and Hacker News: website loading, host display, permission control/grant/revoke, blocked popup behavior, and explicit tab opening. The live test used a disposable browser profile and no production account credentials.

A separate manual workflow, [Signed Windows installer release](.github/workflows/release-signed-windows.yml), is **implemented but not activated**. It requires a protected GitHub `windows-release` environment and real Authenticode certificate secrets, and refuses to publish unsigned binaries. Read [Release signing guide](RELEASE_SIGNING.md) to configure it. There is no production signing certificate, signed release, or enabled Tauri auto-update mechanism yet.

**Remaining production work:** validate certificate provisioning and an actual signed installer, add origin-aware permission handling for embedded frames (the high-level Tauri callback only supplies the main WebView URL), restore trusted SSO popup compatibility, test site permissions across more websites and operating conditions, and complete installer/security audits. These changes do not establish Chrome-level feature parity.
