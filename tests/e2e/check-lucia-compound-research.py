"""Low-memory smoke test of actual Rust source-scoping functions.

This extracts production Rust helpers and compiles them against cached URL
dependencies. It is NOT a replacement for Cargo integration or live Qwen tests.
"""
from pathlib import Path
from tempfile import TemporaryDirectory
import subprocess

root = Path(__file__).resolve().parents[2]


def extract(path: str, name: str) -> str:
    data = (root / path).read_text(encoding="utf-8")
    start = data.index("pub fn " + name + "(") if "pub fn " + name + "(" in data else data.index("fn " + name + "(")
    opening = data.index("{", start)
    depth = 0
    for index in range(opening, len(data)):
        if data[index] == "{":
            depth += 1
        elif data[index] == "}":
            depth -= 1
            if depth == 0:
                return data[start : index + 1]
    raise RuntimeError(f"Function {name} did not close")


policy = extract("src-tauri/src/agent/policy.rs", "requested_compound_page_entry_url")
source = extract("src-tauri/src/agent/evidence_verifier.rs", "compound_research_source")
web_url = extract("src-tauri/src/agent/evidence_verifier.rs", "web_url")

scaffold = r"""
extern crate url;
#[derive(Clone)]
pub struct SourceEvidence { pub url:String, pub excerpt:String }
mod agent {
  pub mod deliberation {
    pub fn read_only(goal:&str)->bool {
      goal.contains("report") && !goal.contains("purchase")
    }
  }
  pub mod policy {
    use url::Url;
    pub fn requested_navigation_url(_goal:&str)->Option<Url>{None}
""" + policy + r"""
  }
  pub mod evidence_verifier {
    use url::Url;
    use crate::SourceEvidence;
""" + web_url + "\n" + source + r"""
  }
}
#[test]
fn selected_source_must_match_observed_deep_page() {
    use agent::evidence_verifier::compound_research_source as select;
    let goal="Visit https://web.dev/learn/accessibility and report one specific recommendation, citing the official guide.";
    let sample=|url:&str|SourceEvidence{url:url.into(),excerpt:"For accessibility, give users clear labels and explain what each interactive control does.".into()};
    for wrong in ["https://web.dev/","https://web.dev/learn/accessibility-other",
                  "https://other.example/learn/accessibility",
                  "https://web.dev/learn/accessibility?injected=1"] {
        assert!(select(goal,&[sample(wrong)]).is_none(),"{wrong}");
    }
    let chosen=select(goal,&[sample("https://web.dev/"),
            sample("https://web.dev/learn/accessibility/")]).unwrap();
    assert_eq!(chosen.url,"https://web.dev/learn/accessibility/");
    assert!(select("Visit https://web.dev/learn/accessibility and purchase a subscription",
            &[sample("https://web.dev/learn/accessibility")]).is_none());
}
"""

with TemporaryDirectory(prefix="lucia-compound-source-") as folder:
    folder = Path(folder)
    file = folder / "source_check.rs"
    binary = folder / "source_check.exe"
    file.write_text(scaffold, encoding="utf-8")
    candidates = sorted((root / "target/debug/deps").glob("liburl-*.rlib"))
    if not candidates:
        raise SystemExit("Cached URL dependency missing; full Cargo still required")
    for dep in candidates:
        build = subprocess.run(["rustc", "--edition=2021", "--test", str(file),
                                "-o", str(binary), "--extern", "url=" + str(dep),
                                "-L", "dependency=" + str(dep.parent)],
                               capture_output=True, text=True)
        if build.returncode == 0:
            print("RUSTC_EXIT=0; cached_url=" + dep.name)
            checked = subprocess.run([str(binary)], capture_output=True, text=True)
            print(checked.stdout)
            if checked.stderr:
                print(checked.stderr)
            raise SystemExit(checked.returncode)
    print(build.stderr)
    raise SystemExit("No compatible cached URL dependency")
