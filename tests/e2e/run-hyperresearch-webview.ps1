# Isolated Windows Tauri/WebView2 research acceptance against the real bridge.
# Reuses the existing public Wikipedia verification diagnostic.
param(
    [string]$ExecutablePath = "",
    [int]$CdpPort = 9292,
    [int]$BridgePort = 8765,
    [int]$ModelPort = 18766,
    [int]$FixturePort = 18764
)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $root
if (-not $ExecutablePath) {
    $ExecutablePath=Join-Path $root 'target\release\browsphere-core.exe'
}
if (-not (Test-Path $ExecutablePath)) { throw "Native Browsphere executable missing: $ExecutablePath" }
# The Python bridge currently uses a fixed loopback port, so do not allow
# the runner to imply it can rebind independently.
if ($BridgePort -ne 8765) { throw 'Bridge currently requires loopback port 8765' }

$profile=Join-Path ([IO.Path]::GetTempPath()) ('browsphere-hpr-webview-'+[guid]::NewGuid().ToString('N'))
$env:APPDATA=Join-Path $profile 'Roaming'
$env:LOCALAPPDATA=Join-Path $profile 'Local'
$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $profile 'WebView2'
$env:LUCIA_CDP_PORT="$CdpPort"
$env:LUCIA_MODEL_PORT="$ModelPort"
$env:LUCIA_FIXTURE_PORT="$FixturePort"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=$CdpPort --remote-allow-origins=*"
New-Item -ItemType Directory -Force $env:APPDATA,$env:LOCALAPPDATA,$env:WEBVIEW2_USER_DATA_FOLDER | Out-Null
$vault=Join-Path $profile 'SyntheticPublicVault'
$bridgeOut=Join-Path $profile 'bridge.stdout.log'
$bridgeErr=Join-Path $profile 'bridge.stderr.log'
$modelOut=Join-Path $profile 'model.stdout.log'
$modelErr=Join-Path $profile 'model.stderr.log'
$gateway=$null;$model=$null;$app=$null
$cdpPolicyPath='HKLM:\SOFTWARE\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments'
$cdpPolicyName=[IO.Path]::GetFileName($ExecutablePath)
$createdPolicy=$false

function Wait-Tcp([int]$targetPort,[int]$attempts=50) {
    for($i=0;$i -lt $attempts;$i++){
        $client=[System.Net.Sockets.TcpClient]::new()
        try {
            $task=$client.ConnectAsync('127.0.0.1',$targetPort)
            if($task.Wait(800) -and $client.Connected){return}
        } catch {} finally {$client.Dispose()}
        Start-Sleep -Milliseconds 250
    }
    throw "Dependency unavailable on port $targetPort"
}

try {
    & python (Join-Path $root 'tests\e2e\seed_hyperresearch_webview_vault.py') $vault
    if($LASTEXITCODE -ne 0){throw 'Cannot seed synthetic Hyperresearch vault'}
    $configDir=Join-Path $env:LOCALAPPDATA 'BrowsphereAI'
    New-Item -ItemType Directory -Force $configDir|Out-Null
    $config=@{
        hyperresearch=@{
            url='http://127.0.0.1:8765/mcp'
            read_only_tools=@('search_notes','read_note','check_source','list_sources','vault_status')
        }
    } | ConvertTo-Json -Depth 5
    [System.IO.File]::WriteAllText((Join-Path $configDir 'mcp-servers.json'),$config)

    $bridgeScript=Join-Path $root 'tools\hyperresearch_readonly_gateway.py'
    $gateway=Start-Process -FilePath 'python' -ArgumentList @('"' + $bridgeScript + '"','--vault','"' + $vault + '"') -WorkingDirectory $root -PassThru -RedirectStandardOutput $bridgeOut -RedirectStandardError $bridgeErr
    Wait-Tcp 8765
    $modelScript=Join-Path $root 'tests\e2e\lucia-local-model-server.cjs'
    $model=Start-Process -FilePath 'node' -ArgumentList @('"' + $modelScript + '"') -WorkingDirectory $root -PassThru -RedirectStandardOutput $modelOut -RedirectStandardError $modelErr
    Wait-Tcp $ModelPort

    if($env:GITHUB_ACTIONS -eq 'true'){
        New-Item -Path $cdpPolicyPath -Force|Out-Null
        $existing=Get-ItemProperty -Path $cdpPolicyPath -Name $cdpPolicyName -ErrorAction SilentlyContinue
        if($null -ne $existing){throw 'Refusing to override existing WebView2 policy'}
        New-ItemProperty -Path $cdpPolicyPath -Name $cdpPolicyName -PropertyType String -Value $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS|Out-Null
        $createdPolicy=$true
    }
    $app=Start-Process -FilePath $ExecutablePath -WorkingDirectory $root -PassThru
    Wait-Tcp $CdpPort 100

    Write-Host 'Checking native Lucia chatbox read-through against real Hyperresearch MCP'
    & node (Join-Path $root 'tests\e2e\lucia-hyperresearch-chatbox.cjs')
    if($LASTEXITCODE -ne 0){throw 'WebView2 Hyperresearch chatbox acceptance failed'}

    Write-Host 'Checking independent live original-source WebView2 verification'
    & node (Join-Path $root 'tests\e2e\lucia-wikipedia-article.cjs')
    if($LASTEXITCODE -ne 0){throw 'Independent original-source browser verification failed'}
    Write-Host 'PASS Hyperresearch cached read and separate original-source browser verification'
} finally {
    if($app){Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue}
    if($model){Stop-Process -Id $model.Id -Force -ErrorAction SilentlyContinue}
    if($gateway){Stop-Process -Id $gateway.Id -Force -ErrorAction SilentlyContinue}
    foreach($p in @($bridgeErr,$modelErr)){
        if(Test-Path $p){Get-Content $p -Tail 15 -ErrorAction SilentlyContinue}
    }
    if($createdPolicy){
        Remove-ItemProperty -Path $cdpPolicyPath -Name $cdpPolicyName -ErrorAction SilentlyContinue
    }
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
}
