# Pimax eye tracking request

Draft for Pimax support or Discord. Not sent.

Hi Pimax team — I use a Dream Air with Pimax EVO Open Port Mode and the SBoys3 native SteamVR driver. Eye tracking reaches SteamVR, but launching Pimax’s Eye Tracking Guide shows only a desktop Unity window. Its calibration targets do not appear in the headset as a VR app. Switching back to the Pimax runtime for calibration is disruptive and sometimes takes headset or PC restarts.

Is there a supported way to run the guide in Open Port Mode, such as an OpenXR or SteamVR build? Alternatively, could Pimax provide an authorized calibration API or helper so a SteamVR app can show the targets while Pimax/Tobii handles collecting, applying, and saving the calibration? Our separate calibration probe was rejected by license validation, so we’re looking for an approved integration.

We also sometimes get no fresh eye samples when starting SteamVR. Restarting the headset restores them; restarting only the Tobii Windows service did not. Is there a supported eye-tracker-only reconnect or reset?

I can provide logs and test a supported solution.
