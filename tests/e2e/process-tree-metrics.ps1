# Read-only performance sampler. A process belongs to Browsphere only if its
# parent chain reaches the exact test-launched native host PID. Never sum every
# unrelated msedgewebview2.exe in the machine.
function Get-BrowserProcessTreeSample {
    param(
        [Parameter(Mandatory)][int]$RootProcessId,
        [Parameter(Mandatory)][object[]]$ProcessRows
    )
    $members = [System.Collections.Generic.HashSet[int]]::new()
    [void]$members.Add($RootProcessId)
    for ($depth = 0; $depth -lt 24; $depth++) {
        $added = $false
        foreach ($entry in $ProcessRows) {
            $id = [int]$entry.ProcessId
            $parent = [int]$entry.ParentProcessId
            if (-not $members.Contains($id) -and $members.Contains($parent)) {
                [void]$members.Add($id)
                $added = $true
            }
        }
        if (-not $added) { break }
    }

    $owned = @($ProcessRows | Where-Object { $members.Contains([int]$_.ProcessId) })
    if (@($owned | Where-Object { [int]$_.ProcessId -eq $RootProcessId }).Count -ne 1) {
        return $null
    }
    $webviews = @($owned | Where-Object { $_.Name -ieq 'msedgewebview2.exe' })
    $bytes = ($owned | Measure-Object -Property WorkingSetSize -Sum).Sum
    $privateBytes = ($owned | Measure-Object -Property PrivatePageCount -Sum).Sum
    $host = @($owned | Where-Object { [int]$_.ProcessId -eq $RootProcessId })[0]
    $webviewBytes = ($webviews | Measure-Object -Property WorkingSetSize -Sum).Sum
    [pscustomobject]@{
        observed_at_utc = [DateTime]::UtcNow.ToString('o')
        process_count = $owned.Count
        webview2_child_count = $webviews.Count
        process_tree_working_set_mb = [math]::Round([double]$bytes / 1MB,1)
        webview2_working_set_mb = [math]::Round([double]$webviewBytes / 1MB,1)
        process_tree_private_bytes_mb = [math]::Round([double]$privateBytes / 1MB,1)
        native_host_working_set_mb = [math]::Round([double]$host.WorkingSetSize / 1MB,1)
    }
}
function Get-NearestRankPercentile {
    param([Parameter(Mandatory)][double[]]$Values,[ValidateRange(0.01,1)][double]$Quantile)
    if ($Values.Count -eq 0) { return $null }
    $sorted=@($Values | Sort-Object)
    $index=[math]::Max(0,[math]::Ceiling($Quantile*$sorted.Count)-1)
    return [math]::Round([double]$sorted[$index],1)
}
