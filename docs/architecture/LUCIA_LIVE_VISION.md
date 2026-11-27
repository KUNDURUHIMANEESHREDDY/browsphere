# Lucia live visual perception

## Design

Reuse the existing Chromium DevTools Protocol exposed by Windows WebView2.
Lucia's normal DOM perception already receives real-time BRPR notifications
via MutationObserver, navigation, resize, and scroll. The native vision path
now samples a recent JPEG video frame from a WebView2 Page.startScreencast
stream rather than requesting a one-off Page.captureScreenshot.

Page.screencastFrame notifications are processed by a single native event
receiver attached to the child WebView. Each notification is acknowledged,
and a bounded in-memory store keeps the most recent compressed frame only.
The local vision model is invoked selectively for ambiguous public pages
and recovery. The native DOM binder, policy checks, and independent evidence
verifier remain authoritative.

This is live video frame acquisition, NOT continuous frame-by-frame LLM
inference or proof of autonomous task completion. The model examines the
latest sampled frame only when an already permitted vision decision is due.

## Limits and privacy

- No frames are persisted, logged, or returned to the frontend.
- The module limits each frame to 3 million encoded characters; the cache
  has at most eight tab slots and one latest frame per active tab.
- The upstream WebView2 screencast is requested at JPEG quality 45 and
  maximum 960x640, every fifteenth rendering frame. This is a limit, not
  a claimed constant FPS.
- The stream stops on successful observation, timeout, error, or tab closure.
  Successful observations await the stop acknowledgment.
- Existing VisionBudget gating blocks private mode and sensitive/non-public
  goals. Local Ollama remains required. Untrusted page events are never
  permissions or browser-action authority.
- Inference and coordinate suggestions must be checked against fresh
  native viewport, URL, DOM and current action policy.

## Acceptance gates

Windows CI runs a separate disposable native WebView2 fixture smoke, distinct
from the older multi-step Lucia test. It verifies a first streamed frame,
changes the rendered fixture, and requires both a BRPR change hint and a
different second streamed frame. The test sends no pixel data to the JS
client or CI logs. A Rust build alone is not evidence that streaming works.

Further work must measure p50/p95/p99 frame and inference latency, memory and
CPU impact under real navigation, and repeated task success with and without
vision on unfamiliar websites. Any clean-machine installed build and privacy
review must be separately verified. Never open a production CDP debugging port
or ship the test-only CI entrypoint as a generally callable browser API.
