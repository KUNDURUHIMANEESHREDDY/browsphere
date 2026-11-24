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
    main.innerHTML='<label>From station <input role="searchbox" aria-label="Enter From station. Input is Mandatory." value="Central"></label><label>To station <input role="searchbox" aria-label="Enter To station. Input is Mandatory." value="North"></label><label>Journey date <input readonly aria-label="Journey date" value="25/11/2026"></label><div class="ui-datepicker"><div><span class="ui-datepicker-month">November</span> <span class="ui-datepicker-year">2026</span></div><table><tr><td><a href="#">25</a></td><td class="ui-datepicker-other-month"><a href="#">26</a></td></tr></table></div><input aria-label="Date of birth" value="01/02/2000"><button>Search Trains</button>';
    const route=snapshot();
    main.innerHTML='<h1>Public catalog</h1><section class="search-results"><article class="product-card"><h2><a href="/notebook">Notebook Pro</a></h2><p>INR 59999</p></article><article class="product-card"><h2><a href="/camera">Camera Model A</a></h2><p>INR 39999</p></article><article class="product-card" style="display:none"><h2>Hidden item</h2><p>INR 99999</p></article></section>';
    const catalog=snapshot();
    main.innerHTML='<h1>Notebook Pro</h1><p class="product-price">INR 59999</p><button>Add to cart</button><p>Public product description and supported specifications.</p>';
    const product=snapshot();
    main.innerHTML='<h1>Search results</h1>'+Array.from({length:160},(_,i)=>'<div role="checkbox">Filter '+i+'</div>').join('')+'<section><a href="/product/matching-notebook-16gb-256gb">Matching notebook 16GB 256GB</a><a href="/other">Other result</a></section>';
    const crowded=snapshot();
    main.innerHTML='<input name="search" placeholder="What are you looking for ?"><input aria-label="Search Brand" placeholder="Search">';
    const semanticSearch=snapshot();
    main.innerHTML='<h1>Notebook Pro 16GB 256GB</h1><div><span class="selling-price">INR 59999</span><del>INR 69999</del><button>Add to cart</button></div><aside>INR 11111</aside>';
    const scopedOffer=snapshot();
    main.innerHTML='<h1>Library reference</h1><h2>Example</h2><p>The documentation describes the public interface and how the method behaves.</p><p>The method accepts an input and returns a value with the documented type.</p><pre>example.method(value)</pre>';
    const reference=snapshot();
    main.innerHTML='<h1>Loading</h1><div role="progressbar" aria-label="Loading results">Loading results...</div>';
    const loading=snapshot();
    main.innerHTML='<h1>Public repository</h1>'+Array.from({length:180},(_,i)=>'<p><a href="/unrelated/'+i+'">Unrelated file '+i+'</a> Long incidental file listing that is not the documentation.</p>').join('')+'<article><h2>README</h2><p>This public widget library can be installed with the package command shown below. Its supported runtime is Python 3.10 or later, and the library provides a public interface for reading documents.</p><pre>pip install widget</pre><p>Use the published documentation to learn how the package behaves and how to use its public interface.</p><a href="/acme/widget/blob/main/README.md">README.md</a><a href="/acme/widget/blob/main/pyproject.toml">pyproject.toml</a><p style="display:none">HIDDEN FIXTURE SECRET</p></article>';
    const repository=snapshot();
    const result=document.createElement('pre'); result.id='probe-result';
    result.textContent=JSON.stringify({modal,form,article,route,catalog,product,crowded,semanticSearch,scopedOffer,reference,loading,repository}); document.body.append(result);
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
    route=result["route"]["interactive_nodes"]
    assert any(n["label"]=="Journey date" and n.get("search_state")=="public_date:2026-11-25" for n in route), route
    assert any(n["label"]=="2026-11-25" and n["tag"]=="a" for n in route), route
    assert not any(n["label"]=="2026-11-26" for n in route), route
    assert not any("2000" in str(n.get("search_state","")) for n in route), route
    catalog=result["catalog"]["structure"];product=result["product"]["structure"];reference=result["reference"]["structure"]
    assert catalog["priced_cards"]==2 and catalog["product_cards"]==2 and catalog["result_links"]==2, catalog
    assert product["offer_blocks"]==1 and product["priced_cards"]==0, product
    assert any(n["label"]=="Matching notebook 16GB 256GB" for n in result["crowded"]["interactive_nodes"]), "Rendered product link was crowded out"
    assert any(n["role"]=="searchbox" and n["label"]=="What are you looking for ?" and n["search_state"]=="empty" for n in result["semanticSearch"]["interactive_nodes"]), "Semantic search field was not recognized"
    # The scoped primary product extractor must never use the crossed-out MRP.
    offer=result["scopedOffer"]["structure"]["product"]
    assert offer and offer["price"]=="INR 59999", offer
    assert reference["headings"]>=2 and reference["paragraphs"]>=2 and reference["code_blocks"]==1, reference
    assert result["loading"]["structure"]["loading"], result["loading"]
    repo=result["repository"]
    assert "pip install widget" in repo["text_excerpt"] and "Python 3.10 or later" in repo["text_excerpt"], repo
    assert "HIDDEN FIXTURE SECRET" not in repo["text_excerpt"], repo
    assert {"README.md","pyproject.toml"}.issubset({n["label"] for n in repo["interactive_nodes"]}), repo
    print("PASS real browser DOM: modal scope, autocomplete option, read-only date, post-modal form, prose-first evidence, exact public journey-date readback, visible product cards, offer, reference, loading structure, prioritized embedded README and metadata links")
if __name__ == "__main__":
    main()
