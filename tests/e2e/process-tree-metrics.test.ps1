$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'process-tree-metrics.ps1')
# 1 MB units are synthetic. Distinct out-of-tree WebView2 processes must not
# inflate metrics or be killed as a cleanup side effect.
$nodes=@(
    [pscustomobject]@{ProcessId=100;ParentProcessId=1;Name='browsphere-core.exe';WorkingSetSize=40MB;PrivatePageCount=22MB},
    [pscustomobject]@{ProcessId=200;ParentProcessId=100;Name='msedgewebview2.exe';WorkingSetSize=65MB;PrivatePageCount=34MB},
    [pscustomobject]@{ProcessId=201;ParentProcessId=200;Name='msedgewebview2.exe';WorkingSetSize=50MB;PrivatePageCount=25MB},
    [pscustomobject]@{ProcessId=202;ParentProcessId=201;Name='helper.exe';WorkingSetSize=5MB;PrivatePageCount=3MB},
    [pscustomobject]@{ProcessId=300;ParentProcessId=2;Name='msedgewebview2.exe';WorkingSetSize=900MB;PrivatePageCount=900MB}
)
$s=Get-BrowserProcessTreeSample -RootProcessId 100 -ProcessRows $nodes
if($s.process_count -ne 4 -or $s.webview2_child_count -ne 2 -or
   $s.process_tree_working_set_mb -ne 160 -or $s.webview2_working_set_mb -ne 115 -or
   $s.process_tree_private_bytes_mb -ne 84) {
    throw ('Wrong owned-process aggregation: ' + ($s | ConvertTo-Json -Compress))
}
if((Get-BrowserProcessTreeSample -RootProcessId 999 -ProcessRows $nodes) -ne $null){
    throw 'Missing process must not produce a fabricated zero-memory sample'
}
$percentile=Get-NearestRankPercentile -Values @([double]10,[double]20,[double]30,[double]100) -Quantile 0.95
if($percentile -ne 100) { throw "p95 was $percentile, expected 100" }
Write-Host 'PASS: process ownership, unrelated WebView2 exclusion, descendants, private bytes, percentile'
