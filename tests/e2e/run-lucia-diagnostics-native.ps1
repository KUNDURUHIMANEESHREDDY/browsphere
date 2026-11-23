# Repeatable packaged-Windows-App smoke test. Uses a disposable local profile.
param([int]$Port = 9277, [string]$ExecutablePath = '')

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$artifactDir = Join-Path $PSScriptRoot 'artifacts'
$releaseExe = if ($ExecutablePath) { $ExecutablePath } else { Join-Path $root 'target\release\browsphere-core.exe' }
if (-not (Test-Path $releaseExe)) { throw 'Build the native executable before running the smoke test.' }
if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
    throw "WebView2 CDP port $Port is already in use. Refusing to attach to another browser."
}
New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
$runId = [guid]::NewGuid().ToString('N')
$smokeExe = if ($ExecutablePath) { $releaseExe } else { Join-Path $artifactDir "Browsphere-Diagnostics-$runId.exe" }
$localData = Join-Path $artifactDir "dashboard-localappdata-$runId"
$previousLocalData = $env:LOCALAPPDATA
$previousAppData = $env:APPDATA
$previousWebViewData = $env:WEBVIEW2_USER_DATA_FOLDER
$previousBrowserArgs = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$previousTestLocal = $env:LUCIA_DIAG_LOCALAPPDATA
$previousTestPort = $env:LUCIA_DIAG_CDP_PORT
$appProcess = $null
try {
    if (-not $ExecutablePath) { Copy-Item -Force $releaseExe $smokeExe }
    $roaming = Join-Path $localData 'Roaming'
    $webviewProfile = Join-Path $localData 'WebView2'
    New-Item -ItemType Directory -Force -Path $localData,$roaming,$webviewProfile | Out-Null
    $env:APPDATA = $roaming
    $env:LOCALAPPDATA = $localData
    $env:WEBVIEW2_USER_DATA_FOLDER = $webviewProfile
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port --remote-allow-origins=*"
    $appProcess = Start-Process -FilePath $smokeExe -WorkingDirectory $root -PassThru
    $env:LUCIA_DIAG_LOCALAPPDATA = $localData
    $env:LUCIA_DIAG_CDP_PORT = "$Port"
    & node (Join-Path $PSScriptRoot 'lucia-diagnostics-native.cjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native Lucia diagnostics dashboard test failed.' }
    Write-Output 'Native Lucia diagnostics dashboard verification passed.'
} finally {
    if ($appProcess) {
        Stop-Process -Id $appProcess.Id -Force -ErrorAction SilentlyContinue
    }
    $env:LOCALAPPDATA = $previousLocalData
    $env:APPDATA = $previousAppData
    $env:WEBVIEW2_USER_DATA_FOLDER = $previousWebViewData
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousBrowserArgs
    $env:LUCIA_DIAG_LOCALAPPDATA = $previousTestLocal
    $env:LUCIA_DIAG_CDP_PORT = $previousTestPort
    if (-not $ExecutablePath) { Remove-Item -Force $smokeExe -ErrorAction SilentlyContinue }
    Remove-Item -Recurse -Force $localData -ErrorAction SilentlyContinue
}
