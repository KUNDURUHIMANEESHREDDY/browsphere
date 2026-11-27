"""Isolated capability probe for upstream Browser Use 1.x, never Lucia.

The upstream Browser class owns startup/shutdown; no REPL or untrusted model
code is granted access to a user browsing profile. A successful local browser
start is NOT an autonomous task or native WebView2 completion.
"""
from __future__ import annotations
import asyncio
import json
import os
from browser_use import Agent, Browser

async def main() -> None:
    assert os.environ.get("GITHUB_ACTIONS") == "true", "CI-only external agent harness"
    os.environ["BROWSER_USE_AUTO_LOGIN"] = "false"
    browser = Browser("chromium", headless=True)
    try:
        state = await asyncio.wait_for(browser.start(), timeout=120)
        assert isinstance(state, dict), "Official Browser Use failed to start local Chromium"
        assert Agent is not None
        print("UPSTREAM_BROWSER_USE_BROWSER_CAPABILITY " + json.dumps({
            "runner": "browser-use",
            "local_isolated_browser": True,
            "engine_start_passed": True,
            "native_webview2_attached": False,
            "agent_llm_task_executed": False,
            "lucia_task_success": False,
        }))
    finally:
        await browser.stop()

if __name__ == "__main__":
    asyncio.run(main())
