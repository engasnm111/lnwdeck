param(
    [string]$BuildDir = "target\release",
    [string]$OutputFile = "",
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$ScriptDir\.."

# The archive name must never be hardcoded: derive the version from the
# canonical installer/package-config.json so a version bump cannot ship a
# mislabelled ZIP.
if (-not $Version) {
    $PackageConfig = Get-Content "$RepoRoot\installer\package-config.json" -Raw | ConvertFrom-Json
    $Version = $PackageConfig.version
}
if (-not $OutputFile) {
    $OutputFile = "lnwdeck_${Version}_portable.zip"
}

Write-Host "=== lnwdeck Portable Packaging v$Version ==="

$TargetDir = "$RepoRoot\$BuildDir"
if (-not (Test-Path $TargetDir)) {
    New-Item -ItemType Directory -Path $TargetDir -Force | Out-Null
}

$MarkerFile = "$TargetDir\.lnwdeck_portable"
Set-Content -Path $MarkerFile -Value "portable" -NoNewline
Write-Host "[OK] Created portable marker file"

# Collect files to zip
$ZipItems = @()
$ExePath = "$TargetDir\lnwdeck-desktop.exe"
if (-not (Test-Path $ExePath)) {
    $ExePath = "$TargetDir\lnwdeck.exe"
}

if (-not (Test-Path $ExePath)) {
    Write-Error "Build artifact not found: $ExePath"
    exit 1
}

$ZipItems += $ExePath
$ZipItems += $MarkerFile

# Add any DLLs next to the exe
$Dlls = Get-ChildItem -Path (Split-Path $ExePath) -Filter "*.dll" -ErrorAction SilentlyContinue
foreach ($dll in $Dlls) {
    $ZipItems += $dll.FullName
}

$OutputPath = "$RepoRoot\$OutputFile"
if (Test-Path $OutputPath) {
    Remove-Item $OutputPath -Force
}

# Compress to ZIP
$ZipItems | Compress-Archive -DestinationPath $OutputPath -CompressionLevel Optimal
Write-Host "[OK] Portable package created: $OutputPath"

# Cleanup marker
Remove-Item $MarkerFile -Force

Write-Host "=== Done ==="
