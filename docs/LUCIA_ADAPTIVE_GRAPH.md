# Lucia adaptive workflow graph

The browser and explicit MCP workflow executors share a bounded in-memory dependency graph. Nodes carry local objective/criterion metadata, lifecycle state, attempts and typed receipts. Only native execution paths record receipts after existing checks. The graph cannot grant permissions or dispatch tools.

- Independent ready MCP calls may run concurrently; failed prerequisite checks block dependants.
- A terminal MCP-to-browser handoff expands into browser stages and a final verification node, preserving earlier receipts.
- Read-only stalled browser stages may insert up to a bounded number of recovery prerequisites; a successful observation or existing evidence gate resumes the suspended stage.
- Writer completion requires exact editor readback.
- The UI displays bounded node kinds, dependencies and states, without raw page data or objective text.
- Recovery is run-scoped; durable cross-session resume and arbitrary model-driven DAG rewrites are not implemented. Explicit MCP authorization and existing action policies remain mandatory.

Run the graph and workflow tests with cargo test -p browsphere-core --offline adaptive_graph and cargo test -p browsphere-core --offline agent::workflow. Run node --test tests/lucia-activity.test.cjs for the UI graph contract.
