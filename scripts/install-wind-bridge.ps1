[CmdletBinding()]
param([string]$SimHubPath = 'C:\Program Files (x86)\SimHub')
$ErrorActionPreference = 'Stop'
$repoPath = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repoPath 'integrations\simhub\RigCompanion.WindBridge.csproj'
& dotnet build $project -c Release "-p:SimHubPath=$SimHubPath"
if ($LASTEXITCODE -ne 0) { throw 'SimHub wind bridge build failed.' }
if (Get-Process SimHubWPF -ErrorAction SilentlyContinue) {
    throw 'Bridge built. Exit SimHub, then rerun this script to install it.'
}
$source = Join-Path $repoPath 'integrations\simhub\bin\Release\net48\RigCompanion.WindBridge.dll'
$destination = Join-Path $SimHubPath 'RigCompanion.WindBridge.dll'
Copy-Item -LiteralPath $source -Destination $destination -Force
if ((Get-FileHash -LiteralPath $source).Hash -ne (Get-FileHash -LiteralPath $destination).Hash) {
    throw 'Installed bridge hash does not match build.'
}
Write-Output 'Wind bridge installed. Start SimHub and enable Rig Companion Wind Bridge in Settings > Plugins.'
