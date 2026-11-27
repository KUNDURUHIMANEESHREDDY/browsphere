# Fresh ephemeral Windows NSIS installation acceptance for Browsphere/HPR.
# Does not touch the developer's actual Browsphere installation or profile.
param([string]$InstallerPath = "")
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $root
if(-not $InstallerPath){
    $candidate=Get-ChildItem (Join-Path $root 'target\release\bundle\nsis') -Filter '*-setup.exe' -File -ErrorAction SilentlyContinue|
        Sort-Object LastWriteTime -Descending|Select-Object -First 1
    if(-not $candidate){throw 'NSIS installer not available'}
    $InstallerPath=$candidate.FullName
}
if(-not(Test-Path $InstallerPath)){throw "Missing NSIS installer: $InstallerPath"}
$installDir=Join-Path $env:RUNNER_TEMP ('BrowsphereHprInstalled'+[guid]::NewGuid().ToString('N'))
if($installDir -match '\s'){throw 'NSIS /D requires a path without spaces'}
if(Test-Path $installDir){throw 'Refusing to reuse an existing installation directory'}

Write-Host 'Installing unsigned CI-test Browsphere NSIS bundle to an isolated directory'
$proc=Start-Process -FilePath $InstallerPath -ArgumentList "/S /D=$installDir" -PassThru -Wait
if($proc.ExitCode -ne 0){throw "NSIS exited with code $($proc.ExitCode)"}
if(-not(Test-Path $installDir)){throw 'NSIS did not create the requested installation directory'}
$exe=Get-ChildItem -Path $installDir -Filter 'browsphere-core.exe' -File -Recurse -ErrorAction SilentlyContinue|Select-Object -First 1
if(-not $exe){throw 'Installer did not put Browsphere core executable in the installation directory'}
Write-Host "Installed executable verified: $($exe.FullName)"
& (Join-Path $root 'tests\e2e\run-hyperresearch-webview.ps1') -ExecutablePath $exe.FullName
if($LASTEXITCODE -ne 0){throw 'Installed-app WebView2 MCP and independent source check failed'}
Write-Host 'PASS: isolated NSIS installation launched; WebView2 MCP retrieval and original-source verification passed'
