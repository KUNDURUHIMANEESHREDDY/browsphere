import glob, os, sqlite3
files = glob.glob(os.path.join(os.environ["TEMP"], "lucia-song-full-*", "Local", "BrowsphereAI", "Browsphere.sqlite"))
if not files:
    raise SystemExit("No isolated test database")
path = max(files, key=os.path.getmtime)
db = sqlite3.connect(path)
for (table,) in db.execute("select name from sqlite_master where type='table'"):
    if "agent" not in table.lower():
        continue
    fields = [row[1] for row in db.execute('pragma table_info("' + table + '")')]
    print("AGENT_TABLE",table,fields)
    if "goal" in fields:
        requested = [n for n in ("run_id", "id", "goal", "status") if n in fields]
        query = "select " + ",".join('"' + n + '"' for n in requested) + ' from "' + table + '" limit 10'
        print("ROWS", db.execute(query).fetchall())
