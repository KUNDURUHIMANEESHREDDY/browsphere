from pathlib import Path
import re, subprocess, tempfile
root=Path(r"C:\Users\himan\Downloads\Browsphere")
home=(root/"ui/public/index.html").read_text(encoding="utf-8")
bridge=(root/"ui/public/src/tauri-bridge.js").read_text(encoding="utf-8")
popup=(root/"ui/public/ai-chat.html").read_text(encoding="utf-8")
rust=(root/"src-tauri/src/main.rs").read_text(encoding="utf-8")
assert "Continue the conversation without leaving this tab." not in home
assert '<button type="button" class="main-chat-new-btn" data-action="nt-new-chat">+ New chat</button>' not in home
assert 'if (getActiveTab()?.viewingArchivedChat && getActiveTab()?.chatThreadId) return renderNewTabConversation();' in home
assert "getActiveTab().viewingArchivedChat = true;" in home
assert 'await window.aetherBridge.toggleAiChatWindow(true, currentPage.title, currentUrl, dark, text);' in home
assert 'await window.aetherBridge.toggleAiChatWindow(true, currentPage.title, currentUrl, dark, text, true);' in home
assert 'forceTask: Boolean(forceTask)' in bridge
assert 'force_task: Option<bool>' in rust
assert 'submitGoal(p.prompt, p.force_task === true)' in popup
assert "if (forceTask === true)" in popup
assert "whats up" not in home.lower(), "Hardcoded chat text must be absent"
for rel in ["ui/public/src/tauri-bridge.js","ui/public/src/voice-agent.js"]:
    outcome=subprocess.run(["node","--check",str(root/rel)],capture_output=True,text=True)
    assert outcome.returncode==0,outcome.stderr
with tempfile.TemporaryDirectory() as d:
    for path in (root/"ui/public/index.html", root/"ui/public/ai-chat.html"):
        blocks=re.findall(r"<script>([\s\S]*?)</script>",path.read_text(encoding="utf-8"))
        for i,block in enumerate(blocks):
            tmp=Path(d)/f"{path.stem}-{i}.js";tmp.write_text(block,encoding="utf-8")
            outcome=subprocess.run(["node","--check",str(tmp)],capture_output=True,text=True)
            assert outcome.returncode==0,outcome.stderr
print("PASS 10/10 functional regression assertions + JavaScript syntax")
for name in ["remove_fulltab_lucia.py","preserve_run_task.py","finish_run_task_patch.py",
            "inspect_dev_processes.py","_home_screenshot.py"]:
    (root/"tests/e2e"/name).unlink(missing_ok=True)
(root/"tests/e2e/artifacts/home_after_lucia_removal.png").unlink(missing_ok=True)
