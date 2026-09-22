# Run in Windows PowerShell 5.1, matching SimHub's .NET Framework runtime.
param([string]$SimHubPath = 'C:\Program Files (x86)\SimHub')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Web.Extensions
[void][Reflection.Assembly]::LoadFrom((Join-Path $SimHubPath 'GameReaderCommon.dll'))
[void][Reflection.Assembly]::LoadFrom((Join-Path $SimHubPath 'SimHub.Plugins.dll'))
$root = Split-Path -Parent $PSScriptRoot
[void][Reflection.Assembly]::LoadFrom((Join-Path $root 'integrations/simhub/bin/Release/net48/RigCompanion.WindBridge.dll'))
Add-Type -ReferencedAssemblies (Join-Path $SimHubPath 'GameReaderCommon.dll') -TypeDefinition @'
public static class WindBridgeFixture {
    public static GameReaderCommon.StatusDataBase Create() {
        return new GameReaderCommon.StatusData<object>(new GameReaderCommon.GameUnitSettings());
    }
}
'@
$plugin = New-Object RigCompanion.WindBridge.WindBridge
$flags = [Reflection.BindingFlags]'Instance,NonPublic'
$type = $plugin.GetType()
# Do not call Init: use an ephemeral test port, never the real companion's port.
$listener = New-Object Net.Sockets.UdpClient(0)
$sender = New-Object Net.Sockets.UdpClient
$sender.Connect([Net.IPAddress]::Loopback, $listener.Client.LocalEndPoint.Port)
$listener.Client.ReceiveTimeout = 2000
$type.GetField('udp',$flags).SetValue($plugin,$sender)
function Receive-Frame {
    $type.GetMethod('Send',$flags).Invoke($plugin,@($null)) | Out-Null
    $source = New-Object Net.IPEndPoint([Net.IPAddress]::Any,0)
    [Text.Encoding]::UTF8.GetString($listener.Receive([ref]$source)) | ConvertFrom-Json
}
try {
    $data = New-Object GameReaderCommon.GameData
    $data.NewData = [WindBridgeFixture]::Create()
    $data.GetType().GetProperty('GameRunning').SetValue($data,$true)
    $data.GetType().GetProperty('GameName').SetValue($data,'IRacing')
    $data.NewData.GetType().GetProperty('SpeedKmh').SetValue($data.NewData,123.4)
    $data.NewData.GetType().GetProperty('CarId').SetValue($data.NewData,'porsche992rgt3')
    $data.NewData.GetType().GetProperty('CarModel').SetValue($data.NewData,'Porsche "Cup" \ test é')
    $data.NewData.GetType().GetProperty('CarClass').SetValue($data.NewData,'GT3')
    $plugin.DataUpdate($null,[ref]$data)
    $frame = Receive-Frame
    if ($frame.version -ne 2 -or !$frame.running -or $frame.speed_kmh -ne 123.4 -or $frame.car_model -ne $data.NewData.CarModel -or $frame.car_id -ne 'porsche992rgt3' -or $frame.car_class -ne 'GT3' -or $frame.game -ne 'IRacing') { throw 'SDK data/UTF-8/JSON round-trip failed' }
    $data.NewData.GetType().GetProperty('CarId').SetValue($data.NewData,'formulavee')
    $data.NewData.GetType().GetProperty('CarModel').SetValue($data.NewData,'Formula Vee')
    $plugin.DataUpdate($null,[ref]$data)
    if ((Receive-Frame).car_id -ne 'formulavee') { throw 'Car switch did not propagate' }
    $type.GetField('updated',$flags).SetValue($plugin,[long]0)
    $frame=Receive-Frame
    if ($frame.running -or $frame.speed_kmh -ne 0 -or $frame.car_id -ne '') { throw 'Stale callback did not clear speed/identity' }
    $data.GetType().GetProperty('GameRunning').SetValue($data,$false)
    $plugin.DataUpdate($null,[ref]$data)
    $frame=Receive-Frame
    if ($frame.running -or $frame.car_model -ne '') { throw 'Game exit retained identity' }
    Write-Output 'PASS: actual SDK callback, v2 UDP JSON/UTF-8, car switch, stale update and game exit; isolated ephemeral port.'
} finally { $plugin.End($null); $listener.Close() }
