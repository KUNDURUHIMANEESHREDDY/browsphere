"""Real Edge test for point-prioritized native observation. No network access."""
import html,json,os,re,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
EDGE=Path(os.environ.get("ProgramFiles(x86)",r"C:\Program Files (x86)"))/"Microsoft/Edge/Application/msedge.exe"
source=(ROOT/"src-tauri/src/agent/dom_extractor.rs").read_text(encoding="utf-8")
template=re.search(r'r#"\(function\(\) \{\{(.*?)\}\}\)\(\);"#',source,re.S).group(0)[3:-2]
js=template.replace("{request_id}","test-observer").replace("{max_nodes}","120").replace("{max_text_chars}","2400").replace("{{","{").replace("}}","}").rstrip(";")
fixture="""<!doctype html><html><body><style>body{margin:0}main button{display:block;width:140px;height:30px}#target{position:fixed;top:60px;left:240px;width:250px;height:65px;font-size:22px}#cover{position:fixed;top:60px;left:240px;width:250px;height:65px;background:#fff;z-index:99}</style><main>"""+''.join(f'<button>Unrelated control {i}</button>' for i in range(140))+"""<button id="target">Field Guide</button></main><script>
const normal=()=>"""+js+""";
const probe=(point,viewport)=>"""+js.replace("const POINT_PROBE = null;","const POINT_PROBE = {point,viewport};")+""";
const initial=normal();const target=document.getElementById('target');const r=target.getBoundingClientRect();
const point={x:Math.round((r.left+r.width/2)/innerWidth*1000),y:Math.round((r.top+r.height/2)/innerHeight*1000)};
const viewport={origin:location.href,width:innerWidth,height:innerHeight,scroll_x:scrollX,scroll_y:scrollY,scale:devicePixelRatio};
const found=probe(point,viewport);const wrong=probe(point,{...viewport,scroll_y:viewport.scroll_y+10});
const cover=document.createElement('div');cover.id='cover';document.body.append(cover);const covered=probe(point,viewport);cover.remove();
target.setAttribute('aria-hidden','true');const hidden=probe(point,viewport);target.removeAttribute('aria-hidden');
target.disabled=true;const disabled=probe(point,viewport);target.disabled=false;
const result=document.createElement('pre');result.id='probe-result';result.textContent=JSON.stringify({initial:initial.interactive_nodes.map(n=>n.label),hit:found.snapshot.interactive_nodes.find(n=>n.node_id===found.hit_id),wrong:wrong.hit_id,covered:covered.hit_id,hidden:hidden.hit_id,disabled:disabled.hit_id});document.body.append(result);
</script></body></html>"""
with tempfile.TemporaryDirectory(prefix="visual-hit-") as work:
 root=Path(work); f=root/"fixture.html";f.write_text(fixture,encoding="utf-8")
 proc=subprocess.run([str(EDGE),"--headless=new","--disable-gpu","--no-first-run","--disable-background-networking","--user-data-dir="+str(root/"profile"),"--dump-dom",f.as_uri()],capture_output=True,text=True,encoding="utf-8",errors="replace",timeout=45)
 m=re.search(r'<pre id="probe-result">(.*?)</pre>',proc.stdout,re.S)
 assert m,"Missing hit-test result"
 result=json.loads(html.unescape(m.group(1)))
 assert len(result["initial"])==120 and "Field Guide" not in result["initial"],result
 assert result["hit"]["label"]=="Field Guide",result
 assert all(result[k] is None for k in ("wrong","covered","hidden","disabled")),result
 print("PASS screenshot-only target omitted from initial 120 nodes, independently observed; viewport drift, occlusion, hidden and disabled controls rejected")
