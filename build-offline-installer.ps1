param([switch]$SkipAppBuild)
$ErrorActionPreference = 'Stop'
$repo = $PSScriptRoot
$payload = Join-Path $repo 'src-tauri\bundled-ai'
$dest = Join-Path $repo 'dist\Browsphere-Offline'
if (-not (Test-Path (Join-Path $payload 'runtime\ollama.exe'))) { throw 'Run prepare-bundled-runtime.ps1 first' }
if (-not (Test-Path (Join-Path $payload 'models\LFM3-VL-LICENSE.txt'))) { throw 'Run prepare-bundled-model.ps1 first' }
$files = @(Get-ChildItem -LiteralPath $payload -Recurse -File | Sort-Object FullName)
$relative = @($files | ForEach-Object { $_.FullName.Substring($payload.Length+1) })
if ($relative.Count -lt 40 -or @($relative | Where-Object { $_ -match '["$]' }).Count -gt 0) { throw 'Invalid payload' }
$hooks = [System.Collections.Generic.List[string]]::new()
$hooks.Add('!macro NSIS_HOOK_PREINSTALL')
foreach ($file in $relative) {
    $hooks.Add('  IfFileExists "$EXEDIR\bundled-ai\'+$file+'" +3 0')
    $hooks.Add('  MessageBox MB_ICONSTOP "Missing offline payload. Keep bundled-ai beside setup."')
    $hooks.Add('  Abort')
}
$hooks.Add('!macroend')
$hooks.Add('!macro NSIS_HOOK_POSTINSTALL')
foreach ($file in $relative) {
    $parent = Split-Path $file -Parent
    $target = '$INSTDIR\bundled-ai'
    if ($parent) { $target += '\'+$parent }
    $hooks.Add('  CreateDirectory "'+$target+'"')
    $hooks.Add('  ClearErrors')
    $hooks.Add('  CopyFiles /SILENT "$EXEDIR\bundled-ai\'+$file+'" "'+$target+'"')
    $hooks.Add('  IfErrors 0 +3')
    $hooks.Add('  MessageBox MB_ICONSTOP "Offline model copy failed. Check available disk space."')
    $hooks.Add('  Abort')
}
$hooks.Add('!macroend')
$hooks.Add('!macro NSIS_HOOK_PREUNINSTALL')
foreach ($file in $relative) { $hooks.Add('  Delete "$INSTDIR\bundled-ai\'+$file+'"') }
$dirs = @($relative | ForEach-Object { $p=Split-Path $_ -Parent; while($p) { $p; $p=Split-Path $p -Parent } } | Sort-Object -Unique | Sort-Object Length -Descending)
foreach ($dir in $dirs) { $hooks.Add('  RMDir "$INSTDIR\bundled-ai\'+$dir+'"') }
$hooks.Add('  RMDir "$INSTDIR\bundled-ai"')
$hooks.Add('!macroend')
$hookPath = Join-Path $payload 'installer-hooks.nsh'
# Store hooks outside the copied payload, so repeated builds do not package generated hooks.
$hookPath = Join-Path $repo 'src-tauri\offline-installer-hooks.nsh'
$hooks | Set-Content -LiteralPath $hookPath -Encoding ASCII
Push-Location (Join-Path $repo 'src-tauri')
try {
    if (-not $SkipAppBuild) {
        cargo tauri build --no-bundle --ci
        if ($LASTEXITCODE -ne 0) { throw 'App build failed' }
    }
    cargo tauri bundle --bundles nsis --config tauri.bundled-model.conf.json --ci
    if ($LASTEXITCODE -ne 0) { throw 'Offline installer build failed' }
} finally { Pop-Location }
$installer = Get-ChildItem (Join-Path $repo 'target\release\bundle\nsis') -File | Where-Object Name -match '^Browsphere[ _-]Offline.*setup\.exe$' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $installer) { throw 'Offline installer not found' }
New-Item -ItemType Directory -Path $dest -Force | Out-Null
Copy-Item -LiteralPath $installer.FullName -Destination (Join-Path $dest 'Browsphere-Offline-setup.exe') -Force
$destPayload=Join-Path $dest 'bundled-ai'
New-Item -ItemType Directory -Path $destPayload -Force | Out-Null
foreach ($file in $relative) {
    $source=Join-Path $payload $file
    $target=Join-Path $destPayload $file
    New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null
    Copy-Item -LiteralPath $source -Destination $target -Force
    if ((Get-FileHash $source).Hash -ne (Get-FileHash $target).Hash) { throw "Payload copy failed: $file" }
}
Write-Output "Offline distribution created at $dest. Keep bundled-ai beside setup.exe."
