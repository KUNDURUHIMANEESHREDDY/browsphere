# Browsphere real-site repeatability work — 23 November 2026

## Scope
- Local Windows native Browsphere / WebView2 release build.
- Actual authorized 9Router `oc/hy4-flash-free` cloud-only endpoint via the application's existing Windows credentials; no API key values in scripts or logs.
- Public websites only. No account access, form submissions with consequences, payments, or downloads.
- Distinguish genuine hosted-model inference from browser-only navigation and simulated-model CI.

## Real-hosted-model checks
1. **Wiktionary**: real WebView2 tab navigated to `en.wiktionary.org` but the site presented a **Wikimedia Error** document. The harness declined to start model inference. Classified `site_unavailable`, not successful autonomy.
2. **Python Docs**: loaded `https://docs.python.org/3/` with title *Python 3.15.2 documentation*, 78 interactive controls and 2,400 captured text characters. The native Lucia run obtained a real run ID but 9Router `oc/hy4-flash-free` returned HTTP 429 before proposing any browser action. Final task status `failed`, zero meaningful actions and no independent successful verification. The remaining public-site inference scenarios were not attempted, respecting the provider's quota.
3. These results **do not** prove repeated hosted-model multi-step autonomy. Do not infer model failure from a website error or claim hosted success from mock-model checks.

## Live real-WebView2 browser controls (no model)
Two runs against actual Python Docs, MDN, Wiktionary, GitHub Docs and Hacker News:
- Python Docs: opened index via visible first-party link — PASS.
- MDN: opened Blog — PASS.
- GitHub Docs: opened Get started — PASS.
- Hacker News: opened newest listing — PASS.
- Wiktionary: public site error / no usable dictionary link — **site unavailable**, not a browser-agent failure.
- First baseline: 4/5 navigation success, 1 site error.
- After reporting fix: 4/5 navigation success with exactly **1 `site_unavailable`**. The suite retains a nonzero exit when any expected public site cannot be verified.

## Implemented test reliability improvements
- `tests/e2e/lucia-real-site-evidence.cjs`: fail-closed, pure evidence checks for actual website readiness, exact target host/path, genuine model-route actions with successful outcome, synchronized tab URL, independently verified final state, and error-page rejection.
- The HY4 live-site runner supports `-Repeats 1..5` (per-site attempt IDs, separate tab/task for each attempt) and explicit `-Model oc/... ` selection. **It does not automatically switch providers/models** after quota failure. Once HTTP 429 is observed, the remaining attempts are reported as skipped, not successes or failures.
- The real-browser-only control harness labels website-generated error pages `site_unavailable` rather than reporting an unexplained missing clickable link.

## Validation
- `node --test tests/*.test.cjs`: **24 passed, 0 failed**, including four new dedicated evidence/repetition tests. Some preexisting unrelated OpenRouter tests are present in the local working tree; this work does not stage them.
- `node --check` for changed browser scripts and PowerShell parser checks passed.
- `git diff --check` passed.
- The Rust production code is unchanged by this focused real-site test instrumentation; earlier native CI checks remain separate.

## Release gate
**Open** — at least three successful, independently evidence-checked 9Router OC model runs per site across multiple external websites are still required for a meaningful repeatability claim. Do not bypass the 9Router free-tier rate limit, substitute a simulated model, or mark unavailable sites as validated.
