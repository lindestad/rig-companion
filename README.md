# Rig Companion

A planned Windows companion for a custom USB control box on a sim-racing rig.

The first feature restores a saved seated reference in SteamVR for a Pimax Dream Air, correcting a misplaced floor without changing iRacing's existing wheel recenter binding. Later features cover SteamVR dashboard/desktop access and commands from an ESP32-S3 or STM32 USB device.

Status: research and implementation plan only. No application or firmware has been implemented or tested on the headset.

- [Stack recommendation](docs/stack.md): Rust + iced, alternatives and integration risks.
- [Implementation plan](docs/plan.md): ordered milestones and acceptance checks.
- [SteamVR source research](docs/seated-reference-research.md): existing sboys3 behaviour, origin APIs, correction mathematics and source links.

Project location: `~/dev/rig-companion`. Keep all project work here. Temporary upstream checkouts are ignored under `.research-seated-20260914/`.
