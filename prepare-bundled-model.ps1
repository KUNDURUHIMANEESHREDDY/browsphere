param(
    [string]$ModelsRoot = $(if ($env:OLLAMA_MODELS) { $env:OLLAMA_MODELS } else { Join-Path $HOME '.ollama\models' }),
    [switch]$Build
)
$ErrorActionPreference = 'Stop'
$repo = $PSScriptRoot
$models = @(
    @{ Path='qwen3.5\0.8b'; Digest='889ea3b9143cdc32146c7d530ce5f3d452ec18069086f6f4167ce9ae72a9691f'; License='QWEN-LICENSE.txt' },
    @{ Path='lfm3\1.7b'; Digest='3793e1e6e2266919d6e015287221b41100175a98eddaba92e111aed3b05c93c4'; License='LFM3-LICENSE.txt' },
    @{ Path='lfm3-vl\2b'; Digest='56dac2e192ff0d9ab8507f4f1aeb4aed0463449bbbefaac79497f041c95f21eb'; License='LFM3-VL-LICENSE.txt' }
)
$dest = Join-Path $repo 'src-tauri\bundled-ai\models'
$stage = "$dest.staging"
$backup = "$dest.previous"
if (Test-Path -LiteralPath $backup) { throw 'Previous bundle backup exists; inspect it before rebuilding.' }
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Path (Join-Path $stage 'blobs') -Force | Out-Null
foreach ($spec in $models) {
    $manifestRel = 'manifests\registry.ollama.ai\library\' + $spec.Path
    $sourceManifest = Join-Path $ModelsRoot $manifestRel
    $manifest = Get-Content -LiteralPath $sourceManifest -Raw | ConvertFrom-Json
    $model = @($manifest.layers | Where-Object mediaType -eq 'application/vnd.ollama.image.model')
    if ($model.Count -ne 1 -or $model[0].digest -ne ('sha256:' + $spec.Digest)) { throw "Unexpected model revision: $($spec.Path)" }
    foreach ($layer in @($manifest.config) + @($manifest.layers)) {
        if ($layer.digest -notmatch '^sha256:[0-9a-f]{64}$') { throw 'Invalid layer digest' }
        $name = $layer.digest.Replace(':','-')
        $source = Join-Path (Join-Path $ModelsRoot 'blobs') $name
        $target = Join-Path (Join-Path $stage 'blobs') $name
        if ((Get-Item -LiteralPath $source).Length -ne $layer.size) { throw "Unexpected size: $name" }
        if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant() -ne $layer.digest.Substring(7)) { throw "Digest mismatch: $name" }
        if (-not (Test-Path -LiteralPath $target)) { Copy-Item -LiteralPath $source -Destination $target }
        if ((Get-Item -LiteralPath $target).Length -ne $layer.size -or (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash.ToLowerInvariant() -ne $layer.digest.Substring(7)) { throw "Copy verification failed: $name" }
    }
    $targetManifest = Join-Path $stage $manifestRel
    New-Item -ItemType Directory -Path (Split-Path $targetManifest) -Force | Out-Null
    Copy-Item -LiteralPath $sourceManifest -Destination $targetManifest
    $license = @($manifest.layers | Where-Object mediaType -eq 'application/vnd.ollama.image.license')
    if ($license.Count -ne 1) { throw "Model license missing: $($spec.Path)" }
    Copy-Item -LiteralPath (Join-Path (Join-Path $stage 'blobs') $license[0].digest.Replace(':','-')) -Destination (Join-Path $stage $spec.License)
    Write-Output "PASS: all layers and license verified for $($spec.Path)"
}
if (Test-Path -LiteralPath $dest) { Move-Item -LiteralPath $dest -Destination $backup }
try { Move-Item -LiteralPath $stage -Destination $dest }
catch {
    if (Test-Path -LiteralPath $backup) { Move-Item -LiteralPath $backup -Destination $dest }
    throw
}
if (Test-Path -LiteralPath $backup) { Remove-Item -LiteralPath $backup -Recurse -Force }
$bytes = (Get-ChildItem -LiteralPath $dest -Recurse -File | Measure-Object -Property Length -Sum).Sum
Write-Output "Verified three-model bundle: $bytes bytes at $dest"
if ($Build) { & (Join-Path $PSScriptRoot 'build-offline-installer.ps1') }
