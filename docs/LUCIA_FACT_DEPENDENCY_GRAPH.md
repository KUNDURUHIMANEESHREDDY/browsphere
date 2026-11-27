# Lucia fact dependency graph

## Purpose

Give the generic research orchestrator a **source × requested fact** checklist so it can acquire evidence before attempting synthesis. This is deliberately conservative and does not grant any new browser or MCP permission.

## Scope implemented

- Named public FREE-plan comparisons: each user-named official source contributes a `free_tier` obligation and, if team capacity was requested, a `free_team_capacity` obligation.
- Explicit public URLs with facets already understood by the native verifier: each requested source gets one obligation per supported facet (for example output behavior and limitation). Unknown or ambiguous goals continue through existing planners and independent verification; they are not reinterpreted as facts.
- Candidate evidence is extracted from the **captured source excerpt**, not model text or web search snippets. Public-domain identity and original exact URLs are enforced. Search-result snippets and lookalike domains cannot satisfy an official source requirement.
- `missing`, `candidate`, `conflicted`, `verified` are distinct states. Conflicting explicit FREE-tier collaborator limits from the same named source block synthesis.
- The active research stage receives a gap-focused objective. Comparison and final verification cannot complete with unresolved graph nodes.
- A source-exact independent report must support every graph dimension before nodes become `verified`. Quotes are checked against native source captures; final validation happens atomically so a partially supported report cannot mark some nodes as verified.
- `agent:fact_graph` publishes status and metadata only, not raw webpage excerpts or private model reasoning. The existing source/research events remain available.

## Safety and reliability

The graph is an **additional gate**, not an evidence generator. It never invents facts, destinations, controls, sources, or credentials. Existing read-only restrictions, policy checks, stage receipt rules, and source-specific evidence collection remain authoritative. Contradictory or missing evidence must produce an incomplete result, not an invented answer.

## Validation

`cargo test --workspace --bins fact_graph -- --nocapture` exercises source binding, dimensions, conflicting limits, stage-scoped gaps, and source-exact final-review promotion.

`cargo test --workspace --bins -- --test-threads=2` runs the full Rust regression suite.

## Limitations

This increment does **not** generalize arbitrary natural-language factual dimensions, resolve conflicts between different source versions, or prove reliable task completion on unfamiliar websites. Source-exact on-page acquisition remains subject to dynamic-site extraction quality. Writing, forms, calendar actions, and MCP workflows retain their existing contracts and require separate end-to-end proof.

## Live execution observation — 26 November 2026

- An isolated debug executable launched the **real chatbox and native browser** with Ollama and read two unfamiliar official documentation URLs: Python's data-structures tutorial and MDN's JavaScript `Array.map` reference.
- A request to compare what each source returns and describe a limitation initially ended `unverified`; it had visited both sources but the generic comparison parser treated requested *fact dimensions* as comparison operands.
- After guarding that parser and enabling fact-derived DOM focus terms, the repeated task ended `completed` with a native `evidence_checked` report. The acceptance harness recorded four source-bound findings across both official domains, all six compiled source+facet requirements covered, and four native actions.
- This constitutes **one pass**, not repeated-success reliability. The observed report is grounded in captured source passages, not independently audited as universally correct or necessarily the best possible comparison.
- The isolated debug executable is not the installed release. No installer or production installation has been replaced.