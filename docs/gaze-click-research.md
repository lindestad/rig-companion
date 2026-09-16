# Gaze click investigation — September 16

**Resolved later on September 16:** the sboys3 fork now accepts the native HMD system-button command. The user confirmed gaze clicks work in VR. See [installation details](driver-installation.md). The investigation below records why the Xbox route was replaced.

The user's headset test still fails after enabling the gamepad driver. Do not call this fixed. The runtime log now confirms the driver loaded; the earlier disabled-driver diagnosis was a real blocker, but not a complete explanation. The current neutral-device test reads our virtual Xbox from XInput slot 0, so Windows enumeration is working.

## Two input routes

Valve documents input sources, their components, and app-specific action bindings. A trigger value and a button click are distinct components. `/input/system/click` is reserved for dashboard interaction and is supplied by a device driver. A background OpenVR application cannot simply publish another device's driver input through its normal IVRInput action-reading API.

Source: [Valve OpenVR driver documentation](https://github.com/ValveSoftware/openvr/blob/master/docs/Driver_API_Documentation.md#device-inputs).

iVRy's developer guide explicitly describes head-directed laser-mouse navigation using gamepad right trigger. The local SteamVR `vrcompositor_bindings_gamepad.json` agrees: right trigger maps to `/actions/lasermouse/in/leftclick`. Thus the Xbox approach has a documented basis, but its actual operation depends on the active binding, source selection and dashboard focus. A controller icon or a successful ViGEm update is insufficient evidence.

Source: [iVRy developer's gamepad guide](https://steamcommunity.com/app/992490/discussions/4/2646360608728224193/).

Matthieu Bucchianeri's keyboard navigation example takes a different route: it wraps the HMD driver, creates the HMD's `/input/system/click` component, and pulses it when a shared-memory request arrives. Its documented behavior is a click at the head-gaze point while aimed at the dashboard, with dashboard toggle behavior elsewhere. This is a closer match to the user's native controllerless cursor. It needs a driver integration, not just a different keyboard code or Windows mouse event.

Sources: [developer walkthrough](https://github.com/mbucchia/SteamVR-Dashboard-KeyboardNav), [actual HMD shim implementation](https://github.com/mbucchia/SteamVR-Dashboard-KeyboardNav/blob/main/driver_shim/HmdShimDriver.cpp).

## Preferred next implementation

The checked-out sboys3 code already creates and updates `ComponentSystemClick` in PimaxSlam. Add an explicit local command to that existing input path, merging the synthetic pulse with physical button state. Give pulses a driver-owned timeout so a companion crash cannot leave the button pressed. Preserve the existing HMD input profile and physical button behavior. Avoid installing the example shim unchanged: it overrides the input profile and must hook early enough to cooperate with sboys3.

An alternative open-source design, [pad-vr](https://github.com/AJBats/pad-vr/blob/main/src/PadVRController.cpp), creates a tracked virtual controller whose pose follows the headset and emits trigger input. That changes the pointer model and is less direct than the existing HMD-button path for this request.

## Diagnostics added now

The companion verifies its own virtual controller's right trigger using XInput before reporting a successful Windows pulse; it still cannot confirm that the dashboard activated a control. Errors distinguish disabled driver, no active OpenVR gamepad, XInput read failure and a trigger value that did not arrive. `cargo run --example gamepad_probe` reads a neutral device; `-- --pulse` additionally exercises press/release (run with SteamVR/games closed). No driver shim was installed in this iteration.
