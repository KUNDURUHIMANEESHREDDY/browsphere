# Lucia / OpenCode free-model verification

This repository includes a non-destructive **model contract test**, `tests/e2e/lucia-opencode-free-contract.cjs`.

## Usage

Install the **official OpenCode v2 CLI** (`@opencode/cli`) or point `OPENCODE_CLI_BIN` at its executable. The existing `opencode-ai` v1 package is not equivalent.

PowerShell:

```powershell
$env:OPENCODE_CLI_BIN = 'C:\path\to\opencode.exe'
$env:OPENCODE_LUCIA_MODEL = 'opencode/deepseek-v4.1-flash-free'
$env:OPENCODE_LUCIA_TIMEOUT_MS = '60000'
node tests/e2e/lucia-opencode-free-contract.cjs
```

The test uses the *official CLI*, confirms the model is in its current catalog, sends synthetic Lucia browser-decision prompts and checks the returned JSON action schema. It runs with a disposable OpenCode database, isolated configuration, denied tools and a bounded timeout. A catalog match **does not** count as a working inference result. A failed request produces a nonzero exit code.

The tested decisions cover initial website navigation and selecting an observed search-box node. This is **not** a production integration or a native WebView2 end-to-end test. It does not let the model directly operate the browser or grant access to local files.

## Service limitation

OpenCode's free-tier chat models are designated for use **inside OpenCode**, not as a general free inference API for external clients. Do not impersonate its client or reverse-proxy the free endpoint into Browsphere. OpenCode's paid/Go offerings have separately documented API access; they require their own authorization and cost controls. Free model names, availability and usage conditions can change. Only submit synthetic, non-sensitive test data to free models.

References: https://opencode.ai/docs/console/models/ and https://opencode.ai/docs/cli/commands

## Required gates before claiming Lucia + OpenCode end-to-end support

1. A live OpenCode v2 free-model request must succeed and return a valid JSON action.
2. Lucia's policy-gated executor must consume an action from the selected, authorized provider, operate a real WebView2 task tab, and observe the resulting URL and DOM.
3. A multi-step search, result click, completion check and recovery case must finish with evidence-based verification.
4. Switching away from the task tab must not stop work; closing the task tab must stop safely.
5. Free-model access must comply with the provider's terms; a test harness does not authorize using the free endpoint as a production backend.

Local regressions: `node --test tests/lucia-local-model.test.cjs` and `cargo test --workspace --quiet`. Passing these is necessary but not sufficient for real OpenCode browsing.

## Windows verification (2026-11-23)

- Existing OpenCode v1.19.5 encountered an incompatible session database and stalled during inference; user files were not reset or deleted.
- Side-by-side OpenCode v2.4.0 successfully listed free models, but a real `opencode/deepseek-v4.1-flash-free` request returned **Free usage limit reached**. The live model-contract test correctly exited nonzero.
- `node --test tests/lucia-local-model.test.cjs`: 4 passed.
- `cargo test --workspace --quiet`: 96 passed, 4 ignored, 0 failed.
- `tests/e2e/run-lucia-multistep.ps1`: passed using a synthetic loopback model in an isolated native WebView2 instance. Covers generic site search, result click, recovery after failed click, evidence checks, background tab switching, tab closure, and View task tab.

**Production status:** No OpenCode free-model inference was integrated into Browsphere or declared working. The native synthetic-model test does not override the live provider quota failure.
