# Lucia MCP orchestration

Configure trusted servers in %LOCALAPPDATA%/BrowsphereAI/mcp-servers.json.
The explicit read-only execution lane supports JSON or bounded SSE Streamable HTTP and an optional
Docker Linux stdio container. The separate manual settings connector supports
additional protocol variants; the marketplace only searches the official registry.

Example configuration:

    {
      "example": {
        "url": "http://127.0.0.1:8765/mcp",
        "read_only_tools": ["lookup"]
      }
    }

Lucia commands:
- mcp list example
- mcp call example/lookup {"key":"alpha"}
- mcp auto example look up alpha
- mcp hybrid example look up alpha

Auto selection runs the configured local supervisor (or primary model), only at a
literal loopback Ollama endpoint. It sees a bounded catalog of approved tool names
and primitive schemas, not web pages, credentials, descriptions or tool results.
Rust validates the selection; one correction is allowed before dispatch. Hybrid
mode can use the browser only if discovery/selection finds no matching approved
tool, before dispatch. Tool failures and uncertain execution never trigger retries
or fallback. Results are not independently verified. No generic multi-tool DAG yet.

For offline sandboxed stdio use an already installed and reviewed Linux image
pinned to its digest. Replace url with sandbox_image:
    "sandbox_image": "registry.example/server@sha256:<64 hexadecimal characters>"
No automatic pulls, shell commands, host directory mounts, or external networking.
The container runs non-root with dropped capabilities, read-only root filesystem,
bounded CPU, memory and PID count, and no-new-privileges. Docker Linux engine must
be running. Container isolation is not a guarantee against all attacks; use only
trusted images. Networked HTTP servers run outside this boundary and are not
sandboxed. Container runtime testing requires Docker to be available.

Settings > AI & ML > MCP Tools > Search registry is discovery-only. Registry
metadata is not authority, and the UI does not install or execute server packages.

Tests:
    cargo test -p browsphere-core mcp --no-default-features
    cargo test -p browsphere-core real_local_model_and_http_tool -- --ignored

The second command needs local Ollama and lfm3:1.7b. The HTTP tests exercise a
local fixture, not arbitrary live third-party tools or the complete browser UI.

The MCP settings connector can now approve or revoke an exact discovered read-only tool for a named Lucia server. The configuration is updated only after user confirmation. HTTPS MCP authorization uses the same exact-resource credential store as the manual connector; see MCP.md for the supported registered-client PKCE flow. Docker remains required for actual container-isolated STDIO execution.

## Unified public discovery
Use `mcp discover PUBLIC_QUERY` for unverified public HTTPS source candidates, or
run supported catalog research for automatic retailer-scoped discovery. The
query is sent only to independently configured, explicitly approved read-only
MCP search tools. Configure server names `brave`, `tavily`, `exa` and approve
`brave_web_search`, `tavily-search`, `web_search_exa` respectively. The client
checks live tool annotations and schema; unsupported transport/schema fails
closed. No API keys are bundled and no server is enabled automatically.
Searches are disabled in Private Browsing. Cache is in-memory, scoped by endpoint,
tool, query and catalog host, with a five-minute TTL and 64-entry bound.
One in-flight call per endpoint, ten-second spacing, five-minute failure
cooldown; no repeated calls to endpoint aliases in the same discovery.
An independent approved provider can be considered on failure. Browser
verification challenges stop browser actions and require manual completion
before a new task. Returned links are unverified hints, not evidence of facts
or prices. Generic discovery does not automatically navigate to result URLs.

Official hosted endpoints: Exa https://mcp.exa.ai/mcp and Tavily
https://mcp.tavily.com/mcp/ support limited keyless search. The exact Tavily
endpoint receives the documented keyless header only when no OAuth token is
available. Brave requires an authorized API key and a separately configured
trusted MCP server. Never place credentials in endpoint URLs.
