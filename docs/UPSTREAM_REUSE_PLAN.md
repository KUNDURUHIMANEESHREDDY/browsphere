# Browsphere: upstream-first capability adoption

**Decision:** keep Windows Rust/Tauri 2/WebView2, the Lucia user-facing workflow,
existing permission gates, and independent task verification. Reuse mature third-party
implementations for generic mechanics. Thin, tested adapters are acceptable; new
general-purpose browser/agent/protocol frameworks are not.

## Evaluated upstreams (verified 2026-11-27)

| Capability to reuse | Upstream | License / language | Decision and constraints |
| --- | --- | --- | --- |
| Browser CDP, DOM locators, navigations, assertions, fixtures and traces | [Microsoft Playwright](https://github.com/microsoft/playwright); [official WebView2 guide](https://playwright.dev/docs/webview2) | Apache-2.0 / TypeScript, Node | **Adopted in CI:** pinned `playwright-core@1.65.1`, no managed Chromium download. See `tests/interop/`. Replace hand-written CDP test wiring incrementally only after live WebView2 checks. Not part of the release installer. |
| Model-driven actions, extraction and cross-site agent workflows | [Browserbase Stagehand v4](https://github.com/browserbase/stagehand), [existing-CDP browser config](https://github.com/browserbase/stagehand/blob/main/packages/docs/v4/configuration/browser.mdx) | MIT / TypeScript | **Candidate, not deployed.** Official `localBrowser.connect({cdpUrl})` allows attaching to a running Chromium instance. Verify WebView2 target discovery and lifecycle in disposable CI first. Do not expose an unsecured CDP endpoint in consumer builds; treat Stagehand results as proposals subject to Lucia's existing native policy/verification. Check model/provider availability before any ship decision. |
| MCP client protocol negotiation, transport, schema and session mechanics | [Official MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) (`rmcp`) | mixed Apache-2.0/MIT during transition / Rust | **Preferred production migration, not yet installed.** Replace custom protocol transport in `src-tauri/src/mcp.rs` and `src-tauri/src/agent/mcp.rs` only after protocol conformance against existing MCP regression suite. **Keep** explicit user allowlisting, read-only enforcement, endpoint restrictions, payload limits, cancellation, tokens from secure storage, and independent readback. The SDK does not replace these security policies. |
| Repeated unfamiliar-site research benchmarks | [ServiceNow BrowserGym](https://github.com/ServiceNow/BrowserGym) | Apache-2.0 / Python | **CI-only candidate.** Adopt upstream task environment/evaluation instead of inventing fake success scores. Run independent success criteria, 3+ trials per task, and report failures. Do not ship its Python dependencies in Windows consumer installer. |
| Agent action orchestration/recovery patterns | [Browser Use](https://github.com/browser-use/browser-use) | MIT / Python | **Evaluate, not deployed.** Useful as an independently runnable reference agent and comparator. Its Python/browser runtime would add substantial packaging/memory overhead if copied into the default Tauri app; no automatic replacement of Lucia's existing browser runtime. |
| Agent chat framework | [Microsoft AutoGen](https://github.com/microsoft/autogen) | repository archived; mixed licensing | **Do not depend on it** for this browser. Archived software is not a maintenance strategy for a security-sensitive desktop app. |
| Rust Stagehand SDK | [Stagehand Rust](https://github.com/browserbase/stagehand-rust) | archived/unmaintained | **Do not use**; upstream explicitly labels it unmaintained. Prefer maintained Stagehand TS if the CDP compatibility experiment passes. |

Before redistributing code or binaries, review the exact dependency version and its
`LICENSE` and `NOTICE` files. The MCP SDK's root LICENSE explicitly notes its
Apache-2.0/MIT relicensing transition; do not label all of it as exclusively Apache-2.0.

## First upstream integration: Playwright ↔ installed WebView2

`tests/interop/` now uses the actual maintained `playwright-core` package
rather than a bespoke CDP WebSocket implementation. The fresh NSIS acceptance
job installs **only this test dependency**, attaches to an isolated loopback
CDP endpoint, identifies Browsphere's Tauri host, validates the actual DOM,
and reads the native `capture_live_snapshot` bridge. Failures fail CI.

This validates browser interop, not model intelligence. Existing Lucia task
and verifier tests still run independently. The dependency is **not in
`ui/package.json` or `src-tauri/Cargo.toml`**, and is never bundled with the
consumer application.

Security boundary: CDP exposes powerful control. It is only permitted in
ephemeral Windows GitHub Actions runners using a temporary WebView2 debug
policy. Do not enable external debug ports by default in signed releases.
No Stagehand or Playwright agent gets credentials or page-driven permissions.

## Phased adoption gates

1. **Playwright (in progress):** pinned dependency, CI-only loopback policy,
   fresh installed-exe readback, no changes to Lucia production browser
   controls until demonstrated WebView2 compatibility. CI passing is required.
2. **Stagehand (candidate):** isolated attach/observe/extract experiment with
   no authenticated accounts, never model output → unchecked browser action.
   Must show equal-or-better task repeatability, bounded memory and reliable
   cancellation before switching Lucia action backends.
3. **rmcp (candidate):** fork-free crate-based transport replacement under
   unchanged user allowlists and readback; run hostile server and OAuth
   regression tests, plus HTTP/SSE/session/version conformance.
4. **BrowserGym/Browser Use (candidates):** external independent comparator
   and benchmark, not production Python sidecars. Only successful observed
   task outcomes count; a model starting or returning valid JSON is not success.
5. **Windows acceptance:** preserve NSIS clean-install, security, full-process
   memory p50/p95/p99 and genuinely unfamiliar-site real-model 3/3 evidence.
   Release signing and updater keys remain separate, mandatory gates.

**Current verdict:** The upstream Playwright adapter has been committed
but needs a passing fresh installed-WebView2 CI run. Stagehand, rmcp, BrowserGym,
and Browser Use have **not** been integrated. No claim of production readiness.
