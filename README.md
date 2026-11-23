# Browsphere

**An AI-native Windows browser with Lucia, a policy-gated agent that can plan, use real webpage controls, and verify what happened.**

Built with **Rust + Tauri v2 + Microsoft WebView2**. Browsphere combines a desktop browsing interface, privacy-focused local storage, and optional local or cloud AI models.

> **Project status — 23 November 2026:** Working research prototype, **not production-ready**. Browsphere's native browser features and several agent workflows have passed targeted tests. **General-purpose, reliably autonomous browsing across arbitrary sites has not been demonstrated.** The latest repeated 9Router live-model tests are blocked by provider HTTP 429 limits. See [verified results](#verification-snapshot) before interpreting capabilities.

## What it can do

| Area | Current implementation |
| --- | --- |
| Desktop browsing | Windows WebView2 tabs, address/search bar, history, bookmarks, reading list, navigation/reload, downloads, and a news feed. |
| Privacy and site controls | Separate private WebView2 sessions, local persistence, tracking-prevention components, HTTPS-origin site permissions, and consent for popups. Sensitive capabilities are denied by default. |
| Lucia assistant | AI chat and a task runner with **Research, Browser, Comparison, Form, Data, and Verification** specialists. Tasks operate on observed webpage controls rather than assuming clicks worked. |
| Browser actions | Page observation, navigation, clicking, typing, supported form controls, multi-step execution, background task tabs, cancellation, and bounded failure recovery. Support varies by site. |
| Evidence and safety | Fresh-DOM action binding, risk-based approval for consequential actions, source/URL checks before completion, and fail-closed behavior for unsupported or ambiguous outcomes. |
| Model routing | Local **Ollama** and configurable OpenAI/LiteLLM-compatible cloud endpoints, with model selection and endpoint-specific credentials. Includes a **9Router** configuration path. |
| Voice | Microphone input and optional voice-agent mode, with ElevenLabs, Kokoro, and Windows-voice integration paths; some provider/device combinations still require validation. |
| Diagnostics | Local structured logs, a Lucia run/action timeline dashboard, manual diagnostics export, and agent trace history. |
| Optional AWS | AWS CLI-profile-based account connection/resource discovery for Bedrock, S3, DynamoDB, Cognito, and Lambda; cloud services are not required for everyday browsing. |

**A successful website load is not the same as successful AI autonomy.** Direct navigation, deterministic/simulated-model tests, and genuine hosted-model agent tests are reported separately.

## Quick start (Windows)

### Requirements

- Windows 10/11 with Microsoft Edge **WebView2 Runtime**
- Rust toolchain and Cargo (including the Windows MSVC build tools)
- Node.js and npm
- Tauri CLI v2 (install with `cargo install tauri-cli --version "^2" --locked` if needed)
- Optional: a running Ollama server or an authorized OpenAI-compatible model endpoint for Lucia inference

### Run from source

~~~powershell
git clone https://github.com/KUNDURUHIMANEESHREDDY/browsphere.git
cd browsphere

cd ui
npm ci
npm run build:css
cd ..

cd src-tauri
cargo tauri dev
~~~

The browser can be explored without configuring a model; **Lucia's model-driven tasks require a functioning model provider**. Browser rendering and model connectivity are separate checks.

### Build the Windows installer

From the repository root, run:

~~~powershell
.\build-windows-installer.bat
~~~

This builds the UI assets and a Tauri NSIS installer, then copies it to `dist\Browsphere-setup.exe` when successful. The ordinary development installer is **unsigned** unless release signing has been configured. A built installer is not evidence of successful clean-machine installation, upgrades, or production readiness. See [release signing](RELEASE_SIGNING.md).

## Configure Lucia models

Open **Settings → AI & ML** and configure a model route. API credentials are managed separately from ordinary UI preferences through **Windows Credential Manager**; do not put keys in the README, source control, logs, or screenshots.

| Provider path | What to configure | Notes |
| --- | --- | --- |
| Ollama (local) | Start Ollama, download a supported model, select the local route | Small models may produce malformed structured actions or time out on complex sites. |
| OpenAI/LiteLLM-compatible cloud | Authorized endpoint, API key where required, and an available model | Model discovery and successful inference are different checks; use the model returned by your provider. |
| 9Router | Run and authorize the local gateway, then configure its **chat-completions endpoint** and select a fetched model | The tested gateway endpoint was `http://127.0.0.1:20128/v1/chat/completions` with model `oc/hy4-flash-free`. The base URL `http://127.0.0.1:20128/v1` is **not itself** a chat-completions request URL. This route uses hosted inference via 9Router, not local Ollama. Quotas apply. |
| OpenCode | Experimental model-contract testing only | OpenCode free-model catalog access does **not** constitute a supported, functioning Browsphere inference integration. See [OpenCode testing](OPENCODE_LUCIA_TESTING.md). |

Use the **Fetch Models** flow where supported to inspect the endpoint's current model list, then explicitly select the model. A model appearing in a catalog does not guarantee authorization, quota, or successful requests.

If Lucia fails, inspect **Settings → AI & ML → Local Logs → Lucia Diagnostics**. An HTTP **401/403** generally points to authorization/access restrictions; **429** indicates rate limiting. Browsphere should report these failures instead of silently switching providers or claiming task completion.

## How Lucia operates

1. **Plan:** interpret the user's request and select an appropriate specialist.
2. **Observe:** capture the active task tab's native WebView2 URL, page content, and interactive elements.
3. **Act:** let the selected model propose a supported action; check it against current DOM and policy before execution.
4. **Re-observe / recover:** inspect the actual result, handle supported navigation delays or action failures, and stop on persistent uncertainty.
5. **Verify and report:** finish only when the independently observed page state supports the claimed result.

Routine, low-risk interactions can proceed automatically. Sensitive data, purchases, account changes, and other consequential operations remain behind policy checks or approval. Rejected/cancelled approvals stop the action. Lucia should not bypass website restrictions or invent outcomes.

The intended scope is **general websites**, not a hardcoded YouTube script. Actual coverage remains dependent on webpage structure, browser controls, provider quality, latency, and rate limits.

## Verification snapshot

**As of 23 November 2026**, from recorded development runs—not a claim that every run will pass:

| Test | Observed result | What it establishes |
| --- | --- | --- |
| Rust workspace regressions | **123 passed, 5 ignored, 0 failed** | Code-level behavior; ignored cases require optional dependencies/services. |
| JavaScript/browser-state regressions | **16/16 passed** | Selected UI/agent contracts, including model configuration. |
| Native public-site browser-control tests | **5/5 passed** on the final repeated samples | Real WebView2 link clicks and resulting page/URL verification on Python Docs, MDN, Wiktionary, GitHub Docs, and Hacker News; **no AI model**. |
| Cross-site browser compatibility | **17/17 passed** in a separate earlier run | Real navigation, permission/popup controls, and address-bar behavior on several public sites; **not autonomy**. |
| Native cloud-compatible simulated-model E2E | **Passed** | Search/click, SPA transitions, forms, recovery, background tabs, and verification using a **simulated** model endpoint. |
| Genuine 9Router `oc/hy4-flash-free` | **One verified Wikipedia multi-step task succeeded** | The model selected real actions, opened the Ada Lovelace article, and passed final page/URL verification. It is **one scenario**, not broad reliability. |
| Later genuine 9Router tests on new sites | **Not passed** | Wiktionary stopped without a verified model action; Python Docs returned HTTP **429** before any model action; MDN and GitHub Docs agent tasks were not attempted after the limit. |

The latest reliability patch stops inappropriate quota retries, improves timeout handling in the native test harness, and clearly labels skipped tasks. It **does not** remove provider limits or prove multi-site autonomy.

Full records: [9Router and public-site tests](tests/e2e/REAL_SITE_RESULTS_20261123.md), [cross-site live results](tests/e2e/CROSS_SITE_LIVE_RESULTS_20261123.md), [test acceptance plan](tests/e2e/CROSS_SITE_TEST_PLAN.md).

### Run tests

~~~powershell
# Run from the repository root
cargo test --workspace --locked
node --test tests/lucia-*.test.cjs tests/browser-state.test.cjs

# Native Windows deterministic/simulated-model E2E
powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-lucia-multistep.ps1

# Native browser-only tests on real public sites
powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-real-public-site-controls.ps1 -CdpPort 9563

# Genuine 9Router agent test (requires configured gateway, credentials and available quota)
powershell -NoProfile -ExecutionPolicy Bypass -File tests/e2e/run-lucia-real-sites-9router.ps1 -Site "Python documentation" -CdpPort 9554
~~~

The native suites use a **disposable test profile** and local debugging facilities; do not expose their CDP port to remote networks. A failed live-provider test must remain failed, even when simulated-model or browser-only tests succeed.

## Architecture

~~~text
Browsphere
├── src-tauri/       Rust/Tauri app, WebView2 integration, Lucia agents, model router, policies, persistence
├── ui/              Browser and Lucia interface (HTML, CSS, JavaScript, Tailwind)
├── tests/           Rust/JS contracts, fixtures, native Windows E2E and live-site runners
├── logs/            Diagnostic channel and privacy documentation
├── .github/         CI and release workflows
├── dist/            Locally generated release outputs (when built)
└── README.md        Project overview and getting started
~~~

SQLite (WAL mode) stores browsing and agent data locally. Credentials are kept outside ordinary config files using Windows Credential Manager. Diagnostic JSONL channels are stored under `%LOCALAPPDATA%\BrowsphereAI\logs\`, with rotation and redaction; manual exports are not uploaded automatically. Logs and prior agent history may still contain operational metadata—review before sharing. See [diagnostics](logs/README.md).

## Known limitations and release gates

- **Autonomy:** repeated **real hosted-model** plan → act → recover → verify workflows must pass on multiple structurally different public websites. Navigation-only and simulated-model results do not satisfy this gate.
- **Providers:** service availability, key permissions, rate limits, malformed model actions, and latency can stop tasks. No implicit quota bypass or undocumented provider fallback.
- **Web compatibility:** complex SPAs, embedded frames, authentication/popup flows, file uploads, accessibility, and site-specific controls need broader live testing. Extension hosting is not available.
- **Safety and privacy:** continue testing site permissions, user approvals, history/private-mode isolation, redaction and prompt-injection resistance. Evidence checks verify correspondence to observed pages, not the independent truth of page content.
- **Shipping:** complete fresh installer/upgrade tests, Windows CI, signing, security review, and release validation. Signed auto-updates are not yet enabled.

## Project documents

- [Lucia production and security gates](LUCIA_PRODUCTION_GATE.md)
- [OpenCode free-model test scope](OPENCODE_LUCIA_TESTING.md)
- [Windows desktop E2E report](tests/e2e/REPORT.md)
- [Voice agent](VOICE_AGENT.md)
- [AWS connection](AWS_INTEGRATION.md)
- [Windows release signing](RELEASE_SIGNING.md)
- [Local logs and Lucia diagnostics](logs/README.md)

## License

[MIT](LICENSE) © 2026 HIMANEESH REDDY K.
