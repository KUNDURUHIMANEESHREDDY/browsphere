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
CASES.append(case("structured-table","skill","live",{"LUCIA_PHASE3_CASE":"structured-table"},"STRUCTURED_TABLE_RESULT",timeout=180))
CASES.append(case("structured-chart-fixture","skill","live",{"LUCIA_PHASE3_CASE":"structured-chart-fixture"},"STRUCTURED_CHART_RESULT",timeout=180))
for key in ("pixel-canvas-fixture","pixel-image-fixture"):
    CASES.append(case(key,"skill","live",{"LUCIA_PHASE3_CASE":key},"PIXEL_CHART_RESULT",timeout=240,expect="partial"))
CASES.append(case("vision-fixture","skill","live",{"LUCIA_PHASE3_CASE":"vision-fixture"},"VISION_RESULT",timeout=220))
CASES.append(case("vision-independent","skill","live",{"LUCIA_PHASE3_CASE":"vision-independent"},"VISION_INDEPENDENT_RESULT",timeout=220))
CASES.append(case("vision-real-example","skill","live",{"LUCIA_PHASE3_CASE":"vision-real-example"},"VISION_REAL_RESULT",timeout=180))
CASES.append(case("vision-real-python","skill","live",{"LUCIA_PHASE3_CASE":"nav-python","LUCIA_REQUIRE_COORDINATE":"1"},"PHASE3_RESULT",timeout=180))
CASES.append(case("form-filter","skill","live",{"LUCIA_FILTER_TEST":"1"},"FILTER_RESULT"))
for key in ("compose-wiki","compose-dictionary"):
    CASES.append(case(key,"composition","live",{"LUCIA_PHASE3_CASE":key},"PHASE3_RESULT"))
CASES.append(case("compose-two-sources","composition","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"research"},"COMPLEX_RESEARCH",timeout=240))
CASES.append(case("hybrid-vision-interleaved","composition","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"interleave"},"INTERLEAVED_VISION_MCP",timeout=240))
CASES.append(case("mcp-browser-alternating","composition","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"alternating"},"ALTERNATING_BROWSER_MCP",timeout=240))
CASES.append(case("mcp-browser-specialist","composition","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"specialist"},"RESUMABLE_BROWSER_SPECIALIST",timeout=240))
CASES.append(case("mcp-browser-dynamic","composition","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"specialist-dynamic"},"DYNAMIC_BROWSER_MCP",timeout=260))
CASES.append(case("mcp-browser-adaptive","composition","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"specialist-adaptive"},"ADAPTIVE_BROWSER_MCP",timeout=260))
CASES.append(case("mcp-browser-link-adaptive","composition","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"specialist-link-adaptive"},"ADAPTIVE_LINK_MCP",timeout=260))
CASES.append(case("mcp-browser-entry-approved","composition","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"entry-approved"},"CROSS_ORIGIN_BROWSER_MCP",timeout=260))
CASES.append(case("mcp-browser-entry-rejected","failure","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"entry-rejected"},"CROSS_ORIGIN_BROWSER_MCP",timeout=260,expect="safe_stop"))
for key in ("delayed-link","missing-link"):
    CASES.append(case(key,"failure","live",{"LUCIA_PHASE3_CASE":key},"PHASE3_RESULT",expect="safe_stop" if key=="missing-link" else "completion"))
CASES.append(case("mcp-negative","failure","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"negative"},"COMPLEX_GRAPH",expect="safe_stop"))
CASES.append(case("mcp-outage","failure","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"outage"},"COMPLEX_GRAPH"))
CASES.append(case("mcp-joint-selection","composition","live",{"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"joint"},"COMPLEX_GRAPH",timeout=240))
for n in range(1,9):
    if n==1: mode={"LUCIA_TRAIN_TEST":"1"};marker="TRAIN_RESULT"
    elif n in (5,6):mode={"LUCIA_COMPLEX_TEST":"1","LUCIA_COMPLEX_CASE":"graph" if n==5 else "outage"};marker="COMPLEX_GRAPH"
    else:mode={"LUCIA_LEVEL_TEST":str(n)};marker="LEVEL_RESULT"
    CASES.append(case("level-"+str(n),"autonomy","live",mode,marker,timeout=260))
for key in ("heldout-mdn","heldout-wiki"):
    CASES.append(case(key,"heldout","live",{"LUCIA_PHASE3_CASE":key},"PHASE3_RESULT"))
TIERS=("contracts","skill","composition","failure","autonomy","heldout")
FOCUS_HYBRID_VISION=frozenset(("mcp-browser-link-adaptive","mcp-browser-adaptive","mcp-browser-dynamic","mcp-browser-specialist","mcp-browser-alternating","mcp-joint-selection","hybrid-vision-interleaved","vision-fixture","vision-independent","vision-real-example","vision-real-python","mcp-negative","mcp-outage","level-4","level-5","level-6"))
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
        if isinstance(r,dict):
            checks["casePassed"]=r.get("pass",r.get("passed")) is True
            runner_checks=r.get("checks",{})
            checks["runnerCheckShape"]=isinstance(runner_checks,dict)
            if isinstance(runner_checks,dict):
                checks["runnerChecksIndependent"]=not bool(set(runner_checks).intersection(checks))
                checks.update({k:v for k,v in runner_checks.items() if isinstance(v,bool) and k not in checks})
            if r.get("blocked") and not r.get("task"):
                status="blocked";reason.append(str(r.get("reason","Starting page unavailable"))[:350])
            t=r.get("task") if isinstance(r.get("task"),dict) else {}
            if status!="blocked":
                # The independent oracle cannot trust the runner's pass flag alone.
                checks["terminalEvidencePresent"]=bool(t)
                checks["noUnexpectedApproval"]=t.get("approvalRequired") is False
                if c["expect"]=="safe_stop":
                    checks["expectedTerminalState"]=t.get("status") in ("failed","cancelled","blocked")
                    checks["expectedVerificationState"]=t.get("verified") is False
                elif c["expect"]=="partial":
                    checks["expectedTerminalState"]=t.get("status")=="unverified"
                    checks["expectedVerificationState"]=t.get("verified") is False
                else:
                    checks["expectedTerminalState"]=t.get("status")=="completed"
                    checks["expectedVerificationState"]=t.get("verified") is True
            if c["id"] in ("mcp-negative","mcp-outage","mcp-joint-selection","level-5","level-6") and status!="blocked":
                graph=t.get("graph",{})
                nodes=graph.get("nodes",[]) if isinstance(graph,dict) else []
                audit=r.get("audit",[])
                nodes=nodes if isinstance(nodes,list) and all(isinstance(n,dict) for n in nodes) else []
                audit=audit if isinstance(audit,list) and all(isinstance(a,dict) for a in audit) else []
                dispatched=[a.get("lane") for a in audit if a.get("method")=="tools/call"]
                checks["graphNoUnapprovedDispatch"]="empty" not in dispatched
                if c["expect"]=="safe_stop":
                    checks["graphOnlyPrerequisiteDispatched"]=dispatched==["github"]
                    checks["graphDependentsBlocked"]=bool(nodes) and any(n.get("state")=="failed" for n in nodes) and any(n.get("state")=="blocked" for n in nodes) and all(n.get("attempts")==0 for n in nodes if n.get("state")=="blocked")
                else:
                    checks["graphDispatchOrder"]=dispatched==["github","pypi"]
                    checks["graphEvidenceTransferred"]=r.get("transferred") is True
                    receipts={n.get("receipt") for n in nodes if isinstance(n.get("receipt"),str)}
                    checks["graphVerifiedCompletion"]=len(nodes)>=5 and all(n.get("state")=="completed" for n in nodes) and {"comparison_check","stage_gate","final_report"}.issubset(receipts)
            if c["id"] in ("mcp-browser-adaptive","mcp-browser-link-adaptive","mcp-browser-entry-approved","mcp-browser-entry-rejected") and status!="blocked":
                graph=t.get("graph",{})
                nodes=graph.get("nodes",[]) if isinstance(graph,dict) else []
                nodes=nodes if isinstance(nodes,list) and all(isinstance(n,dict) for n in nodes) else []
                audit=r.get("audit",[])
                calls=[a for a in audit if isinstance(a,dict) and a.get("method")=="tools/call"] if isinstance(audit,list) else []
                after=r.get("after",{})
                after=after if isinstance(after,dict) else {}
                receipts=t.get("approvalReceipts",[])
                rejected=c["id"]=="mcp-browser-entry-rejected"
                link_adaptive=c["id"]=="mcp-browser-link-adaptive"
                adaptive=c["id"]=="mcp-browser-adaptive" or link_adaptive
                checks["independentParentGraph"]=len(nodes)==2 and ((nodes[0].get("state")=="failed" and nodes[1].get("state")=="blocked" and nodes[1].get("attempts")==0) if rejected else all(n.get("state")=="completed" and n.get("attempts")==1 for n in nodes) and [n.get("receipt") for n in nodes]==["stage_gate","tool_check"])
                checks["independentMcpAudit"]=len(calls)==(0 if rejected else 2 if adaptive else 1) and all(a.get("name")=="verify_excerpt" and a.get("args")=={"text":"https://docs.python.org/3/tutorial/index.html" if link_adaptive and i==0 else "Python is an easy to learn, powerful programming language"} for i,a in enumerate(calls))
                checks["independentApprovalReceipt"]=receipts==([] if adaptive else [{"url":"https://docs.python.org/3/","approved":not rejected}])
                checks["independentNativeDestination"]=after.get("observed")==("https://example.com/" if rejected else "https://docs.python.org/3/tutorial/index.html")
                checks["independentFinalReportCount"]=t.get("verificationReports")== (0 if rejected else 1)
                if adaptive:
                    events=t.get("dynamicTools",[])
                    valid=isinstance(events,list) and len(events)==2 and all(isinstance(e,dict) for e in events)
                    if valid:
                        graphs=[e.get("graph",{}) for e in events]
                        children=[g.get("nodes",[]) if isinstance(g,dict) else [] for g in graphs]
                        valid=[e.get("status") for e in events]==["adaptive_inserted","adaptive_verified"] and all(isinstance(ch,list) for ch in children)
                        if valid:
                            active=[n for n in children[0] if isinstance(n,dict) and n.get("kind")=="tool" and n.get("state")=="active"]
                            completed=[n for n in children[1] if isinstance(n,dict) and n.get("kind")=="tool" and n.get("state")=="completed" and n.get("receipt")=="tool_check"]
                            valid=len(active)==1 and len(completed)==1 and active[0].get("id")==completed[0].get("id")
                    checks["independentAdaptiveToolReceipt"]=bool(valid)
            if c["id"] in ("structured-table","structured-chart-fixture") and status!="blocked":
                report=t.get("report",{})
                findings=report.get("findings",[]) if isinstance(report,dict) else []
                findings=[f for f in findings if isinstance(f,dict)] if isinstance(findings,list) else []
                checks["independentStructuredReport"]=isinstance(report,dict) and report.get("status")=="evidence_checked" and t.get("verificationReports")==1 and t.get("actions")==[]
                if c["id"]=="structured-table":
                    checks["independentTableRow"]=any(f.get("url")=="https://www.w3schools.com/html/html_tables.asp" and all(v in f.get("claim","") for v in ("Company = Alfreds Futterkiste","Contact = Maria Anders","Country = Germany")) for f in findings)
                else:
                    checks["independentChartPairs"]=[f.get("quote") for f in findings]==["Jan: 12 mm","Feb: 9 mm"] and all(f.get("url")=="https://example.com/" for f in findings)
            if c["id"] in ("pixel-canvas-fixture","pixel-image-fixture") and status!="blocked":
                report=t.get("report") or {}
                checks["independentPixelProvisional"]=report.get("status")=="partial_evidence" and report.get("findings")==[] and t.get("verificationReports")==1 and t.get("actions")==[]
                summary=report.get("summary","")
                checks["independentPixelAccuracy"]=all(v in summary for v in ("Jan: 12 mm","Feb: 9 mm","(canvas):" if c["id"]=="pixel-canvas-fixture" else "(img):"))
            if t.get("errors"):reason.extend(str(x)[:240] for x in t["errors"][-3:])
            if t.get("approvalRequired"):reason.append("Unexpected approval request; run cancelled without approval")
            if t.get("status") and t["status"]!="completed" and c["expect"] not in ("safe_stop","partial"):
                reason.append("Agent terminal state: "+str(t["status"]))
            if t.get("status")=="completed" and t.get("verified") is not True and c["expect"] not in ("safe_stop","partial"):reason.append("No independently checked report")
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
    ok=f'LUCIA_RUNTIME_REVISION {REVISION}\nPHASE3_RESULT '+json.dumps({"pass":True,"checks":{"fact":True},"task":{"status":"completed","verified":True,"approvalRequired":False}})
    assert evaluate(c,ok,0)[0]=="passed"
    assert evaluate(c,ok,1)[0]=="failed"
    assert evaluate(c,"PHASE3_RESULT {\"pass\":true}",0)[0]=="blocked"
    assert evaluate(c,f"LUCIA_RUNTIME_REVISION {REVISION}",0)[0]=="failed"
    assert evaluate(c,ok.replace('"fact": true','"fact": false'),0)[0]=="failed"
    assert evaluate(c,ok,0,True)[0]=="failed"
    assert evaluate(c,f'LUCIA_RUNTIME_REVISION {REVISION}\nPHASE3_RESULT {{"pass":false,"blocked":true,"reason":"site unavailable"}}',1)[0]=="blocked"
    negative=case("negative","failure","live",marker="COMPLEX_GRAPH",expect="safe_stop")
    line=f'LUCIA_RUNTIME_REVISION {REVISION}\nCOMPLEX_GRAPH '+json.dumps({"pass":True,"blocked":True,"task":{"status":"failed","verified":False,"approvalRequired":False}})
    assert evaluate(negative,line,0)[0]=="passed"
    def claimed_pass(c,task,extra=None):
        result={"pass":True,"checks":{"fact":True},"task":task}
        result.update(extra or {})
        return evaluate(c,f"LUCIA_RUNTIME_REVISION {REVISION}\n{c['marker']} "+json.dumps(result),0)[0]
    for bad in (None,[],{},{"status":"completed","verified":False,"approvalRequired":False},
                {"status":"failed","verified":True,"approvalRequired":False},
                {"status":"completed","verified":True,"approvalRequired":True},
                {"status":"completed","verified":True},
                {"status":"completed","verified":"true","approvalRequired":False}):
        assert claimed_pass(c,bad)=="failed"
    assert claimed_pass(negative,{"status":"completed","verified":True,"approvalRequired":False})=="failed"
    assert claimed_pass(negative,{"status":"failed","verified":True,"approvalRequired":False})=="failed"
    assert claimed_pass(negative,{"status":"failed","verified":False,"approvalRequired":True})=="failed"
    good_task={"status":"completed","verified":True,"approvalRequired":False}
    assert claimed_pass(c,good_task,{"checks":{"runnerExit":True}})=="failed"
    assert claimed_pass(c,good_task,{"checks":[]})=="failed"
    assert evaluate(c,f"LUCIA_RUNTIME_REVISION {REVISION}\nPHASE3_RESULT [1]",0)[0]=="failed"
    actual_negative=case("mcp-negative","failure","live",marker="COMPLEX_GRAPH",expect="safe_stop")
    failed_task={"status":"failed","verified":False,"approvalRequired":False,"graph":{"nodes":[{"state":"failed","attempts":1},{"state":"blocked","attempts":0}]}}
    assert claimed_pass(actual_negative,failed_task,{"audit":[{"lane":"github","method":"tools/call"}]})=="passed"
    assert claimed_pass(actual_negative,failed_task,{"audit":[{"lane":"github","method":"tools/call"},{"lane":"pypi","method":"tools/call"}]})=="failed"
    positive=case("mcp-outage","failure","live",marker="COMPLEX_GRAPH")
    good_task["graph"]={"nodes":[{"state":"completed","receipt":receipt} for receipt in ("tool_check","tool_check","comparison_check","stage_gate","final_report")]}
    audit=[{"lane":lane,"method":"tools/call"} for lane in ("github","pypi")]
    assert claimed_pass(positive,good_task,{"audit":audit,"transferred":True})=="passed"
    assert claimed_pass(positive,good_task,{"audit":audit[::-1],"transferred":True})=="failed"
    assert claimed_pass(positive,good_task,{"audit":audit,"transferred":False})=="failed"

    entry=case("mcp-browser-entry-rejected","failure","live",marker="CROSS_ORIGIN_BROWSER_MCP",expect="safe_stop")
    failure={"status":"failed","verified":False,"approvalRequired":False,"approvalReceipts":[{"url":"https://docs.python.org/3/","approved":False}],"verificationReports":0,"graph":{"nodes":[{"state":"failed","attempts":1},{"state":"blocked","attempts":0}]}}
    assert claimed_pass(entry,failure,{"audit":[],"after":{"observed":"https://example.com/"}})=="passed"
    assert claimed_pass(entry,failure,{"audit":[{"method":"tools/call"}],"after":{"observed":"https://example.com/"}})=="failed"
    assert claimed_pass(entry,failure,{"audit":[],"after":{"observed":"https://docs.python.org/3/"}})=="failed"
    print("PASS independent report evaluator: revision, exit, missing results, failed checks, timeout, unavailable source, terminal state, verified evidence, approval and expected dependency block")
def atomic_json(path,data):
    tmp=path.with_suffix(".tmp");tmp.write_text(json.dumps(data,indent=2),encoding="utf-8");tmp.replace(path)
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--exe",default="target/debug/browsphere-core.exe")
    parser.add_argument("--tier",action="append",choices=TIERS)
    parser.add_argument("--focus",choices=("hybrid-vision",),help="Run the focused browser-MCP and visual perception acceptance suite")
    parser.add_argument("--case",action="append",choices=[c["id"] for c in CASES])
    parser.add_argument("--train-date",default="2026-11-24")
    parser.add_argument("--approved-exa",action="store_true",help="Use only the existing explicitly approved Exa search tool for Level 4; no credentials copied")
    parser.add_argument("--output",default="tests/e2e/artifacts/phase3")
    parser.add_argument("--repeat",type=int,choices=range(1,6),default=1)
    parser.add_argument("--list",action="store_true")
    parser.add_argument("--self-test",action="store_true")
    args=parser.parse_args()
    if args.self_test:test_evaluator();return 0
    if args.focus and (args.tier or args.case):parser.error("--focus cannot be combined with --tier or --case")
    selected=[c for c in CASES if (not args.tier or c["tier"] in args.tier) and (not args.case or c["id"] in args.case) and (not args.focus or c["id"] in FOCUS_HYBRID_VISION)]
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
        elif kind=="node":cmd=["node","--test","tests/lucia-local-model.test.cjs","tests/lucia-local-profiles.test.cjs","tests/lucia-activity.test.cjs","tests/workflow-native-link.test.cjs"]
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
