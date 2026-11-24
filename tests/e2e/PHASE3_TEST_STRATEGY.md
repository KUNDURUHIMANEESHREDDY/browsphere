# Lucia Phase 3 ? independent evaluation

Native revision: lucia-role-orchestration-v7. No production user data, cloud credentials, approval grants, bookings or purchases are used. Tests run sequentially in disposable WebView2 profiles. The runner hashes the native binary before and after each case and requires the revision in each agent run. Logs and incremental JSON reports are stored under ignored tests/e2e/artifacts/phase3.

## Run

From repository root on Windows, with the four configured local Ollama models installed:

    cargo build -p browsphere-core --offline
    python tests/e2e/run-lucia-phase3.py --self-test
    python tests/e2e/run-lucia-phase3.py --list
    python tests/e2e/run-lucia-phase3.py --tier contracts
    python tests/e2e/run-lucia-phase3.py --tier skill --repeat 2
    python tests/e2e/run-lucia-phase3.py --tier composition
    python tests/e2e/run-lucia-phase3.py --tier failure
    python tests/e2e/run-lucia-phase3.py --tier autonomy --train-date 2026-11-24
    python tests/e2e/run-lucia-phase3.py --tier heldout

Omit tier/case to execute all 29 registered cases; --case selects an individual scenario. --repeat 1?5 runs each case in fresh profiles. Choose a valid future journey date when needed. The original eight benchmark predicates are retained except the train date parser now uses the selected date. The train test is read-only and does not book, enter passenger data or bypass authentication. Native controls and synthetic inference/DOM fixtures are reported separately from autonomous agent results.

| Tier | Evidence |
|---|---|
| Contracts (4) | Rust suite, JS suite, actual Edge DOM fixture, local inference matrix. |
| Skills (8) | Two public search sites, two documentation navigation sites, two factual extraction sites, independent native public-control smoke checks, local catalog filter fixture. |
| Composition (3) | Search + destination + cited answer on two public sites, two-source technical comparison. |
| Failure (4) | Delayed link, missing link, failed MCP dependency (must stop), pre-dispatch HTTP outage (must recover). |
| Autonomy (8) | Original complex benchmarks, separate pass/fail from all other tiers. |
| Held-out (2) | Additional public research objectives not used in the original eight-task benchmark. These are held out from the existing benchmark, not proof of never-seen training data. |

## Acceptance

A successful process exit alone is insufficient. Live agent cases require the native revision, the structured case result and all independent predicates. A safe-stop scenario passes only for the expected refusal/cancellation with no false success or unexpected dispatch; it is not task-completion credit. Blocked websites are classified separately and not counted as successes. The runner records exact failed checks, terminal status, model routes, evidence and raw logs. Timeouts fail. Reports are incomplete until all selected cases finish; a changed binary aborts the run. Explicit model-role probes and native fixture checks do not prove optimal role assignments or broad website autonomy.

The current coverage is intentionally bounded: public autocomplete and calendar handling are exercised by the IRCTC benchmark plus controlled DOM fixtures; not yet independently established across multiple real sites. Native public-control navigation is not autonomous navigation. Extraction tests use explicit factual predicates and exact observed citations; they do not certify arbitrary product matching or semantic correctness. Extend held-out scenarios and use repeated runs before any production reliability claim.
