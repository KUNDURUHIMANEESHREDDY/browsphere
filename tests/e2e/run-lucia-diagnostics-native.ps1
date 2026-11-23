# Repeatable packaged-Windows-App smoke test. Uses a disposable local profile.
param([int]$Port = 9277)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$artifactDir = Join-Path $PSScriptRoot 'artifacts'
$releaseExe = Join-Path $root 'target\release\browsphere-core.exe'
if (-not (Test-Path $releaseExe)) { throw 'Build the release executable before running the smoke test.' }
if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
    throw "WebView2 CDP port $Port is already in use. Refusing to attach to another browser."
}
New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
$runId = [guid]::NewGuid().ToString('N')
$smokeExe = Join-Path $artifactDir "Browsphere-Diagnostics-$runId.exe"
$localData = Join-Path $artifactDir "dashboard-localappdata-$runId"
$previousLocalData = $env:LOCALAPPDATA
$previousBrowserArgs = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$previousTestLocal = $env:LUCIA_DIAG_LOCALAPPDATA
$previousTestPort = $env:LUCIA_DIAG_CDP_PORT
$appProcess = $null
try {
    Copy-Item -Force $releaseExe $smokeExe
    New-Item -ItemType Directory -Force -Path $localData | Out-Null
    $env:LOCALAPPDATA = $localData
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
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousBrowserArgs
    $env:LUCIA_DIAG_LOCALAPPDATA = $previousTestLocal
    $env:LUCIA_DIAG_CDP_PORT = $previousTestPort
    Remove-Item -Force $smokeExe -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $localData -ErrorAction SilentlyContinue
}
