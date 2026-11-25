# Real public-site Lucia/browser tests against the current compiled native app.
# Runs with a disposable WebView2 profile; never uses personal browser credentials.
param([string]$ExecutablePath = "", [int]$CdpPort = 9391, [switch]$OnlyCompatibility, [switch]$OnlyWikipedia, [switch]$OnlyPythonTutorial, [switch]$UseSavedCloud)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if (-not $ExecutablePath) {
    $localRelease = Join-Path $root "target\release\browsphere-core.exe"
    $ExecutablePath = if (Test-Path $localRelease) { $localRelease } else {
        Join-Path $env:TEMP "browsphere-native-cross-site-build\release\browsphere-core.exe"
    }
}
if (-not (Test-Path $ExecutablePath)) { throw "Executable missing: $ExecutablePath" }
if ($UseSavedCloud) {
    # Only copy the public endpoint and model name from saved app settings.
    # The app reads the provider credential itself from Windows Credential Manager.
    $savedDb = Join-Path $env:LOCALAPPDATA "BrowsphereAI\Browsphere.sqlite"
    if (-not (Test-Path $savedDb)) { throw "Saved Browsphere cloud settings not found" }
    $values = & python (Join-Path $PSScriptRoot "read-saved-cloud-settings.py") $savedDb | ConvertFrom-Json
    if (-not $values.endpoint -or -not $values.model) { throw "Cloud endpoint/model are not configured" }
    $env:LUCIA_TEST_CLOUD_ENDPOINT = [string]$values.endpoint
    $env:LUCIA_TEST_CLOUD_MODEL = [string]$values.model
    Write-Host "Using saved cloud model with disposable browser profile (API key is never copied)"
}
$profile = Join-Path $env:TEMP ("browsphere-cross-site-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force (Join-Path $profile "Roaming"),(Join-Path $profile "Local"),(Join-Path $profile "WebView2") | Out-Null
# Explicit authenticated search benchmark: copy only the exact approved tool
# metadata, never a credential, cookie, or personal browser profile.
if ($env:LUCIA_TEST_APPROVED_EXA -eq "1") {
    if ($env:LUCIA_LEVEL_TEST -ne "4") { throw "Authenticated search flag restricted to Level 4" }
    $saved = Join-Path $env:LOCALAPPDATA "BrowsphereAI\mcp-servers.json"
    $config = Get-Content -Raw -LiteralPath $saved | ConvertFrom-Json
    $entry = $config.'exa-oauth'
    if ($entry.url -ne "https://mcp.exa.ai/mcp/oauth" -or $entry.read_only_tools -notcontains "web_search_exa") { throw "Authenticated Exa search has not been approved" }
    $dir = Join-Path $profile "Local\BrowsphereAI"
    New-Item -ItemType Directory -Force $dir | Out-Null
    [IO.File]::WriteAllText((Join-Path $dir "mcp-servers.json"),'{"exa-oauth":{"url":"https://mcp.exa.ai/mcp/oauth","read_only_tools":["web_search_exa"]}}',(New-Object Text.UTF8Encoding($false)))
    Write-Host "LEVEL4 approved authenticated Exa only; credential remains in Windows Credential Manager"
}
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
    $compatExit=0
    if (-not $OnlyWikipedia -and -not $OnlyPythonTutorial) {
        Write-Host "=== Real-site WebView2 compatibility ==="
        & node (Join-Path $root "tests\e2e\browser-compat.mjs")
        $compatExit=$LASTEXITCODE
    }
    if ($OnlyWikipedia) { $env:LUCIA_ONLY_WIKIPEDIA="1" }
    if ($OnlyPythonTutorial) { $env:LUCIA_ONLY_PYTHON_TUTORIAL="1" }
    Write-Host "WEB_COMPAT_EXIT=$compatExit"
    if ($OnlyCompatibility) {
        if ($compatExit -ne 0) { exit 1 }
        Write-Host "CROSS_SITE_BROWSER_COMPAT_PASSED"
        return
    }
    Write-Host "=== Real-site Lucia autonomy ==="
    & node (Join-Path $root "tests\e2e\lucia-demo-w1.cjs")
    $agentExit=$LASTEXITCODE
    Write-Host "LUCIA_CROSS_SITE_EXIT=$agentExit"
    if ($compatExit -ne 0 -or $agentExit -ne 0) { exit 1 }
    Write-Host "CROSS_SITE_TESTS_PASSED"
} finally {
    if ($app) { Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    Remove-Item Env:WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue
    Remove-Item Env:LUCIA_ONLY_WIKIPEDIA -ErrorAction SilentlyContinue
    Remove-Item Env:LUCIA_ONLY_PYTHON_TUTORIAL -ErrorAction SilentlyContinue
    Remove-Item Env:LUCIA_TEST_CLOUD_ENDPOINT -ErrorAction SilentlyContinue
    Remove-Item Env:LUCIA_TEST_CLOUD_MODEL -ErrorAction SilentlyContinue
    Write-Host "Closed isolated Browsphere test instance"
}
