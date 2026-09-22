# Wind simulator

The **Wind simulator** page controls the LINDESTAD WindPCB B.3 over native USB. SimHub forwards normalized vehicle speed to Rig Companion using the included **Rig Companion Wind Bridge** plugin. Rig Companion is the only owner of the COM port; do not configure a SimHub Custom Serial Device for this board. No virtual COM driver is needed.

Enable wind, choose a run mode, set minimum/maximum PWM percentages and the curve shape, then **Apply changes**. Speed follows an editable power curve (default exponent 0.60). The active iRacing car selects an approximate top speed; maximum fan demand is reached 15 km/h below that estimate. The page shows the car, estimate, source category and draft curve graph. Unknown cars use the editable fallback; manual mode overrides the automatic estimate. See [car estimates and curve](wind-car-speeds.md). **Stop now** disables wind immediately and saves the disabled setting. Settings are drafts until applied; moving a slider alone does not change the running fans. A brief 100% startup kick is implemented by the firmware and can exceed the steady-state maximum.

- **Always (minimum when idle):** minimum airflow whenever Rig Companion is open; fresh SimHub game speed increases it up to maximum.
- **While SteamVR is running:** the same behavior while `vrserver.exe` is running. This checks runtime availability, not whether the headset is being worn.
- **Only while iRacing simulator is running:** airflow only while an iRacing simulator process exists. The iRacing launcher alone does not qualify. Supported names: `iRacingSim64DX11.exe`, `_EAC.exe`, `_EOS.exe`. Processes are checked every two seconds.

Each side can be disabled independently. L1/L2 share left demand; R1/R2 share right demand. Both enabled sides currently use the same speed effect. Connect fans directly to their individual board headers to get individual RPM readings. A missing commanded fan raises a warning, without stopping the others. With minimum 0%, the firmware stops below 5% demand.

The controller is discovered by USB VID/PID 303A:1001. Automatic mode only chooses when exactly one matching device exists; otherwise select its port. Before sending anything, the worker waits for valid version-2 wind telemetry. It reconnects automatically. Unknown/old firmware cannot be mistaken for a working RPM controller. **Release USB for firmware update** stops airflow and closes the port; **Reconnect controller** resumes the saved run mode.

Rig Companion must remain open or minimized to control airflow. Closing it sends stop and closes the port. The firmware's independent 500 ms command deadline stops the fans if the app crashes or communication ceases. Control refresh is 100 ms; RPM display refresh is one second. A disconnected/stale board shows unavailable RPM, not an old live value. Missing SimHub data for one second falls back to minimum within the selected run mode; leaving that mode stops airflow entirely. Invalid packets never refresh data freshness.

## Telemetry and limitations

The page shows per-header RPM, missing-tach warnings, requested output, controller state, uptime and accepted/rejected command counts. RPM is estimated over a one-second window assuming two tach pulses per revolution. This factor and absolute RPM have not been independently measured with a tachometer. Zero RPM does not by itself distinguish an empty header from a stopped motor.

The board has no supply voltage/current/temperature ADC. The eFuse fault line drives the red LED but is not wired to an MCU input. The UI therefore does not invent voltage readings or classify hardware power faults. USB telemetry is controller state, not proof of measured power-rail behavior.

Firmware and PCB source: [lindestad/wind-controller](https://github.com/lindestad/wind-controller). Its version-1 command stays `W,<left>,<right>\n` with integer 0–1000 demands. Basic `S,1,<mode>,<mask>\n` reports continue at 500 ms. Extended reports arrive about every second:

```text
T,2,mode,warning_hex,uptime_ms,accepted,rejected,rpm_L1,rpm_L2,rpm_R1,rpm_R2,pwm_L1,pwm_L2,pwm_R1,pwm_R2\n
```

Modes: 0 stopped, 1 precharge, 2 active, 3 command timeout, 4 fault. Warning bits: L1=1, L2=2, R1=4, R2=8. PWM fields are actual requested per-header outputs, including kicks. Firmware queues reports using bounded interrupt-driven TX so USB backpressure cannot block the control loop.

## SimHub bridge

Build/install with SimHub closed:

```powershell
.\scripts\install-wind-bridge.ps1
```

Start SimHub and enable **Rig Companion Wind Bridge** in the detected-plugins dialog or Add/remove features. The plugin is .NET Framework 4.8 and compiles against the locally installed SimHub SDK DLLs. Source stays in `integrations/simhub`; only `RigCompanion.WindBridge.dll` is copied into SimHub. No unrelated settings/plugins are changed.

The plugin sends a heartbeat every 100 ms to `127.0.0.1:29814`. It copies `GameData.NewData.SpeedKmh` in the normal SDK callback and does socket I/O on its own timer. Stale/no game updates produce `running:false`, speed zero. It has no network destination other than loopback and never opens a serial port. The receiver binds only loopback, validates packet shape/ranges, bounds receive work, and uses receive time for freshness. This is a local accessory interface, not an authenticated network protocol.

```json
{"version":2,"running":true,"speed_kmh":123.4,"game":"IRacing","car_id":"formulavee","car_model":"Formula Vee","car_class":"Formula Vee"}
```

SDK references: [SimHub plugin SDK](https://github.com/SHWotever/SimHub/wiki/Plugin-and-extensions-SDKs), the installed `PluginSdk/User.PluginSdkDemo/DataPluginDemo.cs`, and [serialport-rs](https://github.com/serialport/serialport-rs).

## Validation

38 Rust tests pass, including mode gating, speed mapping/caps, stale SimHub fallback, stale controller stop, strict protocol parsing and settings validation. Clippy passes with warnings denied. The plugin builds against the installed SimHub 9.12.4 assemblies with no warnings. The updated firmware passes 26 host tests and 43 target-build checks; flashing was hash-verified and the physical board returned valid stopped-state RPM telemetry.

With the actual SimHub plugin enabled, `cargo run --example wind_smoke` passes USB telemetry/heartbeat, disabled output, release/reconnect and shutdown checks. This test deliberately leaves fan outputs at zero and requires the GUI closed. It does not launch iRacing or prove on-track wind feel. Vehicle-speed behavior is unit-tested; a real driving session remains the user acceptance check.

Settings live beside the existing profile as `wind.json` (`demo-wind.json` in demo). Missing files use disabled defaults; malformed or newer settings are preserved, reported, and cannot be overwritten from the page. Defaults are 20% minimum, 80% maximum, curve exponent 0.60, automatic car estimate and a 250 km/h fallback top speed (maximum at 235 km/h). Open this page directly with `rig-companion.exe --wind`; combine with `--no-launch` to avoid launching the VR stack.

Upgrade the bridge DLL alongside Rig Companion: the new receiver requires v2 car identity fields. `scripts/test-wind-bridge.ps1` exercises actual SDK callbacks and JSON/UDP on an isolated ephemeral port using Windows PowerShell 5.1.
