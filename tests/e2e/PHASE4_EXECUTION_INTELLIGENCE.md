# Lucia Phase 4 — observation-driven execution

Revision: lucia-execution-intelligence-v8.

The native execution_intelligence module compiles a bounded public link-and-title objective into acquire-control, await-destination and verification phases. The run-local world model retains bounded observation digests, public-control counts, evidence-source coverage, completed stage receipts and failed strategies. It is not cross-session browsing history, and does not save private field values or raw page text. Native policy, action binding, approval, and independent report gates still apply.

For a named public link, the unique observed link supplies both its native control and exact URL. A dispatch acknowledgement only creates a pending receipt. Completion requires a later readable native snapshot matching the observed destination, the expected document title and recorded evidence. The verifier need not guess that a link label appears in the URL slug. Bounded observation handles delayed or missing controls. A failed permitted click can use a distinct freshly observed same-origin navigation strategy; policy restrictions do not authorize a fallback. Failed observation of a dispatched navigation does not authorize replay. The stage graph retains completed receipts and appends final verification as before.

For supported explicit source, compiled target and selected-filter workflows, native completion handoffs take precedence over further model action/review calls. The explicit read-current-reference contract permits observation-only research: the original page must be readable, its title must match the requested subject, evidence must be recorded, and the factual reviewer must independently validate the requested answer. This path does not create a synthetic successful click. Unknown or unsupported goals still use the existing orchestrator; unrestricted natural-language skill composition and arbitrary factual reasoning are not established by these bounded contracts.

Run regression tests from the repository root:

    cargo test -p browsphere-core --offline -- --test-threads=2
    python tests/e2e/run-lucia-phase3.py --self-test
    cargo build -p browsphere-core --offline
    python tests/e2e/run-lucia-phase3.py --case nav-rust --case delayed-link --case missing-link --case mcp-negative --case heldout-mdn

The missing-link negative test accepts a bounded unverified stop only when the original page remains observed, no false verification exists, no decoy or invented destination is requested, and no write or approval is attempted. Timeout is not a pass. The evaluator distinguishes expected downstream graph blocking from a blocked starting website. Positive benchmark criteria remain unchanged. Runtime identity and executable hash checks remain mandatory.
