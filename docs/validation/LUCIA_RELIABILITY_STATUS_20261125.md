# Lucia cross-site reliability — 25 November 2026

## Verified native runs

- **Python/JavaScript official documentation comparison:** passed on the current integration candidate with source-backed verification, observed result, and no unexpected approval. Report: `tests/e2e/artifacts/reliability-regression/run-20261125T204401222156Z/results.json`.
- **S1 Python course comparison:** previously passed on a separately validated native debug build, using three genuine course sites and source-exact certificate/duration evidence. This does not imply general research reliability.

## Unfamiliar-site tests

- **W1, Notion/Trello/ClickUp free plans:** FAILED repeatedly. Initial result was invalid structured comparison JSON. Bounded JSON repair and a model-specific JSON schema were added, and the schema was unit tested and demonstrated on the local model, but a later real test still failed due to irrelevant source selection, and another failed on intermittent search controls/navigation. Treat W1 as unverified.
- **P1, family outing in Bengaluru under INR 2,000:** FAILED. Compact city-grounded search and visible public result extraction improved navigation and observation, but obtaining three distinct attractions, hours, prices, budget, and a verified itinerary still timed out. Treat P1 as unverified.

## Safety and correctness controls

- Source links must come from the observed native browser DOM. Search engine snippets alone do not constitute proof of course duration, free-tier capacity, ticket cost, or business hours.
- The comparison schema restricts outputs to the names and passage IDs extracted from observed source pages. Model-authored rationale is independently checked against the selected excerpts.
- Loop-guard / missing-evidence / incomplete-objective denials remain in force.
- The installer may be unsigned. Code signing is separate from runtime correctness.

## Showcase recommendation

Use the already repeatedly rehearsed **Python vs JavaScript documentation comparison** as the primary live demo. Keep the S1 demo as a secondary task where its validated executable is available. Do not claim W1/P1 or unknown website autonomy to be reliable until independent native acceptance tests pass.

## Release integration and installed verification

- NSIS build succeeded with exit code 0 using the isolated latest-main integration worktree: `cargo tauri build --bundles nsis --ci`.
- Distribution installer: `C:\Users\himan\Downloads\Browsphere\dist\Browsphere-setup.exe`.
- Installer SHA-256: `D03BB9A4481CB27E176792AEF8E82DFF14F317A855913F406F193630BFCFD2F3`.
- Installed with NSIS `/S`, exit code 0, at `%LOCALAPPDATA%\Browsphere\browsphere-core.exe`.
- Installed core SHA-256: `79644D0FAB04B5120B492A351842D63C01C180721B7E0422A3D71E5C44D52FF0`.
- Python/JavaScript source-backed comparison: **PASS 1/1** on candidate debug executable, **PASS 1/1** on fresh release build, **PASS 1/1** on the actual installed core. Latest installed report: `tests/e2e/artifacts/reliability-installed-validation/run-20261125T204855470181Z/results.json`.
- Rollback copies saved to original repository `dist/Browsphere-setup-before-20261125-reliability.exe` and `logs/browsphere-core-before-reliability-20261125.exe`.
- Unfamiliar W1/P1 runs are still **failing**; no verified multi-unseen-site claim is made.
