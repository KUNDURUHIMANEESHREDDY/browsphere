# Lucia: one native orchestration loop, interchangeable model providers

Lucia's **production browser task runtime** is the Rust/Tauri 2/WebView2
`src-tauri/src/agent/loop_runner.rs` loop. It owns stages, fresh DOM
observations, retries, recovery budgets, untrusted-page boundaries, native
browser actions, user permission checks and independent evidence verification.

There is no per-model browser orchestrator in the installed application.
`ModelRouter::propose_specialist_action` switches only the **inference
transport**:

- Ollama `/api/generate` with one schema-constrained decode attempt, then
  one JSON-mode compatibility retry when necessary.
- Bedrock Converse as one action proposal.
- OpenAI-compatible endpoints (including Groq and LiteLLM) with their
  supported JSON request format and a bounded retry.

All three receive the **same compact observed-DOM action contract** and
their output goes through `validated_action_from_contract`. It verifies
the native allowed action types, observed request-local control aliases,
bounded parameters and completion eligibility before yielding an action.
Only then does the independent Rust DOM/action policy and verifier decide
whether to execute or finish. Models cannot authorize actions or change
the task success criteria.

This is *model-agnostic orchestration*, **not universal model reliability**.
A model must still be capable of obeying JSON/structured instructions,
reasoning about the observed page and responding inside resource and time
budgets. A provider returning invalid JSON, unsupported structured format or
empty output fails closed; it never bypasses security or invents evidence.
Models are not switched to cloud silently after local inference fails.

`integrations/stagehand-mcp` and `integrations/browser-use-qwen` are
isolated **upstream comparisons and optional read-only capabilities**, not
additional browser orchestrators enabled in the Tauri release. Browser Use
1.x permits model-generated Python, so its benchmark must remain on an
ephemeral credential-free runner until isolation is independently validated.

## Acceptance gates

1. Verify the shared Rust action schema/alias test and full Cargo tests on
   Windows. Run independent local and OpenAI-compatible mock E2E trials.
2. Build the signed/reproducible Windows installer and test native WebView2
   execution and evidence readback on a clean runner.
3. Execute genuine task trials with each *configured* model/provider on
   unfamiliar websites, at least three repeats per task, and record task
   completion and independent source matching; never infer task success from
   a passing build or one navigation.
4. Measure timeouts, prompt processing, RAM and process-tree resources. A
   model failing one of these checks is **not** certified as supported
   regardless of the common orchestration API.

No measured real-model success rate is claimed from this refactor alone.
