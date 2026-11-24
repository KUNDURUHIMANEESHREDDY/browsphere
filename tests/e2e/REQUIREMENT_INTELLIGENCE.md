# Requirement-level intelligence (v12)

The run-local ledger compiles explicit source objectives and supported factual facets for bounded public research. Each requirement is missing, observed (source/target), candidate (potential support), or verified (independently reviewed, exact scoped evidence). Missing requirements block completion. Candidate counts are not semantic certainty. Final report validation requires distinct quotations from the correct source or anchored repository and checks their presence in observed evidence. A repository name is not enough: the target identity must be observed on the named GitHub host. Multiple source pages can supply separate facts. A bounded same-repository acquisition step selects visible README, installation or metadata links only; normal policy and action binding remain in force. Cross-origin linked documentation acquisition is not implemented by this contract.

Requirements are refreshed from the currently retained evidence, so stale or evicted support does not remain falsely verified. The compact action prompt carries missing requirement categories and the agent:requirement_state event exposes only IDs, categories, states and candidate counts. No raw excerpts or source URLs are persisted by the ledger. Unsupported task contracts retain existing verification behavior. This is not yet arbitrary natural-language decomposition or complete semantic entailment checking.

Regression and live tests:

    cargo test -p browsphere-core --offline -- --test-threads=2
    node --test tests/lucia-local-model.test.cjs tests/lucia-local-profiles.test.cjs tests/lucia-activity.test.cjs
    python tests/e2e/run-lucia-phase3.py --self-test
    cargo build -p browsphere-core --offline
    python tests/e2e/run-lucia-phase3.py --case level-2 --case level-3 --case level-8

The original eight benchmark success checks are unchanged. The runner includes per-step requirement telemetry in the task result.
