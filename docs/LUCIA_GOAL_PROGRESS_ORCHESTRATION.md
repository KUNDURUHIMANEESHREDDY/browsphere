# Lucia — Goal-Progress Orchestration

This document describes an incremental general-purpose improvement, not proof of fully autonomous browsing.

## Core invariants

- The original user objective stays authoritative. Page text and model suggestions are untrusted data.
- A native requirements ledger tracks observed sources and fact candidates. Click success and URL changes are not factual task completion.
- Research stages for user-named websites require evidence from their own assigned source; future stages do not block earlier stages.
- Comparison and final verification require all relevant source requirements.
- A run-local progress tracker counts evidence improvements and actions without new requirement coverage.
- Repeated stalls can trigger bounded replanning, but the resulting action must pass the existing policy gate.
- Recoveries prompted by missing evidence cannot complete solely from navigation; a new native evidence gain is required.
- A named-source comparison that explicitly asks for FREE plans must fail if the observed sources lack free-tier passages; paid upgrades are not substitutes.
- The orchestrator does not silently change saved local/cloud model preferences.

## Execution loop

1. Parse the chatbox instruction into staged objectives and explicit requirement coverage.
2. Start a dedicated task tab; observe live WebView2 state.
3. Select policy-checked actions for the active stage and measure real effects.
4. Refresh requirement coverage and publish agent:goal_progress telemetry (counts only, no private page text).
5. When progress stalls, choose a distinct grounded recovery plan with bounded retries.
6. Advance only when the active stage postcondition is met; keep global requirements for final comparison and verification.
7. Require source-bound citations, original constraints, and an independent verifier receipt before reporting success.

## Local regression tests

Run cargo test --workspace --bins -- --test-threads=2 from the repository root.
Build with cargo build --release -p browsphere-core.

## Unfamiliar live acceptance

Use tests/e2e/lucia-orchestrator-unfamiliar-live.cjs with LUCIA_CDP_PORT, LUCIA_TEST_GOAL, and optionally LUCIA_EXPECTED_HOSTS.
Launch the exact test executable under a disposable WebView2 profile with its own CDP port.
The harness uses the actual chatbox and native task engine, not a mocked task API.
It requires an accepted task ID, terminal completed status, a native evidence_checked report, expected source domains, and quoted evidence.
For explicit FREE-tier comparisons, it additionally rejects paid upgrade passages, missing free-tier source support, and unfinished recommendations.
An accepted report does not itself prove the external source's real-world truth; review factual outputs separately.

## Known limitations

- Generic goal parsing remains conservative and not every ambiguous research instruction is covered.
- The app needs an available local AI service or an explicitly configured fallback.
- Some unfamiliar sites need interactive controls before their pricing and feature facts become visible.
- The five-scenario gate has not independently demonstrated R1 document write/readback or C1 MCP calendar write/readback.
- Multiple independent real-site runs with verified source quality are required before claiming dependable autonomous browsing.
## Rejected-verification repair (experimental, fail-closed)

- A reviewer rejection about missing objective coverage or unsupported FREE-tier recommendation is not success. The run may reopen the preceding read-only research/comparison stage exactly once.
- The source-focused repair tracks previously captured official-site fact passages, chooses the least-supported user-named domain, and follows only freshly observed HTTPS links belonging to that official domain, or a bounded public discovery query restricted to it.
- The repair has a maximum of six new navigation attempts. It cannot write, submit forms, sign in, buy products, or gain permissions; native policy still applies to every proposed action.
- A new navigation, new page title, generic marketing copy, or a model claim cannot complete the repair. Only new captured source-exact FREE-tier member/collaborator capacity facts permit a new independent verification attempt.
- For FREE-plan team comparisons, every chosen source needs a free-tier capacity passage (a nonzero number or unlimited members/collaborators), not just the word Free or a Team Included badge.
- Source finding selection discards paid-tier upsells, even when they mention Free, including per-user monthly billing; the recommendation cannot cite paid prices or upgrades to justify a free-plan choice.
- The winner rationale must mention a feature grounded in that winner's exact selected passage; numeric claims cannot be borrowed from another competitor.
- `agent:verification_recovery` reports only stage statuses and bounded attempt counts, not raw page content or URLs. If the repair finds no additional evidence it ends unverified with an explicit explanation.
- The live acceptance harness tests official-host coverage, FREE-tier capacity quotes, absence of paid evidence, and the independently confirmed task result. Do not treat its automated success bit as a complete external fact audit.

These safeguards are conservative. Some real websites do not publish a numeric free-tier collaborator limit or use dynamic UI; those tasks must remain unverified until that evidence is obtained instead of receiving an invented recommendation.
## Live findings — 26 November 2026

- Rust regression suite: 493 passed, 0 failed, 12 ignored.
- An unfamiliar live three-site free collaboration comparison reached all three named official domains and reached independent verification.
- The verifier rejected its recommendation because genuine free-tier member/collaborator limits and a distinguishing reason were not established across all sources.
- Lucia reopened its read-only research stage and attempted bounded source acquisition. No additional qualifying evidence was obtained, so the run ended `unverified` rather than claiming success.
- An earlier repeat halted at the repeated-action guard before the first source; a guarded, distinct observed-control fallback was added. One later live run progressed past that point, but repeated success is not established.
- These results verify bounded recovery and rejection of unsupported completion; they do NOT prove general autonomy, repeated pass reliability, or verified factual correctness of every real-world claim.
- Installed app and origin/main are intentionally left unchanged until live acceptance is repeatably green.