# Driver controls and camera

## Driver settings

The page opens on **Frequently used**, across all applicable groups. Distortion profile, IPD and FOV appear first, followed by common image and tracking controls. Categories separate Optics & FOV, Image & color, Tracking & camera, Power & presence and Advanced. All settings retains every field, including unfamiliar fields added by future driver versions. The group picker can narrow to one headset or global/shader controls. Search spans categories/groups allowed by the Dream Air/global filter, and changing categories does not discard pending edits.

### Distortion profiles and visualizer

The distortion profile control is now a dropdown populated from `info.json`'s `builtInDistortionProfiles` and `%APPDATA%/CustomHeadset/Distortion/*.json`. JSON comments are supported. Built-ins take precedence over identically named custom files, matching the driver. The default comes first; compatible and untagged profiles are included. The current/default selection is always retained even if missing or incompatible. Disabling the Dream Air/global filter also shows incompatible profiles. Reload refreshes the catalogue. Broken custom JSON is reported without preventing access to other settings.

**Visualize profile** expands a panel under the selector. It plots the selected draft's published angle/radial-position pairs, or separate red/blue percentage-correction pairs, and optionally compares them with the default. Hover shows plot coordinates. White is the selected radial mapping; orange/purple identify red/blue correction; gray identifies default-profile data. Preview is collapsed initially to keep everyday settings close together.

Dots are profile control points and connecting lines are guides. This does not reproduce the driver's Bezier interpolation, final per-eye warp or the user's FOV/zoom/IPD settings. It is not a simulation through the lenses and cannot diagnose tracking drift. Runtime-backed Pimax profiles and malformed point arrays display an unavailable/error message instead of a fabricated curve. Selection is a pending setting until Apply; preview modes/comparison never write settings or profile files. Existing backup and stale-write protection still applies.

**Show spatial grid and vectors** adds a circular single-eye schematic below the existing graphs. Gray is a rectilinear scene grid; white is its forward radial mapping to the panel; orange arrows run from original points to warped points without displacement exaggeration. Color-channel selection and angular radius are preview-only. The green outer radius is normalized to match the reference, which illustrates shape rather than absolute panel magnification. Real lenses should undo the panel warp, making straight scene lines appear straight.

For an input radius `r` in the unit disk, the schematic samples the profile at `atan(r * tan(preview_half_angle))`, then divides its radial output by the green profile's radial output at that half-angle. Red/blue channels multiply the radius by `1 + published_correction_percent / 100`. Values between knots are linearly interpolated; color corrections beyond their published angular range hold their endpoint. The diagram does not reproduce sboys' spline interpolation/extrapolation, lens-center offsets, asymmetric per-eye projection, IPD, FOV or zoom modifiers. It declines profiles without suitable published data. Tests cover the center, tangent-reference mapping, normalization, correction percentage and unsupported angular ranges.

Sources: installed driver `info.json`; fork `54547a9`'s `DeviceConfigComponentBase.ts`, `DistortionProfileConstructor.cpp` and `ConfigLoader.cpp`. Validation covers catalogue precedence, compatibility, custom files with comments, missing selections, invalid point arrays and category fallback. Desktop visual verification was blocked by a Windows computer-use window-binding error (same expected/current `RigCompanion.Desktop` owner).

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

## Follow-up camera investigation

`rigctl camera-status` now polls only OpenVR camera headers for three seconds and releases its streaming handle afterward. On this PC it reported `HasCamera=true`, successful stream acquisition/release, zero available frames, and error 113 (`VRTrackedCameraError_NoFrameAvailable`). No pixels were copied or saved. This is a failed public camera API check, not proof that Pimax's underlying camera is stopped.

The local fork at `54547a9`, `PimaxSlam.cpp`, sends NV12 data to SteamVR through `VRBlockQueue` in `RunFrame()`. Its older `GetVideoStreamFrame()` returns null with a "Not supported" comment. That difference makes a block-queue/runtime compatibility problem another possibility. A next driver-level investigation should log whether `RunFrame` executes, camera active/paused/buffer state, PVR frame availability and sequence changes, and block acquisition/release results. Check while the headset is awake. Do not expose raw room images in logs. The current release does not change the driver or claim a passthrough fix.

`forcePimaxPassthrough` is read in the driver constructor, at line 805. The exact condition is `(runtimeNewEnough && enablePimaxPassthrough) || forcePimaxPassthrough`. Force bypasses both the version gate and normal enable flag, but still requires a non-none PVR VST type and RAW8/NV12 format. It is not a visibility toggle, stream restart, or different rendering path. SteamVR/driver restart is needed. The installed Pimax version already passes the gate and the camera component exists, so enabling Force is not expected to help this case. Source: [PimaxSlam.cpp in the fork](https://github.com/lindestad/CustomHeadsetOpenVR/blob/54547a9/CustomHeadsetOpenVR/src/Headsets/PimaxSlam.cpp#L797).

Additional research: [Rectus's compatibility matrix](https://github.com/Rectus/openxr-steamvr-passthrough/wiki/Compatibility) lists camera access for sboys Pimax but marks it untested, with Room View requiring 1.3.0 beta.2. It does not establish that Dream Air passthrough works. [Valve's camera sample](https://github.com/ValveSoftware/openvr/tree/master/samples/tracked_camera_openvr_sample) demonstrates the public streaming API. An independent overlay would still need verified frame delivery and suitable camera projection; changing only the button command cannot establish those.
