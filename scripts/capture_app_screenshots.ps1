# Captures screenshots of the built lnwdeck application.
#
# The application is launched from the release build and each of its top-level
# windows is photographed with the Win32 API. Nothing is stubbed: the images
# show whatever real state exists on the machine the capture runs on.
#
# Usage:
#   pwsh ./scripts/capture_app_screenshots.ps1 [-ShowWidget]

param(
    [string]$Exe = "target/release/lnwdeck-desktop.exe",
    [string]$OutputDir = "assets/screenshots",
    [int]$StartupSeconds = 14,
    # Makes the floating widget visible before launching so it can be captured.
    [switch]$ShowWidget
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
    New-Item -ItemType Directory -Path $OutPath -Force | Out-Null
}

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public class Win32Capture {
    // The application is per-monitor DPI aware. Without matching awareness in
    // this process, GetWindowRect and CopyFromScreen disagree and the capture
    // lands offset from the window.
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
    public static readonly IntPtr DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = new IntPtr(-4);

    public static void MatchApplicationDpiAwareness() {
        try { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2); }
        catch (EntryPointNotFoundException) { /* older Windows: nothing to match */ }
    }

    public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc proc, IntPtr lParam);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hWnd, StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    // Every visible top-level window that belongs to the given process.
    public static List<KeyValuePair<IntPtr, string>> WindowsOf(uint pid) {
        var found = new List<KeyValuePair<IntPtr, string>>();
        EnumWindows((hWnd, lParam) => {
            uint owner;
            GetWindowThreadProcessId(hWnd, out owner);
            if (owner != pid || !IsWindowVisible(hWnd)) { return true; }
            var text = new StringBuilder(512);
            GetWindowTextW(hWnd, text, text.Capacity);
            var title = text.ToString();
            if (title.Length > 0) {
                found.Add(new KeyValuePair<IntPtr, string>(hWnd, title));
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@

# Must run before any window is measured or captured.
[Win32Capture]::MatchApplicationDpiAwareness()

function Enable-Widget {
    # The widget starts hidden by design. The stored setting is flipped through
    # the same app_settings key the application itself writes.
    $db = Join-Path $env:LOCALAPPDATA "lnwdeck\lnwdeck.db"
    if (-not (Test-Path $db)) {
        Write-Output "no application database yet; the widget stays hidden"
        return
    }
    $python = Get-Command python -ErrorAction SilentlyContinue
    if (-not $python) {
        Write-Output "python not available; the widget stays hidden"
        return
    }
    $lines = @(
        "import sqlite3, sys",
        "conn = sqlite3.connect(sys.argv[1])",
        "conn.execute(""INSERT OR REPLACE INTO app_settings (key, value) VALUES ('widget_visible','true')"")",
        "conn.execute(""INSERT OR REPLACE INTO app_settings (key, value) VALUES ('pet_visible','true')"")",
        "for key in ('widget_width', 'widget_height', 'widget_x', 'widget_y'):",
        "    conn.execute('DELETE FROM app_settings WHERE key = ?', (key,))",
        "conn.commit()",
        "print('widget and pet visibility enabled')"
    )
    $temp = Join-Path $env:TEMP "lnwdeck_show_widget.py"
    Set-Content -Path $temp -Value $lines -Encoding utf8
    & $python.Source $temp $db
    Remove-Item $temp -Force
}

function Save-WindowShot([string]$Title, [string]$FileName, [uint32]$OwnerPid) {
    $match = [Win32Capture]::WindowsOf($OwnerPid) |
        Where-Object { $_.Value -eq $Title } |
        Select-Object -First 1
    if (-not $match) {
        $titles = ([Win32Capture]::WindowsOf($OwnerPid) | ForEach-Object { $_.Value }) -join ", "
        Write-Output "window not found: $Title (titles seen: $titles)"
        return
    }

    $handle = $match.Key
    [Win32Capture]::ShowWindow($handle, 9) | Out-Null   # SW_RESTORE
    [Win32Capture]::SetForegroundWindow($handle) | Out-Null
    Start-Sleep -Milliseconds 1000

    $rect = New-Object Win32Capture+RECT
    if (-not [Win32Capture]::GetWindowRect($handle, [ref]$rect)) {
        Write-Output "could not read window bounds: $Title"
        return
    }
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) {
        Write-Output "window has no size: $Title"
        return
    }

    $bitmap = New-Object System.Drawing.Bitmap $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
    $target = Join-Path $OutPath $FileName
    $bitmap.Save($target, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bitmap.Dispose()
    Write-Output "saved $target ($width x $height)"
}

function Send-Nav([string]$Keys) {
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Start-Sleep -Milliseconds 1300
}

if ($ShowWidget) {
    Enable-Widget
}

Write-Output "launching $ExePath"
$process = Start-Process -FilePath $ExePath -PassThru
Start-Sleep -Seconds $StartupSeconds
$ownerPid = [uint32]$process.Id

try {
    Save-WindowShot -Title "lnwdeck" -FileName "overview_dashboard.png" -OwnerPid $ownerPid

    # Pages are reached by keyboard so the capture needs no pointer coordinates.
    Send-Nav "{TAB}"
    Send-Nav "{TAB}{ENTER}"
    Save-WindowShot -Title "lnwdeck" -FileName "providers_page.png" -OwnerPid $ownerPid

    # Sidebar order: Overview, Providers, Analytics, Costs, Budgets, Models,
    # Alerts, Pet, Settings, System — the count below lands on the target page.
    for ($i = 0; $i -lt 3; $i++) { Send-Nav "{TAB}" }
    Send-Nav "{ENTER}"
    Save-WindowShot -Title "lnwdeck" -FileName "costs_page.png" -OwnerPid $ownerPid

    for ($i = 0; $i -lt 6; $i++) { Send-Nav "{TAB}" }
    Send-Nav "{ENTER}"
    Save-WindowShot -Title "lnwdeck" -FileName "system_diagnostics.png" -OwnerPid $ownerPid

    # The dashboard is minimised first: the widget is a small always-on-top
    # window and a screen capture of its rectangle would otherwise include
    # whatever sits behind it.
    $main = [Win32Capture]::WindowsOf($ownerPid) |
        Where-Object { $_.Value -eq "lnwdeck" } |
        Select-Object -First 1
    if ($main) {
        [Win32Capture]::ShowWindow($main.Key, 6) | Out-Null   # SW_MINIMIZE
        Start-Sleep -Milliseconds 900
    }
    Save-WindowShot -Title "lnwdeck quota" -FileName "floating_widget.png" -OwnerPid $ownerPid

    # The desktop pet floats on a transparent window; capture it last. The
    # sprite may be anywhere on screen, so wait for it to settle near the
    # bottom where it normally rests.
    Start-Sleep -Seconds 4
    Save-WindowShot -Title "lnwdeck pet" -FileName "desktop_pet.png" -OwnerPid $ownerPid
}
finally {
    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
        Write-Output "stopped the application"
    }
}
