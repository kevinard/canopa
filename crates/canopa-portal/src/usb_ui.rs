//! Iced device consent. Only explicitly selected devices can be granted.
use crate::accessibility_ui::{self as a11y, Node, semantic};
use crate::dialog_ui;
use crate::{
    usb::{self, Grant, SelectedDevice, UsbRequest},
    worker::run as worker,
};
use iced::{
    Element, Event, Length, Size, Task, Theme, keyboard,
    widget::{checkbox, column, container, scrollable, text},
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
                size: Size::new(
                    640.0,
                    (240.0 + 107.0 * request.devices.len() as f32).clamp(360.0, 580.0),
                ),
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
            let (window, opening) = window::open_dialog(parent.to_owned(), true, settings);
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
                    window,
                    accessibility: Some(a11y::Accessibility::new("canopa-usb", WINDOW_KEY)),
                },
                Task::batch([opening.map(|_| Message::Opened), metadata]),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|_: &Dialog, _| window_title())
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(|app: &Dialog| {
        let input = iced::event::listen().map(Message::Event);
        match &app.accessibility {
            Some(state) => iced::Subscription::batch([input, state.bridge.subscription()]),
            None => input,
        }
    })
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("USB consent result").take())
}

/// The AT-SPI window of the device question and its title.
const WINDOW_KEY: &str = "usb";
fn window_title() -> String {
    fl!("portal-window-title", title = fl!("portal-usb-window"))
}

struct Dialog {
    request: UsbRequest,
    selected: Vec<bool>,
    writable: Vec<bool>,
    name: Option<String>,
    theme: Theme,
    completed: bool,
    output: Arc<Mutex<Option<Grant>>>,
    window: window::Id,
    accessibility: Option<a11y::Accessibility<Message>>,
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
    Accessibility(canopa_ui::accessibility::Event<Message>),
}

impl canopa_ui::accessibility::Message for Message {
    fn accessibility(event: canopa_ui::accessibility::Event<Self>) -> Self {
        Self::Accessibility(event)
    }
}

impl Dialog {
    /// Handle a message, then refresh the accessibility tree if one is read.
    fn update(&mut self, message: Message) -> Task<Message> {
        if let Message::Accessibility(event) = message {
            return a11y::update(&mut self.accessibility, event);
        }
        let task = self.apply(message);
        a11y::refresh(&mut self.accessibility, task)
    }
    fn exit(&mut self) -> Task<Message> {
        self.completed = true;
        if let Some(state) = &mut self.accessibility {
            state.close();
        }
        iced::exit()
    }
    fn apply(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Opened => {
                let window = self.window;
                if let Some(state) = &mut self.accessibility {
                    state.opened(&window_title(), window);
                }
            }
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
                return self.exit();
            }
            Message::Deny | Message::Event(Event::Window(window::Event::CloseRequested)) => {
                return self.exit();
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    return self.apply(Message::Deny);
                }
                if modifiers.alt()
                    && let keyboard::Key::Character(value) = key
                {
                    match value.as_str() {
                        "s" | "S" => return self.apply(Message::SelectAll),
                        "a" | "A" => return self.apply(Message::Allow),
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
        let requester = self.name.clone().unwrap_or_else(|| {
            if self.request.app_id.is_empty() {
                fl!("portal-requesting-app")
            } else {
                self.request.app_id.clone()
            }
        });
        let question = fl!("portal-usb-question");
        let mut body = column![
            semantic(
                Node::heading(question.clone(), 1),
                dialog_ui::heading(question.clone())
            ),
            dialog_ui::detail(fl!("portal-requested-by", requester = requester.as_str()))
        ]
        .spacing(12);
        body = body.push(text(fl!("portal-usb-choose")).size(14));
        let selected = self.selected.iter().filter(|selected| **selected).count();
        let count = fl!(
            "portal-usb-selected",
            count = selected,
            total = self.selected.len()
        );
        let all = selected == self.selected.len();
        let select_all = if all {
            fl!("portal-usb-clear-selection")
        } else {
            fl!("portal-usb-select-all")
        };
        let select_all_button = if all {
            fl!("portal-usb-clear-selection-shortcut")
        } else {
            fl!("portal-usb-select-all-shortcut")
        };
        body = body.push(
            canopa_ui::direction::row([
                semantic(
                    Node::status(&count).key("usb-selection"),
                    dialog_ui::section(count.clone()).width(Length::Fill),
                ),
                semantic(
                    Node::button(select_all)
                        .key("usb-select-all")
                        .shortcut("Alt+S")
                        .on_click(Some(Message::SelectAll)),
                    canopa_ui::action(
                        text(select_all_button),
                        Some(Message::SelectAll),
                        canopa_ui::Action::Normal,
                        false,
                    ),
                ),
            ])
            .spacing(12)
            .align_y(iced::Center),
        );
        let mut devices = column![].spacing(8);
        for (index, device) in self.request.devices.iter().enumerate() {
            let key = format!("usb-device-{}", device.id.escape_default());
            let writable = self.writable[index];
            let access = container(
                text(if writable {
                    fl!("portal-usb-read-write")
                } else {
                    fl!("portal-usb-read-only")
                })
                .size(13),
            )
            .padding([4, 8])
            .style(move |theme| {
                let colors = canopa_ui::colors(theme);
                let tint = if writable {
                    colors.warning
                } else {
                    colors.accent
                };
                container::Style::default()
                    .background(tint.scale_alpha(0.19))
                    .border(
                        iced::Border::default()
                            .width(1)
                            .rounded(5)
                            .color(tint.scale_alpha(0.55)),
                    )
            });
            let mut information = column![
                canopa_ui::direction::row([
                    a11y::check_box(
                        format!("{key}-selected"),
                        &device.name,
                        self.selected[index],
                        Some(Message::Selected(index, !self.selected[index])),
                        checkbox(self.selected[index])
                            .label(&device.name)
                            .style(canopa_ui::checkbox_style)
                            .on_toggle(move |value| Message::Selected(index, value)),
                    ),
                    iced::widget::space().width(Length::Fill).into(),
                    access.into(),
                ])
                .spacing(8)
                .align_y(iced::Center)
            ]
            .spacing(6);
            let hardware = [
                (!device.vendor.is_empty()).then(|| device.vendor.clone()),
                (!device.hardware_id.is_empty()).then(|| device.hardware_id.clone()),
                (!device.serial.is_empty())
                    .then(|| fl!("portal-usb-serial", serial = device.serial.as_str())),
                (!device.location.is_empty())
                    .then(|| fl!("portal-usb-location", location = device.location.as_str())),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
            if !hardware.is_empty() {
                information = information.push(dialog_ui::detail(hardware).size(12));
            }
            if device.writable {
                let selected = self.selected[index];
                let mut control = checkbox(!writable)
                    .label(fl!("portal-usb-use-read-only"))
                    .style(canopa_ui::checkbox_style);
                if selected {
                    control = control.on_toggle(move |value| Message::ReadOnly(index, value));
                }
                information = information.push(
                    container(a11y::check_box(
                        format!("{key}-read-only"),
                        fl!("portal-usb-use-read-only"),
                        !self.writable[index],
                        selected.then_some(Message::ReadOnly(index, writable)),
                        control,
                    ))
                    .padding([2, 22]),
                );
            }
            devices = devices.push(semantic(
                Node::group(&device.name).key(key),
                container(information)
                    .padding(12)
                    .width(Length::Fill)
                    .style(dialog_ui::card),
            ));
        }
        if self.request.devices.iter().any(|device| device.writable) {
            body = body.push(dialog_ui::detail(fl!("portal-usb-read-only-hint")).size(13));
        }
        body = body.push(
            scrollable(devices)
                .style(canopa_ui::scrollbar)
                .spacing(5)
                .height(Length::Fill),
        );
        let allow = (selected != 0).then_some(Message::Allow);
        let actions = dialog_ui::footer([
            a11y::button(
                "usb-deny",
                fl!("portal-access-deny"),
                Some(Message::Deny),
                dialog_ui::action(
                    text(fl!("portal-access-deny")),
                    Some(Message::Deny),
                    canopa_ui::Action::Quiet,
                    false,
                ),
            ),
            semantic(
                Node::button(fl!("portal-usb-allow-selected"))
                    .key("usb-allow")
                    .shortcut("Alt+A")
                    .disabled(allow.is_none())
                    .on_click(allow.clone()),
                dialog_ui::action(
                    text(fl!("portal-usb-allow-selected-shortcut")),
                    allow,
                    canopa_ui::Action::Primary,
                    false,
                ),
            ),
        ]);
        a11y::dialog_window(
            WINDOW_KEY,
            window_title(),
            Node::alert_dialog(question)
                .key("usb-dialog")
                .description(fl!("portal-usb-description", requester = requester)),
            dialog_ui::frame(
                container(body)
                    .padding([20, 24])
                    .width(Length::Fill)
                    .height(Length::Fill),
                actions,
            ),
        )
    }
}
