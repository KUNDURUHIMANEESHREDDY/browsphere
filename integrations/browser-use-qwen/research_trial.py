"""Upstream Browser Use Qwen acceptance: isolated CI only, not a production sidecar.

Browser Use 1.x executes model-written Python. Do not run this against a user
profile or on a personal machine; GitHub runner has no secrets/credentials.
"""
import argparse
import asyncio
from html.parser import HTMLParser
import html
import json
import os
from pathlib import Path
import tempfile
import time
from urllib.parse import urlsplit
from urllib.request import Request, urlopen

CASES = {
    "webdev": ("https://web.dev/learn/accessibility/", "Report one specific accessibility recommendation."),
    "w3c": ("https://www.w3.org/WAI/tutorials/images/informative/", "Report one recommendation about informative images."),
}
MODEL = "qwen2.5:3b-instruct"

class VisibleText(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.text = []
        self.skip = 0
    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style", "svg"):
            self.skip += 1
    def handle_endtag(self, tag):
        if tag in ("script", "style", "svg") and self.skip:
            self.skip -= 1
    def handle_data(self, value):
        if not self.skip:
            self.text.append(value)

def normalize(text):
    return " ".join(html.unescape(text).split()).casefold()

def exact_source(expected, actual):
    a, b = urlsplit(expected), urlsplit(actual)
    return (a.scheme == b.scheme == "https" and a.netloc.lower() == b.netloc.lower()
            and a.path.rstrip("/") == b.path.rstrip("/") and not b.query
            and not b.fragment and not b.username and not b.password)

def quote_verified(expected, actual, quote, claim, html_body):
    if not exact_source(expected, actual) or len(quote.strip()) < 25:
        return False
    doc = VisibleText()
    doc.feed(html_body)
    return normalize(quote) == normalize(claim) and normalize(quote) in normalize(" ".join(doc.text))

def independently_fetch(url):
    req = Request(url, headers={"User-Agent": "Browsphere-CI-Source-Verification/1.0"})
    with urlopen(req, timeout=20) as response:
        if not exact_source(url, response.geturl()):
            raise ValueError("Official source URL mismatch")
        data = response.read(2_000_001)
        if len(data) > 2_000_000:
            raise ValueError("Source HTML exceeds independent evidence limit")
        return data.decode("utf-8", errors="replace")

def ollama_preflight():
    # Keep this diagnostic separate from the much larger Browser Use prompt.
    # A tiny response failing proves the model endpoint, not the browser, is blocked.
    payload = json.dumps({"model": MODEL, "messages": [
        {"role": "user", "content": "Reply exactly READY."}
    ], "max_tokens": 16, "stream": False}).encode("utf-8")
    request = Request("http://127.0.0.1:11434/v1/chat/completions",
        data=payload, headers={"Content-Type": "application/json",
                               "Authorization": "Bearer ollama"})
    with urlopen(request, timeout=65) as response:
        result = json.load(response)
    if not result.get("choices"):
        raise ValueError("Ollama returned no model completion")


async def trial(name):
    if os.getenv("GITHUB_ACTIONS") != "true" or os.getenv("LUCIA_UPSTREAM_AUTONOMY_TEST") != "1":
        raise RuntimeError("Model-written Python requires explicit ephemeral CI authorization")
    if os.getenv("LUCIA_OLLAMA_MODEL", MODEL) != MODEL:
        raise RuntimeError("Only reviewed local Qwen model is permitted")
    from browser_use import Agent, Browser, ChatOpenAI
    from pydantic import BaseModel, Field
    class Finding(BaseModel):
        source_url: str
        quote: str = Field(min_length=25)
        claim: str = Field(min_length=25)
    url, objective = CASES[name]
    result = {"case": name, "model": MODEL, "source": url, "passed": False,
              "agent_done": False, "quote_verified": False}
    started = time.monotonic()
    phase = "ollama_preflight"
    try:
        phase_started = time.monotonic()
        await asyncio.to_thread(ollama_preflight)
        result["ollama_preflight_s"] = round(time.monotonic() - phase_started, 1)
        with tempfile.TemporaryDirectory(prefix="browsphere-upstream-qwen-") as workspace:
            browser = Browser("chromium", allowed_domains=[urlsplit(url).hostname],
                              block_ip_addresses=True)
            try:
                phase = "browser_start"
                phase_started = time.monotonic()
                await asyncio.wait_for(browser.start(), timeout=60)
                result["browser_start_s"] = round(time.monotonic() - phase_started, 1)
                phase = "agent_run"
                agent = Agent(
                    f"Open ONLY {url}. {objective} No logins, form submissions, downloads, file operations, "
                    "OS commands or other sites. Return source_url as the exact requested page, "
                    "quote as a contiguous verbatim recommendation from the visible document, "
                    "and claim as the IDENTICAL quote. If unable, do not invent passages.",
                    llm=ChatOpenAI(model=MODEL, base_url="http://127.0.0.1:11434/v1",
                                   api_key="ollama"),
                    browser=browser, output_model_schema=Finding,
                    mode="ultrafast", max_context_chars=5000,
                    web_search=False, workspace=workspace,
                )
                history = await asyncio.wait_for(agent.run(max_steps=9), timeout=210)
                result["agent_done"] = bool(history.is_done())
                result["steps"] = history.number_of_steps()
                finding = getattr(history, "structured_output", None)
                if isinstance(finding, dict):
                    finding = Finding.model_validate(finding)
                if result["agent_done"] and isinstance(finding, Finding):
                    phase = "independent_quote"
                    original = await asyncio.to_thread(independently_fetch, url)
                    result["quote_verified"] = quote_verified(url, finding.source_url,
                        finding.quote, finding.claim, original)
                    result["passed"] = result["quote_verified"]
            finally:
                try:
                    await asyncio.wait_for(browser.stop(), timeout=20)
                except Exception as cleanup_error:
                    result["cleanup_error_type"] = type(cleanup_error).__name__
    except Exception as exc:
        result["error_type"] = type(exc).__name__
    result["phase"] = "completed" if result["passed"] else phase
    result["seconds"] = round(time.monotonic() - started, 1)
    return result

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=sorted(CASES), required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    try:
        report = asyncio.run(trial(args.case))
    except Exception as exc:
        report = {"case": args.case, "passed": False, "error_type": type(exc).__name__}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("BROWSER_USE_QWEN_ACCEPTANCE " + json.dumps(report))
    raise SystemExit(0 if report["passed"] else 1)

if __name__ == "__main__":
    main()
