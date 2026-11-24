# Lucia multimodal perception v11

The native semantic page profile remains authoritative for objective gates. The optional local vision model consumes a dedicated-tab viewport screenshot and a bounded list of visible, uniquely labelled, nonsensitive DOM candidates, with a coarse task category. The output schema contains page kind, layout, blocker, up to three candidate index/label pairs, and up to two visible excerpts. Unknown or malformed output is rejected. The screenshot is sent only to the configured loopback Ollama endpoint. The fusion layer re-observes the native DOM after inference and discards stale observations. It independently validates the exact candidate index, unique label, visibility, goal relevance, and current DOM identity. A conflicting visual class does not override native perception; visual blocker claims cannot promote controls. Independently localized coordinates can still be validated against native identity, visibility, relevance, bounds and hit-testing without trusting the visual class. Only bounded classifications and counts enter the compact action prompt, and grounded candidate IDs are prioritized in its existing native control list. Native binder, policy and verifier remain mandatory.

A run has at most three inference reservations, a three-step cooldown and no repeat for the same stage/page state. Eligible triggers are unknown page, sparse controls, ambiguous results, failed action or stalled recovery. Private browsing, detected sensitive forms and account paths, blocked/loading/empty pages, verification, and deterministic objective/source/route/filter tasks do not use the optional vision path. These protections are based on observed page state, not a guarantee that arbitrary public webpages contain no personal information. No screenshot, untrusted visual prose, or raw control ID is persisted in vision telemetry. The agent emits analyzing, observed, stale, unavailable, and grounded_action events. A matched action means its executed control matched a fused candidate; it is not evidence of a causal benefit from vision.

Validation:

    cargo test -p browsphere-core --offline -- --test-threads=2
    node --test tests/lucia-local-model.test.cjs tests/lucia-local-profiles.test.cjs tests/lucia-activity.test.cjs
    cargo build -p browsphere-core --offline
    python tests/e2e/run-lucia-phase3.py --case vision-fixture --case heldout-mdn

The vision fixture uses a real native WebView2 screenshot and the configured LFM3-VL model, with a controlled read-only local public-document catalog. It independently checks observed fusion, grounded control, budget, destination, source-backed answer, and no approval/write. MDN is a control case where deterministic current-reference review should not invoke vision. The original eight benchmark cases remain unchanged. The implementation does not infer exact product attributes from photos or establish that a requested result is factually verified by vision alone. Before/after screenshot comparison and richer visual record extraction are separate future capabilities.

## Guarded coordinate clicking

Vision may propose an optional normalized viewport point (0?999 on each axis) for a matched, uniquely labelled control. The host converts to CSS pixels using the captured viewport, checks the live bounds and re-observes the page before dispatch. The executor checks URL, viewport size, scroll, scale, bounds and hit-testing immediately before clicking. Only low/medium-risk, policy-allowed, nonsubmit actions may use this path. It refuses occluded or mismatched controls and unsupported form/canvas/iframe targets. Existing action acknowledgements are not proof of completion. Coordinate dispatch uses WebView JavaScript mouse events, not native OS mouse injection. No coordinates or screenshots are persisted in vision telemetry. The UI exposes a coordinate_click event.

Native Windows visual-model and real-site coordinate click tests must pass before declaring broad coordinate reliability.

## Coordinate recovery refinement

The coordinate matcher is independent of page-kind and blocker classifications. Native unsafe, dialog, loading and blocked states remain excluded. A uniquely relevant public link with a validated point can be proposed before free-form model actions; all existing action binding and policy checks remain required. Explicit visible named links may trigger vision even on a documentation page. Named search objectives reject unrelated observed navigation and prefer a unique visible search field. Route revisit checks apply after recovery substitution as well.

Repeat real-site tests against the copied debug executable: `python tests/e2e/run-lucia-phase3.py --case vision-real-example --case vision-real-python --case search-wikipedia`. The coordinate cases require a real coordinate dispatch, trace evidence, correct observed destination and verified completion. A successful ordinary DOM click does not satisfy the coordinate case.
