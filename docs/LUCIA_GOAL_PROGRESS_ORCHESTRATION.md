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