# 9Router OpenCode FreeUsageLimitError — 2026-11-23

## Observed provider evidence (read-only, no secrets)
- Local 9Router health endpoint on port 20128 was responsive (HTTP 200).
- 9Router usage history shows real **opencode / hy4-flash-free** successful inference requests up to **2026-11-23 16:53:36 UTC**.
- Recent request-details records on the same date, including approximately **17:36:44 UTC**, show failed calls with HTTP **429**.
- An allowlist-only inspection of those error entries matched **FreeUsageLimitError** and **Free usage limit reached**. No trustworthy reset time was present.
- No API keys, emails, raw prompts, provider response bodies, or other confidential 9Router fields are included in this report.
- Do not infer that 429 always means the account permanently lost access. The current evidence establishes a free-usage refusal on the observed requests, not its exact renewal schedule.

## Browser implementation
- `ModelRouter` now shares a provider/model-scoped cooldown across clones, so subsequent Lucia stages do not repeatedly hit a known 429. It uses `Retry-After` if provided, capped to one hour; otherwise a short 90-second local pause.
- Browser-action, planner, verification and connection-check calls all use the guarded route; no automatic fallback, account switching or provider rotation.
- When a 429 error body identifies `FreeUsageLimitError`, the user sees a specific and sanitized explanation instead of generic network-failure messaging. Raw upstream errors are never surfaced.
- The fallback for unclassified 429 still explains ordinary provider rate limiting.

## Verification
- Rust unit regression uses a loopback mock server to return a simulated FreeUsageLimitError, checks the response is sanitized, clones the model router, and asserts the next call is blocked without contacting the mock provider.
- `cargo test --workspace --locked --quiet`: **127 passed, 5 ignored, 0 failed**.
- `node --test tests/lucia-*.test.cjs tests/browser-state.test.cjs`: **18 passed, 0 failed**.
- `cargo build --locked --quiet` (Windows native debug): **passed**.
- Native WebView2 simulated-model integration: the real Windows app passed the DOM controls, model-selected search, SPA navigation, forms, dialogs, and failed-action recovery checks; the suite nevertheless **failed** at the final assigned-task-tab closure assertion (`Closing the assigned tab did not report the cause`). Other uncommitted tab lifecycle changes were active in the working tree, and this failure remains unresolved. It is not a real 9Router-model pass.

## Remaining acceptance gate
Do not represent successful browser-only controls, scripted mock actions, or previous Wikipedia success as multiple new real-model multi-site passes. Repeated genuine 9Router model-driven tasks on independent websites remain blocked until provider usage is again permitted.
