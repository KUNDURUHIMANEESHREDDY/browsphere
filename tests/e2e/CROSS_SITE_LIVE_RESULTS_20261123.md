# Browsphere cross-site live Windows test — 23 November 2026

## Environment
- Source: `main` at `add7ba7`, rebuilt on the authorized Windows computer using `cargo build --release --locked`.
- Runtime: real Tauri/WebView2 Windows app in a disposable local profile (no saved browsing history or accounts).
- Model: local Ollama `qwen2.5:3b-instruct` for the genuine model-driven run. No production or hosted cloud model was used in these live-site tests.
- Reproduction: `powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-lucia-cross-site-live.ps1`; use `-OnlyCompatibility` to run browser compatibility without Lucia model tasks.
- Both scripts report failures using nonzero process exit codes; no secret values or user page text are recorded.

## Results
### Real-site WebView2 browser compatibility: PASS (17/17)
- Example Domain, GitHub, Wikipedia, and Hacker News loaded as actual native WebView2 pages; all four synchronized the visible address bar.
- The test now waits for Browsphere shell initialization and validates the *renderer document URL* (not only the CDP target URL), avoiding a false pass caused by stale content during navigation.
- Website permissions panel, deny-by-default behavior, grant/revoke of a test permission, rejection of insecure/unsupported grants, popup blocking, and explicit user-approved tab opening passed in the disposable profile.

### Lucia website navigation and real-model actions: INCOMPLETE (3/5)
- PASS: `open example.com`; actual WebView2, observed destination, browser toolbar URL, and evidence-checked task completion matched.
- PASS: `open github.com`; same checks.
- FAIL: `open wikipedia.org`; actual Wikipedia page loaded but the navigation result was marked unverified. Recovery invoked local Qwen, which returned no parseable browser action, so the task ended failed. Page load alone must not be counted as task completion.
- PASS: `open news.ycombinator.com`; real Hacker News content, toolbar URL and evidence-checked completion matched.
- FAIL: from the visible `https://en.wikipedia.org/wiki/Main_Page`, ask Lucia to search for Ada Lovelace and open the article. It failed before any browser actions with a local Qwen invalid structured-action response.
- A standalone short synthetic prompt to the *same installed* Qwen/Ollama model returned a well-formed `type_text` action JSON. This narrows the problem to Lucia's full action prompt/response integration rather than model availability; it does not prove why the full prompt failed.
- These navigation-only successes are deterministic URL dispatch and evidence checks, **not** proof of autonomous model-driven multi-step browsing.

## Actionable follow-up
1. Diagnose the local Qwen full-prompt malformed-action failure and add bounded, fail-closed schema repair where appropriate; never guess or execute unobserved DOM node IDs.
2. On delayed external navigation, retry a fresh native observation before requesting another model action; verify whether the recovered destination is genuinely loaded.
3. Repeat multi-step tests (search, click, inspect, recover) using a real configured *cloud* model and more public sites, with no account operations or sensitive forms. Mock cloud-compatible endpoint tests are not proof of a paid/hosted model working.
4. Keep the manual tests out of required CI until the genuine-model failures are understood; simulated-model CI remains a separate gate.

**Current verdict:** core browser cross-site navigation works in this sample, but general-site multi-step Lucia autonomy is not yet demonstrated.
