# Retries only a command that appears to have been killed silently mid-build
# on hosted Windows runners (build progress only, with no diagnostic or test
# output). Diagnostic-bearing failures, including cargo's exit 101, fail
# immediately instead of repeating the same error. A retry cleans the debug
# artifact tree; the final failure prints the runner's free RAM, free disk and
# the command's last 120 output lines as ::error:: annotations so the next fix
# starts from evidence.
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
$attemptsUsed = 0
for ($i = 1; $i -le $Attempts; $i++) {
    $attemptsUsed = $i
    Write-Output "=== $Label - attempt $i/$Attempts ==="
    # cmd /c: the commands come from the workflow (cargo/clippy/pnpm...), and
    # cmd tolerates both plain commands and && chaining regardless of whether
    # the runner's default PowerShell is 5.1 or 7.
    $output = & cmd /c $Command 2>&1
    $code = $LASTEXITCODE
    if ($code -eq 0) {
        Write-Output "=== $Label passed on attempt $i ==="
        exit 0
    }
    $joined = ($output | Out-String)
    $hasDiagnostic = $joined -match '(?im)(^|\s)(error(?:\[[\w-]+\])?\s*:|fatal error|panic:|failed\b|failure\b|FAIL(?:ED)?\b|ERR_PNPM|TS\d{3,5}\b|cannot\b|could not\b)'
    $hasTestOutput = $joined -match '(?im)^\s*(?:ok|not ok)\s+\d+\s+-|test result:|Tests?:\s'
    $retryableFailure = ($code -eq 1) -or ($code -eq 101)
    $canRetry = $retryableFailure -and -not $hasDiagnostic -and -not $hasTestOutput
    if ($i -lt $Attempts -and $canRetry) {
        Write-Output "::warning::$Label failed with only build progress (exit $code) on attempt $i; cleaning debug artifacts and retrying"
        & cmd /c "rd /s /q target\debug 2>nul & rd /s /q target\tests 2>nul"
        Start-Sleep -Seconds 10
    } elseif ($i -lt $Attempts) {
        Write-Output "::notice::$Label failed with a diagnostic or test output (exit $code); not retrying"
        break
    }
}

$os = Get-CimInstance Win32_OperatingSystem
Write-Output "::error::${Label}: free RAM $([math]::Round($os.FreePhysicalMemory / 1MB, 1)) GB / total $([math]::Round($os.TotalVisibleMemorySize / 1MB, 1)) GB"
try {
    $drive = Get-PSDrive C
    Write-Output "::error::${Label}: disk free $([math]::Round($drive.Free / 1GB, 1)) GB"
} catch { }
Write-Output "::error::${Label} failed after $attemptsUsed attempt(s); last output:"
$tail = (($joined -split "`n") | Select-Object -Last 120) -join "`n"
Write-Output "::error::$tail"
exit 1
