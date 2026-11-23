# Browsphere diagnostics

Browsphere creates persistent local diagnostics automatically when the native app starts.

**Live logs folder (Windows):** `%LOCALAPPDATA%\BrowsphereAI\logs\`
**Source documentation:** `logs/README.md` (this file).

## Log channels

- `runtime.jsonl`: app sessions and tracing framework event metadata
- `browser.jsonl`: navigation and page-load events (never full URLs)
- `agent.jsonl`: Lucia agent run IDs, sanitized step/action traces, risk, policy outcome, model-route class, latency, and tab lifecycle (`source` / `kind` / `status`)
- `model.jsonl`: model call status and latency, never prompts/responses
- `automation.jsonl`: scheduled workflow starts, per-step results, and completions
- `voice.jsonl`: local/cloud voice synthesis status and latency
- `security.jsonl`: permission changes/denials and pop-up blocking
- `downloads.jsonl`: download outcomes, not URLs or filenames
- `ui.jsonl`: uncaught frontend errors and unhandled promise rejection counts (no stacks or raw error text)
- `errors.jsonl`: categorized runtime failures without raw exception content
- `crash.jsonl`: Rust panic file/line metadata without user data or panic payload

Each entry contains a timestamp, session ID, process ID, channel, event, and restricted metadata fields. Files rotate after **2 MiB**, retain **4 backups per channel**, and rotate/delete logs older than **14 days** on startup.

During a Lucia run, `tab_lifecycle` identifies an explicit `user_close`, `private_session_end`, `startup_rollback`, or `unknown_close`, separately from unexpected `webview_missing`. It records no URLs or tab IDs; private browsing runs do not write agent diagnostics. Missing WebViews are not silently recreated or replayed because browser actions may have side effects.

The existing SQLite database still stores the ordinary agent trace history for the app's trace viewer. Its contents are intentionally **excluded** from diagnostics export.

## Inspect / export

In **Settings → AI & ML → Local Logs**, use **Open Logs** to open the live folder. **Export Diagnostics** saves a combined JSONL file under `%LOCALAPPDATA%\BrowsphereAI\diagnostic-exports\`. Export is manual and is not uploaded automatically.

The **Lucia Diagnostics** dashboard directly below Local Logs reads the same locally saved, sanitized metadata. Use **All runs**, **Needs attention**, or **Completed**, select a run, and inspect its action timeline and policy outcomes. **Refresh** reloads the retained logs. The dashboard intentionally omits raw browser content, prompts, exact URLs, arbitrary failure text, and private runs; a precise root cause may therefore be unavailable.

Logs may contain operational metadata even after redaction. Review diagnostic exports before sharing. No keys, passwords, page content, prompts, model responses, full URLs or private browsing agent runs are deliberately persisted by the diagnostics system. Logger failures never block normal browsing.
