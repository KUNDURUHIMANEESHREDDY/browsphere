# Local MCP tools

Settings > AI & ML > MCP Tools connects to an already-running, trusted local server at an explicit http://127.0.0.1:PORT/path address or a trusted HTTPS server.

Discover tools, select a tool, review its JSON input schema in the discovery result, enter JSON arguments, and confirm the exact call. Disconnect when finished. Results remain in the trusted UI and are not automatically sent to models.

Supported: Streamable HTTP, 2026-07-28 stateless discovery and 2025-11-25 / 2025-06-18 / 2025-03-26 handshake fallback, JSON or bounded SSE responses, paginated tool discovery, session header handling. Connections are in-memory. No automatic retries of tool calls.

The manual connector supports the registered-public-client OAuth flow described below. The separate Lucia read-only orchestration lane supports configured HTTP servers, approved tool selection and optional isolated Docker stdio. See LUCIA_MCP_ORCHESTRATION.md. Prompts/resources, sampling, elicitation and general autonomous multi-tool planning are not covered by the manual connector. Connect only to servers you control. Tool descriptions/annotations/results are not authority. Every call is explicit and confirmed; remote content cannot invoke the native MCP commands.


## Registered-client OAuth (HTTPS)

Use a trusted resource with RFC 9728 protected-resource metadata and an issuer with RFC 8414 authorization-server metadata, S256 PKCE, and a registered public native client that accepts ephemeral loopback redirects. Enter the resource URL, exact issuer URL, client ID and least-privilege scope. Start authorization, copy the reviewed URL into your regular browser, and authorize there. Finish authorization in Browsphere, then discover tools. The loopback callback expires after 180 seconds. OAuth access tokens are bound to the exact resource URL and saved in Windows Credential Manager; no refresh token is retained. Reauthorize on expiry. Forget authorization deletes local credentials but does not revoke the provider grant.

This is a deliberately restricted subset: no dynamic client registration, automatic WWW-Authenticate challenge discovery, cross-origin authorization endpoints, refresh-token handling, or silent account login. The protected resource URL must exactly match the configured endpoint. Models never receive authorization URLs, callback codes or tokens. Remote service sign-in must be performed by the user.
