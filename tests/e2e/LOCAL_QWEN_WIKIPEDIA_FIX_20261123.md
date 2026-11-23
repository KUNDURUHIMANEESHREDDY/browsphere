# Lucia local Qwen Wikipedia search fix — 23 November 2026

## Reproduced root cause

The actual Windows WebView2 snapshot of `https://en.wikipedia.org/wiki/Main_Page` exposed a visible link labeled `Search Wikipedia [f]` with `tag=a`, `is_submit=false`. Ollama Qwen 2.5 3B selected a click on that link but marked it as a submission. Lucia's old binder used `*is_submit |= node.is_submit`, allowing the model-supplied `true` to override the native DOM's `false`. Lucia therefore applied the **form-submission** postcondition to a **search-interface opener** and halted when no form-success navigation was observed. Another run reached `/w/index.php?search=&title=Special%3ASearch` because the query had never been entered. The verifier correctly refused to call that success, but too late.

## Implemented corrections

- Treat the freshly observed DOM as authoritative for `is_submit` (both true and false), while retaining independent browser policy and approval checks.
- In narrow read-only named article-title searches, type the exact user-supplied query only into one uniquely observed, non-sensitive search field; don't guess a node ID, URL, or article result, and don't select ambiguous or sensitive fields.
- Native WebView2 click and Enter execution refuse blank search-form submissions before dispatch. Normal search-openers with no form remain clickable.
- Preserve the duplicate-query recovery and result-verification safeguards: a repeated local-model typing proposal can use an unambiguous observed Search control, and only the exact observed destination and independent evidence can certify completion.
- Typed field values are not placed in stored trace text. Sensitive and consequential form approval policies remain enforced.

## Verification

- `cargo test --workspace --locked --quiet`: **146 passed, 5 ignored, 0 failed**.
- `node --test tests/*.test.cjs`: **30 passed, 0 failed**.
- Native Windows debug build: **passed**.
- Real installed local Ollama model: `qwen2.5:3b-instruct` via `http://127.0.0.1:11434` (no hosted model or mocked action inference).
- Real Wikipedia article task, isolated WebView2 profile, two runs (CDP ports 9689 and 9693): **2/2 passed**. The browser opened `https://en.wikipedia.org/wiki/Ada_Lovelace`, observed title `Ada Lovelace - Wikipedia`, recorded Qwen-selected search-interface/result actions, one successful trusted exact-query type action, and a final independent read-only verification report.
- A model-selected click can fail without the overall run being counted successful prematurely: the native page state and independent verifier determine the final outcome.

## Scope

This establishes the **specific Wikipedia search → article** workflow, not universal autonomous browsing across every public website. The Windows release executable currently open on the user's computer must be rebuilt/reinstalled before it receives the patched Rust logic; source push and CI build do not update an active process.
