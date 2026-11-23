//! The Users page: your account (name, picture, password) and, for
//! administrators, the other accounts (type, password at next login,
//! removal) and adding one. AccountsService and polkit decide what each
//! person may change; your own password goes to `passwd` (ADR 0143).
use super::{Message, State, field, focus, kit, live_page::LivePage, settings_group, toggle};
use crate::{
    live::{Event, LiveAction},
    users::{UsersAction, UsersService, UsersState, check_picture, suggested_name, wipe},
};
use canopa_services::accounts::{PasswordMode, User, valid_user_name};
use canopa_ui::direction;
use iced::{
    Alignment, Element, Length, Task,
    widget::{column, text, text_input},
};
use std::{collections::HashMap, path::PathBuf};

pub(super) type UsersPage = LivePage<UsersService>;

pub(super) fn message(event: Event<UsersService>) -> Message {
    Message::Users(event)
}

fn apply(action: UsersAction) -> Message {
    message(Event::Apply(action))
}

fn edit(edit: Edit) -> Message {
    Message::UsersForm(edit)
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label)
        .size(13)
        .style(canopa_ui::muted)
        .width(Length::Fill)
        .into()
}

/// A button with a focus id of its own, for rows with repeated labels.
fn treated_button<'a>(
    id: String,
    label: String,
    message: Option<Message>,
    treatment: super::kit::Treatment,
) -> Element<'a, Message> {
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::button(label.clone())
            .key(id.clone())
            .disabled(message.is_none())
            .on_click(message.clone()),
        focus::control(
            id,
            message.clone(),
            None,
            None,
            false,
            iced::widget::button(text(label))
                .on_press_maybe(message)
                .style(move |theme, status| super::kit::button_style(theme, status, treatment))
                .padding([7, 14]),
        ),
    )
}

fn row_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
    treated_button(id, label, message, super::kit::Treatment::Secondary)
}
/// The action a card leads with.
fn primary_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
    treated_button(id, label, message, super::kit::Treatment::Primary)
}
/// An action that removes or forgets something.
fn danger_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
    treated_button(id, label, message, super::kit::Treatment::Destructive)
}

/// A person's initials in a round badge, then their name and a detail.
fn profile<'a>(
    name: &str,
    detail: String,
    size: f32,
    picture: Option<iced::widget::image::Handle>,
) -> Element<'a, Message> {
    let initials: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    let badge: Element<'a, Message> = if let Some(picture) = picture {
        iced::widget::image(picture)
            .width(size)
            .height(size)
            .content_fit(iced::ContentFit::Cover)
            .border_radius(size / 2.0)
            .into()
    } else {
        iced::widget::container(
            text(initials)
                .size(size * 0.38)
                .font(kit::strong_font(iced::font::Weight::Semibold))
                .style(|theme: &iced::Theme| text::Style {
                    color: Some(kit::on_selection(theme)),
                }),
        )
        .width(size)
        .height(size)
        .center_x(size)
        .center_y(size)
        .style(|theme: &iced::Theme| iced::widget::container::Style {
            background: Some(kit::selection(theme).into()),
            border: iced::border::rounded(u32::MAX),
            ..Default::default()
        })
        .into()
    };
    direction::row([
        badge,
        column![
            text(name.to_owned())
                .size(if size > 48.0 { 17 } else { kit::size::BODY })
                .font(kit::strong_font(iced::font::Weight::Semibold)),
            muted(detail),
        ]
        .spacing(2)
        .width(Length::Fill)
        .into(),
    ])
    .spacing(14)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn secret<'a>(
    id: &'static str,
    label: String,
    value: &str,
    change: fn(String) -> Edit,
) -> Element<'a, Message> {
    // The secret itself never enters the accessibility tree: only its length.
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::new(canopa_ui::accessibility::Role::PasswordInput)
            .label(label.clone())
            .value("•".repeat(value.chars().count()))
            .key(format!("{id}-field")),
        column![
            text(label),
            text_input("", value)
                .id(id)
                .secure(true)
                .on_input(move |value| edit(change(value)))
                .align_x(direction::start())
                .padding([6, 12])
                .style(kit::text_input_style)
        ]
        .spacing(7)
        .width(Length::Fill),
    )
}

#[derive(Clone, Debug)]
pub(super) enum Edit {
    Name(u64, String),
    SaveName(u64),
    ChoosePicture(u64),
    PictureChosen(u64, Option<PathBuf>),
    Current(String),
    New(String),
    Confirm(String),
    ChangePassword,
    PasswordChanged(Result<(), String>),
    Adding(bool),
    AddFullName(String),
    AddName(String),
    AddAdministrator(bool),
    AddSubmit,
    /// Ask how to remove this account, or stop asking.
    Remove(Option<u64>),
    /// Remove it, deleting its home folder or keeping it.
    RemoveConfirmed(u64, bool),
    /// A picture file, decoded for display; `None` when it can't be shown.
    Picture(String, Option<iced::widget::image::Handle>),
}

/// The page's unsaved input. Passwords are wiped once sent or abandoned.
#[derive(Default)]
pub(super) struct UsersForms {
    names: HashMap<u64, String>,
    current: String,
    new: String,
    confirm: String,
    changing: bool,
    password_result: Option<Result<(), String>>,
    adding: bool,
    add_full_name: String,
    add_name: String,
    add_name_typed: bool,
    add_administrator: bool,
    removing: Option<u64>,
    /// Decoded account pictures by file; `None` while loading or unreadable.
    pictures: HashMap<String, Option<iced::widget::image::Handle>>,
}

/// Decode an account picture with the greeter's limits, for display only.
fn decode_picture(path: &str) -> Option<iced::widget::image::Handle> {
    use canopa_assets::{
        DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
    };
    let mut batch = RasterBatch::new(
        RasterPolicy {
            max_encoded_bytes: 2 * 1024 * 1024,
            max_dimension: 2048,
            max_pixels: 4_194_304,
            decoder_allocation: DecoderAllocation::IncludingOutput(32 * 1024 * 1024),
            formats: Formats::EnabledRasterCodecs,
            file_access: FileAccess::Follow,
            apply_orientation: true,
            thumbnail: Some(192),
        },
        RasterBudget {
            file_attempts: 1,
            encoded_bytes: 2 * 1024 * 1024,
            pixel_bytes: 16 * 1024 * 1024,
        },
        &|| false,
    );
    let raster = batch.load(std::path::Path::new(path))?;
    Some(iced::widget::image::Handle::from_rgba(
        raster.width,
        raster.height,
        raster.pixels,
    ))
}

impl UsersForms {
    pub(super) fn clear_secrets(&mut self) {
        wipe(&mut self.current);
        wipe(&mut self.new);
        wipe(&mut self.confirm);
    }
}

impl State {
    /// Decode the pictures of the accounts shown that are not decoded yet.
    pub(super) fn load_user_pictures(&mut self) -> Task<Message> {
        let crate::live::Status::Ready(state) = self.users.live.status() else {
            return Task::none();
        };
        let wanted: Vec<String> = state
            .accounts
            .users
            .iter()
            .map(|user| user.icon_file.clone())
            .filter(|file| !file.is_empty() && !self.users_forms.pictures.contains_key(file))
            .collect();
        Task::batch(wanted.into_iter().map(|file| {
            self.users_forms.pictures.insert(file.clone(), None);
            let decode = file.clone();
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || decode_picture(&decode))
                        .await
                        .ok()
                        .flatten()
                },
                move |handle| edit(Edit::Picture(file.clone(), handle)),
            )
        }))
    }

    /// The decoded picture of `user`, if it has one that can be shown.
    fn picture(&self, user: &User) -> Option<iced::widget::image::Handle> {
        self.users_forms
            .pictures
            .get(&user.icon_file)
            .cloned()
            .flatten()
    }

    pub(super) fn users_edit(&mut self, change: Edit) -> Task<Message> {
        let forms = &mut self.users_forms;
        match change {
            Edit::Picture(path, handle) => {
                self.users_forms.pictures.insert(path, handle);
                return Task::none();
            }
            Edit::Name(uid, name) => {
                forms.names.insert(
                    uid,
                    name.chars().filter(|c| !c.is_control()).take(128).collect(),
                );
            }
            Edit::SaveName(uid) => {
                if let Some(name) = forms.names.remove(&uid) {
                    return self
                        .users
                        .update(Event::Apply(UsersAction::RealName(uid, name)), message);
                }
            }
            Edit::ChoosePicture(uid) => {
                let title = fl!("settings-users-choose-picture");
                return Task::perform(
                    async move { crate::network::choose_file(&title).await },
                    move |file| edit(Edit::PictureChosen(uid, file.ok().flatten())),
                );
            }
            Edit::PictureChosen(uid, Some(file)) => match check_picture(&file) {
                Ok(()) => {
                    return self.users.update(
                        Event::Apply(UsersAction::Picture(
                            uid,
                            file.to_string_lossy().into_owned(),
                        )),
                        message,
                    );
                }
                Err(error) => self.notice = error,
            },
            Edit::PictureChosen(_, None) => {}
            Edit::Current(value) => forms.current = value,
            Edit::New(value) => forms.new = value,
            Edit::Confirm(value) => forms.confirm = value,
            Edit::ChangePassword if !forms.changing => {
                if forms.new != forms.confirm {
                    forms.password_result = Some(Err(fl!("settings-users-password-mismatch")));
                    return Task::none();
                }
                forms.changing = true;
                forms.password_result = None;
                let current = std::mem::take(&mut forms.current);
                let new = std::mem::take(&mut forms.new);
                forms.clear_secrets();
                return Task::perform(
                    async move {
                        crate::users::change_own_password(&crate::users::passwd(), current, new)
                            .await
                    },
                    |result| edit(Edit::PasswordChanged(result)),
                );
            }
            Edit::ChangePassword => {}
            Edit::PasswordChanged(result) => {
                forms.changing = false;
                forms.password_result = Some(result);
            }
            Edit::Adding(open) => {
                forms.adding = open;
                if !open {
                    forms.add_full_name.clear();
                    forms.add_name.clear();
                    forms.add_name_typed = false;
                    forms.add_administrator = false;
                }
            }
            Edit::AddFullName(name) => {
                forms.add_full_name = name
                    .chars()
                    .filter(|c| !c.is_control() && *c != ':')
                    .take(128)
                    .collect();
                if !forms.add_name_typed {
                    forms.add_name = suggested_name(&forms.add_full_name);
                }
            }
            Edit::AddName(name) => {
                forms.add_name = name.chars().take(32).collect();
                forms.add_name_typed = true;
            }
            Edit::AddAdministrator(value) => forms.add_administrator = value,
            Edit::AddSubmit => {
                let action = UsersAction::Create {
                    name: forms.add_name.trim().to_owned(),
                    full_name: forms.add_full_name.trim().to_owned(),
                    administrator: forms.add_administrator,
                };
                let task = self.users.update(Event::Apply(action), message);
                return Task::batch([task, self.users_edit(Edit::Adding(false))]);
            }
            Edit::Remove(uid) => forms.removing = uid,
            Edit::RemoveConfirmed(uid, remove_files) => {
                forms.removing = None;
                return self.users.update(
                    Event::Apply(UsersAction::Delete { uid, remove_files }),
                    message,
                );
            }
        }
        Task::none()
    }

    fn name_editor<'a>(&'a self, user: &'a User) -> Element<'a, Message> {
        let uid = user.uid;
        let typed = self
            .users_forms
            .names
            .get(&uid)
            .map_or(user.real_name.as_str(), String::as_str);
        let changed = self
            .users_forms
            .names
            .get(&uid)
            .is_some_and(|name| name.trim() != user.real_name);
        column![
            kit::row(
                fl!("settings-users-full-name"),
                None,
                // Save appears once the typed name differs.
                direction::row(
                    [Some(super::bare_field(
                        format!("users-{uid}-name"),
                        fl!("settings-users-full-name"),
                        &fl!("settings-users-full-name-placeholder"),
                        typed,
                        move |value| edit(Edit::Name(uid, value)),
                        Length::Fixed(kit::PICK_WIDTH),
                    ))]
                    .into_iter()
                    .chain([changed.then(|| {
                        primary_button(
                            format!("users-{uid}-save-name"),
                            fl!("settings-users-save-name"),
                            Some(edit(Edit::SaveName(uid))),
                        )
                    })])
                    .flatten(),
                )
                .spacing(8)
                .align_y(Alignment::Center),
            ),
            self.users.control_status(
                &UsersAction::RealName(uid, String::new()).control(),
                message
            ),
        ]
        .spacing(4)
        .into()
    }

    fn picture_row<'a>(&'a self, user: &'a User) -> Element<'a, Message> {
        let uid = user.uid;
        column![
            direction::row([
                column![
                    text(fl!("settings-users-picture-label")),
                    muted(if self.picture(user).is_some() {
                        fl!("settings-users-picture-custom")
                    } else {
                        fl!("settings-users-picture-initials")
                    }),
                ]
                .spacing(2)
                .width(Length::Fill)
                .into(),
                row_button(
                    format!("users-{uid}-picture"),
                    fl!("settings-users-choose-picture-button"),
                    Some(edit(Edit::ChoosePicture(uid))),
                ),
                danger_button(
                    format!("users-{uid}-picture-remove"),
                    fl!("settings-users-remove-picture"),
                    self.picture(user)
                        .is_some()
                        .then(|| apply(UsersAction::Picture(uid, String::new()))),
                ),
            ])
            .spacing(10)
            .align_y(Alignment::Center),
            self.users
                .control_status(&UsersAction::Picture(uid, String::new()).control(), message),
        ]
        .spacing(4)
        .into()
    }

    fn administrator_toggle<'a>(&'a self, user: &'a User) -> Element<'a, Message> {
        let uid = user.uid;
        let control = UsersAction::Administrator(uid, true).control();
        let administrator = match self.users.live.pending(&control) {
            Some(UsersAction::Administrator(_, value)) => *value,
            _ => user.administrator,
        };
        column![
            toggle(
                format!("users-{uid}-administrator"),
                fl!("settings-users-administrator-toggle"),
                administrator,
                apply(UsersAction::Administrator(uid, !administrator)),
            ),
            self.users.control_status(&control, message),
        ]
        .spacing(4)
        .into()
    }

    fn my_account<'a>(&'a self, me: &'a User) -> Element<'a, Message> {
        let forms = &self.users_forms;
        let ready = !forms.changing
            && !forms.current.is_empty()
            && !forms.new.is_empty()
            && !forms.confirm.is_empty();
        let mut password = column![
            secret(
                "users-password-current",
                fl!("settings-users-password-current"),
                &forms.current,
                Edit::Current
            ),
            direction::row([
                secret(
                    "users-password-new",
                    fl!("settings-users-password-new"),
                    &forms.new,
                    Edit::New,
                ),
                secret(
                    "users-password-confirm",
                    fl!("settings-users-password-confirm"),
                    &forms.confirm,
                    Edit::Confirm,
                ),
            ])
            .spacing(12),
            direction::row([
                iced::widget::space().width(Length::Fill).into(),
                primary_button(
                    "users-password-change".into(),
                    if forms.changing {
                        fl!("settings-users-password-changing")
                    } else {
                        fl!("settings-users-password-change")
                    },
                    ready.then(|| edit(Edit::ChangePassword)),
                ),
            ])
            .spacing(16)
            .align_y(Alignment::Center),
        ]
        .spacing(12);
        match &forms.password_result {
            Some(Ok(())) => password = password.push(muted(fl!("settings-users-password-changed"))),
            Some(Err(error)) => password = password.push(text(error.clone()).size(14)),
            None => {}
        }

        column![
            super::noted_group(
                fl!("settings-users-yours"),
                kit::rows([
                    profile(
                        me.display_name(),
                        // The login name only when the full name hides it.
                        if me.real_name.trim().is_empty() {
                            if me.administrator {
                                fl!("settings-users-role-administrator")
                            } else {
                                fl!("settings-users-role-standard")
                            }
                        } else {
                            fl!(
                                "settings-users-me",
                                name = me.name.as_str(),
                                administrator = if me.administrator { "yes" } else { "no" }
                            )
                        },
                        56.0,
                        self.picture(me),
                    ),
                    self.name_editor(me),
                    self.picture_row(me),
                ]),
                [fl!("settings-users-picture-hint")],
            ),
            super::noted_group(
                fl!("settings-users-password-title"),
                password,
                [fl!("settings-users-password-hint")],
            ),
        ]
        .spacing(28)
        .into()
    }

    fn other_account<'a>(&'a self, user: &'a User) -> Element<'a, Message> {
        let uid = user.uid;
        let mut card = column![
            direction::row([
                profile(
                    user.display_name(),
                    user.name.clone(),
                    40.0,
                    self.picture(user)
                ),
                danger_button(
                    format!("users-{uid}-remove"),
                    fl!("settings-users-remove"),
                    Some(edit(Edit::Remove(Some(uid)))),
                ),
            ])
            .spacing(10)
            .align_y(Alignment::Center),
            self.administrator_toggle(user),
            self.name_editor(user),
        ]
        .spacing(10);
        let password = UsersAction::PasswordAtLogin(uid).control();
        card = card.push(
            column![
                direction::row([
                    text(match user.password_mode {
                        PasswordMode::SetAtLogin => fl!("settings-users-password-at-login"),
                        PasswordMode::None => fl!("settings-users-password-none"),
                        PasswordMode::Regular if user.locked => fl!("settings-users-locked"),
                        PasswordMode::Regular => fl!("settings-users-password-set"),
                    })
                    .width(Length::Fill)
                    .align_x(direction::text_start())
                    .into(),
                    row_button(
                        format!("users-{uid}-password"),
                        fl!("settings-users-ask-new-password"),
                        (user.password_mode != PasswordMode::SetAtLogin)
                            .then(|| apply(UsersAction::PasswordAtLogin(uid))),
                    ),
                ])
                .spacing(10)
                .align_y(Alignment::Center),
                self.users.control_status(&password, message),
            ]
            .spacing(4),
        );
        card = card.push(self.picture_row(user));
        if self.users_forms.removing == Some(uid) {
            card = card.push(
                column![
                    text(if user.home.is_empty() {
                        fl!(
                            "settings-users-remove-question-no-home",
                            name = user.display_name()
                        )
                    } else {
                        fl!(
                            "settings-users-remove-question",
                            name = user.display_name(),
                            home = user.home.as_str()
                        )
                    }),
                    direction::row([
                        row_button(
                            format!("users-{uid}-remove-keep"),
                            fl!("settings-users-remove-keep"),
                            Some(edit(Edit::RemoveConfirmed(uid, false))),
                        ),
                        danger_button(
                            format!("users-{uid}-remove-delete"),
                            fl!("settings-users-remove-delete"),
                            Some(edit(Edit::RemoveConfirmed(uid, true))),
                        ),
                        row_button(
                            format!("users-{uid}-remove-cancel"),
                            fl!("settings-cancel"),
                            Some(edit(Edit::Remove(None))),
                        ),
                    ])
                    .spacing(10),
                ]
                .spacing(8),
            );
        }
        card = card.push(
            self.users.control_status(
                &UsersAction::Delete {
                    uid,
                    remove_files: false,
                }
                .control(),
                message,
            ),
        );
        card.spacing(14).into()
    }

    fn add_account(&self) -> Element<'_, Message> {
        let forms = &self.users_forms;
        if !forms.adding {
            return column![
                row_button(
                    "users-add".into(),
                    fl!("settings-users-add-open"),
                    Some(edit(Edit::Adding(true)))
                ),
                self.users.control_status("create", message),
            ]
            .spacing(4)
            .into();
        }
        let valid = valid_user_name(forms.add_name.trim());
        column![
            field(
                "users-add-full-name",
                fl!("settings-users-full-name"),
                &forms.add_full_name,
                |value| { edit(Edit::AddFullName(value)) }
            ),
            field(
                "users-add-name",
                fl!("settings-users-user-name"),
                &forms.add_name,
                |value| { edit(Edit::AddName(value)) }
            ),
            muted(if valid || forms.add_name.is_empty() {
                fl!("settings-users-name-rules")
            } else {
                fl!("settings-users-name-invalid")
            }),
            toggle(
                "users-add-administrator",
                fl!("settings-users-administrator"),
                forms.add_administrator,
                edit(Edit::AddAdministrator(!forms.add_administrator)),
            ),
            muted(fl!("settings-users-add-hint")),
            direction::row([
                primary_button(
                    "users-add-submit".into(),
                    fl!("settings-users-add"),
                    valid.then(|| edit(Edit::AddSubmit)),
                ),
                row_button(
                    "users-add-cancel".into(),
                    fl!("settings-cancel"),
                    Some(edit(Edit::Adding(false))),
                ),
            ])
            .spacing(10),
        ]
        .spacing(10)
        .into()
    }

    fn users_sections<'a>(&'a self, state: &'a UsersState) -> Element<'a, Message> {
        let mut body = column![].spacing(28);
        match state.me() {
            Some(me) => body = body.push(self.my_account(me)),
            None => {
                body = body.push(settings_group(
                    fl!("settings-users-yours"),
                    muted(fl!("settings-users-unknown-me")),
                ))
            }
        }
        let administrator = state.me().is_some_and(|me| me.administrator);
        let mut others: Vec<Element<'_, Message>> = state
            .accounts
            .users
            .iter()
            .filter(|user| user.uid != state.me)
            .map(|user| self.other_account(user))
            .collect();
        if others.is_empty() {
            others.push(super::kit::empty(fl!("settings-users-no-others")));
        }
        others.push(self.add_account());
        body.push(super::noted_group(
            fl!("settings-users-others"),
            kit::rows(others),
            (!administrator).then(|| fl!("settings-users-others-hint")),
        ))
        .into()
    }

    pub(super) fn users_view(&self) -> Element<'_, Message> {
        self.users.view(message, |state| self.users_sections(state))
    }
}
