//! Iced device consent. Only explicitly selected devices can be granted.
use crate::{
    usb::{self, Grant, SelectedDevice, UsbRequest},
    worker::run as worker,
};
use iced::{
    Element, Event, Length, Size, Task, Theme, keyboard,
    widget::{checkbox, column, container, row, scrollable, text},
    window,
};
use std::sync::{Arc, Mutex};

pub fn run(
    request: UsbRequest,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<Grant>> {
    anyhow::ensure!(request.valid(), "Invalid USB consent request");
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(680.0, 600.0),
                min_size: Some(Size::new(440.0, 360.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let parent = request
                .parent_window
                .strip_prefix("wayland:")
                .unwrap_or_default();
            let (_, opening) = window::open_dialog(parent.to_owned(), true, settings);
            let app_id = request.app_id.clone();
            let metadata = Task::perform(
                worker(move || {
                    canopa_apps::discover_choice_metadata()
                        .ok()?
                        .into_iter()
                        .find(|app| {
                            app.id == app_id
                                || app.id.strip_suffix(".desktop") == Some(app_id.as_str())
                        })
                        .map(|app| usb::label(&app.name).chars().take(256).collect::<String>())
                        .filter(|name| !name.is_empty())
                }),
                |result| Message::Name(result.ok().flatten()),
            );
            (
                Dialog {
                    selected: vec![false; request.devices.len()],
                    writable: request
                        .devices
                        .iter()
                        .map(|device| device.writable)
                        .collect(),
                    request: request.clone(),
                    name: None,
                    theme: theme.clone(),
                    completed: false,
                    output: output.clone(),
                },
                Task::batch([opening.map(|_| Message::Opened), metadata]),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|_: &Dialog, _| "USB device access — Canopa".to_owned())
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(|_: &Dialog| iced::event::listen().map(Message::Event))
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("USB consent result").take())
}

struct Dialog {
    request: UsbRequest,
    selected: Vec<bool>,
    writable: Vec<bool>,
    name: Option<String>,
    theme: Theme,
    completed: bool,
    output: Arc<Mutex<Option<Grant>>>,
}
#[derive(Clone, Debug)]
enum Message {
    Opened,
    Name(Option<String>),
    Selected(usize, bool),
    ReadOnly(usize, bool),
    SelectAll,
    Allow,
    Deny,
    Event(Event),
}
impl Dialog {
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Opened => {}
            Message::Name(name) => self.name = name,
            Message::Selected(index, value) => {
                if let Some(selected) = self.selected.get_mut(index) {
                    *selected = value;
                }
            }
            Message::ReadOnly(index, value) => {
                if self
                    .request
                    .devices
                    .get(index)
                    .is_some_and(|device| device.writable)
                    && let Some(writable) = self.writable.get_mut(index)
                {
                    *writable = !value;
                }
            }
            Message::SelectAll => {
                let value = !self.selected.iter().all(|selected| *selected);
                self.selected.fill(value);
            }
            Message::Allow if self.selected.iter().any(|selected| *selected) => {
                *self.output.lock().expect("USB consent result") = Some(Grant {
                    devices: self
                        .request
                        .devices
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| self.selected[*index])
                        .map(|(index, device)| SelectedDevice {
                            id: device.id.clone(),
                            writable: self.writable[index],
                        })
                        .collect(),
                });
                self.completed = true;
                return iced::exit();
            }
            Message::Deny | Message::Event(Event::Window(window::Event::CloseRequested)) => {
                self.completed = true;
                return iced::exit();
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    return self.update(Message::Deny);
                }
                if modifiers.alt()
                    && let keyboard::Key::Character(value) = key
                {
                    match value.as_str() {
                        "s" | "S" => return self.update(Message::SelectAll),
                        "a" | "A" => return self.update(Message::Allow),
                        "r" | "R" => {
                            for (index, selected) in self.selected.iter().enumerate() {
                                if *selected {
                                    self.writable[index] = false;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let requester = self.name.as_deref().unwrap_or_else(|| {
            if self.request.app_id.is_empty() {
                "The requesting app"
            } else {
                &self.request.app_id
            }
        });
        let mut body = column![
            text("Allow USB device access?").size(24),
            text(requester).size(18)
        ]
        .spacing(12);
        if self.name.is_some() && !self.request.app_id.is_empty() {
            body = body.push(text(&self.request.app_id).size(12).style(canopa_ui::muted));
        }
        body = body.push(text("Choose the devices this app may access.").size(14));
        let selected = self.selected.iter().filter(|selected| **selected).count();
        body = body.push(
            row![
                text(format!("{selected} of {} selected", self.selected.len())).width(Length::Fill),
                canopa_ui::action(
                    text(if selected == self.selected.len() {
                        "Clear selection (Alt+S)"
                    } else {
                        "Select all (Alt+S)"
                    }),
                    Some(Message::SelectAll),
                    canopa_ui::Action::Quiet,
                    false
                )
            ]
            .spacing(12)
            .align_y(iced::Center),
        );
        let mut devices = column![].spacing(12);
        for (index, device) in self.request.devices.iter().enumerate() {
            let mut information = column![
                checkbox(self.selected[index])
                    .label(&device.name)
                    .style(canopa_ui::checkbox_style)
                    .on_toggle(move |value| Message::Selected(index, value)),
                text(if device.writable {
                    "Read-write access requested"
                } else {
                    "Read-only access requested"
                })
                .size(13)
                .style(canopa_ui::muted)
            ]
            .spacing(6);
            for detail in [&device.vendor, &device.hardware_id] {
                if !detail.is_empty() {
                    information = information.push(text(detail).size(13));
                }
            }
            if !device.serial.is_empty() {
                information = information.push(
                    text(format!("Serial: {}", device.serial))
                        .size(12)
                        .style(canopa_ui::muted),
                );
            }
            if !device.location.is_empty() {
                information = information.push(
                    text(format!("Location: {}", device.location))
                        .size(12)
                        .style(canopa_ui::muted),
                );
            }
            if device.writable {
                information = information.push(
                    checkbox(!self.writable[index])
                        .label("Use read-only access instead")
                        .style(canopa_ui::checkbox_style)
                        .on_toggle(move |value| Message::ReadOnly(index, value)),
                );
            }
            devices = devices.push(
                container(information)
                    .padding(12)
                    .width(Length::Fill)
                    .style(canopa_ui::surface),
            );
        }
        body = body.push(scrollable(devices).height(Length::Fill));
        body = body.push(
            text("Alt+R makes the selected devices read-only.")
                .size(12)
                .style(canopa_ui::muted),
        );
        body = body.push(
            row![
                canopa_ui::action(
                    text("Deny"),
                    Some(Message::Deny),
                    canopa_ui::Action::Quiet,
                    false
                )
                .width(Length::Fill),
                canopa_ui::action(
                    text("Allow selected (Alt+A)"),
                    (selected != 0).then_some(Message::Allow),
                    canopa_ui::Action::Normal,
                    false
                )
                .width(Length::Fill)
            ]
            .spacing(12),
        );
        container(body)
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(canopa_ui::surface)
            .into()
    }
}
