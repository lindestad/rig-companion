# One-app VR startup

Open **Rig Companion** from the Windows Start menu. Launching it again brings the existing companion window forward.

1. Start Pimax EVO if `PimaxClient.exe` is absent, then wait up to 30 seconds for `pi_server.exe`.
2. Check the persistent custom sboys build and its OpenVR registration. Register it if needed while SteamVR is stopped. The driver is a DLL loaded by SteamVR, so no separate sboys GUI is necessary.
3. Start SteamVR if neither its server nor launcher is running. The companion reconnects every two seconds until available.

Existing processes are reused. Quit, window close and Alt+F4 request SteamVR shutdown and exit the companion, leaving Pimax EVO running. The driver DLL unloads with SteamVR. A settings operation already in progress is allowed to finish first. Startup errors appear in the app; resolve the reported issue and reopen it. A duplicate internal CustomHeadset installation is reported rather than overwritten. `--no-launch` skips startup; `--demo` never launches the stack.

`launch-config.json`, beside the default profile in `%LOCALAPPDATA%\RigCompanion\data`, stores the Pimax executable and persistent driver directory. Edit it if either moves. `launch.log` in the same directory records startup progress. SteamVR's runtime location comes from its OpenVR registration.

After building with `cargo build --release --bins`, run `scripts/install-shortcut.ps1` with the app closed. It deploys the app, CLI, calibration helper and icon to `%LOCALAPPDATA%\Programs\Rig Companion`, then installs the current user's `Programs\Rig Companion.lnk` shortcut. The executable, window and shortcut share the app icon and explicit `RigCompanion.Desktop` identity. It also registers the per-user App Paths entry. Rebuilding alone does not replace the installed app; rerun the installer.

Use `target\release\rigctl.exe quit` from this repo to signal the running GUI. The signal is a per-session auto-reset Windows event, separate for live and demo instances. A successful response means the request was sent, not that SteamVR has finished exiting. The app releases hotkeys, disables automatic reconnection, disconnects OpenVR, invokes `vrmonitor.exe vrmonitor://quit`, and waits up to 20 seconds for SteamVR server/monitor to exit. On timeout the app stays open and shows an error so Quit can be retried. Pimax is never terminated. `Stop-Process`, Task Manager End process and other forced kills cannot invoke graceful cleanup.

Verification on 2026-09-16: the installed shortcut launches the app correctly and shows the new icon. The Windows `Get-StartApps` inventory still omits it on this PC, even after shortcut notification and a Start menu host refresh; Start/search visibility remains unverified. Do not treat the presence of the `.lnk` alone as proof that Windows search has indexed it.

F15 and **Dashboard · F15** now toggle: open the desktop panel when closed, close the dashboard when visible. Closing uses SteamVR's compositor `system_dashboard_toggle` command through `vrcmd.exe`, then checks the dashboard visibility. Valve's developer describes this command [here](https://steamcommunity.com/app/250820/discussions/0/4036976070312856172/?l=russian). The gaze-click pulse remains exclusively on F14.
