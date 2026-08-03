param(
    [ValidateSet("register", "unregister")]
    [string]$Action = "register",
    [string]$HostName = "app.lnwdeck.browser_helper",
    [string]$HostExePath = "$env:LOCALAPPDATA\Programs\lnwdeck\lnwdeck-browser-host.exe",
    [string]$ChromeExtensionId = "",
    [string]$EdgeExtensionId = ""
)

$ErrorActionPreference = "Stop"

function Write-Registry {
    param([string]$Browser, [string]$RegPath, [string]$ExtensionId)

    if ([string]::IsNullOrEmpty($ExtensionId)) {
        Write-Host "[SKIP] $Browser extension ID not provided"
        return
    }

    $HkcuPath = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$HostName"
    if ($Browser -eq "Edge") {
        $HkcuPath = "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\$HostName"
    }

    if ($Action -eq "register") {
        $ManifestDir = Join-Path (Split-Path $HostExePath) "native-messaging"
        if (-not (Test-Path $ManifestDir)) {
            New-Item -ItemType Directory -Path $ManifestDir -Force | Out-Null
        }

        $manifest = @{
            name = $HostName
            description = "lnwdeck Browser Helper Native Messaging Host"
            path = $HostExePath
            type = "stdio"
            allowed_origins = @(
                "chrome-extension://$ChromeExtensionId/",
                "chrome-extension://$EdgeExtensionId/"
            )
        }

        $ManifestPath = Join-Path $ManifestDir "$HostName.json"
        $manifest | ConvertTo-Json -Depth 3 | Set-Content -Path $ManifestPath

        New-Item -Path $HkcuPath -Force | Out-Null
        Set-ItemProperty -Path $HkcuPath -Name "(Default)" -Value $ManifestPath
        Write-Host "[OK] Registered $Browser native host"
    }
    elseif ($Action -eq "unregister") {
        if (Test-Path $HkcuPath) {
            Remove-Item -Path $HkcuPath -Recurse -Force
            Write-Host "[OK] Unregistered $Browser native host"
        }
        else {
            Write-Host "[SKIP] $Browser native host not registered"
        }
    }
}

Write-Host "=== lnwdeck Native Messaging Host $Action ==="

Write-Registry -Browser "Chrome" -ExtensionId $ChromeExtensionId
Write-Registry -Browser "Edge" -ExtensionId $EdgeExtensionId

Write-Host "=== Done ==="
