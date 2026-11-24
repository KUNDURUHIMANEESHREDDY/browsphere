param([string]$ModelsRoot = (Join-Path $PSScriptRoot 'src-tauri\bundled-ai\models'), [switch]$VisionOnly)
$ErrorActionPreference = 'Stop'
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = $listener.LocalEndpoint.Port
$listener.Stop()
$env:OLLAMA_HOST = "127.0.0.1:$port"
$env:OLLAMA_MODELS = (Resolve-Path -LiteralPath $ModelsRoot).Path
$env:OLLAMA_NOPRUNE = 'true'
$env:OLLAMA_NO_CLOUD = '1'
$env:OLLAMA_MAX_LOADED_MODELS = '1'
$ollama = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'src-tauri\bundled-ai\runtime\ollama.exe')).Path
$env:OLLAMA_LLM_LIBRARY = 'cpu'
$proc = Start-Process -FilePath $ollama -ArgumentList 'serve' -PassThru -WindowStyle Hidden
try {
    $base = "http://127.0.0.1:$port"
    $ready = $false
    for ($i=0; $i -lt 30; $i++) {
        if ($proc.HasExited) { throw 'Isolated runtime exited' }
        try { $tags = Invoke-RestMethod "$base/api/tags" -TimeoutSec 2; $ready = $true; break } catch { Start-Sleep -Milliseconds 500 }
    }
    if (-not $ready) { throw 'Isolated runtime failed to start' }
    foreach ($model in @('qwen3.5:0.8b','lfm3:1.7b','lfm3-vl:2b')) {
        if ($model -notin @($tags.models.name)) { throw "Bundled model missing: $model" }
    }
    Write-Output 'PASS: all three bundled models discovered'
    $failures = @()
    if (-not $VisionOnly) {
        foreach ($model in @('qwen3.5:0.8b','lfm3:1.7b')) {
            try {
                $body = @{model=$model;prompt='Reply with exactly the word READY.';stream=$false;think=$false;keep_alive=0;options=@{temperature=0;num_predict=32;num_ctx=2048}} | ConvertTo-Json -Depth 5
                $reply = Invoke-RestMethod "$base/api/generate" -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 120
                if (-not $reply.done -or $reply.response.Trim() -notmatch '^READY[.!]?$') { throw "Unexpected response: $($reply.response)" }
                Write-Output "PASS: $model returned $($reply.response.Trim())"
            } catch { $failures += "$model : $_"; Write-Output "FAIL: $model : $_" }
        }
    }
    Add-Type -AssemblyName System.Drawing
    $bitmap = [Drawing.Bitmap]::new(224,224)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $memory = [IO.MemoryStream]::new()
    try {
        $graphics.Clear([Drawing.Color]::White)
        $graphics.FillRectangle([Drawing.Brushes]::Blue,32,32,160,160)
        $bitmap.Save($memory,[Drawing.Imaging.ImageFormat]::Png)
        $encoded = [Convert]::ToBase64String($memory.ToArray())
    } finally { $graphics.Dispose(); $bitmap.Dispose(); $memory.Dispose() }
    try {
        $body = @{model='lfm3-vl:2b';messages=@(@{role='user';content='What color is the large square in the center of the image? Answer with one color word.';images=@($encoded)});stream=$false;think=$false;keep_alive=0;options=@{temperature=0;num_predict=128;num_ctx=4096}} | ConvertTo-Json -Depth 8
        $reply = Invoke-RestMethod "$base/api/chat" -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 120
        if (-not $reply.done -or $reply.message.content.Trim() -notmatch '^blue[.!]?$') { throw "Unexpected vision response: $($reply.message.content); reason: $($reply.done_reason)" }
        Write-Output "PASS: lfm3-vl:2b identified the actual image color as $($reply.message.content.Trim())"
    } catch { $failures += "Vision: $_"; Write-Output "FAIL: Vision: $_" }
    if ($failures.Count -gt 0) { throw ($failures -join '; ') }
} finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}
