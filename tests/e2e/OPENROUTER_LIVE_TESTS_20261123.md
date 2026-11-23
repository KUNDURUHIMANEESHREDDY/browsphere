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
