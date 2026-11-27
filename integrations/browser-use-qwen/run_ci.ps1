# CI only. Browser Use runs model-generated Python in a separate disposable runner.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:LUCIA_UPSTREAM_AUTONOMY_TEST -ne '1') {
    throw 'Browser Use autonomous research requires explicit isolated Windows CI'
}
$dir = Join-Path $env:RUNNER_TEMP 'lucia-upstream-ollama'
$archive = Join-Path $dir 'ollama-portable.zip'
$binaries = Join-Path $dir 'portable'
$env:OLLAMA_MODELS = Join-Path $dir 'models'
$env:OLLAMA_HOST = '127.0.0.1:11434'
$env:OLLAMA_NUM_PARALLEL = '1'
$env:OLLAMA_MAX_LOADED_MODELS = '1'
$env:OLLAMA_KEEP_ALIVE = '0'
New-Item -ItemType Directory -Force $dir,$binaries,$env:OLLAMA_MODELS | Out-Null
$server = $null
try {
    $source='https://github.com/ollama/ollama/releases/download/v0.20.2/ollama-windows-amd64.zip'
    Invoke-WebRequest -Uri $source -OutFile $archive -MaximumRetryCount 2 -RetryIntervalSec 3
    if ((Get-Item $archive).Length -lt 1000000) { throw 'Ollama archive appears truncated' }
    Expand-Archive -Path $archive -DestinationPath $binaries
    $exe=Get-ChildItem $binaries -Recurse -File -Filter ollama.exe | Select-Object -First 1 -ExpandProperty FullName
    if (-not $exe) { throw 'Official Ollama portable executable missing' }
    $server=Start-Process -FilePath $exe -ArgumentList @('serve') -PassThru -RedirectStandardOutput (Join-Path $dir 'ollama-stdout.log') -RedirectStandardError (Join-Path $dir 'ollama-stderr.log')
    $ready=$false
    for ($i=0;$i -lt 100;$i++) {
        $server.Refresh()
        if ($server.HasExited) { throw 'Ollama server exited unexpectedly' }
        try {
            $tags=Invoke-RestMethod 'http://127.0.0.1:11434/api/tags' -TimeoutSec 2
            if ($null -ne $tags.models) { $ready=$true; break }
        } catch {}
        Start-Sleep -Milliseconds 500
    }
    if (-not $ready) { throw 'Local Ollama server unavailable' }
    & $exe pull 'qwen2.5:3b-instruct'
    if ($LASTEXITCODE -ne 0) { throw 'Genuine Qwen model pull failed' }
    $tags=Invoke-RestMethod 'http://127.0.0.1:11434/api/tags'
    if (-not ($tags.models | Where-Object { $_.name -eq 'qwen2.5:3b-instruct' })) {
        throw 'Downloaded Qwen model not present in Ollama catalog'
    }
    $failures=0
    foreach ($site in @('webdev','w3c')) {
        $report=Join-Path $env:RUNNER_TEMP "lucia-browser-use-$site.json"
        & python integrations/browser-use-qwen/research_trial.py --case $site --report $report
        if ($LASTEXITCODE -ne 0) {
            $failures++
            Write-Warning "Genuine Qwen failed independent verification at $site"
        }
    }
    if ($failures -gt 0) { throw "$failures of 2 genuine Qwen tasks failed" }
    Write-Host 'BROWSER_USE_REAL_QWEN_2_OF_2_PASS'
} finally {
    if ($server) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
}
