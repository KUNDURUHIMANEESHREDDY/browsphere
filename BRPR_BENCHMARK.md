# BRPR native WebView2 end-to-end benchmark

Date: 23 November 2026. Windows debug build, isolated native Browsphere process and local HTTP fixture at `http://127.0.0.1:18764/`.

Test: `tests/e2e/brpr-live.cjs`. It opens the local fixture in a child WebView2, invokes the trusted `run_brpr_live_benchmark` Tauri command, and performs visible DOM text mutations. Browser MutationObserver sends `brpr:dirty` through native WebView2 WebMessage; the Rust cache records a new generation and refreshes the structured DOM with native `ExecuteScript`. Each iteration succeeds only if a *new native hint* is counted and the cache subsequently contains that iteration's unique text marker. Nothing is submitted or clicked on a third-party site.

## Results
| Window | Metric | p50 | p95 | p99 | Failures |
|---|---|---:|---:|---:|---:|
| 25 mutations | Host dispatch -> native hint | 1.8417 ms | 2.2458 ms | 298.7347 ms | 0 |
| 25 mutations | Host dispatch -> usable new observation | 47.4675 ms | 57.7562 ms | 329.6921 ms | 0 |
| 100 mutations | Host dispatch -> native hint | 1.5495 ms | 2.0598 ms | 3.0746 ms | 0 |
| 100 mutations | Host dispatch -> usable new observation | 47.5080 ms | 48.1145 ms | 62.3031 ms | 0 |

The first smaller run contained an outlier. Its cause has not been established. The longer 100-change run had steadier tail performance.

## Measurement interpretation
Both measurements start from a Rust `Instant` taken **before** the trusted native `ExecuteScript` call, rather than at the exact JavaScript mutation; thus the values are upper bounds and include host dispatch overhead. "Native hint" ends after the WebView2 WebMessage callback invalidates the cache, observed with Rust polling. "Usable new observation" ends when the trusted native DOM snapshot containing the new marker is accepted into the BRPR cache; it includes browser scheduling, the 16ms JavaScript signal coalescing interval, the 24ms Rust debounce, script extraction and polling. No wall-clock or JavaScript-to-Rust clock synchronization is claimed.

`host_p50_us` and similar unit-test numbers are *not* end-to-end metrics. No 30-microsecond reaction guarantee is established. This benchmark does not cover model decision time, user approvals, external sites, downloads or end-to-end action execution.

## Reproduction
1. Build with `cargo build --manifest-path src-tauri/Cargo.toml`.
2. Start the fixture with `python -m http.server 18764 --bind 127.0.0.1 --directory tests/e2e/fixture`.
3. Launch the debug Browsphere executable with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9258 --remote-allow-origins=*` and an isolated `LOCALAPPDATA` directory **only for local testing**. Do not expose the debugging port to untrusted networks.
4. Run `node tests/e2e/brpr-live.cjs`.
5. Inspect `tests/e2e/brpr-live-results.json`. Terminate the test browser and fixture server afterwards.

The synthetic benchmark command is restricted to the trusted main app view and the exact localhost test fixture. Real user pages cannot invoke it.
