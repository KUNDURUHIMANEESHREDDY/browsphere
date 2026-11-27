# Microsoft Playwright MCP (Apache-2.0) opt-in adapter for isolated WebView2.
# Does not install into Browsphere, change settings, or start CDP.
[CmdletBinding()]
param(
    [ValidateRange(1024,65535)][int]$CdpPort=9222,
    [ValidateRange(1024,65535)][int]$McpPort=8931,
    [switch]$VerifyOnly,
    [switch]$AcknowledgeCdpAccess
)
$ErrorActionPreference='Stop'
if($CdpPort -eq $McpPort){throw "CDP and MCP ports must differ"}
if(-not $VerifyOnly -and -not $AcknowledgeCdpAccess){
    throw "Playwright MCP has full access to the attached browser. Use an isolated disposable Browsphere profile and pass -AcknowledgeCdpAccess."
}
$cdp="http://127.0.0.1:$CdpPort"
$targets=@(Invoke-RestMethod -Uri "$cdp/json/list" -TimeoutSec 4)
$nativeTargets=@($targets | Where-Object {
    $_.type -eq 'page' -and $_.url -like '*tauri.localhost*'
})
if($nativeTargets.Count -eq 0){throw "No native Browsphere WebView2 shell target at $cdp. Start an isolated local app with CDP enabled."}
Write-Host "Verified Browsphere CDP shell target: $cdp"
if($VerifyOnly){return}
$listener=Get-NetTCPConnection -LocalPort $McpPort -State Listen -ErrorAction SilentlyContinue
if($listener){throw "MCP port $McpPort is already in use"}
$npx=Get-Command npx.cmd -ErrorAction Stop
Write-Host "Launching upstream @playwright/mcp@0.0.86 on 127.0.0.1:$McpPort"
Write-Host "This trusted external process is NOT sandboxed and can control this browser via CDP."
Write-Host "In Browsphere Settings > AI & ML > MCP Tools approve ONLY browser_snapshot for the playwright server."
Write-Host "Connect via http://127.0.0.1:$McpPort/mcp; stop this terminal process to disconnect."
& $npx.Source --yes '@playwright/mcp@0.0.86' --host 127.0.0.1 --port $McpPort --cdp-endpoint $cdp
if($LASTEXITCODE -ne 0){throw "Playwright MCP exited with status $LASTEXITCODE"}
