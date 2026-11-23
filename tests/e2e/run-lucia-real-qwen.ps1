# Genuine Qwen 2.5 native Windows test using pinned Ollama portable binaries.
# Model and browser run only on a temporary GitHub-hosted Windows worker.
param([string]$Model = 'qwen2.5:3b-instruct')
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') {
    throw 'Real Qwen CI experiment requires an isolated GitHub-hosted Windows runner.'
}
if ($Model -ne 'qwen2.5:3b-instruct') {
    throw 'Only the reviewed qwen2.5:3b-instruct model is supported in this experiment.'
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
    $source = 'https://github.com/ollama/ollama/releases/download/v0.20.2/ollama-windows-amd64.zip'
    Write-Host 'Fetching pinned official Ollama Windows portable CLI.'
    Invoke-WebRequest -Uri $source -OutFile $zip -MaximumRetryCount 2 -RetryIntervalSec 3
    if ((Get-Item $zip).Length -lt 1000000) { throw 'Ollama portable archive appears truncated' }
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
    Write-Host 'Pulling qwen2.5:3b-instruct for real inference.'
    & $ollamaExe pull $Model
    if ($LASTEXITCODE -ne 0) { throw 'Could not pull the Qwen test model' }
    $catalog = Invoke-RestMethod -Uri 'http://127.0.0.1:11434/api/tags'
    if (-not ($catalog.models | Where-Object { $_.name -eq $Model -or $_.model -eq $Model })) {
        throw 'The downloaded Qwen model was not listed in Ollama'
    }
    Write-Host 'Running genuine Qwen decisions through native WebView2 integration.'
    & (Join-Path $root 'tests\e2e\run-lucia-multistep.ps1') -ExecutablePath (Join-Path $root 'target\release\browsphere-core.exe')
    if ($LASTEXITCODE -ne 0) {
        throw "Real Qwen native browser E2E failed with exit code $LASTEXITCODE"
    }
    Write-Host 'PASS: Real Qwen model browser E2E completed.'
} finally {
    if ($server) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    if (Test-Path (Join-Path $profile 'ollama.stderr.log')) {
        $last = Get-Content (Join-Path $profile 'ollama.stderr.log') -Tail 14
        if ($last) { Write-Host ('Ollama log tail: ' + ($last -join [Environment]::NewLine)) }
    }
    Remove-Item Env:LUCIA_E2E_REAL_MODEL -ErrorAction SilentlyContinue
}