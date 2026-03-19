# install-hook.ps1 — Patch MTGO.exe.config and copy DLLs for capture hook
# Run this BEFORE launching MTGO. Close MTGO first if running.

$ErrorActionPreference = "Stop"

# Auto-detect MTGO directory from running process, or find the most recent one
$mtgoProc = Get-Process mtgo* -ErrorAction SilentlyContinue | Select-Object -First 1
if ($mtgoProc -and $mtgoProc.Path) {
    $mtgoDir = Split-Path -Parent $mtgoProc.Path
    Write-Host "Detected MTGO directory from running process: $mtgoDir"
    Write-Host "NOTE: Close MTGO before continuing, then re-run this script."
} else {
    # Find most recently modified MTGO.exe in ClickOnce cache
    $appsDir = Join-Path $env:LOCALAPPDATA "Apps\2.0"
    $mtgoExe = Get-ChildItem -Path $appsDir -Recurse -Filter "MTGO.exe" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $mtgoExe) {
        Write-Error "Could not find MTGO.exe in ClickOnce cache"
        exit 1
    }
    $mtgoDir = $mtgoExe.DirectoryName
    Write-Host "Found MTGO directory: $mtgoDir"
}
$hookDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$buildDir = Join-Path $hookDir "bin\Release\net472"
$configPath = Join-Path $mtgoDir "MTGO.exe.config"
$backupPath = Join-Path $mtgoDir "MTGO.exe.config.bak"

# Verify build exists
if (-not (Test-Path (Join-Path $buildDir "CaptureHook.dll"))) {
    Write-Error "Build not found. Run: dotnet build -c Release"
    exit 1
}

# Backup original config
if (-not (Test-Path $backupPath)) {
    Copy-Item $configPath $backupPath
    Write-Host "Backed up original config to $backupPath"
} else {
    Write-Host "Backup already exists at $backupPath"
}

# Copy DLLs to MTGO directory (so assembly resolver can find them)
Copy-Item (Join-Path $buildDir "CaptureHook.dll") $mtgoDir -Force
Copy-Item (Join-Path $buildDir "0Harmony.dll") $mtgoDir -Force
Write-Host "Copied CaptureHook.dll and 0Harmony.dll to MTGO directory"

# Patch MTGO.exe.config to add AppDomainManager
[xml]$config = Get-Content $configPath
$runtime = $config.configuration.runtime
if (-not $runtime) {
    $runtime = $config.CreateElement("runtime")
    $config.configuration.AppendChild($runtime) | Out-Null
}

# Check if already patched
$existing = $runtime.SelectSingleNode("appDomainManagerAssembly")
if ($existing) {
    Write-Host "Config already patched. Skipping."
} else {
    $asmElem = $config.CreateElement("appDomainManagerAssembly")
    $asmElem.SetAttribute("value", "CaptureHook, Version=1.0.0.0, Culture=neutral, PublicKeyToken=null")
    $runtime.AppendChild($asmElem) | Out-Null

    $typeElem = $config.CreateElement("appDomainManagerType")
    $typeElem.SetAttribute("value", "CaptureHook.CaptureHookManager")
    $runtime.AppendChild($typeElem) | Out-Null

    $config.Save($configPath)
    Write-Host "Patched MTGO.exe.config with AppDomainManager entries"
}

Write-Host ""
Write-Host "Done! Now launch MTGO normally."
Write-Host "Capture output: ~/Desktop/mtgo-capture/single_game.bin"
Write-Host "Hook log: ~/Desktop/mtgo-capture/hook_log.txt"
Write-Host ""
Write-Host "After capturing, run uninstall-hook.ps1 to restore original config."
