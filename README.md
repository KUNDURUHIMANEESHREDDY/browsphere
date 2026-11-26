# Browsphere

**An agent-first desktop browser powered by Lucia.** Instead of manually repeating searches, clicks, and comparisons, describe a goal and let Lucia attempt to plan, interact with real pages, recover from failures, and verify the result.

Built for **Windows** with **Rust, Tauri 2, and Microsoft WebView2**, with a web-based interface, local-first AI options, optional cloud inference, and experimental Model Context Protocol (MCP) support.

> **Status ? 25 November 2026:** Working research prototype, **not a production-ready browser or a reliably general-purpose autonomous agent**. Some real native browser workflows have passed verification; other unfamiliar multi-site tasks still fail. A successful page load, simulated-model run, or passing unit test is **not** evidence that Lucia can complete arbitrary tasks autonomously. See [verified demo results](#verification-and-demo-readiness).

## Why Browsphere?

Traditional browsers help people navigate pages. Browsphere explores a different interaction model: **tell the browser what outcome you want**, rather than prescribing every action.

Lucia aims to understand the request, break it into requirements, act through a real browser tab and available tools, collect source evidence, recover when a page changes, and report success **only when the observed result supports it**.

## Features

| Area | Current scope |
| --- | --- |
| Browser | WebView2 tabs, navigation, address/search bar, history, bookmarks, downloads, private-session handling, and news feed. |
| Lucia | Research, browser-action, comparison, form, data, and verification workflows with observed page controls and task tabs. |
| Task orchestration | Requirement tracking, action selection, checkpoints, bounded replanning, stalled-loop detection, and completion gates. Reliability varies across sites. |
| Evidence | Source URL and original-page evidence collection, relevance checks, and independent completion verification. A captured quote does not necessarily prove the claimed conclusion. |
| Models | Local Ollama, selectable specialist models, and configurable OpenAI-compatible hosted endpoints. Model quality, availability, and hardware constraints affect outcomes. |
| MCP | Tool discovery and hybrid browser/tool orchestration under development; integration and permission behavior require individual tests. |
| Voice | Microphone-driven interaction and voice-provider integration paths; support depends on the selected device and provider. |
| Diagnostics | Task/action timeline, local logs, checkpoints, and test harnesses for inspecting failures. |

## How Lucia works

```text
User goal
   |
   v
Interpret requirements and plan
   |
   v
Choose browser / research / tool specialist
   |
   v
Observe WebView2 URL, page state and available controls
   |
   v
Select and validate action -> execute -> re-observe
   |                              |
   |                         failure or stall
   |                              |
   |                        bounded recovery
   v
Collect relevant source evidence
   |
   v
Independent completion verification
   |
   +-- Verified -> report grounded result
   +-- Missing / blocked -> report incomplete or failed
```

The browser, model, verifier, and MCP servers are separate trust boundaries. Website and tool output is treated as untrusted data; the action/policy layer should reject unsupported actions and sensitive operations that require appropriate controls. **Do not interpret a model-generated completion message as a verified browser outcome.**

### Local-first model architecture

The project's specialist model stack includes:

| Model | Intended use |
| --- | --- |
| Qwen3.5 0.8B | Lightweight browser decisions and local tasks |
| Qwen2.5 3B Instruct | More capable structured-action, planning, or evidence selection routes |
| LFM3 1.7B | Supervisory and recovery support |
| LFM3-VL 2B | Visual-page understanding experiments |

Routes are configurable and **not every model is bundled in the installer**. Local models require suitable runtimes and memory. Hosted providers can fail due to credentials, quotas (HTTP 429), missing models, or timeouts. Embedded-model packaging is documented in [Bundled model setup](docs/BUNDLED_MODEL.md).

### MCP integrations

The MCP design allows Lucia to discover and choose compatible tools alongside browser interactions. This is **not** a claim that all servers or hybrid tasks work end to end. Review [MCP](docs/MCP.md) and [Lucia MCP orchestration](docs/LUCIA_MCP_ORCHESTRATION.md) for design and test scope.

## Quick start (Windows)

**Prerequisites:** Windows 10/11, Edge WebView2 Runtime, Git, Rust/Cargo with MSVC build tools, Node.js/npm, and Tauri CLI v2. An optional locally installed Ollama model or authorized cloud provider is needed for model-driven Lucia tasks.

```powershell
git clone https://github.com/KUNDURUHIMANEESHREDDY/browsphere.git
cd browsphere

cd ui
npm ci
npm run build:css
cd ..

cd src-tauri
cargo tauri dev
```

If Tauri CLI is missing, install it with:

```powershell
cargo install tauri-cli --version "^2" --locked
```

The desktop browser can launch without a working inference provider, but **Lucia cannot complete model-driven workflows without one**.

### Configure AI

Open **Settings > AI & ML** in Browsphere, configure the local or cloud route, select an available model, and test connectivity where supported. Keep API keys out of source control and documentation; the Windows implementation uses credential-management paths for secrets.

- **Local:** start Ollama and select an installed model; not all smaller models produce reliable structured actions.
- **Cloud:** use a supported OpenAI-compatible endpoint with authorized credentials and a real available model.
- **9Router/OpenCode:** experimental paths; catalog discovery does not guarantee successful inference, quota, or permission.

Use Lucia Diagnostics and local logs when an action, navigation, or verification fails. HTTP 401/403 normally signals access issues; 429 signals rate limiting.

### Build an installer

From the repository root:

```powershell
.\build-windows-installer.bat
```

When successful, the script places an installer at `dist\Browsphere-setup.exe`. An unsigned installer does not establish clean-machine compatibility or release readiness. See [Windows release signing](docs/release/RELEASE_SIGNING.md). The optional offline model build uses the separate root `prepare-bundled-*.ps1` and `build-offline-installer.ps1` scripts; see [bundled-model documentation](docs/BUNDLED_MODEL.md).

## Verification and demo readiness

The following reflects **recorded 25 November 2026 development and installed-app runs**, not a fresh acceptance run of this README commit. Results from different binaries and times must not be combined into a single pass rate.

| Scenario | Evidence recorded | Readiness |
| --- | --- | --- |
| Python list comprehensions vs. JavaScript `Array.map` using official Python and MDN docs | Native core comparison passed in repeated showcase runs; source-backed comparison also passed on an installed release candidate. | **Best rehearsed live demo** (still retest the actual executable before presenting). |
| Wikipedia article lookup | Passed some runs, but one wrong-article navigation was correctly rejected. | Variable; use as a secondary scenario. |
| S1: compare three free beginner Python courses, certificates, and durations | A separate candidate previously passed, but the later installed audience acceptance run failed. | **Not reliably passing**. |
| W1: compare Notion, Trello, and ClickUp free plans | Installed audience test failed due to navigation / missing evidence / timeout. | **Not reliably passing**. |
| P1: plan a Bengaluru family outing within a budget | Installed audience test failed with navigation and low-memory constraints. | **Not reliably passing**. |

A task counts as passed only when its observed page state and relevant original-source evidence satisfy the **whole** request; partial navigation, irrelevant snippets, or a simulated-model success do not qualify.

Read the detailed [showcase acceptance](docs/validation/DEMO_SHOWCASE_ACCEPTANCE.md), [audience acceptance](docs/validation/DEMO_AUDIENCE_ACCEPTANCE_20261125.md), and [reliability status](docs/validation/LUCIA_RELIABILITY_STATUS_20261125.md). The exact executable and dated artifacts are documented there.

### Run checks

From the repository root:

```powershell
# Rust workspace tests
cargo test --workspace --locked

# JavaScript contracts (PowerShell)
node --test (Get-ChildItem tests -Filter '*.test.cjs' | ForEach-Object FullName)

# Native multi-step test (requires the appropriate Windows executable and test setup)
powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-lucia-multistep.ps1

# Source-backed showcase tests (requires native executable and supported local model)
python tests/e2e/run-lucia-phase3.py --case compose-two-sources --repeat 2 --output tests/e2e/artifacts/demo-showcase
```

Real-site tests can require browser profiles, a local model, credentials, and network access. Inspect the generated JSON reports; do not count skipped, blocked, or unverified actions as successes.

## Repository layout

```text
browsphere/
|-- src-tauri/                Rust backend, WebView2, Lucia agents, routing, storage
|-- ui/                       Browser interface and Tailwind CSS
|-- tests/                    JS tests and native Windows / live-site E2E
|-- docs/
|   |-- architecture/        BRPR and voice-agent notes
|   |-- integrations/        Optional service integration docs
|   |-- validation/          Showcase, acceptance and reliability reports
|   |-- release/             Production and signing gates
|   `-- ...                  MCP, model and workflow specifications
|-- scripts/maintenance/     One-off Python migration/diagnostic utilities
|-- .github/workflows/        Windows CI and release workflows
|-- logs/                     Local diagnostics guidance (generated logs ignored)
|-- Cargo.toml, Cargo.lock    Rust workspace definitions
|-- build-*.ps1 / *.bat       Root-level installer entry points
`-- README.md                 This document
```

The release/build scripts remain at the root because existing commands and relative paths depend on that location. `dist/`, `target/`, local model weights, test artifacts, credentials, and operational logs are not committed to the repository.

## Known limitations

- **Autonomy:** browser navigation and structured-action improvements are real, but end-to-end success across unfamiliar websites remains inconsistent.
- **Grounding:** a source URL or exact quote is insufficient unless the content is relevant and covers every required fact.
- **Web compatibility:** complex SPAs, popups, logins, uploads, and dynamically generated controls need more cross-site testing.
- **Model constraints:** local inference can be slow or memory-limited; cloud inference can be unavailable or rate-limited.
- **MCP and bundled runtime:** broader server compatibility, permission handling, offline packaging, and clean-install checks remain incomplete.
- **Distribution:** code signing, CI/release validation, clean installs, upgrades, and updater security are separate release gates.

## Documentation

Start at the [documentation index](docs/README.md). Useful references include [Lucia workflows](docs/LUCIA_WORKFLOWS.md), [adaptive graph](docs/LUCIA_ADAPTIVE_GRAPH.md), [BRPR](docs/architecture/BRPR.md), [release gates](docs/release/LUCIA_PRODUCTION_GATE.md), [voice agent](docs/architecture/VOICE_AGENT.md), [AWS integration](docs/integrations/AWS_INTEGRATION.md), and [diagnostic logs](logs/README.md).

## License

[MIT](LICENSE) ? 2026 HIMANEESH REDDY K.
