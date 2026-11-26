param([string]$ExecutablePath = '', [int]$CdpPort = 0)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if(-not $ExecutablePath){$ExecutablePath=Join-Path $root 'target\release\browsphere-core.exe'}
if(-not (Test-Path $ExecutablePath)){throw "Executable not found: $ExecutablePath"}
$profile=Join-Path $env:TEMP ('lucia-chatbox-playback-'+[guid]::NewGuid().ToString('N'))
$env:APPDATA=Join-Path $profile 'Roaming'
$env:LOCALAPPDATA=Join-Path $profile 'Local'
$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $profile 'WebView2'
if($CdpPort -eq 0){
  $listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0)
  $listener.Start()
  try{$CdpPort=([Net.IPEndPoint]$listener.LocalEndpoint).Port}finally{$listener.Stop()}
}
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=$CdpPort --remote-allow-origins=*"
$env:LUCIA_CDP_PORT="$CdpPort"
Write-Output "CHATBOX_TEST_CDP_PORT=$CdpPort"
New-Item -ItemType Directory -Force $env:APPDATA,$env:LOCALAPPDATA,$env:WEBVIEW2_USER_DATA_FOLDER | Out-Null
$app=$null
try {
  Set-Location $root
  $app=Start-Process -FilePath $ExecutablePath -WorkingDirectory $root -PassThru
  Write-Output "CHATBOX_TEST_PID=$($app.Id)"
  & node (Join-Path $root 'tests\e2e\lucia-chatbox-playback-live.cjs')
  $result=$LASTEXITCODE
  Write-Output "CHATBOX_TEST_EXIT=$result"
  if($result -ne 0){exit $result}
} finally {
  if($app){Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue}
}
