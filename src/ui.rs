use crate::hotkeys::{HotkeyEvent, Hotkeys};
use iced::{
    Color, Element, Fill, Font, Length, Point, Rectangle, Renderer, Size, Subscription, Task,
    Theme, alignment, border, mouse,
    widget::{
        self, Space, button, canvas, checkbox, column, container, row, scrollable, text, text_input,
    },
};
use rig_companion::{
    calibration::validate_height,
    service::{Command, Snapshot, Worker},
};
use std::os::windows::process::CommandExt;
use std::time::{Duration, Instant};

const BG: Color = Color::from_rgb8(14, 14, 16);
const PANEL: Color = Color::from_rgb8(25, 25, 28);
const INNER: Color = Color::from_rgb8(17, 17, 19);
const LINE: Color = Color::from_rgb8(53, 53, 58);
const WHITE: Color = Color::from_rgb8(238, 238, 240);
const MUTED: Color = Color::from_rgb8(164, 164, 173);
const ACCENT: Color = Color::from_rgb8(188, 168, 255);
const AMBER: Color = Color::from_rgb8(255, 167, 97);
const RED: Color = AMBER;

pub struct App {
    worker: Worker,
    state: Snapshot,
    height: String,
    step: f64,
    countdown_enabled: bool,
    pending: Option<(Instant, Command)>,
    hotkeys: Option<Hotkeys>,
    gaze_held: bool,
    gaze_refreshed: Instant,
    joystick_direction: i8,
    joystick_refreshed: Instant,
    local_message: Option<String>,
    eye_page: bool,
    settings_page: bool,
    wind_page: bool,
    wind: crate::wind_ui::State,
    settings: crate::settings_ui::State,
    quitting: bool,
    quit_queued: bool,
    quit_error: Option<String>,
    logo: widget::image::Handle,
    game_icons: Vec<widget::image::Handle>,
    launch_busy: bool,
    steamvr_starting: bool,
    launch_status: String,
    launch_error: bool,
    launch_cooldown: Option<Instant>,
    launch_cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    launch_progress: Option<std::sync::mpsc::Receiver<String>>,
    quit_signal: Option<rig_companion::ipc::QuitSignal>,
    focused: bool,
    keep_eyes_live: bool,
    eye_pending: bool,
    eye_reading: Option<rig_companion::eyes::Reading>,
    eye_error: Option<String>,
    calibration_check_pending: bool,
    calibration_status: String,
    pointer_calibration_running: bool,
    pointer_calibration_status: String,
    _lock: std::fs::File,
}

#[derive(Debug, Clone)]
pub enum Message {
    StartSteamVr,
    SteamVrStarted(Result<(), String>),
    Launch(rig_companion::game_launch::Game),
    LaunchDone(Result<String, String>),
    Quit,
    QuitDone(Result<(), String>),
    Passthrough,
    EyePage(bool),
    SettingsPage,
    WindPage,
    Wind(crate::wind_ui::Message),
    DriverSettings(crate::settings_ui::Message),
    Focused(bool),
    KeepEyesLive(bool),
    EyeTick,
    EyeRead(Result<rig_companion::eyes::Reading, String>),
    CheckEyeCalibration,
    LaunchEyePointerCalibration,
    EyePointerCalibrationDone(Result<String, String>),
    EyeCalibrationResult(String),
    Tick,
    Connect,
    Height(String),
    Save,
    Capture,
    Restore,
    RestoreFull,
    Nudge(f64),
    Step(f64),
    Undo,
    Dashboard,
    GazeClick,
    GazePointerToggle,
    Delay(bool),
    Shortcuts(bool),
    Cancel,
}

impl App {
    pub fn new(worker: Worker, lock: std::fs::File, wind_page: bool) -> Self {
        let state = worker.snapshot();
        let height = state
            .profile
            .height_m
            .map(|h| format!("{:.1}", h * 100.0))
            .unwrap_or_else(|| "98.0".into());
        let (hotkeys, local_message) = match Hotkeys::start() {
            Ok(h) => (Some(h), None),
            Err(e) => (None, Some(e.to_string())),
        };
        Self {
            wind: crate::wind_ui::State::new(state.demo),
            wind_page,
            quit_signal: rig_companion::ipc::QuitSignal::create(state.demo).ok(),
            worker,
            state,
            height,
            step: 0.01,
            countdown_enabled: true,
            pending: None,
            hotkeys,
            gaze_held: false,
            gaze_refreshed: Instant::now(),
            joystick_direction: 0,
            joystick_refreshed: Instant::now(),
            local_message,
            eye_page: false,
            settings_page: false,
            settings: crate::settings_ui::State::default(),
            quitting: false,
            quit_queued: false,
            quit_error: None,
            game_icons: [
                include_bytes!("../assets/game-icons/iracing.rgba").as_slice(),
                include_bytes!("../assets/game-icons/content_manager.rgba").as_slice(),
                include_bytes!("../assets/game-icons/rally.rgba").as_slice(),
                include_bytes!("../assets/game-icons/evo.rgba").as_slice(),
                include_bytes!("../assets/game-icons/lmu.rgba").as_slice(),
            ].into_iter().map(|bytes| widget::image::Handle::from_rgba(64,64,bytes)).collect(),
            launch_busy: false,
            steamvr_starting: false,
            launch_status: "SimPro + SimHub · MAIRA for iRacing".into(),
            launch_error: false,
            launch_cooldown: None,
            launch_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            launch_progress: None,
            logo: widget::image::Handle::from_rgba(64,64,include_bytes!("../assets/icon.rgba").as_slice()),
            focused: true,
            keep_eyes_live: false,
            eye_pending: false,
            eye_reading: None,
            eye_error: None,
            calibration_check_pending: false,
            calibration_status: "Check whether Pimax exposes a tracker with 3D calibration and a retrievable backup. This check does not change your calibration.".into(),
            pointer_calibration_running: false,
            pointer_calibration_status: "No pointer alignment is running.".into(),
            _lock: lock,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StartSteamVr => {
                if self.state.demo
                    || self.state.connected
                    || self.wind.steamvr_running()
                    || self.launch_busy
                    || self.quitting
                    || self.quit_queued
                {
                    return Task::none();
                }
                self.launch_busy = true;
                self.steamvr_starting = true;
                self.launch_error = false;
                self.local_message = None;
                self.launch_status = "Starting SteamVR…".into();
                let (sender, receiver) = std::sync::mpsc::channel();
                self.launch_progress = Some(receiver);
                self.launch_cancelled =
                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let cancelled = self.launch_cancelled.clone();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            rig_companion::startup::run(&cancelled, |status| {
                                let _ = sender.send(status.into());
                            })
                        })
                        .await
                        .map_err(|e| e.to_string())?
                        .map_err(|e| format!("{e:#}"))
                    },
                    Message::SteamVrStarted,
                );
            }
            Message::SteamVrStarted(result) => {
                self.launch_busy = false;
                self.steamvr_starting = false;
                self.launch_progress = None;
                self.launch_error = result.is_err();
                match result {
                    Ok(()) => {
                        self.launch_status = "SteamVR started · connecting…".into();
                        if !self.quitting && !self.quit_queued {
                            self.worker.send(Command::Connect);
                        }
                    }
                    Err(error) => {
                        self.launch_status = error.clone();
                        self.local_message = Some(error);
                    }
                }
            }
            Message::WindPage => {
                self.wind_page = true;
                self.eye_page = false;
                self.settings_page = false;
            }
            Message::Wind(message) => return self.wind.update(message).map(Message::Wind),
            Message::Launch(game) => {
                if self.launch_busy
                    || self.quitting
                    || self.quit_queued
                    || self
                        .launch_cooldown
                        .is_some_and(|deadline| Instant::now() < deadline)
                {
                    return Task::none();
                }
                if self.state.demo {
                    self.launch_status = format!("{} · demo, no apps launched", game.label());
                    return Task::none();
                }
                self.launch_busy = true;
                self.launch_error = false;
                self.launch_status = format!("Preparing {}…", game.label());
                let (sender, receiver) = std::sync::mpsc::channel();
                self.launch_progress = Some(receiver);
                self.launch_cancelled =
                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let cancelled = self.launch_cancelled.clone();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            rig_companion::game_launch::launch(game, &cancelled, |status| {
                                let _ = sender.send(status);
                            })
                        })
                        .await
                        .map_err(|e| e.to_string())?
                        .map_err(|e| format!("{e:#}"))
                    },
                    Message::LaunchDone,
                );
            }
            Message::LaunchDone(result) => {
                self.launch_busy = false;
                self.launch_progress = None;
                self.launch_cooldown = Some(Instant::now() + Duration::from_secs(5));
                self.launch_error = result.is_err();
                self.launch_status = result.unwrap_or_else(|error| error);
            }
            Message::Quit => {
                self.launch_cancelled
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                if self.settings.busy() {
                    self.quit_queued = true;
                    self.quit_error = Some("Finishing settings operation before quitting…".into());
                    return Task::none();
                }
                self.quit_queued = false;
                if !self.quitting {
                    self.quitting = true;
                    self.quit_error = None;
                    self.pending = None;
                    self.release_gaze();
                    self.release_joystick();
                    self.hotkeys = None;
                    self.worker.begin_shutdown();
                    self.wind.suspend();
                    if self.state.demo {
                        return iced::exit();
                    }
                    return Task::perform(
                        async {
                            tokio::task::spawn_blocking(rig_companion::startup::quit_steamvr)
                                .await
                                .map_err(|e| e.to_string())?
                                .map_err(|e| format!("{e:#}"))
                        },
                        Message::QuitDone,
                    );
                }
            }
            Message::QuitDone(result) => match result {
                Ok(()) => return iced::exit(),
                Err(e) => {
                    self.quitting = false;
                    self.quit_error = Some(e);
                }
            },
            Message::SettingsPage => {
                self.wind_page = false;
                self.eye_page = false;
                self.settings_page = true;
                if !self.settings.loaded() {
                    return self
                        .settings
                        .update(crate::settings_ui::Message::Load, self.state.demo)
                        .map(Message::DriverSettings);
                }
            }
            Message::DriverSettings(message) => {
                return self
                    .settings
                    .update(message, self.state.demo)
                    .map(Message::DriverSettings);
            }
            Message::CheckEyeCalibration => {
                if !self.calibration_check_pending && !self.state.demo {
                    self.calibration_check_pending = true;
                    return Task::perform(
                        async {
                            match tokio::task::spawn_blocking(
                                rig_companion::eye_calibration::inspect,
                            )
                            .await
                            {
                                Ok(Ok(result)) => rig_companion::eye_calibration::summary(&result),
                                Ok(Err(e)) => format!("{e:#}"),
                                Err(e) => e.to_string(),
                            }
                        },
                        Message::EyeCalibrationResult,
                    );
                }
            }
            Message::LaunchEyePointerCalibration => {
                if self.pointer_calibration_running {
                    return Task::none();
                }
                self.pointer_calibration_running = true;
                self.pointer_calibration_status =
                    "Closing the dashboard. First target appears in five seconds; look at each small center dot. Press Escape to cancel.".into();
                return Task::perform(
                    async {
                        match tokio::task::spawn_blocking(|| {
                            let exe = std::env::current_exe()
                                .map(|exe| exe.parent().unwrap().join("eye-calibrate.exe"))
                                .map_err(|error| error.to_string())?;
                            let output = std::process::Command::new(exe)
                                .creation_flags(0x08000000)
                                .output()
                                .map_err(|error| error.to_string())?;
                            if output.status.success() {
                                Ok("Pointer alignment saved. Game eye tracking is unchanged."
                                    .into())
                            } else {
                                Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
                            }
                        })
                        .await
                        {
                            Ok(result) => result,
                            Err(error) => Err(error.to_string()),
                        }
                    },
                    Message::EyePointerCalibrationDone,
                );
            }
            Message::EyePointerCalibrationDone(result) => {
                self.pointer_calibration_running = false;
                self.pointer_calibration_status = result.unwrap_or_else(|error| error);
            }
            Message::EyeCalibrationResult(status) => {
                self.calibration_check_pending = false;
                self.calibration_status = status;
            }
            Message::EyePage(value) => {
                self.wind_page = false;
                self.settings_page = false;
                self.eye_page = value;
                self.eye_reading = None;
            }
            Message::Focused(value) => {
                self.focused = value;
                if !value && !self.keep_eyes_live {
                    self.eye_reading = None;
                }
            }
            Message::KeepEyesLive(value) => self.keep_eyes_live = value,
            Message::EyeTick => {
                if !self.settings_page
                    && (self.focused || self.keep_eyes_live)
                    && !self.eye_pending
                    && !self.state.demo
                {
                    self.eye_pending = true;
                    return Task::perform(
                        async {
                            tokio::task::spawn_blocking(rig_companion::eyes::read)
                                .await
                                .map_err(|e| e.to_string())?
                                .map_err(|e| format!("{e:#}"))
                        },
                        Message::EyeRead,
                    );
                }
            }
            Message::EyeRead(result) => {
                self.eye_pending = false;
                if !self.settings_page && (self.focused || self.keep_eyes_live) {
                    match result {
                        Ok(reading) => {
                            self.eye_reading = Some(reading);
                            self.eye_error = None;
                        }
                        Err(e) => {
                            self.eye_reading = None;
                            self.eye_error = Some(e);
                        }
                    }
                }
            }
            Message::Tick => {
                if let Some(progress) = &self.launch_progress {
                    for status in progress.try_iter() {
                        self.launch_status = status;
                    }
                }
                if !self.quitting
                    && ((self.quit_queued && !self.settings.busy())
                        || self.quit_signal.as_ref().is_some_and(|s| s.requested()))
                {
                    return self.update(Message::Quit);
                }
                let fresh = self.worker.snapshot();
                if fresh.revision != self.state.revision {
                    self.local_message = None;
                    if fresh.profile.height_m != self.state.profile.height_m
                        && let Some(h) = fresh.profile.height_m
                    {
                        self.height = format!("{:.1}", h * 100.0);
                    }
                }
                self.state = fresh;
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|(deadline, _)| Instant::now() >= *deadline)
                    && let Some((_, command)) = self.pending.take()
                {
                    self.dispatch(command);
                }
                let shortcuts: Vec<_> = self
                    .hotkeys
                    .as_ref()
                    .map(|h| h.events.try_iter().collect())
                    .unwrap_or_default();
                for event in shortcuts {
                    match event {
                        HotkeyEvent::GazeDown => {
                            if self.state.connected && !self.quitting && !self.gaze_held {
                                self.gaze_held = true;
                                self.gaze_refreshed = Instant::now();
                                self.worker.send(Command::GazeDown);
                            }
                        }
                        HotkeyEvent::GazeUp => self.release_gaze(),
                        HotkeyEvent::Joystick(direction) => {
                            if direction == 0 || !self.state.connected || self.quitting {
                                self.release_joystick();
                            } else {
                                self.joystick_direction = direction;
                                self.joystick_refreshed = Instant::now();
                                self.worker.send(Command::Joystick(direction));
                            }
                        }
                        HotkeyEvent::Shortcut(1) => self.schedule(Command::RestoreHeight),
                        HotkeyEvent::Shortcut(2) => {
                            self.pending = None;
                            self.dispatch(Command::Undo);
                        }
                        HotkeyEvent::Shortcut(3) => self.dispatch(Command::Dashboard),
                        HotkeyEvent::Shortcut(4) => self.schedule(Command::Recenter98),
                        HotkeyEvent::Shortcut(6) => self.dispatch(Command::Dashboard),
                        HotkeyEvent::Shortcut(7) => self.dispatch(Command::Passthrough),
                        HotkeyEvent::Shortcut(10) => self.dispatch(Command::GazePointerToggle),
                        _ => {}
                    }
                }
                if self.gaze_held && self.gaze_refreshed.elapsed() >= Duration::from_millis(250) {
                    self.gaze_refreshed = Instant::now();
                    self.worker.send(Command::GazeRefresh);
                }
                if self.joystick_direction != 0
                    && self.joystick_refreshed.elapsed() >= Duration::from_millis(250)
                {
                    self.joystick_refreshed = Instant::now();
                    self.worker.send(Command::Joystick(self.joystick_direction));
                }
            }
            Message::Height(value) => self.height = value,
            Message::Save => {
                match self
                    .height
                    .replace(',', ".")
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .and_then(|v| validate_height(v / 100.0).ok())
                {
                    Some(h) => self.dispatch(Command::SaveHeight(h)),
                    None => {
                        self.local_message =
                            Some("Enter a seated height between 20 and 250 cm.".into())
                    }
                }
            }
            Message::Connect => self.dispatch(Command::Connect),
            Message::Capture => self.schedule(Command::Capture),
            Message::Restore => self.schedule(Command::RestoreHeight),
            Message::RestoreFull => self.schedule(Command::RestoreReference),
            Message::Nudge(delta) => self.dispatch(Command::Nudge(delta)),
            Message::Step(step) => self.step = step,
            Message::Undo => {
                self.pending = None;
                self.dispatch(Command::Undo);
            }
            Message::Dashboard => self.dispatch(Command::Dashboard),
            Message::Passthrough => {
                self.wind_page = false;
                self.settings_page = false;
                self.eye_page = false;
                self.dispatch(Command::Passthrough);
            }
            Message::GazeClick => self.dispatch(Command::GazeClick),
            Message::GazePointerToggle => self.dispatch(Command::GazePointerToggle),
            Message::Delay(value) => self.countdown_enabled = value,
            Message::Cancel => self.pending = None,
            Message::Shortcuts(enabled) => {
                if enabled {
                    match Hotkeys::start() {
                        Ok(h) => self.hotkeys = Some(h),
                        Err(e) => self.local_message = Some(e.to_string()),
                    }
                } else {
                    self.release_gaze();
                    self.release_joystick();
                    self.hotkeys = None;
                }
            }
        }
        Task::none()
    }

    fn dispatch(&mut self, command: Command) {
        if self.state.busy || self.quitting || self.quit_queued {
            return;
        }
        self.local_message = None;
        self.state.busy = true;
        self.worker.send(command);
    }

    fn release_gaze(&mut self) {
        if self.gaze_held {
            self.gaze_held = false;
            self.worker.send(Command::GazeUp);
        }
    }

    fn release_joystick(&mut self) {
        if self.joystick_direction != 0 {
            self.joystick_direction = 0;
            self.worker.send(Command::Joystick(0));
        }
    }

    fn schedule(&mut self, command: Command) {
        if self.state.busy || self.pending.is_some() {
            return;
        }
        if !self.state.ready {
            self.local_message =
                Some("Connect your headset and allow tracking to settle first.".into());
            return;
        }
        if self.countdown_enabled {
            let delay = if matches!(command, Command::Recenter98) {
                Duration::from_millis(500)
            } else {
                Duration::from_secs(3)
            };
            self.pending = Some((Instant::now() + delay, command));
        } else {
            self.dispatch(command);
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            // Read the worker's cached snapshot; this does not poll the hardware.
            // Refresh faster than the one-second SimHub heartbeat expiry.
            iced::time::every(Duration::from_millis(100))
                .map(|_| Message::Wind(crate::wind_ui::Message::Tick)),
            iced::time::every(Duration::from_millis(50)).map(|_| Message::Tick),
            iced::event::listen_with(|event, _, _| match event {
                iced::Event::Window(iced::window::Event::CloseRequested) => Some(Message::Quit),
                iced::Event::Window(iced::window::Event::Focused) => Some(Message::Focused(true)),
                iced::Event::Window(iced::window::Event::Unfocused) => {
                    Some(Message::Focused(false))
                }
                _ => None,
            }),
        ];
        if !self.settings_page && (self.focused || self.keep_eyes_live) {
            subscriptions
                .push(iced::time::every(Duration::from_millis(250)).map(|_| Message::EyeTick));
        }
        Subscription::batch(subscriptions)
    }

    pub fn theme(&self) -> Theme {
        Theme::custom(
            "Rig night",
            iced::theme::Palette {
                background: BG,
                text: WHITE,
                primary: ACCENT,
                success: ACCENT,
                danger: RED,
                warning: AMBER,
            },
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        if self.wind_page {
            return container(
                column![self.navigation(), self.wind.view().map(Message::Wind)].spacing(20),
            )
            .padding(24)
            .height(Fill)
            .width(Fill)
            .into();
        }
        if self.settings_page {
            return container(
                column![
                    self.navigation(),
                    self.settings.view().map(Message::DriverSettings)
                ]
                .spacing(20),
            )
            .padding(24)
            .height(Fill)
            .width(Fill)
            .into();
        }
        if self.eye_page {
            return self.eye_view();
        }
        let free = !self.state.busy && self.pending.is_none();
        let ready = self.state.ready && free;
        let status_color = if self.state.demo {
            AMBER
        } else if self.state.ready {
            ACCENT
        } else {
            MUTED
        };
        let status_label = if self.state.demo {
            "DEMO MODE"
        } else if self.state.ready {
            "TRACKING READY"
        } else if self.state.connected {
            "WAITING FOR TRACKING"
        } else {
            "STEAMVR OFFLINE"
        };
        let header = row![
            widget::image(self.logo.clone()).width(42).height(42),
            text("Rig Companion").size(24).font(bold()),
            Space::new().width(Fill),
            container(text(status_label).size(12).font(bold()).color(status_color))
                .padding([10, 15])
                .style(|_| box_style(PANEL)),
        ]
        .spacing(16)
        .align_y(alignment::Vertical::Center);

        let current = self
            .state
            .pose
            .map(|p| format!("{:.1}", p.position[1] * 100.0))
            .unwrap_or_else(|| "—".into());
        let saved = self
            .state
            .profile
            .height_m
            .map(|v| format!("{:.1}", v * 100.0))
            .unwrap_or_else(|| "—".into());
        let metrics = row![
            metric("CURRENT HEIGHT", current, "cm above SteamVR floor", WHITE),
            metric(
                "SEATED REFERENCE",
                saved,
                if self.state.profile.height_m.is_some() {
                    "cm · your seated height"
                } else {
                    "set your height to begin"
                },
                ACCENT
            ),
        ]
        .spacing(12);

        let restore_label = if let Some((deadline, _)) = &self.pending {
            format!(
                "Sit back · {:.1}s",
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_secs_f32()
            )
        } else if self.state.busy {
            "Working…".into()
        } else {
            "Restore seated height".into()
        };
        let restore = action(
            restore_label,
            Message::Restore,
            ready && self.state.profile.height_m.is_some(),
            true,
        );
        let gauge = canvas(Gauge {
            current: self.state.pose.map(|p| p.position[1]),
            target: self.state.profile.height_m,
        })
        .width(Fill)
        .height(64);
        let calibration = panel(
            column![
                text("FLOOR CALIBRATION").size(12).color(MUTED),
                metrics,
                gauge,
                row![
                    text("FLOOR  /  0 cm").size(11).color(MUTED),
                    Space::new().width(Fill),
                    text("—  Seated reference").size(11).color(ACCENT)
                ],
                restore,
                row![
                    checkbox(self.countdown_enabled)
                        .label("3-second countdown")
                        .on_toggle(Message::Delay)
                        .size(18),
                    Space::new().width(Fill),
                    if self.pending.is_some() {
                        button(text("Cancel").size(14))
                            .on_press(Message::Cancel)
                            .style(button::text)
                    } else {
                        button(text("Ctrl + Alt + F8").size(13).color(MUTED)).style(button::text)
                    }
                ]
                .align_y(alignment::Vertical::Center),
            ]
            .spacing(12),
        )
        .width(Length::FillPortion(3));

        let field = text_input("98.0", &self.height)
            .on_input(Message::Height)
            .size(30)
            .padding(12)
            .width(Fill);
        let reference = panel(
            column![
                text("YOUR REFERENCE").size(12).color(MUTED),
                text("Seated height (cm)").size(15).color(MUTED),
                field,
                action("Save height", Message::Save, free, false),
                text("Eye height above the real floor.")
                    .size(14)
                    .color(MUTED),
                widget::rule::horizontal(1),
                action("Capture current position", Message::Capture, ready, false),
                action(
                    "Restore position + heading",
                    Message::RestoreFull,
                    ready && self.state.profile.reference.is_some(),
                    false
                ),
                text(if self.state.profile.reference.is_some() {
                    "Full reference saved. Look forward before restoring."
                } else {
                    "Capture also saves your horizontal position and forward direction."
                })
                .size(13)
                .color(MUTED),
            ]
            .spacing(10),
        )
        .width(Length::FillPortion(2));

        let controls = panel(
            column![
                row![
                    text("FINE ADJUSTMENT").size(12).color(MUTED),
                    Space::new().width(Fill),
                    step_button("5 mm", 0.005, self.step),
                    step_button("1 cm", 0.01, self.step),
                    step_button("5 cm", 0.05, self.step)
                ]
                .spacing(8)
                .align_y(alignment::Vertical::Center),
                row![
                    action(
                        "−  Lower viewpoint",
                        Message::Nudge(-self.step),
                        ready,
                        false
                    ),
                    action(
                        "+  Raise viewpoint",
                        Message::Nudge(self.step),
                        ready,
                        false
                    ),
                    action(
                        "Undo last change",
                        Message::Undo,
                        self.state.can_undo && free,
                        false
                    )
                ]
                .spacing(12),
            ]
            .spacing(18),
        );

        let notice = if self.steamvr_starting {
            &self.launch_status
        } else if self.pending.is_some() {
            "Sit normally and look forward. Hold still until the countdown finishes."
        } else if let Some(m) = &self.local_message {
            m
        } else {
            &self.state.message
        };
        let notice_color = if self.local_message.is_some() || self.state.error {
            AMBER
        } else {
            MUTED
        };
        let footer = column![
            column![
                text(&self.state.headset).size(14).font(bold()),
                text(if self.state.demo {
                    "Simulation only · no SteamVR changes"
                } else {
                    "SteamVR floor space · iRacing recenter stays on your wheel"
                })
                .size(12)
                .color(MUTED)
            ]
            .spacing(5),
            row![
                button(
                    text(if self.state.connected {
                        "Reconnect"
                    } else {
                        "Connect SteamVR"
                    })
                    .size(14)
                )
                .padding([12, 16])
                .on_press_maybe(free.then_some(Message::Connect))
                .style(secondary),
                button(text("Dashboard · F15").size(14))
                    .padding([12, 16])
                    .on_press_maybe((self.state.connected && free).then_some(Message::Dashboard))
                    .style(secondary),
                button(text("VR gaze click · hold F14 to drag").size(14))
                    .padding([12, 16])
                    .on_press_maybe((self.state.connected && free).then_some(Message::GazeClick))
                    .style(secondary),
                button(text("Eye pointer · F19 toggle").size(14))
                    .padding([12, 16])
                    .on_press_maybe(
                        (self.state.connected && free).then_some(Message::GazePointerToggle)
                    )
                    .style(secondary),
                button(text("Camera · F16").size(14))
                    .padding([12, 16])
                    .on_press_maybe((self.state.connected && free).then_some(Message::Passthrough))
                    .style(secondary)
            ]
            .spacing(10)
        ]
        .spacing(12);
        let shortcuts =
            row![
            checkbox(self.hotkeys.is_some())
                .label("Global shortcuts")
                .on_toggle(Message::Shortcuts)
                .size(17),
            text("F13 recenter / F14 click or drag / F15 desktop / F16 camera / F17-F18 joystick / F19 eye pointer | Ctrl+Alt: F8 height / F9 undo / F7 dashboard")
                .size(12)
                .color(MUTED),
            Space::new().width(Fill),
            text("Keep this window open or minimized")
                .size(12)
                .color(MUTED)
        ]
            .spacing(14)
            .align_y(alignment::Vertical::Center);

        let mut notice_row = row![text(notice).size(14).color(notice_color).width(Fill)]
            .spacing(14)
            .align_y(alignment::Vertical::Center);
        if !self.state.demo && !self.state.connected && !self.wind.steamvr_running() {
            notice_row = notice_row.push(
                button(
                    text(if self.steamvr_starting {
                        "Starting…"
                    } else {
                        "Start SteamVR"
                    })
                    .size(14),
                )
                .on_press_maybe(
                    (!self.launch_busy && !self.quitting && !self.quit_queued)
                        .then_some(Message::StartSteamVr),
                )
                .padding([8, 12])
                .style(secondary),
            );
        }
        let content = column![
            self.navigation(),
            header,
            self.game_launchers(),
            self.compact_eyes(),
            container(notice_row)
                .padding([10, 14])
                .width(Fill)
                .style(|_| box_style(INNER)),
            row![calibration, reference].spacing(18),
            controls,
            footer,
            shortcuts
        ]
        .spacing(16)
        .max_width(1120);
        container(scrollable(container(content).padding(24).center_x(Fill)))
            .height(Fill)
            .width(Fill)
            .into()
    }

    fn game_launchers(&self) -> Element<'_, Message> {
        let enabled = !self.launch_busy
            && !self.quitting
            && !self.quit_queued
            && self
                .launch_cooldown
                .is_none_or(|deadline| Instant::now() >= deadline);
        let mut games = row![].spacing(10);
        for (index, game) in rig_companion::game_launch::Game::ALL
            .into_iter()
            .enumerate()
        {
            games = games.push(
                button(
                    container(
                        column![
                            widget::image(self.game_icons[index].clone())
                                .width(36)
                                .height(36),
                            text(game.label()).size(14).font(bold())
                        ]
                        .spacing(9)
                        .align_x(alignment::Horizontal::Center),
                    )
                    .center_x(Fill),
                )
                .width(Fill)
                .padding([12, 8])
                .style(secondary)
                .on_press_maybe(enabled.then_some(Message::Launch(game))),
            );
        }
        column![
            games,
            text(&self.launch_status)
                .size(12)
                .color(if self.launch_error { AMBER } else { MUTED })
        ]
        .spacing(8)
        .into()
    }

    fn navigation(&self) -> Element<'_, Message> {
        column![
            row![
                button("Seated reference")
                    .on_press(Message::EyePage(false))
                    .style(secondary),
                button("Eye tracking")
                    .on_press(Message::EyePage(true))
                    .style(secondary),
                button("Driver settings")
                    .on_press(Message::SettingsPage)
                    .style(secondary),
                button("Wind simulator")
                    .on_press(Message::WindPage)
                    .style(secondary),
                Space::new().width(Fill),
                button(if self.quitting {
                    "Closing SteamVR…"
                } else {
                    "Quit + SteamVR"
                })
                .on_press_maybe((!self.quitting).then_some(Message::Quit))
                .style(secondary),
            ]
            .spacing(10),
            text(self.quit_error.as_deref().unwrap_or(""))
                .size(12)
                .color(AMBER)
        ]
        .spacing(2)
        .into()
    }

    fn compact_eyes(&self) -> Element<'_, Message> {
        let reading = self
            .eye_reading
            .as_ref()
            .filter(|r| r.usable() && (self.focused || self.keep_eyes_live));
        let status = if !self.focused && !self.keep_eyes_live {
            "Paused"
        } else if reading.is_some() {
            "Valid gaze"
        } else {
            "No valid gaze"
        };
        container(
            row![
                column![
                    text("Eye tracking").size(15).font(bold()),
                    text(status).size(12).color(MUTED),
                    button("Details")
                        .on_press(Message::EyePage(true))
                        .style(secondary)
                ]
                .spacing(5),
                canvas(EyePlot {
                    degrees: reading.map(|r| r.gaze.left.degrees()),
                    color: WHITE
                })
                .width(100)
                .height(88),
                canvas(EyePlot {
                    degrees: reading.map(|r| r.gaze.right.degrees()),
                    color: ACCENT
                })
                .width(100)
                .height(88),
                text(
                    reading
                        .map(|r| {
                            let l = r.gaze.left.degrees();
                            let r = r.gaze.right.degrees();
                            format!(
                                "L  {:+.1}° / {:+.1}°\nR  {:+.1}° / {:+.1}°",
                                l[0], l[1], r[0], r[1]
                            )
                        })
                        .unwrap_or_else(|| "—".into())
                )
                .size(13)
                .color(MUTED),
                Space::new().width(Fill),
                checkbox(self.keep_eyes_live)
                    .label("Keep live in VR")
                    .on_toggle(Message::KeepEyesLive)
            ]
            .spacing(16)
            .align_y(alignment::Vertical::Center),
        )
        .padding([8, 14])
        .style(|_| box_style(PANEL))
        .into()
    }

    fn eye_view(&self) -> Element<'_, Message> {
        let paused = !self.focused && !self.keep_eyes_live;
        let reading = self.eye_reading.as_ref();
        let usable = !paused && self.eye_error.is_none() && reading.is_some_and(|r| r.usable());
        let status = if self.state.demo {
            "PREVIEW UNAVAILABLE IN DEMO"
        } else if paused {
            "PAUSED"
        } else if self.eye_error.is_some() {
            "WAITING FOR DIAGNOSTICS"
        } else if reading.is_some_and(|r| !r.live()) {
            "STALE / DRIVER OFFLINE"
        } else if reading.is_some_and(|r| !r.gaze.valid) {
            "NO VALID GAZE"
        } else if usable {
            "DRIVER REPORTS VALID"
        } else {
            "WAITING"
        };
        let eye = |label: &'static str, angles: Option<rig_companion::eyes::Angles>, color| {
            let degrees = if usable {
                angles.map(|a| a.degrees())
            } else {
                None
            };
            panel(
                column![
                    text(label).size(14).font(bold()).color(color),
                    canvas(EyePlot { degrees, color }).width(Fill).height(260),
                    text(
                        degrees
                            .map(|d| format!("X {:+.1}°     Y {:+.1}°", d[0], d[1]))
                            .unwrap_or_else(|| "No live estimate".into())
                    )
                    .size(22)
                    .font(bold()),
                    text("Driver angular coordinates · ±30° plot")
                        .size(12)
                        .color(MUTED),
                ]
                .spacing(12),
            )
            .width(Fill)
        };
        let age = reading
            .map(|r| format!("Telemetry written {:.2} s ago", r.age_ms as f64 / 1000.0))
            .unwrap_or_else(|| "No telemetry received".into());
        let content = column![
            self.navigation(),
            row![text("Eye tracking").size(36).font(bold()), Space::new().width(Fill), text(status).size(13).color(if usable { ACCENT } else { AMBER })].align_y(alignment::Vertical::Center),
            text("Live gaze estimates").size(18).color(MUTED),
            row![eye("LEFT GAZE ESTIMATE", reading.map(|r| r.gaze.left), ACCENT), eye("RIGHT GAZE ESTIMATE", reading.map(|r| r.gaze.right), AMBER)].spacing(18),
            panel(column![
                row![text(if paused { "Preview paused while unfocused".into() } else { age }).size(15), Space::new().width(Fill), checkbox(self.keep_eyes_live).label("Keep live in VR").on_toggle(Message::KeepEyesLive)].spacing(16),
                text(self.eye_error.as_deref().unwrap_or("4 updates/second from the custom driver. File freshness does not prove a new eye-camera sample; the driver can reuse cached data. These are gaze estimates, not eye images.")).size(14).color(MUTED),
            ].spacing(12)),
            panel(column![
                section("EYE POINTER ALIGNMENT", "Correct the SteamVR dashboard pointer without changing game gaze"),
                text("Look at the center target, then two rings of targets in the headset. The app measures gaze at each one and applies a smooth spatial correction to the F19 pointer only.").size(14).color(MUTED),
                button(if self.pointer_calibration_running { "Aligning…" } else { "Align eye pointer in VR" })
                    .on_press_maybe((!self.state.demo && self.state.connected && !self.pointer_calibration_running).then_some(Message::LaunchEyePointerCalibration))
                    .style(secondary).padding(12),
                text(&self.pointer_calibration_status).size(14).color(MUTED),
            ].spacing(12)),
            panel(column![
                section("EYE CALIBRATION", "Native tracker calibration"),
                text(&self.calibration_status).size(16),
                button(if self.calibration_check_pending { "Checking tracker…" } else { "Check calibration availability" })
                    .on_press_maybe((!self.calibration_check_pending && !self.state.demo).then_some(Message::CheckEyeCalibration)).style(secondary).padding(12),
            ].spacing(12)),
            text("F13 recenter · F14 click or drag · F15 dashboard · F16 camera · F19 eye pointer toggle.").size(14).color(MUTED),
        ].spacing(20).max_width(1120);
        container(scrollable(container(content).padding(24).center_x(Fill)))
            .height(Fill)
            .width(Fill)
            .into()
    }
}

struct EyePlot {
    degrees: Option<[f64; 2]>,
    color: Color,
}
impl canvas::Program<Message> for EyePlot {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = bounds.height.min(bounds.width) / 2.0 - 16.0;
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), INNER);
        for fraction in [0.33, 0.66, 1.0] {
            frame.stroke(
                &canvas::Path::circle(center, radius * fraction),
                canvas::Stroke::default().with_color(LINE),
            );
        }
        for (a, b) in [
            (
                Point::new(center.x - radius, center.y),
                Point::new(center.x + radius, center.y),
            ),
            (
                Point::new(center.x, center.y - radius),
                Point::new(center.x, center.y + radius),
            ),
        ] {
            frame.stroke(
                &canvas::Path::line(a, b),
                canvas::Stroke::default().with_color(LINE),
            );
        }
        if let Some([x, y]) = self.degrees {
            let point = Point::new(
                center.x + (x as f32 / 30.0).clamp(-1.0, 1.0) * radius,
                center.y - (y as f32 / 30.0).clamp(-1.0, 1.0) * radius,
            );
            frame.stroke(
                &canvas::Path::line(center, point),
                canvas::Stroke::default()
                    .with_color(self.color)
                    .with_width(1.5),
            );
            frame.stroke(
                &canvas::Path::circle(point, 12.0),
                canvas::Stroke::default()
                    .with_color(self.color)
                    .with_width(2.0),
            );
            frame.fill(&canvas::Path::circle(point, 4.0), self.color);
        }
        vec![frame.into_geometry()]
    }
}

fn bold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..Font::with_name("Segoe UI")
    }
}
fn box_style(background: Color) -> container::Style {
    container::Style {
        background: Some(background.into()),
        border: border::rounded(14).width(1).color(LINE),
        ..Default::default()
    }
}
fn panel<'a>(content: impl Into<Element<'a, Message>>) -> container::Container<'a, Message> {
    container(content).padding(20).style(|_| box_style(PANEL))
}
fn section<'a>(label: &'a str, description: &'a str) -> Element<'a, Message> {
    column![
        text(label).size(12).font(bold()).color(ACCENT),
        text(description).size(15).color(MUTED)
    ]
    .spacing(7)
    .into()
}
fn metric<'a>(
    label: &'a str,
    value: String,
    caption: &'a str,
    color: Color,
) -> Element<'a, Message> {
    container(
        column![
            text(label).size(11).color(MUTED),
            text(value).size(45).font(bold()).color(color),
            text(caption).size(12).color(MUTED)
        ]
        .spacing(6),
    )
    .padding(14)
    .width(Fill)
    .style(|_| box_style(INNER))
    .into()
}
fn action<'a>(
    label: impl Into<String>,
    message: Message,
    enabled: bool,
    primary: bool,
) -> Element<'a, Message> {
    button(
        text(label.into())
            .size(17)
            .font(bold())
            .width(Fill)
            .align_x(alignment::Horizontal::Center),
    )
    .width(Fill)
    .padding([14, 12])
    .on_press_maybe(enabled.then_some(message))
    .style(if primary { primary_style } else { secondary })
    .into()
}
fn primary_style(_: &Theme, status: button::Status) -> button::Style {
    let disabled = matches!(status, button::Status::Disabled);
    button::Style {
        background: Some(
            if disabled {
                LINE
            } else if matches!(status, button::Status::Hovered) {
                Color::from_rgb8(210, 200, 238)
            } else {
                WHITE
            }
            .into(),
        ),
        text_color: if disabled { MUTED } else { BG },
        border: border::rounded(10),
        ..Default::default()
    }
}
fn secondary(_: &Theme, status: button::Status) -> button::Style {
    let disabled = matches!(status, button::Status::Disabled);
    button::Style {
        background: Some(
            if matches!(status, button::Status::Hovered) {
                LINE
            } else {
                INNER
            }
            .into(),
        ),
        text_color: if disabled {
            Color::from_rgb8(100, 100, 108)
        } else {
            WHITE
        },
        border: border::rounded(10).width(1).color(LINE),
        ..Default::default()
    }
}
fn step_button(label: &str, step: f64, selected: f64) -> Element<'_, Message> {
    button(text(label).size(13))
        .padding([9, 12])
        .on_press(Message::Step(step))
        .style(if (step - selected).abs() < 1e-8 {
            primary_style
        } else {
            secondary
        })
        .into()
}

struct Gauge {
    current: Option<f64>,
    target: Option<f64>,
}
impl canvas::Program<Message> for Gauge {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), PANEL);
        let w = bounds.width;
        let floor = bounds.height - 27.0;
        let scale = 40.0;
        for i in 0..=8 {
            let x = i as f32 * w / 8.0;
            frame.stroke(
                &canvas::Path::line(
                    Point::new(w / 2.0 + (x - w / 2.0) * 0.38, floor - 26.0),
                    Point::new(x, bounds.height),
                ),
                canvas::Stroke::default().with_color(LINE).with_width(0.7),
            );
        }
        frame.stroke(
            &canvas::Path::line(Point::new(0.0, floor), Point::new(w, floor)),
            canvas::Stroke::default().with_color(MUTED).with_width(1.0),
        );
        if let Some(target) = self.target {
            let y = (floor - target as f32 * scale).clamp(12.0, floor);
            frame.stroke(
                &canvas::Path::line(Point::new(10.0, y), Point::new(w - 10.0, y)),
                canvas::Stroke::default().with_color(ACCENT).with_width(1.0),
            );
        }
        if let Some(height) = self.current {
            let y = (floor - height as f32 * scale).clamp(14.0, bounds.height - 9.0);
            let color = if height < 0.0 { AMBER } else { WHITE };
            let path = canvas::Path::rounded_rectangle(
                Point::new(w * 0.68 - 22.0, y - 10.0),
                Size::new(44.0, 20.0),
                6.0.into(),
            );
            frame.fill(&path, INNER);
            frame.stroke(
                &path,
                canvas::Stroke::default().with_color(color).with_width(2.0),
            );
            frame.fill(
                &canvas::Path::circle(Point::new(w * 0.68 - 8.0, y), 3.0),
                color,
            );
            frame.fill(
                &canvas::Path::circle(Point::new(w * 0.68 + 8.0, y), 3.0),
                color,
            );
        }
        vec![frame.into_geometry()]
    }
}
