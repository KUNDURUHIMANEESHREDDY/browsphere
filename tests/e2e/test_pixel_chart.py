"""Real Edge test of the production pixel region selector (no model inference)."""
import html,json,os,re,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
EDGE=Path(os.environ.get("ProgramFiles(x86)",r"C:\Program Files (x86)"))/"Microsoft/Edge/Application/msedge.exe"
SCRIPT=(ROOT/"src-tauri/src/agent/pixel_chart.js").read_text(encoding="utf-8")
def main():
    fixture="""<!doctype html><title>Pixel chart regions</title><style>body{margin:0}figure{margin:8px}canvas,img{display:block;width:240px;height:120px}</style>
    <main><figure id="c"><figcaption>Rainfall chart</figcaption><canvas width="240" height="120"></canvas></figure>
    <figure id="i"><figcaption>Temperature graph</figcaption><img width="240" height="120"></figure>
    <figure hidden><figcaption>Hidden chart</figcaption><canvas width="240" height="120"></canvas></figure>
    <figure style="position:absolute;top:3000px"><figcaption>Offscreen chart</figcaption><canvas width="240" height="120"></canvas></figure>
    <figure style="position:absolute;left:300px;top:10px"><figcaption>Ordinary photo</figcaption><canvas width="240" height="120"></canvas></figure>
    </main><script>
    const locate=()=>""" + SCRIPT + """;
    const c=document.querySelector('#c canvas');const ctx=c.getContext('2d');ctx.fillStyle='white';ctx.fillRect(0,0,240,120);ctx.fillStyle='blue';ctx.fillRect(30,30,40,80);
    const i=document.querySelector('#i img');i.onload=()=>{
      const initial=locate();const b=c.getBoundingClientRect();const cover=document.createElement('div');cover.style='position:fixed;background:red;z-index:999;left:'+b.left+'px;top:'+b.top+'px;width:'+b.width+'px;height:'+b.height+'px';document.body.append(cover);
      const covered=locate();cover.remove();document.querySelector('#c').style.display='none';const hidden=locate();
      const pre=document.createElement('pre');pre.id='probe-result';pre.textContent=JSON.stringify({initial,covered,hidden});document.body.append(pre);
    };i.src=c.toDataURL('image/png');</script>"""
    with tempfile.TemporaryDirectory(prefix="lucia-pixel-region-") as work:
        root=Path(work);f=root/"fixture.html";f.write_text(fixture,encoding="utf-8")
        p=subprocess.run([str(EDGE),"--headless=new","--disable-gpu","--no-first-run","--disable-background-networking","--window-size=1000,800","--virtual-time-budget=1500","--user-data-dir="+str(root/"profile"),"--dump-dom",f.as_uri()],capture_output=True,text=True,encoding="utf-8",errors="replace",timeout=45)
        match=re.search(r'<pre id="probe-result">(.*?)</pre>',p.stdout,re.S)
        assert match,"No native region fixture result"
        r=json.loads(html.unescape(match.group(1)))
    assert [x["kind"] for x in r["initial"]["regions"]]==["canvas","img"],r
    assert [x["caption"] for x in r["initial"]["regions"]]==["Rainfall chart","Temperature graph"],r
    assert [x["kind"] for x in r["covered"]["regions"]]==["img"],r
    assert [x["kind"] for x in r["hidden"]["regions"]]==["img"],r
    assert r["initial"]["omitted"]>=2,r
    print("PASS real Edge pixel region discovery: canvas, raster image, visibility, caption, viewport and occlusion gates")
if __name__=="__main__":main()
