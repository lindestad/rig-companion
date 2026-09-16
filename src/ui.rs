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
    _lock: std::fs::File,
}

#[derive(Debug, Clone)]
pub enum Message {
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
            _lock: lock,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
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
            self.pending = Some((Instant::now() + Duration::from_secs(3), command));
        } else {
            self.dispatch(command);
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_millis(250)).map(|_| Message::Tick)
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
                "Sit back · {}",
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_secs_f32()
                    .ceil() as u32
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
            button(text("Open dashboard").size(14))
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
            text("F13 recenter 98 cm / F14 VR click | Ctrl+Alt: F8 height / F9 undo / F7 dashboard")
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
