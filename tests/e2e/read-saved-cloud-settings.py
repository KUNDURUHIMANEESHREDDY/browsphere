"""Read only the public provider endpoint/model for disposable Lucia E2E tests.

Never export credentials or copy the user's browser database into the test profile.
"""
import json
import sqlite3
import sys
from pathlib import Path
from urllib.parse import urlsplit

if len(sys.argv) != 2:
    raise SystemExit("Expected saved Browsphere SQLite path")
# Opening SQLite explicitly in read-only mode protects the user's settings.
source_uri = Path(sys.argv[1]).resolve().as_uri() + "?mode=ro"
with sqlite3.connect(source_uri, uri=True) as db:
    result = db.execute("SELECT value_json FROM app_settings WHERE key=?", ("router_config",)).fetchone()
settings = json.loads(result[0]) if result else {}
endpoint = settings.get("litellm_url") or ""
model = settings.get("litellm_model") or ""
parsed = urlsplit(endpoint)
if not model or not parsed.hostname or parsed.username or parsed.password or parsed.query or parsed.fragment:
    raise SystemExit("Saved model URL or model name is invalid")
if parsed.scheme != "https" and not (parsed.scheme == "http" and parsed.hostname in {"127.0.0.1", "localhost", "::1"}):
    raise SystemExit("Saved endpoint must be HTTPS (except localhost)")
print(json.dumps({"endpoint": endpoint, "model": model}))
