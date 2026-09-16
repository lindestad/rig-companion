# One-app VR startup

Open **Rig Companion** from the Windows Start menu. Launching it again brings the existing companion window forward.

1. Start Pimax EVO if `PimaxClient.exe` is absent, then wait up to 30 seconds for `pi_server.exe`.
2. Check the persistent custom sboys build and its OpenVR registration. Register it if needed while SteamVR is stopped. The driver is a DLL loaded by SteamVR, so no separate sboys GUI is necessary.
3. Start SteamVR if neither its server nor launcher is running. The companion reconnects every two seconds until available.

Existing processes are reused. Closing the companion leaves Pimax and SteamVR running. Startup errors appear in the app; resolve the reported issue and reopen it. A duplicate internal CustomHeadset installation is reported rather than overwritten. `--no-launch` skips startup; `--demo` never launches the stack.

`launch-config.json`, beside the default profile in `%LOCALAPPDATA%\RigCompanion\data`, stores the Pimax executable and persistent driver directory. Edit it if either moves. `launch.log` in the same directory records startup progress. SteamVR's runtime location comes from its OpenVR registration.

After building with `cargo build --release`, run `scripts/install-shortcut.ps1` to install or refresh the current user's Start menu shortcut. It points at the persistent release executable in this repository.

F15 and **Dashboard · F15** now toggle: open the desktop panel when closed, close the dashboard when visible. Closing uses SteamVR's compositor `system_dashboard_toggle` command through `vrcmd.exe`, then checks the dashboard visibility. Valve's developer describes this command [here](https://steamcommunity.com/app/250820/discussions/0/4036976070312856172/?l=russian). The gaze-click pulse remains exclusively on F14.
