$ErrorActionPreference = 'Stop'
$repoPath = Split-Path -Parent $PSScriptRoot
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\Rig Companion'
$exePath = Join-Path $installDir 'rig-companion.exe'
$buildDir = Join-Path $repoPath 'target\release'
foreach ($file in @('rig-companion.exe', 'rigctl.exe', 'eye-probe.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $buildDir $file))) { throw 'Run cargo build --release --bins first.' }
}
if (Get-Process rig-companion -ErrorAction SilentlyContinue | Where-Object Path -EQ $exePath) {
    throw 'Run target\release\rigctl.exe quit and wait for the app to exit before installing.'
}
$shortcutPath = Join-Path ([Environment]::GetFolderPath('Programs')) 'Rig Companion.lnk'
$shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcutPath)
$oldTarget = Join-Path $buildDir 'rig-companion.exe'
if ((Test-Path -LiteralPath $shortcutPath) -and $shortcut.TargetPath -notin @($exePath, $oldTarget)) {
    throw "A shortcut with a different target already exists: $shortcutPath"
}
New-Item -ItemType Directory -Path $installDir -Force | Out-Null
foreach ($file in @('rig-companion.exe', 'rigctl.exe', 'eye-probe.exe')) {
    Copy-Item -LiteralPath (Join-Path $buildDir $file) -Destination (Join-Path $installDir $file) -Force
    $installedFile = Get-Item -LiteralPath (Join-Path $installDir $file)
    $installedFile.Attributes = $installedFile.Attributes -band (-bnot [IO.FileAttributes]::NotContentIndexed)
}
Copy-Item -LiteralPath (Join-Path $repoPath 'assets\rig-companion.ico') -Destination $installDir -Force
$shortcut.TargetPath = $exePath
$shortcut.WorkingDirectory = $installDir
$shortcut.Description = 'Launch Pimax EVO, the custom headset driver and SteamVR'
$shortcut.IconLocation = "$(Join-Path $installDir 'rig-companion.ico'),0"
$shortcut.Save()
if (-not ('RigShortcut' -as [type])) { Add-Type -Path (Join-Path $PSScriptRoot 'shortcut-properties.cs') }
[RigShortcut]::Register($shortcutPath)
$appPathsKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\App Paths\rig-companion.exe'
New-Item -Path $appPathsKey -Force | Out-Null
Set-Item -LiteralPath $appPathsKey -Value $exePath
New-ItemProperty -LiteralPath $appPathsKey -Name Path -Value $installDir -PropertyType String -Force | Out-Null
Write-Output $shortcutPath
