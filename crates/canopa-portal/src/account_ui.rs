//! A sharing draft lives only in this request. Only Share completes it.
#[cfg(test)]
mod tests;
use crate::dialog_ui;
use crate::{
    account::{AccountRequest, FIELD_BYTES, UserInformation},
    account_profile::{self, Avatar, Profile},
    chooser_ui::{self, Outcome, Picker, PickerOptions, PickerStart},
    file_chooser::Mode,
    worker::run as worker,
};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{checkbox, column, container, image, text, text_input},
    window,
};
use std::{
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub fn run(
    request: AccountRequest,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<UserInformation>> {
    let foreground = appearance.palette().text;
    let font = canopa_ui::font(&appearance);
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(680.0, 510.0),
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
            let source = request.clone();
            let load = Task::perform(
                worker(move || account_profile::load(&source, foreground)),
                Message::Loaded,
            );
            (
                Dialog::new(
                    request.clone(),
                    appearance.clone(),
                    foreground,
                    output.clone(),
                ),
                Task::batch([opening.map(|_| Message::Opened), load]),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|app: &Dialog, _| {
        let title = if matches!(app.view, View::Profile) {
            fl!("portal-account-window")
        } else {
            fl!("portal-account-image-window")
        };
        fl!("portal-window-title", title = title)
    })
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("account result").take())
}

enum View {
    Profile,
    PreparingPicker(u64),
    Picker(u64, Box<Picker>),
}

struct Dialog {
    request: AccountRequest,
    appearance: canopa_theme::Appearance,
    theme: Theme,
    foreground: canopa_theme::Rgb,
    output: Arc<Mutex<Option<UserInformation>>>,
    view: View,
    generation: u64,
    image_generation: u64,
    loading: bool,
    image_loading: bool,
    busy: bool,
    completed: bool,
    id: String,
    name: String,
    invalid_id: bool,
    invalid_name: bool,
    share_id: bool,
    share_name: bool,
    share_image: bool,
    avatar: Option<Avatar>,
    preview: Option<image::Handle>,
    pictures: PathBuf,
    requester: String,
    error: Option<String>,
}

#[derive(Clone, Debug)]
enum Message {
    Opened,
    Loaded(Result<Profile, String>),
    Id(String),
    Name(String),
    ShareId(bool),
    ShareName(bool),
    ShareImage(bool),
    ChooseImage,
    ClearImage,
    Back,
    PickerReady(u64, Result<Box<PickerStart>, String>),
    Picker(u64, chooser_ui::Message),
    Avatar(u64, Result<Avatar, String>),
    Share(u64),
    Shared(u64, Result<UserInformation, String>),
    Cancel,
    Event(Event),
}

impl Dialog {
    fn new(
        request: AccountRequest,
        appearance: canopa_theme::Appearance,
        foreground: canopa_theme::Rgb,
        output: Arc<Mutex<Option<UserInformation>>>,
    ) -> Self {
        let requester = if request.app_id.is_empty() {
            fl!("portal-requesting-app")
        } else {
            request.app_id.chars().take(120).collect()
        };
        Self {
            request,
            theme: canopa_ui::theme(appearance.clone()),
            appearance,
            foreground,
            output,
            view: View::Profile,
            generation: 0,
            image_generation: 0,
            loading: true,
            image_loading: false,
            busy: false,
            completed: false,
            id: String::new(),
            name: String::new(),
            invalid_id: false,
            invalid_name: false,
            share_id: true,
            share_name: true,
            share_image: false,
            avatar: None,
            preview: None,
            pictures: std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| PathBuf::from("/")),
            requester,
            error: None,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        if let View::Picker(generation, picker) = &self.view {
            return picker
                .subscription()
                .with(*generation)
                .map(|(generation, message)| Message::Picker(generation, message));
        }
        iced::event::listen_with(|event, status, _| {
            let escape = matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            );
            (escape || status == iced::event::Status::Ignored).then_some(Message::Event(event))
        })
    }

    fn editable(&self) -> bool {
        matches!(self.view, View::Profile) && !self.loading && !self.busy && !self.completed
    }

    fn can_share(&self) -> bool {
        self.editable()
            && !self.image_loading
            && (!self.share_id || !self.invalid_id)
            && (!self.share_name || !self.invalid_name)
    }

    fn install_avatar(&mut self, avatar: Option<Avatar>) {
        self.preview = avatar.as_ref().map(|avatar| {
            image::Handle::from_rgba(
                avatar.raster.width,
                avatar.raster.height,
                avatar.raster.pixels.clone(),
            )
        });
        self.share_image = avatar.is_some();
        self.avatar = avatar;
    }

    fn back(&mut self) -> Task<Message> {
        self.generation = self.generation.wrapping_add(1);
        self.view = View::Profile;
        Task::none()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Opened => {}
            Message::Loaded(result) if self.loading => {
                self.loading = false;
                self.generation = self.generation.wrapping_add(1);
                match result {
                    Ok(profile) => {
                        self.id = profile.id;
                        self.name = profile.name;
                        self.share_id = !self.id.trim().is_empty();
                        self.share_name = !self.name.trim().is_empty();
                        self.requester = profile.requester;
                        self.pictures = profile.pictures;
                        self.install_avatar(profile.avatar);
                    }
                    Err(_) => self.error = Some(fl!("portal-account-enter-information")),
                }
            }
            Message::Id(value) if self.editable() && self.share_id => {
                self.invalid_id = value.len() > FIELD_BYTES || value.contains('\0');
                if !self.invalid_id {
                    self.id = value;
                }
            }
            Message::Name(value) if self.editable() && self.share_name => {
                self.invalid_name = value.len() > FIELD_BYTES || value.contains('\0');
                if !self.invalid_name {
                    self.name = value;
                }
            }
            Message::ShareId(value) if self.editable() => {
                self.share_id = value;
                if value {
                    self.invalid_id = false;
                }
            }
            Message::ShareName(value) if self.editable() => {
                self.share_name = value;
                if value {
                    self.invalid_name = false;
                    if self.name.is_empty() {
                        return iced::widget::operation::focus("account-name");
                    }
                }
            }
            Message::ShareImage(value)
                if self.editable() && !self.image_loading && self.avatar.is_some() =>
            {
                self.share_image = value
            }
            Message::ClearImage if self.editable() => {
                self.image_generation = self.image_generation.wrapping_add(1);
                self.image_loading = false;
                self.generation = self.generation.wrapping_add(1);
                self.install_avatar(None);
                self.error = None;
            }
            Message::ChooseImage if self.editable() && !self.image_loading => {
                self.generation = self.generation.wrapping_add(1);
                let generation = self.generation;
                self.view = View::PreparingPicker(generation);
                self.error = None;
                let mut request = crate::file_chooser::parse_request(
                    Mode::Open,
                    self.request.app_id.clone(),
                    String::new(),
                    fl!("portal-account-image-window"),
                    Default::default(),
                )
                .expect("local picker request");
                request.accept_label = fl!("portal-account-select");
                let mut folder = self.pictures.as_os_str().as_bytes().to_vec();
                folder.push(0);
                request.current_folder = Some(folder);
                let filter = (
                    fl!("portal-account-images"),
                    [
                        "image/png",
                        "image/jpeg",
                        "image/gif",
                        "image/webp",
                        "image/svg+xml",
                    ]
                    .into_iter()
                    .map(|mime| (1, mime.into()))
                    .collect(),
                );
                request.filters = vec![filter];
                request.current_filter = request.filters.first().cloned();
                let appearance = self.appearance.clone();
                return Task::perform(
                    worker(move || chooser_ui::prepare_picker(request, appearance)),
                    move |result| {
                        Message::PickerReady(
                            generation,
                            result.and_then(|value| {
                                value
                                    .map(Box::new)
                                    .map_err(|_| fl!("portal-account-picker-failed"))
                            }),
                        )
                    },
                );
            }
            Message::PickerReady(generation, result) if matches!(self.view, View::PreparingPicker(current) if current == generation) => {
                match result {
                    Ok(start) => {
                        let (picker, task) = Picker::new(
                            *start,
                            PickerOptions {
                                read_only_choice: false,
                                retain_file_target: false,
                            },
                        );
                        self.view = View::Picker(generation, Box::new(picker));
                        return task.map(move |message| Message::Picker(generation, message));
                    }
                    Err(error) => {
                        self.error = Some(error);
                        return self.back();
                    }
                }
            }
            Message::Picker(generation, message) => {
                let View::Picker(current, picker) = &mut self.view else {
                    return Task::none();
                };
                if *current != generation {
                    return Task::none();
                }
                if matches!(
                    &message,
                    chooser_ui::Message::Event(Event::Window(window::Event::CloseRequested))
                ) {
                    return self.update(Message::Cancel);
                }
                let task = picker.update(message);
                match picker.take_outcome() {
                    Some(Outcome::Cancelled) => return self.back(),
                    Some(Outcome::Selected(selection)) => {
                        let _ = self.back();
                        let Some(path) = selection.paths.into_iter().next() else {
                            return Task::none();
                        };
                        let path = PathBuf::from(std::ffi::OsString::from_vec(path));
                        self.image_generation = self.image_generation.wrapping_add(1);
                        let generation = self.image_generation;
                        self.image_loading = true;
                        let foreground = self.foreground;
                        return Task::perform(
                            worker(move || account_profile::prepare_avatar(&path, foreground)),
                            move |result| {
                                Message::Avatar(
                                    generation,
                                    result.and_then(|value| {
                                        value.map_err(|_| fl!("portal-account-image-failed"))
                                    }),
                                )
                            },
                        );
                    }
                    None => return task.map(move |message| Message::Picker(generation, message)),
                }
            }
            Message::Back if !matches!(self.view, View::Profile) => return self.back(),
            Message::Avatar(generation, result)
                if self.image_loading && generation == self.image_generation =>
            {
                self.image_loading = false;
                self.generation = self.generation.wrapping_add(1);
                match result {
                    Ok(avatar) => {
                        self.install_avatar(Some(avatar));
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Message::Share(generation) if generation == self.generation && self.can_share() => {
                let information = UserInformation {
                    id: if self.share_id {
                        self.id.clone()
                    } else {
                        String::new()
                    },
                    name: if self.share_name {
                        self.name.clone()
                    } else {
                        String::new()
                    },
                    image: None,
                };
                let avatar = self.share_image.then(|| self.avatar.clone()).flatten();
                self.busy = true;
                self.error = None;
                return Task::perform(
                    worker(move || {
                        let image = avatar.as_ref().map(account_profile::publish).transpose()?;
                        Ok::<_, anyhow::Error>(UserInformation {
                            image,
                            ..information
                        })
                    }),
                    move |result| {
                        Message::Shared(
                            generation,
                            result.and_then(|value| {
                                value.map_err(|_| fl!("portal-account-image-prepare-failed"))
                            }),
                        )
                    },
                );
            }
            Message::Shared(generation, result) if self.busy && generation == self.generation => {
                self.busy = false;
                match result {
                    Ok(information) => {
                        *self.output.lock().expect("account result") = Some(information);
                        self.completed = true;
                        return iced::exit();
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Message::Cancel => {
                self.completed = true;
                return iced::exit();
            }
            Message::Event(Event::Window(window::Event::CloseRequested)) => {
                return self.update(Message::Cancel);
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    return self.update(if matches!(self.view, View::Profile) {
                        Message::Cancel
                    } else {
                        Message::Back
                    });
                }
                if key == keyboard::Key::Named(keyboard::key::Named::Tab) {
                    return if modifiers.shift() {
                        iced::widget::operation::focus_previous()
                    } else {
                        iced::widget::operation::focus_next()
                    };
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn view(&self, _: window::Id) -> Element<'_, Message> {
        match &self.view {
            View::Picker(generation, picker) => {
                let generation = *generation;
                return picker
                    .view()
                    .map(move |message| Message::Picker(generation, message));
            }
            View::PreparingPicker(_) => {
                return container(
                    column![
                        text(fl!("portal-account-preparing-picker")),
                        canopa_ui::action(
                            text(canopa_ui::direction::back_label()),
                            Some(Message::Back),
                            canopa_ui::Action::Normal,
                            false
                        )
                    ]
                    .spacing(16),
                )
                .padding(20)
                .height(Length::Fill)
                .width(Length::Fill)
                .style(dialog_ui::surface)
                .into();
            }
            View::Profile => {}
        }
        let enabled = self.editable();
        let mut content = column![
            dialog_ui::heading(fl!("portal-account-title")),
            dialog_ui::detail(fl!(
                "portal-requested-by",
                requester = self.requester.as_str()
            )),
        ]
        .spacing(12)
        .width(Length::Fill);
        if !self.request.reason.is_empty() {
            content = content.push(text(&self.request.reason));
        }
        if self.loading {
            content = content.push(text(fl!("portal-account-loading")).style(canopa_ui::muted));
        } else {
            let preview: Element<'_, Message> = match &self.preview {
                Some(handle) => image(handle.clone()).width(78).height(78).into(),
                None => container(text(fl!("portal-account-no-image")).style(canopa_ui::muted))
                    .center_x(78)
                    .center_y(78)
                    .style(dialog_ui::card)
                    .into(),
            };
            let mut image_actions = vec![
                dialog_ui::action(
                    text(fl!("portal-account-choose-image")),
                    (enabled && !self.image_loading).then_some(Message::ChooseImage),
                    canopa_ui::Action::Normal,
                    false,
                )
                .into(),
            ];
            if self.avatar.is_some() || self.image_loading {
                image_actions.push(
                    dialog_ui::action(
                        text(fl!("portal-account-clear-image")),
                        enabled.then_some(Message::ClearImage),
                        canopa_ui::Action::Quiet,
                        false,
                    )
                    .into(),
                );
            }
            let mut share_image = checkbox(self.share_image)
                .label(fl!("portal-account-share-image"))
                .style(canopa_ui::checkbox_style);
            if enabled && !self.image_loading && self.avatar.is_some() {
                share_image = share_image.on_toggle(Message::ShareImage);
            }
            let mut image_controls = column![
                canopa_ui::direction::row(image_actions)
                    .spacing(6)
                    .align_y(iced::Center)
            ]
            .spacing(8);
            if self.avatar.is_some() {
                image_controls = image_controls.push(share_image);
            }
            content = content.push(
                container(
                    canopa_ui::direction::row([preview, image_controls.into()])
                        .spacing(14)
                        .align_y(iced::Center),
                )
                .padding(12)
                .width(Length::Fill)
                .style(dialog_ui::card),
            );
            let mut share_id = checkbox(self.share_id)
                .label(fl!("portal-account-share-username"))
                .style(canopa_ui::checkbox_style);
            let mut share_name = checkbox(self.share_name)
                .label(fl!("portal-account-share-name"))
                .style(canopa_ui::checkbox_style);
            if enabled {
                share_id = share_id.on_toggle(Message::ShareId);
                share_name = share_name.on_toggle(Message::ShareName);
            }
            let username: Element<'_, Message> = if self.share_id {
                text_input(&fl!("portal-account-username"), &self.id)
                    .id("account-id")
                    .padding([7, 10])
                    .style(dialog_ui::field)
                    .on_input_maybe(enabled.then_some(Message::Id))
                    .into()
            } else {
                container(dialog_ui::detail(fl!("portal-account-withheld")))
                    .padding([5, 0])
                    .into()
            };
            let name: Element<'_, Message> = if self.share_name {
                text_input(&fl!("portal-account-add-name"), &self.name)
                    .id("account-name")
                    .padding([7, 10])
                    .style(dialog_ui::field)
                    .on_input_maybe(enabled.then_some(Message::Name))
                    .into()
            } else {
                container(dialog_ui::detail(if self.name.is_empty() {
                    fl!("portal-account-name-unset")
                } else {
                    fl!("portal-account-withheld")
                }))
                .padding([5, 0])
                .into()
            };
            content = content.push(
                container(column![share_id, username, share_name, name].spacing(8))
                    .padding(12)
                    .width(Length::Fill)
                    .style(dialog_ui::card),
            );
            if (self.share_id && self.invalid_id) || (self.share_name && self.invalid_name) {
                content =
                    content.push(text(fl!("portal-account-invalid-name")).style(text::danger));
            }
            if self.image_loading {
                content = content
                    .push(text(fl!("portal-account-preparing-image")).style(canopa_ui::muted));
            }
        }
        let fields = [
            (
                self.share_id && !self.id.trim().is_empty(),
                fl!("portal-account-disclosure-username"),
            ),
            (
                self.share_name && !self.name.trim().is_empty(),
                fl!("portal-account-disclosure-name"),
            ),
            (
                self.share_image && self.avatar.is_some(),
                fl!("portal-account-disclosure-photo"),
            ),
        ]
        .into_iter()
        .filter_map(|(selected, field)| selected.then_some(field))
        .collect::<Vec<_>>();
        let disclosure = if self.loading {
            String::new()
        } else if fields.is_empty() {
            fl!("portal-account-disclosure-empty")
        } else {
            fl!("portal-account-disclosure", fields = fields.join(" · "))
        };
        if let Some(error) = &self.error {
            content = content.push(text(error).style(text::danger));
        }
        let footer = canopa_ui::direction::row([
            dialog_ui::section(disclosure).width(Length::Fill).into(),
            dialog_ui::action(
                text(fl!("portal-cancel")),
                Some(Message::Cancel),
                canopa_ui::Action::Normal,
                false,
            )
            .into(),
            dialog_ui::action(
                text(if self.busy {
                    fl!("portal-account-sharing")
                } else if fields.is_empty() && !self.loading {
                    fl!("portal-account-share-none")
                } else {
                    fl!("portal-share")
                }),
                self.can_share().then_some(Message::Share(self.generation)),
                canopa_ui::Action::Primary,
                false,
            )
            .into(),
        ])
        .spacing(8)
        .align_y(iced::Center);
        dialog_ui::layout(content, footer)
    }
}
