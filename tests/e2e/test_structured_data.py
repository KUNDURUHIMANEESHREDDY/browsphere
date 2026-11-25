"""Run the exact production extraction script in isolated real Edge DOM fixtures."""
import html,json,os,re,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
EDGE=Path(os.environ.get("ProgramFiles(x86)",r"C:\Program Files (x86)"))/"Microsoft/Edge/Application/msedge.exe"
SCRIPT=(ROOT/"src-tauri/src/agent/structured_data.js").read_text(encoding="utf-8")
def main():
    fixture="""<!doctype html><title>Structured fixture</title><main>
    <table><caption>Rainfall</caption><thead><tr><th>Month</th><th>mm</th></tr></thead><tbody><tr><td>Jan<span hidden>HIDDEN_SECRET</span></td><td>12</td></tr><tr><td>Feb</td><td>9</td></tr><tr hidden><td>Hidden</td><td>999</td></tr></tbody></table>
    <table><tr><th>A</th><th>B</th></tr><tr><td colspan="2">ambiguous</td></tr></table>
    <table><tr><th>A</th><th>B</th></tr><tr><td><input value="PRIVATE_SECRET"></td><td>8</td></tr></table>
    <figure><figcaption>Rainfall chart</figcaption><svg role="img" width="360" height="150" viewBox="0 0 360 150"><g><rect x="20" y="30" width="60" height="80"/><text x="20" y="125">Jan</text><text x="20" y="20">12 mm</text></g><g><rect x="120" y="55" width="60" height="55"/><text x="120" y="125">Feb</text><text x="120" y="45">9 mm</text></g></svg></figure>
    <figure><figcaption>Unlabelled chart</figcaption><svg role="img" width="300" height="100"><g><rect x="10" y="5" width="50" height="80"/></g><g><rect x="80" y="10" width="50" height="60"/></g></svg></figure>
    <canvas width="300" height="100"></canvas>
    <table hidden><tr><th>X</th><th>Y</th></tr><tr><td>HIDDEN_PRIVATE</td><td>7</td></tr></table>
    </main><script>const extract=()=>""" + SCRIPT + """;
    const first=extract();const second=extract();document.querySelector('tbody td:nth-child(2)').textContent='13';const changed=extract();
    const pre=document.createElement('pre');pre.id='probe-result';pre.textContent=JSON.stringify({first,second,changed});document.body.append(pre);</script>"""
    with tempfile.TemporaryDirectory(prefix="lucia-structured-") as work:
        root=Path(work); file=root/"fixture.html";file.write_text(fixture,encoding="utf-8")
        run=subprocess.run([str(EDGE),"--headless=new","--disable-gpu","--no-first-run","--disable-background-networking","--user-data-dir="+str(root/"profile"),"--dump-dom",file.as_uri()],capture_output=True,text=True,encoding="utf-8",errors="replace",timeout=45)
        match=re.search(r'<pre id="probe-result">(.*?)</pre>',run.stdout,re.S)
        assert match,"No observed fixture result"
        results=json.loads(html.unescape(match.group(1)))
    first=results["first"]
    assert first==results["second"],results
    assert first["tables"]==[{"caption":"Rainfall","headers":["Month","mm"],"rows":[["Jan","12"],["Feb","9"]]}],first
    assert first["charts"]==[{"caption":"Rainfall chart","points":[{"label":"Jan","value":"12 mm"},{"label":"Feb","value":"9 mm"}]}],first
    assert first["omitted_tables"]==2 and first["omitted_charts"]==2,first
    assert "SECRET" not in json.dumps(first),first
    assert results["changed"]["tables"][0]["rows"][0][1]=="13" and first!=results["changed"],results
    print("PASS native Edge structured DOM: rectangular cells, labelled SVG points, hidden data excluded, merged cells, input tables and unsupported charts rejected, mutation detected")
if __name__=="__main__":main()
