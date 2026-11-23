# OpenRouter native real-website testing

This suite targets Browsphere's **real Windows WebView2** and a **real OpenRouter hosted model**, never a mock. It uses the same already-tested native Lucia action/verification logic as the 9Router live-site runner.

## Fixed test configuration

- Cloud endpoint: `https://openrouter.ai/api/v1/chat/completions`
- Explicit model: `inclusionai/ling-3.2-flash:free`
- Cloud-only: true; no Ollama fallback
- Public websites: Python documentation, MDN, Wiktionary, and GitHub Docs
- Actions allowed: public read-only search, typing, clicking, and navigation. Any approval request stops the run.
- Pass criterion: OpenRouter model action traces, at least two meaningful browser actions, independently observed requested final URL and native source verification; **URL opening alone is never counted as autonomous success**.
- The model is confirmed free and advertises structured outputs in OpenRouter's public catalog as of 23 November 2026. This can change, so the runner checks the current catalog before any live task.

## Setup on Windows

1. Open the Browsphere Windows app, **Settings > AI & ML**.
2. In **Cloud provider**, choose **OpenRouter**. The preset fills the endpoint and free model. Enable **Cloud** and **Lucia cloud-only reasoning**.
3. Sign in to an authorized OpenRouter account and create your own API key at the provider's API Keys page. In Browsphere, enter it in **Cloud Models > API key**, then **save the AI settings**. The existing app stores it in Windows Credential Manager scoped to `openrouter.ai:443`, not inside tests or Git. **Do not send API keys in chat or commit them to the project.**
4. Run **Test cloud connection** and **Test Lucia reasoning** in the app.
5. From the project root, inspect model availability without a key:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-lucia-real-sites-openrouter.ps1 -CheckCatalogOnly
   ```

   For OpenRouter's own free-model router instead of the pinned Ling model, add `-Model 'openrouter/free'`. The runner checks that the selected model costs $0 and supports structured outputs, and rejects paid-model IDs.

6. After saving the key, test one public site:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-lucia-real-sites-openrouter.ps1 -Site Wiktionary -CdpPort 9585
   ```

7. Test all four sites by omitting `-Site`; repeat successful cases for reliability. The native suite emits `REAL_SITE_RESULT` and `REAL_SITES_SUMMARY` with independently checked results.

The test runner uses a separate temporary app/browser profile, so it does not navigate or modify a person's normal tabs. A missing saved credential is detected before launching the browser. Provider 401/403 and 429 errors stop remaining scenarios; they are not treated as successful browser outcomes.

## Results on 23 November 2026

| Check | Observed |
| --- | --- |
| OpenRouter public API catalog reachable from user's Windows computer | **PASS — HTTP 200** |
| Selected model currently free and supports structured outputs | **PASS** |
| OpenRouter preset and native-test guard unit tests | **PASS — 2/2** |
| Complete JavaScript test suite | **PASS — 20/20** |
| PowerShell script syntax and JavaScript script syntax | **PASS** |
| Saved OpenRouter API key for this Windows user | **NOT FOUND** |
| Full authenticated model-driven multi-step flow across live sites | **NOT RUN — requires saved key** |

**No live OpenRouter inference success is claimed.** Public model listing and simulated cloud routing tests do not constitute authenticated model-driven website autonomy. After saving the key, repeat the four live cases and record final independent evidence, action traces, and failures.

References: https://openrouter.ai/docs/api/api-reference/chat/create-a-chat-completion and https://openrouter.ai/inclusionai/ling-3.2-flash%3Afree

### Build and native browser follow-up

- A **debug Windows build** (`cargo build --locked`) succeeded, incorporating the new OpenRouter free-model preset. The release rebuild failed because Windows was already running the release executable from Explorer, so Cargo could not overwrite the locked file; the active user session was deliberately left untouched. Do not claim the installed release executable contains this change yet.
- The isolated **native WebView2 browser-only smoke test** using the updated debug executable navigated and clicked real links on **Python Docs, MDN, GitHub Docs and Hacker News (4 passes)**. Wiktionary returned a real **Wikimedia Error** page, classified as **site_unavailable** with no click; overall smoke suite exited nonzero (1). This is an external-page-availability outcome, not a verified agent result.
- The real OpenRouter runner was launched with the selected free model and stopped at its **missing endpoint-scoped Windows Credential Manager key** preflight. There were **zero authenticated OpenRouter inference calls and zero completed real-model website tasks**. Browser-only smoke and free-model catalog checks cannot substitute for the missing live-model pass.

## NVIDIA Omni 30B free-model attempt — 23 November 2026

- **Exact ID:** `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free` on OpenRouter.
- **Catalog check:** HTTP 200; input and output pricing `$0`; `structured_outputs` **not advertised**. The runner now reports this capability gap as a warning rather than refusing an explicitly free model.
- **Native Windows test:** Isolated debug Browsphere app loaded `https://docs.python.org/3/` (78 browser controls) and started a Lucia Browser Agent task with this model.
- **Provider response:** HTTP **429** on the first inference; status `failed`, `provider_rate_limited`. There were **zero** model-generated actions, no destination change and no independent completion verification. Remaining sites were not attempted.
- **Interpretation:** OpenRouter rate limiting prevented testing the model's actual action JSON quality or browser autonomy; it is neither a success nor evidence that Omni 30B is incompatible. Do not retry repeatedly or bypass limits.

## Authenticated-key retest — 23 November 2026

An OpenRouter credential target `model_api_key:openrouter.ai:443` was **present** in the Windows Credential Manager during this retest; the key value was never read into logs or displayed. This supersedes the earlier "key missing" status, which accurately described the prior check.

### Genuine OpenRouter model and native WebView2

Command: `run-lucia-real-sites-openrouter.ps1 -Site 'Python documentation' -CdpPort 9595 -ExecutablePath target/debug/browsphere-core.exe` (isolated temporary profile; normal app unaffected).

- **Catalog:** free, structured-output-compatible `inclusionai/ling-3.2-flash:free` was found.
- **Public page:** `https://docs.python.org/3/` loaded correctly, title `Python 3.15.2 documentation`, 78 exposed native interactive controls.
- **Lucia:** Browser Agent task started with a fresh run ID.
- **Provider inference:** OpenRouter returned **HTTP 429** on the first browser-action request. Lucia reported `failed` and `provider_rate_limited` and stopped without changing the provider.
- **Action trace:** zero actual model-generated browser actions; no requested destination URL; no independent completed-task verification.
- **Verdict:** **FAIL / provider rate-limited**, not a real autonomous task pass. Other live-model site cases were not attempted after this failure. Do not claim that the key's presence proves usable quota, or that the provider 429 establishes whether the limit is model-specific versus account-wide.
- **Additional free-router attempt:** `-Model 'openrouter/free'` passed the public $0/structured-outputs catalog check and launched the same isolated Python Docs task, but also returned **HTTP 429**, with zero model actions and no verified result. This was stopped after the first response without retry loops. The runner now accepts explicitly catalog-verified `:free` models or `openrouter/free`, and rejects paid IDs.

### Separate independent regressions

- Rust: **128 passed / 0 failed / 5 ignored**.
- JavaScript: **28 passed / 0 failed**.
- Native Windows WebView2 `run-lucia-multistep.ps1 -CloudCompat` using a **local simulated cloud-compatible model**: **PASS**; source evidence and page navigation, Enter-to-article, non-YouTube search/click, deliberate failure recovery, tab switching/closure and independent result verification all passed. This does **not** use the OpenRouter hosted model.

**Release gate remains OPEN:** repeated model-driven multi-step browsing and independent verification on multiple *real websites* using real hosted OpenRouter inference are **not verified**. Further inference should respect the provider rate limit rather than rotate models or credentials to work around it.

## Strict 4-site OpenRouter repeatability gate — 23 November 2026

**Implementation:** The OpenRouter WebView2 runner now uses the shared real-site evidence evaluator, rejects error pages, requires independent verification, synchronized active tab and captured source URL, at least two *successful* actions from the exact hosted model, and terminal task success. Failed or denied actions and local-only actions cannot count as progress. `-Repeats 1..5` creates independent run IDs/cases and reports per-site pass counts, missing attempts and reasoned stoppages. No fallback, key rotation or quota-bypassing retries.

**Executed:**
```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-lucia-real-sites-openrouter.ps1 -Model 'inclusionai/ling-3.2-flash:free' -Repeats 3 -CdpPort 9612 -ExecutablePath target/debug/browsphere-core.exe
```

- Public catalog: HTTP 200; selected model lists $0 usage and structured output compatibility.
- Loaded real Python Docs page `https://docs.python.org/3/` in isolated native WebView2; title `Python 3.15.2 documentation`, 78 observed controls.
- Launched real Browser Agent for `Python documentation #1/3`.
- **OpenRouter HTTP 429** on the first requested browser action. No successful model action traces, no destination navigation or verified final result.
- **Summary:** 0 passes of 12 planned attempts, 1 attempted and 11 correctly reported skipped. No other websites contacted by the model. Do not count skipped runs as tested failures or successes.
- **Offline regression:** 130 Rust tests passed (5 ignored); 30 JavaScript tests passed; Windows debug build succeeded.

**Release gate stays open:** General cross-site model-driven autonomy has not been established. An unsigned Windows installer built by the CI pipeline is a development/test artifact only, not a production-ready signed release.
