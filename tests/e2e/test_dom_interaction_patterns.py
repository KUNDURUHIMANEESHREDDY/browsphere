"""Run the real Rust-embedded DOM observer in an isolated Edge fixture."""
import html
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "src-tauri/src/agent/dom_extractor.rs"
EDGE = Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "Microsoft/Edge/Application/msedge.exe"

def main():
    raw = SOURCE.read_text(encoding="utf-8")
    template = re.search(r'r#"\(function\(\) \{\{(.*?)\}\}\)\(\);"#', raw, re.S).group(0)[3:-2]
    js = template.replace("{request_id}", "test-observer").replace("{max_nodes}", "120").replace("{max_text_chars}", "2400").replace("{{", "{").replace("}}", "}")
    document = """<!doctype html><html><head><title>Interaction fixture</title></head><body>
    <main><label>From station <input role="combobox" aria-label="From station"></label>
    <label>Journey date <input readonly aria-label="Journey date" value="24/11/2026"></label>
    <ul role="listbox"><li role="option">Central Station</li></ul>
    <button>Search trains</button></main>
    <dialog id="welcome"><h2>Choose language</h2><button aria-label="Confirmation. SafeValue must use [property]=binding: &lt;body&gt;">Hindi</button><button aria-label="Confirmation. SafeValue must use [property]=binding: &lt;body&gt;">English</button></dialog>
    <script>const snapshot=()=>""" + js.rstrip(";") + """;
    const dialog=document.querySelector('#welcome'); dialog.showModal();
    const modal=snapshot(); dialog.close(); const form=snapshot();
    const main=document.querySelector('main');
    main.innerHTML='<nav>'+('Navigation menu '.repeat(300))+'</nav><table><tr><td>'+('Unrelated table numbers '.repeat(300))+'</td></tr></table><p style="display:none">HIDDEN PRIVATE TEST TEXT</p><p>The public field guide explains how an orchard produces fruit. The orchard receives sunlight and rain throughout the growing season. Its harvest depends on conditions including temperature and water availability.</p><p>Pollinators support many flowering plants.</p>';
    const article=snapshot();
    const result=document.createElement('pre'); result.id='probe-result';
    result.textContent=JSON.stringify({modal,form,article}); document.body.append(result);
    </script></body></html>"""
    with tempfile.TemporaryDirectory(prefix="lucia-dom-") as work:
        root = Path(work)
        path = root / "fixture.html"
        path.write_text(document, encoding="utf-8")
        proc = subprocess.run([str(EDGE), "--headless=new", "--disable-gpu",
            "--no-first-run", "--disable-background-networking",
            "--user-data-dir="+str(root/"profile"), "--dump-dom", path.as_uri()],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=45)
        match = re.search(r'<pre id="probe-result">(.*?)</pre>', proc.stdout, re.S)
        assert match, "No browser-observed fixture result"
        result = json.loads(html.unescape(match.group(1)))
    modal, form = result["modal"], result["form"]
    assert modal["text_excerpt"].startswith("Active dialog:"), modal
    assert {n["label"] for n in modal["interactive_nodes"]} == {"English", "Hindi"}, modal
    assert not any("station" in n["label"].lower() for n in modal["interactive_nodes"]), modal
    assert any(n["role"]=="option" and n["label"]=="Central Station" for n in form["interactive_nodes"]), form
    assert any(n["role"]=="combobox" and n["label"]=="From station" and n.get("search_state")=="empty" for n in form["interactive_nodes"]), form
    assert any(n["label"]=="Journey date" and n.get("search_state")=="read_only" for n in form["interactive_nodes"]), form
    assert any(n["label"]=="Search trains" for n in form["interactive_nodes"]), form
    assert result["article"]["text_excerpt"].startswith("The public field guide"), result["article"]
    assert "HIDDEN PRIVATE TEST TEXT" not in result["article"]["text_excerpt"], result["article"]
    print("PASS real browser DOM: modal scope, autocomplete option, read-only date, post-modal form, prose-first evidence")
if __name__ == "__main__":
    main()
