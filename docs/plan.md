# Implementation plan

## Product behaviour

Restore a known seated reference in SteamVR after Pimax starts with the wrong origin or floor height. The user sits in the same rig position and explicitly presses restore. Save the desired reference, calculate a fresh correction per restore, then allow natural head motion. Do not continuously pin the head in place.

The existing iRacing wheel recenter remains separate. The first target is the SteamVR lobby, regardless of the user's physical seated posture. Correct standing/floor coordinates first; evaluate seated-space and game transitions explicitly.

## Milestone 1: Read-only OpenVR prototype

- Create a Cargo workspace with a small calibration library, OpenVR adapter and diagnostic executable. Keep the first executable's default action read-only.
- Confirm MSVC/CMake/SDK build prerequisites and binding coverage, including chaperone setup function tables.
- Use an appropriate non-scene OpenVR application type; do not compete with the running VR game for scene ownership.
- One worker thread owns initialization, interface handles, polling and shutdown. It accepts commands and returns plain Rust results; UI callbacks never directly call the FFI.
- Show SteamVR connection, headset identity, tracking validity, current standing-space height, and available origin transforms. Record installed versions for reproducibility.
- Exit cleanly with SteamVR absent, and recover after it restarts without using stale handles.

Acceptance: inspect the real Dream Air's bad height without changing any runtime state. Confirm a current valid pose and read the standing origin transform successfully.

## Milestone 2: Height correction and undo

- Store a versioned desired seated height in an application-data profile, outside the source tree.
- Add explicit save-reference, restore-height, signed height-step and undo commands.
- Sample a short steady interval on command; require valid positional tracking, not merely orientation. Reject nonfinite values or unavailable transforms.
- Read the existing calibration, calculate the correction, apply it and read back the resulting height. Return a result with before/after values.
- Preserve a pre-change snapshot. Undo must be tied to the current runtime/origin context; do not blindly restore an obsolete snapshot after a runtime reset.
- Account for sboys3's startup auto-recenter before applying our correction. Do not automatically change its configuration during discovery.
- Establish preview versus commit semantics on the actual runtime; preserve boundary geometry and avoid committing every encoder tick.

Meaningful unit tests: transform direction and sign, the -0.30 m to 1.15 m example, repeated-restore idempotence, nonfinite input, and profile version handling. Hardware acceptance: one restore fixes lobby height; repeated restore does not accumulate it; undo works and head motion remains natural.

## Milestone 3: Desktop lifecycle prototype

- Build an iced daemon with tray open/quit actions and one registered global shortcut that dispatches restore-height through the same command path.
- Integrate tray and hotkey message-loop ownership on Windows. Feed events through channels/subscriptions rather than busy polling.
- Ensure shortcut press fires once, release does not repeat the action, and collisions produce a useful message.
- Closing settings leaves the companion running; quitting releases hotkeys, tray and OpenVR handles. Add single-instance behaviour before enabling background autostart.
- Verify idle CPU/GPU behaviour with the window closed and during an iRacing session.

Decision gate: if iced lifecycle plumbing is awkward, switch the shell to Tauri while retaining the Rust core. Do this before investing in UI styling.

## Milestone 4: Reference settings and full restore

- UI fields/actions: connection status, profile, target height, capture reference, restore height, restore position/heading, height step, shortcut binding, undo and last result.
- Add horizontal position and yaw calibration using the documented transform maths. Preserve pitch and roll; do not mistake head tilt for floor tilt.
- Saving a reference is distinct from restoring it. Height nudges do not silently overwrite the saved baseline.
- Treat standing and seated origins as distinct, maintaining their relationship only after testing the runtime semantics.
- Validate standby/resume, startup with the headset elsewhere, SteamVR restart, dashboard open/close, game launch/exit, and tracked-controller alignment.

Acceptance: the user can recover from the usual startup miscalibration while seated, without holding a VR controller. No claim of completion until validated in the headset.

## Milestone 5: Dashboard and desktop access

- Prototype OpenVR ShowDashboard and determine how the installed SteamVR exposes its desktop panel and return-to-game behaviour.
- Verify physical mouse interaction with each required panel; do not assume a USB mouse becomes a tracked controller pointer.
- Prefer existing runtime facilities. Custom overlay rendering or a virtual controller is a separate feature if required.

## Milestone 6: USB control box

- Keep normal mouse/keyboard USB HID functions in firmware, independent of the Windows helper.
- Add a vendor-defined HID command interface to the same physical device. Define protocol version, command IDs, request IDs, signed encoder deltas and acknowledgements/status.
- Route RestoreHeight, RestoreReference, HeightStep, ShowDashboard and ShowDesktop through the existing command dispatcher.
- Implement reconnect/identity handling and avoid replaying an old restore command after reconnection. Enable CDC serial only if useful during bring-up.
- Select the MCU board and pin allocation after identifying the user's actual joystick and controls. Firmware language/toolchain is a separate decision from the Windows app.

## Architecture boundaries

Planned modules/crates, introduced as needed:

- `calibration`: pure reference/profile types and transform calculations.
- `steamvr`: safe API around the narrow FFI boundary, snapshots and runtime lifecycle.
- `companion`: worker/command coordination, storage and diagnostics; initially a CLI, later the desktop daemon.
- UI/platform and USB adapters dispatch semantic commands; they do not own calibration logic.

Keep logs and profiles out of Git. Public APIs and a standalone companion are the first approach. Modify sboys3 only if the real runtime tests demonstrate that correction cannot be maintained through those APIs.

## Development workflow

Use Cargo formatting, Clippy and focused tests for each implementation milestone; add just recipes when there are executable targets. Commit atomic milestones locally. No remote publication is currently requested.

All future project files and temporary research belong under `~/dev/rig-companion`, never the iRacing workspace. The imported research checkout directory is ignored and can be removed once no longer useful. It contains upstream sources, not implementation deliverables.
