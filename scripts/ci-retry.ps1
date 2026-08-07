# Retries a cargo command that dies silently mid-build on hosted Windows
# runners (exit 1, no error line - Defender contention or a transient kill).
# Each retry cleans the debug artifact tree; the final failure prints the
# runner's free RAM, free disk and the command's last 120 output lines as
# ::error:: annotations so the next fix starts from evidence, not a guess.
#
# Usage:
#   pwsh scripts/ci-retry.ps1 -Command "cargo test -p lnwdeck-desktop" [-Attempts 3] [-Label "desktop tests"]

param(
    [Parameter(Mandatory = $true)][string]$Command,
    [int]$Attempts = 3,
    [string]$Label = "cargo step"
)

$ErrorActionPreference = "Continue"

$joined = ""
for ($i = 1; $i -le $Attempts; $i++) {
    Write-Output "=== $Label - attempt $i/$Attempts ==="
    $output = & powershell -NoProfile -Command $Command 2>&1
    $code = $LASTEXITCODE
    if ($code -eq 0) {
        Write-Output "=== $Label passed on attempt $i ==="
        exit 0
    }
    $joined = ($output | Out-String)
    if ($i -lt $Attempts) {
        Write-Output "::warning::$Label failed with exit $code on attempt $i; cleaning debug artifacts and retrying"
        & cmd /c "rd /s /q target\debug 2>nul & rd /s /q target\tests 2>nul"
        Start-Sleep -Seconds 10
    }
}

$os = Get-CimInstance Win32_OperatingSystem
Write-Output "::error::${Label}: free RAM $([math]::Round($os.FreePhysicalMemory / 1MB, 1)) GB / total $([math]::Round($os.TotalVisibleMemorySize / 1MB, 1)) GB"
try {
    $drive = Get-PSDrive C
    Write-Output "::error::${Label}: disk free $([math]::Round($drive.Free / 1GB, 1)) GB"
} catch { }
Write-Output "::error::${Label} failed after $Attempts attempts; last output:"
$tail = (($joined -split "`n") | Select-Object -Last 120) -join "`n"
Write-Output "::error::$tail"
exit 1
