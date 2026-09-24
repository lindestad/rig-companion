use crate::{
    calibration::{
        self, Pose, corrected_origin, height_correction, matrix_close, reference_correction,
    },
    profile::Profile,
    steamvr::{Reading, SteamVr},
};
use anyhow::{Context, Result, ensure};
use glam::{DMat4, DVec3};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

#[derive(Debug, Clone)]
pub enum Command {
    Disconnect,
    Connect,
    SaveHeight(f64),
    Capture,
    RestoreHeight,
    RestoreReference,
    Recenter98,
    GazeClick,
    GazeDown,
    GazeRefresh,
    GazeUp,
    Joystick(i8),
    GazePointerToggle,
    Nudge(f64),
    Undo,
    Dashboard,
    Passthrough,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub connected: bool,
    pub ready: bool,
    pub demo: bool,
    pub headset: String,
    pub pose: Option<Pose>,
    pub profile: Profile,
    pub can_undo: bool,
    pub busy: bool,
    pub message: String,
    pub error: bool,
    pub revision: u64,
}

enum Backend {
    Live(SteamVr),
    Demo { origin: DMat4, raw_pose: Box<DMat4> },
}

impl Backend {
    fn reading(&self) -> Result<Reading> {
        match self {
            Self::Live(vr) => vr.reading(),
            Self::Demo { origin, raw_pose } => Ok(Reading {
                pose: Some(calibration::pose_from_matrix(
                    origin.inverse() * **raw_pose,
                )?),
                headset: "Dream Air · simulated".into(),
                universe: 30,
                ready: true,
            }),
        }
    }
    fn origin(&self) -> Result<DMat4> {
        match self {
            Self::Live(vr) => vr.origin(),
            Self::Demo { origin, .. } => Ok(*origin),
        }
    }
    fn set_origin(&mut self, matrix: DMat4) -> Result<()> {
        match self {
            Self::Live(vr) => vr.set_origin(matrix),
            Self::Demo { origin, .. } => {
                *origin = matrix;
                Ok(())
            }
        }
    }
}

struct Undo {
    before: DMat4,
    after: DMat4,
    universe: u64,
}

pub struct Engine {
    backend: Option<Backend>,
    undo: Option<Undo>,
    path: PathBuf,
    pub state: Snapshot,
}

impl Engine {
    pub fn new(path: PathBuf, demo: bool) -> Result<Self> {
        let profile = Profile::load(&path)?;
        Ok(Self {
            backend: None,
            undo: None,
            path,
            state: Snapshot {
                connected: false,
                ready: false,
                demo,
                headset: "No headset connected".into(),
                pose: None,
                profile,
                can_undo: false,
                busy: false,
                message: "Connect to SteamVR to get started.".into(),
                error: false,
                revision: 0,
            },
        })
    }

    pub fn refresh(&mut self) {
        if let Some(backend) = &self.backend {
            match backend.reading() {
                Ok(reading) => {
                    self.state.connected = true;
                    self.state.ready = reading.ready && reading.pose.is_some();
                    self.state.headset = reading.headset;
                    self.state.pose = reading.pose;
                    if let Some(undo) = &self.undo
                        && (reading.universe != undo.universe
                            || backend
                                .origin()
                                .is_ok_and(|origin| !matrix_close(origin, undo.after)))
                    {
                        self.undo = None;
                    }
                }
                Err(e) => {
                    self.backend = None;
                    self.undo = None;
                    self.state.connected = false;
                    self.state.ready = false;
                    self.state.pose = None;
                    self.result(Err(e));
                }
            }
        }
        self.state.can_undo = self.undo.is_some();
    }

    pub fn run(&mut self, command: Command) -> Result<String> {
        let result = self.execute(command);
        let returned = match &result {
            Ok(m) => Ok(m.clone()),
            Err(e) => Err(anyhow::anyhow!("{e:#}")),
        };
        self.result(result);
        self.refresh();
        returned
    }

    fn result(&mut self, result: Result<String>) {
        self.state.error = result.is_err();
        self.state.message = match result {
            Ok(m) => m,
            Err(e) => format!("{e:#}"),
        };
        self.state.revision += 1;
    }

    fn steady_pose(&self) -> Result<(Pose, u64, DMat4)> {
        let backend = self.backend.as_ref().context("Connect to SteamVR first")?;
        let first = backend.reading()?;
        ensure!(
            first.ready,
            "Waiting for positional tracking to settle. Sit normally and try again."
        );
        let start = first.pose.context("Headset position is unavailable")?;
        let origin = backend.origin()?;
        let mut sum = DVec3::ZERO;
        let mut sin = 0.0;
        let mut cos = 0.0;
        for _ in 0..8 {
            if !self.state.demo {
                thread::sleep(Duration::from_millis(40));
            }
            let reading = backend.reading()?;
            ensure!(
                reading.ready && reading.universe == first.universe,
                "Tracking changed during capture; try again"
            );
            let p = reading
                .pose
                .context("Position unavailable during capture")?;
            ensure!(
                (DVec3::from(p.position) - DVec3::from(start.position)).length() < 0.025,
                "Hold your head still briefly, then try again"
            );
            let yaw_delta = (p.yaw - start.yaw).sin().atan2((p.yaw - start.yaw).cos());
            ensure!(
                yaw_delta.abs() < 0.06,
                "Look forward and hold still briefly"
            );
            sum += DVec3::from(p.position);
            sin += p.yaw.sin();
            cos += p.yaw.cos();
        }
        ensure!(
            matrix_close(origin, backend.origin()?),
            "SteamVR recentered during capture; try again"
        );
        Ok((
            Pose {
                position: (sum / 8.0).to_array(),
                yaw: sin.atan2(cos),
            },
            first.universe,
            origin,
        ))
    }

    fn execute(&mut self, command: Command) -> Result<String> {
        match command {
            Command::Passthrough => {
                match self.backend.as_ref().context("Connect to SteamVR first")? {
                    Backend::Live(vr) => vr.passthrough(),
                    Backend::Demo { .. } => Ok("Camera toggle simulated.".into()),
                }
            }
            Command::Disconnect => {
                self.backend = None;
                self.state.connected = false;
                self.state.ready = false;
                Ok("Disconnected.".into())
            }
            Command::Connect => {
                // End the old context before opening a replacement.
                self.backend = None;
                self.undo = None;
                self.state.connected = false;
                self.state.ready = false;
                self.state.pose = None;
                self.backend = Some(if self.state.demo {
                    Backend::Demo {
                        origin: DMat4::IDENTITY,
                        raw_pose: Box::new(
                            DMat4::from_translation(DVec3::new(0.25, -0.3, 0.1))
                                * DMat4::from_rotation_y(0.3),
                        ),
                    }
                } else {
                    Backend::Live(SteamVr::connect()?)
                });
                Ok(if self.state.demo {
                    "Demo connected. All tracking changes are simulated."
                } else {
                    "Connected. Allow tracking to settle, then restore your reference."
                }
                .into())
            }
            Command::SaveHeight(height) => {
                calibration::validate_height(height)?;
                let mut profile = self.state.profile.clone();
                profile.height_m = Some(height);
                if let Some(p) = &mut profile.reference {
                    p.position[1] = height;
                }
                profile.save(&self.path)?;
                self.state.profile = profile;
                Ok(format!(
                    "Saved seated height: {:.1} cm. Restore when you're seated.",
                    height * 100.0
                ))
            }
            Command::Capture => {
                let (pose, _, _) = self.steady_pose()?;
                calibration::validate_height(pose.position[1]).context(
                    "Adjust the floor first; current height cannot be used as a reference",
                )?;
                let profile = Profile {
                    version: 1,
                    height_m: Some(pose.position[1]),
                    reference: Some(pose),
                };
                profile.save(&self.path)?;
                self.state.profile = profile;
                Ok("Saved your current height, position and forward direction.".into())
            }
            Command::RestoreHeight
            | Command::RestoreReference
            | Command::Recenter98
            | Command::Nudge(_) => {
                let (pose, universe, before) = self.steady_pose()?;
                let (correction, label) = match command {
                    Command::Recenter98 => {
                        let mut target = self.state.profile.reference.unwrap_or(Pose {
                            position: [0.0, 0.98, 0.0],
                            yaw: 0.0,
                        });
                        target.position[1] = 0.98;
                        (reference_correction(pose, target)?, "Recentered at 98 cm")
                    }
                    Command::RestoreHeight => (
                        height_correction(
                            pose,
                            self.state
                                .profile
                                .height_m
                                .context("Save your seated height first")?,
                        )?,
                        "Height restored",
                    ),
                    Command::RestoreReference => (
                        reference_correction(
                            pose,
                            self.state
                                .profile
                                .reference
                                .context("Capture a full reference first")?,
                        )?,
                        "Position and heading restored",
                    ),
                    Command::Nudge(delta) => {
                        ensure!(
                            delta.is_finite() && delta.abs() <= 0.1,
                            "Height step must be at most 10 cm"
                        );
                        // A nudge must work even when the starting viewpoint is below the floor.
                        (DMat4::from_translation(DVec3::Y * delta), "Height adjusted")
                    }
                    _ => unreachable!(),
                };
                let after = corrected_origin(before, correction);
                let backend = self.backend.as_mut().context("Disconnected")?;
                ensure!(
                    matrix_close(before, backend.origin()?),
                    "Origin changed before applying correction; try again"
                );
                if matrix_close(before, after) {
                    return Ok("Already at your saved reference.".into());
                }
                self.undo = Some(Undo {
                    before,
                    after,
                    universe,
                });
                backend.set_origin(after)?;
                let reading = backend.reading()?;
                let height = reading
                    .pose
                    .map(|p| format!(" · {:.1} cm", p.position[1] * 100.0))
                    .unwrap_or_default();
                Ok(format!(
                    "{label}{height}. Your saved reference is unchanged."
                ))
            }
            Command::Undo => {
                let undo = self
                    .undo
                    .as_ref()
                    .context("No correction to undo in this session")?;
                let backend = self.backend.as_mut().context("Disconnected")?;
                ensure!(
                    backend.reading()?.universe == undo.universe
                        && matrix_close(backend.origin()?, undo.after),
                    "The tracking origin changed elsewhere. Undo was not applied."
                );
                backend.set_origin(undo.before)?;
                self.undo = None;
                Ok("Previous floor calibration restored.".into())
            }
            Command::Dashboard => {
                match self.backend.as_ref().context("Connect to SteamVR first")? {
                    Backend::Live(vr) => vr.dashboard(),
                    Backend::Demo { .. } => Ok("Demo: dashboard request simulated.".into()),
                }
            }
            Command::GazeClick => {
                if self.state.demo {
                    return Ok("Demo: VR gaze click simulated.".into());
                }
                let Backend::Live(vr) =
                    self.backend.as_ref().context("Connect to SteamVR first")?
                else {
                    unreachable!()
                };
                vr.headset_gaze_click()?;
                Ok("Headset driver accepted a 120 ms system-button pulse.".into())
            }
            Command::GazeDown | Command::GazeRefresh | Command::GazeUp => {
                if self.state.demo {
                    return Ok("Demo: VR gaze button state simulated.".into());
                }
                let Backend::Live(vr) =
                    self.backend.as_ref().context("Connect to SteamVR first")?
                else {
                    unreachable!()
                };
                match command {
                    Command::GazeDown => {
                        vr.headset_gaze_down()?;
                        Ok("VR gaze button held while F14 is down.".into())
                    }
                    Command::GazeRefresh => {
                        vr.headset_gaze_refresh()?;
                        Ok("VR gaze hold refreshed.".into())
                    }
                    Command::GazeUp => {
                        vr.headset_gaze_up()?;
                        Ok("VR gaze button released.".into())
                    }
                    _ => unreachable!(),
                }
            }
            Command::Joystick(direction) => {
                if self.state.demo {
                    return Ok(format!("Demo: headset joystick direction {direction}."));
                }
                if direction == 0 && self.backend.is_none() {
                    return Ok("Headset joystick released.".into());
                }
                let Backend::Live(vr) =
                    self.backend.as_ref().context("Connect to SteamVR first")?
                else {
                    unreachable!()
                };
                vr.headset_joystick(direction)?;
                Ok(format!("Headset joystick direction {direction}."))
            }
            Command::GazePointerToggle => {
                if self.state.demo {
                    return Ok("Demo: eye pointer toggle simulated.".into());
                }
                let Backend::Live(vr) =
                    self.backend.as_ref().context("Connect to SteamVR first")?
                else {
                    unreachable!()
                };
                let enabled = vr.headset_gaze_pointer_toggle()?;
                Ok(if enabled {
                    "Eye pointer on. Invalid gaze falls back to head aim."
                } else {
                    "Eye pointer off; head aim restored."
                }
                .into())
            }
        }
    }
}

pub struct Worker {
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    sender: Option<mpsc::Sender<Command>>,
    snapshot: Arc<Mutex<Snapshot>>,
    join: Option<thread::JoinHandle<()>>,
}

impl Worker {
    pub fn spawn(path: PathBuf, demo: bool) -> Result<Self> {
        Self::spawn_with_launch(path, demo, false)
    }

    pub fn spawn_with_launch(path: PathBuf, demo: bool, launch: bool) -> Result<Self> {
        // Only load a profile here; the OpenVR context is created on the worker thread.
        let initial = Engine::new(path.clone(), demo)?.state;
        let snapshot = Arc::new(Mutex::new(initial.clone()));
        let shared = snapshot.clone();
        let (sender, receiver) = mpsc::channel();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_cancelled = cancelled.clone();
        let join = thread::Builder::new()
            .name("steamvr".into())
            .spawn(move || {
                let mut engine = Engine {
                    backend: None,
                    undo: None,
                    path,
                    state: initial,
                };
                let mut auto_connect = true;
                if launch
                    && !demo
                    && let Err(error) = crate::startup::run(&thread_cancelled, |message| {
                        engine.state.message = message.into();
                        *shared.lock().unwrap() = engine.state.clone();
                    })
                {
                    engine.result(Err(error));
                    auto_connect = false;
                }
                if auto_connect && !thread_cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = engine.run(Command::Connect);
                }
                let mut last_connect_attempt = std::time::Instant::now();
                loop {
                    *shared.lock().unwrap() = engine.state.clone();
                    match receiver.recv_timeout(Duration::from_millis(250)) {
                        Ok(command) => {
                            if matches!(command, Command::Disconnect) {
                                auto_connect = false;
                            }
                            if matches!(command, Command::Connect) {
                                auto_connect = true;
                            }
                            if matches!(command, Command::GazeRefresh | Command::Joystick(_)) {
                                if let Err(error) = engine.execute(command) {
                                    engine.result(Err(error));
                                }
                            } else {
                                shared.lock().unwrap().busy = true;
                                let _ = engine.run(command);
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            engine.refresh();
                            if auto_connect
                                && !thread_cancelled.load(std::sync::atomic::Ordering::Relaxed)
                                && !engine.state.connected
                                && last_connect_attempt.elapsed() >= Duration::from_secs(2)
                            {
                                // Background initialization never launches SteamVR itself.
                                last_connect_attempt = std::time::Instant::now();
                                let _ = engine.run(Command::Connect);
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok(Self {
            cancelled,
            sender: Some(sender),
            snapshot,
            join: Some(join),
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().unwrap().clone()
    }
    pub fn begin_shutdown(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.send(Command::Disconnect);
    }
    pub fn send(&self, command: Command) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(command);
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.sender.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn engine() -> (tempfile::TempDir, Engine) {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().join("profile.json"), true).unwrap();
        engine.run(Command::Connect).unwrap();
        (dir, engine)
    }
    #[test]
    fn restore_and_undo_use_same_command_path_as_ui() {
        let (_dir, mut e) = engine();
        assert_eq!(e.state.profile.height_m, Some(0.98));
        e.run(Command::SaveHeight(1.15)).unwrap();
        e.run(Command::RestoreHeight).unwrap();
        assert!((e.state.pose.unwrap().position[1] - 1.15).abs() < 1e-8);
        e.run(Command::RestoreHeight).unwrap();
        e.run(Command::Undo).unwrap();
        assert!((e.state.pose.unwrap().position[1] + 0.3).abs() < 1e-8);
    }
    #[test]
    fn new_profile_restores_the_users_98_cm_reference() {
        let (_dir, mut e) = engine();
        e.run(Command::RestoreHeight).unwrap();
        assert!((e.state.pose.unwrap().position[1] - 0.98).abs() < 1e-8);
    }
    #[test]
    fn recenter_98_restores_position_heading_and_can_undo() {
        let (_dir, mut e) = engine();
        let original = e.state.pose.unwrap();
        e.run(Command::Recenter98).unwrap();
        let p = e.state.pose.unwrap();
        assert!(DVec3::from(p.position).distance(DVec3::new(0.0, 0.98, 0.0)) < 1e-8);
        assert!(p.yaw.abs() < 1e-8);
        e.run(Command::Recenter98).unwrap();
        e.run(Command::Undo).unwrap();
        assert!(
            DVec3::from(e.state.pose.unwrap().position).distance(original.position.into()) < 1e-8
        );
    }
    #[test]
    fn recenter_98_uses_saved_horizontal_reference_without_overwriting_it() {
        let (_dir, mut e) = engine();
        let reference = Pose {
            position: [0.5, 1.15, -0.8],
            yaw: 0.7,
        };
        e.state.profile.reference = Some(reference);
        e.run(Command::Recenter98).unwrap();
        let p = e.state.pose.unwrap();
        assert!(DVec3::from(p.position).distance(DVec3::new(0.5, 0.98, -0.8)) < 1e-8);
        assert!((p.yaw - 0.7).abs() < 1e-8);
        assert_eq!(
            e.state.profile.reference.unwrap().position,
            reference.position
        );
    }
    #[test]
    fn nudge_does_not_rewrite_saved_reference() {
        let (_dir, mut e) = engine();
        e.run(Command::SaveHeight(1.15)).unwrap();
        e.run(Command::Nudge(0.01)).unwrap();
        assert!((e.state.pose.unwrap().position[1] + 0.29).abs() < 1e-8);
        assert_eq!(Profile::load(&e.path).unwrap().height_m, Some(1.15));
    }
    #[test]
    fn external_origin_change_prevents_undo() {
        let (_dir, mut e) = engine();
        e.run(Command::Nudge(0.01)).unwrap();
        e.backend
            .as_mut()
            .unwrap()
            .set_origin(DMat4::from_translation(DVec3::X))
            .unwrap();
        assert!(e.run(Command::Undo).is_err());
    }
    #[test]
    fn invalid_capture_preserves_profile() {
        let (_dir, mut e) = engine();
        e.run(Command::SaveHeight(1.15)).unwrap();
        assert!(e.run(Command::Capture).is_err());
        assert_eq!(Profile::load(&e.path).unwrap().height_m, Some(1.15));
    }
}
