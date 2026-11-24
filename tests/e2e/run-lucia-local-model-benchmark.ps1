# Compare real, installed local reasoning models using identical isolated browser fixtures.
param([string]$ExecutablePath = "")
$ErrorActionPreference = 'Stop'
$models = @('lfm3:1.7b','qwen3.5:0.8b')
$catalog = Invoke-RestMethod 'http://127.0.0.1:11434/api/tags'
foreach ($model in $models) {
    if (-not ($catalog.models | Where-Object { $_.name -eq $model })) {
        throw "Install $model before running this benchmark"
    }
}
$previous = $env:LUCIA_E2E_REAL_MODEL
$results = @()
try {
    foreach ($model in $models) {
        $env:LUCIA_E2E_REAL_MODEL = $model
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $passed = $false
        try {
            & (Join-Path $PSScriptRoot 'run-lucia-multistep.ps1') -ExecutablePath $ExecutablePath
            $passed = $true
        } catch {
            Write-Warning ("Benchmark failed for " + $model + ": " + $_.Exception.Message)
        } finally {
            $watch.Stop()
            $results += [pscustomobject]@{model=$model; passed=$passed; elapsedSeconds=[Math]::Round($watch.Elapsed.TotalSeconds,2)}
        }
    }
} finally {
    $env:LUCIA_E2E_REAL_MODEL = $previous
    $results | ConvertTo-Json
}
if ($results | Where-Object { -not $_.passed }) { exit 1 }
