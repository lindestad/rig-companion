use crate::hotkeys::Hotkeys;
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
use std::time::{Duration, Instant};

const BG: Color = Color::from_rgb8(13, 18, 24);
const PANEL: Color = Color::from_rgb8(22, 30, 39);
const INNER: Color = Color::from_rgb8(16, 23, 31);
const LINE: Color = Color::from_rgb8(43, 57, 70);
const WHITE: Color = Color::from_rgb8(235, 242, 247);
const MUTED: Color = Color::from_rgb8(150, 170, 185);
const ACCENT: Color = Color::from_rgb8(126, 237, 191);
const AMBER: Color = Color::from_rgb8(246, 191, 105);
const RED: Color = Color::from_rgb8(255, 145, 142);

pub struct App {
    worker: Worker,
    state: Snapshot,
    height: String,
    step: f64,
    countdown_enabled: bool,
    pending: Option<(Instant, Command)>,
    hotkeys: Option<Hotkeys>,
    local_message: Option<String>,
    eye_page: bool,
    focused: bool,
    keep_eyes_live: bool,
    eye_pending: bool,
    eye_reading: Option<rig_companion::eyes::Reading>,
    eye_error: Option<String>,
    calibration_check_pending: bool,
    calibration_status: String,
    _lock: std::fs::File,
}

#[derive(Debug, Clone)]
pub enum Message {
    EyePage(bool),
    Focused(bool),
    KeepEyesLive(bool),
    EyeTick,
    EyeRead(Result<rig_companion::eyes::Reading, String>),
    CheckEyeCalibration,
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
    Delay(bool),
    Shortcuts(bool),
    Cancel,
}

impl App {
    pub fn new(worker: Worker, lock: std::fs::File) -> Self {
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
            worker,
            state,
            height,
            step: 0.01,
            countdown_enabled: true,
            pending: None,
            hotkeys,
            local_message,
            eye_page: false,
            focused: true,
            keep_eyes_live: false,
            eye_pending: false,
            eye_reading: None,
            eye_error: None,
            calibration_check_pending: false,
            calibration_status: "Check whether Pimax exposes a tracker with 3D calibration and a retrievable backup. This check does not change your calibration.".into(),
            _lock: lock,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
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
            Message::EyeCalibrationResult(status) => {
                self.calibration_check_pending = false;
                self.calibration_status = status;
            }
            Message::EyePage(value) => {
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
                if self.eye_page
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
                if self.eye_page && (self.focused || self.keep_eyes_live) {
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
                for id in shortcuts {
                    match id {
                        1 => self.schedule(Command::RestoreHeight),
                        2 => {
                            self.pending = None;
                            self.dispatch(Command::Undo);
                        }
                        3 => self.dispatch(Command::Dashboard),
                        4 => self.schedule(Command::Recenter98),
                        5 => self.dispatch(Command::GazeClick),
                        6 => self.dispatch(Command::Dashboard),
                        _ => {}
                    }
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
            Message::GazeClick => self.dispatch(Command::GazeClick),
            Message::Delay(value) => self.countdown_enabled = value,
            Message::Cancel => self.pending = None,
            Message::Shortcuts(enabled) => {
                if enabled {
                    match Hotkeys::start() {
                        Ok(h) => self.hotkeys = Some(h),
                        Err(e) => self.local_message = Some(e.to_string()),
                    }
                } else {
                    self.hotkeys = None;
                }
            }
        }
        Task::none()
    }

    fn dispatch(&mut self, command: Command) {
        if self.state.busy {
            return;
        }
        self.local_message = None;
        self.state.busy = true;
        self.worker.send(command);
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
            iced::time::every(Duration::from_millis(50)).map(|_| Message::Tick),
            iced::event::listen_with(|event, _, _| match event {
                iced::Event::Window(iced::window::Event::Focused) => Some(Message::Focused(true)),
                iced::Event::Window(iced::window::Event::Unfocused) => {
                    Some(Message::Focused(false))
                }
                _ => None,
            }),
        ];
        if self.eye_page && (self.focused || self.keep_eyes_live) {
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
            container(text("R / C").size(19).font(bold()).color(ACCENT))
                .padding([14, 16])
                .style(|_| box_style(INNER)),
            column![
                text("RIG COMPANION").size(22).font(bold()),
                text("Your seat. Your reference.").size(14).color(MUTED)
            ]
            .spacing(4),
            Space::new().width(Fill),
            container(text(status_label).size(12).font(bold()).color(status_color))
                .padding([10, 15])
                .style(|_| box_style(PANEL)),
        ]
        .spacing(16)
        .align_y(alignment::Vertical::Center);

        let intro = row![
            column![
                text("Back where you belong.").size(34).font(bold()),
                text("Restore your seated position in SteamVR, with a single click.")
                    .size(16)
                    .color(MUTED)
            ]
            .spacing(8),
            Space::new().width(Fill),
            text("01 / CALIBRATION").size(12).color(MUTED)
        ]
        .align_y(alignment::Vertical::Bottom);

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
        .height(106);
        let calibration = panel(
            column![
                section("FLOOR CALIBRATION", "A familiar starting point"),
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
                section("YOUR REFERENCE", "Set once. Restore anytime."),
                text("Seated height (cm)").size(15).color(MUTED),
                field,
                action("Save height", Message::Save, free, false),
                text(
                    "Your eye height above the real floor. You can also adjust below, then capture."
                )
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
                    section("FINE ADJUSTMENT", "Small changes, right from your seat"),
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

        let notice = if self.pending.is_some() {
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
        let footer = row![
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
            Space::new().width(Fill),
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
            button(text("VR gaze click · F14").size(14))
                .padding([12, 16])
                .on_press_maybe((self.state.connected && free).then_some(Message::GazeClick))
                .style(secondary)
        ]
        .spacing(10)
        .align_y(alignment::Vertical::Center);
        let shortcuts =
            row![
            checkbox(self.hotkeys.is_some())
                .label("Global shortcuts")
                .on_toggle(Message::Shortcuts)
                .size(17),
            text("F13 recenter / F14 click / F15 desktop | Ctrl+Alt: F8 height / F9 undo / F7 dashboard")
                .size(12)
                .color(MUTED),
            Space::new().width(Fill),
            text("Keep this window open or minimized")
                .size(12)
                .color(MUTED)
        ]
            .spacing(14)
            .align_y(alignment::Vertical::Center);

        let content = column![
            row![
                button("Seated reference").style(primary_style),
                button("Eye tracking")
                    .on_press(Message::EyePage(true))
                    .style(secondary)
            ]
            .spacing(10),
            header,
            intro,
            container(text(notice).size(14).color(notice_color))
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
            row![button("Seated reference").on_press(Message::EyePage(false)).style(secondary), button("Eye tracking").style(primary_style)].spacing(10),
            row![text("Eye tracking").size(36).font(bold()), Space::new().width(Fill), text(status).size(13).color(if usable { ACCENT } else { AMBER })].align_y(alignment::Vertical::Center),
            text("See where your headset thinks you are looking.").size(18).color(MUTED),
            row![eye("LEFT GAZE ESTIMATE", reading.map(|r| r.gaze.left), ACCENT), eye("RIGHT GAZE ESTIMATE", reading.map(|r| r.gaze.right), Color::from_rgb8(142, 190, 255))].spacing(18),
            panel(column![
                row![text(if paused { "Preview paused while unfocused".into() } else { age }).size(15), Space::new().width(Fill), checkbox(self.keep_eyes_live).label("Keep live in VR").on_toggle(Message::KeepEyesLive)].spacing(16),
                text(self.eye_error.as_deref().unwrap_or("4 updates/second from the custom driver. File freshness does not prove a new eye-camera sample; the driver can reuse cached data. These are gaze estimates, not eye images.")).size(14).color(MUTED),
            ].spacing(12)),
            panel(column![
                section("EYE CALIBRATION", "Native tracker calibration"),
                text(&self.calibration_status).size(16),
                button(if self.calibration_check_pending { "Checking tracker…" } else { "Check calibration availability" })
                    .on_press_maybe((!self.calibration_check_pending && !self.state.demo).then_some(Message::CheckEyeCalibration)).style(secondary).padding(12),
            ].spacing(12)),
            text("F13 recenter · F14 gaze click · F15 dashboard toggle stay active on this page.").size(14).color(MUTED),
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
                Color::from_rgb8(164, 255, 217)
            } else {
                ACCENT
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
            Color::from_rgb8(91, 109, 123)
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
