# Eye pointer alignment

The **Eye tracking → Align eye pointer in VR** button starts a temporary SteamVR overlay. With the headset on, look at the center purple target, then each target in the inner and outer rings until the next one appears. The 17 targets take about a minute; markers along the bottom show progress. Keep the headset in its usual position on your face. Press Escape on the keyboard to cancel.

The helper gathers distinct, fresh eye samples for each target, rejects unstable or inconsistent captures, and fits a smooth two-dimensional correction across a wider area than the original nine-point view. A failed or cancelled run keeps the prior profile. A successful run saves the profile to `%APPDATA%\Rig Companion\eye-pointer-calibration.json` and applies it immediately. The headset driver reloads the profile when it starts again. Existing nine-point profiles still load until replaced.

This corrects only the dashboard pointer enabled with F19. The SteamVR eye tracking component and the gaze data used by games or foveated rendering are unchanged. It is an alignment aid, not a replacement for Pimax/Tobii tracker calibration. Remove the profile file and restart SteamVR to return to unadjusted eye aim.

The helper only runs during alignment. It creates no virtual controller and no persistent overlay. `eye-calibrate.exe --preview` displays a center target for six seconds without recording gaze or changing the profile.
