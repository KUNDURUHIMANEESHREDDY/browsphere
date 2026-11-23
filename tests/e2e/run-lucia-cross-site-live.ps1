# Real public-site Lucia/browser tests against the current compiled native app.
# Runs with a disposable WebView2 profile; never uses personal browser credentials.
param([string]$ExecutablePath = "", [int]$CdpPort = 9391, [switch]$OnlyCompatibility)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if (-not $ExecutablePath) {
    $ExecutablePath = Join-Path $env:TEMP "browsphere-native-cross-site-build\release\browsphere-core.exe"
}
if (-not (Test-Path $ExecutablePath)) { throw "Executable missing: $ExecutablePath" }
$profile = Join-Path $env:TEMP ("browsphere-cross-site-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force (Join-Path $profile "Roaming"),(Join-Path $profile "Local"),(Join-Path $profile "WebView2") | Out-Null
$env:APPDATA = Join-Path $profile "Roaming"
$env:LOCALAPPDATA = Join-Path $profile "Local"
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $profile "WebView2"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$CdpPort --remote-allow-origins=*"
$env:LUCIA_CDP_PORT = "$CdpPort"
$env:BROWSER_CDP_PORT = "$CdpPort"
$app = $null
try {
    $app = Start-Process -FilePath $ExecutablePath -WorkingDirectory $root -PassThru
    Write-Host "Isolated native instance: PID=$($app.Id) CDP=$CdpPort"
    $ready = $false
    for ($i=0; $i -lt 90; $i++) {
        try {
            $pages = Invoke-RestMethod -Uri "http://127.0.0.1:$CdpPort/json" -TimeoutSec 2
            if (@($pages | Where-Object { $_.url -like '*tauri.localhost*' }).Count -gt 0) { $ready=$true; break }
        } catch {}
        Start-Sleep -Milliseconds 400
    }
    if (-not $ready) { throw "Native app did not expose an isolated WebView2 CDP target" }
    Set-Location $root
    Write-Host "=== Real-site WebView2 compatibility ==="
    & node (Join-Path $root "tests\e2e\browser-compat.mjs")
    $compatExit=$LASTEXITCODE
    Write-Host "WEB_COMPAT_EXIT=$compatExit"
    if ($OnlyCompatibility) {
        if ($compatExit -ne 0) { exit 1 }
        Write-Host "CROSS_SITE_BROWSER_COMPAT_PASSED"
        return
    }
    Write-Host "=== Real-site Lucia autonomy ==="
    & node (Join-Path $root "tests\e2e\lucia-cross-site-live.cjs")
    $agentExit=$LASTEXITCODE
    Write-Host "LUCIA_CROSS_SITE_EXIT=$agentExit"
    if ($compatExit -ne 0 -or $agentExit -ne 0) { exit 1 }
    Write-Host "CROSS_SITE_TESTS_PASSED"
} finally {
    if ($app) { Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    Remove-Item Env:WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue
    Write-Host "Closed isolated Browsphere test instance"
}
