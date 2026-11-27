# Repeated, independent native-chatbox research acceptance on websites outside fixed fixtures.
# Each attempt uses a fresh WebView2 profile, port and native app process.
# A pass requires the JS harness's evidence-checked completed status.
param(
  [ValidateSet('all','webdev','w3c')][string]$Scenario='all',
  [ValidateRange(1,10)][int]$Repetitions=3,
  [int]$BaseCdpPort=9740,
  [string]$ActionModel='qwen3.5:0.8b',
  [string]$SupervisorModel='lfm3:1.7b',
  [switch]$RequireRecovery,
  [switch]$MeasuredStartupHeadroom,
  [string]$ExecutablePath='',
  [string]$ReportPath=''
)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if(-not $ExecutablePath){$ExecutablePath=Join-Path $root 'target\release\browsphere-core.exe'}
if(-not (Test-Path $ExecutablePath)){throw "Missing native executable: $ExecutablePath"}
# Never certify new source behavior by running an old native executable.
# This is a freshness gate, not a cryptographic build-provenance attestation.
$latestRust=Get-ChildItem (Join-Path $root 'src-tauri\src') -Filter '*.rs' -File -Recurse |
  Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
if($latestRust -and (Get-Item $ExecutablePath).LastWriteTimeUtc -lt $latestRust.LastWriteTimeUtc){
  throw "Native executable predates Rust source $($latestRust.FullName). Rebuild before real-Qwen acceptance."
}
if(-not $ReportPath){$ReportPath=Join-Path $root 'logs\lucia-unfamiliar-reliability.json'}
$cases=@(
  @{name='webdev';hosts='web.dev';url='https://web.dev/learn/accessibility';goal='Visit https://web.dev/learn/accessibility and report one specific recommendation from the official Learn Accessibility guide, citing a supporting passage from that exact website.'},
  @{name='w3c';hosts='w3.org';url='https://www.w3.org/WAI/tutorials/images/';goal='Visit https://www.w3.org/WAI/tutorials/images/ and report one specific accessibility recommendation for informative images, citing the observed official W3C source.'}
)
if($Scenario -ne 'all'){$cases=@($cases | Where-Object {$_.name -eq $Scenario})}
$records=[System.Collections.Generic.List[object]]::new()
$counter=0
foreach($case in $cases){
  for($run=1;$run -le $Repetitions;$run++){
    # An isolated WebView2 shell consumes RAM before Lucia's native 2048 MiB
    # inference admission runs. The default launch gate reserves 1024 MiB.
    # Optional diagnostic mode remeasures post-start RAM before model dispatch.
    $availableMiB=[math]::Floor((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory/1024)
    # Default acceptance retains the original conservative 3072 MiB gate.
    # Explicit diagnostic mode can launch with 512 MiB startup reserve,
    # but must remeasure RAM after WebView2 startup before any model task.
    $requiredLaunchMiB=if($MeasuredStartupHeadroom){2560}else{3072}
    # Windows free RAM can fluctuate by tens of MiB between launch checks.
    # Briefly resample only near the gate; never lower it or launch below it.
    if($availableMiB -ge ($requiredLaunchMiB - 128)){
      for($headroomRetry=0; $headroomRetry -lt 5 -and $availableMiB -lt $requiredLaunchMiB; $headroomRetry++){
        Start-Sleep -Seconds 2
        $availableMiB=[math]::Floor((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory/1024)
      }
    }
    if($availableMiB -lt $requiredLaunchMiB){
      $message="Insufficient browser + model headroom: $availableMiB MiB free before launch, $requiredLaunchMiB MiB needed for 2048 MiB native minimum plus browser startup"
      $records.Add([ordered]@{case=$case.name;run=$run;passed=$false;exit_code=2;seconds=0;final_status='resource_blocked';actions=0;model_actions=0;qwen_actions=0;findings=0;errors=@($message);log=''})
      Write-Output ("$($case.name) #$run SKIPPED: $message")
      continue
    }
    $counter++
    $port=$BaseCdpPort+$counter
    $profile=Join-Path $env:TEMP ('browsphere-unfamiliar-'+[guid]::NewGuid().ToString('N'))
    $logFile=Join-Path $root ("logs\unfamiliar-$($case.name)-$run.log")
    New-Item -ItemType Directory -Force (Join-Path $profile 'Roaming'),(Join-Path $profile 'Local'),(Join-Path $profile 'WebView2') | Out-Null
    $started=Get-Date
    $app=$null
    $exitCode=-1
    $resourceBlockReason=$null
    try{
      $env:APPDATA=Join-Path $profile 'Roaming'
      $env:LOCALAPPDATA=Join-Path $profile 'Local'
      $env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $profile 'WebView2'
      $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=$port --remote-allow-origins=*"
      $env:LUCIA_CDP_PORT="$port"
      $env:LUCIA_TEST_GOAL=$case.goal
      $env:LUCIA_EXPECTED_HOSTS=$case.hosts
      $env:LUCIA_EXPECTED_SOURCE_URL=$case.url
      $env:LUCIA_TEST_ACTION_MODEL=$ActionModel
      $env:LUCIA_TEST_SUPERVISOR_MODEL=$SupervisorModel
      $env:LUCIA_REQUIRE_RECOVERY=if($RequireRecovery){'1'}else{'0'}
      $app=Start-Process -FilePath $ExecutablePath -WorkingDirectory $root -PassThru
      $ready=$false
      for($i=0;$i -lt 85;$i++){
        try{
          $pages=Invoke-RestMethod "http://127.0.0.1:$port/json" -TimeoutSec 2
          if(@($pages | Where-Object {$_.type -eq 'page' -and $_.url -like '*tauri.localhost*'}).Count -gt 0){$ready=$true;break}
        }catch{}
        Start-Sleep -Milliseconds 400
      }
      if(-not $ready){throw 'No isolated native chatbox CDP target'}
      if($MeasuredStartupHeadroom){
        # Measure the actual isolated WebView2 footprint before dispatching
        # the real model. Native inference independently enforces this floor.
        $afterStartupMiB=[math]::Floor((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory/1024)
        if($afterStartupMiB -lt 2048){
          $resourceBlockReason="Insufficient local-model headroom after isolated WebView2 launch: $afterStartupMiB MiB free (2048 MiB native minimum)"
          throw $resourceBlockReason
        }
        Write-Output "MEASURED_POST_BROWSER_FREE_MIB=$afterStartupMiB"
      }
      Push-Location $root
      try {
        # Native Node stderr can become a PowerShell error record under
        # ErrorActionPreference=Stop. It must not skip LASTEXITCODE capture:
        # a failed real model trial still needs its genuine process exit code.
        $savedPreference=$ErrorActionPreference
        try {
          $ErrorActionPreference='Continue'
          & node 'tests\e2e\lucia-orchestrator-unfamiliar-live.cjs' *> $logFile
          $exitCode=$LASTEXITCODE
        } finally { $ErrorActionPreference=$savedPreference }
      } finally {Pop-Location}
    } catch {
      Add-Content -Path $logFile -Value ("RUNNER_ERROR: "+$_.Exception.Message)
    } finally {
      if($app){Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue}
      foreach($name in 'APPDATA','LOCALAPPDATA','WEBVIEW2_USER_DATA_FOLDER','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','LUCIA_CDP_PORT','LUCIA_TEST_GOAL','LUCIA_EXPECTED_HOSTS','LUCIA_EXPECTED_SOURCE_URL','LUCIA_TEST_ACTION_MODEL','LUCIA_TEST_SUPERVISOR_MODEL','LUCIA_REQUIRE_RECOVERY'){
        Remove-Item "Env:$name" -ErrorAction SilentlyContinue
      }
      # The disposable profile is never re-used; cleanup is best-effort.
      Remove-Item -LiteralPath $profile -Force -Recurse -ErrorAction SilentlyContinue
    }
    $raw=if(Test-Path $logFile){Get-Content -Raw $logFile}else{''}
    $resultLine=@($raw -split "\r?\n" | Where-Object {$_ -match '^ORCHESTRATOR_RESULT '}) | Select-Object -Last 1
    $result=$null
    if($resultLine){try{$result=($resultLine -replace '^ORCHESTRATOR_RESULT ','') | ConvertFrom-Json}catch{}}
    $passed=(!$resourceBlockReason) -and $exitCode -eq 0 -and $null -ne $result -and $result.passed -eq $true -and $result.status -eq 'completed'
    $record=[ordered]@{
      case=$case.name; run=$run; passed=$passed; exit_code=if($resourceBlockReason){2}else{$exitCode}
      seconds=[math]::Round(((Get-Date)-$started).TotalSeconds,1)
      final_status=if($resourceBlockReason){'resource_blocked'}elseif($result){$result.status}else{'no verified result'}
      actions=if($resourceBlockReason){0}elseif($result){$result.actions}else{0}
      model_actions=if($resourceBlockReason){0}elseif($result){$result.modelActions}else{0}
      qwen_actions=if($ActionModel -match '^qwen' -and $result){$result.modelActions}else{0}
      findings=if($resourceBlockReason){0}elseif($result){$result.findings}else{0}
      errors=if($resourceBlockReason){@($resourceBlockReason)}elseif($result){@($result.errors)}else{@('No machine-readable verified completion')}
      log=$logFile
    }
    $records.Add($record)
    Write-Output ("{0} #{1}: {2}, status={3}, actions={4}" -f $case.name,$run,$(if($passed){'PASS'}else{'FAIL'}),$record.final_status,$record.actions)
  }
}
$passedCount=@($records | Where-Object {$_.passed}).Count
$resourceBlocked=@($records | Where-Object {$_.final_status -eq 'resource_blocked'}).Count
$executed=$records.Count-$resourceBlocked
$failed=$executed-$passedCount
$report=[ordered]@{
  generated_utc=(Get-Date).ToUniversalTime().ToString('o')
  branch=(git -C $root rev-parse --abbrev-ref HEAD)
  commit=(git -C $root rev-parse --short HEAD)
  action_model=$ActionModel;supervisor_model=$SupervisorModel
  total=$records.Count;executed=$executed;resource_blocked=$resourceBlocked
  passed=$passedCount;failed=$failed
  all_passed=($passedCount -eq $records.Count -and $resourceBlocked -eq 0)
  qualification='Resource-blocked attempts never reached task execution; they do not count as passed or failed model-driven trials. A real pass needs independent source evidence and final task verification.'
  runs=@($records)
}
New-Item -ItemType Directory -Force (Split-Path -Parent $ReportPath)|Out-Null
$report | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 $ReportPath
Write-Output "REPORT: $ReportPath"
if(-not $report.all_passed){exit 1}
