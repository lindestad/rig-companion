# Dream Air gaze display and calibration

## September 24 startup failure observation

During a live SteamVR session, Rig Companion reported invalid gaze while the Tobii VR4PIMAXP3B Windows service was running and Windows showed the EyeChip device as OK. A new driver diagnostic recorded `pvr_getEyeTrackingInfo` returning success with a zero sample timestamp. That establishes that the driver had no gaze sample to forward; it does not distinguish a sleeping headset, vendor startup race, or a stalled eye camera. Restarting SteamVR alone did not restore a nonzero sample in this observation. The user reports that restarting the headset in Pimax EVO usually does.

The driver now checks the PVR result and only forwards an advancing sample timestamp. A backward clock reset after headset restart is accepted after three advancing timestamps in the new epoch. It logs one stall and one resumption transition. This prevents repeated identical PVR timestamps from being presented as fresh gaze and lets the optional dashboard eye pointer fall back to head aim. A vendor call could still return a changing timestamp for stale image data, so this is not proof of camera freshness.

The bundled PVR API exposes a gaze read but no eye-tracker-only reset operation. Pimax service logs show an `active_tobii_runtime` launcher command, but its behavior and safety as a recovery action are not documented here; it has not been invoked by Rig Companion. A targeted recovery control needs a before/after test with the headset worn, a verified fresh gaze sample, and confirmation that a running game's gaze/DFR survives it. Until then, Pimax EVO's own restart remains the known user-confirmed recovery path.

## Implementation follow-up, September 16

### Licensed connection test

With the user's authorization, the probe now uses Tobii's normal `tobii_device_create_ex` entry point and the installed Pimax XR5 license, only after an unlicensed connection identifies generation `XR5` and a Dream Air model prefix `XR5_PIMAX_DA_`. The original connection is closed first. The installed license stays in its original location; its contents are never printed, copied into the repository or transmitted. Pimax's wrapper uses UTF-16LE data and an explicit byte count, which the helper reproduces.

**Live result: license validation code 6, `invalid process name`.** The connection function itself returned success, demonstrating why its return code alone cannot establish license acceptance. The helper rejects any nonzero license-validation result, closes the returned handle and performs no restricted operations. Subsequent driver diagnostics still reported valid gaze with SteamVR running. No backup was retrieved and no calibration was started.

The installed vendor license therefore does not authorize the current helper process. Native calibration remains blocked through this route. Remaining integration options are a vendor-supported calibration entry point in the existing Pimax application, or a license that permits our companion. The probe does not rename executables, change signatures, patch validation or modify license contents. The running app's availability check uses the updated helper immediately; `eye-probe --unlicensed` retains the original diagnostic comparison path.

Validation: 18 existing tests, Clippy and release helper build. The calibration start/apply/rollback path remains unimplemented and untested.

The companion now has an **Eye tracking** page: two angular plots, degrees, driver validity and diagnostic file age. Reads run off the UI/SteamVR worker at 250 ms intervals only on this page while focused; **Keep live in VR** allows unfocused viewing. Invalid, stale, offline and temporarily unreadable samples hide the markers. The recorded PID must match a running vrserver process. Both diagnostic locations are supported. No continuous gaze logging is added. Existing F13/F14/F15 commands stay active.

The **Check calibration availability** button runs a separate `eye-probe.exe` process with a 12-second timeout. It pins Pimax EyeTrackingGuide's installed Tobii ABI to 5.9.0.2, enumerates devices, queries model/generation and 3D calibration capability, and checks calibration retrieval on XR5. It only counts retrieval bytes, without saving a backup or claiming rollback is verified. Following the licensed connection test above it uses the installed XR5 license and reports validation rejection before attempting restricted calls. It does not subscribe to gaze streams, start calibration, change lens geometry or restart services.

Live result during implementation: API version matched, enumeration succeeded but returned **zero devices**. The driver file was fresh, with `valid: false`, and there was no Tobii-named runtime process in the process snapshot. This does not establish the cause; headset power state, Pimax eye-tracking enablement and runtime availability need checking. It prevented testing device coexistence, calibration capability, backup and calibration writes. The in-app button makes this discovery repeatable once eye tracking is active. Native calibration is **not implemented** yet; no start-calibration control is advertised.

CLI: `rigctl eye-status` reads one diagnostic sample; `rigctl eye-calibration-status` runs the isolated discovery helper. Both may run alongside the GUI. Build all executables with `cargo build --release --bins`.

Follow-up after the user restarted eye tracking in Pimax EVO: the user confirmed the preview works in VR. A fresh diagnostic read reported valid gaze with the registered driver running. The isolated probe connected concurrently to generation **XR5**, model **XR5_PIMAX_DA_K11_RAWH**, and the device reported **3D calibration supported**. Calibration retrieval returned code **2**, **Insufficient permissions when using a restricted feature**, with zero backup bytes. This establishes concurrent discovery and a device capability, not permission to calibrate or verified rollback. The current plain `tobii_device_create` client cannot retrieve a backup. The vendor application's licensed `tobii_device_create_ex` path remains a separate integration question; no license was loaded, calibration started, or tracker settings changed.

The installed Pimax interop declarations were inspected again to confirm cdecl ABI, the 2304-byte device-info layout, calibration-3D capability value 2 and no-storage/transfer field-of-use value 1. Temporary inspection tools and excerpts are removed after use. No proprietary code or binaries are checked in.

Research date: 2026-09-16. Scope: this Windows PC, Dream Air SLAM, sboys3 CustomHeadset 1.3.0, and Rig Companion. Research only; no calibration, runtime restart, device subscription or driver configuration change was performed.

## Conclusions

- A live gaze diagnostic panel is straightforward without acquiring the eye tracker: consume sboys' existing diagnostic output. Four updates/second are available today. Pause reads and redraws when the window is unfocused/minimized; preserve the existing command/hotkey worker.
- Smooth gaze monitoring is plausible through SteamVR's OpenVR eye action API, or a small sboys telemetry extension. Neither should require taking ownership of the Tobii device. Concurrent performance and action availability still need measurement.
- A genuine replacement calibration app is technically promising. The installed Pimax application calls Tobii's 3D calibration API separately from its PVR rendering path. Rendering targets in SteamVR while sboys stays enabled appears feasible, but concurrent access, capability/license availability, coordinate mapping, and persistence have not been demonstrated.
- Prefer reusing the vendor calibration algorithm. A correction fitted to already-calibrated gaze is an alternative, but is not equivalent to calibrating the underlying tracker and would need to be applied in sboys to benefit other applications.

## Hardware and actual data path

Pimax identifies Dream Air's tracker as Tobii, with IR emitters and cameras around the lens edges, designed for its pancake optics. This is not evidence that a generic Tobii desktop calibration SDK is directly compatible. [Pimax hardware announcement](https://store.pimax.com/blogs/blogs/pimax-chooses-tobii-eye-tracking-for-the-ultra-compact-dream-air-vr-headset)

The source checkout and upstream HEAD both resolved to `21185da63a9e4307ae876b3702a43a43044d451c` during this research, the 1.3.0 release. The relevant chain is:

```text
Dream Air eye cameras -> Tobii/Pimax runtime -> PVR gaze data
    -> sboys PimaxCommon::EyeTrackingThread
    -> EyeTrackingOutput
        -> SteamVR eye component -> OpenVR/OpenXR clients
        -> diagnostic.json -> companion monitor
```

`PimaxCommon::EyeTrackingThread` polls PVR on an 8 ms timer (integer `1000 / 120`; the source calls this approximately 120 Hz). That is the polling cadence, not proof of physical sensor frequency. It calls `pvr_getEyeTrackingInfo`, checks for a nonzero timestamp and converts per-eye tangent values to angles. `EyeTrackingOutput` produces SteamVR's `/eyetracking` component and sets `Prop_SupportsXrEyeGazeInteraction_Bool`.

Sources: [PimaxCommon.cpp](https://github.com/sboys3/CustomHeadsetOpenVR/blob/21185da63a9e4307ae876b3702a43a43044d451c/CustomHeadsetOpenVR/src/Helpers/PimaxCommon.cpp), [EyeTrackingOutput.cpp](https://github.com/sboys3/CustomHeadsetOpenVR/blob/21185da63a9e4307ae876b3702a43a43044d451c/CustomHeadsetOpenVR/src/Helpers/EyeTrackingOutput.cpp), [PVR types](https://github.com/mbucchia/PVR/blob/main/PVR_Types.h).

The inspected PVR structure contains gaze tangents, a timestamp, convergence distance and blink fields. The sboys path inspected here only forwards the tangent-derived angles; do not promise eyelid, pupil size, raw eye-camera images or independent per-eye confidence from its current diagnostic feed.

## Live panel recommendation

### First implementation: existing diagnostics

On this PC the live file exists at `%APPDATA%/CustomHeadset/diagnostic.json`. The Pimax-branded build uses `%APPDATA%/Pimax/CustomHeadset/diagnostic.json`. sboys writes it every 250 ms whether our app reads it or not. I read a current file while the user's SteamVR session continued; it contained a valid flag, left/right angles and a focal point. No tracker connection was opened by the research.

Implement an iced panel with two angular markers, horizontal/vertical angles, driver-reported validity, and telemetry age. Angles are radians in the source; convert to degrees for display. Label these as gaze estimates, not pictures of the eyes. The observed left and right values were identical in the single inspected snapshot, so do not infer independent binocular accuracy or meaningful convergence from those labels alone.

Use the existing UI cadence or a separate 250 ms task only while focused, visible and the panel is selected. After losing focus, stop eye reads/redraws and label the display paused. An explicit 'keep preview live' toggle is useful when viewing the desktop through VR, where Windows keyboard focus can differ from visual attention. Closing already exits the app. Do not suspend the hotkey/calibration worker when suspending this panel.

The writer truncates and rewrites the file, so tolerate temporary sharing/parse failures and retry at the next normal tick. Check driver PID and file modification age. An old file from a stopped driver is not live telemetry. Do not log continuous gaze samples.

Reading this small existing file cannot take exclusive ownership of the eye camera or a Tobii stream. It adds filesystem/JSON/paint work; its actual CPU/GPU impact has not been benchmarked. Four Hz is adequate for a coarse diagnostic but will look choppy and cannot characterize saccades or end-to-end tracking latency.

Source: [ConfigLoader.cpp](https://github.com/sboys3/CustomHeadsetOpenVR/blob/21185da63a9e4307ae876b3702a43a43044d451c/CustomHeadsetOpenVR/src/Config/ConfigLoader.cpp), particularly `GetConfigFolder`, `WriteDiagnosticInfo`, and `WriteDiagnosticInfoThread`.

### Important validity limitation

The inspected driver supplies a fresh local timestamp when forwarding PVR data instead of propagating the PVR sample timestamp. It only checks that PVR's timestamp is nonzero, not that it advanced, and ignores the function result in that loop. Repeated cached samples could therefore look fresh. Separately, after updates stop, EyeTrackingOutput reports the last gaze for about one second, a valid centered fallback until about three seconds, and then invalid data. Diagnostic angles remain the stored angles during that fallback, while the SteamVR output has become centered.

Consequently, file age means 'the driver wrote telemetry recently', not 'the tracker captured a new sample recently'. This is source-derived behavior, not proof that it caused the user's intermittent failures. For reliable diagnostics, patch sboys to report source timestamp, monotonically increasing sample sequence, PVR result, real validity, and fallback state. Preserve clock-domain distinctions rather than comparing unrelated timestamps.

### Smooth monitor after the initial panel

OpenVR's inspected headers expose `IVRInput::GetEyeTrackingDataRelativeToNow` and `GetEyeTrackingDataForNextFrame`. Use a dedicated eye action/manifest, normal action priority and the existing background OpenVR context. Verify background action access with the installed SteamVR version. Do not add compositor `WaitGetPoses` calls or a scene application simply to draw a desktop monitor. The next-frame API depends on compositor timing, so the relative-to-now API is the better starting point for a background reader.

This yields a combined gaze origin/target and active/valid/tracked flags, not the full Tobii diagnostics. Convert world-space gaze to headset-relative coordinates using a matched head pose before drawing the angular plot. Start at 30 Hz while active. OpenXR's `XR_EXT_eye_gaze_interaction` is another combined-gaze path, but a second OpenXR session introduces unnecessary session/focus concerns for this desktop app.

If per-eye freshness is more important than avoiding a driver patch, publish an on-demand, latest-sample-only snapshot through a named pipe or shared memory from sboys. Avoid writing JSON to disk at sensor rate or blocking vrserver on a slow client. Pause the consumer when inactive; leave the driver's existing eye pipeline running for games/DFR.

Sources: [OpenVR API header](https://github.com/ValveSoftware/openvr/blob/master/headers/openvr.h), [OpenXR eye-gaze extension](https://registry.khronos.org/OpenXR/specs/1.0/man/html/XR_EXT_eye_gaze_interaction.html).

## What Pimax's installed calibration app actually does

Read-only interoperability inspection of `C:/Program Files/Pimax/EyeTrackingGuide/EyeTrackingGuide_Data/Managed/Assembly-CSharp.dll` identified the following behavior. These are local binary findings, not a supported Pimax API contract. The assembly SHA-256 is `ADE48C18C95660706DF78B70B56E25777B948289491B8426EDEBD81C13B0E0E0`.

- A Tobii manager enumerates local devices, queries device generation, chooses an XR5-specific or older license path, and creates a licensed Stream Engine device. It subscribes to wearable consumer/foveated streams.
- Calibration invokes `tobii_calibration_start`, `tobii_calibration_collect_data_3d`, `tobii_calibration_compute_and_apply`, and `tobii_calibration_stop`.
- The target sequence uses two background brightness phases, applying/stopping between them. Sample coordinates are converted to millimetres and include a horizontal sign conversion. Rendering geometry must be understood before reproducing those conversions in OpenVR; do not copy Unity-space coordinates blindly.
- The app waits for a ready `PVRSession`, sets `disable_hmd_ipd_auto_adjust`, and marks `eyetracking_calibrated` on completion. Thus launching the unmodified app under SteamVR is not a demonstrated fix: it has a real PVR dependency, not just a generic OpenXR renderer.
- A lens-configuration method exists, including an IPD adjustment. Its presence is not proof it is called on Dream Air. Do not change device lens geometry as part of a first prototype.

Two different DLL versions are installed: `Runtime/tobii_stream_engine.dll` is **4.24.0.33**, whereas EyeTrackingGuide's plugin is **5.9.0.2**. Use exact, explicitly selected ABI/version information, preferably isolated in a helper process. This discrepancy is a compatibility concern to test, not an established explanation of the user's restarts. Both older Pimax and XR5-named Tobii platform-runtime processes were present; that alone does not identify which owns this headset or prove a conflict.

Vendor license files exist, but their contents were not read or copied. Their presence does not establish permission or capability for a separate app. Tobii's public PDK documentation confirms custom calibration is a licensed capability, but that page concerns desktop products and does not establish Dream Air entitlement. Determine the available supported route and capability results before implementing calibration writes. [Tobii PDK calibration documentation](https://developer.tobii.com/how-to-get-started-with-developing-software-for-the-eye-tracker-with-the-pdk/)

## Dream Air-only calibration architecture

Keep iced as the launcher/status UI. A small separate calibration executable renders stereo targets through **OpenVR**, using SteamVR and sboys. OpenXR could render the same targets if SteamVR is the active runtime, but neither gaze-read API performs Tobii calibration. A separate native adapter must control the calibration backend.

Recommended controlled sequence:

1. While sboys remains active and no race is running, enumerate/query the Dream Air through the available vendor interface. Identify the exact device, DLL ABI and calibration capability; do not select the first arbitrary Tobii device.
2. Establish whether this additional client can coexist with Pimax's existing client. Previewing through SteamVR does not need this access, but calibration mutates shared tracker state and may temporarily interrupt DFR/gaze output.
3. Verify a retrievable/restorable previous calibration and the actual persistence mechanism. Wrapper declarations for retrieve/apply exist; successful backup, rollback and persistence were not tested.
4. Show targets with correct per-eye projections, headset-relative coordinates, units and handedness. Keep IPD/fit stable. Match the required brightness conditions rather than assuming a single five-point pass is equivalent.
5. Start, collect, compute/apply, stop in a serialized worker with explicit errors, cancellation and cleanup. Do not treat success from the last operation as success for the whole sequence.
6. Validate with separate target locations and angular error, then confirm sboys receives fresh gaze again without disabling/restarting it. Test a runtime/headset restart to establish persistence.

Stopping sboys' PVR polling alone may not release Tobii ownership: Pimax's runtime, rather than sboys, is the direct tracker client. If calibration needs exclusivity, investigate coordination at that layer rather than guessing that disabling the SteamVR driver is necessary. Do not switch/restart services automatically in an initial prototype.

If vendor calibration cannot coexist, a fallback is a small per-eye residual correction in sboys before `EyeTrackingOutput`, fitted from known targets. This can correct consistent angular offsets while retaining the vendor tracker. It cannot restore missing eyes, repair raw camera detection, replace lens calibration, or ensure quality after headset movement. A companion-only correction would not fix game DFR. Call this an alignment adjustment, not native tracker calibration.

## Next work and acceptance criteria

First implement the focused-window diagnostic panel and honest paused/stale/driver-valid indicators. Then test a 30 Hz OpenVR reader concurrently with the user's usual game and compare frame timing with the panel active/inactive. Separate that straightforward feature from the experimental calibration helper.

For calibration, the first milestone is capability/coexistence/backup discovery, not a polished target animation. Acceptance requires successful calibration with sboys enabled, no driver switching, verified post-calibration gaze in the companion and game, successful cancel/rollback, and persistence across restart. No such calibration was attempted during this research because the user was testing the live app.

Temporary decompiled excerpts and the local inspection tool were removed after documenting findings. Existing upstream research checkouts were reused; no additional upstream repositories were cloned. No proprietary code, binaries, license keys or recorded gaze streams are included in the repository.
