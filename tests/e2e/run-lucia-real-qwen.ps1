# Genuine Qwen3.5 0.8B native Windows test using pinned Ollama portable binaries.
# Model and browser run only on a temporary GitHub-hosted Windows worker.
param(
  [string]$Model = 'qwen3.5:0.8b',
  [switch]$UnfamiliarSites,
  [ValidateSet('all','webdev','w3c')][string]$Scenario='all',
  [ValidateRange(1,3)][int]$Repetitions=3
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') {
    throw 'Real Qwen CI experiment requires an isolated GitHub-hosted Windows runner.'
}
if ($Model -ne 'qwen3.5:0.8b') {
    throw 'Only the pinned qwen3.5:0.8b local model is supported in this experiment.'
}
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $root
$profile = Join-Path $env:RUNNER_TEMP 'lucia-ollama-e2e'
$binaryDir = Join-Path $profile 'portable'
$zip = Join-Path $profile 'ollama-windows-amd64.zip'
$env:OLLAMA_HOST = '127.0.0.1:11434'
$env:OLLAMA_MODELS = Join-Path $profile 'models'
$env:OLLAMA_NUM_PARALLEL = '1'
$env:OLLAMA_MAX_LOADED_MODELS = '1'
$env:LUCIA_E2E_REAL_MODEL = $Model
New-Item -Force -ItemType Directory $profile,$binaryDir,$env:OLLAMA_MODELS | Out-Null
$server = $null
try {
    # Pin to a published official release. Do not pipe remote scripts to PowerShell.
    $source = 'https://github.com/ollama/ollama/releases/download/v0.40.18/ollama-windows-amd64.zip'
    Write-Host 'Fetching pinned official Ollama Windows portable CLI.'
    Invoke-WebRequest -Uri $source -OutFile $zip -MaximumRetryCount 2 -RetryIntervalSec 3
    # Pin both the official release URL and GitHub's published asset checksum.
    # The full CPU-capable Windows archive is ~1.7 GB; no arbitrary URL fallback.
    if ((Get-Item $zip).Length -ne 1717307988) { throw 'Official Ollama archive length mismatch' }
    $expectedSha256 = 'ca97292ac1e9acc77fcf473bb2741fa6acd12e3798f261b419680068b7cc4c16'
    if ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expectedSha256) {
        throw 'Official Ollama archive checksum mismatch'
    }
    Expand-Archive -Path $zip -DestinationPath $binaryDir -Force
    $ollamaExe = Get-ChildItem -Path $binaryDir -Filter ollama.exe -File -Recurse | Select-Object -First 1 -ExpandProperty FullName
    if (-not $ollamaExe) { throw 'Portable package did not contain ollama.exe' }
    $stdout = Join-Path $profile 'ollama.stdout.log'
    $stderr = Join-Path $profile 'ollama.stderr.log'
    $server = Start-Process -FilePath $ollamaExe -ArgumentList @('serve') -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $ready = $false
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        $server.Refresh()
        if ($server.HasExited) { throw "Ollama service exited with code $($server.ExitCode)" }
        try {
            $health = Invoke-RestMethod -Uri 'http://127.0.0.1:11434/api/tags' -TimeoutSec 2
            if ($null -ne $health.models) { $ready = $true; break }
        } catch {}
        Start-Sleep -Milliseconds 500
    }
    if (-not $ready) { throw 'Real Ollama server never became available on loopback' }
    Write-Host 'Pulling genuine Qwen and the independent local supervisor for research verification.'
    & $ollamaExe pull $Model
    if ($LASTEXITCODE -ne 0) { throw 'Could not pull the Qwen test model' }
    & $ollamaExe pull 'lfm3:1.7b'
    if ($LASTEXITCODE -ne 0) { throw 'Could not pull the supervisor model' }
    $catalog = Invoke-RestMethod -Uri 'http://127.0.0.1:11434/api/tags'
    if (-not ($catalog.models | Where-Object { $_.name -eq $Model -or $_.model -eq $Model })) {
        throw 'The downloaded Qwen model was not listed in Ollama'
    }
    if ($UnfamiliarSites) {
        # A successfully downloaded model is not proof that it can respond
        # within Lucia's actual inference deadline. Exercise the genuine
        # Ollama binary once without any page content, then report only numeric
        # timing metadata (not prompts, tokens, credentials, or model output).
        $probeWatch=[Diagnostics.Stopwatch]::StartNew()
        $probeBody=@{
            model=$Model
            prompt='Return exactly this JSON object and nothing else: {"type":"scroll","delta_y":150}'
            format='json'
            # Preflight must test a completed JSON answer, not spend the
            # short generation budget on hidden reasoning tokens.
            think=$false
            stream=$false
            keep_alive='20m'
            options=@{temperature=0;num_predict=120;num_ctx=3072}
        } | ConvertTo-Json -Depth 6 -Compress
        $probe=Invoke-RestMethod -Uri 'http://127.0.0.1:11434/api/generate' -Method Post -Body $probeBody -ContentType 'application/json' -TimeoutSec 120
        $probeWatch.Stop()
        if(-not $probe.done -or [string]::IsNullOrWhiteSpace([string]$probe.response)) {
            throw 'Real Qwen preflight returned no completed model response; browser task repeatability cannot be measured.'
        }
        try {
            $preflightAction=([string]$probe.response) | ConvertFrom-Json -ErrorAction Stop
        } catch {
            throw 'Real Qwen preflight returned invalid JSON; real-site task acceptance is blocked.'
        }
        if ($preflightAction.type -ne 'scroll' -or $preflightAction.delta_y -ne 150) {
            throw 'Real Qwen preflight did not follow the requested action schema.'
        }
        $metrics=[ordered]@{
            wall_ms=[math]::Round($probeWatch.Elapsed.TotalMilliseconds,1)
            model_load_ms=[math]::Round([double]$probe.load_duration/1000000,1)
            prompt_eval_ms=[math]::Round([double]$probe.prompt_eval_duration/1000000,1)
            generation_ms=[math]::Round([double]$probe.eval_duration/1000000,1)
            prompt_tokens=[int]$probe.prompt_eval_count
            generated_tokens=[int]$probe.eval_count
            tokens_per_second=if($probe.eval_duration -gt 0){[math]::Round(([double]$probe.eval_count*1000000000)/[double]$probe.eval_duration,1)}else{$null}
            done_reason=[string]$probe.done_reason
        }
        Write-Host ('REAL_QWEN_PREFLIGHT '+($metrics|ConvertTo-Json -Compress))
        $attempts=$Repetitions * $(if($Scenario -eq 'all'){2}else{1})
        Write-Host ("Running {0} genuine-model trial(s); scenario={1}; no unverified trial counts as success." -f $attempts,$Scenario)
        $resultPath=Join-Path $env:RUNNER_TEMP 'lucia-unfamiliar-evidence.json'
        & (Join-Path $root 'tests\e2e\run-lucia-unfamiliar-hosted.ps1') -ExecutablePath (Join-Path $root 'target\release\browsphere-core.exe') -Scenario $Scenario -Repetitions $Repetitions -ActionModel $Model -SupervisorModel $Model -ReportPath $resultPath
        if ($LASTEXITCODE -ne 0) {
            throw "Real Qwen unfamiliar-site acceptance failed; resource blocks and unverified outcomes are not passes."
        }
        if($Scenario -eq 'all' -and $Repetitions -eq 3) {
            Write-Host 'PASS: Six independent real-Qwen unfamiliar-site task completions were source-verified.'
        } else {
            Write-Host 'DIAGNOSTIC_PASS: limited trial(s) passed; full 6/6 release acceptance has NOT run.'
        }
    } else {
        Write-Host 'Running genuine Qwen decisions through native WebView2 fixture integration.'
        & (Join-Path $root 'tests\e2e\run-lucia-multistep.ps1') -ExecutablePath (Join-Path $root 'target\release\browsphere-core.exe')
        if ($LASTEXITCODE -ne 0) {
            throw "Real Qwen native browser fixture E2E failed with exit code $LASTEXITCODE"
        }
        Write-Host 'PASS: Real Qwen model browser fixture E2E completed.'
    }
} finally {
    if ($server) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    if (Test-Path (Join-Path $profile 'ollama.stderr.log')) {
        $last = Get-Content (Join-Path $profile 'ollama.stderr.log') -Tail 14
        if ($last) { Write-Host ('Ollama log tail: ' + ($last -join [Environment]::NewLine)) }
    }
    Remove-Item Env:LUCIA_E2E_REAL_MODEL -ErrorAction SilentlyContinue
}