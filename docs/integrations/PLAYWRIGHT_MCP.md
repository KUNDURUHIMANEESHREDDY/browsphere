# Lucia: upstream Playwright MCP adapter (opt-in)

**Upstream:** Microsoft Playwright MCP, https://github.com/microsoft/playwright-mcp (Apache-2.0).
**Pinned adapter executable:** @playwright/mcp@0.0.86.
**WebView2 CDP documentation:** https://playwright.dev/docs/webview2

## What Lucia reuses

The upstream server implements accessibility snapshots; we did not rebuild a
browser accessibility engine or a new agent planner. Lucia already has a
user-approved native MCP client and a stage-aware Rust specialist planner.

For initial complex planning on pages that Lucia natively classifies as
Unknown, Dialog or Autocomplete, the existing planner now **optionally**
queries a separately configured, trusted, loopback Microsoft Playwright MCP
server through its read-only browser_snapshot tool.

The response must identify exactly the same URL as the currently observed
native WebView2 task tab. Lucia extracts only bounded accessibility role
counts (dialogs, searchboxes, comboboxes, options, buttons and links), and
passes those counts to the existing planner with the native page profile.
It discards raw snapshots, labels, URLs and page content. The normal native
action-policy, DOM binding, user permissions and independently checked final
results remain authoritative.

The MCP call times out after 2.5 seconds and falls back to normal native
planning. It never runs in Private Browsing, cannot launch tools suggested by
websites and cannot promote external snapshots into completion evidence.

## Windows setup — disposable test profile only

1. Start a separate Browsphere WebView2 test instance with a fresh, isolated
   user data directory and a loopback CDP port (for example 9222). Use the
   repo's existing native E2E launch scripts as a reference. Never enable
   debugging on your personal browsing profile.
2. From the Browsphere checkout, verify the local CDP endpoint:

       ./scripts/integrations/start-playwright-mcp.ps1 -CdpPort 9222 -VerifyOnly

3. Run the reviewed pinned upstream package in the foreground:

       ./scripts/integrations/start-playwright-mcp.ps1 -CdpPort 9222 -McpPort 8931 -AcknowledgeCdpAccess

   Requires Node.js and npm. On first use, npx can download the pinned
   external package. This script does not modify the Browsphere installer,
   spawn CDP by itself, or write your saved MCP configuration.
4. In Settings > AI & ML > MCP Tools, connect to
   http://127.0.0.1:8931/mcp and approve **only** browser_snapshot
   for the named server playwright. The native MCP settings connector
   can add/revoke the exact approved tool. Equivalent object inside the
   existing %LOCALAPPDATA%/BrowsphereAI/mcp-servers.json is:

       {
         "playwright": {
           "url": "http://127.0.0.1:8931/mcp",
           "read_only_tools": ["browser_snapshot"]
         }
       }

   Merge this object into an existing config; do not overwrite other servers.
5. Test the explicit chat commands:

       mcp list playwright
       mcp call playwright/browser_snapshot {}

   Then run a multi-stage task through Lucia's chatbox on an isolated local
   fixture whose initial page is Unknown, Dialog or Autocomplete. The final
   answer must still pass native evidence verification. If the external
   server is not available, behavior must remain unchanged.

## Safety, trust and limitations

- Microsoft Playwright MCP is **not sandboxed**. Although Lucia only permits
  an explicitly approved read-only snapshot, the separate MCP process has
  complete CDP control of the attached browser and other local processes
  could connect to the MCP port. Bind to 127.0.0.1 and use an ephemeral
  test profile without credentials.
- Never give a model or webpage permission to start the server, replace the
  config, choose a CDP endpoint, or invoke browser_click/browser_type/code.
- A different-tab response, missing Page Info header, mismatched query URL,
  malicious/untrusted content, or excessive output is ignored; the
  native WebView2 snapshot is always the source for allowed actions.
- This is an optional **perception/initial-planning integration**, not
  autonomous Playwright action execution. It does not prove live task
  success, resolve real-model inference timeouts, or make Browser MCP safe
  for a consumer installer.
- Do not bundle or auto-enable the upstream executable in a release without
  separate supply-chain pin/signing, Windows security review, performance
  evidence and clean-machine validation.

## Test gates

    cargo test --workspace --locked upstream_playwright
    cargo test --workspace --locked native_page_planner

Run a fresh Windows Tauri/NSIS build and repeated real-model chatbox tasks
with independently checked outcomes. A passing Rust test or installer build
must not be interpreted as a passing live browser task.
