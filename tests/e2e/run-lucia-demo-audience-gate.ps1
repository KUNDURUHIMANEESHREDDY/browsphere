# Run all audience-specific Lucia demos on one exact native executable.
# Each scenario uses a disposable WebView2 profile and a distinct CDP port.
param(
    [Parameter(Mandatory=$true)][string]$ExecutablePath,
    [int]$StartPort = 9475,
    [ValidateRange(1,3)][int]$Repetitions = 2
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$ExecutablePath = (Resolve-Path -LiteralPath $ExecutablePath).Path
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$output = Join-Path $root "tests\e2e\artifacts\demo-audience-gate-$stamp"
New-Item -ItemType Directory -Path $output -Force | Out-Null
$sha = (Get-FileHash -LiteralPath $ExecutablePath -Algorithm SHA256).Hash
$cases = @(
    @{ Key = 'S1'; Env = 'LUCIA_DEMO_S1'; Script = 'run-lucia-demo-s1.ps1'; Port = $StartPort },
    @{ Key = 'W1'; Env = 'LUCIA_DEMO_W1'; Script = 'run-lucia-demo-w1.ps1'; Port = $StartPort + 1 },
    @{ Key = 'P1'; Env = 'LUCIA_DEMO_P1'; Script = 'run-lucia-demo-p1.ps1'; Port = $StartPort + 2 }
)
$results = @()
foreach ($case in $cases) {
  for ($run=1; $run -le $Repetitions; $run++) {
    # Local 3B inference shares RAM with Rust linking. A concurrent build
    # can trigger Lucia's correctly enforced low-memory protection, which is
    # a resource blockage rather than a browsing-regression verdict.
    $resourcesReady = $false
    for ($check = 0; $check -lt 36; $check++) {
        $freeGiB = (Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1MB
        $compilerRunning = [bool](Get-Process rustc -ErrorAction SilentlyContinue)
        if ($freeGiB -ge 2.5 -and -not $compilerRunning) { $resourcesReady = $true; break }
        Write-Output "AUDIENCE_RESOURCE_WAIT case=$($case.Key) freeGiB=$([math]::Round($freeGiB,2)) rustc=$compilerRunning"
        Start-Sleep -Seconds 5
    }
    if (-not $resourcesReady) {
        $results += [pscustomobject]@{
            case = $case.Key; run = $run; passed = $false; exitCode = $null
            status = 'blocked_low_memory_or_build'; verified = $false
            durationSeconds = 0; log = $null
        }
        Write-Output "AUDIENCE_CASE $($case.Key) passed=False status=blocked_low_memory_or_build"
        continue
    }
    foreach ($name in @('LUCIA_DEMO_S1','LUCIA_DEMO_W1','LUCIA_DEMO_P1')) {
        [Environment]::SetEnvironmentVariable($name, $null, 'Process')
    }
    [Environment]::SetEnvironmentVariable($case.Env, '1', 'Process')
    # Validate the exact presentation prompts, including their scenario labels.
    if ($case.Key -eq 'W1') { $env:LUCIA_W1_DEMO_WRAPPED = '1' }
    if ($case.Key -eq 'P1') { $env:LUCIA_P1_DEMO_WRAPPED = '1' }
    $log = Join-Path $output "$($case.Key)-run$run.log"
    Write-Output "AUDIENCE_START $($case.Key) exe_sha256=$sha log=$log"
    $started = Get-Date
    try {
        # Browser compatibility has its own 17-check suite. Running it before
        # every live model task needlessly consumes memory on a 16-GiB PC.
        & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot $case.Script) -SkipCompatibility -ExecutablePath $ExecutablePath -CdpPort ($case.Port + (($run - 1) * 3)) *> $log
        $exitCode = $LASTEXITCODE
    } catch {
        $_ | Out-String | Add-Content -Path $log
        $exitCode = 1
    } finally {
        [Environment]::SetEnvironmentVariable($case.Env, $null, 'Process')
        Remove-Item Env:LUCIA_W1_DEMO_WRAPPED -ErrorAction SilentlyContinue
        Remove-Item Env:LUCIA_P1_DEMO_WRAPPED -ErrorAction SilentlyContinue
    }
    $line = Get-Content -Path $log -ErrorAction SilentlyContinue |
        Where-Object { $_ -like "DEMO_$($case.Key)_RESULT *" } | Select-Object -Last 1
    $taskStatus = 'missing_report'
    $verified = $false
    $scenarioPass = $false
    if ($line) {
        try {
            $parsed = ($line -replace "^DEMO_$($case.Key)_RESULT ", '') | ConvertFrom-Json
            $task = if ($parsed.task) { $parsed.task } else { $parsed }
            $taskStatus = [string]$task.status
            $verified = $task.verified -eq $true
            $scenarioPass = ($parsed.pass -eq $true)
        } catch { $taskStatus = 'invalid_report' }
    }
    $pass = ($exitCode -eq 0 -and $scenarioPass -and $verified -and $taskStatus -eq 'completed')
    $results += [pscustomobject]@{
        case = $case.Key; run = $run; passed = $pass; exitCode = $exitCode
        status = $taskStatus; verified = $verified
        durationSeconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
        log = $log
    }
    Write-Output "AUDIENCE_CASE $($case.Key) passed=$pass status=$taskStatus verified=$verified exit=$exitCode"
  }
}
foreach ($missing in @(
    @{ Key = 'R1'; Label = 'Research and write a report'; Requirement = 'Observed research, saved report artifact, readback, independent verification' },
    @{ Key = 'C1'; Label = 'Research and prepare a calendar event'; Requirement = 'Observed research, MCP event write, event readback, independent verification' }
)) {
    # Report-writing and calendar tools need a real native task run plus
    # persisted artifact/event readback. The contract executable is available,
    # but never substitutes a mock receipt for actual browser+MCP execution.
    $receipt = Join-Path $output "$($missing.Key)-receipt.json"
    $outcome = 'native_execution_not_implemented'
    $outputValid = $false
    if (Test-Path -LiteralPath $receipt) {
        & node (Join-Path $PSScriptRoot 'demo-output-contract.cjs') $missing.Key $receipt
        $outputValid = ($LASTEXITCODE -eq 0)
        $outcome = if ($outputValid) { 'readback_contract_passed' } else { 'readback_contract_failed' }
    }
    $results += [pscustomobject]@{
        case = $missing.Key; passed = $false; exitCode = $null
        status = $outcome; verified = $false
        durationSeconds = 0; log = $receipt
        requiredEvidence = $missing.Requirement
    }
    Write-Output "AUDIENCE_CASE $($missing.Key) passed=False status=$outcome"
}
$report = Join-Path $output 'results.json'
[pscustomobject]@{ executable=$ExecutablePath; sha256=$sha; repetitions=$Repetitions; results=$results } |
    ConvertTo-Json -Depth 6 | Set-Content -Path $report -Encoding UTF8
Write-Output "AUDIENCE_REPORT $report"
if (@($results | Where-Object { -not $_.passed }).Count -gt 0) { exit 1 }
Write-Output 'AUDIENCE_GATE_PASSED'
