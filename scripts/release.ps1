# Build and deploy the committed app to its permanent path.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repoPath = Split-Path -Parent $PSScriptRoot
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\Rig Companion'
$buildDir = Join-Path $repoPath 'target\release'
$installedExe = Join-Path $installDir 'rig-companion.exe'
$buildExe = Join-Path $buildDir 'rig-companion.exe'

Push-Location $repoPath
try {
    # Compile first: a build failure must leave the installed version usable.
    & cargo build --release --bins
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed; the installed app was not replaced.' }

    $running = @(Get-Process rig-companion -ErrorAction SilentlyContinue)
    foreach ($process in $running) {
        if ($process.Path -notin @($installedExe, $buildExe)) {
            throw "Another Rig Companion executable is running: $($process.Path). It was not stopped."
        }
    }
    $relaunch = $running.Count -gt 0
    if ($relaunch) {
        & (Join-Path $buildDir 'rigctl.exe') quit
        if ($LASTEXITCODE -ne 0) { throw 'Graceful quit request failed; installed files were not replaced.' }
        $deadline = (Get-Date).AddSeconds(30)
        do {
            $remaining = @($running | Where-Object { -not $_.HasExited })
            if ($remaining.Count -eq 0) { break }
            if ((Get-Date) -ge $deadline) { throw 'The app did not exit gracefully; installed files were not replaced.' }
            Start-Sleep -Milliseconds 250
        } while ($true)
    }

    & (Join-Path $PSScriptRoot 'install-shortcut.ps1')
    foreach ($file in @('rig-companion.exe', 'rigctl.exe', 'eye-probe.exe', 'eye-calibrate.exe')) {
        $builtHash = (Get-FileHash -LiteralPath (Join-Path $buildDir $file) -Algorithm SHA256).Hash
        $installedHash = (Get-FileHash -LiteralPath (Join-Path $installDir $file) -Algorithm SHA256).Hash
        if ($builtHash -ne $installedHash) { throw "Installed file does not match release build: $file" }
    }
    if ($relaunch) {
        Start-Process -FilePath $installedExe -WorkingDirectory $installDir -WindowStyle Hidden
    }
    Write-Output "Release installed and verified: $installedExe"
} finally {
    Pop-Location
}
