"""Repeatable local model role probes, using synthetic public fixtures only.

This tests inference quality and latency, NOT live browser autonomy.
Use --matrix to compare all three text models on every role.
"""
import argparse, base64, io, json, time, urllib.request
from pathlib import Path

BASE = "http://127.0.0.1:11434"
MODELS = ["qwen2.5:3b-instruct", "qwen3.5:0.8b", "lfm3:1.7b"]
ASSIGNED = {"planner": MODELS[0], "action": MODELS[1], "supervisor": MODELS[2], "decomposer": MODELS[2], "vision": "lfm3-vl:2b"}
OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}))
def call(model, prompt, schema, image=None):
    msg = {"role":"user","content":prompt}
    if image: msg["images"]=[image]
    payload={"model":model,"messages":[msg],"stream":False,"think":False,"keep_alive":"30s",
             "format":schema,"options":{"temperature":0,"num_ctx":4096,"num_predict":350}}
    start=time.monotonic()
    try:
        req=urllib.request.Request(BASE+"/api/chat",json.dumps(payload).encode(),{"Content-Type":"application/json"})
        with OPENER.open(req,timeout=75) as r: data=json.load(r)
        result=json.loads(data["message"]["content"])
        return result,round(time.monotonic()-start,2),None
    except Exception as e:
        return None,round(time.monotonic()-start,2),type(e).__name__
def object_schema(props):
    return {"type":"object","properties":props,"required":list(props),"additionalProperties":False}
def enum(*values): return {"type":"string","enum":list(values)}
stage=object_schema({"specialist":enum("research","browser","comparison","form","data"),
                     "objective":{"type":"string","maxLength":200},"success_criteria":{"type":"string","maxLength":200}})
plan_schema=object_schema({"stages":{"type":"array","minItems":1,"maxItems":4,"items":stage}})
def valid_plan(x,required):
    if not isinstance(x,dict) or set(x)!={"stages"}: return False
    rows=x["stages"]
    if not isinstance(rows,list) or not 1<=len(rows)<=4:return False
    if not all(isinstance(r,dict) and set(r)=={"specialist","objective","success_criteria"} and r["specialist"] in ("research","browser","comparison","form","data") and isinstance(r["objective"],str) and 0<len(r["objective"])<=200 and isinstance(r["success_criteria"],str) and 0<len(r["success_criteria"])<=200 for r in rows):return False
    text=" ".join(r["objective"]+" "+r["success_criteria"] for r in rows).lower()
    return all(any(k in text for k in group) for group in required)
def probes():
    # Independent expected answers. Schema validity alone is not a pass.
    return [
      ("planner","multi_source","Plan the original user goal. Return stages only, with concrete observable success criteria. Goal: Compare two public camera manuals. Find battery life and weight for both cameras. Cite the observed source for each. Do not buy anything.",plan_schema,
       lambda x:valid_plan(x,[["battery"],["weight"],["both","each","two"],["cite","source","evidence"]]),None),
      ("decomposer","conditional_read_only","Decompose the user goal into 1-4 stages with observable success criteria, not an answer. Goal: Inspect the public documentation for package Alpha; if that source is unavailable use the explicitly permitted package mirror. Report the installation command and supported runtime version. Do not install anything.",plan_schema,
       lambda x:valid_plan(x,[["install"],["version","runtime"],["mirror","alternative","fallback"],["documentation","source","evidence"]]),None),
      ("action","empty_search","Choose only the necessary next action using the observed controls. Goal: Search for Ada Lovelace. Controls: c1 public empty search input; c2 Search button disabled until query is entered; c3 unrelated help link. Return type, node_id and exact text; never invent a control.",object_schema({"type":enum("type_text","click","complete"),"node_id":enum("c1","c2","c3"),"text":{"type":"string"}}),
       lambda x:x=={"type":"type_text","node_id":"c1","text":"Ada Lovelace"},None),
      ("action","matching_option","Goal: choose Central Station from the observed public autocomplete. c1 option Central East; c2 option Central Station; c3 option Central West. Query already entered. Select one exact observed option, not a second query.",object_schema({"type":enum("click","type_text","complete"),"node_id":enum("c1","c2","c3")}),
       lambda x:x=={"type":"click","node_id":"c2"},None),
      ("supervisor","repeated_action","Diagnose a failed public lookup. User wants package documentation, but the previous two clicks opened Pricing and did not advance. Observed controls: c1 Pricing; c2 Documentation; c3 Sign in. Diagnose the failure and select a distinct grounded control. Never grant permissions.",object_schema({"diagnosis":enum("wrong_destination","missing_input","progress","unknown"),"control":enum("c1","c2","c3","none")}),
       lambda x:x=={"diagnosis":"wrong_destination","control":"c2"},None),
      ("supervisor","missing_evidence","Goal: compare feature and limitation of both libraries. The actual observed evidence has only a feature for each. Diagnose next phase, do not claim completion.",object_schema({"diagnosis":enum("insufficient_evidence","progress","missing_input"),"phase":enum("read_evidence","verify","submit")}),
       lambda x:x=={"diagnosis":"insufficient_evidence","phase":"read_evidence"},None),
    ]
def vision_probe():
    from PIL import Image,ImageDraw,ImageFont
    im=Image.new("RGB",(640,360),"white");d=ImageDraw.Draw(im)
    try: font=ImageFont.truetype("arial.ttf",24)
    except OSError: font=ImageFont.load_default()
    d.rectangle((20,20,620,340),outline="#6b7280",width=3)
    d.text((36,40),"Public document library",fill="black",font=font)
    d.rectangle((36,110,430,158),outline="#6b7280",width=2)
    d.text((48,120),"Search documents",fill="#444444",font=font)
    d.rectangle((445,110,600,158),fill="#234c91")
    d.text((465,120),"Search",fill="white",font=font)
    d.text((36,200),"No results yet",fill="black",font=font)
    buf=io.BytesIO();im.save(buf,format="PNG")
    schema=object_schema({"visible_control":enum("search","payment","login","none"),"results":enum("empty","present","unknown")})
    return ("vision","synthetic_page","Inspect only the screenshot. Which control is visible and are results currently empty or present?",schema,
            lambda x:x=={"visible_control":"search","results":"empty"},base64.b64encode(buf.getvalue()).decode())
def main():
    parser=argparse.ArgumentParser();parser.add_argument("--matrix",action="store_true");parser.add_argument("--output",default="tests/e2e/local-role-benchmark-results.json");args=parser.parse_args()
    with OPENER.open(BASE+"/api/tags",timeout=10) as r: installed={m.get("name",m.get("model")) for m in json.load(r)["models"]}
    rows=[]
    for role,case,prompt,schema,check,image in probes()+[vision_probe()]:
        candidates=MODELS if args.matrix and role!="vision" else [ASSIGNED[role]]
        for model in candidates:
            if model not in installed:
                row={"role":role,"case":case,"model":model,"passed":False,"error":"not_installed"}
            else:
                response,seconds,error=call(model,prompt,schema,image)
                row={"role":role,"case":case,"model":model,"passed":error is None and check(response),"seconds":seconds,"error":error,"response":response}
            rows.append(row);print("ROLE_RESULT "+json.dumps(row),flush=True)
            # Write after each probe, so interrupted runs are clearly partial.
            Path(args.output).write_text(json.dumps({"complete":False,"fixture_only":True,"rows":rows},indent=2),encoding="utf8")
    Path(args.output).write_text(json.dumps({"complete":True,"fixture_only":True,"rows":rows},indent=2),encoding="utf8")
    print("ROLE_BENCHMARK_COMPLETE "+json.dumps({"passed":sum(r["passed"] for r in rows),"total":len(rows)}),flush=True)
    return int(not all(r["passed"] for r in rows))
if __name__=="__main__":raise SystemExit(main())
