"""Run real Lucia Rust MCP transport against real Python Hyperresearch FastMCP.

Designed for Windows CI. The temporary research vault contains only synthetic data.
"""
from __future__ import annotations

import os
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from hyperresearch.core.note import write_note
from hyperresearch.core.sync import compute_sync_plan, execute_sync
from hyperresearch.core.vault import Vault

REPO = Path(__file__).resolve().parents[1]
GATEWAY = REPO / "tools" / "hyperresearch_readonly_gateway.py"


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="browsphere-hpr-rust-") as temp:
        root = Path(temp)
        vault = Vault.init(root, platforms=())
        write_note(
            vault.notes_dir, title="Browsphere public test research",
            body="Browsable public information about Browsphere research reliability.",
            tags=["public", "fixture"], status="draft",
            source="https://example.org/browsphere-research",
            summary="Public test information about browsphere",
        )
        execute_sync(vault, compute_sync_plan(vault))
        vault.close()
        child = subprocess.Popen(
            [sys.executable, str(GATEWAY), "--vault", str(root)],
            cwd=REPO, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
            text=True, env={**os.environ, "PYTHONUNBUFFERED": "1"},
        )
        try:
            for _ in range(50):
                if child.poll() is not None:
                    raise RuntimeError("MCP bridge quit: " + child.stderr.read()[-2500:])
                try:
                    with socket.create_connection(("127.0.0.1", 8765), timeout=0.2):
                        break
                except OSError:
                    time.sleep(0.2)
            else:
                raise RuntimeError("MCP bridge did not start on loopback")
            env = {**os.environ, "BROWSPHERE_HPR_MCP_URL": "http://127.0.0.1:8765/mcp"}
            command = [
                "cargo", "test", "--manifest-path", str(REPO / "src-tauri" / "Cargo.toml"),
                "-p", "browsphere-core", "actual_hyperresearch_mcp_gateway_via_lucia",
                "--", "--ignored", "--nocapture",
            ]
            result = subprocess.run(command, cwd=REPO, env=env, timeout=1200, check=False)
            return result.returncode
        finally:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)


if __name__ == "__main__":
    sys.exit(main())
