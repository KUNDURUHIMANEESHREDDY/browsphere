# Lucia Form Submission Proof — P0

## Problem

A WebView click dispatch or model-generated stage summary was being confused with a successful form submission. The previous general-purpose confirmation checker could also accept evidence from a different form on the same page. The Form stage must not advance when the user asked for submission until the intended form has been submitted and the result is visible.

## Implemented contract

1. Recognize an explicit user-named form, e.g. "submit the feedback form," and match a *fresh observed*, non-sensitive submit button. An unrelated submit button (e.g. "Submit sample application") is rejected **before dispatch**. Ambiguous generic submission controls are not guessed.
2. Keep standard action-policy/approval checks. The form selector cannot authorize actions on its own. When the Form Agent has already interacted with a form, a unique grounded requested submit button may be used as a bounded fallback instead of model-proposed unrelated actions.
3. After clicking the intended button, read a **new native DOM snapshot**. Submission success requires a change in the observed page, a newly appeared target-specific confirmation, and no pre-existing identical confirmation in the before snapshot.
4. Record a run-local `SubmissionReceipt` only from this post-action verification. Click dispatch success cannot create it.
5. Require this receipt and its still-visible confirmation in `Form` stage completion *and* `Verification` stage completion. Never submit the same form twice after a receipt.
6. Produce an independent final `evidence_checked` report only from the native captured page's source-exact confirmation. Do not claim downstream processing or acceptance beyond the observed UI message.

## Tests

- Rust: `cargo test --workspace --bins -- --test-threads=2` — **527 passed, 0 failed, 12 ignored** (26 Nov 2026), including a guard against interpreting an explicitly negated submit instruction as authorization.
- Live local fixture: `tests/e2e/lucia-autonomous-form.cjs` — **2 successful runs** through native app, with `agent:form_submission_proof`, final `completed`, visible feedback confirmation, and independent verification. The strict repeat additionally confirmed the unrelated application form remained unchanged.
- A separate retry was blocked at inference admission because free system RAM fell below the unchanged 2 GiB safety threshold. After temporarily unloading unused resident Ollama models, the strict repeat passed. No model installation or saved model configuration was changed.

## Limitations

This validates a harmless local feedback form. It does not prove arbitrary sites' required fields, CAPTCHA, authentication, external form processing, payments, or multi-page workflows. The named-form parser is intentionally conservative and may decline ambiguous forms rather than risking the wrong submission. The installed release and GitHub `main` are not changed as part of this P0 source update.
