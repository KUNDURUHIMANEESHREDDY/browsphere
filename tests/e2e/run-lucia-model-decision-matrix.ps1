# Strict model-decision acceptance suite on isolated native WebView2 runs.
# Requires a locally installed model. Never substitutes a mock inference server.
param(
  [Parameter(Mandatory=$true)][string]$ExecutablePath,
  [ValidateRange(1,10)][int]$Repetitions=2,
  [int]$StartPort=9550,
  [string]$ReportPath=""
)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$runner=Join-Path $PSScriptRoot 'run-lucia-cross-site-live.ps1'
if(-not (Test-Path $ExecutablePath)){throw "Executable not found: $ExecutablePath"}
if(-not $ReportPath){$ReportPath=Join-Path $root 'logs\lucia-model-decision-matrix.json'}
$cases=@(
  @{id='original';variant=0;negative=0},
  @{id='reversed';variant=1;negative=0},
  @{id='rotated';variant=2;negative=0},
  @{id='missing-answer';variant=0;negative=1}
)
$results=@(); $index=0
try{
  foreach($round in 1..$Repetitions){
    foreach($case in $cases){
      $index++
      $env:LUCIA_MODEL_DECISION_TEST='1'
      $env:LUCIA_MODEL_DECISION_VARIANT=[string]$case.variant
      $env:LUCIA_MODEL_NEGATIVE_TEST=[string]$case.negative
      $log=Join-Path $env:TEMP ("lucia-model-decision-$round-$($case.id)-$([guid]::NewGuid().ToString('N')).log")
      & powershell -NoProfile -ExecutionPolicy Bypass -File $runner -ExecutablePath $ExecutablePath -CdpPort ($StartPort+$index) *> $log
      $code=$LASTEXITCODE
      $lines=@(Get-Content $log | Where-Object {$_ -like 'MODEL_DECISION_RESULT *'})
      $parsed=$null
      if($lines.Count -eq 1){try{$parsed=($lines[0] -replace '^MODEL_DECISION_RESULT ','') | ConvertFrom-Json}catch{}}
      $success=($code -eq 0 -and $null -ne $parsed -and $parsed.pass -eq $true)
      $results+=@{round=$round;case=$case.id;passed=$success;exit_code=$code;checks=$(if($parsed){$parsed.checks}else{$null});log=$log}
      Write-Host ("Round {0} / {1}: {2}" -f $round,$case.id,$(if($success){'PASS'}else{'FAIL'}))
    }
  }
}finally{
  Remove-Item Env:LUCIA_MODEL_DECISION_TEST,Env:LUCIA_MODEL_DECISION_VARIANT,Env:LUCIA_MODEL_NEGATIVE_TEST -ErrorAction SilentlyContinue
}
$fail=@($results|Where-Object {-not $_.passed}).Count
$report=@{created=(Get-Date).ToUniversalTime().ToString('o');executable=$ExecutablePath;repetitions=$Repetitions;total=$results.Count;passed=$results.Count-$fail;failed=$fail;all_passed=$fail -eq 0;zero_false_completion=$results.Where({$_.case -eq 'missing-answer' -and -not $_.passed}).Count -eq 0;results=$results}
New-Item -ItemType Directory -Force (Split-Path $ReportPath -Parent)|Out-Null
$report|ConvertTo-Json -Depth 7|Set-Content -Path $ReportPath -Encoding UTF8
Write-Host "Report: $ReportPath"
if($fail -gt 0){exit 1}
