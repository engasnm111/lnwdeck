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
$policyPath = "HKCU:\Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments"
$policyName = [System.IO.Path]::GetFileName($appExe)
$policyHadValue = $false
$previousPolicyValue = $null
$policyConfigured = $false
$app = $null

try {
    # Some hosted WebView2 environments ignore the process variable. Apply the
    # documented per-app policy as a temporary fallback and restore it below.
    try {
        $existingPolicy = Get-ItemProperty -Path $policyPath -Name $policyName -ErrorAction SilentlyContinue
        if ($existingPolicy) {
            $policyProperty = $existingPolicy.PSObject.Properties[$policyName]
            if ($policyProperty) {
                $policyHadValue = $true
                $previousPolicyValue = [string]$policyProperty.Value
            }
        }
        New-Item -Path $policyPath -Force | Out-Null
        New-ItemProperty -Path $policyPath -Name $policyName -Value $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -PropertyType String -Force | Out-Null
        $policyConfigured = $true
    } catch {
        Write-Output "WebView2 app policy unavailable; continuing with process environment: $($_.Exception.Message)"
    }

    $app = Start-Process -FilePath $appExe -WorkingDirectory $repo -PassThru
    Write-Output "Started $profile app PID $($app.Id); waiting for WebView2 CDP on port $port"

    # Wait for the CDP endpoint to come up.
    $ready = $false
    $startupTimeoutSeconds = 60
    for ($i = 0; $i -lt $startupTimeoutSeconds; $i++) {
        Start-Sleep -Seconds 1
        if ($app.HasExited) {
            $exitCode = $app.ExitCode
            throw "$profile app exited before WebView2 debugging endpoint started (exit code $exitCode; CDP port $port)"
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
    if ($policyConfigured) {
        if ($policyHadValue) {
            New-ItemProperty -Path $policyPath -Name $policyName -Value $previousPolicyValue -PropertyType String -Force | Out-Null
        } else {
            Remove-ItemProperty -Path $policyPath -Name $policyName -ErrorAction SilentlyContinue
        }
    }
}
