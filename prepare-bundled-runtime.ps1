param([string]$RuntimeRoot = (Join-Path $env:LOCALAPPDATA 'Programs\Ollama'))
$ErrorActionPreference = 'Stop'
$source = Join-Path $RuntimeRoot 'ollama.exe'
$lib = Join-Path $RuntimeRoot 'lib\ollama'
if (-not (Test-Path $source) -or -not (Test-Path $lib)) { throw 'Provide the official extracted Ollama CLI and runtime directory.' }
$dest = Join-Path $PSScriptRoot 'src-tauri\bundled-ai\runtime'
$stage = "$dest.staging"
if (Test-Path $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Path (Join-Path $stage 'lib\ollama') -Force | Out-Null
Copy-Item -LiteralPath $source -Destination $stage
Get-ChildItem -LiteralPath $lib -File | Copy-Item -Destination (Join-Path $stage 'lib\ollama')
# Only CPU libraries are included; GPU acceleration is not required.
Invoke-WebRequest -UseBasicParsing -Uri 'https://raw.githubusercontent.com/ollama/ollama/main/LICENSE' -OutFile (Join-Path $stage 'OLLAMA-LICENSE.txt')
if (-not (Select-String -Path (Join-Path $stage 'OLLAMA-LICENSE.txt') -SimpleMatch 'MIT License' -Quiet)) { throw 'Unexpected runtime license' }
if (Test-Path $dest) { Remove-Item -LiteralPath $dest -Recurse -Force }
Move-Item -LiteralPath $stage -Destination $dest
$version = & (Join-Path $dest 'ollama.exe') --version
Write-Output "Bundled CLI: $version"
Get-ChildItem -LiteralPath $dest -Recurse -File | Measure-Object -Property Length -Sum | Select-Object Count,Sum
