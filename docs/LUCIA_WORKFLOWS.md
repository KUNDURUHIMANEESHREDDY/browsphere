# Lucia dependency-aware workflows

Use the existing trusted server allowlist in %LOCALAPPDATA%/BrowsphereAI/mcp-servers.json.
This opt-in graph supports up to eight steps, with explicit dependencies, typed
scalar references, deterministic value verification and cross-endpoint comparison.
It is sequential by design, not a parallel task executor.

Enter this single-line task in Lucia (example assumes two approved lookup servers):
    mcp workflow {"steps":[{"id":"a","action":{"type":"auto","servers":["one"],"task":"look up alpha","expect":{"pointer":"/structuredContent/value","equals":"alpha-result"}}},{"id":"b","depends_on":["a"],"action":{"type":"call","server":"two","tool":"lookup","arguments":{"key":{"step":"a","pointer":"/structuredContent/value"}},"expect":{"pointer":"/structuredContent/value","equals":"alpha-result"}}},{"id":"check","depends_on":["a","b"],"action":{"type":"compare","left":{"step":"a","pointer":"/structuredContent/value"},"right":{"step":"b","pointer":"/structuredContent/value"}}},{"id":"web","depends_on":["a","b","check"],"action":{"type":"browser","task":"Open https://example.org"}}]}

- call: explicit configured read-only tool with primitive literal or referenced arguments.
- auto: local specialist model selects an approved tool from one to three named servers.
  A confirmed pre-dispatch no-match permits trying another listed server.
- compare: check exact equality of observed scalar values from distinct configured
  server endpoints. Agreement is not proof that both sources are accurate or independent.
- browser: at most one terminal user-specified browser task, after every prior step.

Every tool step must specify a concrete expected scalar and JSON pointer.
References only resolve prior declared dependencies; cross-tool data transfer must
be explicitly specified in the user-authored graph. No tool output is injected into
the model prompt or browser goal. Malformed graphs, cycles, missing tools, unexpected
results and uncertain dispatch stop the run. No automatic retry after dispatch.
Pause is checked before steps and immediately before tool dispatch. A running tool
cannot necessarily be interrupted at the server. The workflow is disabled in private
browsing. MCP permissions and existing browser policy checks remain applicable.

A successful terminal handoff is not a completed browser task; the native browser
loop must establish its own success. Graph-level natural language synthesis and
arbitrary interleaving of browser steps with MCP steps are not yet implemented.

Tests:
    cargo test -p browsphere-core agent::workflow --no-default-features
    cargo test -p browsphere-core live_model_multi_server_workflow -- --ignored

Integration fixtures use two separate loopback HTTP servers, validate passed arguments,
and assert that failed verification prevents dependent dispatch. The model test needs
local Ollama with lfm3:1.7b. Browser handoff is tested at the workflow boundary;
full WebView UI execution is a separate validation gate.
## Advanced orchestration

Use `mcp plan <natural language objective>` to draft a validated JSON workflow
for review. It does not call any tools or execute the graph. Submit the reviewed
`mcp workflow ...` separately. The planner currently uses configured approved
names, not live schema discovery; correct all proposed fields and expected values.

An auto step may specify a user-approved fallback_task. It is tried only after
all listed servers confirm no matching tool and before any tool dispatch.
Two independent explicit calls to distinct configured endpoints may run in
parallel (maximum two); the scheduler joins them before further dispatch.

An observe step accepts a literal HTTPS URL with no credentials or query and
an exact expected title. It can occur between MCP steps. Native browser policy
and approval checks still apply, and a fresh snapshot must match both fields.
Full arbitrary multi-action browser steps are still terminal handoffs.

A review step names a declared tool-result dependency via source (step and
JSON pointer) and a user-defined criterion. A local model must return a positive
verdict and an exact quote from the text. This is a heuristic relevance check,
not proof of factual correctness, and does not replace deterministic checks.

Tests:
    cargo test -p browsphere-core agent::workflow --no-default-features
    cargo test -p browsphere-core live_planner_and_semantic_review -- --ignored
