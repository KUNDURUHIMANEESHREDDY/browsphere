Set-Location 'C:\Users\himan\Downloads\Browsphere'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9228 --remote-allow-origins=*'
cargo tauri dev
