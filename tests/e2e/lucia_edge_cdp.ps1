$edge = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
if (-not (Test-Path $edge)) { $edge = "C:\Program Files\Microsoft\Edge\Application\msedge.exe" }
$profile = "C:\Users\himan\Downloads\Browsphere\tests\e2e\edge-cdp-lucia"
$url = "file:///C:/Users/himan/Downloads/Browsphere/ui/public/ai-chat.html"
& $edge --headless=new --no-first-run --disable-extensions --disable-gpu --remote-debugging-port=9231 --remote-allow-origins=* --user-data-dir="$profile" --window-size=650,740 "$url"
