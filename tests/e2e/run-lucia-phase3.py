"""Tiered Lucia evaluation; Windows native runtime, disposable profiles.
No credentials, production writes, cloud inference, or implicit approvals.
"""
import argparse, datetime, hashlib, json, os, re, socket, subprocess, sys, time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
REVISION="lucia-approved-mcp-discovery-v13.11"
MODELS={"LUCIA_TEST_LOCAL_MODEL":"qwen3.5:0.8b","LUCIA_TEST_PLANNER_MODEL":"qwen2.5:3b-instruct","LUCIA_TEST_SUPERVISOR_MODEL":"lfm3:1.7b","LUCIA_TEST_VISION_MODEL":"lfm3-vl:2b"}
def case(id,tier,kind,mode=None,marker=None,timeout=200,expect="completion"):
    return dict(id=id,tier=tier,kind=kind,mode=mode or {},marker=marker,timeout=timeout,expect=expect)
CASES=[
    case("native-contracts","contracts","cargo",timeout=180,marker="test result:"),
    case("js-contracts","contracts","node",timeout=45),
    case("dom-fixture","contracts","dom",timeout=60),
    case("model-roles","contracts","roles",timeout=480,marker="ROLE_BENCHMARK_COMPLETE"),
]
for key in ("search-wikipedia","search-wiktionary","nav-rust","nav-python","extract-mdn","extract-python"):
    CASES.append(case(key,"skill","live",{"LUCIA_PHASE3_CASE":key},"PHASE3_RESULT"))
CASES.append(case("public-controls","skill","controls",timeout=180))
CASES.append(case("vision-fixture","skill","live",{"LUCIA_PHASE3_CASE":"vision-fixture"},"VISION_RESULT",timeout=220))
CASES.append(case("vision-independent","skill","live",{"LUCIA_PHASE3_CASE":"vision-independent"},"VISION_INDEPENDENT_RESULT",timeout=220))
CASES.append(case("vision-real-example","skill","live",{"LUCIA_PHASE3_CASE":"vision-real-example"},"VISION_REAL_RESULT",timeout=180))
CASES.append(case("vision-real-python","skill","live",{"LUCIA_PHASE3_CASE":"nav-python","LUCIA_REQUIRE_COORDINATE":"1"},"PHASE3_RESULT",timeout=180))
CASES.append(case("form-filter","skill","live",{"LUCIA_FILTER_TEST":"1"},"FILTER_RESULT"))
for key in ("compose-wiki","compose-dictionary"):
    CASES.append(case(key,"composition","live",{"LUCIA_PHASE3_CASE":key},"PHASE3_RESULT"))
CASES.append(case("compose-two-sources","composition","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"research"},"COMPLEX_RESEARCH",timeout=240))
for key in ("delayed-link","missing-link"):
    CASES.append(case(key,"failure","live",{"LUCIA_PHASE3_CASE":key},"PHASE3_RESULT",expect="safe_stop" if key=="missing-link" else "completion"))
CASES.append(case("mcp-negative","failure","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"negative"},"COMPLEX_GRAPH",expect="safe_stop"))
CASES.append(case("mcp-outage","failure","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"outage"},"COMPLEX_GRAPH"))
for n in range(1,9):
    if n==1: mode={"LUCIA_TRAIN_TEST":"1"};marker="TRAIN_RESULT"
    elif n in (5,6):mode={"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"graph" if n==5 else "outage"};marker="COMPLEX_GRAPH"
    else:mode={"LUCIA_LEVEL_TEST":str(n)};marker="LEVEL_RESULT"
    CASES.append(case("level-"+str(n),"autonomy","live",mode,marker,timeout=260))
for key in ("heldout-mdn","heldout-wiki"):
    CASES.append(case(key,"heldout","live",{"LUCIA_PHASE3_CASE":key},"PHASE3_RESULT"))
TIERS=("contracts","skill","composition","failure","autonomy","heldout")
def hash_file(p):
    h=hashlib.sha256()
    with p.open("rb") as f:
        for b in iter(lambda:f.read(1024*1024),b""):h.update(b)
    return h.hexdigest()
def port():
    with socket.socket() as s:
        s.bind(("127.0.0.1",0));return s.getsockname()[1]
def result_object(log,marker):
    if not marker:return None
    for line in reversed(log.splitlines()):
        if line.startswith(marker+" "):
            try:return json.loads(line[len(marker)+1:])
            except ValueError:return None
    return None
def evaluate(c,log,rc,timed_out=False):
    marker=c["marker"];kind=c["kind"];r=result_object(log,marker)
    checks={};reason=[];status="passed"
    if timed_out:return "failed",{},["Harness exceeded its bounded deadline"],None
    if kind=="live":
        checks["runtimeRevision"]=f"LUCIA_RUNTIME_REVISION {REVISION}" in log
        checks["runnerExit"]=rc==0
        checks["structuredResult"]=isinstance(r,dict)
        if r:
            checks["casePassed"]=r.get("pass",r.get("passed")) is True
            checks.update({k:v for k,v in r.get("checks",{}).items() if isinstance(v,bool)})
            if r.get("blocked") and not r.get("task"):
                status="blocked";reason.append(str(r.get("reason","Starting page unavailable"))[:350])
            t=r.get("task") or {}
            if t.get("errors"):reason.extend(str(x)[:240] for x in t["errors"][-3:])
            if t.get("approvalRequired"):reason.append("Unexpected approval request; run cancelled without approval")
            if t.get("status") and t["status"]!="completed" and c["expect"]!="safe_stop":
                reason.append("Agent terminal state: "+t["status"])
            if t.get("status")=="completed" and not t.get("verified") and c["expect"]!="safe_stop":reason.append("No independently checked report")
        if not checks["runtimeRevision"]:
            status="blocked";reason.append("Expected native runtime revision was not observed")
    elif kind=="cargo":
        checks={"runnerExit":rc==0,"contractResults":("test result: ok." in log and "0 failed" in log and "0 passed;" not in log)}
    elif kind=="roles":
        checks={"runnerExit":rc==0,"completeMatrix":bool(r and r.get("total")==19),"allProbesPassed":bool(r and r.get("passed")==r.get("total"))}
        if r and not checks["allProbesPassed"]:reason.append(f"Real inference matrix: {r.get('passed')}/{r.get('total')} passed")
    elif kind=="dom":checks={"runnerExit":rc==0,"browserFixture": "PASS real browser DOM:" in log}
    elif kind=="controls":checks={"runnerExit":rc==0,"nativeControls": "REAL_SITE_TESTS_PASSED" in log}
    else:
        passed=re.search(r"(?m)^.*\bpass (\d+)\s*$",log)
        failed=re.search(r"(?m)^.*\bfail (\d+)\s*$",log)
        checks={"runnerExit":rc==0,"testResults":bool(passed and failed and int(passed.group(1))>0 and int(failed.group(1))==0)}
    reason.extend("Failed check: "+k for k,v in checks.items() if not v)
    if not all(checks.values()) and status!="blocked":status="failed"
    if status!="passed" and not r:
        failure_lines=[l.strip()[:240] for l in log.splitlines() if l.startswith(("FAIL ","Error:","error:","thread "))]
        reason.extend(failure_lines[-3:])
    return status,checks,list(dict.fromkeys(reason)),r
def test_evaluator():
    c=case("test","skill","live",marker="PHASE3_RESULT")
    ok=f'LUCIA_RUNTIME_REVISION {REVISION}\nPHASE3_RESULT '+json.dumps({"pass":True,"checks":{"fact":True},"task":{"status":"completed","verified":True}})
    assert evaluate(c,ok,0)[0]=="passed"
    assert evaluate(c,ok,1)[0]=="failed"
    assert evaluate(c,"PHASE3_RESULT {\"pass\":true}",0)[0]=="blocked"
    assert evaluate(c,f"LUCIA_RUNTIME_REVISION {REVISION}",0)[0]=="failed"
    assert evaluate(c,ok.replace('"fact": true','"fact": false'),0)[0]=="failed"
    assert evaluate(c,ok,0,True)[0]=="failed"
    assert evaluate(c,f'LUCIA_RUNTIME_REVISION {REVISION}\nPHASE3_RESULT {{"pass":false,"blocked":true,"reason":"site unavailable"}}',1)[0]=="blocked"
    negative=case("negative","failure","live",marker="COMPLEX_GRAPH",expect="safe_stop")
    line=f'LUCIA_RUNTIME_REVISION {REVISION}\nCOMPLEX_GRAPH '+json.dumps({"pass":True,"blocked":True,"task":{"status":"failed","verified":False}})
    assert evaluate(negative,line,0)[0]=="passed"
    print("PASS independent report evaluator: revision, exit, missing results, failed checks, timeout, unavailable source, expected dependency block")
def atomic_json(path,data):
    tmp=path.with_suffix(".tmp");tmp.write_text(json.dumps(data,indent=2),encoding="utf-8");tmp.replace(path)
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--exe",default="target/debug/browsphere-core.exe")
    parser.add_argument("--tier",action="append",choices=TIERS)
    parser.add_argument("--case",action="append",choices=[c["id"] for c in CASES])
    parser.add_argument("--train-date",default="2026-11-24")
    parser.add_argument("--approved-exa",action="store_true",help="Use only the existing explicitly approved Exa search tool for Level 4; no credentials copied")
    parser.add_argument("--output",default="tests/e2e/artifacts/phase3")
    parser.add_argument("--repeat",type=int,choices=range(1,6),default=1)
    parser.add_argument("--list",action="store_true")
    parser.add_argument("--self-test",action="store_true")
    args=parser.parse_args()
    if args.self_test:test_evaluator();return 0
    selected=[c for c in CASES if (not args.tier or c["tier"] in args.tier) and (not args.case or c["id"] in args.case)]
    if args.list:
        for c in selected:print(c["tier"],c["id"],c["expect"])
        return 0
    if not selected:parser.error("No selected cases")
    datetime.date.fromisoformat(args.train_date)
    exe=(ROOT/args.exe).resolve()
    if not exe.is_file():parser.error("Native executable missing; build before running")
    sha=hash_file(exe)
    run_dir=(ROOT/args.output)/("run-"+datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S%fZ"))
    run_dir.mkdir(parents=True)
    report={"revision":REVISION,"executableSha256":sha,"models":MODELS,"trainDate":args.train_date,"approvedExaLevel4":args.approved_exa,"complete":False,"planned":[c["id"] for c in selected],"results":[]}
    atomic_json(run_dir/"results.json",report)
    print("PHASE3_REPORT "+str(run_dir/"results.json"),flush=True)
    print("PHASE3_BUILD "+sha,flush=True)
    schedule=[dict(c,attempt=n) for n in range(1,args.repeat+1) for c in selected]
    report["planned"]=[{"id":c["id"],"attempt":c["attempt"]} for c in schedule]
    atomic_json(run_dir/"results.json",report)
    for c in schedule:
        if hash_file(exe)!=sha:
            report["abort"]="Executable changed between cases";atomic_json(run_dir/"results.json",report)
            print("PHASE3_ABORT executable changed",flush=True);return 2
        env={k:v for k,v in os.environ.items() if not k.startswith(("LUCIA_","WEBVIEW2_","BROWSER_CDP_PORT"))}
        env.update(MODELS);env.update(c["mode"]);env["LUCIA_TRAIN_DATE"]=args.train_date
        if c["id"]=="level-4":
            if args.approved_exa:env["LUCIA_TEST_APPROVED_EXA"]="1"
            if os.environ.get("LUCIA_CATALOG_RAW_PROBE")=="1":env["LUCIA_CATALOG_RAW_PROBE"]="1"
        kind=c["kind"]
        if kind=="cargo":cmd=["cargo","test","-p","browsphere-core","--offline","--","--test-threads=2"]
        elif kind=="node":cmd=["node","--test","tests/lucia-local-model.test.cjs","tests/lucia-local-profiles.test.cjs","tests/lucia-activity.test.cjs"]
        elif kind=="dom":cmd=[sys.executable,"tests/e2e/test_dom_interaction_patterns.py"]
        elif kind=="roles":cmd=[sys.executable,"tests/e2e/benchmark-local-model-roles.py","--matrix","--output",str(run_dir/("model-role-results-"+str(c["attempt"])+".json"))]
        elif kind=="controls":cmd=["powershell","-NoProfile","-File","tests/e2e/run-real-public-site-controls.ps1","-ExecutablePath",str(exe),"-CdpPort",str(port())]
        else:cmd=["powershell","-NoProfile","-File","tests/e2e/run-lucia-cross-site-live.ps1","-OnlyWikipedia","-ExecutablePath",str(exe),"-CdpPort",str(port())]
        print("PHASE3_START "+c["id"]+" attempt="+str(c["attempt"]),flush=True)
        start=time.monotonic();timed_out=False
        log_path=run_dir/(c["id"]+"-attempt-"+str(c["attempt"])+".log")
        with log_path.open("wb") as out:
            child=subprocess.Popen(cmd,cwd=ROOT,env=env,stdout=out,stderr=subprocess.STDOUT)
            try:rc=child.wait(timeout=c["timeout"])
            except subprocess.TimeoutExpired:
                timed_out=True
                # Terminate ONLY the subprocess tree launched for this case.
                subprocess.run(["taskkill","/PID",str(child.pid),"/T","/F"],capture_output=True,timeout=15)
                rc=child.wait(timeout=15)
        log=log_path.read_text(encoding="utf-8",errors="replace")
        status,checks,reasons,raw=evaluate(c,log,rc,timed_out)
        if hash_file(exe)!=sha:
            status="blocked";reasons.append("Executable changed during case")
        task=(raw or {}).get("task") or {}
        row={"id":c["id"],"attempt":c["attempt"],"tier":c["tier"],"expect":c["expect"],"status":status,"seconds":round(time.monotonic()-start,2),"exitCode":rc,"checks":checks,"reasons":reasons,"agentStatus":task.get("status"),"modelRoles":task.get("modelRoles",[]),"log":log_path.name}
        if raw:row["evidence"]=raw
        report["results"].append(row);atomic_json(run_dir/"results.json",report)
        print("PHASE3_CASE "+json.dumps({k:v for k,v in row.items() if k!="evidence"}),flush=True)
    report["complete"]=True
    report["summary"]={tier:{status:sum(r["tier"]==tier and r["status"]==status for r in report["results"]) for status in ("passed","failed","blocked")} for tier in TIERS}
    atomic_json(run_dir/"results.json",report)
    print("PHASE3_COMPLETE "+json.dumps(report["summary"]),flush=True)
    return int(any(r["status"]!="passed" for r in report["results"]))
if __name__=="__main__":raise SystemExit(main())
