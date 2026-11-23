# Manual public-site Windows WebView2 regression using the installed local Ollama model.
# Tests native YouTube search -> first observed watch link in a background Lucia tab.
# Uses a temporary browser profile, not the user's saved browsing state.
param([string]$ExecutablePath = "")

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $root
if (-not $ExecutablePath) {
    $ExecutablePath = Join-Path $root 'target\debug\browsphere-core.exe'
}
if (-not (Test-Path $ExecutablePath)) {
    throw "Build Browsphere first. Native executable not found: $ExecutablePath"
}
$profile = Join-Path ([IO.Path]::GetTempPath()) ('lucia-youtube-background-' + [Guid]::NewGuid().ToString('N'))
$env:APPDATA = Join-Path $profile 'Roaming'
$env:LOCALAPPDATA = Join-Path $profile 'Local'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $profile 'WebView2'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9273 --remote-allow-origins=*'
$env:LUCIA_CDP_PORT = '9273'
New-Item -ItemType Directory -Force $env:APPDATA, $env:LOCALAPPDATA, $env:WEBVIEW2_USER_DATA_FOLDER | Out-Null

$appProcess = $null
try {
    $ollama = Invoke-RestMethod -Uri 'http://127.0.0.1:11434/api/tags' -TimeoutSec 5
    if (-not @($ollama.models.name).Contains('qwen2.5:3b-instruct')) {
        throw 'Local qwen2.5:3b-instruct is unavailable. No cloud model will be used.'
    }
    $appProcess = Start-Process -FilePath $ExecutablePath -WorkingDirectory $root -PassThru
    $ready = $false
    for ($i = 0; $i -lt 65; $i++) {
        try {
            $response = Invoke-WebRequest -Uri 'http://127.0.0.1:9273/json' -UseBasicParsing -TimeoutSec 2
            if ($response.StatusCode -eq 200) { $ready = $true; break }
        } catch {}
        Start-Sleep -Milliseconds 450
    }
    if (-not $ready) { throw 'Isolated native browser WebView2 did not become ready.' }
    & node (Join-Path $root 'tests\e2e\lucia-youtube-search.cjs')
    if ($LASTEXITCODE -ne 0) {
        throw "Live YouTube background navigation test failed (exit $LASTEXITCODE)."
    }
    Write-Host 'PASS: live YouTube search -> first video -> observed watch page in background tab'
} finally {
    if ($appProcess) { Stop-Process -Id $appProcess.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    Remove-Item Env:LUCIA_CDP_PORT -ErrorAction SilentlyContinue
}
