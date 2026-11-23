# Browsphere — AI-Native Windows Desktop Browser (Tauri v2 + WebView2 + Rust)

A production-structured Windows desktop browser scaffold built with **Rust**, **Tauri v2**, and **WebView2**, featuring:
- **macOS-inspired light minimalist UI** with a **top-right AI Quick Chat popover** (no permanent right sidebar) and **non-intrusive background execution**.
- **Policy-Gated ReAct Browsing Agent** with deterministic risk classification (`low`, `medium`, `high`, `critical`), human-in-the-loop approval channels, untrusted page-data tagging, and step-by-step **execution trace logs**.
- **Hybrid Model Router** supporting local **Ollama** (e.g., `qwen3.5:4b`) and **LiteLLM** proxy/multi-provider endpoints with per-domain routing rules.
- **SQLite WAL** (`PRAGMA journal_mode = WAL;`) persistence for browsing history, downloads, agent runs, and execution traces.
- **Windows Credential Manager** (`keyring` crate on Windows) for storing API keys and secrets securely outside plain config files.
- **Intelligent Tracking Prevention (ITP)** tracker/fingerprinting blocker & native download manager.

## Quick Start (Windows)

1. Install **Rust** (`rustup`), **Node.js** (v26+ supported), and **WebView2 Runtime** (preinstalled on modern Windows 10/11).
2. Ensure **Ollama** is running locally (`ollama pull qwen3.5:4b`) or configure a **LiteLLM** endpoint.
3. Run `build-windows-installer.bat` or run `cargo tauri dev`.


## Optional AWS account connection

In the Windows app open Settings > AI & ML > Connect AWS. The button uses your existing AWS CLI v2 profile, opens the official AWS browser login when necessary, and discovers existing private Browsphere resources automatically. It saves resource identifiers locally rather than asking you to paste access keys. Bedrock inference and cloud data transfers remain off until explicitly enabled. For prerequisites, resource naming rules, permissions, and security details, see AWS_INTEGRATION.md.
