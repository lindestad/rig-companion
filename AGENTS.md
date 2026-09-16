# Rig Companion working agreements

## Scope and existing work

- Work in `C:\Users\danie\dev\rig-companion`; never put project files in the iRacing directory.
- Inspect recent git history and current changes before modifying existing work. Never revert user changes.
- Reproduce the real failing path before fixing a bug. A successful command or an existing shortcut file is not proof of a working headset feature or a visible Start menu entry.
- Use the personal `gh-publish` skill for commits and publishing. Keep commit messages short and lowercase. Do not push unless asked.
- Keep Pimax EVO running when shutting down the companion. Use `rigctl quit` and wait for graceful shutdown; do not force-kill it to replace binaries.

## Required final step after every commit

After committing completed work, the final delivery step MUST be:

```powershell
.\scripts\release.ps1
```

This compiles **after the commit**, using `cargo build --release --bins`, and installs into the same persistent directory every time:

`C:\Users\danie\AppData\Local\Programs\Rig Companion`

The main executable MUST always be named **`rig-companion.exe`**, overwriting the previous installed version. Also overwrite `rigctl.exe`, `eye-probe.exe` and the icon in that directory. Never deliver a timestamped, versioned or temporary executable name. Never leave only a debug build or a build under `target` as the delivered app.

The release script preserves the previous installation if compilation fails, requests graceful shutdown before overwriting a running installation, refreshes the shortcut, verifies copied executable hashes, and reopens the installed app if it was running. Verification/reporting may follow this step; if code changes afterward, commit those changes and repeat the release step. Do not report completion if release compilation or installation failed.

Run relevant tests and checks before committing. Keep release output out of git. The stable shortcut target is the installed executable above, never a transient build artifact.

## Known failures requiring real verification

- As of 2026-09-16, the user confirms F16 camera passthrough does not display. SteamVR logs receiving the toggle command, but the visual outcome is broken. Do not describe passthrough as working until verified in the headset.
- Start menu discovery was fixed on 2026-09-16. Packaged-host filesystem virtualization had redirected the shortcut to a private LocalCache/Roaming directory. Always verify the shortcut's physical path and its RigCompanion.Desktop entry in Get-StartApps. For a first install from a packaged host, use Explorer to copy the staged shortcut into the real Programs folder; do not treat a virtualized shortcut as installed.
