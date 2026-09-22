use iced::{
    Element, Fill, Task,
    widget::{
        Space, button, checkbox, column, container, pick_list, row, scrollable, slider, text,
    },
};
use rig_companion::wind::{self, RunMode, Settings, Snapshot, Worker};
use rig_companion::wind_curve::{Curve, MAX_POINTS, Preset};
use std::time::Instant;

pub struct State {
    worker: Worker,
    draft: Settings,
    applied: Settings,
    snapshot: Snapshot,
    demo: bool,
    load_failed: bool,
    notice: String,
    selected_point: Option<usize>,
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
    Smoothing(f64),
    Preset(Preset),
    SelectPoint(usize),
    MovePoint(usize, f64),
    AddPoint(f64, f64),
    AddPointAuto,
    RemovePoint,
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
            selected_point: None,
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
            Message::Smoothing(value) => self.draft.curve.smoothing = value,
            Message::Preset(preset) => {
                self.draft.curve = preset.curve();
                self.selected_point = None;
            }
            Message::SelectPoint(i) => self.selected_point = Some(i),
            Message::MovePoint(i, y) => self.draft.curve.move_point(i, y),
            Message::AddPoint(x, y) => {
                if let Some(i) = self.draft.curve.insert(x, y) {
                    self.selected_point = Some(i);
                }
            }
            Message::AddPointAuto => {
                if let Some(i) = self.draft.curve.add_in_largest_gap() {
                    self.selected_point = Some(i);
                }
            }
            Message::RemovePoint => {
                if let Some(i) = self.selected_point
                    && self.draft.curve.remove(i)
                {
                    self.selected_point = None;
                }
            }
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
        let preset = Preset::ALL
            .into_iter()
            .find(|p| p.curve() == self.draft.curve);
        let selected_detail = self
            .selected_point
            .and_then(|i| self.draft.curve.points.get(i).map(|p| (i, p)))
            .map(|(i, p)| {
                format!(
                    "Point {} · {:.0} km/h · {:.1}% fan{}",
                    i + 1,
                    p.x * f64::from(estimate.full_speed_kmh()),
                    f64::from(self.draft.minimum)
                        + p.y * f64::from(self.draft.maximum - self.draft.minimum),
                    if self.draft.curve.movable(i) {
                        ""
                    } else {
                        " · endpoint uses min/max"
                    }
                )
            })
            .unwrap_or_else(|| "Select a point to see its speed and fan setting.".into());
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
            row![text("Curve preset").size(14), pick_list(Preset::ALL, preset, Message::Preset).placeholder("Custom").width(Fill)].spacing(14),
            text(format!("Smoothing · {:.0}%", self.draft.curve.smoothing*100.)).size(14),
            slider(0.0..=1.0, self.draft.curve.smoothing, Message::Smoothing).step(0.05),
            text("0%: straight segments · 100%: rounded transitions through every point").size(12),
            iced::widget::canvas(CurvePlot { minimum:self.draft.minimum, maximum:self.draft.maximum, curve:&self.draft.curve, selected:self.selected_point, full_speed:estimate.full_speed_kmh() }).width(Fill).height(220),
            row![button("Add point").on_press_maybe((self.draft.curve.points.len()<MAX_POINTS).then_some(Message::AddPointAuto)), button("Remove selected").on_press_maybe(self.selected_point.filter(|&i| self.draft.curve.movable(i)).map(|_| Message::RemovePoint)), text(format!("{} / {} points", self.draft.curve.points.len(), MAX_POINTS)).size(12)].spacing(12),
            text(selected_detail).size(13),
            text(if self.draft.minimum == self.draft.maximum { "Minimum equals maximum: airflow is constant. Lower minimum to edit point heights." } else { "Click empty space to add a point; drag points up/down. Endpoints follow min/max. Changes take effect on Apply." }).size(12),
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

struct CurvePlot<'a> {
    minimum: u16,
    maximum: u16,
    curve: &'a Curve,
    selected: Option<usize>,
    full_speed: u16,
}
#[derive(Default)]
struct CurveDrag {
    point: Option<usize>,
}
impl CurvePlot<'_> {
    fn plot(bounds: iced::Rectangle) -> iced::Rectangle {
        iced::Rectangle {
            x: 32.,
            y: 12.,
            width: (bounds.width - 64.).max(1.),
            height: (bounds.height - 42.).max(1.),
        }
    }
    fn position(
        &self,
        bounds: iced::Rectangle,
        p: rig_companion::wind_curve::Point,
    ) -> iced::Point {
        let r = Self::plot(bounds);
        let y = (f64::from(self.minimum) + p.y * f64::from(self.maximum - self.minimum)) / 100.;
        iced::Point::new(r.x + p.x as f32 * r.width, r.y + (1. - y as f32) * r.height)
    }
    fn hit(&self, bounds: iced::Rectangle, cursor: iced::Point) -> Option<usize> {
        self.curve
            .points
            .iter()
            .enumerate()
            .map(|(i, &p)| (i, self.position(bounds, p).distance(cursor)))
            .filter(|&(_, d)| d <= 11.)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }
    fn height(&self, bounds: iced::Rectangle, cursor: iced::Point) -> f64 {
        let r = Self::plot(bounds);
        ((f64::from(1. - (cursor.y - r.y) / r.height) * 100. - f64::from(self.minimum))
            / f64::from((self.maximum - self.minimum).max(1)))
        .clamp(0., 1.)
    }
}
impl iced::widget::canvas::Program<Message> for CurvePlot<'_> {
    type State = CurveDrag;
    fn update(
        &self,
        state: &mut CurveDrag,
        event: &iced::widget::canvas::Event,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<iced::widget::Action<Message>> {
        use iced::{
            mouse,
            widget::{Action, canvas::Event},
        };
        match event {
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Window(iced::window::Event::Unfocused) => {
                return state
                    .point
                    .take()
                    .map(|_| Action::request_redraw().and_capture());
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.point.is_some() => {
                let p = cursor.position()? - iced::Vector::new(bounds.x, bounds.y);
                return Some(
                    Action::publish(Message::MovePoint(
                        state.point.unwrap(),
                        self.height(bounds, p),
                    ))
                    .and_capture(),
                );
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_in(bounds)?;
                if let Some(i) = self.hit(bounds, p) {
                    state.point =
                        (self.curve.movable(i) && self.maximum > self.minimum).then_some(i);
                    return Some(Action::publish(Message::SelectPoint(i)).and_capture());
                }
                let r = Self::plot(bounds);
                if r.contains(p)
                    && self.maximum > self.minimum
                    && self.curve.points.len() < MAX_POINTS
                {
                    let x = f64::from((p.x - r.x) / r.width);
                    if self
                        .curve
                        .points
                        .iter()
                        .all(|p| (p.x - x).abs() >= rig_companion::wind_curve::MIN_GAP)
                    {
                        state.point = Some(self.curve.points.partition_point(|p| p.x < x));
                        return Some(
                            Action::publish(Message::AddPoint(x, self.height(bounds, p)))
                                .and_capture(),
                        );
                    }
                }
            }
            _ => {}
        }
        None
    }
    fn mouse_interaction(
        &self,
        state: &CurveDrag,
        bounds: iced::Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        if state.point.is_some() {
            iced::mouse::Interaction::Grabbing
        } else if cursor
            .position_in(bounds)
            .and_then(|p| self.hit(bounds, p))
            .is_some_and(|i| self.curve.movable(i) && self.maximum > self.minimum)
        {
            iced::mouse::Interaction::Grab
        } else if cursor
            .position_in(bounds)
            .is_some_and(|p| Self::plot(bounds).contains(p))
        {
            iced::mouse::Interaction::Crosshair
        } else {
            iced::mouse::Interaction::default()
        }
    }
    fn draw(
        &self,
        _: &CurveDrag,
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
        let r = Self::plot(bounds);
        let w = r.width;
        let h = r.height;
        let point = |x: f32, y: f32| Point::new(r.x + x * w, r.y + (1.0 - y) * h);
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
            for i in 0..=300 {
                let x = i as f32 / 300.0;
                let y = (self.minimum as f64
                    + (self.maximum - self.minimum) as f64 * self.curve.sample(x as f64))
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
        for (i, &p) in self.curve.points.iter().enumerate() {
            let center = self.position(bounds, p);
            let selected = self.selected == Some(i);
            frame.fill(
                &Path::circle(center, if selected { 7.5 } else { 5.5 }),
                if self.curve.movable(i) {
                    Color::from_rgb8(181, 155, 255)
                } else {
                    Color::from_rgb8(145, 145, 155)
                },
            );
            if selected {
                frame.stroke(
                    &Path::circle(center, 10.),
                    Stroke::default().with_color(Color::WHITE).with_width(1.5),
                );
            }
        }
        for (label, p) in [
            ("100%".to_string(), Point::new(0.0, 0.0)),
            ("0".into(), Point::new(16.0, h + 6.)),
            ("0 km/h".into(), Point::new(32.0, h + 22.0)),
            (
                format!("{} km/h", self.full_speed),
                Point::new(32.0 + w - 50.0, h + 22.0),
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

#[cfg(test)]
mod curve_editor_tests {
    use super::*;
    use iced::{
        Point, Rectangle, Vector, mouse,
        widget::canvas::{Event, Program},
    };

    #[test]
    fn dragging_is_vertical_clamped_and_released_outside_canvas() {
        let curve = Preset::Linear.curve();
        let plot = CurvePlot {
            minimum: 35,
            maximum: 80,
            curve: &curve,
            selected: None,
            full_speed: 235,
        };
        let bounds = Rectangle {
            x: 50.,
            y: 120.,
            width: 480.,
            height: 220.,
        };
        let offset = Vector::new(bounds.x, bounds.y);
        let start = plot.position(bounds, curve.points[2]) + offset;
        let mut state = CurveDrag::default();
        let action = plot
            .update(
                &mut state,
                &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(start),
            )
            .unwrap();
        assert!(matches!(
            action.into_inner().0,
            Some(Message::SelectPoint(2))
        ));
        let outside = Point::new(-50., -100.);
        let action = plot
            .update(
                &mut state,
                &Event::Mouse(mouse::Event::CursorMoved { position: outside }),
                bounds,
                mouse::Cursor::Available(outside),
            )
            .unwrap();
        assert!(matches!(action.into_inner().0,Some(Message::MovePoint(2,y)) if y==1.));
        plot.update(
            &mut state,
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert!(state.point.is_none());
        assert!(
            plot.update(
                &mut state,
                &Event::Mouse(mouse::Event::CursorMoved { position: start }),
                bounds,
                mouse::Cursor::Available(start)
            )
            .is_none()
        );
    }
    #[test]
    fn click_adds_at_cursor_and_endpoints_cannot_drag() {
        let curve = Preset::Linear.curve();
        let plot = CurvePlot {
            minimum: 20,
            maximum: 80,
            curve: &curve,
            selected: None,
            full_speed: 235,
        };
        let bounds = Rectangle {
            x: 80.,
            y: 100.,
            width: 480.,
            height: 220.,
        };
        let offset = Vector::new(bounds.x, bounds.y);
        let mut state = CurveDrag::default();
        let p = plot.position(bounds, rig_companion::wind_curve::Point { x: 0.4, y: 0.7 }) + offset;
        let action = plot
            .update(
                &mut state,
                &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(p),
            )
            .unwrap();
        assert!(
            matches!(action.into_inner().0,Some(Message::AddPoint(x,y)) if (x-0.4).abs()<1e-6 && (y-0.7).abs()<1e-6)
        );
        plot.update(
            &mut state,
            &Event::Window(iced::window::Event::Unfocused),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert!(state.point.is_none());
        let p = plot.position(bounds, curve.points[0]) + offset;
        let action = plot
            .update(
                &mut state,
                &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(p),
            )
            .unwrap();
        assert!(matches!(
            action.into_inner().0,
            Some(Message::SelectPoint(0))
        ));
        assert!(state.point.is_none());
    }
}
