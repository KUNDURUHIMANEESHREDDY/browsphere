# Lucia agent production gate

Updated: 22 November 2026

## Scope implemented in this pass

- Guard click/type actions using a fresh WebView DOM snapshot. Model-supplied click labels and safety flags cannot downgrade observed controls.
- Deny stale, unknown, or unlabeled clickable controls; deny text input into non-text controls.
- Allow routine low-risk form submissions without interrupting the task, while requiring confirmation for consequential submissions, sensitive form content, and high-consequence controls.
- Stop on denied, timed-out, or cancelled approvals; approval waits are bounded to 120 seconds and cancelled runs cannot execute a late authorization.
- Deny downloads using non-HTTP(S) URLs or URLs with embedded credentials; likewise disallow credentialed navigation URLs.
- Redact typed data, raw model rationale, approval descriptions, sensitive URL paths/parameters, and completion summaries from live agent traces and stored SQLite audit records.
- Do not persist workflow macro action sequences (including URLs and typed values). Macro replay remains disabled until independently verified.

## Verification evidence

- Rust tests include dedicated DOM-binding, approval-stop, URL-policy, and trace-redaction regressions.
- Live local Qwen/Ollama routing, research-action proposal and grounded verifier tests were individually run successfully on 22 November 2026.
- **Real Windows WebView2 test passed:** `tests/e2e/lucia-native-gate.cjs` opened a local fixture, captured live DOM directly from the external WebView2 and correctly recognized click and submit controls.
- **Real local-model workflow passed:** `tests/e2e/lucia-live-agent.cjs` used Ollama/Qwen to plan a browser task, click an observed control, re-observe newly revealed text, advance through independently source-checked verification, and report `completed`.
- **Consent safety passed:** `tests/e2e/lucia-approval-rejection.cjs` confirmed rejection prevents submission, while `tests/e2e/lucia-approval-cancellation.cjs` confirmed cancelled pending consent clears approval and prevents submission.
- **Current backend regression:** 59 passed, 0 failed, 4 local-model-dependent tests ignored in the default test suite. Local-model integration tests and browser state/JS syntax checks were separately exercised.
- **Windows installer packaged:** `cargo tauri build --bundles nsis` succeeded and produced `target/release/bundle/nsis/Browsphere_0.1.0_x64-setup.exe` (~4.73 MiB). Installer installation, upgrade, uninstall, signing, and clean-machine smoke tests have NOT been verified.
- Live tests use a harmless local HTTP fixture and an isolated test data profile. They are not a broad real-internet reliability sample.
- See `lucia-evidence-live.log`, `lucia-native-approval-rejection.log`, `lucia-native-approval-cancellation.log`, and `lucia-evidence-transition-tests.log` for machine-local test logs.

## Still REQUIRED before a production release

1. Repeat the successful native WebView2 end-to-end workflow across multiple external sites and dynamic SPAs, redirects, paginated tasks, and real packaged-installer launches; test actual app-chat handoff as well as native command invocation.
2. Extend the now-passing rejected and cancelled form-approval tests to approved submissions, redirect/navigation failures, dynamic SPA DOM controls, network timeouts, retries, and site-specific confirmation patterns.
3. Validate approval UI expiry and window-dismissal cleanup (cancellation cleanup passed), and verify no stale approval cards survive a cancelled task.
4. Check cloud and offline model routing, rate limits, token budgets, retries, and error messages in an installed release.
5. Audit historical local agent traces created by previous versions; the new masking applies to NEW traces, not an automatic migration of older records.
6. Run Windows release/installer smoke tests and GitHub Actions, review security and privacy threat models, and sign/distribute only after all gates pass.

**Release status: NOT PRODUCTION-CERTIFIED.** This pass hardens critical safety/privacy paths; it does not certify unrestricted real-world autonomy.


## Autonomous browsing hardening — 23 November 2026 (code implemented; live rerun pending)

The latest `main` includes these changes:

- A named-site search such as **"Search YouTube for NASA Artemis"** can bootstrap its requested public website from an empty native tab. This initial navigation is derived from the user's explicit request; models still decide subsequent search and result actions from observed webpage controls.
- A dynamic webpage receives unique interactive-node identifiers, reducing collisions after controls appear or disappear.
- If a small model supplies an invalid/stale node ID, Lucia may rebind only to an **unambiguous, freshly observed** matching control. Search-text rebinding is limited to text explicitly present in the user's request. Unmatched or ambiguous controls still fail closed; observed sensitive flags and approval policy still apply.
- Native type and click actions refuse detached, disabled, hidden, read-only, or noneditable controls rather than reporting false success.
- Final review retries **once** when the model emits malformed evidence/quotes, not when evidence is absent, unobserved, or explicitly insufficient. Every final quote and URL remains independently checked.
- A requested YouTube video-result task cannot claim completion from YouTube's search/results page: it requires a freshly observed video-page URL.
- Agent traces trigger a native-snapshot reconciliation of the **originating tab's** visible URL, covering SPA history transitions that may not fire a regular WebView2 page-load callback.
- Windows CI now syntax-checks the manual Lucia native E2E regression scripts.

**Verification boundary:** Earlier live tests confirmed direct `open youtube` navigation and one successful Qwen failure-recovery run. A separate Qwen run reproduced intermittent `unverified` final status, and a real multi-step YouTube search run halted after denied actions. Those results happened **before** the hardening changes above. In this pass the edited JavaScript files parsed successfully and Rust embedded-JS format delimiters were statically inspected, but fresh Windows compilation, actual YouTube search/click E2E, repeated verifier tests, and installed release smoke testing were **not** run after these commits because the authorized desktop connection reached its monthly usage limit.

**Release status remains NOT PRODUCTION-CERTIFIED.** Do not mark GitHub issues #1 (multi-step browsing) or #2 (intermittent verifier) resolved until independent live native regression tests pass.

Recommended validation on the authorized Windows development machine with an isolated local WebView2/CDP profile and configured authorized model:

1. `cargo test --workspace --locked` (includes new DOM/action, authorization, verifier, and video-result postcondition regressions).
2. `node --check tests/e2e/lucia-youtube-search.cjs` and `node --check tests/e2e/lucia-failure-recovery.cjs`.
3. Launch the isolated native app + Ollama and run `tests/e2e/lucia-start-page-navigation.cjs`, `tests/e2e/lucia-youtube-search.cjs`, and `tests/e2e/lucia-failure-recovery.cjs` with their documented local ports.
4. Repeat against external dynamic SPAs and redirects and confirm that the address bar always matches the observed native URL.
5. Verify the Windows CI installer build, permissions, consent cancellation, and privacy audit before distribution.
