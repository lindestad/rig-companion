# Native headset click installation — September 16, 2026

The user confirmed the new gaze click works in VR on the Dream Air.

- Fork: https://github.com/lindestad/CustomHeadsetOpenVR/tree/headset-gaze-click
- Checkout: `C:\Users\danie\dev\CustomHeadsetOpenVR`
- Active persistent driver: `C:\Users\danie\dev\CustomHeadsetOpenVR\output\CustomHeadsetOpenVR`
- Companion executable: `C:\Users\danie\dev\rig-companion\target\release\rig-companion.exe`
- Original release folder, untouched: `C:\Users\danie\vr\CustomHeadset-1.3.0-Windows`
- Previous installed SteamVR driver, preserved: `C:\Users\danie\dev\CustomHeadsetOpenVR\output\rollback-20260916\CustomHeadsetOpenVR`
- Previous registration file backup: `output\rollback-20260916\openvrpaths.vrpath` in the driver checkout.

SteamVR now loads the fork through external-driver registration. The old internal driver was moved to the backup directory to avoid duplicate drivers with the same name. The live capability query returned `ok:rigcompanion:gaze-click:v1`. Existing AppData headset settings are shared and were not changed.

Build instructions and protocol are in the fork's `Docs/companion-gaze-click.md`. Build only with SteamVR stopped because it holds the active DLL open. Use `/p:SkipPostBuild=true` to avoid copying over the internal SteamVR directory. Do not use the old Custom Headset GUI's install/update action: it may replace this registration/build. Its settings UI remains the upstream version.

## Rollback

Close SteamVR and Rig Companion. Unregister the development driver:

```powershell
& 'C:\Program Files (x86)\Steam\steamapps\common\SteamVR\bin\win64\vrpathreg.exe' removedriver `
  'C:\Users\danie\dev\CustomHeadsetOpenVR\output\CustomHeadsetOpenVR'
```

Copy the preserved `rollback-20260916\CustomHeadsetOpenVR` directory back to `C:\Program Files (x86)\Steam\steamapps\common\SteamVR\drivers\CustomHeadsetOpenVR`, after confirming no other installation exists there. Restart SteamVR. Native F14 requires the fork and will report unsupported on the original build; F13 calibration still works.

The abandoned Xbox bridge remains only as diagnostic source/example code. The app's gaze-click command no longer uses it. ViGEmBus and the SteamVR gamepad setting were left installed/enabled; neither is needed for the new click path.
