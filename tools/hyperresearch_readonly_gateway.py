"""Opt-in loopback MCP adapter for *reading* a dedicated Hyperresearch vault.

Not a general research runner. Does not expose fetch_url/create_note/update_note.
Start manually with --vault; never auto-launch from an untrusted webpage.
Requires hyperresearch 0.13.3 and MCP Python SDK 1.x (>=1.8).
"""
from __future__ import annotations

import argparse
import json
import os
import re
from pathlib import Path

from mcp.server.fastmcp import FastMCP
from mcp.types import ToolAnnotations
from hyperresearch.mcp import server as upstream

from hyperresearch_gateway_core import (
    note_id, note_result, public_source, search_results, source_check, sources_result,
)

READ_ONLY = ToolAnnotations(
    readOnlyHint=True, destructiveHint=False, idempotentHint=True, openWorldHint=False,
)
# Fixed loopback endpoint; the process is never network-accessible.
mcp = FastMCP("lucia-hyperresearch-readonly", host="127.0.0.1",
              port=8765, streamable_http_path="/mcp")


@mcp.tool(annotations=READ_ONLY)
def search_notes(query: str, limit: int = 5) -> str:
    """Search an explicitly configured local research vault (unverified hints)."""
    if not isinstance(query, str) or not 2 <= len(query.strip()) <= 200:
        raise ValueError("Search must be between 2 and 200 characters")
    return search_results(upstream.search_notes(query=query.strip(), limit=max(1, min(limit, 8))))


@mcp.tool(annotations=READ_ONLY)
def read_note(note_id_value: str) -> str:
    """Read a bounded excerpt of a known note; not proof of a live web fact."""
    return note_result(upstream.read_note(note_id(note_id_value)))


@mcp.tool(annotations=READ_ONLY)
def check_source(url: str) -> str:
    """Check whether a public URL is already cached; no network fetch."""
    if public_source(url) is None:
        raise ValueError("Expected a public HTTP(S) URL without credentials, query or fragment")
    return source_check(upstream.check_source(url))


@mcp.tool(annotations=READ_ONLY)
def list_sources(domain: str = "", limit: int = 10) -> str:
    """List cached source metadata; no source bodies or external requests."""
    if not isinstance(domain, str) or len(domain) > 150 or (
        domain and not re.fullmatch(r"[A-Za-z0-9.-]+", domain)
    ):
        raise ValueError("Invalid domain filter")
    return sources_result(upstream.list_sources(domain=domain, limit=max(1, min(limit, 20))))


@mcp.tool(annotations=READ_ONLY)
def vault_status() -> str:
    """Report bounded local vault counts without revealing note bodies."""
    raw = json.loads(upstream.vault_status())
    if not isinstance(raw, dict):
        raise ValueError("Unexpected vault status")
    return json.dumps({
        "total_notes": raw.get("total_notes", 0),
        "total_links": raw.get("total_links", 0),
        "broken_links": raw.get("broken_links", 0),
        "trusted": False,
    })


def main() -> None:
    parser = argparse.ArgumentParser(description="Opt-in, loopback-only Lucia research bridge")
    parser.add_argument("--vault", required=True, help="Dedicated non-sensitive Hyperresearch vault")
    args = parser.parse_args()
    root = Path(args.vault).expanduser().resolve()
    if not (root / ".hyperresearch" / "config.toml").is_file():
        parser.error("Vault must already contain .hyperresearch/config.toml")
    # Hyperresearch discovers its vault from the current working directory.
    os.chdir(root)
    mcp.run(transport="streamable-http")


if __name__ == "__main__":
    main()
