# One-app VR startup

Open **Rig Companion** from the Windows Start menu. Launching it again brings the existing companion window forward.

1. Start Pimax EVO if `PimaxClient.exe` is absent, then wait up to 30 seconds for `pi_server.exe`.
2. Check the persistent custom sboys build and its OpenVR registration. Register it if needed while SteamVR is stopped. The driver is a DLL loaded by SteamVR, so no separate sboys GUI is necessary.
3. Start SteamVR if neither its server nor launcher is running. The companion reconnects every two seconds until available.

Existing processes are reused. Quit, window close and Alt+F4 request SteamVR shutdown and exit the companion, leaving Pimax EVO running. The driver DLL unloads with SteamVR. A settings operation already in progress is allowed to finish first. Startup errors appear in the app; resolve the reported issue and reopen it. A duplicate internal CustomHeadset installation is reported rather than overwritten. `--no-launch` skips startup; `--demo` never launches the stack.

`launch-config.json`, beside the default profile in `%LOCALAPPDATA%\RigCompanion\data`, stores the Pimax executable and persistent driver directory. Edit it if either moves. `launch.log` in the same directory records startup progress. SteamVR's runtime location comes from its OpenVR registration.

After building with `cargo build --release --bins`, run `scripts/install-shortcut.ps1` with the app closed. It deploys the app, CLI, calibration helper and icon to `%LOCALAPPDATA%\Programs\Rig Companion`, then installs the current user's `Programs\Rig Companion.lnk` shortcut. The executable, window and shortcut share the app icon and explicit `RigCompanion.Desktop` identity. It also registers the per-user App Paths entry. Rebuilding alone does not replace the installed app; rerun the installer.

Use `target\release\rigctl.exe quit` from this repo to signal the running GUI. The signal is a per-session auto-reset Windows event, separate for live and demo instances. A successful response means the request was sent, not that SteamVR has finished exiting. The app releases hotkeys, disables automatic reconnection, disconnects OpenVR, invokes `vrmonitor.exe vrmonitor://quit`, and waits up to 20 seconds for SteamVR server/monitor to exit. On timeout the app stays open and shows an error so Quit can be retried. Pimax is never terminated. `Stop-Process`, Task Manager End process and other forced kills cannot invoke graceful cleanup.

Verification on 2026-09-16: Start menu discovery is fixed. The original shortcut had been redirected by the packaged Codex host into its LocalCache/Roaming/Microsoft/Windows/Start Menu/Programs directory. Its logical path looked correct from the creating process, but Explorer and Start could not see it. GetFinalPathNameByHandle revealed the physical location. The executable was already in the real permanent directory.

The staged shortcut was copied through Explorer into the real per-user Programs folder, and the obsolete private-cache copy was removed. Windows now enumerates Rig Companion / RigCompanion.Desktop in Get-StartApps. Repeated scripted installations have been verified to update the real shortcut successfully. The installer stages the shortcut under target/release, verifies its destination's physical path, and requires Start app discovery before reporting success. A first install from a packaged host that redirects the new file fails with instructions to copy the staged shortcut using Explorer. It never silently reports success. See Microsoft's [packaged desktop filesystem virtualization documentation](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-behind-the-scenes).

F15 and **Dashboard · F15** now toggle: open the desktop panel when closed, close the dashboard when visible. Closing uses SteamVR's compositor `system_dashboard_toggle` command through `vrcmd.exe`, then checks the dashboard visibility. Valve's developer describes this command [here](https://steamcommunity.com/app/250820/discussions/0/4036976070312856172/?l=russian). The gaze-click pulse remains exclusively on F14.

F15 dashboard-close and F16 camera helper processes have a three-second deadline. A stalled helper is terminated and its exit checked for up to one further second; SteamVR and Pimax EVO are left running. Errors are shown in the companion, and commands are never retried automatically because a toggle may already have reached SteamVR. Output capture cannot block on a full pipe. This bounds the helper-process waits; it does not add timeouts to in-process OpenVR calls or prove that VRAM pressure caused a particular shortcut failure. Regression tests reproduce a stalled child, verify cleanup and a successful subsequent command, and check large-output failures.

## Post-commit delivery

After every commit run `scripts/release.ps1` (or `just release`). It builds all release binaries after the commit, closes a running live app through its quit signal, overwrites the same files in `%LOCALAPPDATA%\Programs\Rig Companion`, verifies SHA-256 hashes and reopens the app if it was running. The main executable always remains `rig-companion.exe`. This workflow is required by the repository `AGENTS.md`.
