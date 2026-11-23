# Lucia agent production gate

Updated: 22 November 2026

## Scope implemented in this pass

- Guard click/type actions using a fresh WebView DOM snapshot. Model-supplied click labels and safety flags cannot downgrade observed controls.
- Deny stale, unknown, or unlabeled clickable controls; deny text input into non-text controls.
- Require approval for live-page submit controls, sensitive fields, and high-consequence buttons.
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
