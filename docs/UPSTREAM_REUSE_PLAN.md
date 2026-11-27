# Browsphere: upstream-first capability adoption

**Decision:** keep Windows Rust/Tauri 2/WebView2, the Lucia user-facing workflow,
existing permission gates, and independent task verification. Reuse mature third-party
implementations for generic mechanics. Thin, tested adapters are acceptable; new
general-purpose browser/agent/protocol frameworks are not.

## Evaluated upstreams (verified 2026-11-27)

| Capability to reuse | Upstream | License / language | Decision and constraints |
| --- | --- | --- | --- |
| Browser CDP, DOM locators, navigations, assertions, fixtures and traces | [Microsoft Playwright](https://github.com/microsoft/playwright); [official WebView2 guide](https://playwright.dev/docs/webview2) | Apache-2.0 / TypeScript, Node | **Adopted in CI:** pinned `playwright-core@1.65.1`, no managed Chromium download. See `tests/interop/`. Replace hand-written CDP test wiring incrementally only after live WebView2 checks. Not part of the release installer. |
| Model-driven actions, extraction and cross-site agent workflows | [Browserbase Stagehand v4](https://github.com/browserbase/stagehand), [existing-CDP browser config](https://github.com/browserbase/stagehand/blob/main/packages/docs/v4/configuration/browser.mdx) | MIT / TypeScript | **Experimental read-only SDK bridge added in `integrations/stagehand-mcp`, not deployed.** Official `localBrowser.connect({cdpUrl})` allows attaching to a running Chromium instance. Verify WebView2 target discovery and lifecycle in disposable CI first. Do not expose an unsecured CDP endpoint in consumer builds; treat Stagehand results as proposals subject to Lucia's existing native policy/verification. Check model/provider availability before any ship decision. |
| MCP client protocol negotiation, transport, schema and session mechanics | [Official MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) (`rmcp`) | mixed Apache-2.0/MIT during transition / Rust | **Official `rmcp` interoperability client added under `integrations/rmcp-probe`; native transport migration not yet installed.** Replace custom protocol transport in `src-tauri/src/mcp.rs` and `src-tauri/src/agent/mcp.rs` only after protocol conformance against existing MCP regression suite. **Keep** explicit user allowlisting, read-only enforcement, endpoint restrictions, payload limits, cancellation, tokens from secure storage, and independent readback. The SDK does not replace these security policies. |
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
2. **Stagehand (experimental branch):** isolated launch/observe/extract experiment with
   no authenticated accounts, never model output → unchecked browser action.
   Must show equal-or-better task repeatability, bounded memory and reliable
   cancellation before switching Lucia action backends.
3. **rmcp (experimental probe):** fork-free SDK connection and discovery probe, followed by a crate-based native transport replacement under
   unchanged user allowlists and readback; run hostile server and OAuth
   regression tests, plus HTTP/SSE/session/version conformance.
4. **BrowserGym/Browser Use (candidates):** external independent comparator
   and benchmark, not production Python sidecars. Only successful observed
   task outcomes count; a model starting or returning valid JSON is not success.
5. **Windows acceptance:** preserve NSIS clean-install, security, full-process
   memory p50/p95/p99 and genuinely unfamiliar-site real-model 3/3 evidence.
   Release signing and updater keys remain separate, mandatory gates.

**Current verdict:** The upstream Playwright adapter has been committed
but needs a passing fresh installed-WebView2 CI run. Stagehand's read-only MCP sidecar and the `rmcp` standalone interoperability probe exist only in the experimental branch; their live Windows verification is pending. BrowserGym and Browser Use are **not** integrated. No claim of production readiness.


## Integrated Windows acceptance — Stagehand upstream initialization boundary (27 Nov 2026)

This consolidation intentionally distinguishes **three different facts**:

- Official Stagehand v4 `localBrowser.connect({cdpUrl})` and the upstream page client can attach to a disposable installed Windows WebView2 test profile for read-only shell metadata. The CI probe uses upstream `browser.disconnect()` (introduced in Stagehand v4.4) to leave the installed browser alive.
- Stagehand `observe` and `extract` execute through its **separate, opt-in, loopback-only MCP sidecar** with isolated upstream Chromium, not by granting Stagehand control of a consumer WebView2 profile. Lucia still treats all results as untrusted until its own verifier checks evidence.
- Full `Stagehand.create({browser})` **has not been established as repeatably compatible** with installed WebView2. Combined installed-run [45969144624](https://github.com/KUNDURUHIMANEESHREDDY/browsphere/actions/runs/45969144624) recorded the upstream 60-second initialization timeout *after* official Playwright had observed 120 native controls. The upstream [Stagehand issue #2782](https://github.com/browserbase/stagehand/issues/2782) separately documents the same extension-worker handshake timeout, even with a valid browser session. Root cause in WebView2 is not proven.

The CI-only `integrations/stagehand-mcp/webview2-attach.ci.mjs` probe
therefore requires `BROWSPHERE_STAGEHAND_INIT_PROBE=1` for the **experimental**
full initializer test. When this flag is not set, the test only certifies
official `localBrowser.connect` + real Tauri shell readback, with the output
explicitly reporting `full_stagehand_initialized:false`. Do not portray this
as Stagehand actions/vision/extraction working inside WebView2.

The clean-installed NSIS test must still pass native task completion,
independent verification, installed WebView2 process-tree measurements, and
uninstall. No conditional path may silently replace a failed model task with a
simulated or unverified completion.

The upstream Microsoft Playwright MCP accessibility snapshot and native CDP
frame stream have independent *real WebView2* acceptance gates. Neither
requires Stagehand's extension handshake. The intent is to reuse working
upstream capabilities while isolating incomplete upstream integration—not
reimplement Stagehand internally.

## Confirmed installed-WebView2 Stagehand issue and non-deceptive gate

Two separate fresh-runner checks failed at the **upstream Stagehand**
extension handshake, including the supposed connect-only test:
[45969144624](https://github.com/KUNDURUHIMANEESHREDDY/browsphere/actions/runs/45969144624)
and [45971391182](https://github.com/KUNDURUHIMANEESHREDDY/browsphere/actions/runs/45971391182).
Official Playwright Core successfully attached and observed 120 native controls
before the Stagehand timeout, which points to a Stagehand-specific
compatibility/initialization failure, not general WebView2/CDP unavailability.
Stagehand's issue [#2782](https://github.com/browserbase/stagehand/issues/2782)
has the same timeout symptom, but does not establish its exact cause here.

**Correction to the earlier connect-only hypothesis:** Stagehand
`localBrowser.connect()` can itself participate in extension-worker
initialization, and is not a proven lightweight CDP-only API for WebView2.
Never describe it as a passed integrated native capability.

Windows clean-install E2E keeps the independently tested
`UpstreamMcpInterop` route: installed Lucia chatbox → existing Rust MCP
permissions → official Stagehand read-only MCP server → isolated Chromium
research fixture. It still requires the task's actual native terminal result,
policy enforcement and readback; no verifier is weakened and no synthetic
completion is substituted.

`-StagehandWebView2Experimental` is a separate opt-in CI-only compatibility
probe that calls the actual upstream `localBrowser.connect` and **fails on a
timeout**. It is not run in consumer releases or used as a dependency of the
working read-only MCP workflow. Its result must remain **unverified/failed**
until real native WebView2 evidence passes.

The real-Qwen unfamiliar-site run
[45969137732](https://github.com/KUNDURUHIMANEESHREDDY/browsphere/actions/runs/45969137732)
also failed: 5 explicitly unsuccessful attempts (3 web.dev, 2 W3C), then the
30-minute step deadline ended the sixth. The existing harness did not upload
its final JSON when killed at that deadline. Do not report 0/6 executed
completed failures without noting that sixth-run interruption.
