# Rig Companion

A Windows companion for restoring a comfortable seated reference in SteamVR, built with Rust and iced.

The first feature restores a saved seated reference in SteamVR for a Pimax Dream Air, correcting a misplaced floor without changing iRacing's existing wheel recenter binding. Later features cover SteamVR dashboard/desktop access and commands from an ESP32-S3 or STM32 USB device.

The first implementation includes a dark desktop interface with large controls, a live height diagram, a three-second restore countdown, saved height/position/heading, fine adjustment, undo, optional global hotkeys and a SteamVR dashboard button. **New profiles default to 98 cm**, as requested. Explicitly saved heights are preserved.

## Run

```powershell
cargo run --release --bin rig-companion
```

Or launch `target/release/rig-companion.exe` after `cargo build --release`. The app automatically retries connecting every two seconds while SteamVR is unavailable, including after a runtime restart. At launch it starts Pimax EVO if needed, checks the custom headset driver registration, then starts SteamVR if needed. Use `--no-launch` to connect without launching the stack. See [startup and shortcut](docs/startup.md). Allow tracking to settle, sit normally and use **Restore seated height**. View this desktop app through SteamVR's desktop panel; this is not a custom in-headset overlay.

Use `cargo run --bin rig-companion -- --demo` to exercise the interface without touching SteamVR. Demo profiles are separate from live profiles.

Keep the window open or minimized. Closing it quits the app and releases the hotkeys. **Global shortcuts** are enabled at launch, unless registration fails:

- **F13:** restore 98 cm plus saved horizontal position and heading. Without a captured reference, use the SteamVR origin and forward direction. Waits **0.5 seconds**, then checks tracking stability and applies the correction; sit normally and look forward. The GUI height/capture countdown remains three seconds. Disabling countdown removes the delay for both.
- **F14:** click the SteamVR dashboard gaze pointer through the modified sboys3 HMD driver. The dashboard must be visible. The driver supplies and releases a 120 ms native headset system-button pulse. No virtual Xbox controller is created.
- **F15:** toggle the dashboard. When closed, open the first available desktop panel (or the dashboard if desktop is still loading). When visible, close it. The GUI **Dashboard · F15** button uses the same action.
- **Ctrl+Alt+F8:** saved height only; **Ctrl+Alt+F9:** undo/cancel countdown; **Ctrl+Alt+F7:** toggle dashboard.

F13 always uses 98 cm, without overwriting the saved profile. Disable Global shortcuts to release the bindings. The F14 bridge is intended for the dashboard, not for clicking scene objects in SteamVR Home or games. Holding F14 does not repeat or drag. `rigctl headset-bridge-status` checks the active driver's capability without clicking. See [driver installation and rollback](docs/driver-installation.md).

**Capture current position** records a full reference only when the floor is already correct. Height nudges do not overwrite the saved reference. Undo covers the last correction in the current connection; a detected external origin change invalidates it.

## CLI and development

The **Eye tracking** page displays the custom driver's left/right gaze estimates at four updates per second, with validity and stale-data indicators. Enable **Keep live in VR** to keep the preview updating when Windows focus moves elsewhere. The calibration availability check is read-only; native eye calibration is still experimental and has not been implemented. See [eye-tracking findings](docs/eye-tracking-research.md).

`rigctl eye-status` and `rigctl eye-calibration-status` work alongside the GUI. Build with `cargo build --release --bins` to include the isolated vendor discovery helper.

```powershell
cargo run --bin rigctl -- status
cargo run --bin rigctl -- set-height 98
cargo run --bin rigctl -- restore
cargo run --bin rigctl -- nudge -1
cargo run --bin rigctl -- demo-check
```

CLI results are JSON; errors use stderr and a nonzero exit status. Close the GUI before live CLI operations: the app and CLI share an instance lock. `demo-check` uses temporary storage and runs without SteamVR. The CLI does not yet control an already running GUI process.

Rust 1.98.1 is pinned. The OpenVR dependency also needs Visual Studio C++ Build Tools, a Windows SDK, CMake and libclang. The existing installation on this PC successfully built it. `just check` runs formatting, Clippy and tests; `just demo` starts the simulated interface.

Profiles live in the Windows local application-data directory returned by `directories`, under `RigCompanion` (`profile.json` / `demo-profile.json`), not in this repository. `--profile <path>` overrides the profile file for testing. Saves replace the file atomically; malformed or unsupported profiles are not silently overwritten.

## Validation and scope

Automated calibration/storage tests and desktop demo interaction have passed. The user has confirmed live F13 correction, native gaze clicking and desktop dashboard opening in VR. See [testing notes](docs/testing.md).

The correction currently changes SteamVR's standing/floor origin, preserving the seated origin and boundary data. It does not change Pimax tracking itself or iRacing's recenter bind. Changes are committed to SteamVR calibration; they can persist after quitting. Avoid running another playspace-offset tool while testing. A tray daemon, firmware and USB hardware integration remain future work.

- [Stack recommendation](docs/stack.md): Rust + iced, alternatives and integration risks.
- [Implementation plan](docs/plan.md): ordered milestones and acceptance checks.
- [Testing and first headset session](docs/testing.md)
- [SteamVR source research](docs/seated-reference-research.md): existing sboys3 behaviour, origin APIs, correction mathematics and source links.
- [Dream Air eye-tracking research](docs/eye-tracking-research.md): live monitoring, calibration feasibility, and findings from the installed Pimax software.

Project location: `~/dev/rig-companion`. Keep all project work here. Temporary upstream checkouts are ignored under `.research-seated-20260914/`.
