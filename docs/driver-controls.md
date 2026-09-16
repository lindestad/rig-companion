# Driver controls and camera

## Driver settings

The settings page reads `%APPDATA%\CustomHeadset\info.json` for the installed driver's defaults and `%APPDATA%\CustomHeadset\settings.json` for overrides. It is an editor for that installed driver, not a second driver process.

The default filter includes Dream Air, General Headset and global fields, plus applicable Custom Shader fields. Turning it off exposes other headset groups. Search spans all groups permitted by the filter. Unknown settings added by the driver are retained. Fields that have no documented GUI metadata remain plain typed inputs.

Sliders, number steps, dropdown values, help text and restart notes come from the sboys GUI's `headset-settings`, `pimax-slam-settings` and `general` components. The metadata was imported from the local fork at `54547a9`. Their metadata is embedded in `assets/driver-controls.json`; refresh it using `python scripts/import-driver-controls.py <path-to-fork>`. This is independent of the live defaults schema. Sources: [sboys3 headset controls](https://github.com/sboys3/CustomHeadsetOpenVR/blob/main/CustomHeadsetGUI/src/app/pages/devices/headset-settings/headset-settings.component.html), [general controls](https://github.com/sboys3/CustomHeadsetOpenVR/blob/main/CustomHeadsetGUI/src/app/pages/devices/general/general.component.html). The imported tooltips retain their upstream wording. Numeric input steppers use the upstream increment; slider ranges are UI ranges, not blanket validation limits on typed values. Dream Air FOV limits come from the connected driver's resolution data at load time, with 98/88 degree fallbacks like upstream.

Hover the info icon for details, including restart requirements. Edits, steps, sliders and resets are drafts until Apply. Reset removes the override, restoring the driver default. Apply validates the type and known limits, refuses stale writes if the file changed externally, makes an exact backup in the companion data directory's `driver-settings-backups`, and atomically replaces the settings file. Other groups and unknown keys are preserved. Comments are retained in the backup, but not in the rewritten JSON. The driver watches the file; some changes still need SteamVR or game restart as described by their help. No automatic restart occurs on Apply.

## Camera toggle

F16 and Camera use `vrcmd.exe --compositorcmd camera_room_view_toggle`. This command exists in the installed SteamVR dashboard's `resources/webinterface/dashboard/debugcommands.js`; it is a compositor command rather than a documented OpenVR application toggle API. It should be rechecked after major SteamVR updates.

Prerequisites:

- Dream Air `enablePimaxPassthrough` enabled in the custom driver, with compatible Pimax EVO.
- SteamVR Camera enabled and Room View configured. Restart SteamVR if its settings request it.
- OpenVR's `IVRTrackedCamera::HasCamera` reports a camera for HMD index 0.

The app checks `camera.enableCamera` and `HasCamera`, then sends the toggle. It reports that a toggle was requested, not that it verified a camera image. It does not capture or store camera frames. The driver's `forcePimaxPassthrough` setting bypasses its Pimax compatibility check; it is not a show/hide switch and is not changed by F16.

References: [OpenVR tracked camera API](https://github.com/ValveSoftware/openvr/blob/master/headers/openvr.h), [sboys3 Pimax SLAM controls](https://github.com/sboys3/CustomHeadsetOpenVR/blob/main/CustomHeadsetGUI/src/app/pages/devices/pimax-slam-settings/pimax-slam-settings.component.html).

## Verification, 2026-09-16

22 tests, Clippy with warnings denied, formatting and release build passed. Desktop inspection verified the default filter, other-headset groups when unfiltered, embedded number buttons (black level 0 to 0.001), and the hover help. The draft was discarded; live display settings were not changed. SteamVR reported a camera and accepted the toggle command; in-headset Room View appearance still needs user verification. Two CLI shutdown/relaunch cycles exited Rig Companion and SteamVR, preserved Pimax EVO and loaded the custom driver again.

User follow-up: passthrough does **not** display in the headset. SteamVR compositor logs show Room View initialization and receipt of the toggle, and the server logs show Pimax camera queue creation. This confirms command delivery only; the camera stream/display path remains unresolved.
