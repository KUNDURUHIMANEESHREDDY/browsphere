from pathlib import Path
import psutil, socket, time
base=Path(r"C:\Users\himan\Downloads\Browsphere")
procs=[]
for proc in psutil.process_iter(["name","cmdline"]):
    try:
        name=(proc.info["name"] or "").lower()
        arguments=" ".join(proc.info["cmdline"] or []).lower()
        if name=="msedge.exe" and "edge-cdp-voice-split" in arguments:
            procs.append(proc)
    except (psutil.NoSuchProcess,psutil.AccessDenied):
        pass
for proc in procs:
    try: proc.terminate()
    except psutil.Error: pass
psutil.wait_procs(procs,timeout=5)
time.sleep(1)
sock=socket.socket()
port_closed=sock.connect_ex(("127.0.0.1",9231))!=0
sock.close()
print("Temporary Voice Agent test browser closed:",port_closed)
for filename in ["patch_voice_split_a.py","patch_voice_split_b.py","patch_voice_split_tests.py","patch_voice_split_docs.py","lucia_voice_split_edge.ps1"]:
    file=base/"tests"/"e2e"/filename
    file.unlink(missing_ok=True)
print("Temporary patch scripts cleaned.")
assert port_closed,"Debug port remains open; close the temporary Edge CDP process"
