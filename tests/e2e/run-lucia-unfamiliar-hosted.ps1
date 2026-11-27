# Requires a REAL, installed Ollama Qwen model, not a scripted mock.
# Repeated, independent native-chatbox research acceptance on websites outside fixed fixtures.
# Each attempt uses a fresh WebView2 profile, port and native app process.
# A pass requires the JS harness's evidence-checked completed status.
param(
  [ValidateSet('all','webdev','w3c')][string]$Scenario='all',
  [ValidateRange(1,10)][int]$Repetitions=2,
  [int]$BaseCdpPort=9740,
  [string]$ActionModel='qwen2.5:3b-instruct',
  [string]$SupervisorModel='lfm3:1.7b',
  [switch]$RequireRecovery,
  [string]$ExecutablePath='',
  [string]$ReportPath=''
)
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true') {
    throw 'Hosted real-model unfamiliar acceptance is restricted to an ephemeral CI Windows runner.'
}
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if(-not $ExecutablePath){$ExecutablePath=Join-Path $root 'target\release\browsphere-core.exe'}
if(-not (Test-Path $ExecutablePath)){throw "Missing native executable: $ExecutablePath"}
$ExecutablePath=(Resolve-Path -LiteralPath $ExecutablePath).Path
$workingDirectory=(Split-Path -Parent $ExecutablePath)
$policyPath='HKLM:\SOFTWARE\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments'
$policyName=[IO.Path]::GetFileName($ExecutablePath)
$priorEnvironment=@{}
foreach($name in @('APPDATA','LOCALAPPDATA','WEBVIEW2_USER_DATA_FOLDER','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','LUCIA_CDP_PORT','LUCIA_TEST_GOAL','LUCIA_EXPECTED_HOSTS','LUCIA_EXPECTED_SOURCE_URL','LUCIA_TEST_ACTION_MODEL','LUCIA_TEST_SUPERVISOR_MODEL','LUCIA_REQUIRE_RECOVERY')){
    $priorEnvironment[$name]=[Environment]::GetEnvironmentVariable($name,'Process')
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
    # inference admission runs. Reserve 1024 MiB for the new browser process;
    # this is a conservative test-launch budget, not a lowered safety gate.
    $availableMiB=[math]::Floor((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory/1024)
    $requiredLaunchMiB=3072
    if($availableMiB -lt $requiredLaunchMiB){
      $message="Insufficient browser + model headroom: $availableMiB MiB free before launch, $requiredLaunchMiB MiB needed for 2048 MiB native minimum plus browser startup"
      $records.Add([ordered]@{case=$case.name;run=$run;passed=$false;exit_code=2;seconds=0;final_status='resource_blocked';actions=0;errors=@($message);log=''})
      Write-Output ("$($case.name) #$run SKIPPED: $message")
      continue
    }
    $counter++
    $port=$BaseCdpPort+$counter
    $profile=Join-Path $env:TEMP ('browsphere-unfamiliar-'+[guid]::NewGuid().ToString('N'))
    $logDirectory=Join-Path $env:RUNNER_TEMP 'lucia-real-unfamiliar'
    New-Item -ItemType Directory -Force $logDirectory | Out-Null
    $logFile=Join-Path $logDirectory ("unfamiliar-$($case.name)-$run.log")
    New-Item -ItemType Directory -Force (Join-Path $profile 'Roaming'),(Join-Path $profile 'Local'),(Join-Path $profile 'WebView2') | Out-Null
    $started=Get-Date
    $app=$null
    $createdPolicy=$false
    $exitCode=-1
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
      # An elevated hosted runner needs a per-executable WebView2 CDP policy.
      # Create only if absent; never overwrite an administrator's existing rule.
      New-Item -Path $policyPath -Force | Out-Null
      if($null -ne (Get-ItemProperty -Path $policyPath -Name $policyName -ErrorAction SilentlyContinue)){
          throw 'Refusing to replace existing WebView2 debugging policy'
      }
      New-ItemProperty -Path $policyPath -Name $policyName -PropertyType String -Value $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS | Out-Null
      $createdPolicy=$true
      $app=Start-Process -FilePath $ExecutablePath -WorkingDirectory $workingDirectory -PassThru
      $ready=$false
      for($i=0;$i -lt 85;$i++){
        try{
          $pages=Invoke-RestMethod "http://127.0.0.1:$port/json" -TimeoutSec 2
          if(@($pages | Where-Object {$_.type -eq 'page' -and $_.url -like '*tauri.localhost*'}).Count -gt 0){$ready=$true;break}
        }catch{}
        Start-Sleep -Milliseconds 400
      }
      if(-not $ready){throw 'No isolated native chatbox CDP target'}
      Push-Location $root
      try {
        & node 'tests\e2e\lucia-unfamiliar-source-acceptance.cjs' *> $logFile
        $exitCode=$LASTEXITCODE
      } finally {Pop-Location}
    } catch {
      Add-Content -Path $logFile -Value ("RUNNER_ERROR: "+$_.Exception.Message)
    } finally {
      if($app){Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue}
      if($createdPolicy){Remove-ItemProperty -Path $policyPath -Name $policyName -ErrorAction SilentlyContinue}
      foreach($name in 'APPDATA','LOCALAPPDATA','WEBVIEW2_USER_DATA_FOLDER','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','LUCIA_CDP_PORT','LUCIA_TEST_GOAL','LUCIA_EXPECTED_HOSTS','LUCIA_EXPECTED_SOURCE_URL','LUCIA_TEST_ACTION_MODEL','LUCIA_TEST_SUPERVISOR_MODEL','LUCIA_REQUIRE_RECOVERY'){
        [Environment]::SetEnvironmentVariable($name,$priorEnvironment[$name],'Process')
      }
      # The disposable profile is never re-used; cleanup is best-effort.
      Remove-Item -LiteralPath $profile -Force -Recurse -ErrorAction SilentlyContinue
    }
    $raw=if(Test-Path $logFile){Get-Content -Raw $logFile}else{''}
    $resultLine=@($raw -split "\r?\n" | Where-Object {$_ -match '^ORCHESTRATOR_RESULT '}) | Select-Object -Last 1
    $result=$null
    if($resultLine){try{$result=($resultLine -replace '^ORCHESTRATOR_RESULT ','') | ConvertFrom-Json}catch{}}
    $passed=$exitCode -eq 0 -and $null -ne $result -and $result.passed -eq $true -and $result.status -eq 'completed' -and
        $result.genuineModelUsed -eq $true -and $result.genuineModelActions -ge 1
    $record=[ordered]@{
      case=$case.name; run=$run; passed=$passed; exit_code=$exitCode
      seconds=[math]::Round(((Get-Date)-$started).TotalSeconds,1)
      final_status=if($result){$result.status}else{'no verified result'}
      actions=if($result){$result.actions}else{0}
      # Only quantitative model diagnostics, never the user goal or raw DOM.
      model_performance=if($result){@($result.modelPerformance)}else{@()}
      errors=if($result){@($result.errors)}else{@('No machine-readable verified completion')}
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
