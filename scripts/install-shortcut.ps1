$ErrorActionPreference = 'Stop'
$repoPath = Split-Path -Parent $PSScriptRoot
$exePath = Join-Path $repoPath 'target\release\rig-companion.exe'
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) {
    throw 'Build the release executable first: cargo build --release'
}
$shortcutPath = Join-Path ([Environment]::GetFolderPath('Programs')) 'Rig Companion.lnk'
$shortcutShell = New-Object -ComObject WScript.Shell
$shortcut = $shortcutShell.CreateShortcut($shortcutPath)
if ((Test-Path -LiteralPath $shortcutPath) -and $shortcut.TargetPath -ne $exePath) {
    throw "A shortcut with a different target already exists: $shortcutPath"
}
$shortcut.TargetPath = $exePath
$shortcut.WorkingDirectory = $repoPath
$shortcut.Description = 'Launch Pimax EVO, the custom headset driver and SteamVR'
$shortcut.IconLocation = "$exePath,0"
$shortcut.Save()
Write-Output $shortcutPath
