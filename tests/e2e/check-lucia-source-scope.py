"""Low-memory source-level smoke test of Lucia's explicit-source navigation guard.

Extracts the actual Rust functions. Supporting types are stubs; cargo integration
and real-WebView behavior are intentionally NOT claimed.
"""
from pathlib import Path
from tempfile import TemporaryDirectory
import subprocess

root = Path(__file__).resolve().parents[2]

def extract(path, name):
    src = (root / path).read_text(encoding="utf-8")
    start = src.index("pub fn " + name + "(")
    brace = src.index("{", start)
    depth = 0
    for idx in range(brace, len(src)):
        if src[idx] == "{": depth += 1
        elif src[idx] == "}":
            depth -= 1
            if depth == 0: return src[start:idx + 1]
    raise RuntimeError("Cannot extract " + name)

requested = extract("src-tauri/src/agent/policy.rs", "requested_compound_page_entry_url")
guard = extract("src-tauri/src/agent/action_intelligence.rs", "outside_requested_source_page")

scaffold = r"""
extern crate url;
mod agent {
    pub mod deliberation { pub fn read_only(_goal:&str)->bool { true } }
    pub mod dom_extractor {
        pub struct DomNode { pub node_id:String, pub href:Option<String> }
        pub struct DomSnapshot { pub origin:String, pub interactive_nodes:Vec<DomNode> }
    }
    pub mod policy {
        use url::Url;
        pub fn requested_navigation_url(_goal:&str)->Option<Url> { None }
        pub enum BrowserAction {
            Navigate{url:String},
            Click{node_id:String,is_submit:bool},
            Scroll{delta_y:i32}
        }
"""
scaffold += requested + "\n}\n"
scaffold += """pub mod action_intelligence {
   use super::{policy::BrowserAction, dom_extractor::DomSnapshot};
""" + guard + "\n}\n}\n"

smoke = r"""
#[test]
fn source_scope_smoke() {
  use agent::action_intelligence::outside_requested_source_page as bad;
  use agent::policy::BrowserAction;
  use agent::dom_extractor::{DomNode, DomSnapshot};
  let goal="Visit https://docs.example.org/learn/accessibility and report one recommendation, citing an official supporting passage.";
  let p=DomSnapshot{origin:"https://docs.example.org/learn/accessibility".into(),
      interactive_nodes:vec![
       DomNode{node_id:"home".into(),href:Some("https://docs.example.org/".into())},
       DomNode{node_id:"child".into(),href:Some("/learn/accessibility/images".into())}
      ]};
  let nav=|url:&str|BrowserAction::Navigate{url:url.into()};
  assert!(bad(goal,&p,&nav("https://docs.example.org/")));
  assert!(bad(goal,&p,&nav("https://docs.example.org.evil.test/learn/accessibility")));
  assert!(bad(goal,&p,&nav("https://docs.example.org/learn/accessibility-extra")));
  assert!(!bad(goal,&p,&nav("https://docs.example.org/learn/accessibility/")));
  assert!(!bad(goal,&p,&nav("https://docs.example.org/learn/accessibility/images")));
  assert!(bad(goal,&p,&BrowserAction::Click{node_id:"home".into(),is_submit:false}));
  assert!(!bad(goal,&p,&BrowserAction::Click{node_id:"child".into(),is_submit:false}));
  assert!(!bad(goal,&p,&BrowserAction::Scroll{delta_y:300}));
  assert!(!bad("Visit https://docs.example.org/learn/accessibility and compare with others",
       &p,&nav("https://different.example.org/")));
}
"""

with TemporaryDirectory(prefix="lucia-source-scope-") as temp:
    rs = Path(temp) / "smoke.rs"
    exe = Path(temp) / "smoke.exe"
    rs.write_text(scaffold + smoke, encoding="utf-8")
    libs = list((root / "target/debug/deps").glob("liburl-*.rlib"))
    if not libs:
        raise SystemExit("No cached URL dependency; full Cargo required")
    success = False
    for lib in libs:
        cmd = ["rustc", "--edition=2021", "--test", str(rs), "-o", str(exe),
               "--extern", f"url={lib}", "-L", f"dependency={lib.parent}"]
        done = subprocess.run(cmd, capture_output=True, text=True)
        if done.returncode == 0:
            print("RUSTC_EXIT=0; dependency=" + lib.name)
            success = True
            break
        print("Failed cached dependency " + lib.name + ": " + done.stderr[-500:])
    if not success:
        raise SystemExit("Could not link cached URL library")
    done = subprocess.run([str(exe), "--nocapture"], capture_output=True, text=True)
    print("TEST_EXIT=" + str(done.returncode))
    print(done.stdout + done.stderr)
    raise SystemExit(done.returncode)
