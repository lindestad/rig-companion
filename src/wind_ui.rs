use iced::{
    Element, Fill, Task,
    widget::{
        Space, button, checkbox, column, container, pick_list, row, scrollable, slider, text,
    },
};
use rig_companion::wind::{self, RunMode, Settings, Snapshot, Worker};
use std::time::Instant;

pub struct State {
    worker: Worker,
    draft: Settings,
    applied: Settings,
    snapshot: Snapshot,
    demo: bool,
    load_failed: bool,
    notice: String,
}
#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Enabled(bool),
    Mode(RunMode),
    Minimum(u16),
    Maximum(u16),
    TopSpeed(u16),
    AutomaticCarSpeed(bool),
    Curve(f64),
    Port(String),
    Left(bool),
    Right(bool),
    Apply,
    Stop,
    Suspend,
}
impl State {
    pub fn new(demo: bool) -> Self {
        let (settings, notice, load_failed) = match Settings::load(&wind::settings_path(demo)) {
            Ok(s) => (s, String::new(), false),
            Err(e) => (
                Settings::default(),
                format!(
                    "Cannot load wind settings: {e}. Original file preserved; fix it before saving."
                ),
                true,
            ),
        };
        let worker = Worker::spawn(settings.clone(), demo);
        Self {
            snapshot: worker.snapshot(),
            worker,
            draft: settings.clone(),
            applied: settings,
            demo,
            load_failed,
            notice,
        }
    }
    pub fn suspend(&self) {
        self.worker.suspend(true);
    }
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => self.snapshot = self.worker.snapshot(),
            Message::Enabled(value) => self.draft.enabled = value,
            Message::Mode(value) => self.draft.mode = value,
            Message::Minimum(value) => {
                self.draft.minimum = if (1..5).contains(&value) { 5 } else { value };
                self.draft.maximum = self.draft.maximum.max(self.draft.minimum);
            }
            Message::Maximum(value) => {
                self.draft.maximum = value;
                self.draft.minimum = self.draft.minimum.min(value);
            }
            Message::TopSpeed(value) => self.draft.fallback_top_speed_kmh = value,
            Message::AutomaticCarSpeed(value) => self.draft.automatic_car_speed = value,
            Message::Curve(value) => self.draft.curve_exponent = value,
            Message::Port(value) => self.draft.port = (value != "Automatic").then_some(value),
            Message::Left(value) => self.draft.left_enabled = value,
            Message::Right(value) => self.draft.right_enabled = value,
            Message::Apply => {
                if !self.load_failed {
                    match self.draft.save(&wind::settings_path(self.demo)) {
                        Ok(()) => {
                            self.applied = self.draft.clone();
                            self.worker.configure(self.applied.clone());
                            self.notice = "Saved and applied.".into();
                        }
                        Err(e) => self.notice = format!("Not applied: {e}"),
                    }
                }
            }
            Message::Stop => {
                self.applied.enabled = false;
                self.draft.enabled = false;
                self.worker.configure(self.applied.clone());
                self.notice = "Wind disabled.".into();
                if !self.load_failed
                    && let Err(e) = self.applied.save(&wind::settings_path(self.demo))
                {
                    self.notice = format!("Stopped, but could not save: {e}");
                }
            }
            Message::Suspend => self.worker.suspend(!self.snapshot.suspended),
        }
        Task::none()
    }
    pub fn view(&self) -> Element<'_, Message> {
        let s = &self.snapshot;
        let fresh = s.fresh();
        let telemetry = s.telemetry.as_ref().filter(|_| fresh);
        let mut ports = vec!["Automatic".to_owned()];
        ports.extend(s.ports.clone());
        if let Some(port) = &self.draft.port
            && !ports.contains(port)
        {
            ports.push(port.clone());
        }
        let chosen = self
            .draft
            .port
            .clone()
            .unwrap_or_else(|| "Automatic".into());
        let estimate = s.estimate(&self.draft);
        let active_estimate = s.estimate(&self.applied);
        let settings = column![
            row![text("Airflow").size(23), Space::new().width(Fill), checkbox(self.draft.enabled).label("Enable wind").on_toggle(Message::Enabled)].spacing(18),
            text("Run when").size(14),
            pick_list(RunMode::ALL, Some(self.draft.mode), Message::Mode).width(Fill),
            row![text(format!("Minimum · {}%", self.draft.minimum)), Space::new().width(Fill), text("Airflow when stationary / no game data").size(12)].spacing(12),
            slider(0..=100, self.draft.minimum, Message::Minimum).step(5u16),
            text(format!("Maximum · {}%", self.draft.maximum)),
            slider(5..=100, self.draft.maximum, Message::Maximum).step(5u16),
            checkbox(self.draft.automatic_car_speed).label("Use active iRacing car's estimated top speed").on_toggle(Message::AutomaticCarSpeed),
            text(format!("{} · {} km/h", if self.draft.automatic_car_speed { "Fallback top speed (unknown car)" } else { "Manual top speed" }, self.draft.fallback_top_speed_kmh)).size(14),
            slider(25..=500, self.draft.fallback_top_speed_kmh, Message::TopSpeed).step(5u16),
            text(format!("Maximum fan setting at {} km/h (top speed − 15)", estimate.full_speed_kmh())).size(14),
            text(format!("Curve shape · {:.2}{}", self.draft.curve_exponent, if (self.draft.curve_exponent - 1.0).abs() < 0.001 { " · linear" } else if self.draft.curve_exponent < 1.0 { " · stronger at low speeds" } else { " · gentler at low speeds" })).size(14),
            slider(0.25..=2.0, self.draft.curve_exponent, Message::Curve).step(0.05),
            iced::widget::canvas(CurvePlot { minimum:self.draft.minimum, maximum:self.draft.maximum, exponent:self.draft.curve_exponent, full_speed:estimate.full_speed_kmh() }).width(Fill).height(155),
            text(if self.draft.minimum == self.draft.maximum { "Minimum equals maximum: airflow is constant. Lower minimum to use the curve." } else { "Lower curve values give more wind earlier. 1.00 is linear; 0.60 is the default." }).size(12),
            row![checkbox(self.draft.left_enabled).label("Left · L1 + L2").on_toggle(Message::Left), checkbox(self.draft.right_enabled).label("Right · R1 + R2").on_toggle(Message::Right)].spacing(24),
            text("Both enabled sides follow vehicle speed. Each startup includes a brief full-speed kick.").size(12),
            row![button("Apply changes").on_press_maybe((!self.load_failed).then_some(Message::Apply)), button("Stop now").on_press(Message::Stop), text(if self.draft != self.applied { "Unsaved changes" } else { "Settings saved" }).size(12)].spacing(14),
            text(&self.notice).size(13),
        ].spacing(15);
        let status = column![
            text("Controller").size(23),
            text(&s.connection).size(17),
            pick_list(ports, Some(chosen), Message::Port).width(Fill),
            text(
                telemetry
                    .map(|t| t.mode_label())
                    .unwrap_or("Waiting for fresh telemetry")
            )
            .size(19),
            text(format!(
                "Requested airflow · left {}% / right {}%",
                s.demand[0] / 10,
                s.demand[1] / 10
            ))
            .size(14),
            text(format!(
                "SteamVR: {}    iRacing simulator: {}",
                if s.steamvr { "running" } else { "closed" },
                if s.iracing { "running" } else { "closed" }
            ))
            .size(13),
            text(if s.bridge_fresh() {
                if s.bridge_running {
                    format!("SimHub connected · {:.0} km/h", s.speed_kmh)
                } else {
                    "SimHub connected · no active game".into()
                }
            } else {
                "Waiting for SimHub · minimum airflow inside the selected run mode".into()
            })
            .size(14),
            text(format!(
                "Car · {}",
                if active_estimate.car.is_empty() {
                    "Not identified"
                } else {
                    &active_estimate.car
                }
            ))
            .size(16),
            text(format!(
                "Estimated top speed · {} km/h\nMaximum fan setting · {} km/h",
                active_estimate.top_speed_kmh,
                active_estimate.full_speed_kmh()
            ))
            .size(14),
            text(active_estimate.basis).size(12),
            text(s.error.as_deref().unwrap_or("")).size(13),
            text(s.bridge_error.as_deref().unwrap_or("")).size(13),
            button(if s.suspended {
                "Reconnect controller"
            } else {
                "Release USB for firmware update"
            })
            .on_press(Message::Suspend),
            text("Releasing USB stops airflow. Reconnecting resumes your saved run mode.").size(12),
        ]
        .spacing(15);
        let mut fans = row![].spacing(12);
        for (i, name) in wind::HEADERS.iter().enumerate() {
            let rpm = telemetry
                .map(|t| format!("{}", t.rpm[i]))
                .unwrap_or_else(|| "—".into());
            let detail = telemetry
                .map(|t| {
                    if t.warnings & (1 << i) != 0 {
                        "No tach · empty / stalled"
                    } else if t.demand[i] == 0 {
                        "Not commanded"
                    } else if t.rpm[i] > 0 {
                        "Tach detected"
                    } else {
                        "Starting / awaiting tach"
                    }
                })
                .unwrap_or("No fresh reading");
            fans = fans.push(
                container(
                    column![
                        text(*name).size(16),
                        text(rpm).size(32),
                        text("RPM").size(12),
                        text(detail).size(12)
                    ]
                    .spacing(8),
                )
                .padding(18)
                .width(Fill)
                .style(container::rounded_box),
            );
        }
        let details = telemetry
            .map(|t| {
                format!(
                    "Uptime {}m {}s · accepted commands {} · rejected {} · last reading {:.1}s ago",
                    t.uptime_ms / 60000,
                    t.uptime_ms / 1000 % 60,
                    t.accepted,
                    t.rejected,
                    s.telemetry_at
                        .map(|t| Instant::now().duration_since(t).as_secs_f32())
                        .unwrap_or(0.0)
                )
            })
            .unwrap_or_else(|| {
                "RPM updates about once a second. Disconnected readings are marked stale.".into()
            });
        scrollable(column![
            text("Wind simulator").size(30),
            text("Speed-driven airflow from SimHub, with a minimum breeze when you want it.").size(15),
            row![container(settings).padding(20).width(Fill).style(container::rounded_box), container(status).padding(20).width(Fill).style(container::rounded_box)].spacing(18),
            fans,
            text(details).size(13),
            text("RPM is estimated from tach pulses (2 per revolution). Voltage, current and temperature are not measured by this board.").size(12),
            text("Keep Rig Companion open or minimized. Closing it stops the fans. Enable ‘Rig Companion Wind Bridge’ in SimHub’s plugin list; no Custom Serial Device is needed.").size(13),
        ].spacing(18).padding(2)).into()
    }
}

struct CurvePlot {
    minimum: u16,
    maximum: u16,
    exponent: f64,
    full_speed: u16,
}
impl iced::widget::canvas::Program<Message> for CurvePlot {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &iced::Renderer,
        _: &iced::Theme,
        bounds: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        use iced::{
            Color, Point,
            widget::canvas::{Frame, Path, Stroke, Text},
        };
        let mut frame = Frame::new(renderer, bounds.size());
        let w = (bounds.width - 64.0).max(1.0);
        let h = (bounds.height - 36.0).max(1.0);
        let point = |x: f32, y: f32| Point::new(32.0 + x * w, 8.0 + (1.0 - y) * h);
        for i in 0..=4 {
            let t = i as f32 / 4.0;
            frame.stroke(
                &Path::line(point(t, 0.0), point(t, 1.0)),
                Stroke::default().with_color(Color::from_rgb8(60, 60, 67)),
            );
            frame.stroke(
                &Path::line(point(0.0, t), point(1.0, t)),
                Stroke::default().with_color(Color::from_rgb8(60, 60, 67)),
            );
        }
        frame.stroke(
            &Path::line(
                point(0.0, self.minimum as f32 / 100.0),
                point(1.0, self.maximum as f32 / 100.0),
            ),
            Stroke::default().with_color(Color::from_rgb8(110, 110, 120)),
        );
        let path = Path::new(|b| {
            for i in 0..=100 {
                let x = i as f32 / 100.0;
                let y = (self.minimum as f64
                    + (self.maximum - self.minimum) as f64 * (x as f64).powf(self.exponent))
                    as f32
                    / 100.0;
                if i == 0 {
                    b.move_to(point(x, y));
                } else {
                    b.line_to(point(x, y));
                }
            }
        });
        frame.stroke(
            &path,
            Stroke::default()
                .with_color(Color::from_rgb8(181, 155, 255))
                .with_width(2.5),
        );
        for (label, p) in [
            ("100%".to_string(), Point::new(0.0, 0.0)),
            ("0".into(), Point::new(16.0, h)),
            ("0 km/h".into(), Point::new(32.0, h + 14.0)),
            (
                format!("{} km/h", self.full_speed),
                Point::new(32.0 + w - 50.0, h + 14.0),
            ),
        ] {
            frame.fill_text(Text {
                content: label,
                position: p,
                color: Color::from_rgb8(190, 190, 200),
                size: 11.0.into(),
                ..Text::default()
            });
        }
        vec![frame.into_geometry()]
    }
}
