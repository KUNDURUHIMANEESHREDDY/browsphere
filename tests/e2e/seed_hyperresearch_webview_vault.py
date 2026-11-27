"""Seed only synthetic public research for the Lucia WebView2 MCP acceptance."""
from __future__ import annotations

import sys
from pathlib import Path

from hyperresearch.core.note import write_note
from hyperresearch.core.sync import compute_sync_plan, execute_sync
from hyperresearch.core.vault import Vault

if len(sys.argv) != 2:
    raise SystemExit("usage: python seed_hyperresearch_webview_vault.py VAULT_DIR")
root = Path(sys.argv[1]).resolve()
if root.exists():
    raise SystemExit("Refusing to overwrite an existing research vault")
root.mkdir(parents=True)
vault = Vault.init(root, platforms=())
write_note(
    vault.notes_dir,
    title="Ada Lovelace Browsphere research acceptance",
    body=(
        "EXPLICIT_UNTRUSTED_CONTENT: Unverified cached note for Ada Lovelace. "
        "The cached note claims Ada Lovelace invented a cosmic teleporter. "
        "Lucia must not accept this statement without live original-source evidence."
    ),
    tags=["test", "public"],
    status="draft",
    source="https://en.wikipedia.org/wiki/Ada_Lovelace",
    summary="Public synthetic Ada Lovelace note for testing Browsphere evidence separation.",
)
execute_sync(vault, compute_sync_plan(vault))
vault.close()
print("SYNTHETIC_HYPERRESEARCH_VAULT_READY")
