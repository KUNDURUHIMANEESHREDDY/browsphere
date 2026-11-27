# Native Lucia end-to-end tests with a loopback-only model and fixture.
# Never uses the user's actual browser data, accounts, or API credentials.
param(
    [string]$ExecutablePath = "",
    [switch]$CloudCompat,
    [switch]$PlaywrightSmoke,
    [int]$McpPort = 18931,
    [int]$CdpPort = 9272,
    [int]$ModelPort = 18766,
    [int]$FixturePort = 18764
)

$ErrorActionPreference = 'Stop'
$previousCloudMode = $env:LUCIA_E2E_CLOUD_COMPAT
$previousMcpPort = $env:LUCIA_MCP_PORT
if ($CloudCompat) { $env:LUCIA_E2E_CLOUD_COMPAT = '1' }
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
$env:LUCIA_CDP_PORT = "$CdpPort"
$env:LUCIA_MODEL_PORT = "$ModelPort"
$env:LUCIA_FIXTURE_PORT = "$FixturePort"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$CdpPort --remote-allow-origins=*"
New-Item -ItemType Directory -Force $env:APPDATA, $env:LOCALAPPDATA, $env:WEBVIEW2_USER_DATA_FOLDER | Out-Null

$modelProcess = $null
$appProcess = $null
$mcpProcess = $null
# WebView2 150+ ignores CDP environment flags in elevated host processes.
# GitHub Windows runners are elevated, so set a temporary machine-level
# policy for this one executable in the ephemeral CI worker only.
$cdpPolicyPath = 'HKLM:\SOFTWARE\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments'
$cdpPolicyName = [IO.Path]::GetFileName($ExecutablePath)
$createdCdpPolicy = $false
$serverOut = Join-Path $profile 'mock-model.stdout.log'
$serverErr = Join-Path $profile 'mock-model.stderr.log'
$mcpOut = Join-Path $profile 'playwright-mcp.stdout.log'
$mcpErr = Join-Path $profile 'playwright-mcp.stderr.log'

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
    if ($env:GITHUB_ACTIONS -eq 'true') {
        New-Item -Path $cdpPolicyPath -Force | Out-Null
        $preexisting = Get-ItemProperty -Path $cdpPolicyPath -Name $cdpPolicyName -ErrorAction SilentlyContinue
        if ($null -ne $preexisting) {
            throw "Refusing to overwrite an existing WebView2 policy for $cdpPolicyName"
        }
        New-ItemProperty -Path $cdpPolicyPath -Name $cdpPolicyName -PropertyType String -Value $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS | Out-Null
        $createdCdpPolicy = $true
        Write-Host "Configured temporary CI-only WebView2 debugging policy for $cdpPolicyName"
    }
    $serverScript = Join-Path $root 'tests\e2e\lucia-local-model-server.cjs'
    $modelProcess = Start-Process -FilePath 'node' -ArgumentList @('"' + $serverScript + '"') -WorkingDirectory $root -PassThru -RedirectStandardOutput $serverOut -RedirectStandardError $serverErr

    Wait-ForLoopback "http://127.0.0.1:$ModelPort/health"
    Wait-ForLoopback "http://127.0.0.1:$FixturePort/health"

    Write-Host 'Starting isolated Browsphere native WebView2 fixture tests'
    $appProcess = Start-Process -FilePath $ExecutablePath -WorkingDirectory $root -PassThru
    try {
        Wait-ForLoopback "http://127.0.0.1:$CdpPort/json" 55
    } catch {
        $appProcess.Refresh()
        Write-Host "Native app exited: $($appProcess.HasExited)"
        if ($appProcess.HasExited) { Write-Host "Native app exit code: $($appProcess.ExitCode)" }
        $webviews = @(Get-Process -Name 'msedgewebview2' -ErrorAction SilentlyContinue)
        Write-Host "WebView2 processes observed: $($webviews.Count)"
        Write-Host 'Windows CI browser debugging port was not opened; this is a startup/CDP failure, not a passed agent test'
        throw
    }

    if ($PlaywrightSmoke) {
        # Independent upstream MCP smoke against this same disposable WebView2
        # instance, without using a simulated MCP provider or user profile.
        $npx = Get-Command npx.cmd -ErrorAction Stop
        $mcpArgs = @(
            '--yes', '@playwright/mcp@0.0.86',
            '--host', '127.0.0.1',
            '--port', "$McpPort",
            '--cdp-endpoint', "http://127.0.0.1:$CdpPort"
        )
        $mcpProcess = Start-Process -FilePath $npx.Source -ArgumentList $mcpArgs -WorkingDirectory $root -PassThru -RedirectStandardOutput $mcpOut -RedirectStandardError $mcpErr
        $env:LUCIA_MCP_PORT = "$McpPort"
        $env:LUCIA_PLAYWRIGHT_NATIVE_PAGE_FILE = Join-Path $profile 'playwright-target.url'
        Write-Host 'LUCIA_UPSTREAM_PLAYWRIGHT_WEBVIEW2_START'
        & node (Join-Path $root 'tests\e2e\lucia-playwright-mcp-native.cjs')
        if ($LASTEXITCODE -ne 0) {
            throw "Live Microsoft Playwright MCP WebView2 smoke failed ($LASTEXITCODE)"
        }
        # Separate native Rust client test verifies the same live upstream
        # snapshot goes through Lucia's allowlist, schema, URL and role gates.
        & cargo test --workspace --locked live_upstream_playwright_webview2 -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) {
            throw "Lucia native MCP adapter rejected the real upstream server ($LASTEXITCODE)"
        }
        Write-Host 'LUCIA_UPSTREAM_PLAYWRIGHT_WEBVIEW2_VERIFIED'
        return
    }

    # Optional public-site diagnostic. Do not bypass Google challenges or
    # misrepresent fixture tests as live Google acceptance.
    if (-not $CloudCompat) {
        Write-Host 'LUCIA_GOOGLE_NATIVE_DIAGNOSTIC_START'
        & node (Join-Path $root 'tests\e2e\lucia-google-public-search.cjs')
        if ($LASTEXITCODE -ne 0) {
            Write-Warning 'LUCIA_GOOGLE_NATIVE_DIAGNOSTIC_UNVERIFIED'
        } else {
            Write-Host 'LUCIA_GOOGLE_NATIVE_DIAGNOSTIC_PASS'
        }
    }

    if (-not $CloudCompat) {
        Write-Host 'LUCIA_WIKIPEDIA_NATIVE_DIAGNOSTIC_START'
        & node (Join-Path $root 'tests\e2e\lucia-wikipedia-article.cjs')
        if ($LASTEXITCODE -ne 0) {
            Write-Warning 'LUCIA_WIKIPEDIA_NATIVE_DIAGNOSTIC_UNVERIFIED'
        } else {
            Write-Host 'LUCIA_WIKIPEDIA_NATIVE_DIAGNOSTIC_PASS'
        }
    }

    # Music-entry smoke check is intentionally independent of actual audio
    # playback, which still requires its own native advancing media receipt.
    if (-not $CloudCompat) {
        Write-Host 'LUCIA_MUSIC_ENTRY_DIAGNOSTIC_START'
        & node (Join-Path $root 'tests\e2e\lucia-play-any-song-entry.cjs')
        if ($LASTEXITCODE -ne 0) {
            Write-Warning 'LUCIA_MUSIC_ENTRY_DIAGNOSTIC_UNVERIFIED'
        } else {
            Write-Host 'LUCIA_MUSIC_ENTRY_DIAGNOSTIC_PASS'
        }
    }

    $cli = Join-Path $root 'tests\e2e\cli-browsphere.cjs'
    & node $cli --json doctor
    if ($LASTEXITCODE -ne 0) { throw 'Native Browsphere test dependencies failed CLI health checks' }
    & node $cli --json test
    if ($LASTEXITCODE -ne 0) {
        throw "Lucia multi-step native CLI test failed with exit code $LASTEXITCODE"
    }
    & node (Join-Path $root 'tests\e2e\lucia-tab-closure.cjs')
    if ($LASTEXITCODE -ne 0) {
        throw "Native Lucia task-tab closure test failed with exit code $LASTEXITCODE"
    }
    Write-Host 'PASS: multi-step native browsing, recovery, tab closure, and evidence checks'
} finally {
    if ($mcpProcess) {
        # Only terminate the child tree created by this isolated test.
        & taskkill.exe /PID $mcpProcess.Id /T /F 2>$null | Out-Null
    }
    if (Test-Path $mcpOut) { Get-Content $mcpOut -Tail 20 }
    if (Test-Path $mcpErr) {
        $messages = Get-Content $mcpErr -Tail 20
        if ($messages) { Write-Warning ($messages -join [Environment]::NewLine) }
    }
    if ($appProcess) { Stop-Process -Id $appProcess.Id -Force -ErrorAction SilentlyContinue }
    if ($modelProcess) { Stop-Process -Id $modelProcess.Id -Force -ErrorAction SilentlyContinue }
    $env:LUCIA_E2E_CLOUD_COMPAT = $previousCloudMode
    $env:LUCIA_MCP_PORT = $previousMcpPort
    Remove-Item Env:LUCIA_PLAYWRIGHT_NATIVE_PAGE_FILE -ErrorAction SilentlyContinue
    if (Test-Path $serverOut) { Get-Content $serverOut -Tail 25 }
    if (Test-Path $serverErr) {
        $errors = Get-Content $serverErr -Tail 25
        if ($errors) { Write-Warning ($errors -join [Environment]::NewLine) }
    }
    if ($createdCdpPolicy) {
        Remove-ItemProperty -Path $cdpPolicyPath -Name $cdpPolicyName -ErrorAction SilentlyContinue
        Write-Host "Removed temporary CI-only WebView2 policy for $cdpPolicyName"
    }
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
}
