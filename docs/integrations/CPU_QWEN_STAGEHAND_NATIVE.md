# Lucia upstream-first CPU action recovery and WebView2 attachment experiment

Status: **experimental branch; do not merge until current Windows CI and real-Qwen
evidence are green.** This is stacked on the existing Stagehand v4 + official
MCP SDK branch. No new browser, CDP, MCP protocol, agent planner, or LLM inference
engine was implemented.

## Reused capabilities

- Browserbase **Stagehand v4**: `localBrowser.connect({cdpUrl})` for read-only
  compatibility inspection against the **temporary CI-only installed WebView2**
  profile; existing Stagehand MCP `inspect`, `observe`, `extract` stay unchanged.
  A WebView2 attach test is not a successful Stagehand action or live Lucia task.
- **Ollama** native `/api/generate` metrics: `load_duration`,
  `prompt_eval_count`, `prompt_eval_cached_count`, `prompt_eval_duration`,
  `eval_count`, `eval_duration`, `total_duration`. Values are nanoseconds
  and counts from Ollama itself; only these counters, the selected model,
  prompt size and elapsed time are logged, never model input/output.
- Lucia's **existing** constrained action grammar and observed-control aliases:
  apply it to `qwen2.5:3b-instruct` as well as the smaller Qwen/LFM3 models.
  Native live-DOM binding, permission gates, page trust boundaries, recovery
  and independent source verification are unchanged.
- Existing upstream Microsoft Playwright interop remains the installed-app
  CDP test harness. No CDP debugging port is exposed in consumer builds.

## Verification

The branch-specific Windows CI runs Rust unit/regression tests, Windows installer
build and clean install, Stagehand native WebView2 attachment, existing
Stagehand MCP read-only-tool calls through the installed Lucia chatbox, and
the six **real-Qwen** unfamiliar-site tests (three each on web.dev and W3C).

A successful release compilation, a green Stagehand attach or an MCP tool result
**never** counts as a completed real-model website task. Every real-Qwen task
needs at least one genuine Qwen browser action, the exact requested source,
independent evidence readback and a successful final status.

A timed-out action cannot expose Ollama's final token counters because the
request has been cancelled. The caller separately emits numeric
`agent:model_performance` events with `wall_ms`, `budget_ms`,
`prompt_chars`, and `timed_out`. The unfamiliar-site JSON report records the
last twelve events per attempt.

Detailed Ollama counters are persisted by the app's existing privacy-guarded
`model.jsonl` logger **only in CI**, when `GITHUB_ACTIONS=true` and
`LUCIA_E2E_REAL_MODEL` exactly matches the selected local model. The
test harness collects permitted numeric fields before deleting each disposable
profile. Ordinary/private browsing never enables this diagnostic mode; raw
prompts and model responses are neither recorded nor exported.

## Safety

Stagehand CDP attachment is opt-in **only** when `GITHUB_ACTIONS=true` and
`BROWSPHERE_STAGEHAND_WEBVIEW2_CI=1`, with an ephemeral allowed loopback
port. It reads page metadata only and never calls `stagehand.act`, requests a
tool permission, navigates an account session, or closes the installed browser.
The normal signed/release browser must not use an open CDP port.

Remaining gates: evaluate Qwen's new completion rate, compare faster installed
models against **actual native tasks** before dynamically choosing routes,
validate source-semantic correctness, keep installer and security checks green,
and measure full-process p50/p95/p99 on a clean Windows machine.

Prior measured baseline: **0/6** source-verified real-Qwen completions, with
85-second native action timeouts. This branch claims no improvement until CI
reports separately establish it.
