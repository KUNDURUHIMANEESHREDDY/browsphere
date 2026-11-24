# Local model role orchestration (v7)

## Configuration

In Settings > AI & ML > Lucia local profiles, select Role-based local team. The profile requires all four model IDs already installed and keeps provider credentials and domain overrides unchanged. Qwen3.5 0.8B remains the action model; Qwen2.5 3B Instruct is the planner and preferred factual reviewer; LFM3 1.7B drafts decomposition and supervises; LFM3-VL 2B provides advisory visual observations. The bundled three-model service uses its existing LFM3 planner unless a separate compatible model is configured. No automatic model download is performed.

For complex tasks with a separate planner configured, a bounded LFM3 decomposition is validated and passed as untrusted advisory data to the planner alongside the unchanged original goal. Rust validates the final stage plan, appends independent verification, and retains all native approval gates. Recognized deterministic skills need not call the planner. A missing or invalid draft can be skipped; it never authorizes a browser action. Local secondary model routing respects cloud-only and domain settings.

## Real local inference benchmark

Run `python tests/e2e/benchmark-local-model-roles.py --matrix --output <path>` with local Ollama running. These are synthetic structured-response fixtures, not live website tasks. No external websites, user screenshots, personal data, or model credentials were used. One run on the installed models completed 10 of 19 probes; assigned roles completed 5 of 7.

| Text model | Planning | Decomposition | Action (2 cases) | Supervision (2 cases) | Total |
|---|---:|---:|---:|---:|---:|
| Qwen2.5 3B Instruct | 1/1 | 1/1 | 1/2 | 1/2 | 4/6 |
| Qwen3.5 0.8B | 1/1 | 0/1 | 1/2 | 0/2 | 2/6 |
| LFM3 1.7B | 1/1 | 1/1 | 0/2 | 1/2 | 3/6 |

LFM3-VL 2B identified the search control and empty results state in a synthetic image (1/1, 9.20 s). All three text models failed the empty-search fixture and the repeated-action diagnosis fixture. Qwen3.5 incorrectly proposed installing a package in a read-only decomposition fixture; this was a rejected inference, not an executed action. Latencies are end-to-end single-run measurements including model loading, not stable throughput estimates. These results are insufficient to establish an optimal model assignment or prove live browser reliability. Keep simple skills native and independently verify every required result.
