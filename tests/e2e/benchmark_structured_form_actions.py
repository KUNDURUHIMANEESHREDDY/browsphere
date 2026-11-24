"""Real local-model evaluation of independent, synthetic form observations.
No booking, website actions, fine-tuning, or private data."""
import json, time, urllib.request
import sys
MODEL = sys.argv[1] if len(sys.argv)>1 else "qwen3.5:0.8b"
GOAL = "Choose English if prompted. Find trains from Bengaluru to New Delhi on 2026-11-24 after 6 PM. Search only, never book or pay."
def control(id, tag, label, role="", state=""):
    return dict(id=id,tag=tag,role=role or tag,label=label,search_state=state)
cases = [
    ("language dialog","Active dialog: Choose preferred language.",[control("c1","button","Hindi"),control("c2","button","English")],{"type":"click","node_id":"c2"}),
    ("source input","Form: origin and destination both empty.",[control("c1","input","From station","searchbox","empty"),control("c2","input","To station","searchbox","empty"),control("c3","input","Journey date","textbox","read_only")],{"type":"type_text","node_id":"c1","text":"Bengaluru"}),
    ("source suggestion","From typed Bengaluru. Select a matching visible station before entering the destination.",[control("c1","li","Bengaluru City Jn","option"),control("c2","li","Bangalore Cantonment","option"),control("c3","input","To station","searchbox","empty")],{"type":"click","node_id":"c1"}),
    ("destination input","Source selected. Destination empty.",[control("c1","input","From station","searchbox","filled"),control("c2","input","To station","searchbox","empty"),control("c3","input","Journey date","textbox","read_only")],{"type":"type_text","node_id":"c2","text":"New Delhi"}),
    ("date picker","Both stations selected. Journey date not yet selected.",[control("c1","input","Journey date","textbox","read_only"),control("c2","button","Search trains")],{"type":"click","node_id":"c1"}),
    ("calendar day","Calendar November 2026 is open. Choose requested date.",[control("c1","button","November 23, 2026"),control("c2","button","November 24, 2026"),control("c3","button","November 25, 2026")],{"type":"click","node_id":"c2"}),
    ("submit search","Both stations and 24 November 2026 selected. Search public results.",[control("c1","button","Search trains"),control("c2","button","Book now")],{"type":"click","node_id":"c1"}),
]
def variant(kind, props):
    fields={"type":{"type":"string","enum":[kind]},**props}
    return {"type":"object","properties":fields,"required":list(fields),"additionalProperties":False}
def enum(ids):
    return {"type":"string","enum":ids}
def main():
    passed=0
    for name,state,controls,expected in cases:
        clicks=[c["id"] for c in controls if c["tag"] in ("button","li") or c["search_state"]=="read_only"]
        texts=[c["id"] for c in controls if c["tag"]=="input" and c["search_state"]!="read_only"]
        variants=[]
        if clicks: variants.append(variant("click",{"node_id":enum(clicks)}))
        if texts: variants.append(variant("type_text",{"node_id":enum(texts),"text":{"type":"string"}}))
        schema={"type":"object","properties":{"action":{"anyOf":variants}},"required":["action"],"additionalProperties":False}
        prompt=f"Original user goal: {GOAL}\nCurrent observed state: {state}\nFresh controls: {json.dumps(controls)}\nHandle a blocking dialog first. Fill each distinct required field with its corresponding user-supplied value. After typing, click a matching observed suggestion. Click read-only date fields to open their calendar. Never book or pay. Choose the necessary next action."
        body={"model":MODEL,"system":"You are Lucia's browser action proposer. Output only one JSON action conforming to the supplied schema. Use only the current observed controls.","prompt":prompt,"stream":False,"format":schema,"think":False,"options":{"temperature":0,"num_predict":192,"num_ctx":4096}}
        start=time.monotonic()
        try:
            req=urllib.request.Request("http://127.0.0.1:11434/api/generate",data=json.dumps(body).encode(),headers={"Content-Type":"application/json"})
            with urllib.request.urlopen(req,timeout=40) as r: result=json.load(r)
            action=json.loads(result.get("response","")).get("action",{})
            ok=isinstance(action,dict) and all(str(action.get(k,"")).casefold()==str(v).casefold() for k,v in expected.items())
            passed+=ok
            print(json.dumps({"case":name,"passed":ok,"action":action,"seconds":round(time.monotonic()-start,2)}),flush=True)
        except Exception as e:
            print(json.dumps({"case":name,"passed":False,"error":str(e)}),flush=True)
    print(json.dumps({"model":MODEL,"passed":passed,"total":len(cases)}),flush=True)
    return 0 if passed==len(cases) else 1
if __name__=="__main__":
    raise SystemExit(main())
