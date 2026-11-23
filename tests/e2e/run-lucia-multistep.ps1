# Native Lucia end-to-end tests with a loopback-only model and fixture.
# Never uses the user's actual browser data, accounts, or API credentials.
param([string]$ExecutablePath = "")

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $root

if (-not $ExecutablePath) {
    $candidates = @(
        (Join-Path $root 'target\release\browsphere-core.exe'),
        (Join-Path $root 'target\debug\browsphere-core.exe')
    )
    $ExecutablePath = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $ExecutablePath -or -not (Test-Path $ExecutablePath)) {
    throw 'Browsphere native executable not found. Build the app first.'
}

$profile = Join-Path ([IO.Path]::GetTempPath()) ('lucia-native-e2e-' + [Guid]::NewGuid().ToString('N'))
$env:APPDATA = Join-Path $profile 'Roaming'
$env:LOCALAPPDATA = Join-Path $profile 'Local'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $profile 'WebView2'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9272 --remote-allow-origins=*'
New-Item -ItemType Directory -Force $env:APPDATA, $env:LOCALAPPDATA, $env:WEBVIEW2_USER_DATA_FOLDER | Out-Null

$modelProcess = $null
$appProcess = $null
$serverOut = Join-Path $profile 'mock-model.stdout.log'
$serverErr = Join-Path $profile 'mock-model.stderr.log'

function Wait-ForLoopback($url, $tries = 35) {
    for ($i=0; $i -lt $tries; $i++) {
        try {
            $response = Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 2
            if ($response.StatusCode -eq 200) { return }
        } catch {}
        Start-Sleep -Milliseconds 400
    }
    throw "Local test dependency did not respond: $url"
}

try {
    $serverScript = Join-Path $root 'tests\e2e\lucia-local-model-server.cjs'
    $modelProcess = Start-Process -FilePath 'node' -ArgumentList @('"' + $serverScript + '"') -WorkingDirectory $root -PassThru -RedirectStandardOutput $serverOut -RedirectStandardError $serverErr

    Wait-ForLoopback 'http://127.0.0.1:18766/health'
    Wait-ForLoopback 'http://127.0.0.1:18764/health'

    Write-Host 'Starting isolated Browsphere native WebView2 fixture tests'
    $appProcess = Start-Process -FilePath $ExecutablePath -WorkingDirectory $root -PassThru
    Wait-ForLoopback 'http://127.0.0.1:9272/json' 55

    $cli = Join-Path $root 'tests\e2e\cli-browsphere.cjs'
    & node $cli --json doctor
    if ($LASTEXITCODE -ne 0) { throw 'Native Browsphere test dependencies failed CLI health checks' }
    & node $cli --json test
    if ($LASTEXITCODE -ne 0) {
        throw "Lucia multi-step native CLI test failed with exit code $LASTEXITCODE"
    }
    Write-Host 'PASS: multi-step native browsing, recovery, and evidence checks'
} finally {
    if ($appProcess) { Stop-Process -Id $appProcess.Id -Force -ErrorAction SilentlyContinue }
    if ($modelProcess) { Stop-Process -Id $modelProcess.Id -Force -ErrorAction SilentlyContinue }
    if (Test-Path $serverOut) { Get-Content $serverOut -Tail 25 }
    if (Test-Path $serverErr) {
        $errors = Get-Content $serverErr -Tail 25
        if ($errors) { Write-Warning ($errors -join [Environment]::NewLine) }
    }
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
}
