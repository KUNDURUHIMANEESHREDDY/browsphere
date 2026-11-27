# Real-website Lucia acceptance for all eligible installed Ollama model tags.
# Runs the existing native WebView2 acceptance sequentially, never a fake site.
param(
  [ValidateSet('webdev','w3c','all')][string]$Scenario='webdev',
  [ValidateRange(1,10)][int]$Repetitions=1,
  [string[]]$Models=@(),
  [string]$SupervisorModel='lfm3:1.7b',
  [string]$ExecutablePath='',
  [string]$ReportPath=''
)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if(-not $ExecutablePath){$ExecutablePath=Join-Path $root 'target\release\browsphere-core.exe'}
if(-not (Test-Path $ExecutablePath)){throw "Missing fresh native executable: $ExecutablePath"}
if(-not $ReportPath){$ReportPath=Join-Path $root 'logs\lucia-installed-model-matrix.json'}
$tags=Invoke-RestMethod 'http://127.0.0.1:11434/api/tags' -TimeoutSec 8
if(-not $tags.models){throw 'No local Ollama models installed'}
$records=[System.Collections.Generic.List[object]]::new()
$eligible=0
foreach($model in @($tags.models)){
  $name=[string]$model.name
  if($Models.Count -gt 0 -and $Models -notcontains $name){continue}
  $status='metadata_unavailable'
  $detail=''
  $runReport=''
  $verified=0
  $failed=0
  $blocked=0
  try {
    $show=Invoke-RestMethod 'http://127.0.0.1:11434/api/show' -Method Post -ContentType 'application/json' -Body (@{model=$name}|ConvertTo-Json -Compress) -TimeoutSec 8
    if(@($show.capabilities) -notcontains 'completion') {
      $status='unsupported_text_completion'
    } else {
      $eligible++
      $free=[math]::Floor((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory/1024)
      # File size is a lower bound, not the model's actual RAM footprint.
      # Retain an additional reserve for WebView2, KV cache and Windows.
      $modelMiB=[math]::Ceiling([double]$model.size/1MB)
      $needed=[math]::Max(3072,[math]::Ceiling($modelMiB*1.5+1536))
      if($free -lt $needed) {
        $status='resource_blocked_model_budget'
        $detail="$free MiB free; estimated safe launch gate $needed MiB"
        $scenarioCount=if($Scenario -eq 'all'){2}else{1}
        $blocked=$Repetitions*$scenarioCount
      } else {
        $sha=[System.Security.Cryptography.SHA256]::Create()
        try {$short=([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($name))).Replace('-','').ToLowerInvariant()).Substring(0,12)}
        finally {$sha.Dispose()}
        $runReport=Join-Path $root "logs\lucia-model-$short.json"
        $runner=Join-Path $PSScriptRoot 'run-lucia-unfamiliar-reliability.ps1'
        & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $runner -Scenario $Scenario -Repetitions $Repetitions -ActionModel $name -SupervisorModel $SupervisorModel -ExecutablePath $ExecutablePath -ReportPath $runReport
        $trialExit=$LASTEXITCODE
        if(Test-Path $runReport){
          $trial=Get-Content -Raw $runReport | ConvertFrom-Json
          $verified=[int]$trial.passed
          $failed=[int]$trial.failed
          $blocked=[int]$trial.resource_blocked
          $status=if($trial.all_passed -and $trialExit -eq 0){'passed'}elseif($trial.executed -gt 0){'failed_verification'}else{'resource_blocked'}
        }else{$status='missing_trial_report'}
      }
    }
  } catch {
    $detail=$_.Exception.Message
  }
  $records.Add([ordered]@{model=$name;status=$status;verified=$verified;failed=$failed;resource_blocked=$blocked;reason=$detail;run_report=$runReport})
  Write-Output "MODEL_RESULT $name $status verified=$verified failed=$failed blocked=$blocked"
}
if($Models.Count -gt 0){
  foreach($requested in $Models){
    if(-not @($records|Where-Object {$_.model -eq $requested}).Count){throw "Requested installed model not found: $requested"}
  }
}
$summary=[ordered]@{
  generated_utc=(Get-Date).ToUniversalTime().ToString('o')
  acceptance='Only real websites, a native WebView2 action from the selected model, and independent source verification count as passes'
  scenario=$Scenario;repetitions=$Repetitions;eligible_models=$eligible
  passed_models=@($records|Where-Object {$_.status -eq 'passed'}).Count
  failed_models=@($records|Where-Object {$_.status -eq 'failed_verification'}).Count
  blocked_models=@($records|Where-Object {$_.status -like 'resource_blocked*'}).Count
  records=@($records)
}
New-Item -ItemType Directory -Force (Split-Path -Parent $ReportPath)|Out-Null
$summary|ConvertTo-Json -Depth 7|Set-Content -Encoding UTF8 $ReportPath
Write-Output "MODEL_MATRIX_REPORT $ReportPath"
if($summary.passed_models -ne $eligible){exit 1}
