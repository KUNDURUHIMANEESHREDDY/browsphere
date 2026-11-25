# Lucia reliability gate: repeated isolated native WebView2 website runs.
# No mocked-model run counts as proof of real-world autonomy.
param(
    [Parameter(Mandatory=$true)][string]$ExecutablePath,
    [ValidateRange(3,30)][int]$Repetitions=3,
    [int]$BaseCdpPort=9490,
    [string]$ReportPath=""
)
$ErrorActionPreference="Stop"
$root=(Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$runner=Join-Path $PSScriptRoot "run-lucia-cross-site-live.ps1"
if(-not (Test-Path $ExecutablePath)){throw "Native executable missing: $ExecutablePath"}
if(-not $ReportPath){$ReportPath=Join-Path $root "logs\lucia-reliability-gate.json"}
$records=@()
for($run=1;$run -le $Repetitions;$run++){
    $started=Get-Date
    $port=$BaseCdpPort+$run
    $logFile=Join-Path $env:TEMP ("lucia-reliability-$run-$([guid]::NewGuid().ToString('N')).log")
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $runner -ExecutablePath $ExecutablePath -CdpPort $port *> $logFile
    $exitCode=$LASTEXITCODE
    $output=Get-Content -Raw $logFile
    $pass=$exitCode -eq 0 -and $output.Contains("CROSS_SITE_TESTS_PASSED") -and $output.Contains("LUCIA_CROSS_SITE_EXIT=0")
    $records+=@{run=$run;passed=$pass;exit_code=$exitCode;seconds=[math]::Round(((Get-Date)-$started).TotalSeconds,1);log=$logFile}
    Write-Host ("Run {0}/{1}: {2}" -f $run,$Repetitions,$(if($pass){"PASS"}else{"FAIL"}))
}
$success=@($records|Where-Object {$_.passed}).Count
$report=@{
    date_utc=(Get-Date).ToUniversalTime().ToString("o")
    executable=$ExecutablePath
    repetitions=$Repetitions
    passed=$success
    failed=$Repetitions-$success
    success_rate=[math]::Round($success/$Repetitions,4)
    all_passed=($success -eq $Repetitions)
    caveat="Fixed public-site cases only. No guarantee of unfamiliar-site autonomy. Check individual assertions and logs for false-completion evidence."
    runs=$records
}
New-Item -ItemType Directory -Force (Split-Path -Parent $ReportPath)|Out-Null
$report|ConvertTo-Json -Depth 6|Set-Content -Encoding UTF8 $ReportPath
Write-Host "Reliability report: $ReportPath"
if(-not $report.all_passed){exit 1}
