# Browsphere — Windows Desktop E2E Test Report

Date: 22 November 2026
Project: C:\Users\himan\Downloads\Browsphere
Application: Tauri v2 + WebView2 development build
Result: **15/15 runtime UI/integration checks passed**

## Verified in the running Windows application

1. Start page renders correctly.
2. Native Tauri bridge is available.
3. Memory toolbar button is absent.
4. SQLite memory-list command works through the native bridge.
5. SQLite browsing-history query works through the native bridge.
6. Settings → Privacy pane renders.
7. The Privacy memory list resolves live data from SQLite.
8. A temporary, uniquely identified memory appears in the Privacy UI.
9. Deleting that memory via the Privacy UI removes it from SQLite.
10. Automatic-memory switch turns off and on and saves its setting.
11. The agent launch path reads the persisted memory setting.
12. Private Browsing switches on and then back off.
13. Scrolling reaches the News section.
14. Live Hacker News items populate the News section (35 real stories).
15. No uncaught JavaScript exceptions were observed.

The test memory was separately verified absent from the SQLite database following the run. Existing user memories were not deleted.

## Bug fixed during testing

The native frontend bridge used to inspect a window-level browserSettings object that was not actually global. Disabling automatic memory therefore did not reliably propagate into subsequent agent tasks.

The bridge now reads the persisted automatic-memory preference directly from localStorage, matching the checkbox in Settings → Privacy.

## Outstanding limitation

**A complete real-LLM agent run was not tested.** Both local model endpoints were unavailable on this machine during the test:
- Ollama: 127.0.0.1:11434 (connection refused)
- LiteLLM: 127.0.0.1:4000 (connection refused)

Memory storage, retrieval commands, and real UI deletion were tested. AI inference and automatic extraction still require an online model endpoint for a separate live-model test.

## Evidence (local files)

- artifacts/01-start.png — start page
- artifacts/02-privacy.png — Privacy panel
- artifacts/03-news.png — news section
- artifacts/results.json — complete machine-readable results

## Repeatable harness

The CDP test script is tests/e2e/cdp-smoke.mjs.
The temporary data seeding and cleanup helper is tests/e2e/memory_fixture.py.

Only use CDP remote debugging during an authorized local test. Shut down the debug instance when testing is complete. The fixture helper only creates and removes the E2E record named browsphere-e2e-memory-fixture.
