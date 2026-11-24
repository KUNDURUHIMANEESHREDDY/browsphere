# Phase 7 - bounded autonomous evidence acquisition (v13)

Runtime revision: lucia-evidence-acquisition-v13.

Baseline v12 live tests: three-library comparison 5/6 original factual checks; GitHub research 1/3; PostgreSQL SQL INSERT reference search halted after looping back to the index. All three failed the original task-level checks. The aiohttp default timeout was a real source passage, but did not satisfy the original session-management-focused benchmark predicate.

The native DOM observer prioritizes observed same-origin documentation/metadata links and visible embedded articles (including short rendered code blocks) before large surrounding file listings and navigation. Hidden blocks are excluded. The existing text and node budgets remain bounded. No raw remote file download or hidden metadata retrieval is introduced.

The acquisition planner ranks observed repository links by the missing installation and runtime requirements, deduplicates attempted destinations and caps acquisition attempts at six. Every link still passes native policy and objective checks. A missing requirement with no suitable unvisited link stops unverified. The documentation reference skill recognizes numbered current-version and SQL hierarchy labels, rejects cross-origin waypoints and avoids returning to ancestor indexes or previously attempted targets. Evidence ranking prefers direct operational warnings over incidental default settings when the goal requests usage caveats and qualifying warnings are present. These are heuristic contracts, not arbitrary semantic research or guarantees of finding all relevant facts.

The native requirement ledger and distinct exact quote verification remain mandatory. Existing benchmark predicates are unchanged. New telemetry includes acquisition attempt counts and missing categories, not raw page passages.

Tests: cargo test -p browsphere-core --offline -- --test-threads=2; python tests/e2e/test_dom_interaction_patterns.py; node --test tests/lucia-local-model.test.cjs tests/lucia-local-profiles.test.cjs tests/lucia-activity.test.cjs. Live eight-case command: python tests/e2e/run-lucia-phase3.py --case level-2 --case level-3 --case level-8 --case level-1 --case level-4 --case level-5 --case level-6 --case level-7 --train-date 2026-11-25.

Limitations: the planner only follows freshly observed same-repository documentation/metadata links and bounded recognized reference navigation. It does not yet maintain a multi-branch frontier with backtracking, automatically authorize cross-origin documentation, acquire arbitrary product attributes, or recover each rejected semantic finding. Do not treat unit or fixture results as proof of broad live-site autonomy.
