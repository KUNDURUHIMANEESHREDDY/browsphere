# Native semantic page understanding (v10)

Runtime revision: lucia-semantic-perception-v10.

The native visible DOM observer records bounded structural counts (headings, prose paragraphs, code blocks, articles, result regions/links, visible priced identity cards, visible offer blocks and loading state). It does not read hidden product JSON or additional arbitrary input values. The Rust classifier combines these signals with observed controls and public excerpt context. It classifies blocked pages, dialogs, loading states, autocomplete, transport results, empty results, product listings, product details, search results, documentation, articles, search forms and unknown pages. It distinguishes loaded result/content transitions from loading or cosmetic control removal. Classifications are advisory, not permissions, exact product matching or factual completion.

The train result contract requires a visible result heading, numbered schedule records and corroborating result controls/context, plus the requested cities and date within the heading. It no longer requires one of the old literal phrases such as "train list". It never books or confirms seat availability solely from a schedule. A verified result page can hand off to the separate factual reviewer. The original benchmark predicates are unchanged; its literal train-result predicate can still reject a real result page. Keep semantic recognition and benchmark pass scores separate until an independently reviewed benchmark revision is adopted.

Run:

    cargo test -p browsphere-core --offline -- --test-threads=2
    python tests/e2e/test_dom_interaction_patterns.py
    node --test tests/lucia-local-model.test.cjs tests/lucia-local-profiles.test.cjs tests/lucia-activity.test.cjs
    cargo build -p browsphere-core --offline
    python tests/e2e/run-lucia-phase3.py --case level-1 --train-date 2026-11-25

The agent:execution_state event contains only the classification, structural signal labels and counts in the page field. The benchmark captures these events as executionStates[].page. The real DOM fixture validates actual visible price/identity cards, excludes hidden cards, confirms offer blocks, reference content structure, loading and previous public journey-date behavior. The Rust tests verify positive and misleading pages and meaningful transitions. New site families and unusual markup may remain unknown; classification alone cannot prove retail identity, exact product specifications, evidence completeness, or general-purpose autonomy.
