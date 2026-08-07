# Runs the Playwright UI smoke tests against a real release build.
# The app is started with WebView2 remote debugging enabled; Playwright
# attaches to the CDP endpoint and drives the real frontend + backend.
$ErrorActionPreference = "Stop"

# The workspace root is three levels above the desktop app (apps/desktop),
# which is where this script is normally invoked from.
$desktop = Get-Location
if ($desktop.Path -notlike "*apps*desktop") {
    $desktop = Join-Path $PSScriptRoot "..\apps\desktop"
}
$repo = (Resolve-Path (Join-Path $desktop "..\..")).Path
$appExe = Join-Path $repo "target\release\lnwdeck-desktop.exe"
if (-not (Test-Path $appExe)) {
    Write-Error "release build not found at $appExe - run pnpm tauri build --no-bundle first"
}

$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
$app = Start-Process -FilePath $appExe -PassThru

try {
    # Wait for the CDP endpoint to come up.
    $ready = $false
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep -Seconds 1
        try {
            Invoke-RestMethod -Uri "http://127.0.0.1:9222/json/version" -TimeoutSec 2 | Out-Null
            $ready = $true
            break
        } catch { }
    }
    if (-not $ready) {
        Write-Error "WebView2 debugging endpoint did not start"
    }
    Write-Output "CDP endpoint ready; running Playwright tests"
    pnpm --filter @lnwdeck/desktop exec playwright test
    exit $LASTEXITCODE
}
finally {
    if ($app -and -not $app.HasExited) {
        Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
    }
}
