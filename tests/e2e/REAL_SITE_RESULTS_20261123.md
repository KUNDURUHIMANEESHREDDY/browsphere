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
