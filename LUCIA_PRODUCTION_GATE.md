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
- Existing recorded desktop UI smoke tests are not equivalent to a complete live-agent end-to-end run.
- See lucia-prod-final-tests.log, lucia-prod-live-router.log, lucia-prod-live-specialist.log, and lucia-prod-live-verifier.log for captured test output.

## Still REQUIRED before a production release

1. Run a real packaged WebView2 multi-step task from chat through site navigation, DOM interactions, specialist transitions, confirmation and source-verified result; repeat with multiple real and local fixture sites.
2. Add full desktop integration coverage for approved and denied form submissions, redirect/navigation failures, dynamic SPA DOM controls, network timeouts, and user cancellation during an approval.
3. Validate UI approval timeout/cleanup, including dismissed windows; cancelled approvals must disappear from the pending list.
4. Check cloud and offline model routing, rate limits, token budgets, retries, and error messages in an installed release.
5. Audit historical local agent traces created by previous versions; the new masking applies to NEW traces, not an automatic migration of older records.
6. Run Windows release/installer smoke tests and GitHub Actions, review security and privacy threat models, and sign/distribute only after all gates pass.

**Release status: NOT PRODUCTION-CERTIFIED.** This pass hardens critical safety/privacy paths; it does not certify unrestricted real-world autonomy.
