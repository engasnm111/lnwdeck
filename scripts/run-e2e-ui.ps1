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
    throw "release build not found at $appExe - run pnpm tauri build --no-bundle first"
}

# Hosted runners can have a service already using a fixed debugging port. Reserve
# an ephemeral loopback port before starting WebView2 and pass the same value to
# Playwright through the child process environment.
$tcpListener = New-Object System.Net.Sockets.TcpListener([System.Net.IPAddress]::Loopback, 0)
try {
    $tcpListener.Start()
    $port = $tcpListener.LocalEndpoint.Port
}
finally {
    if ($tcpListener) {
        $tcpListener.Stop()
    }
}

$previousBrowserArguments = [Environment]::GetEnvironmentVariable(
    "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
    "Process"
)
$previousCdpPort = [Environment]::GetEnvironmentVariable(
    "LNWD_E2E_CDP_PORT",
    "Process"
)
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$port"
$env:LNWD_E2E_CDP_PORT = "$port"
$app = $null

try {
    $app = Start-Process -FilePath $appExe -WorkingDirectory $repo -PassThru
    Write-Output "Started release app PID $($app.Id); waiting for WebView2 CDP on port $port"

    # Wait for the CDP endpoint to come up.
    $ready = $false
    $startupTimeoutSeconds = 60
    for ($i = 0; $i -lt $startupTimeoutSeconds; $i++) {
        Start-Sleep -Seconds 1
        if ($app.HasExited) {
            $exitCode = $app.ExitCode
            throw "release app exited before WebView2 debugging endpoint started (exit code $exitCode; CDP port $port)"
        }
        try {
            Invoke-RestMethod -Uri "http://127.0.0.1:$port/json/version" -TimeoutSec 2 | Out-Null
            $ready = $true
            break
        } catch { }
    }
    if (-not $ready) {
        $listenerState = (Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue |
            Select-Object -First 1 -ExpandProperty State)
        throw "WebView2 debugging endpoint did not start on port $port (app PID $($app.Id); app exited $($app.HasExited); listener state $listenerState)"
    }
    Write-Output "CDP endpoint ready on port $port; running Playwright tests"
    pnpm --filter @lnwdeck/desktop exec playwright test
    exit $LASTEXITCODE
}
finally {
    if ($app -and -not $app.HasExited) {
        Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
        $null = $app.WaitForExit(5000)
    }
    if ($null -eq $previousBrowserArguments) {
        Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    } else {
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $previousBrowserArguments
    }
    if ($null -eq $previousCdpPort) {
        Remove-Item Env:LNWD_E2E_CDP_PORT -ErrorAction SilentlyContinue
    } else {
        $env:LNWD_E2E_CDP_PORT = $previousCdpPort
    }
}
