"""Bounded, untrusted-data adapters for Lucia's optional Hyperresearch bridge.

Pure standard-library helpers so the safety contract can be tested without
installing Hyperresearch or starting an MCP server.
"""
from __future__ import annotations

import json
import re
from urllib.parse import urlsplit

_NOTE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,99}$")


def note_id(value: str) -> str:
    if not isinstance(value, str) or not _NOTE_ID.fullmatch(value):
        raise ValueError("Invalid Hyperresearch note ID")
    return value


def public_source(value: object) -> str | None:
    if not isinstance(value, str) or len(value) > 2048:
        return None
    try:
        parsed = urlsplit(value)
        if (parsed.scheme not in {"http", "https"} or not parsed.hostname
                or parsed.username is not None or parsed.password is not None
                or parsed.query or parsed.fragment):
            return None
    except ValueError:
        return None
    return value


def _field(row: dict, key: str, limit: int) -> str:
    value = row.get(key, "")
    return value[:limit] if isinstance(value, str) else ""


def _loads(raw: str, expected: type):
    if len(raw) > 2_000_000:
        raise ValueError("Hyperresearch MCP payload exceeds safety limit")
    obj = json.loads(raw)
    if not isinstance(obj, expected):
        raise ValueError("Unexpected Hyperresearch MCP payload")
    return obj


def search_results(raw: str) -> str:
    """Only small snippets/metadata; never return bulk note bodies from search."""
    rows = _loads(raw, list)
    results = []
    for entry in rows[:8]:
        if not isinstance(entry, dict):
            continue
        results.append({
            "id": _field(entry, "id", 100),
            "title": _field(entry, "title", 180),
            "summary": _field(entry, "summary", 320),
            "source": public_source(entry.get("source")),
            "status": _field(entry, "status", 20),
        })
    return json.dumps({"trusted": False, "evidence_status": "cached_unverified",
                       "results": results}, ensure_ascii=False)


def note_result(raw: str) -> str:
    row = _loads(raw, dict)
    if row.get("error") or not row.get("id"):
        raise ValueError("Hyperresearch note lookup failed")
    body = _field(row, "body", 6000)
    whole = row.get("body", "")
    return json.dumps({
        "trusted": False, "evidence_status": "cached_unverified",
        "id": _field(row, "id", 100),
        "title": _field(row, "title", 180),
        "source": public_source(row.get("source")),
        "body": body,
        "truncated": not isinstance(whole, str) or len(whole) > len(body),
    }, ensure_ascii=False)


def sources_result(raw: str) -> str:
    rows = _loads(raw, list)
    return json.dumps({
        "trusted": False, "evidence_status": "cached_unverified",
        "sources": [
            {"url": public_source(row.get("url")),
             "note_id": _field(row, "note_id", 100),
             "domain": _field(row, "domain", 180)}
            for row in rows[:20] if isinstance(row, dict)
        ],
    }, ensure_ascii=False)


def source_check(raw: str) -> str:
    row = _loads(raw, dict)
    return json.dumps({
        "trusted": False, "evidence_status": "cached_unverified",
        "exists": row.get("exists") is True,
        "url": public_source(row.get("url")),
        "note_id": _field(row, "note_id", 100),
    }, ensure_ascii=False)
