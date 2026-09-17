use iced::{Color, Point, Rectangle, Renderer, Theme, mouse, widget::canvas};
use rig_companion::distortion::Profile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Mapping,
    Chromatic,
}
impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Mapping => "Radial mapping",
            Self::Chromatic => "Color correction",
        })
    }
}
pub struct Plot {
    pub lines: Vec<(Vec<[f32; 2]>, Color)>,
    pub mode: Mode,
}
impl Plot {
    pub fn new(
        profile: &Profile,
        comparison: Option<&Profile>,
        mode: Mode,
    ) -> Result<Self, String> {
        let mut lines = Vec::new();
        let keys: &[(&str, Color)] = match mode {
            Mode::Mapping => &[("distortions", Color::from_rgb8(235, 235, 239))],
            Mode::Chromatic => &[
                ("distortionsRed", Color::from_rgb8(241, 166, 96)),
                ("distortionsBlue", Color::from_rgb8(180, 147, 238)),
            ],
        };
        if let Some(other) = comparison {
            for (key, _) in keys {
                if let Ok(points) = other.points(key) {
                    lines.push((points, Color::from_rgb8(104, 104, 114)));
                }
            }
        }
        let mut available = false;
        for (key, color) in keys {
            let points = profile.points(key)?;
            available |= !points.is_empty();
            lines.push((points, *color));
        }
        if !available {
            return Err("No points published for this view.".into());
        }
        Ok(Self { lines, mode })
    }
}
impl<Message> canvas::Program<Message> for Plot {
    type State = ();
    fn update(
        &self,
        _: &mut (),
        event: &canvas::Event,
        _: Rectangle,
        _: mouse::Cursor,
    ) -> Option<iced::widget::Action<Message>> {
        matches!(
            event,
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft)
        )
        .then(iced::widget::Action::request_redraw)
    }
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut f = canvas::Frame::new(renderer, bounds.size());
        let left = 58.;
        let top = 20.;
        let width = (bounds.width - 82.).max(1.);
        let height = (bounds.height - 60.).max(1.);
        let mut xmax = 1f32;
        let mut ymin = 0f32;
        let mut ymax = 0f32;
        for (pts, _) in &self.lines {
            for p in pts {
                xmax = xmax.max(p[0]);
                ymin = ymin.min(p[1]);
                ymax = ymax.max(p[1]);
            }
        }
        if ymax - ymin < 0.01 {
            ymax = ymin + 1.;
        }
        let pad = (ymax - ymin) * 0.07;
        ymin -= pad;
        ymax += pad;
        let project = |p: [f32; 2]| {
            Point::new(
                left + p[0] / xmax * width,
                top + height - (p[1] - ymin) / (ymax - ymin) * height,
            )
        };
        let muted = Color::from_rgb8(160, 160, 170);
        for i in 0..=4 {
            let t = i as f32 / 4.;
            let x = left + t * width;
            let y = top + t * height;
            for (a, b) in [
                (Point::new(x, top), Point::new(x, top + height)),
                (Point::new(left, y), Point::new(left + width, y)),
            ] {
                f.stroke(
                    &canvas::Path::line(a, b),
                    canvas::Stroke::default().with_color(Color::from_rgb8(49, 49, 55)),
                );
            }
            f.fill_text(canvas::Text {
                content: format!("{:.0}°", t * xmax),
                position: Point::new(x - 9., top + height + 8.),
                color: muted,
                size: 12.into(),
                ..Default::default()
            });
            f.fill_text(canvas::Text {
                content: format!("{:.1}", ymax - t * (ymax - ymin)),
                position: Point::new(4., y - 7.),
                color: muted,
                size: 12.into(),
                ..Default::default()
            });
        }
        for (points, color) in &self.lines {
            for pair in points.windows(2) {
                f.stroke(
                    &canvas::Path::line(project(pair[0]), project(pair[1])),
                    canvas::Stroke::default().with_color(*color).with_width(1.5),
                );
            }
            for p in points {
                f.fill(&canvas::Path::circle(project(*p), 3.), *color);
            }
        }
        if let Some(p) = cursor
            .position_in(bounds)
            .filter(|p| p.x >= left && p.x <= left + width && p.y >= top && p.y <= top + height)
        {
            f.stroke(
                &canvas::Path::line(Point::new(p.x, top), Point::new(p.x, top + height)),
                canvas::Stroke::default().with_color(muted),
            );
            f.fill_text(canvas::Text {
                content: format!(
                    "{:.1}° / {:.2}{}",
                    (p.x - left) / width * xmax,
                    ymax - (p.y - top) / height * (ymax - ymin),
                    if self.mode == Mode::Chromatic {
                        "%"
                    } else {
                        ""
                    }
                ),
                position: Point::new(left + 8., top + 4.),
                color: Color::WHITE,
                size: 13.into(),
                ..Default::default()
            });
        }
        vec![f.into_geometry()]
    }
}
