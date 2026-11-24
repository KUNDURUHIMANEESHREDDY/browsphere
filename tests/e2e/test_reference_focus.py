"""Rendered reference evidence beyond the normal excerpt limit. Offline Edge."""
import html,json,os,re,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
EDGE=Path(os.environ.get("ProgramFiles(x86)",r"C:\Program Files (x86)"))/"Microsoft/Edge/Application/msedge.exe"
source=(ROOT/"src-tauri/src/agent/dom_extractor.rs").read_text(encoding="utf-8")
template=re.search(r'r#"\(function\(\) \{\{(.*?)\}\}\)\(\);"#',source,re.S).group(0)[3:-2]
js=template.replace("{request_id}","reference-probe").replace("{max_nodes}","120").replace("{max_text_chars}","24000").replace("{{","{").replace("}}","}").rstrip(";")
a="DO NOTHING skips the conflicting insertion when the requested row already exists."
b="DO UPDATE updates the matching row with the specified column values."
long="".join("<p>Unrelated section "+str(i)+". "+("Background explanation of a public reference. "*24)+"</p>" for i in range(35))
fixture="<!doctype html><title>Reference</title><main>"+long+'<div hidden><p>DO NOTHING returns all private records.</p></div><nav><p>DO UPDATE updates unrelated navigation.</p></nav><p>'+a+' '+b+'</p></main>'
fixture+="<script>const original=()=>"+js+";const focused=()=>"+js.replace("const FOCUS_TERMS = [];","const FOCUS_TERMS = ['do nothing','do update'];")+";const p=document.createElement('pre');p.id='probe-result';p.textContent=JSON.stringify({plain:original().text_excerpt,focused:focused().text_excerpt});document.body.append(p);</script>"
with tempfile.TemporaryDirectory(prefix="reference-focus-") as work:
 root=Path(work);f=root/"fixture.html";f.write_text(fixture,encoding="utf-8")
 proc=subprocess.run([str(EDGE),"--headless=new","--disable-gpu","--no-first-run","--disable-background-networking","--user-data-dir="+str(root/"profile"),"--dump-dom",f.as_uri()],capture_output=True,text=True,encoding="utf-8",errors="replace",timeout=45)
 m=re.search(r'<pre id="probe-result">(.*?)</pre>',proc.stdout,re.S)
 assert m,"Missing reference observation"
 result=json.loads(html.unescape(m.group(1)))
 assert a not in result["plain"] and b not in result["plain"]
 assert a in result["focused"] and b in result["focused"]
 assert "returns all private records" not in result["focused"] and "updates unrelated navigation" not in result["focused"]
 assert len(result["focused"])<=24000
 print("PASS focused reference evidence beyond 24000-character limit; hidden and navigation decoys excluded")
