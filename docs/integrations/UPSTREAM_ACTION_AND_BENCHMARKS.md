# Upstream capability integration: supported execution paths

Goal: **reuse upstream browsers, accessibility engines, native-model APIs,
MCP SDKs and benchmark environments; do not implement another CDP agent**.

## Verified compatibility boundary

The clean-installed Windows run
[45967899714](https://github.com/KUNDURUHIMANEESHREDDY/browsphere/actions/runs/45967899714)
established the following:

- The NSIS installer successfully installed and launched Browsphere.
- Microsoft's **Playwright** attached to installed WebView2 and observed 120
  interactive controls via the existing smoke test.
- **Stagehand 4.5.0 `localBrowser.connect` timed out after 60 seconds**
  while trying to initialize against that same WebView2 process. This is an
  upstream compatibility blocker, not permission to reinvent CDP.
- Until the incompatibility is resolved upstream, Stagehand runs inside its
  **own isolated Chromium**, where its read-only MCP `inspect`, `observe`
  and `extract` tools and a deterministic `act(Action)` smoke can be tested.
- The consumer installer does not auto-enable a CDP endpoint, a Stagehand
  executable, a model-written Python REPL or unapproved MCP write tools.

## External repositories and the actual integration

| Upstream | Capability reused | Integration evidence | Limitation |
| --- | --- | --- | --- |
| Microsoft Playwright (Apache-2.0) | `chromium.connectOverCDP`, locators `fill`, `press`, `click` | `tests/interop/playwright-native-actions.cjs` | CI-only disposable native profile; no public-site task claim |
| Browserbase Stagehand 4.5.0 (MIT) | `localBrowser.launch`, `Stagehand.create`, `act(Action)`, `observe`, `extract` | `integrations/stagehand-mcp/stagehand-actions.smoke.mjs`, existing read-only MCP sidecar | Native WebView2 attach not yet supported |
| Official MCP Rust SDK (Apache/MIT) | `rmcp` with Streamable HTTP client / tool discovery | `integrations/rmcp-probe` | Production MCP transport is still custom; SDK probe isn't migration |
| Ollama (MIT) | Native `/api/generate` constrained output and inference counters | Existing Rust model router and real-Qwen CI diagnostics | Timeouts and verified completion remain distinct |
| ServiceNow BrowserGym 0.14.3 (Apache-2.0) | Official Gymnasium browser environment, action runner and observations | `tests/upstream/browsergym_smoke.py` | Comparator environment only, not a native task |
| Browser Use 1.0.9 (MIT) | Official local `Browser('chromium')` engine | `tests/upstream/browser_use_smoke.py` | Its model-written Python execution is not safe to enable in a user WebView2 session |

BrowserGym and Browser Use run in **separate ephemeral Linux CI jobs**
with exact top-level version pins, no credentials, and no user profiles.
Their browser-engine smoke tests pass in
[run 45969147531](https://github.com/KUNDURUHIMANEESHREDDY/browsphere/actions/runs/45969147531).
This proves install/engine compatibility **only**, not head-to-head agent
success or better latency.

## Installed WebView2 Playwright action readback

The Windows acceptance runner opens a disposable
`generic-search.html` task tab through Browsphere's existing native UI.
Official Playwright then fills its observed search field, submits with
Enter and clicks its API reference result. After each Playwright action,
the separate Browsphere `capture_live_snapshot` command must observe
the expected state, and the final observed URL must end in `?doc=api`.
An upstream tool reporting success alone is insufficient.

This is a fixed, reproducible fixture acceptance, **not** a real model
or unfamiliar website task. The Stagehand MCP read-only tool discovery
and call through Lucia's native chatbox are separate gates.

## Security and next acceptance gates

No browser sidecar is trusted to execute an arbitrary model/webpage
instruction against a consumer profile. Native policy, sensitive-field
protection, authenticated MCP permissions, DOM freshness and
independent source verification stay authoritative.

To release beyond this branch: pin full transitive dependency locks,
verify supply-chain licensing and isolation, demonstrate genuine Qwen
completion of the six unfamiliar-site tasks three times per scenario,
benchmark faster local models on identical genuine prompts, measure
installed-process memory and startup p50/p95/p99, verify updater and
uninstall, and complete an independent security review.

Do **not** merge the stacked experimental PRs or call Browsphere
production-ready merely because these upstream fixture tests compile.
