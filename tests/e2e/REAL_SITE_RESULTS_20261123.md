# Browsphere real public website tests — 2026-11-23

## Scope and reproducibility

Tests ran against the actual rebuilt Windows `target/release/browsphere-core.exe` on the authorized laptop with native WebView2/CDP, in a disposable APPDATA, LOCALAPPDATA, and WebView2 profile. No personal browsing account or private data was used. Two suites are intentionally distinct: **Lucia + a genuine 9Router hosted-model route**, and **direct browser-control compatibility tests without AI**. Do not combine these into one autonomy pass rate.

## 9Router `oc/hy4-flash-free`: real model-driven tasks

The agent runner `tests/e2e/lucia-real-sites-9router.cjs` uses `http://localhost:20128/v1/chat/completions` in cloud-only mode and the exact model ID `oc/hy4-flash-free`. Credentials are read through Browsphere's normal endpoint-scoped Windows Credential Manager integration; test scripts contain no keys. The runner asserts model-derived actions, destination URL and final native evidence verification. Approval-requiring operations are cancelled, not bypassed.

| Site | Starting page | Observed result | Model-driven task |
| --- | --- | --- | --- |
| Wiktionary | `https://en.wiktionary.org/wiki/Wiktionary:Main_Page` | Real page title and 120 interactive controls observed. Test ended `cancelled` without a recorded model action; no verified result page | **NOT PASSED** |
| Python Docs | `https://docs.python.org/3/` | Real Python 3.15.2 documentation loaded with 78 interactive controls. Provider returned `HTTP 429 Too Many Requests` from the configured local 9Router gateway, before any model browser action | **FAILED — provider rate limit** |
| MDN, GitHub Docs | Public sites | Model-driven tasks not attempted after the provider rate-limit response | **NOT TESTED** |

The earlier Wikipedia/Ada Lovelace multi-step 9Router test succeeded in the preceding verification session; it is not counted as a fresh success for the above new-site tasks. The Wiktionary cancellation does not establish whether that site's DOM is unsupported; no valid inference/action trace was observed.

**Conclusion:** This run did *not* demonstrate successful general-purpose model-driven autonomy on new sites. HTTP 429 is an external provider limitation; tests must not silently switch to Ollama, forge actions, retry through alternate accounts, or mark an unverified task complete.

Reproduce a single model-driven public-site task only when quota permits:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-lucia-real-sites-9router.ps1 -Site "Python documentation" -CdpPort 9554
```

Supported case filters: `Python documentation`, `MDN Web Docs`, `Wiktionary`, and `GitHub Docs`. The runner deliberately avoids purchases, accounts, downloads and consequential submissions.

## Native browser-control checks on real public websites — no model used

The separate `tests/e2e/real-public-site-controls.cjs` suite navigated to each actual site, selected a visible first-party non-account link, clicked it in native WebView2, and verified the resulting URL, title, and populated rendered body. This validates browser behavior and link handling, not autonomous planning.

| Live site | Clicked link | Verified destination | Result |
| --- | --- | --- | --- |
| Python Docs | `index` | `https://docs.python.org/3/genindex.html`, title `Index — Python 3.15.2 documentation` | **PASS** |
| MDN Web Docs | `Blog` | `https://developer.mozilla.org/en-US/blog/`, title `MDN Blog` | **PASS** |
| Wiktionary | `free` dictionary link | `https://en.wiktionary.org/wiki/free`, title `free - Wiktionary, the free dictionary` | **PASS** |
| GitHub Docs | `Get started` | `https://docs.github.com/en/get-started`, title `Get started with GitHub documentation - GitHub Docs` | **PASS** |
| Hacker News | `new` | `https://news.ycombinator.com/newest`, title `New Links \| Hacker News` | **PASS** |

**Browser-control result: 5/5 passed in the first run, 4/5 in an immediate repeat, then 5/5 in each of two runs after refining the smoke-test link selection.** On the first repeat, clicking Wiktionary's `Search` link left the renderer on its main page despite the original run navigating successfully. This flaky observation is preserved here: it is not proof of a reliable Search UI. The refined test clicks an ordinary dictionary word (`free`) instead, and Hacker News uses the distinct `new` listing rather than a near-no-op homepage link. These remain browser-only link/navigation smoke tests, not autonomous-model success.

Reproduce the browser-only checks:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-real-public-site-controls.ps1 -CdpPort 9563
```

## Next validation gate

Once the selected 9Router model is usable again, test multiple genuinely distinct sites with **observed model-selected interactions** and independently checked final browser state. Repeat each scenario for reliability. Do not certify unrestricted autonomy from deterministic URL opening, browser-only click tests, or the earlier single-site Wikipedia result.

## Reliability patch and retest — 23 November 2026

- Router: A hosted-model HTTP 429 is no longer automatically retried when `Retry-After` is absent, zero, or greater than three seconds. A short explicit 1–3-second retry window can still be honored; transient 408/502/503/504 and connection failures retain bounded retries. This does **not** bypass provider limits or silently use another model.
- Real-site native CDP harness: task launches now return to the CDP event loop immediately while an asynchronous native start settles. A late start result is observed without submitting a second task. CDP commands have cleared timers and reject immediately on WebSocket closure; an unresponsive CDP session ends the run rather than contaminating later site results.
- Cross-site reporting: on HTTP 429 or fatal CDP loss, remaining sites are classified as **not attempted**, not failed. The suite retains a nonzero exit code and prints a stop reason and attempted/skipped counts.
- Backend regression: `cargo test --workspace --locked --quiet` **123 passed, 5 ignored, 0 failed**; new loopback regression asserts that a quota 429 with absent or long `Retry-After` is not replayed.
- JavaScript regression: `node --test tests/lucia-*.test.cjs tests/browser-state.test.cjs` **16/16 passed** and the new live-site harness passed syntax validation.
- Windows debug build: `cargo build --locked --quiet` **passed**.
- Real public-site retest: Python Docs loaded in actual Windows WebView2 (78 native controls). The `oc/hy4-flash-free` task launched and returned a genuine run ID, but the 9Router gateway returned **429 before any model action**. The run correctly reported `failed`, 0 verified actions, and `provider_rate_limited`; **no real-model autonomy success** may be claimed from it.
- Separate native cloud-compatible simulated-model regression exercised multi-step search, SPA transitions, forms, recovery, assigned-tab behavior and independent evidence checks, and **passed**. It does not count as real 9Router inference.

**Remaining gate:** When 9Router's quota actually permits a series of model calls, perform repeated complete agent-driven search/inspect/verify workflows on more than one external site. Record observed action traces and destination URLs. Until then unrestricted website autonomy remains unverified. Run tests in isolated profiles and preserve the approval checks for consequential actions.

## Follow-up: provider diagnostics and live-site retest (23 November 2026)

- Updated `src-tauri/src/router/model_router.rs` to return clear, sanitized errors for HTTP 429, 401/403 and temporary upstream failures from both planning/connection requests and browser-action inference. A numeric `Retry-After` in the 1–3600-second range may be reported; raw provider responses, page content and secrets are not displayed. This does not bypass 9Router quotas, change providers, or weaken action approval policy.
- Added regression coverage for rate-limit error descriptions, authentication errors, temporary outages and malformed retry-delay headers.
- `cargo test --workspace --locked --quiet`: **126 passed, 0 failed, 5 ignored** in the final rerun on the current local working tree (which also contains separate, uncommitted agent work).
- `node --test tests/lucia-*.test.cjs tests/browser-state.test.cjs`: **16 passed, 0 failed**; Windows debug native build passed.
- Genuine 9Router `oc/hy4-flash-free` + native WebView2 Wiktionary run: the real page loaded with 120 interactive controls; the run started and returned a UUID, but **the provider responded HTTP 429 before producing any action**. Final status correctly reported `failed` with zero model-driven actions and no verified destination.
- Browser-only public website navigation: **5/5** observed and verified on Python Docs, MDN Web Docs, Wiktionary, GitHub Docs, and Hacker News. This proves native page controls, not model reasoning.
- Native cloud-compatible **simulated-model** multi-step fixture: **passed** search, typing, clicks, delayed SPA state, widgets, recovery, tab switching/closure and evidence verification. Not a real hosted-model success.

**Production gate remains open:** no repeatable real-model tasks on multiple external websites were demonstrated in this run. Do not count browser-only or simulated-model passes as proof of autonomous model intelligence. Reattempt real-model tasks only when the authorized 9Router provider permits requests.

## Narrow read-only search reliability improvements — 23 November 2026

- For explicitly recognized, single-subject **search for X, open the article, report its title** requests, Lucia now uses a fixed Browser → Verification specialist plan. It does **not** pick or synthesize an article URL, a search query beyond the user's request, or a DOM control; actual browser actions still require the selected inference model, native DOM binding, and the independent evidence verifier. Tasks involving payments, accounts, submissions, downloads, comparisons, extraction, multiple requested results, or similar side effects still use the normal model-generated planner.
- Search fields on some sites navigate directly to the named article when Enter is pressed, without showing a separate search result to click. A model-selected Enter on a freshly observed, non-sensitive search input can now be recognized as **success only after** a fresh WebView2 observation proves the requested article's matching URL, title, readable body, and recorded source evidence. A delayed transition can be confirmed on a subsequent DOM capture, without re-submitting or inventing a link. Sensitive inputs, unrelated buttons, Escape, wrong pages, and unsupported goals cannot use the exception.
- The new disposable native fixture at `tests/e2e/fixture/named-enter.html` and `/nebula.html` exercises real browser typing, an observed Enter form navigation and a title/evidence check, with a local simulated cloud-compatible model. The full native WebView2 suite **passed** and its mock provider counted **zero planning requests** for this narrow workflow while counting the actual model-selected browser actions. This is **not** real 9Router inference.
- **Controlled live 9Router retest on Wiktionary:** the actual public main page loaded, the new Browser specialist started, and the first model-action request to `oc/hy4-flash-free` returned **HTTP 429**. Task ended `failed`, **zero browser-action traces**, no independent verified result. The clear diagnostic identifies rate limiting and explicitly refuses to change providers. No provider quota bypass, new credentials or rapid extra inference attempts were used.
- Therefore the live model's general multi-step autonomy on Wiktionary, Python Docs, MDN, and GitHub Docs is **not proven**. The prior Wikipedia real-model success remains a narrow single-site achievement and must not be presented as universal autonomy.

### Final validation of this update

- Rust `cargo test --workspace --locked --quiet`: **126 passed, 0 failed, 5 ignored** (131 total).
- JavaScript `node --test tests/*.test.cjs`: **18 passed, 0 failed**.
- Windows `cargo build --release --locked`: **successful**.
- Final native `run-lucia-multistep.ps1 -CloudCompat`: **passed** with a real WebView2 executable, a local simulated cloud-model endpoint, and an isolated browser profile. In particular the new named-title lookup navigated via model-selected `type_text` and `press_key Enter` to `/nebula.html`, then passed independent URL/title/source checking with no cloud planner request. Existing SPA, forms, failure recovery, tab switching/closing and 401/429/503 fail-closed checks also passed.
- Genuine 9Router `oc/hy4-flash-free` on Wiktionary: **failed with HTTP 429 before the first browser action**. This cannot be described as successful live hosted-model autonomy. Do not automatically change providers or infer success from the mock tests.
