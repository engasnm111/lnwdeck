# Runs the Playwright UI smoke tests against a real Tauri build.
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
$profile = $env:LNWD_E2E_BUILD_PROFILE
if ([string]::IsNullOrWhiteSpace($profile)) {
    $profile = "debug"
}
if ($profile -ne "debug" -and $profile -ne "release") {
    throw "LNWD_E2E_BUILD_PROFILE must be debug or release, got '$profile'"
}
$appExe = Join-Path $repo ("target\{0}\lnwdeck-desktop.exe" -f $profile)
if (-not (Test-Path $appExe)) {
    throw "$profile build not found at $appExe - build the desktop app before running UI smoke"
}

# Hosted runners can have a service already using a fixed debugging port. Reserve
# an ephemeral loopback port before starting WebView2 and pass the same value to
# the native app through the child process environment.
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

$previousCdpPort = [Environment]::GetEnvironmentVariable(
    "LNWD_E2E_CDP_PORT",
    "Process"
)
$env:LNWD_E2E_CDP_PORT = "$port"
$app = $null

try {
    $app = Start-Process -FilePath $appExe -WorkingDirectory $repo -PassThru
    Write-Output "Started $profile app PID $($app.Id); waiting for WebView2 CDP on port $port"

    # Wait for the CDP endpoint to come up.
    $ready = $false
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($app.HasExited) {
            $exitCode = $app.ExitCode
            throw "$profile app exited before WebView2 debugging endpoint started (exit code $exitCode; CDP port $port)"
        }
        try {
            Invoke-RestMethod -Uri "http://127.0.0.1:$port/json/version" -TimeoutSec 1 | Out-Null
            $ready = $true
            break
        } catch {
            Start-Sleep -Milliseconds 250
        }
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
    if ($null -eq $previousCdpPort) {
        Remove-Item Env:LNWD_E2E_CDP_PORT -ErrorAction SilentlyContinue
    } else {
        $env:LNWD_E2E_CDP_PORT = $previousCdpPort
    }
}
