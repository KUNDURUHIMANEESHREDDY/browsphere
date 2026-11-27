"""Compile-test the exact Lucia recheck helper, without linking the full Tauri app.

This is a low-memory source-level smoke test, NOT a cargo integration test.
"""
from pathlib import Path
from tempfile import TemporaryDirectory
import subprocess

root = Path(__file__).resolve().parents[2]
src = (root / "src-tauri/src/agent/loop_runner.rs").read_text(encoding="utf-8")

def extract(name):
    start = src.index("fn " + name + "(")
    brace = src.index("{", start)
    depth = 0
    for idx in range(brace, len(src)):
        if src[idx] == "{":
            depth += 1
        elif src[idx] == "}":
            depth -= 1
            if depth == 0:
                return src[start:idx + 1]
    raise ValueError("Unclosed Rust function " + name)

helper = extract("bounded_verification_recheck")
case = extract("verification_recheck_is_bounded_and_never_exempts_browser_actions")
scaffold = """#[derive(PartialEq, Eq, Clone, Copy)]
enum Specialist { Browser, Research, Verification }
enum BrowserAction { Complete { summary: String }, Navigate { url: String } }
"""
with TemporaryDirectory(prefix="lucia-recheck-smoke-") as directory:
    rs = Path(directory) / "test.rs"
    executable = Path(directory) / "test.exe"
    rs.write_text(scaffold + helper + "\n#[test]\n" + case + "\n", encoding="utf-8")
    compile_result = subprocess.run(
        ["rustc", "--edition=2021", "--test", str(rs), "-o", str(executable)],
        capture_output=True, text=True)
    print("RUSTC_EXIT=" + str(compile_result.returncode))
    print(compile_result.stdout + compile_result.stderr)
    if compile_result.returncode:
        raise SystemExit(compile_result.returncode)
    result = subprocess.run([str(executable), "--nocapture"],
                            capture_output=True, text=True)
    print("TEST_EXIT=" + str(result.returncode))
    print(result.stdout + result.stderr)
    raise SystemExit(result.returncode)
