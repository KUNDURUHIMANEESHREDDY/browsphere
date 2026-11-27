# Native Lucia end-to-end tests with a loopback-only model and fixture.
# Never uses the user's actual browser data, accounts, or API credentials.
param(
    [string]$ExecutablePath = "",
    [switch]$CloudCompat,
    [switch]$PlaywrightSmoke,
    [switch]$LiveVisionSmoke,
    [int]$McpPort = 18931,
    [int]$CdpPort = 9272,
    [int]$ModelPort = 18766,
    [int]$FixturePort = 18764,
    [string]$PerfReportPath = "",
    [switch]$PlaywrightInterop,
    [switch]$UpstreamMcpInterop
)

$ErrorActionPreference = 'Stop'
$previousCloudMode = $env:LUCIA_E2E_CLOUD_COMPAT
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
$ExecutablePath = (Resolve-Path -LiteralPath $ExecutablePath).Path
$executableWorkingDirectory = (Split-Path -Parent $ExecutablePath)
# Restoration matters if an engineer invokes this script from their own
# PowerShell session. The local test environment must never leak into it.
$profileEnv = @{}
foreach ($key in @('APPDATA','LOCALAPPDATA','WEBVIEW2_USER_DATA_FOLDER',
    'WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','LUCIA_CDP_PORT',
    'LUCIA_MODEL_PORT','LUCIA_FIXTURE_PORT',
    'LUCIA_MCP_PORT','LUCIA_PLAYWRIGHT_NATIVE_PAGE_FILE',
    'BROWSPHERE_STAGEHAND_ENABLE','BROWSPHERE_STAGEHAND_CI_FIXTURE','BROWSPHERE_STAGEHAND_MCP_PORT')) {
    $profileEnv[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
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
$stagehandProcess = $null
$mcpProcess = $null
$appProcess = $null
$startupMilliseconds = $null
$startupWatch = $null
$treeSampler = $null
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

    if ($UpstreamMcpInterop) {
        if ($env:GITHUB_ACTIONS -ne 'true') {
            throw 'Upstream MCP experimental integration requires disposable Windows CI'
        }
        $configDir=Join-Path $env:LOCALAPPDATA 'BrowsphereAI'
        New-Item -ItemType Directory -Force $configDir | Out-Null
        @{stagehand=@{
            url='http://127.0.0.1:18888/mcp'
            read_only_tools=@('stagehand_inspect','stagehand_observe','stagehand_extract')
        }} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $configDir 'mcp-servers.json') -Encoding UTF8
        $env:BROWSPHERE_STAGEHAND_ENABLE='1'
        $env:BROWSPHERE_STAGEHAND_CI_FIXTURE='1'
        $env:BROWSPHERE_STAGEHAND_MCP_PORT='18888'
        $stagehandScript=Join-Path $root 'integrations\stagehand-mcp\server.mjs'
        $stagehandProcess=Start-Process -FilePath 'node' -ArgumentList @('"' + $stagehandScript + '"') -WorkingDirectory (Join-Path $root 'integrations\stagehand-mcp') -PassThru -RedirectStandardOutput (Join-Path $profile 'stagehand.out') -RedirectStandardError (Join-Path $profile 'stagehand.err')
        $stagehandReady=$false
        for($i=0;$i -lt 45;$i++){
            try {
                $tcp=[Net.Sockets.TcpClient]::new('127.0.0.1',18888)
                $tcp.Dispose()
                $stagehandReady=$true
                break
            }catch{Start-Sleep -Milliseconds 250}
        }
        if (-not $stagehandReady) { throw 'Official Stagehand MCP server did not bind on loopback' }
    }

    Write-Host 'Starting isolated Browsphere native WebView2 fixture tests'
    $startupWatch=[Diagnostics.Stopwatch]::StartNew()
    # The app must start from its installation directory, not the source tree.
    # Otherwise an unpackaged file could accidentally mask a broken bundle.
    $appProcess = Start-Process -FilePath $ExecutablePath -WorkingDirectory $executableWorkingDirectory -PassThru
    if (-not [string]::IsNullOrWhiteSpace($PerfReportPath) -and $env:GITHUB_ACTIONS -eq 'true') {
        # Sampling runs concurrently with the actual browser task. Collect
        # descendants of this exact PID only; never touch unrelated processes.
        $samplerPath = Join-Path $PSScriptRoot 'process-tree-metrics.ps1'
        $treeSampler = Start-Job -ArgumentList @([int]$appProcess.Id,$samplerPath) -ScriptBlock {
            param([int]$nativePid, [string]$library)
            . $library
            while ($true) {
                try {
                    $rows = @(Get-CimInstance Win32_Process -Property ProcessId,ParentProcessId,Name,WorkingSetSize,PrivatePageCount -ErrorAction Stop)
                    $sample = Get-BrowserProcessTreeSample -RootProcessId $nativePid -ProcessRows $rows
                    if ($null -ne $sample) { $sample | ConvertTo-Json -Compress }
                } catch { }
                Start-Sleep -Milliseconds 1200
            }
        }
    }
    try {
        Wait-ForLoopback "http://127.0.0.1:$CdpPort/json" 55
        $startupWatch.Stop()
        $startupMilliseconds=[math]::Round($startupWatch.Elapsed.TotalMilliseconds,1)
        Write-Host ("BROWSER_STARTUP_CDP_MS={0}" -f $startupMilliseconds)
    } catch {
        $appProcess.Refresh()
        Write-Host "Native app exited: $($appProcess.HasExited)"
        if ($appProcess.HasExited) { Write-Host "Native app exit code: $($appProcess.ExitCode)" }
        $webviews = @(Get-Process -Name 'msedgewebview2' -ErrorAction SilentlyContinue)
        Write-Host "WebView2 processes observed: $($webviews.Count)"
        Write-Host 'Windows CI browser debugging port was not opened; this is a startup/CDP failure, not a passed agent test'
        throw
    }

    if ($LiveVisionSmoke) {
        if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Live visual smoke requires a disposable Windows CI runner' }
        Write-Host 'LUCIA_LIVE_VISUAL_STREAM_START'
        & node (Join-Path $root 'tests\e2e\lucia-live-vision.cjs')
        if ($LASTEXITCODE -ne 0) { throw "Native CDP + BRPR live visual integration failed ($LASTEXITCODE)" }
        Write-Host 'LUCIA_LIVE_VISUAL_STREAM_VERIFIED'
        return
    }

    if ($PlaywrightSmoke) {
        if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Playwright MCP smoke requires isolated Windows CI' }
        # Reuse the exact upstream Microsoft Playwright MCP npm package.
        # Never start this on a user profile or expose consumer CDP ports.
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
        if ($LASTEXITCODE -ne 0) { throw "Real upstream Microsoft MCP WebView2 smoke failed ($LASTEXITCODE)" }
        # The live server must also pass Lucia's existing Rust MCP gateway,
        # exact URL binding, read-only permission and bounded-role checks.
        & cargo test --workspace --locked live_upstream_playwright_webview2 -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw "Lucia native MCP gateway rejected the upstream server ($LASTEXITCODE)" }
        Write-Host 'LUCIA_UPSTREAM_PLAYWRIGHT_WEBVIEW2_VERIFIED'
        return
    }

    if ($PlaywrightInterop) {
        if ($env:GITHUB_ACTIONS -ne 'true') {
            throw 'Upstream CDP interop is CI-only, not enabled for end users.'
        }
        Write-Host 'Checking installed WebView2 with upstream Microsoft Playwright'
        & node (Join-Path $root 'tests\interop\playwright-webview2-smoke.cjs')
        if ($LASTEXITCODE -ne 0) {
            throw 'Playwright WebView2 CDP interoperability failed after successful CDP startup.'
        }
    }

    if ($UpstreamMcpInterop) {
        if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Upstream native interop is restricted to disposable CI profiles' }
        # Stagehand's full initialization against installed WebView2 times
        # out after 60s (reproduced in two Windows runs). Do not invent a
        # custom CDP actuator, silently skip the test, or falsely mark
        # Stagehand native compatibility as proven. Its isolated Chromium
        # research/act engine is verified in the separate upstream SDK job.
        Write-Host 'STAGEHAND_NATIVE_WEBVIEW2_UNSUPPORTED: official Playwright owns native WebView2 actions'
        Write-Host 'Checking official Stagehand MCP tools through actual installed Lucia chatbox'
        & node (Join-Path $root 'tests\interop\upstream-stagehand-native.cjs')
        if ($LASTEXITCODE -ne 0) {
            throw 'Native Lucia-to-upstream-Stagehand MCP end-to-end acceptance failed'
        }
        Write-Host 'Checking actual Microsoft Playwright actions with native Browsphere state readback'
        & node (Join-Path $root 'tests\interop\playwright-native-actions.cjs')
        if ($LASTEXITCODE -ne 0) {
            throw 'Official Playwright actions did not pass independent native-WebView2 readback'
        }
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
        & taskkill.exe /PID $mcpProcess.Id /T /F 2>$null | Out-Null
    }
    if (Test-Path $mcpOut) { Get-Content $mcpOut -Tail 10 }
    if (Test-Path $mcpErr) {
        $mcpMessages = Get-Content $mcpErr -Tail 10
        if ($mcpMessages) { Write-Warning ($mcpMessages -join [Environment]::NewLine) }
    }
    $processTreeSamples = @()
    if ($treeSampler) {
        Stop-Job -Job $treeSampler -ErrorAction SilentlyContinue
        $sampleLines = @(Receive-Job -Job $treeSampler -ErrorAction SilentlyContinue)
        $processTreeSamples = @($sampleLines | ForEach-Object {
            try { $_ | ConvertFrom-Json -ErrorAction Stop } catch { }
        } | Where-Object { $null -ne $_ -and $_.process_count -ge 1 })
        Remove-Job -Job $treeSampler -Force -ErrorAction SilentlyContinue
    }
    if (-not [string]::IsNullOrWhiteSpace($PerfReportPath)) {
        # Performance is a measured CI baseline, NOT an arbitrary marketing
        # latency claim or aggregate of unrelated system WebView processes.
        try {
            $hostProcess=if ($appProcess) { Get-Process -Id $appProcess.Id -ErrorAction SilentlyContinue } else { $null }
            $mb=1MB
            # Helper is dot-sourced for testability, then used only on
            # numeric observations (no command-lines or page content).
            . (Join-Path $PSScriptRoot 'process-tree-metrics.ps1')
            $data=[ordered]@{
                cdp_ready_ms=$startupMilliseconds
                native_host_working_set_mb=if($hostProcess){[math]::Round($hostProcess.WorkingSet64/$mb,1)}else{$null}
                native_host_peak_working_set_mb=if($hostProcess){[math]::Round($hostProcess.PeakWorkingSet64/$mb,1)}else{$null}
                native_host_private_bytes_mb=if($hostProcess){[math]::Round($hostProcess.PrivateMemorySize64/$mb,1)}else{$null}
                sample="host process point-in-time; owned native host + child processes sampled concurrently during task"
                process_tree_sample_count=$processTreeSamples.Count
                webview2_child_count_peak=if($processTreeSamples.Count){($processTreeSamples | Measure-Object -Property webview2_child_count -Maximum).Maximum}else{0}
                process_tree_working_set_mb_p50=if($processTreeSamples.Count){Get-NearestRankPercentile -Values @($processTreeSamples | ForEach-Object {[double]$_.process_tree_working_set_mb}) -Quantile 0.50}else{$null}
                process_tree_working_set_mb_p95=if($processTreeSamples.Count){Get-NearestRankPercentile -Values @($processTreeSamples | ForEach-Object {[double]$_.process_tree_working_set_mb}) -Quantile 0.95}else{$null}
                process_tree_working_set_mb_p99=if($processTreeSamples.Count){Get-NearestRankPercentile -Values @($processTreeSamples | ForEach-Object {[double]$_.process_tree_working_set_mb}) -Quantile 0.99}else{$null}
                process_tree_working_set_mb_peak=if($processTreeSamples.Count){($processTreeSamples | Measure-Object -Property process_tree_working_set_mb -Maximum).Maximum}else{$null}
                webview2_working_set_mb_peak=if($processTreeSamples.Count){($processTreeSamples | Measure-Object -Property webview2_working_set_mb -Maximum).Maximum}else{$null}
                process_tree_private_bytes_mb_peak=if($processTreeSamples.Count){($processTreeSamples | Measure-Object -Property process_tree_private_bytes_mb -Maximum).Maximum}else{$null}
                process_tree_ownership="Only descendants of launched browsphere-core PID. Excludes standalone Ollama and unrelated WebView2."
                ci_run=$env:GITHUB_RUN_ID
                commit=$env:GITHUB_SHA
                model_mode=if($CloudCompat){"mock-cloud"}else{"mock-local"}
            }
            New-Item -ItemType Directory -Force (Split-Path -Parent $PerfReportPath) | Out-Null
            $data | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $PerfReportPath -Encoding UTF8
            Write-Host ("BROWSER_HOST_PERF_REPORT_SAVED mode={0}" -f $data.model_mode)
        } catch {
            Write-Warning ("Could not record optional host performance baseline: " + $_.Exception.GetType().Name)
        }
    }
    if ($appProcess) { Stop-Process -Id $appProcess.Id -Force -ErrorAction SilentlyContinue }
    if ($stagehandProcess) { Stop-Process -Id $stagehandProcess.Id -Force -ErrorAction SilentlyContinue }
    if ($modelProcess) { Stop-Process -Id $modelProcess.Id -Force -ErrorAction SilentlyContinue }
    $env:LUCIA_E2E_CLOUD_COMPAT = $previousCloudMode
    if (Test-Path $serverOut) { Get-Content $serverOut -Tail 25 }
    if (Test-Path $serverErr) {
        $errors = Get-Content $serverErr -Tail 25
        if ($errors) { Write-Warning ($errors -join [Environment]::NewLine) }
    }
    if ($createdCdpPolicy) {
        Remove-ItemProperty -Path $cdpPolicyPath -Name $cdpPolicyName -ErrorAction SilentlyContinue
        Write-Host "Removed temporary CI-only WebView2 policy for $cdpPolicyName"
    }
    foreach ($key in $profileEnv.Keys) {
        [Environment]::SetEnvironmentVariable($key, $profileEnv[$key], 'Process')
    }
    # Test profiles are disposable. An active WebView2 child may still hold a
    # file briefly after the host exits; never force-stop unrelated processes.
    Remove-Item -LiteralPath $profile -Recurse -Force -ErrorAction SilentlyContinue
}
