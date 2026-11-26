import glob,os,sqlite3
files=glob.glob(os.path.join(os.environ["TEMP"],"lucia-song-after-fix-*","Local","BrowsphereAI","Browsphere.sqlite"))
db=sqlite3.connect(max(files,key=os.path.getmtime))
for row in db.execute("select run_id,step_index,active_origin,model_route,action_json,decision_json,outcome from agent_traces order by created_at"):
    print("TRACE",tuple(str(x)[:250] if x is not None else "" for x in row))
