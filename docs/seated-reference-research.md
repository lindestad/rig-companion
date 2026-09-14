# Saved seated reference for a Pimax Dream Air

Research date: 14 September 2026. Scope: correcting SteamVR lobby/floor calibration from a repeatable seated position; iRacing's existing wheel recenter binding remains a separate action.

## Conclusion

Build a small Windows OpenVR companion application first. It should read the current HMD pose, calculate a fresh origin correction using a saved target height/position/heading, and apply it through SteamVR's chaperone/origin APIs. A keyboard shortcut can exercise this before any electronics are built. Later the USB box invokes exactly the same command.

The APIs and an existing implementation support this approach. It has not been run against the user's headset, installed sboys3 build, or SteamVR runtime. Reliability across startup, standby and game transitions remains a hardware validation task.

## Source findings

Examined temporary checkouts:

- sboys3/CustomHeadsetOpenVR: `21185da63a9e4307ae876b3702a43a43044d451c` (version bump to 1.3.0).
- OpenVR-Advanced-Settings/OpenVR-AdvancedSettings: `31df8b476b4cee7c29695bae993839ef48ec7ea1`.
- mbucchia/PVR, the SDK dependency named by sboys3: `95c3f84a6ab6d1d7a3858b1dca6f5223c06e91d1`.

### sboys3 already compensates for an origin problem

The GUI's **Recenter Pimax Playspace** setting explicitly describes a workaround for Pimax Play forgetting the playspace origin between restarts. This is the project's explanation, not a diagnosis independently confirmed on this headset.

[GUI setting and explanation](https://github.com/sboys3/CustomHeadsetOpenVR/blob/21185da63a9e4307ae876b3702a43a43044d451c/CustomHeadsetGUI/src/app/pages/devices/pimax-slam-settings/pimax-slam-settings.component.html#L9-L22)

`recenterPimaxPlayspace` defaults to true. In `PimaxSlamDriver::PvrTrackingThread`, after about two seconds of continuous positional tracking, the driver selects `pvrTrackingOrigin_FloorLevel` and calls `pvr_recenterTrackingOrigin`. A local `hasRecentered` flag makes this once per tracking-thread lifetime. It does not wait for a user command or consult a saved seated eye height. Merely toggling the config is not a reliable repeatable recenter button because this flag is already set after the first recenter.

[Default setting](https://github.com/sboys3/CustomHeadsetOpenVR/blob/21185da63a9e4307ae876b3702a43a43044d451c/CustomHeadsetOpenVR/src/Config/Config.h#L268-L273) · [Tracking loop](https://github.com/sboys3/CustomHeadsetOpenVR/blob/21185da63a9e4307ae876b3702a43a43044d451c/CustomHeadsetOpenVR/src/Headsets/PimaxSlam.cpp#L1039-L1125)

The same loop forwards PVR headset position, orientation and velocities into SteamVR, with an identity world-from-driver rotation and zero world-from-driver translation. The driver advertises universe ID 30, matching aapvr, for this tracking path. Its `poseOffset` elsewhere in the file belongs to controller model alignment, not headset floor calibration.

The SDK exposes eye-level and floor-level origin modes and a recenter call, but the inspected headers do not establish enough behavioural detail to promise that calling PVR recenter alone fixes a bad floor height.

[PVR origin modes](https://github.com/mbucchia/PVR/blob/95c3f84a6ab6d1d7a3858b1dca6f5223c06e91d1/PVR_Types.h#L870-L875) · [PVR API wrappers](https://github.com/mbucchia/PVR/blob/95c3f84a6ab6d1d7a3858b1dca6f5223c06e91d1/PVR_API.h#L252-L274)

### SteamVR has the necessary correction layer

SteamVR distinguishes raw driver tracking, standing space (floor at Y=0), and seated space. Being physically seated does not mean the lobby uses the seated coordinate space. The floor correction must target the standing space; a seated-only reset is insufficient as a design.

OpenVR exposes current/raw HMD poses, standing and seated origin transforms, working-copy setters, preview, and commit operations. Valve documents these as the mapping between driver coordinates and application coordinates.

[Valve chaperone documentation](https://github.com/ValveSoftware/openvr/blob/master/docs/Driver_API_Documentation.md#chaperone) · [OpenVR interfaces](https://github.com/ValveSoftware/openvr/blob/master/headers/openvr.h)

OVR Advanced Settings demonstrates this in actual application code. `MoveCenterTabController` calculates translations and yaw offsets, sets standing/seated transforms, and shows the working-set preview. Its floor-fix feature derives an offset from a controller on the floor; our reference would instead be the headset at a known seated height. Its existing floor-fix flow expects controllers, so it is not our desired one-button experience unchanged.

[Origin adjustment implementation](https://github.com/OpenVR-Advanced-Settings/OpenVR-AdvancedSettings/blob/31df8b476b4cee7c29695bae993839ef48ec7ea1/src/tabcontrollers/MoveCenterTabController.cpp#L2520-L2720) · [Controller floor measurement](https://github.com/OpenVR-Advanced-Settings/OpenVR-AdvancedSettings/blob/31df8b476b4cee7c29695bae993839ef48ec7ea1/src/tabcontrollers/FixFloorTabController.cpp#L13-L223)

## Proposed user behaviour

1. Establish a good floor once, using a measured seated headset height or manual visual adjustment. Save the desired headset height; optionally also save horizontal position and forward heading in the calibrated standing space.
2. Each session, sit normally and press **Restore seated reference**. Sample a valid, steady pose briefly and calculate a fresh correction. Leave pitch and roll alone: looking slightly down must not tilt the virtual floor.
3. Use the height encoder for small adjustments. Saving a new baseline is a separate deliberate action.
4. Offer height-only restore, full position-and-yaw restore, and undo. Height-only avoids changing the lobby's horizontal alignment.

Store the desired reference in the companion's own profile. Do not simply replay a saved raw pose or yesterday's offset: the raw origin can differ today.

This is a one-shot calibration. Once applied, preserve natural head movement; do not continuously force the HMD back to its target position.

## Calculation and API direction

For a level coordinate system, height-only correction is:

`viewpoint adjustment = desired seated height - current standing-space HMD height`

Example: desired height 1.15 m and current reported height -0.30 m require +1.45 m to the viewpoint. After the correction, a repeated restore from the same position should require approximately zero adjustment.

For full position/yaw restore, using column-vector transforms:

- Let `A` map raw tracking coordinates to the current standing space.
- Measure current position `p` and yaw in that standing space.
- Choose yaw rotation `R` to align current forward direction with the saved direction.
- For target position `p_target`, set translation `t = p_target - R*p`.
- The world-space correction is `C = [R, t]`; the new raw-to-standing transform is `A_new = C*A`.
- `SetWorkingStandingZeroPoseToRawTrackingPose` expects the inverse direction, so pass `inverse(A_new)`.

Do not invert the sign twice or repeatedly accumulate the entire correction. Verify the resulting pose numerically after applying it. If pitch/roll calibration itself is wrong, that is a separate problem; a single natural seated head pose cannot identify true floor tilt reliably.

Read current working transforms only after refreshing the working copy and checking API success. Preserve existing boundary configuration and keep a rollback snapshot. Standing and seated transforms are distinct; do not assign the same matrix to both. Initially validate the lobby's standing correction, then decide how to preserve their relationship for seated applications.

Use working-set preview for interactive encoder adjustment if runtime testing confirms appropriate behaviour; commit a settled change as needed rather than writing persistent configuration on every detent. Preview ownership and persistence must be tested. Avoid two applications simultaneously managing these transforms, such as OVRAS continuously reapplying its own offsets.

## Startup and fallback

Apply calibration after sboys3's initial PVR recenter has completed, or deliberately disable that startup behaviour as part of a later tested configuration. A correction calculated before an upstream recenter may become stale immediately afterward. Wait for stable tracking, re-read poses after applying, and handle SteamVR disconnect/reconnect explicitly.

If standalone chaperone corrections cannot survive the relevant runtime transitions, a targeted sboys3 change is feasible in the PVR tracking path. A shared translation/yaw transform could be applied using the driver pose's world-from-driver fields. It must apply consistently to headset and associated tracked controllers, with review of camera/passthrough alignment. A companion command would be queued and consumed safely by the tracking thread. Keep USB handling outside that time-critical thread.

Do not switch to this fallback before testing the public SteamVR APIs. It adds maintenance and affects more tracking paths. Also, SteamVR corrections do not automatically apply to a separate native Pimax OpenXR runtime that bypasses SteamVR.

## Hardware-independent proof of concept

Build a minimal Windows helper with commands to inspect pose/origins, save a reference, restore height, restore position/yaw, nudge height, and undo. Bind it to spare keyboard shortcuts first. No headset-driver rebuild or firmware is needed for this validation.

Acceptance checks in the actual Dream Air:

- From a visibly wrong lobby height, one restore puts the headset at the stored height.
- Repeating restore does not add the correction again.
- Leaning and looking around still work naturally; the floor stays level.
- Startup with the headset elsewhere, followed by putting it on in the rig, is recoverable by one press.
- Test standby/resume and SteamVR restart, checking whether another press is required.
- Open dashboard/desktop and launch/exit iRacing; verify coordinate transitions and the existing wheel recenter behaviour.
- If controllers are used, verify that their alignment is preserved.
- Undo restores the pre-command calibration.

Once validated, an ESP32-S3 or USB-capable STM32 can send semantic commands such as `RESTORE_REFERENCE` and signed `HEIGHT_STEP` values over an additional USB HID interface or serial interface. Standard USB mouse and keyboard functions remain independent of the companion.

No runtime settings were changed and no driver was built or installed during this research. Initial deletion of temporary checkouts was rejected by automatic approval review. In the subsequent project setup, these notes and the checkouts were moved out of the iRacing workspace into `C:\Users\danie\dev\rig-companion`. Checkouts are now under the ignored `.research-seated-20260914` directory; they are not part of the committed project.
