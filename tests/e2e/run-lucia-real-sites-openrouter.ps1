# Native Browsphere real-website tests with the explicitly chosen free OpenRouter model.
# No mock model is used. No personal browsing data or existing browser profile is used.
# An OpenRouter API key must first be saved in Browsphere's Cloud Models settings.
param(
    [string]$Site = "",
    [int]$CdpPort = 9585,
    [ValidateRange(1,5)][int]$Repeats = 1,
    [string]$ExecutablePath = "",
    [string]$Model = 'inclusionai/ling-3.2-flash:free',
    [switch]$CheckCatalogOnly
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$model = $Model.Trim()
if ($model -ne 'openrouter/free' -and -not $model.EndsWith(':free', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'ONLY_FREE_MODELS: Model must be an OpenRouter :free variant or openrouter/free.'
}
$endpoint = 'https://openrouter.ai/api/v1/chat/completions'

# Verify the model is currently free. Report, but do not assume, structured-output support.
$catalog = Invoke-RestMethod -Uri 'https://openrouter.ai/api/v1/models' -TimeoutSec 20
$entry = @($catalog.data | Where-Object { $_.id -eq $model })
if ($entry.Count -ne 1) { throw "OpenRouter free test model not listed: $model" }
if ([double]$entry[0].pricing.prompt -ne 0 -or [double]$entry[0].pricing.completion -ne 0) {
    throw "OpenRouter catalog no longer lists the selected model as free; refusing to spend credits"
}
$structured = @($entry[0].supported_parameters) -contains 'structured_outputs'
# This model is explicitly requested for testing. The absence of advertised
# schema support is a compatibility risk, not proof the API cannot answer.
# Do not report a schema-compatible model or autonomous success prematurely.
Write-Host "OPENROUTER_CATALOG_OK model=$model free=true structured_outputs=$($structured.ToString().ToLowerInvariant())"
if (-not $structured) {
    Write-Warning 'Model does not advertise structured outputs. Lucia may reject malformed action JSON; live verification is required.'
}
if ($CheckCatalogOnly) { return }

# Browsphere stores its endpoint-scoped provider credentials in Windows Credential Manager.
# Only check the target name; never read, print, or log the secret.
$target = 'model_api_key:openrouter.ai:443.BrowsphereAI'
$credentialExists = [bool]@(cmdkey /list | Select-String -SimpleMatch $target).Count
if (-not $credentialExists) {
    throw "MISSING_OPENROUTER_KEY: Save your key in Browsphere > Settings > AI & ML > Cloud Models (OpenRouter preset). Do not paste it into scripts or chat."
}
if (-not $ExecutablePath) { $ExecutablePath = Join-Path $root 'target\release\browsphere-core.exe' }
if (-not (Test-Path $ExecutablePath)) { throw "Native Browsphere release build missing" }
$profile = Join-Path $env:TEMP ('browsphere-openrouter-real-sites-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force (Join-Path $profile 'Roaming'),(Join-Path $profile 'Local'),(Join-Path $profile 'WebView2') | Out-Null

$names = @('APPDATA','LOCALAPPDATA','WEBVIEW2_USER_DATA_FOLDER','WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS','LUCIA_CDP_PORT','LUCIA_TEST_CLOUD_ENDPOINT','LUCIA_TEST_CLOUD_MODEL','LUCIA_REAL_SITE','LUCIA_REAL_REPEATS')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name,'Process') }
$env:APPDATA = Join-Path $profile 'Roaming'
$env:LOCALAPPDATA = Join-Path $profile 'Local'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $profile 'WebView2'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$CdpPort --remote-allow-origins=*"
$env:LUCIA_CDP_PORT = "$CdpPort"
$env:LUCIA_TEST_CLOUD_ENDPOINT = $endpoint
$env:LUCIA_TEST_CLOUD_MODEL = $model
$env:LUCIA_REAL_SITE = $Site
$env:LUCIA_REAL_REPEATS = "$Repeats"

$app = $null
try {
    $app = Start-Process -FilePath $ExecutablePath -WorkingDirectory $root -PassThru
    Write-Host "OPENROUTER_NATIVE_STARTED PID=$($app.Id) CDP=$CdpPort site=$Site model=$model"
    $ready = $false
    for ($i=0; $i -lt 75; $i++) {
        try {
            $pages = Invoke-RestMethod -Uri "http://127.0.0.1:$CdpPort/json" -TimeoutSec 2
            if (@($pages | Where-Object {$_.url -like '*tauri.localhost*'}).Count -gt 0) {
                $ready = $true; break
            }
        } catch {}
        Start-Sleep -Milliseconds 400
    }
    if (-not $ready) { throw 'Native WebView2 CDP port did not open' }
    Set-Location $root
    & node (Join-Path $root 'tests\e2e\lucia-real-sites-openrouter.cjs')
    $exitCode = $LASTEXITCODE
    Write-Host "OPENROUTER_REAL_SITE_TEST_EXIT=$exitCode"
    if ($exitCode -ne 0) { exit 1 }
    Write-Host 'OPENROUTER_REAL_SITE_TESTS_PASSED'
} finally {
    if ($app) { Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue }
    foreach ($name in $names) {
        [Environment]::SetEnvironmentVariable($name,$previous[$name],'Process')
    }
    Write-Host 'Closed isolated OpenRouter native test app'
}
