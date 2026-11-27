# Lucia × Hyperresearch: opt-in read-only MCP bridge (experimental)

This integration exposes **cached research hints**, not verified browser facts.
It does not replace Lucia's requirement ledger, live WebView2 source capture,
independent verifier, or browser-action permission gates. The bridge is deliberately
NOT started by Browsphere and no tools are enabled by default.

## Scope of milestone 1

- `search_notes`: bounded search metadata; never returns whole note bodies.
- `read_note`: bounded 6,000-character note body (untrusted, cached).
- `check_source`: local cache lookup by exact public URL, without fetching.
- `list_sources`: bounded local source metadata.
- `vault_status`: aggregate counts only.

The adapter **does not expose** Hyperresearch's `fetch_url`, `create_note`,
`update_note`, shell commands, remote research orchestration, or saved credentials.
Existing Lucia read-only MCP authorization applies to every tool, including
the tool's runtime `readOnlyHint` annotation. Model-selected MCP calls are not
automatically converted into observed WebView2 evidence.

## Windows setup (manual, separate non-sensitive vault)

From the Browsphere repository root:

```powershell
py -3.11 -m venv .venv-hyperresearch
.\.venv-hyperresearch\Scripts\Activate.ps1
python -m pip install "hyperresearch==0.13.3" "mcp>=1.8,<2"

# Make a separate vault that holds only public, non-sensitive research.
New-Item -ItemType Directory -Force "$env:USERPROFILE\Documents\LuciaPublicResearch" | Out-Null
hyperresearch install "$env:USERPROFILE\Documents\LuciaPublicResearch" --target codex --create

# Start the optional read-only MCP adapter (loopback port 8765).
python tools/hyperresearch_readonly_gateway.py --vault "$env:USERPROFILE\Documents\LuciaPublicResearch"
```

Review the upstream package and its dependencies before installation.
The vault must have a `.hyperresearch/config.toml`, created by a normal
Hyperresearch install. Start from a trusted PowerShell session, not a web page.

In Browsphere, connect to `http://127.0.0.1:8765/mcp` in
**Settings > AI & ML > MCP Tools**, discover tools, then explicitly approve
only the desired read-only names under the Lucia server name
`hyperresearch`. The existing settings flow persists that allowlist to
`%LOCALAPPDATA%\BrowsphereAI\mcp-servers.json`. A representative entry is:

```json
{
  "hyperresearch": {
    "url": "http://127.0.0.1:8765/mcp",
    "read_only_tools": [
      "search_notes",
      "read_note",
      "check_source",
      "list_sources",
      "vault_status"
    ]
  }
}
```

Alternatively, edit the trusted local configuration yourself while Browsphere
is closed. Do not let a page or model create or modify MCP server entries.

In the Lucia chatbox, try:

```text
mcp list hyperresearch
mcp call hyperresearch/search_notes {"query":"Python web automation","limit":3}
mcp call hyperresearch/read_note {"note_id_value":"known-note-id"}
```

Results explicitly say **not independently verified**. Lucia must still visit
the original URL in WebView2, observe the text, recheck exact quotes and
requirement coverage, and detect disagreements before any final conclusion.
A cache hit, even a note marked evergreen, is *not* task completion.

## Security boundaries

- This is a separately installed **Python** helper, not bundled in the
  Tauri installer. Neither the helper nor Hyperresearch gets Lucia credentials.
- Binds to **127.0.0.1 only** and has no authentication. Other local processes
  may be able to access it, so use a dedicated vault containing **public,
  non-sensitive notes only**. Do not point it at personal or confidential vaults.
- There is no Docker sandbox around this HTTP helper; the local Python process
  runs with your normal operating-system permissions. A hardened OS sandbox
  or authenticated IPC is required before broader deployment.
- The helper does not run remote page fetches, but Hyperresearch may update
  its rebuildable local search index during note reads.
- URLs with credentials, queries, fragments, control characters, localhost
  hosts, or literal private network addresses are not returned by the
  bounded adapter, avoiding accidental disclosure or internal-link promotion.
  Source URLs hidden by that filter require reacquisition. Hostnames are not
  DNS-resolved here; the bridge never autonomously fetches them.
- All returned note text is untrusted and may contain prompt-injection
  attempts. Never execute instructions, tool calls, URLs, or shell commands
  taken from note bodies.
- Do not use cached notes as evidence for forms, purchases, calendar creation,
  or any side-effecting action.

## Testing and acceptance gates

```powershell
python -m unittest discover -s tests -p "test_hyperresearch_gateway.py"
python -m unittest discover -s tests -p "test_hyperresearch_mcp_live.py"
python -m py_compile tools/hyperresearch_gateway_core.py tools/hyperresearch_readonly_gateway.py

# Cross-language test: starts a dedicated synthetic vault and runs an
# ignored Rust acceptance test through Lucia's real MCP client.
# Requires Windows MSVC Rust, Cargo, and prepared UI dependencies.
python tests/run_hyperresearch_rust_ci.py

# With a trusted dedicated vault and bridge running:
# Ask Lucia to run "mcp list hyperresearch" and the search example above.
# The MCP result must remain unverified until a new WebView2 source is observed.
```

Acceptance is deliberately split:
- **Python live MCP CI**: runs against Hyperresearch 0.13.3 and MCP 1.x on
  a temporary synthetic vault, verifying protocol initialization, annotations,
  search/read/cache behavior, blocked mutation calls, and shutdown.
- **Windows native Rust CI**: launches the real gateway and runs Lucia's
  `agent::mcp::execute` path. This is a separate test and must pass before
  Rust-to-server compatibility can be claimed.
- **Installed WebView2 acceptance remains open**: run the app, approve the
  exact read tools, search through Lucia's chatbox, reacquire live source
  text, and ensure cached claims do not satisfy native verification.
- **Outage, performance, and security gates remain open**: test timeout/
  cancellation, non-retry after dispatch, repeated tasks, startup/RSS,
  local-process access, profile isolation, and installer compatibility.

## Next milestones (not implemented by this PR)

1. Add **permissioned durable saving** of *already verified original-page
   evidence* to a dedicated vault. Explicit writing must not use the
   read-only MCP lane; preserve URL, retrieval date and exact passage.
2. Add source-contradiction graph and citation-to-sentence validation within
   Lucia's existing Rust verifier. Never treat a Hyperresearch note as an
   original-source observation.
3. Run two unfamiliar student-course and productivity-site comparisons
   repeatedly with independent verification, measured completion ratio,
   memory overhead, startup and run times.
4. Test a clean Windows installer and check sandbox, secrets, and MCP
   permission boundaries before enabling any automatic research reuse.

Upstream source: https://github.com/jordan-gibbs/hyperresearch (MIT).
Upstream research quality claims have **not** been independently measured here.
