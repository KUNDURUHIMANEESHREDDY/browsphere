from pathlib import Path
import subprocess,re,json,os,time,socket
import psutil
base=Path(r"C:\Users\himan\Downloads\Browsphere")
for relative in ["ui/public/index.html","ui/public/ai-chat.html"]:
    html=(base/relative).read_text(encoding="utf-8")
    blocks=re.findall(r"<script>([\s\S]*?)</script>",html)
    assert blocks,(relative,"missing inline JavaScript")
    for i,code in enumerate(blocks):
        testfile=base/"tests"/"e2e"/f"_tmp_{i}.js"
        testfile.write_text(code,encoding="utf-8")
        try:
            result=subprocess.run(["node","--check",str(testfile)],capture_output=True,text=True)
            assert result.returncode==0,(relative,result.stderr)
        finally:
            testfile.unlink(missing_ok=True)
    print("PASS JavaScript syntax",relative)
popup=(base/"ui/public/ai-chat.html").read_text(encoding="utf-8")
assert 'id="btnPause"' not in popup and 'id="btnStop"' not in popup and 'data-chip=' not in popup
assert 'Your AI browsing partner.' in popup
assert 'id="btnVoice"' in popup and 'id="btnVoiceSettings"' in popup
print("PASS floating Lucia design markup")
page=(base/"ui/public/index.html").read_text(encoding="utf-8")
legacy=page.split('id="aiChatPopover"',1)[1].split('id="automationPopover"',1)[0]
assert 'chatPauseAgentBtn' not in legacy and 'data-action="agent-stop"' not in legacy and 'data-action="chat-chip"' not in legacy
assert 'lucia-toolbar-mark" aria-hidden="true">L' not in page
print("PASS old inline fallback chrome cleaned")
result=json.loads((base/"tests/e2e/artifacts/lucia-layout-results.json").read_text(encoding="utf-8"))
assert all(item['passed'] for item in result['outcomes'])
print("PASS visual UI regression",len(result['outcomes']),"checks")
for name in ["lucia_doc_cleanup.py","patch_lucia_fallback.py"]:
    (base/"tests/e2e"/name).unlink(missing_ok=True)
# Stop only our temporary Edge headless CDP browser, never the user's normal Edge.
target=[]
for p in psutil.process_iter(["name","cmdline"]):
    try:
        name=(p.info["name"] or "").lower()
        cmd=" ".join(p.info["cmdline"] or []).lower()
        if name == "msedge.exe" and ("edge-cdp-lucia" in cmd or "--remote-debugging-port=9231" in cmd):
            target.append(p)
    except (psutil.NoSuchProcess,psutil.AccessDenied):
        continue
for p in target:
    try:p.terminate()
    except psutil.Error:pass
psutil.wait_procs(target,timeout=5)
time.sleep(1)
sock=socket.socket()
print("PASS temporary Edge debugging closed" if sock.connect_ex(("127.0.0.1",9231)) != 0 else "WARNING debug port still reachable")
sock.close()
print("LUClA FINAL CHECK COMPLETE")
