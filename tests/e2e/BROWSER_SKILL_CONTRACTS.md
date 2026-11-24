# Lucia native browser skill library

Runtime revision: lucia-browser-skills-v6

The eight reusable primitives in `src-tauri/src/agent/browser_skills.rs` share the same live DOM snapshot, native action binder, and permission gate. They do not grant tool or website permissions. The compiled public-search state machine is in `agent/skills.rs`; the bounded route and explicit public-filter planners are in `agent/prerequisites.rs`.

| Skill | Preconditions | Observable outcome / limit |
|---|---|---|
| Search | Unique non-sensitive public search field, explicit authorized query | Input occupancy after native typing; target link observed separately. Occupancy is not exact value readback. |
| Navigation | Observed safe link, task-relevant score, route budget | Native destination and readable page checked; no guessed URL. |
| Autocomplete | Unambiguous matching visible option | Clicked matching option, popup closed, corresponding field populated. Does not certify hidden station ID. |
| Form | Explicit requested field/option or exact calendar-day label | Native select readback; date-picker actions are proposals, not date confirmation. |
| Research | Required source URLs and readable public observations | Each required source independently present. Source coverage is not factual coverage. |
| Extraction | Readable native capture | Bounded source excerpt or exact contiguous passage; no invented numbers. Candidate product prices must be tied to the correct listing in later review. |
| Verification | Actual source capture and required dimensions | Exact supporting quotes and every declared dimension; attribution, not independent fact checking. |
| Recovery | Observed failure classification and remaining budget | Reobserve, select alternate observed control, resolve ambiguity, collect evidence, or stop. Never bypass authentication or approval. |

## Independent contract tests

Run `cargo test -p browsphere-core --offline agent::browser_skills::tests -- --test-threads=2` from repository root. Tests cover both positive and negative outcomes for all eight skill types. The existing `agent::skills::tests` exercise the composed search workflow. The full suite checks compatibility with native policy, MCP, orchestration and evidence verification. `python tests/e2e/test_dom_interaction_patterns.py` exercises real browser DOM fixtures.

## Live acceptance gates

The complex benchmark requires the running app to report the v6 revision. The benchmark pass criteria are unchanged. Test each supported skill against different live sites before claiming generalization. The prior v5 repository lookup reached its target but did not prove installation facts; documentation hierarchy traversal remained incomplete. General-purpose natural-language compilation, full arbitrary form execution, cross-retailer product matching, and independent end-to-end reliability are not established by contract tests.
