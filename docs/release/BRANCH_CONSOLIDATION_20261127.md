# Browsphere branch consolidation — 27 November 2026

## Status: integration candidate, NOT a release
- Base: origin/main (2523711)
- Head: integration/all-prs-20261127; draft PR #28.
- All 17 previously open draft PR heads (#11–#27) and their upstream dependencies are ancestors of this consolidation. Later unified-model, Qwen research, and verification work is also integrated with conflict resolutions.
- No original PRs, source branches, or main were deleted or modified; the developer's dirty Browsphere Windows checkout remained untouched.

## Superseded experimental branches
Seven divergent November 23 branch histories were merged with Git's **ours** strategy, preserving ancestry and newer source code but NOT replaying their obsolete patches:
- feature/lucia-general-sites-qwen-20261123
- feature/lucia-cross-site-reconciled-20261123
- fix/lucia-real-qwen-ci-deadline-20261123
- test/lucia-cloud-only-ci-20261123
- test/lucia-cloud-only-rebased-20261123
- test/lucia-native-windows-ci-20261123
- test/lucia-qwen-cross-site-validation-20261123

All paths changed on those branches already exist in the consolidated source; generic-site test fixtures, the cross-site plan, and cloud-provider tests matched exactly. Other code files differ because of newer changes. Merge ancestry alone does not prove behavior equivalence.

## Validation
- 18 top-level Node test files passed on the isolated Windows worktree.
- Additional unified-model, Stagehand URL policy, and release-integrity checks passed.
- JS syntax and Windows PowerShell script parsing passed.
- Local cargo check could not compile because MSVC link.exe was absent. Hosted Windows CI is required to determine build correctness.
- No repeatable real-Qwen unfamiliar-site completion, signed clean-machine installer, security review, or reliable memory/latency acceptance has been established for the combined head.

Before merging this draft PR into main: verify Windows Rust/Tauri build and NSIS packaging, independent real website task success, security and MCP permission gates, measured performance, and clean-machine install/update/uninstall. Build success is not agent success.