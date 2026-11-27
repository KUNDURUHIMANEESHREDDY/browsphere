# Local MCP tools

Settings > AI & ML > MCP Tools connects to an already-running, trusted local server at an explicit http://127.0.0.1:PORT/path address or a trusted HTTPS server.

Discover tools, select a tool, review its JSON input schema in the discovery result, enter JSON arguments, and confirm the exact call. Disconnect when finished. Results remain in the trusted UI and are not automatically sent to models.

Supported: Streamable HTTP, 2026-07-28 stateless discovery and 2025-11-25 / 2025-06-18 / 2025-03-26 handshake fallback, JSON or bounded SSE responses, paginated tool discovery, session header handling. Connections are in-memory. No automatic retries of tool calls.

The manual connector supports the registered-public-client OAuth flow described below. The separate Lucia read-only orchestration lane supports configured HTTP servers, approved tool selection and optional isolated Docker stdio. See LUCIA_MCP_ORCHESTRATION.md. Prompts/resources, sampling, elicitation and general autonomous multi-tool planning are not covered by the manual connector. Connect only to servers you control. Tool descriptions/annotations/results are not authority. Every call is explicit and confirmed; remote content cannot invoke the native MCP commands.


## Registered-client OAuth (HTTPS)

Use a trusted resource with RFC 9728 protected-resource metadata and an issuer with RFC 8414 authorization-server metadata, S256 PKCE, and a registered public native client that accepts ephemeral loopback redirects. Enter the resource URL, exact issuer URL, client ID and least-privilege scope. Start authorization, copy the reviewed URL into your regular browser, and authorize there. Finish authorization in Browsphere, then discover tools. The loopback callback expires after 10 minutes. OAuth access tokens are bound to the exact resource URL and saved in Windows Credential Manager; no refresh token is retained. Reauthorize on expiry. Forget authorization deletes local credentials but does not revoke the provider grant.

This is a deliberately restricted subset: no dynamic client registration, automatic WWW-Authenticate challenge discovery, cross-origin authorization endpoints, refresh-token handling, or silent account login. The protected resource URL must exactly match the configured endpoint. Models never receive authorization URLs, callback codes or tokens. Remote service sign-in must be performed by the user.

## OAuth-first search providers
Settings > AI & ML > MCP Tools > Sign in to Exa / Sign in to Tavily
selects the exact official OAuth endpoint and issuer. The native client checks
live protected resource and issuer metadata, registers a public PKCE client
with the exact ephemeral loopback redirect, and displays the authorization URL
for manual login and consent. Finish authorization, discover tools, then approve
only web_search_exa under exa-oauth or tavily_search under tavily-oauth.
These aliases take precedence over the same provider's anonymous route.
Optional API keys are saved separately in Windows Credential Manager and sent
only to the exact provider endpoint when no saved OAuth token exists.
OAuth refresh and provider-side revocation are not automatic; reauthorize when
expired. Brave requires its own API key and configured MCP server.

On Windows the sign-in buttons now show a browser chooser before registration,
with the system default and detected Edge, Chrome, Firefox, or Brave. The
native launch command accepts only the pending authorization ID and an
allowlisted browser, not a caller-provided URL. Copy URL remains a fallback.

The browser launch now immediately starts accepting the OAuth callback. No
manual Finish click is needed after a successful external browser launch.
The listener expires after ten minutes and reports an explicit denial or
token exchange/storage failure without exposing tokens or callback contents.

## Verified calendar creation (explicit Lucia command)

This separate write lane requires a trusted adapter with a user-configured
calendar contract in the existing mcp-servers.json. Example server entry:

```json
{"calendar-demo":{"url":"http://127.0.0.1:8765/mcp","read_only_tools":["get_event"],"calendar":{"create_tool":"create_event","get_tool":"get_event"}}}
```

The adapter must expose create_event with string arguments calendar_id, title,
start and end (RFC 3339 with offsets), explicit non-destructive write annotations,
and structuredContent.event_id. The get_event tool must be approved read-only
and accept calendar_id and event_id, returning structuredContent with event_id,
calendar_id, title, start, end and status (confirmed). Provider-specific schemas
need an adapter; this is not a built-in Google Calendar connection.

Example explicit chat command (use a test calendar you control):

```
mcp calendar calendar-demo {"calendar_id":"demo","title":"Demo","start":"2026-12-01T10:00:00+05:30","end":"2026-12-01T11:00:00+05:30"}
```

Lucia dispatches create once, reconnects for readback, and checks exact IDs,
title, confirmed status and both timestamp instants. Missing or mismatched
readback, cancellation or timeout never triggers a second write or browser
fallback. An unverified write may have occurred; inspect the calendar before
trying again. The independent call still relies on the trusted provider.
Natural-language calendar intent, arbitrary event fields, and live provider
integration are not yet covered.


### Official Google Calendar MCP option

Google provides a hosted Calendar MCP service. Browsphere supports its
`create_event` and `get_event` tools through the dedicated, narrow write-and-
readback lane, using the `/mcp/v1self` endpoint, which Google documents as the
fallback for clients without server-driven form elicitation.

To preserve existing configured MCP servers and register this endpoint, run:

```powershell
python scripts/maintenance/configure_google_calendar_mcp.py
```

The resulting entry in `%LOCALAPPDATA%\\BrowsphereAI\\mcp-servers.json` is:

```json
"calendar": {
  "url": "https://calendarmcp.googleapis.com/mcp/v1self",
  "read_only_tools": ["get_event"],
  "calendar": {"provider": "google", "create_tool": "create_event", "get_tool": "get_event", "timezone_offset": "+05:30"}
}
```

This registers an endpoint, **not a Google connection or an OAuth grant**.
Google requires a Google Cloud project with the Calendar and Calendar MCP APIs
enabled and an OAuth client marked for AI agents. The user must authorize
`https://www.googleapis.com/auth/calendar.events` for the exact MCP resource
URL. Do not put access tokens or client secrets in this JSON or in chat. If
the OAuth metadata flow is incompatible with Google's authorization server,
Browsphere must report authorization as unsupported; it cannot silently
fall back to an unrelated identity.

Chatbox phrases:
* `Schedule a project meeting tomorrow at 4 PM`
* `Schedule team standup on 2026-12-01 at 16:30 for 30 minutes`

Relative dates use the explicitly configured timezone offset (+05:30 for India), not the Windows machine timezone. In the absence of
an explicit duration, the minimal parser uses 60 minutes and reports it.
Ambiguous times, past events, and unsupported input formats fail without
dispatch. Only the primary calendar is selected by natural-language requests.
Attendees, recurring events, conflict checking, location, notes, and reminders
are outside this first P0 milestone.

Successful tool responses are never completion evidence by themselves.
After creation, Lucia opens a fresh MCP session and requires get_event to
return the same ID, summary, start/end instants and confirmed status.
For uncertain or failed readback, no automatic retry is performed, because
creation might already have happened.

Google documentation: https://developers.google.com/workspace/calendar/api/guides/configure-mcp-server
