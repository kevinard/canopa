//! The Users page: your account (name, picture, password) and, for
//! administrators, the other accounts (type, password at next login,
//! removal) and adding one. AccountsService and polkit decide what each
//! person may change; your own password goes to `passwd` (ADR 0143).
use super::{Message, State, field, focus, live_page::LivePage, settings_group, toggle};
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
    text(label).size(14).style(canopa_ui::muted).into()
}

/// A button with a focus id of its own, for rows with repeated labels.
fn row_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
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
            canopa_ui::action(text(label), message, canopa_ui::Action::Normal, false)
                .padding([6, 12]),
        ),
    )
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
                .padding(10)
        ]
        .spacing(6)
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
}

impl UsersForms {
    pub(super) fn clear_secrets(&mut self) {
        wipe(&mut self.current);
        wipe(&mut self.new);
        wipe(&mut self.confirm);
    }
}

impl State {
    pub(super) fn users_edit(&mut self, change: Edit) -> Task<Message> {
        let forms = &mut self.users_forms;
        match change {
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
            direction::row([
                field(
                    format!("users-{uid}-name"),
                    fl!("settings-users-full-name"),
                    typed,
                    move |value| edit(Edit::Name(uid, value)),
                ),
                row_button(
                    format!("users-{uid}-save-name"),
                    fl!("settings-users-save-name"),
                    changed.then(|| edit(Edit::SaveName(uid))),
                ),
            ])
            .spacing(12)
            .align_y(Alignment::End),
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
                text(if user.icon_file.is_empty() {
                    fl!("settings-users-picture-none")
                } else {
                    fl!("settings-users-picture", file = user.icon_file.as_str())
                })
                .width(Length::Fill)
                .align_x(direction::text_start())
                .into(),
                row_button(
                    format!("users-{uid}-picture"),
                    fl!("settings-users-choose-picture-button"),
                    Some(edit(Edit::ChoosePicture(uid))),
                ),
                row_button(
                    format!("users-{uid}-picture-remove"),
                    fl!("settings-users-remove-picture"),
                    (!user.icon_file.is_empty())
                        .then(|| apply(UsersAction::Picture(uid, String::new()))),
                ),
            ])
            .spacing(10)
            .align_y(Alignment::Center),
            muted(fl!("settings-users-picture-hint")),
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
            row_button(
                "users-password-change".into(),
                if forms.changing {
                    fl!("settings-users-password-changing")
                } else {
                    fl!("settings-users-password-change")
                },
                ready.then(|| edit(Edit::ChangePassword)),
            ),
        ]
        .spacing(10);
        match &forms.password_result {
            Some(Ok(())) => password = password.push(muted(fl!("settings-users-password-changed"))),
            Some(Err(error)) => password = password.push(text(error.clone()).size(14)),
            None => {}
        }
        password = password.push(muted(fl!("settings-users-password-hint")));
        settings_group(
            fl!("settings-users-yours"),
            column![
                text(fl!(
                    "settings-users-me",
                    name = me.name.as_str(),
                    administrator = if me.administrator { "yes" } else { "no" }
                )),
                self.name_editor(me),
                self.picture_row(me),
                password,
            ]
            .spacing(14),
        )
    }

    fn other_account<'a>(&'a self, user: &'a User) -> Element<'a, Message> {
        let uid = user.uid;
        let mut card = column![
            direction::row([
                column![
                    text(user.display_name().to_owned()),
                    muted(user.name.clone())
                ]
                .spacing(2)
                .align_x(direction::start())
                .width(Length::Fill)
                .into(),
                row_button(
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
                        row_button(
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
        iced::widget::container(card)
            .padding(12)
            .width(Length::Fill)
            .style(canopa_ui::surface)
            .into()
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
                row_button(
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
        let mut body = column![].spacing(14);
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
        let mut others = column![].spacing(12);
        let mut any = false;
        for user in state
            .accounts
            .users
            .iter()
            .filter(|user| user.uid != state.me)
        {
            any = true;
            others = others.push(self.other_account(user));
        }
        if !any {
            others = others.push(muted(fl!("settings-users-no-others")));
        }
        others = others.push(self.add_account());
        if !administrator {
            others = others.push(muted(fl!("settings-users-others-hint")));
        }
        body.push(settings_group(fl!("settings-users-others"), others))
            .into()
    }

    pub(super) fn users_view(&self) -> Element<'_, Message> {
        self.users.view(message, |state| self.users_sections(state))
    }
}
