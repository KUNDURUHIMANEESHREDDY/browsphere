import sqlite3, sys
from pathlib import Path
from datetime import datetime, timezone, timedelta
db=Path.home()/"AppData"/"Local"/"BrowsphereAI"/"Browsphere.sqlite"
fixture_id="browsphere-e2e-memory-fixture"
if not db.exists():
    raise SystemExit("Database not found; refuse creating a replacement")
with sqlite3.connect(str(db),timeout=10) as conn:
    if sys.argv[1]=="install":
        cols={row[1] for row in conn.execute("PRAGMA table_info(agent_memories)")}
        expected={"memory_kind","memory_key","evidence_count","expires_at"}
        assert expected.issubset(cols), "Memory migration not applied"
        now=datetime.now(timezone.utc)
        conn.execute("DELETE FROM agent_memories WHERE id=?", (fixture_id,))
        conn.execute("""INSERT INTO agent_memories (id,scope,content,created_at,updated_at,memory_kind,memory_key,evidence_count,expires_at)
            VALUES (?,?,?,?,?,?,?,?,?)""",(
            fixture_id,"docs.example.com","E2E test only: learned a documentation lookup task",
            now.isoformat(),now.isoformat(),"workflow","e2e:test-memory",1,(now+timedelta(days=1)).isoformat()))
        print("FIXTURE_INSTALLED")
    elif sys.argv[1]=="cleanup":
        n=conn.execute("DELETE FROM agent_memories WHERE id=?", (fixture_id,)).rowcount
        print("FIXTURE_CLEANUP_DELETED",n)
    elif sys.argv[1]=="verify":
        n=conn.execute("SELECT COUNT(*) FROM agent_memories WHERE id=?", (fixture_id,)).fetchone()[0]
        print("FIXTURE_EXISTS",n)
        assert n==0
    else: raise SystemExit("Expected install, verify, or cleanup")
