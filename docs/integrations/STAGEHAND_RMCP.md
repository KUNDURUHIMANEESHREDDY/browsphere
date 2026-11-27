# Stagehand + official MCP SDK: experimental, upstream-first integration

**Status:** Source bridge and independent Windows acceptance tests added on an
isolated branch. A green Windows CI report is required before claiming SDK
interoperability. **No native installed-WebView2 task success has been established.**

## Reused upstream code, not rebuilt by Browsphere

- `@browserbasehq/stagehand@4.5.0` (MIT): browser launch, element
  observation, structured extraction. Browsphere provides only URL policy,
  invocation, and serialization glue.
- `@modelcontextprotocol/server@2.4.0` and
  `@modelcontextprotocol/node@2.1.2`: official TypeScript MCP protocol
  implementation for the isolated Stagehand server.
- `rmcp@3.12.0`: official Rust MCP SDK, currently exercised by the
  standalone `integrations/rmcp-probe` client for protocol negotiation and
  tools/list. It is **not** yet the transport inside the installed browser.

No production executable bundles these Node packages or launches the server.
The existing `src-tauri/src/agent/mcp.rs` read-only permission gate is still
the only Lucia MCP execution path; its custom HTTP transport remains pending
migration. No unapproved tool can be called through Lucia.

## Local developer invocation (isolated research only)

Install Node 22.18+ and an installed local Chromium browser. In
`integrations/stagehand-mcp`, run `npm install`, `npm test`,
`npm run check`, and `node stagehand.smoke.mjs`. The smoke test launches an isolated
ephemeral Chromium session; it does not attach to Browsphere or an account.

Set `BROWSPHERE_STAGEHAND_ENABLE=1` explicitly to permit the loopback-only
MCP sidecar to start, then execute `npm start`. It exposes
`http://127.0.0.1:18888/mcp` only. No cloud API key is required; extraction
and observation use the configured local Ollama model by default.

**Do not start this service while browsing authenticated/private websites.**
The read-only source allowlist and isolated profile reduce exposure, but the
sidecar has **not** been independently security-audited for navigations or
subresources. Never route secret-bearing URLs, credentials, or user profiles
through the research sidecar.

An example *explicitly user-authorized* `mcp-servers.json` entry is:

```json
{
  "stagehand": {
    "url": "http://127.0.0.1:18888/mcp",
    "read_only_tools": [
      "stagehand_inspect",
      "stagehand_observe",
      "stagehand_extract"
    ]
  }
}
```

Use Browsphere's trusted MCP settings to discover the exact server and grant
only the required read-only tools. The sidecar cannot grant itself permissions.
Example Lucia request after authorization:

```text
mcp call stagehand/stagehand_inspect {"url":"https://www.w3.org/WAI/","instruction":"Read the page title"}
```

The result is **not task completion**: Stagehand extraction is untrusted
source content. Lucia must independently retrieve and verify source evidence
and final browser state. `stagehand_observe` proposes actions; it must never
directly dispatch model-generated actions against the private WebView2 profile.

## Acceptance tests and outstanding gates

The experimental Windows workflow does four independent checks:
Node policy/contract tests, real isolated Stagehand Chrome inspection,
`rmcp` Rust SDK compilation, and official SDK `tools/list` against the
live Stagehand sidecar. It keeps failed checks red and does not convert them
into success signals.

Outstanding before production adoption:

1. Commit a reviewed, integrity-pinned npm lockfile (the experimental job
   currently resolves versions and retains its lock as an artifact).
2. Replace the native custom MCP transport with `rmcp` *without* losing
   secure token lookup, redirect/SSRF restrictions, payload bounds, sessions,
   cancellation, OAuth, user allowlists or independent readback.
3. Prove Stagehand compatibility with WebView2 under an ephemeral Windows test
   profile **without** exposing CDP to consumer releases.
4. Compare Stagehand against Lucia using independent BrowserGym and Browser Use
   evaluations. Require repeated unfamiliar-site successes, not valid JSON.
5. Ship and measure clean Windows NSIS installs, p50/p95/p99 resource metrics,
   update/signing configuration and end-to-end installed-app verification.

**Prior real-Qwen baseline: 0/6 independently verified unfamiliar-site tasks.**
No change in this branch establishes an improved completion rate.
