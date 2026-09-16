# Testing the first iced implementation

**Latest result, September 16:** the modified sboys3 driver built successfully, its pulse expiry/concurrency tests passed, the live capability handshake succeeded, and the user confirmed F14 gaze clicking works in VR. The app now uses the native HMD system button. Earlier Xbox diagnostics below are historical. F13 was also confirmed working by the user.

## First session in the Dream Air

1. Start SteamVR normally and put on the headset. Open Rig Companion on the desktop, then view it through SteamVR's desktop panel.
2. Connect if the companion started before SteamVR. Wait for **Tracking ready**. The default seated height is **98 cm**. Change and save it only if needed.
3. Click **Restore seated height**. Settle back into the normal seated position during the three-second countdown and hold still briefly.
4. Check both the live height reading and the apparent SteamVR floor. The reported headset height should be near 98 cm. Lean and look around to check normal head movement.
5. Use **Undo last change** to return to the pre-correction origin. Repeat restore and check that repeated presses do not accumulate an offset.
6. Try 5 mm / 1 cm / 5 cm steps. The plus button raises the viewpoint; minus lowers it. Your saved reference should stay at 98 cm unless you save or capture another value.
7. Once the floor looks correct, sit normally and look forward, then **Capture current position**. This stores height, horizontal position and yaw. Test **Restore position + heading** separately.
8. Global shortcuts start enabled. F13 restores 98 cm and recenters position/heading; without a captured reference it uses origin/forward. Undo cancels a pending countdown. F14 pulses virtual gamepad right trigger while the dashboard is visible. Test by looking at a harmless dashboard tab and pressing F14. The first press attaches the gamepad; retry once if SteamVR is still discovering it.

Do not use current-position capture while below the floor: that would teach the wrong reference. The app rejects a captured height outside the configured 20–250 cm range. Restoring the 98 cm target works from a negative measured height.

## What has been checked

September 16: 13 unit tests pass, including F13 default-origin restoration, saved-position/heading restoration at 98 cm and undo. Clippy passes. A real ViGEmBus neutral-controller probe attached successfully and detached without pressing any buttons (`cargo run --example gamepad_probe`). Live gaze selection still requires the user's headset check. Live GUI connected to Dream Air and reported tracking ready.

- Rust/MSVC build against the OpenVR SDK bindings on this PC.
- Unit tests for correction sign, repeated restores, rotated existing origins, matrix ABI layout conversion, invalid input, atomic profile replacement, preservation of unsupported profiles, default 98 cm restore, nudge behaviour and protection against undo after an external origin change.
- CLI simulated end-to-end sequence: connect, save height, restore, nudge, undo, inspect JSON result.
- Actual iced desktop window: default 98 cm, baseline save, restore countdown, reported height change, undo, and global shortcut registration/restore in demo mode.
- Live read-only CLI with SteamVR stopped: correctly returns OpenVR background-app error 121 without starting SteamVR or applying any correction.
- Formatting and Clippy with warnings treated as errors.

## Historical validation notes (superseded where noted below)

- Live SteamVR commits/read-back with the Dream Air, behaviour across standby/restart and iRacing transitions, dashboard opening, and controller alignment.
- No performance claim has been established for use during racing. The UI uses iced's wgpu renderer and snapshots at 4 Hz; profile/command work and OpenVR calls run on an owning worker thread.
- The startup guard is three seconds from connection plus steady-pose sampling. It reduces conflicts with sboys3's startup recenter, but does not receive an explicit “Pimax startup recenter complete” signal.

## Runtime behaviour and limitations

September 16 gaze-click diagnosis: the actual SteamVR 2.17 installation defaults `driver_gamepad.enable` to false, and vrserver logged that it skipped loading the driver. ViGEm attachment alone did not establish an active OpenVR gamepad. Enable the setting and restart SteamVR before testing. `rigctl enable-gamepad` changes it through the live settings API; `rigctl gamepad-status` reads the setting and checks for an active device with tracking-system name `gamepad`. The GUI now refuses to report a click when either prerequisite is missing. The setting was enabled in this PC's user configuration while SteamVR was stopped, with the previous file backed up under ignored `target/steamvr-before-gamepad-*.vrsettings`. Gaze selection after restart remains to be verified.

This build commits the standing-to-raw origin through ChaperoneSetup. It refreshes the working copy before each change, preserves other calibration fields, and verifies the resulting live transform. The change can persist in SteamVR after exiting the app. It is not a transient preview session. Undo is held in memory and invalidated when a different live origin/universe is detected; restarting the companion loses undo history.

The seated origin is deliberately not overwritten by the standing correction. A game with its own seated recenter may behave differently from the lobby; test those transitions before expanding the correction policy. The box's Windows mouse and keyboard firmware remains a separate future component.

F15 now opens the desktop through the OpenVR overlay API and closes a visible dashboard through SteamVR's compositor command. Closing the app quits it; minimizing keeps hotkeys active. Automatic reconnect retries every two seconds; tray support and Windows autostart remain unimplemented.

## September 16 launcher and toggle verification

- The user confirmed F13 correction, native HMD gaze clicks and desktop opening. The earlier gamepad experiment below is superseded by the native bridge.
- Formatting, Clippy with warnings denied, and all 15 tests passed; release build succeeded.
- With Pimax running and SteamVR stopped, opening the companion started SteamVR and connected automatically. The native driver capability query returned supported.
- F15 closed the dashboard (visibility read-back confirmed) and reopened `system.desktop.1`. The Start menu shortcut target was verified.
- Pimax cold startup was not exercised; existing Pimax processes were reused. Driver registration was already present, so automatic registration remains untested live.

The F13 delay is now 500 ms, followed by the existing 320 ms stability sampling. UI timer resolution is 50 ms; other GUI capture/height commands retain their three-second countdown. The physical XInput probe confirmed virtual trigger 0 → 255 → 0 with SteamVR stopped. See [gaze-click research](gaze-click-research.md) for documented HMD and gamepad routes and the still-unresolved dashboard selection failure.
