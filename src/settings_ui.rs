use iced::{
    Element, Fill, Task,
    widget::{
        button, checkbox, column, container, pick_list, row, scrollable, slider, text, text_input,
        tooltip,
    },
};
use rig_companion::driver_settings::{self, Settings};
use std::collections::BTreeMap;
#[derive(Debug, Clone)]
pub enum Message {
    Load,
    Loaded(Result<Settings, String>),
    Filter(bool),
    Search(String),
    Group(String),
    Edit(String, String),
    Reset(String),
    Apply,
    Applied(Result<(Settings, String), String>),
}
pub struct State {
    settings: Option<Settings>,
    changes: BTreeMap<String, Option<String>>,
    filtered: bool,
    search: String,
    group: String,
    busy: bool,
    status: String,
}
impl Default for State {
    fn default() -> Self {
        Self {
            settings: None,
            changes: BTreeMap::new(),
            filtered: true,
            search: String::new(),
            group: "Dream Air".into(),
            busy: false,
            status: "Load settings from the custom driver.".into(),
        }
    }
}
impl State {
    pub fn busy(&self) -> bool {
        self.busy
    }
    pub fn loaded(&self) -> bool {
        self.settings.is_some() || self.busy
    }
    pub fn update(&mut self, msg: Message, demo: bool) -> Task<Message> {
        match msg {
            Message::Load if !self.busy => {
                if demo {
                    self.status = "Unavailable in demo mode".into();
                    return Task::none();
                }
                self.busy = true;
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(Settings::load)
                            .await
                            .map_err(|e| e.to_string())?
                            .map_err(|e| format!("{e:#}"))
                    },
                    Message::Loaded,
                );
            }
            Message::Loaded(r) => {
                self.busy = false;
                match r {
                    Ok(s) => {
                        self.settings = Some(s);
                        self.changes.clear();
                        self.status =
                            "Loaded. Apply saves edits; Reset uses the driver default.".into();
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::Filter(v) => {
                self.filtered = v;
                if !self.groups().contains(&self.group) {
                    self.group = "Dream Air".into();
                }
            }
            Message::Search(v) => self.search = v,
            Message::Group(v) => self.group = v,
            Message::Edit(p, v) if !self.busy => {
                if self
                    .settings
                    .as_ref()
                    .and_then(|s| s.fields.iter().find(|f| f.path == p))
                    .is_some_and(|f| driver_settings::display(&f.value) == v)
                {
                    self.changes.remove(&p);
                } else {
                    self.changes.insert(p, Some(v));
                }
            }
            Message::Reset(p) if !self.busy => {
                self.changes.insert(p, None);
            }
            Message::Apply if !self.busy && !demo => {
                if let Some(s) = self.settings.clone() {
                    let changes = self.changes.clone();
                    self.busy = true;
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move||->anyhow::Result<_>{let backup=s.save(&changes)?;Ok((Settings::load()?,format!("Saved. Backup: {}. Some changes require a SteamVR restart.",backup.display()))) }).await.map_err(|e|e.to_string())?.map_err(|e|format!("{e:#}"))
                        },
                        Message::Applied,
                    );
                }
            }
            Message::Applied(r) => {
                self.busy = false;
                match r {
                    Ok((s, status)) => {
                        self.settings = Some(s);
                        self.changes.clear();
                        self.status = status;
                    }
                    Err(e) => self.status = e,
                }
            }
            _ => {}
        }
        Task::none()
    }
    fn groups(&self) -> Vec<String> {
        let mut g: Vec<_> = self
            .settings
            .as_ref()
            .map(|s| {
                s.fields
                    .iter()
                    .filter(|f| !self.filtered || driver_settings::relevant(&f.path))
                    .map(|f| f.group.clone())
                    .collect()
            })
            .unwrap_or_default();
        g.sort();
        g.dedup();
        g
    }
    pub fn view(&self) -> Element<'_, Message> {
        let muted = iced::Color::from_rgb8(164, 164, 173);
        let mut rows = column![].spacing(10);
        let query = self.search.to_lowercase();
        let mut count = 0;
        if let Some(s) = &self.settings {
            for f in s.fields.iter().filter(|f| {
                (!self.filtered || driver_settings::relevant(&f.path))
                    && if query.is_empty() {
                        f.group == self.group
                    } else {
                        format!("{} {} {}", f.group, f.label, f.path)
                            .to_lowercase()
                            .contains(&query)
                    }
            }) {
                count += 1;
                let input = match self.changes.get(&f.path) {
                    Some(Some(v)) => v.clone(),
                    Some(None) => driver_settings::display(&f.default),
                    None => driver_settings::display(&f.value),
                };
                let path = f.path.clone();
                let metadata = driver_settings::control(&f.path);
                let control: Element<'_, Message> = if f.default.is_boolean() {
                    checkbox(input == "true")
                        .label(if input == "true" { "On" } else { "Off" })
                        .on_toggle_maybe(
                            (!self.busy).then_some(move |v: bool| {
                                Message::Edit(path.clone(), v.to_string())
                            }),
                        )
                        .into()
                } else if let Some(meta) = metadata.filter(|m| !m.options.is_empty()) {
                    let selected = meta.options.iter().find(|c| c.value == input).cloned();
                    let mut options = meta.options.clone();
                    // Preserve a value added by a newer driver rather than replacing it.
                    let selected = selected.or_else(|| {
                        let current = driver_settings::Choice {
                            value: input.clone(),
                            label: input.clone(),
                        };
                        options.push(current.clone());
                        Some(current)
                    });
                    pick_list(options, selected, move |choice: driver_settings::Choice| {
                        Message::Edit(path.clone(), choice.value)
                    })
                    .width(250)
                    .into()
                } else if f.default.is_number() && metadata.is_some_and(|m| m.step.is_some()) {
                    let meta = metadata.unwrap();
                    let step = meta.step.unwrap();
                    let value = input.parse::<f64>().ok().filter(|n| n.is_finite());
                    let arrows = column![
                        button(text("▴").size(12))
                            .width(28)
                            .padding(0)
                            .style(button::secondary)
                            .on_press_maybe(value.filter(|_| !self.busy).map(|n| Message::Edit(
                                path.clone(),
                                driver_settings::number_text(n + step)
                            ))),
                        button(text("▾").size(12))
                            .width(28)
                            .padding(0)
                            .style(button::secondary)
                            .on_press_maybe(value.filter(|_| !self.busy).map(|n| Message::Edit(
                                path.clone(),
                                driver_settings::number_text(n - step)
                            )))
                    ];
                    let input_path = path.clone();
                    let number = container(
                        row![
                            text_input("Value", &input)
                                .padding(8)
                                .width(Fill)
                                .on_input_maybe(
                                    (!self.busy)
                                        .then_some(move |v| Message::Edit(input_path.clone(), v))
                                )
                                .style(|theme, status| {
                                    let mut style = text_input::default(theme, status);
                                    style.border.width = 0.0;
                                    style
                                }),
                            arrows
                        ]
                        .align_y(iced::Alignment::Center),
                    )
                    .style(|_| container::Style {
                        background: Some(iced::Color::from_rgb8(17, 17, 19).into()),
                        border: iced::border::rounded(5)
                            .width(1)
                            .color(iced::Color::from_rgb8(53, 53, 58)),
                        ..Default::default()
                    })
                    .padding(3)
                    .width(140);
                    let max = meta.max.or_else(|| {
                        meta.dynamic_fov.as_ref().map(|axis| {
                            let index = usize::from(axis == "y");
                            if f.path.starts_with("/dreamAir/") {
                                s.dream_air_fov_limits[index].unwrap_or([98.0, 88.0][index])
                            } else {
                                f.default.as_f64().unwrap_or(100.0)
                            }
                        })
                    });
                    if let (Some(min), Some(max), Some(n)) = (meta.min, max, value) {
                        let slider = slider(min..=max, n, move |v| {
                            Message::Edit(path.clone(), driver_settings::number_text(v))
                        })
                        .step(step)
                        .width(175);
                        row![slider, number]
                            .spacing(16)
                            .align_y(iced::Alignment::Center)
                            .into()
                    } else {
                        number.into()
                    }
                } else {
                    text_input("Value", &input)
                        .on_input_maybe(
                            (!self.busy).then_some(move |v| Message::Edit(path.clone(), v)),
                        )
                        .width(240)
                        .padding(10)
                        .into()
                };
                let mut title = row![text(&f.label).size(16)]
                    .spacing(8)
                    .align_y(iced::Alignment::Center);
                if let Some(meta) = metadata.filter(|m| !m.help.is_empty() || !m.restart.is_empty())
                {
                    let restart = match meta.restart.as_str() {
                        "steamvrRestart" => "Restart SteamVR to apply.",
                        "gameRestart" => "Restart the game to apply.",
                        _ => "",
                    };
                    let help = format!("{}\n{}", meta.help, restart).trim().to_owned();
                    title = title.push(
                        tooltip(
                            container(text("ⓘ").size(17).color(muted)).padding(3),
                            container(text(help).size(14))
                                .width(380)
                                .padding(12)
                                .style(container::rounded_box),
                            tooltip::Position::Bottom,
                        )
                        .gap(6),
                    );
                }
                let origin = if self.changes.contains_key(&f.path) {
                    "Pending"
                } else if f.overridden {
                    "Override"
                } else {
                    "Default"
                };
                let caption = driver_settings::validate(f, &input)
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| {
                        format!(
                            "{} · {} · default: {}",
                            f.group,
                            origin,
                            driver_settings::display(&f.default)
                        )
                    });
                rows = rows.push(
                    container(
                        row![
                            column![title, text(caption).size(12).color(muted)]
                                .spacing(5)
                                .width(Fill),
                            control,
                            button("Reset")
                                .style(button::secondary)
                                .on_press_maybe(
                                    (!self.busy
                                        && (f.overridden || self.changes.contains_key(&f.path)))
                                    .then_some(Message::Reset(f.path.clone()))
                                )
                                .padding(10)
                        ]
                        .spacing(16)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding(14)
                    .style(|_| container::Style {
                        background: Some(iced::Color::from_rgb8(25, 25, 28).into()),
                        border: iced::border::rounded(12),
                        ..Default::default()
                    }),
                );
            }
        }
        if count == 0 {
            rows = rows.push(text("No settings match this selection."));
        }
        column![text("Headset driver settings").size(34),text("Custom Headset / sboys3").color(muted),checkbox(self.filtered).label("Show only global and Dream Air settings").on_toggle(Message::Filter),row![pick_list(self.groups(),Some(self.group.clone()),Message::Group).width(235),text_input("Search all visible groups…",&self.search).on_input(Message::Search).padding(10)].spacing(14),row![button(if self.busy{"Working…"}else{"Apply changes"}).on_press_maybe((!self.busy&&!self.changes.is_empty()).then_some(Message::Apply)).padding(12).style(button::secondary),button(if self.changes.is_empty(){"Reload"}else{"Discard changes & reload"}).on_press_maybe((!self.busy).then_some(Message::Load)).padding(12).style(button::secondary),text(format!("{} settings · {} pending",count,self.changes.len())).color(muted)].spacing(12).align_y(iced::Alignment::Center),text(&self.status).size(14).color(muted),text("The driver reloads saved settings. SteamVR is never restarted automatically. Arrays use JSON notation.").size(12).color(muted),scrollable(rows).height(Fill)].spacing(14).into()
    }
}
