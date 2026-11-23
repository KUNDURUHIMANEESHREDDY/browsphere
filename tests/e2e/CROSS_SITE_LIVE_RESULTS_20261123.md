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

## Follow-up verification — 23 November 2026 (post-hardening build)

This follow-up uses a freshly built `target/release/browsphere-core.exe` containing the navigation settle checks, bounded action-JSON retry, and narrower observed-title verifier. The test launcher supports `-UseSavedCloud`, which loads only the saved cloud endpoint/model from the user's SQLite settings using `read-saved-cloud-settings.py`; the helper opens the database read-only. Keys remain in Windows Credential Manager, not test output or the disposable WebView2 profile.

- **Unit regressions:** `cargo test --workspace --locked` passed: 118 passed, 0 failed, 5 intentionally ignored. `node --test tests/*.test.cjs` passed: 16/16.
- **Actual external WebView2 page compatibility:** 17/17 passed on Example Domain, GitHub, Wikipedia, and Hacker News, including observed content, toolbar sync, permission controls and popup consent.
- **Live agent navigation:** 4/4 passed with genuine native snapshots and independently evidence-checked destination URLs. These short tasks dispatch URLs directly from explicit user requests; they do **not** demonstrate cloud-model reasoning.
- **Hosted-model multi-step search:** **FAILED**, not certified. The saved Groq route (`openai/gpt-oss-20b`) produced an actual browser `click`, then returned HTTP 429 `rate_limit_exceeded` on a later action request. Wikipedia's Ada Lovelace article was not opened; the run was correctly reported failed. Do not count the cloud model as end-to-end verified.
- **Native cloud-compatible simulation:** `run-lucia-multistep.ps1 -CloudCompat` **PASSED** against the rebuilt WebView2 app and a local simulated OpenAI-compatible model endpoint. Tests covered multistep search/click, SPA URL sync, large navigation menus, Enter search, delayed content, dropdowns, checkbox/radio/switch, rich editor, dialogs, fallback, assigned-tab switching/closure, and fail-closed cloud error responses. Simulated decisions are **not** real cloud provider verification.

Reproduce the saved-provider test on the authorized Windows computer after resolving provider availability/quotas:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-lucia-cross-site-live.ps1 -UseSavedCloud
```

For only the long cloud-model Wikipedia task, add `-OnlyWikipedia`. Do not change the result to passing without observed native action traces and a verified destination. **The project is still not certified for unrestricted website autonomy or production.**


## Follow-up implementation and testing — 23 November 2026

Changes (tested in the current native debug build):
- Compact local browser-action prompts preserve explicitly requested goal, current origin and exact observed node IDs, with bounded page excerpt; reject unparseable action JSON after at most one schema-specific retry.
- OpenAI-compatible cloud action responses also get at most one schema-retry on the **same** endpoint; HTTP errors such as 401, 429 and 503 are not silently changed to local Ollama.
- Model actions that fail fresh-DOM binding can be re-proposed once on the same observed page, then must still pass normal DOM validation and action permissions.
- Delayed navigation polls freshly captured native evidence for the previously dispatched URL. No blind URL replay or claimed success on acknowledgment.
- Bare-host -> www canonical navigation is accepted only on exact scheme/port/path/query and a valid loaded page; unrelated redirects remain rejected.
- Narrow "search for a named result, open it, report its title" tasks have a grounded page-title handoff requiring an actual successful observed click, changed article URL, matching title/path and saved WebView source evidence. This is not general webpage fact verification.
- Manual live-site test harness now supports an isolated Wikipedia-only run and saved non-secret cloud endpoint/model selection, with API secrets retained by Windows Credential Manager. The saved-settings helper outputs **only** endpoint/model, never API keys.

Verification:
- Rust: **119 passed, 0 failed, 5 ignored** (124 tests total); debug native build passed.
- JavaScript regression: **16 passed, 0 failed**; 9Router preset/selection contract tests passed.
- Real four-site WebView2 browsing: **17/17 compatibility checks passed** after waiting for current renderer page URL and address-bar convergence.
- Real local Qwen 2.5 3B navigation of four distinct public sites: **4/4 simple direct navigation tasks passed** in one run.
- Real local Qwen Wikipedia search -> Ada Lovelace: **not reliable**. One earlier run actually reached `/wiki/Ada_Lovelace` but halted before verification; later repeated runs halted at denied type_text or repeated model actions. Do not mark full multi-step autonomy as passed.
- Simulated OpenAI-compatible cloud endpoint in the real native WebView2 app: **PASS** across search, result click, SPA transitions, delayed controls, dropdown/checkbox/radio, failure recovery, assigned-tab lifecycle, and strict evidence review. The cloud-only traces showed planner/browser/verifier requests on the mock OpenAI API with **no Ollama fallback**. This is **not** real hosted inference.
- **Actual saved Groq-hosted `openai/gpt-oss-20b`: partial only.** The native model agent made a real observed click, then next inference returned **HTTP 429 `rate_limit_exceeded`**; this run ended `failed` as required. No quota-bypassing retries were attempted.
- **9Router:** local gateway `127.0.0.1:20128` not available; unauthenticated cloud request to `https://9router.com/v1/chat/completions` returned HTTP 404. No 9Router authenticated model call or end-to-end task was verified. Provider preset/model selection tests passed independently.

Release gate remains open: prove repeatable real-model, multi-step completion and final independent verification on multiple sites and at least one authenticated hosted provider within available quotas. Existing approval and sensitive-input gates must remain active.
