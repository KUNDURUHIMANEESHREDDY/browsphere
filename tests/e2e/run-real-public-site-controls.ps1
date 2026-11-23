# Test actual public links and rendered pages in native WebView2, without AI inference.
# Uses an isolated Browsphere profile; no browser account is needed.
param(
    [int]$CdpPort = 9563,
    [string]$ExecutablePath = ""
)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if (-not $ExecutablePath) { $ExecutablePath = Join-Path $root "target\release\browsphere-core.exe" }
if (-not (Test-Path $ExecutablePath)) { throw "Native Browsphere release build missing" }
$profile = Join-Path $env:TEMP ("browsphere-public-controls-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force (Join-Path $profile "Roaming"),(Join-Path $profile "Local"),(Join-Path $profile "WebView2") | Out-Null
$env:APPDATA = Join-Path $profile "Roaming"
$env:LOCALAPPDATA = Join-Path $profile "Local"
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $profile "WebView2"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$CdpPort --remote-allow-origins=*"
$env:LUCIA_CDP_PORT = "$CdpPort"
$app = $null
try {
    $app = Start-Process -FilePath $ExecutablePath -WorkingDirectory $root -PassThru
    Write-Host "Started isolated real-site WebView2 test PID=$($app.Id), CDP=$CdpPort"
    $ready = $false
    for ($i = 0; $i -lt 75; $i++) {
        try {
            $pages = Invoke-RestMethod -Uri "http://127.0.0.1:$CdpPort/json" -TimeoutSec 2
            if (@($pages | Where-Object {$_.url -like '*tauri.localhost*'}).Count -gt 0) {
                $ready = $true; break
            }
        } catch {}
        Start-Sleep -Milliseconds 400
    }
    if (-not $ready) { throw "Native WebView2 CDP port did not open" }
    Set-Location $root
    & node (Join-Path $root "tests\e2e\real-public-site-controls.cjs")
    $exitCode = $LASTEXITCODE
    Write-Host "REAL_SITE_TEST_EXIT=$exitCode"
    if ($exitCode -ne 0) { exit 1 }
    Write-Host "REAL_SITE_TESTS_PASSED"
} finally {
    if ($app) { Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    Remove-Item Env:WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue
    Write-Host "Closed isolated native test app"
}
