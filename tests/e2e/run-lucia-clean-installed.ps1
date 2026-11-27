# Clean installed-app acceptance. Runs ONLY on a disposable GitHub-hosted Windows
# runner, never against the developer's profile or existing installation.
# A compiled executable passing tests is not an installation acceptance.
param(
    [Parameter(Mandatory=$true)][string]$InstallerPath,
    [string]$EvidenceDirectory = ""
)
$ErrorActionPreference = "Stop"
if ($env:GITHUB_ACTIONS -ne 'true' -or [string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
    throw 'Clean-install acceptance is permitted only on ephemeral GitHub Actions Windows runners.'
}
$source=(Resolve-Path -LiteralPath $InstallerPath -ErrorAction Stop).Path
if ([IO.Path]::GetFileName($source) -notmatch '-setup\.exe$') {
    throw 'Input is not a Tauri NSIS setup.exe artifact.'
}
$base=Join-Path $env:RUNNER_TEMP ('browsphere-clean-install-' + [Guid]::NewGuid().ToString('N'))
$destination=Join-Path $base 'app'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) { $EvidenceDirectory=Join-Path $base 'evidence' }
if ($destination.Contains(' ')) {
    # NSIS /D must be last and cannot be quoted. Do not silently install
    # into an unintended profile when the path contains whitespace.
    throw 'CI install directory contains a space; cannot safely use NSIS /D.'
}
New-Item -ItemType Directory -Force $base,$EvidenceDirectory | Out-Null
if (Test-Path $destination) { throw 'Refusing to overwrite an existing installation.' }
$start=[Diagnostics.Stopwatch]::StartNew()
$installation=$null
$executable=$null
$failures=[System.Collections.Generic.List[string]]::new()
$nativePassed=$false
$cloudPassed=$false
$uninstallPassed=$false
function Assert-ProcessTreeMeasurements([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Process tree performance evidence missing." }
    $metrics = Get-Content -Raw -LiteralPath $path | ConvertFrom-Json
    if ($metrics.process_tree_sample_count -lt 2 -or $metrics.webview2_child_count_peak -lt 1 -or
        $metrics.process_tree_working_set_mb_p95 -le 0 -or
        $metrics.process_tree_working_set_mb_peak -lt $metrics.webview2_working_set_mb_peak) {
        throw "The installed browser did not yield credible owned-WebView2 process tree samples."
    }
    Write-Host ("OWNED_WEBVIEW2_PROCESS_TREE_OK: observations={0} children_peak={1} total_p95_mb={2}" -f
        $metrics.process_tree_sample_count,$metrics.webview2_child_count_peak,$metrics.process_tree_working_set_mb_p95)
}
try {
    # Both /S and /D are NSIS documented options; /D=<path> must be last.
    $installation=Start-Process -FilePath $source -ArgumentList @('/S',"/D=$destination") -Wait -PassThru
    if ($installation.ExitCode -ne 0) { throw "NSIS installer exited with code $($installation.ExitCode)" }
    $installSeconds=[math]::Round($start.Elapsed.TotalSeconds,2)
    if (-not (Test-Path $destination -PathType Container)) {
        throw 'Silent NSIS setup returned success but no requested installation directory exists.'
    }
    $found=@(Get-ChildItem -LiteralPath $destination -File -Recurse -Filter 'browsphere-core.exe')
    if ($found.Count -ne 1) { throw "Expected one INSTALLED browsphere-core.exe; found $($found.Count)." }
    $executable=$found[0].FullName
    $sourceRoot=Join-Path $root 'target\release\browsphere-core.exe'
    if ($executable -eq $sourceRoot) { throw 'Installed executable resolves to the build tree.' }
    $installedHash=(Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash
    Write-Host ("CLEAN_INSTALL_OK: executable_name={0}; installed_size_bytes={1}; installer_seconds={2}" -f $found[0].Name,$found[0].Length,$installSeconds)

    # Real UI -> Rust -> installed WebView2. The runner uses a unique profile
    # and loopback model, never system accounts or external write privileges.
    & (Join-Path $root 'tests\e2e\run-lucia-multistep.ps1') -ExecutablePath $executable -CdpPort 9317 -ModelPort 18817 -FixturePort 18818 -PlaywrightInterop -UpstreamMcpInterop -PerfReportPath (Join-Path $EvidenceDirectory 'native-process-metrics.json')
    if ($LASTEXITCODE -ne 0) { throw 'Installed native Windows task E2E failed.' }
    Assert-ProcessTreeMeasurements (Join-Path $EvidenceDirectory 'native-process-metrics.json')
    $nativePassed=$true
    Write-Host 'CLEAN_INSTALLED_NATIVE_E2E_PASS'

    & (Join-Path $root 'tests\e2e\run-lucia-multistep.ps1') -ExecutablePath $executable -CloudCompat -CdpPort 9318 -ModelPort 18819 -FixturePort 18820 -PerfReportPath (Join-Path $EvidenceDirectory 'cloud-process-metrics.json')
    if ($LASTEXITCODE -ne 0) { throw 'Installed cloud-compatible Windows task E2E failed.' }
    Assert-ProcessTreeMeasurements (Join-Path $EvidenceDirectory 'cloud-process-metrics.json')
    $cloudPassed=$true
    Write-Host 'CLEAN_INSTALLED_CLOUD_E2E_PASS'

    # Uninstall lifecycle must not touch a developer path. Reject attempts
    # outside the one fresh installation root.
    $uninstallers=@(Get-ChildItem -LiteralPath $destination -Recurse -File |
        Where-Object { $_.Name -match '^uninstall(?:er)?(?:\s.*)?\.exe$' })
    if ($uninstallers.Count -ne 1) {
        throw "Expected one bundled NSIS uninstaller; found $($uninstallers.Count)."
    }
    $uninstall=Start-Process -FilePath $uninstallers[0].FullName -ArgumentList '/S' -Wait -PassThru
    if ($uninstall.ExitCode -ne 0) { throw "NSIS uninstaller returned $($uninstall.ExitCode)" }
    if (Test-Path -LiteralPath $executable) { throw 'Installed application remained after NSIS uninstall.' }
    $uninstallPassed=$true
    Write-Host 'CLEAN_INSTALLED_UNINSTALL_PASS'

    # No page content, prompts, form values or credentials in this JSON.
    [ordered]@{
        gate='windows-clean-installed-app'
        passed=$true
        ci_run=$env:GITHUB_RUN_ID
        commit=$env:GITHUB_SHA
        installer_sha256=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
        installed_exe_sha256=$installedHash
        installer_seconds=$installSeconds
        installed_native_webview2=$nativePassed
        installed_cloud_webview2=$cloudPassed
        clean_uninstall=$uninstallPassed
        limitations='Ephemeral Windows hosted runner, unsigned CI package and deterministic model fixture. No guarantee of consumer clean-room install, live-model autonomy or runtime security.'
    }|ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'installation-acceptance.json') -Encoding UTF8
} finally {
    # Avoid deleting paths outside this disposable CI directory or interfering
    # with unrelated WebView2 applications. Preserve evidence for CI artifacts.
    if (Test-Path -LiteralPath $destination -PathType Container) {
        Write-Host 'CI cleanup: installation root remains for ephemeral runner teardown.'
    }
}
