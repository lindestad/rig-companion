//! OpenVR is initialized and used on one owning thread. Unsafe ABI calls stay here.
use crate::calibration::{Pose, from_rows, matrix_close, pose_from_matrix, to_rows};
use anyhow::{Context, Result, bail, ensure};
use glam::DMat4;
use openvr_sys as vr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    cell::Cell,
    ffi::{CStr, CString},
    marker::PhantomData,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

pub struct SteamVr {
    system: *const vr::VR_IVRSystem_FnTable,
    setup: *const vr::VR_IVRChaperoneSetup_FnTable,
    connected_at: Instant,
    quitting: Cell<bool>,
    _thread_bound: PhantomData<Rc<()>>,
}

static CONTEXT_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct Reading {
    pub pose: Option<Pose>,
    pub headset: String,
    pub universe: u64,
    pub ready: bool,
}

impl SteamVr {
    pub fn connect() -> Result<Self> {
        ensure!(
            CONTEXT_ACTIVE
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok(),
            "An OpenVR context is already active in this process"
        );
        let result = Self::connect_inner();
        if result.is_err() {
            CONTEXT_ACTIVE.store(false, Ordering::SeqCst);
        }
        result
    }

    fn connect_inner() -> Result<Self> {
        // SAFETY: process owns a single OpenVR context; callers serialize access.
        unsafe {
            ensure!(vr::VR_IsRuntimeInstalled(), "SteamVR is not installed");
            let mut error = 0;
            vr::VR_InitInternal(&mut error, vr::EVRApplicationType_VRApplication_Background);
            if error != 0 {
                let desc = vr::VR_GetVRInitErrorAsEnglishDescription(error);
                let message = if desc.is_null() {
                    format!("OpenVR error {error}")
                } else {
                    CStr::from_ptr(desc).to_string_lossy().into_owned()
                };
                bail!("Start SteamVR, then connect. {message}");
            }
            let result = (|| {
                let system = table::<vr::VR_IVRSystem_FnTable>(c"FnTable:IVRSystem_026")?;
                let setup =
                    table::<vr::VR_IVRChaperoneSetup_FnTable>(c"FnTable:IVRChaperoneSetup_006")?;
                Ok(Self {
                    system,
                    setup,
                    connected_at: Instant::now(),
                    quitting: Cell::new(false),
                    _thread_bound: PhantomData,
                })
            })();
            if result.is_err() {
                vr::VR_ShutdownInternal();
            }
            result
        }
    }

    fn system(&self) -> &vr::VR_IVRSystem_FnTable {
        // SAFETY: the table was acquired from this live context, owned by this thread.
        unsafe { &*self.system }
    }
    fn setup(&self) -> &vr::VR_IVRChaperoneSetup_FnTable {
        // SAFETY: same lifetime invariant as system().
        unsafe { &*self.setup }
    }

    pub fn reading(&self) -> Result<Reading> {
        ensure!(
            !self.quitting.get(),
            "SteamVR closed. Reconnect after restarting it."
        );
        let s = self.system();
        // SAFETY: ABI-typed buffers and function pointers come from matching SDK tables.
        unsafe {
            let mut event = vr::VREvent_t::default();
            while s.PollNextEvent.context("Missing event API")?(
                &mut event,
                size_of::<vr::VREvent_t>() as u32,
            ) {
                if event.eventType == vr::EVREventType_VREvent_Quit as u32 {
                    self.quitting.set(true);
                    if let Some(ack) = s.AcknowledgeQuit_Exiting {
                        ack();
                    }
                    bail!("SteamVR closed. Reconnect after restarting it.");
                }
            }
            let mut pose = vr::TrackedDevicePose_t::default();
            s.GetDeviceToAbsoluteTrackingPose
                .context("Missing pose API")?(
                vr::ETrackingUniverseOrigin_TrackingUniverseStanding,
                0.0,
                &mut pose,
                1,
            );
            let valid = pose.bPoseIsValid
                && pose.bDeviceIsConnected
                && pose.eTrackingResult == vr::ETrackingResult_TrackingResult_Running_OK;
            let mut model = [0i8; 256];
            let mut error = 0;
            s.GetStringTrackedDeviceProperty
                .context("Missing property API")?(
                0,
                vr::ETrackedDeviceProperty_Prop_ModelNumber_String,
                model.as_mut_ptr(),
                model.len() as u32,
                &mut error,
            );
            model[255] = 0;
            let headset = if error == 0 {
                CStr::from_ptr(model.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            } else {
                "VR headset".into()
            };
            let universe = s
                .GetUint64TrackedDeviceProperty
                .context("Missing universe API")?(
                0,
                vr::ETrackedDeviceProperty_Prop_CurrentUniverseId_Uint64,
                &mut error,
            );
            ensure!(
                error == 0 || !valid,
                "Cannot identify the active tracking universe"
            );
            Ok(Reading {
                pose: if valid {
                    pose_from_matrix(from_rows(pose.mDeviceToAbsoluteTracking.m)).ok()
                } else {
                    None
                },
                headset,
                universe,
                ready: valid && self.connected_at.elapsed() >= Duration::from_secs(3),
            })
        }
    }

    pub fn origin(&self) -> Result<DMat4> {
        // Get the live transform without altering the shared working copy.
        unsafe {
            let matrix = self
                .system()
                .GetRawZeroPoseToStandingAbsoluteTrackingPose
                .context("Missing origin API")?();
            let m = from_rows(matrix.m);
            ensure!(
                m.is_finite() && (m.determinant() - 1.0).abs() < 0.01,
                "SteamVR returned an invalid origin"
            );
            Ok(m.inverse())
        }
    }

    pub fn set_origin(&self, origin: DMat4) -> Result<()> {
        ensure!(
            origin.is_finite() && (origin.determinant() - 1.0).abs() < 0.01,
            "Invalid correction transform"
        );
        let setup = self.setup();
        // Refresh the shared working copy, preserving its current bounds and seated origin.
        unsafe {
            setup.RevertWorkingCopy.context("Missing calibration API")?();
            let mut current = vr::HmdMatrix34_t::default();
            ensure!(
                setup
                    .GetWorkingStandingZeroPoseToRawTrackingPose
                    .context("Missing calibration API")?(&mut current),
                "No usable standing calibration. Complete SteamVR room setup first."
            );
            let mut target = vr::HmdMatrix34_t { m: to_rows(origin) };
            setup
                .SetWorkingStandingZeroPoseToRawTrackingPose
                .context("Missing calibration API")?(&mut target);
            if !setup.CommitWorkingCopy.context("Missing commit API")?(
                vr::EChaperoneConfigFile_Live,
            ) {
                setup.RevertWorkingCopy.context("Missing calibration API")?();
                bail!("SteamVR could not commit the floor correction");
            }
        }
        // Runtime propagation is asynchronous. Confirm the actual transform, not just the return code.
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(30));
            if matrix_close(self.origin()?, origin) {
                return Ok(());
            }
        }
        bail!(
            "SteamVR accepted the change but read-back differs. Check for another active playspace tool before retrying."
        )
    }

    pub fn dashboard(&self) -> Result<String> {
        if self.dashboard_visible()? {
            crate::startup::toggle_dashboard_closed()?;
            for _ in 0..20 {
                thread::sleep(Duration::from_millis(50));
                if !self.dashboard_visible()? {
                    return Ok("Dashboard closed.".into());
                }
            }
            bail!("Dashboard close was requested, but SteamVR still reports it visible");
        }
        unsafe {
            let overlay = &*table::<vr::VR_IVROverlay_FnTable>(c"FnTable:IVROverlay_028")?;
            let show = overlay.ShowDashboard.context("Missing dashboard API")?;
            let find = overlay.FindOverlay.context("Missing overlay lookup API")?;
            // Desktop overlays may not exist until the dashboard has initialized.
            for attempt in 0..2 {
                let keys = (1..=8)
                    .map(|i| format!("system.desktop.{i}"))
                    .chain(["system.desktop".into(), "valve.steam.desktop".into()]);
                for name in keys {
                    let key = CString::new(name.as_str())?;
                    let mut handle = 0;
                    if find(key.as_ptr().cast_mut(), &mut handle) == 0 && handle != 0 {
                        show(key.as_ptr().cast_mut());
                        return Ok(format!("Desktop dashboard requested ({name})."));
                    }
                }
                if attempt == 0 {
                    show(c"".as_ptr().cast_mut());
                    thread::sleep(Duration::from_millis(200));
                }
            }
        }
        Ok(
            "Dashboard opened; desktop is still loading. Toggle closed and reopen to select it."
                .into(),
        )
    }

    /// Briefly request streaming and inspect headers only; never copy camera pixels.
    pub fn camera_status(&self) -> Result<serde_json::Value> {
        // SAFETY: matching SDK table, initialized output structures, owned context.
        unsafe {
            let camera =
                &*table::<vr::VR_IVRTrackedCamera_FnTable>(c"FnTable:IVRTrackedCamera_006")?;
            let mut present = false;
            let has_code = camera.HasCamera.context("Missing camera API")?(0, &mut present);
            let acquire = camera
                .AcquireVideoStreamingService
                .context("Missing stream API")?;
            let release = camera
                .ReleaseVideoStreamingService
                .context("Missing release API")?;
            let get_frame = camera
                .GetVideoStreamFrameBuffer
                .context("Missing frame API")?;
            let mut handle = 0;
            let acquire_code = if present && has_code == 0 {
                acquire(0, &mut handle)
            } else {
                -1
            };
            let mut sequences = std::collections::BTreeSet::new();
            let mut last_code = None;
            let mut dimensions = None;
            let mut release_code = None;
            if acquire_code == 0 {
                let deadline = Instant::now() + Duration::from_secs(3);
                while Instant::now() < deadline {
                    let mut header = vr::CameraVideoStreamFrameHeader_t::default();
                    let code = get_frame(
                        handle,
                        vr::EVRTrackedCameraFrameType_VRTrackedCameraFrameType_Distorted,
                        std::ptr::null_mut(),
                        0,
                        &mut header,
                        size_of::<vr::CameraVideoStreamFrameHeader_t>() as u32,
                    );
                    last_code = Some(code);
                    if code == 0 {
                        sequences.insert(header.nFrameSequence);
                        dimensions = Some([header.nWidth, header.nHeight]);
                    }
                    thread::sleep(Duration::from_millis(30));
                }
                release_code = Some(release(handle));
            }
            Ok(
                serde_json::json!({"has_camera":present,"has_camera_code":has_code,
                "acquire_code":acquire_code,"last_frame_code":last_code,"release_code":release_code,
                "dimensions":dimensions,"distinct_sequences":sequences.len(),
                "frames_advancing":sequences.len()>1,"pixels_captured":false}),
            )
        }
    }

    pub fn passthrough(&self) -> Result<String> {
        unsafe {
            let settings = &*table::<vr::VR_IVRSettings_FnTable>(c"FnTable:IVRSettings_003")?;
            let mut error = 0;
            let enabled = settings.GetBool.context("Missing settings API")?(
                c"camera".as_ptr().cast_mut(),
                c"enableCamera".as_ptr().cast_mut(),
                &mut error,
            );
            ensure!(
                error == 0 && enabled,
                "Enable Camera in SteamVR Settings > Camera, enable Room View, then restart SteamVR if requested."
            );
            let camera =
                &*table::<vr::VR_IVRTrackedCamera_FnTable>(c"FnTable:IVRTrackedCamera_006")?;
            let mut has_camera = false;
            let code = camera.HasCamera.context("Missing camera API")?(0, &mut has_camera);
            ensure!(
                code == 0 && has_camera,
                "No headset camera exposed to SteamVR. Enable Pimax passthrough in the Dream Air driver settings and restart SteamVR."
            );
        }
        crate::startup::toggle_passthrough()?;
        Ok("Camera passthrough toggle requested · F16 toggles back.".into())
    }

    pub fn dashboard_visible(&self) -> Result<bool> {
        unsafe {
            let overlay = &*table::<vr::VR_IVROverlay_FnTable>(c"FnTable:IVROverlay_028")?;
            Ok(overlay
                .IsDashboardVisible
                .context("Missing dashboard visibility API")?(
            ))
        }
    }

    pub fn headset_bridge_request(&self, request: &CStr) -> Result<String> {
        // SAFETY: IVRDebug_001 matches this table; HMD index 0 and bounded response buffer.
        unsafe {
            let debug = &*table::<vr::VR_IVRDebug_FnTable>(c"FnTable:IVRDebug_001")?;
            let mut response = [0i8; 256];
            debug
                .DriverDebugRequest
                .context("Missing driver debug API")?(
                0,
                request.as_ptr().cast_mut(),
                response.as_mut_ptr(),
                response.len() as u32,
            );
            response[255] = 0;
            Ok(CStr::from_ptr(response.as_ptr())
                .to_string_lossy()
                .into_owned())
        }
    }

    pub fn headset_gaze_click(&self) -> Result<()> {
        ensure!(
            self.dashboard_visible()?,
            "Open the SteamVR dashboard before using F14"
        );
        let capabilities = self.headset_bridge_request(c"rigcompanion:capabilities:v1")?;
        ensure!(
            capabilities == "ok:rigcompanion:gaze-click:v1",
            "Modified sboys3 headset driver is not active (reply: {capabilities:?})"
        );
        let response = self.headset_bridge_request(c"rigcompanion:gaze-click:v1")?;
        ensure!(
            response == "ok:queued:120ms",
            "Headset click rejected: {response}"
        );
        Ok(())
    }

    fn headset_gaze_hold_request(&self, request: &CStr, expected: &str) -> Result<()> {
        let response = self.headset_bridge_request(request)?;
        ensure!(
            response == expected,
            "Headset gaze hold rejected: {response}"
        );
        Ok(())
    }

    pub fn headset_gaze_down(&self) -> Result<()> {
        ensure!(
            self.dashboard_visible()?,
            "Open the SteamVR dashboard before using F14"
        );
        self.headset_gaze_hold_request(c"rigcompanion:gaze-down:v2", "ok:down")
    }

    pub fn headset_gaze_refresh(&self) -> Result<()> {
        self.headset_gaze_hold_request(c"rigcompanion:gaze-refresh:v2", "ok:refreshed")
    }

    pub fn headset_gaze_up(&self) -> Result<()> {
        self.headset_gaze_hold_request(c"rigcompanion:gaze-up:v2", "ok:up")
    }

    pub fn headset_joystick(&self, direction: i8) -> Result<()> {
        ensure!((-1..=1).contains(&direction), "Invalid joystick direction");
        if direction != 0 {
            ensure!(
                self.dashboard_visible()?,
                "Open the SteamVR dashboard before using F17 or F18"
            );
        }
        let capability = self.headset_bridge_request(c"rigcompanion:capabilities:joystick:v1")?;
        ensure!(
            capability == "ok:rigcompanion:joystick:v1",
            "Headset driver does not support the joystick bridge (reply: {capability:?})"
        );
        let (request, expected) = match direction {
            1 => (c"rigcompanion:joystick-up:v1", "ok:up"),
            -1 => (c"rigcompanion:joystick-down:v1", "ok:down"),
            _ => (c"rigcompanion:joystick-neutral:v1", "ok:neutral"),
        };
        let response = self.headset_bridge_request(request)?;
        ensure!(
            response == expected,
            "Headset joystick rejected: {response}"
        );
        Ok(())
    }

    pub fn headset_gaze_pointer_toggle(&self) -> Result<bool> {
        let capability =
            self.headset_bridge_request(c"rigcompanion:capabilities:gaze-pointer:v1")?;
        ensure!(
            capability == "ok:rigcompanion:gaze-pointer:v1",
            "Headset driver does not support the eye pointer (reply: {capability:?})"
        );
        let response = self.headset_bridge_request(c"rigcompanion:gaze-pointer-toggle:v1")?;
        match response.as_str() {
            "ok:on" => Ok(true),
            "ok:off" => Ok(false),
            _ => anyhow::bail!("Headset eye pointer toggle rejected: {response}"),
        }
    }

    pub fn gamepad_enabled(&self) -> Result<bool> {
        // SAFETY: exact SDK settings table and writable error output.
        unsafe {
            let settings = &*table::<vr::VR_IVRSettings_FnTable>(c"FnTable:IVRSettings_003")?;
            let mut error = 0;
            let enabled = settings.GetBool.context("Missing settings API")?(
                c"driver_gamepad".as_ptr().cast_mut(),
                c"enable".as_ptr().cast_mut(),
                &mut error,
            );
            ensure!(
                error == 0,
                "Cannot read gamepad driver setting (error {error})"
            );
            Ok(enabled)
        }
    }

    pub fn enable_gamepad(&self) -> Result<()> {
        unsafe {
            let settings = &*table::<vr::VR_IVRSettings_FnTable>(c"FnTable:IVRSettings_003")?;
            let mut error = 0;
            settings.SetBool.context("Missing settings API")?(
                c"driver_gamepad".as_ptr().cast_mut(),
                c"enable".as_ptr().cast_mut(),
                true,
                &mut error,
            );
            ensure!(
                error == 0 && self.gamepad_enabled()?,
                "Cannot enable gamepad driver (error {error})"
            );
        }
        Ok(())
    }

    pub fn gamepad_connected(&self) -> Result<bool> {
        unsafe {
            let system = self.system();
            for index in 0..vr::k_unMaxTrackedDeviceCount {
                if !system
                    .IsTrackedDeviceConnected
                    .context("Missing device API")?(index)
                {
                    continue;
                }
                let mut name = [0i8; 128];
                let mut error = 0;
                system
                    .GetStringTrackedDeviceProperty
                    .context("Missing property API")?(
                    index,
                    vr::ETrackedDeviceProperty_Prop_TrackingSystemName_String,
                    name.as_mut_ptr(),
                    name.len() as u32,
                    &mut error,
                );
                name[127] = 0;
                if error == 0 && CStr::from_ptr(name.as_ptr()).to_bytes() == b"gamepad" {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

// SAFETY: callers request the exact SDK table matching T, after successful initialization.
unsafe fn table<T>(name: &CStr) -> Result<*const T> {
    let mut error = 0;
    let ptr = unsafe { vr::VR_GetGenericInterface(name.as_ptr(), &mut error) };
    ensure!(
        error == 0 && ptr != 0,
        "SteamVR interface {} unavailable (error {error}); update SteamVR",
        name.to_string_lossy()
    );
    Ok(ptr as *const T)
}

impl Drop for SteamVr {
    fn drop(&mut self) {
        // SAFETY: all table references and calls end before this sole owner is dropped.
        unsafe { vr::VR_ShutdownInternal() }
        CONTEXT_ACTIVE.store(false, Ordering::SeqCst);
    }
}
