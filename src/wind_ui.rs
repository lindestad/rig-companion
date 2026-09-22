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
    FullSpeed(u16),
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
            Message::FullSpeed(value) => self.draft.full_speed_kmh = value,
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
        let settings = column![
            row![text("Airflow").size(23), Space::new().width(Fill), checkbox(self.draft.enabled).label("Enable wind").on_toggle(Message::Enabled)].spacing(18),
            text("Run when").size(14),
            pick_list(RunMode::ALL, Some(self.draft.mode), Message::Mode).width(Fill),
            row![text(format!("Minimum · {}%", self.draft.minimum)), Space::new().width(Fill), text("Airflow when stationary / no game data").size(12)].spacing(12),
            slider(0..=100, self.draft.minimum, Message::Minimum).step(5u16),
            text(format!("Maximum · {}%", self.draft.maximum)),
            slider(5..=100, self.draft.maximum, Message::Maximum).step(5u16),
            text(format!("Reach maximum at {} km/h", self.draft.full_speed_kmh)),
            slider(10..=500, self.draft.full_speed_kmh, Message::FullSpeed).step(10u16),
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
