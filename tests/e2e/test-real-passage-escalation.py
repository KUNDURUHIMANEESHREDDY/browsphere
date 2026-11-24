"""Isolated, real-model escalation test with a deliberately rejected first selection.
Never modify the live profile, model server, or saved router preferences.
"""
import argparse, ctypes, hashlib, json, os, socket, subprocess, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.request import build_opener, ProxyHandler, HTTPRedirectHandler, Request

ROOT=Path(__file__).resolve().parents[2]
class Memory(ctypes.Structure):
    _fields_=[("length",ctypes.c_uint32),("load",ctypes.c_uint32)]+[(name,ctypes.c_uint64) for name in ("total","available","page","page_available","virtual","virtual_available","extended")]
def free_mib():
    m=Memory();m.length=ctypes.sizeof(m)
    if not ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(m)):return None
    return m.available//1048576
class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self,*args,**kwargs):return None
def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--exe",required=True)
    ap.add_argument("--output",required=True)
    ap.add_argument("--observe",action="store_true",help="Record public fixture selection without injecting a fault")
    a=ap.parse_args()
    exe=(ROOT/a.exe).resolve();out=(ROOT/a.output).resolve();out.mkdir(parents=True,exist_ok=True)
    calls=[];memory=free_mib()
    result={"startFreeMiB":memory,"modelCalls":calls,"status":"blocked"}
    def save():
        (out/"results.json").write_text(json.dumps(result,indent=2),encoding="utf-8")
        print("ESCALATION_RESULT "+json.dumps(result),flush=True)
    if not a.observe and (memory is None or memory<3072):
        result["reason"]="Less than 3 GiB available before test; production escalation memory gate not bypassed";save();return 2
    opener=build_opener(ProxyHandler({}),NoRedirect())
    class Gateway(BaseHTTPRequestHandler):
        def log_message(self,*args):pass
        def relay(self):
            if self.path not in ("/api/tags","/api/ps","/api/chat","/api/show","/api/generate","/api/version"):
                self.send_error(404);return
            body=self.rfile.read(int(self.headers.get("Content-Length","0"))) if self.command=="POST" else None
            if body and len(body)>4*1024*1024:self.send_error(413);return
            request=json.loads(body) if body else {}
            messages=request.get("messages",[])
            selector=self.path=="/api/chat" and len(messages)>=2 and "numbered candidate passages" in messages[0].get("content","")
            row=None
            if selector:
                row={"model":request.get("model"),"payloadHash":hashlib.sha256(messages[1]["content"].encode()).hexdigest(),"formatHash":hashlib.sha256(json.dumps(request.get("format"),sort_keys=True).encode()).hexdigest(),"freeMiB":free_mib(),"injected":False}
                calls.append(row)
            try:
                req=Request("http://127.0.0.1:11434"+self.path,data=body,headers={"Content-Type":"application/json"},method=self.command)
                with opener.open(req,timeout=65) as response:raw=response.read()
                if row is not None:
                    response=json.loads(raw)
                    row["completed"]=True
                    row["freeAfterMiB"]=free_mib()
                    if a.observe:
                        row["rawSelection"]=response["message"]["content"]
                        row["publicCandidates"]=json.loads(messages[1]["content"])["candidates"]
                    # Actual first inference completed before deliberate rejection.
                    if not a.observe and len(calls)==1 and row["model"]=="lfm3:1.7b":
                        response["message"]["content"]='{"ids":[]}'
                        row["injected"]=True;raw=json.dumps(response).encode()
                    else:
                        try:row["selectedIds"]=json.loads(response["message"]["content"])["ids"]
                        except (KeyError,ValueError,TypeError):row["selectedIds"]=None
                self.send_response(200);self.send_header("Content-Type","application/json");self.send_header("Content-Length",str(len(raw)));self.end_headers();self.wfile.write(raw)
            except Exception as exc:
                if row is not None:row["error"]=type(exc).__name__
                self.send_error(502)
        do_GET=relay
        do_POST=relay
    with ThreadingHTTPServer(("127.0.0.1",0),Gateway) as server:
        worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
        with socket.socket() as s:s.bind(("127.0.0.1",0));port=s.getsockname()[1]
        env={k:v for k,v in os.environ.items() if not k.startswith(("LUCIA_","WEBVIEW2_","BROWSER_CDP_PORT"))}
        env.update(LUCIA_TEST_OLLAMA_URL=f"http://127.0.0.1:{server.server_port}",LUCIA_TEST_LOCAL_MODEL="qwen3.5:0.8b",LUCIA_TEST_PLANNER_MODEL="lfm3:1.7b",LUCIA_TEST_SUPERVISOR_MODEL="lfm3:1.7b",LUCIA_TEST_VISION_MODEL="lfm3-vl:2b",LUCIA_PHASE3_CASE="heldout-mdn")
        try:
            log=out/"live.log"
            with log.open("w",encoding="utf-8") as f:
                run=subprocess.run(["powershell","-NoProfile","-File","tests/e2e/run-lucia-cross-site-live.ps1","-OnlyWikipedia","-ExecutablePath",str(exe),"-CdpPort",str(port)],cwd=ROOT,env=env,stdout=f,stderr=subprocess.STDOUT,timeout=220)
            result["runnerExit"]=run.returncode
            evidence=None
            for line in log.read_text(encoding="utf-8",errors="replace").splitlines():
                if line.startswith("PHASE3_RESULT "):evidence=json.loads(line[len("PHASE3_RESULT "):])
            result["browserChecks"]=evidence.get("checks",{}) if evidence else {}
            result["browserStatus"]=(evidence.get("task") or {}).get("status") if evidence else None
            checks={"exactTwoRealCalls":len(calls)==2 and all(c.get("completed") for c in calls),"lfm3Rejected":bool(calls and calls[0].get("injected")),"qwenRetry":len(calls)==2 and calls[1]["model"]=="qwen2.5:3b-instruct","sameCatalog":len(calls)==2 and calls[0]["payloadHash"]==calls[1]["payloadHash"] and calls[0]["formatHash"]==calls[1]["formatHash"],"verifiedBrowserResult":bool(run.returncode==0 and evidence and evidence.get("pass") is True and (evidence.get("task") or {}).get("verified"))}
            result["checks"]=checks
            result["status"]="observed" if a.observe else ("passed" if all(checks.values()) else "failed")
        except subprocess.TimeoutExpired:result["status"]="failed";result["reason"]="Bounded runner timed out"
        finally:server.shutdown()
    save();return 0 if result["status"]=="passed" else 1
if __name__=="__main__":raise SystemExit(main())
