"""Real Hyperresearch -> FastMCP -> SDK client integration test.

Requires pinned Hyperresearch + MCP dependencies, and runs only against a
temporary non-sensitive vault. Does not access the public web.
"""
from __future__ import annotations

import asyncio
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client
from hyperresearch.core.vault import Vault
from hyperresearch.core.note import write_note
from hyperresearch.core.sync import compute_sync_plan, execute_sync


REPO = Path(__file__).resolve().parents[1]
GATEWAY = REPO / "tools" / "hyperresearch_readonly_gateway.py"
URL = "http://127.0.0.1:8765/mcp"


async def connect_and_verify() -> None:
    async with streamablehttp_client(URL, timeout=2) as (reader, writer, _):
        async with ClientSession(reader, writer) as client:
            await client.initialize()
            tools = (await client.list_tools()).tools
            expected = {"search_notes", "read_note", "check_source",
                        "list_sources", "vault_status"}
            assert {t.name for t in tools} == expected
            for tool in tools:
                assert tool.annotations is not None
                assert tool.annotations.readOnlyHint is True
                assert tool.annotations.destructiveHint is False

            found = await client.call_tool("search_notes", {"query": "browsphere", "limit": 2})
            assert not found.isError
            payload = json.loads(found.content[0].text)
            assert payload["trusted"] is False
            assert payload["evidence_status"] == "cached_unverified"
            assert len(payload["results"]) == 1
            assert "EXPLICIT_UNTRUSTED_CONTENT" not in json.dumps(payload)
            id_value = payload["results"][0]["id"]

            note = await client.call_tool("read_note", {"note_id_value": id_value})
            assert not note.isError
            data = json.loads(note.content[0].text)
            assert data["trusted"] is False
            assert data["evidence_status"] == "cached_unverified"
            assert "EXPLICIT_UNTRUSTED_CONTENT" in data["body"]

            source = await client.call_tool("check_source",
                                            {"url": "https://example.org/browsphere-research"})
            assert not source.isError
            assert json.loads(source.content[0].text)["exists"] is True

            count = await client.call_tool("vault_status", {})
            assert not count.isError
            assert json.loads(count.content[0].text)["total_notes"] >= 1

            sources = await client.call_tool("list_sources", {"limit": 2})
            assert not sources.isError
            assert json.loads(sources.content[0].text)["trusted"] is False

            # The bridge never advertises or dispatches mutation tools.
            # A genuine unknown-tool call may be an MCP error response.
            try:
                denied = await client.call_tool("create_note", {"title": "BAD"})
            except Exception:
                pass
            else:
                assert denied.isError, "An undiscovered write tool unexpectedly executed"

            # Invalid path-style IDs must fail, not read unrelated files.
            bad = await client.call_tool("read_note", {"note_id_value": "../bad"})
            assert bad.isError


class RealGatewayTests(unittest.TestCase):
    def test_live_mcp_protocol_and_read_only_vault(self):
        with tempfile.TemporaryDirectory(prefix="lucia-hpr-ci-") as temp:
            root = Path(temp)
            vault = Vault.init(root, platforms=())
            write_note(vault.notes_dir, title="Browsphere research course evidence",
                       body="EXPLICIT_UNTRUSTED_CONTENT: a cached example note, not a verified browser source.",
                       tags=["public", "test"], status="draft",
                       source="https://example.org/browsphere-research",
                       summary="Browsphere sample research note")
            plan = compute_sync_plan(vault)
            execute_sync(vault, plan)
            vault.close()

            proc = subprocess.Popen(
                [sys.executable, str(GATEWAY), "--vault", str(root)],
                cwd=str(REPO), stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE, text=True,
                env={**os.environ, "PYTHONUNBUFFERED": "1"},
            )
            try:
                last_error = None
                for _ in range(35):
                    if proc.poll() is not None:
                        raise AssertionError(f"Gateway exited early: {proc.stderr.read()[-2500:]}")
                    try:
                        asyncio.run(connect_and_verify())
                        break
                    except (OSError, ConnectionError) as exc:
                        last_error = exc
                        time.sleep(0.2)
                    except Exception as exc:
                        # HTTP endpoint can refuse initialization while booting.
                        if "connect" in str(exc).lower():
                            last_error = exc
                            time.sleep(0.2)
                        else:
                            raise
                else:
                    raise AssertionError(f"MCP initialization timed out: {last_error}")
            finally:
                proc.terminate()
                try:
                    proc.wait(timeout=4)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait(timeout=4)


if __name__ == "__main__":
    unittest.main()
