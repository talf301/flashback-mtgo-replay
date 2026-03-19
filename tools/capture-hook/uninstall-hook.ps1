# uninstall-hook.ps1 — Restore original MTGO.exe.config and remove hook DLLs
# Run this after capturing to return MTGO to normal.

$ErrorActionPreference = "Stop"

# Find the MTGO directory — check running process first, then ClickOnce cache
$mtgoProc = Get-Process mtgo* -ErrorAction SilentlyContinue | Select-Object -First 1
if ($mtgoProc -and $mtgoProc.Path) {
    $mtgoDir = Split-Path -Parent $mtgoProc.Path
} else {
    $appsDir = Join-Path $env:LOCALAPPDATA "Apps\2.0"
    $mtgoExe = Get-ChildItem -Path $appsDir -Recurse -Filter "MTGO.exe" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($mtgoExe) {
        $mtgoDir = $mtgoExe.DirectoryName
    }
}

if (-not $mtgoDir -or -not (Test-Path (Join-Path $mtgoDir "MTGO.exe.config.bak"))) {
    Write-Host "No backup found — hook may not be installed."
    exit 0
}
Write-Host "Found hook installation in: $mtgoDir"
$configPath = Join-Path $mtgoDir "MTGO.exe.config"
$backupPath = Join-Path $mtgoDir "MTGO.exe.config.bak"

# Restore original config
if (Test-Path $backupPath) {
    Copy-Item $backupPath $configPath -Force
    Remove-Item $backupPath
    Write-Host "Restored original MTGO.exe.config"
} else {
    Write-Host "No backup found at $backupPath — config may already be clean."
}

# Remove hook DLLs
$removed = @()
foreach ($dll in @("CaptureHook.dll", "0Harmony.dll")) {
    $path = Join-Path $mtgoDir $dll
    if (Test-Path $path) {
        Remove-Item $path
        $removed += $dll
    }
}
if ($removed.Count -gt 0) {
    Write-Host "Removed: $($removed -join ', ')"
} else {
    Write-Host "No hook DLLs found to remove."
}

Write-Host ""
Write-Host "Done! MTGO is back to normal."
