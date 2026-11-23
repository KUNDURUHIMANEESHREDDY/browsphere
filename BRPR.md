# Browsphere Reactive Perception Runtime

BRPR installs a top-level MutationObserver, navigation, resize and scroll listeners in child WebView2 pages. Changes generate content-free dirty hints to native Rust through WebMessageReceived. Each hint invalidates a per-tab cached observation; a bounded background worker refetches structured DOM using the trusted native WebView2 ExecuteScript callback. Cache generations prevent stale captures from replacing new ones.

The source webpage is untrusted: hints never supply page contents, drive clicks, or override policy and approval. Existing Lucia action validation still requires fresh native snapshots. Cached state is not an authority for sensitive actions. Cache is in-memory only.

get_brpr_status is available only from trusted app views. Host p50/p95/p99 measure the native invalidation function only, NOT browser scheduling, model reasoning, screenshots or full reaction latency. The 0.03 ms target is not a guarantee.

Run: cargo test --manifest-path src-tauri/Cargo.toml reactive_perception -- --nocapture

Read-only cached structured page access: get_brpr_cached_observation (trusted app views only; at most 250 ms old). Closing a tab immediately removes its cache.
