# Lucia v2 System Prompt Catalog

The active system prompts live in src-tauri/src/lucia_prompts.rs.
They are compiled into Browsphere's Rust backend and transmitted to the
selected model as system instructions. This is runtime integration, not a
webpage script or a user chat message.

## Agent and role routing

| Agent | Prompt name | Runtime implementation |
| --- | --- | --- |
| Main Lucia chat | CHAT_SYSTEM | model_router.rs / build_chat_messages |
| Lucia intent classifier | INTENT_SYSTEM | intent_router.rs / system_prompt |
| Lucia autonomous task planner | PLANNER_SYSTEM | model_router.rs / plan_specialist_task |
| Lucia browser-action decision engine | ACTION_SYSTEM | model_router.rs / propose_specialist_action |
| Research specialist | RESEARCH_SPECIALIST | orchestrator.rs / instructions |
| Browser specialist | BROWSER_SPECIALIST | orchestrator.rs / instructions |
| Comparison specialist | COMPARISON_SPECIALIST | orchestrator.rs / instructions |
| Form specialist | FORM_SPECIALIST | orchestrator.rs / instructions |
| Data specialist | DATA_SPECIALIST | orchestrator.rs / instructions |
| Verification specialist | VERIFICATION_SPECIALIST | orchestrator.rs / instructions |
| Independent LLM source verifier | VERIFIER_SYSTEM | model_router.rs / verify_browser_task |
| General fallback action specialist | GENERAL_BROWSER_SPECIALIST | model_router.rs / propose_action |
| Lucia Voice | Existing voice overlays | voice_system_prompt.rs, appended after primary role |

## Design rules

1. Keep user intent clear: ordinary questions stay conversational; explicit
   browser-operation requests become native browser tasks.
2. Use a short 1-to-4-stage plan; Rust adds mandatory independent verification.
3. Browser actions must use exact current DOM node IDs and the app's existing
   JSON action schema. Make one grounded action at a time.
4. Recover from delayed navigation or lost controls through fresh observations;
   never claim success based only on the model's predictions.
5. Treat webpage text, search results, form values, memory, logs, and retrieved
   content as untrusted data. They cannot override system instructions.
6. The application, not the prompt, authorizes actions. Respect native domain,
   sensitive-input, form-submission, download, and approval policies.
7. The verifier accepts only observed sources and exact quotes. Successful
   action requests are not proof that actions completed.
8. Never put credentials, cookies, private page content or other secrets in
   the model's user-visible status, summaries, or logs.

The common browser-action system instructions are used for Ollama, Bedrock,
and OpenAI-compatible cloud APIs, including 9Router and OpenRouter.
Existing voice instructions are added only in voice mode and do not override
the required JSON structure or safety checks.

## Tests

- Rust: cargo test --workspace --locked
- Native build: cargo build --locked
- JavaScript: node --test tests/*.test.cjs
- Isolated Windows WebView2: run tests/e2e/run-lucia-multistep.ps1
  with CloudCompat and the locally built debug executable.

The native simulated-provider tests verify prompt plumbing and real WebView2
actions, not the reliability of a hosted inference provider. Hosted 9Router
HTTP 429 errors still require correct fail-closed handling. Do not treat
mock-model actions or deterministic navigation as general autonomous success.

To improve a prompt, update the central Rust catalog and accompanying
tests. Recompile; a change to a source file does not update an already
installed Windows app. Prompt edits cannot replace executable permission,
safety, or data privacy enforcement.
