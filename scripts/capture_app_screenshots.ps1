# Captures screenshots of the built lnwdeck application.
#
# The application is launched from the release build, driven only by clicking
# through its own navigation, and each window is captured with the Win32 API.
# Nothing is stubbed: the images show whatever real state exists on the machine
# the capture runs on.
#
# Usage:
#   pwsh ./scripts/capture_app_screenshots.ps1 [-OutputDir assets/screenshots]

param(
    [string]$Exe = "target/release/lnwdeck-desktop.exe",
    [string]$OutputDir = "assets/screenshots",
    [int]$StartupSeconds = 12
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$ScriptDir\.."
$ExePath = Join-Path $RepoRoot $Exe
$OutPath = Join-Path $RepoRoot $OutputDir

if (-not (Test-Path $ExePath)) {
    Write-Error "Build artifact not found: $ExePath. Run pnpm tauri:build first."
    exit 1
}
if (-not (Test-Path $OutPath)) {
    New-Item -ItemType Directory -Path $OutPath -Force
}

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32Capture {
    [DllImport("user32.dll")] public static extern IntPtr FindWindow(string cls, string name);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@

function Save-WindowShot([string]$Title, [string]$FileName) {
    # The window is located through the running process, because the Tauri
    # window class name is not stable across builds.
    $handle = [IntPtr]::Zero
    foreach ($p in Get-Process -Name "lnwdeck-desktop" -ErrorAction SilentlyContinue) {
        if ($p.MainWindowTitle -eq $Title) { $handle = $p.MainWindowHandle; break }
    }
    if ($handle -eq [IntPtr]::Zero) {
        Write-Output ("window not found: {0}; titles seen: {1}" -f $Title, ((Get-Process -Name "lnwdeck-desktop" -ErrorAction SilentlyContinue | ForEach-Object { $_.MainWindowTitle }) -join ", "))
        return $false
    }
    [Win32Capture]::ShowWindow($handle, 9) | Out-Null   # SW_RESTORE
    [Win32Capture]::SetForegroundWindow($handle) | Out-Null
    Start-Sleep -Milliseconds 900

    $rect = New-Object Win32Capture+RECT
    if (-not [Win32Capture]::GetWindowRect($handle, [ref]$rect)) {
        Write-Output "could not read window bounds: $Title"
        return $false
    }
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) {
        Write-Output "window has no size: $Title"
        return $false
    }

    $bitmap = New-Object System.Drawing.Bitmap $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
    $target = Join-Path $OutPath $FileName
    $bitmap.Save($target, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bitmap.Dispose()
    Write-Output "saved $target ($width x $height)"
    return $true
}

function Send-Nav([string]$Keys) {
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Start-Sleep -Milliseconds 1200
}

Write-Output "launching $ExePath"
$process = Start-Process -FilePath $ExePath -PassThru
Start-Sleep -Seconds $StartupSeconds

try {
    # The pages are reached by keyboard: focus the navigation and move down.
    Save-WindowShot -Title "lnwdeck" -FileName "overview_dashboard.png"

    Send-Nav "{TAB}"
    Send-Nav "{TAB}{ENTER}"
    Save-WindowShot -Title "lnwdeck" -FileName "providers_page.png"

    for ($i = 0; $i -lt 7; $i++) { Send-Nav "{TAB}" }
    Send-Nav "{ENTER}"
    Save-WindowShot -Title "lnwdeck" -FileName "system_diagnostics.png"

    Save-WindowShot -Title "lnwdeck quota" -FileName "floating_widget.png"
}
finally {
    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
        Write-Output "stopped the application"
    }
}


