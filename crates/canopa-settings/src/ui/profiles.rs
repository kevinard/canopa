use super::{Message, State, choice, field, pick, task};
use crate::{
    display::{Monitor, TrialHandle},
    profiles::{Book, Switching},
    session::Paths,
};
use anyhow::{Context, Result};
use canopa_ui::direction;
use iced::{
    Element, Length, Task,
    widget::{column, container, text},
};

#[derive(Clone, Debug)]
pub(super) enum Action {
    Load,
    Loaded(Result<Book, String>),
    Stored(Result<Book, String>),
    Title(String),
    Select(String),
    Switching(Switching),
    SaveLayout,
    SaveSwitching,
    Delete,
    Test,
}
fn message(action: Action) -> Message {
    Message::DisplayProfiles(action)
}

#[derive(Default)]
pub(super) struct Editor {
    book: Option<Result<Book, String>>,
    title: String,
    pub(super) selected: Option<String>,
    pub(super) prominent: bool,
    switching: Switching,
}

impl Editor {
    pub(super) fn load(paths: &Paths) -> Task<Message> {
        let paths = paths.clone();
        task(
            move || Book::load(&paths),
            |value| message(Action::Loaded(value)),
        )
    }

    /// Drop the profile drafts before reloading the saved profiles.
    pub(super) fn discard(&mut self) {
        self.book = None;
        self.title.clear();
    }

    fn set_book(&mut self, book: Result<Book, String>) {
        if let Ok(book) = &book {
            self.switching = book.switching;
            if self
                .selected
                .as_ref()
                .is_some_and(|name| book.get(name).is_err())
            {
                self.selected = None;
            }
        }
        self.book = Some(book);
    }
}

impl State {
    pub(super) fn display_draft(&self) -> Result<Vec<Monitor>> {
        self.monitors
            .iter()
            .cloned()
            .zip(&self.positions)
            .map(|(mut monitor, (x, y))| {
                monitor.x = x
                    .parse()
                    .with_context(|| fl!("settings-displays-position-invalid"))?;
                monitor.y = y
                    .parse()
                    .with_context(|| fl!("settings-displays-position-invalid"))?;
                Ok(monitor)
            })
            .collect()
    }

    pub(super) fn update_profiles(&mut self, action: Action) -> Task<Message> {
        match action {
            Action::Loaded(book) => self.display_profiles.set_book(book),
            Action::Stored(result) => {
                self.busy = false;
                match result {
                    Ok(book) => {
                        self.display_profiles.set_book(Ok(book));
                        self.notice = fl!("settings-profiles-saved");
                    }
                    Err(error) => self.notice = error,
                }
            }
            _ if self.busy || self.trial.is_some() || !self.paths.active() => {}
            Action::Load => return Editor::load(&self.paths),
            Action::Title(title) => self.display_profiles.title = title.chars().take(80).collect(),
            Action::Select(name) => self.display_profiles.selected = Some(name),
            Action::Switching(policy) => self.display_profiles.switching = policy,
            Action::SaveLayout | Action::SaveSwitching | Action::Delete | Action::Test => {
                let Some(Ok(book)) = self.display_profiles.book.clone() else {
                    return Task::none();
                };
                let selected = self.display_profiles.selected.clone();
                let paths = self.paths.clone();
                let switching = self.display_profiles.switching;
                let title = self.display_profiles.title.trim().to_owned();
                let draft = matches!(action, Action::SaveLayout).then(|| self.display_draft());
                let outputs = self.outputs.clone();
                self.busy = true;
                if matches!(action, Action::Test) {
                    return task(
                        move || {
                            let name = selected.with_context(|| fl!("settings-profiles-choose"))?;
                            let layout = book.get(&name)?.resolve(&outputs)?;
                            TrialHandle::start(&paths, layout, Some(&name), false)
                        },
                        Message::TrialStarted,
                    );
                }
                return task(
                    move || match action {
                        Action::SaveLayout => book.save_layout(
                            &title,
                            &draft.expect("save captured a draft")?,
                            &outputs,
                        ),
                        Action::SaveSwitching => book.set_switching(switching),
                        Action::Delete => {
                            book.remove(&selected.with_context(|| fl!("settings-profiles-choose"))?)
                        }
                        _ => unreachable!(),
                    },
                    |value| message(Action::Stored(value)),
                );
            }
        }
        Task::none()
    }

    pub(super) fn profiles_view(&self) -> Element<'_, Message> {
        let editor = &self.display_profiles;
        let enabled = self.paths.active() && !self.busy && self.trial.is_none();
        let mut content = column![text(fl!("settings-profiles")).size(22)].spacing(10);
        if !self.paths.active() {
            content = content.push(text(fl!("settings-profiles-needs-session")));
        } else {
            content = content.push(self.action(
                "settings-button-reload-display-profiles",
                fl!("settings-profiles-reload"),
                enabled.then(|| message(Action::Load)),
                false,
            ));
            match &editor.book {
                Some(Ok(book)) => {
                    let names = book
                        .profiles
                        .iter()
                        .map(|p| p.name.clone())
                        .collect::<Vec<_>>();
                    let selected = editor
                        .selected
                        .as_ref()
                        .and_then(|name| book.get(name).ok());
                    let compatible = selected.map(|profile| profile.resolve(&self.outputs));
                    content = content
                        .push(text(fl!("settings-profiles-when-changed")))
                        .push(pick(
                            "display-profile-switching",
                            Switching::ALL
                                .map(|policy| (policy, policy.to_string()))
                                .to_vec(),
                            Some(editor.switching),
                            |value| message(Action::Switching(value)),
                        ))
                        .push(
                            self.action(
                                "settings-button-save-switching-policy",
                                fl!("settings-profiles-save-policy"),
                                (enabled && book.switching != editor.switching)
                                    .then(|| message(Action::SaveSwitching)),
                                false,
                            ),
                        )
                        .push(choice(
                            "saved-display-profile",
                            names,
                            editor.selected.clone(),
                            |value| message(Action::Select(value)),
                        ))
                        .push(
                            direction::row([
                                self.action(
                                    "settings-button-test-saved-profile",
                                    fl!("settings-profiles-test"),
                                    (enabled
                                        && !self.refreshing
                                        && matches!(compatible, Some(Ok(_))))
                                    .then(|| message(Action::Test)),
                                    false,
                                ),
                                self.action(
                                    "settings-button-delete-selected-profile",
                                    fl!("settings-profiles-delete"),
                                    (enabled && selected.is_some())
                                        .then(|| message(Action::Delete)),
                                    false,
                                ),
                            ])
                            .spacing(8),
                        )
                        .push(field(
                            "display-profile-name",
                            fl!("settings-profiles-new-name"),
                            &editor.title,
                            |value| message(Action::Title(value)),
                        ))
                        .push(
                            self.action(
                                "settings-button-save-layout-as-profile",
                                fl!("settings-profiles-save-layout"),
                                (enabled
                                    && !self.refreshing
                                    && !self.monitors.is_empty()
                                    && !editor.title.trim().is_empty())
                                .then(|| message(Action::SaveLayout)),
                                false,
                            ),
                        );
                    if let Some(Err(error)) = compatible {
                        content = content.push(text(fl!(
                            "settings-profiles-incompatible",
                            error = format!("{error:#}")
                        )));
                    }
                }
                Some(Err(error)) => content = content.push(text(error)),
                None => content = content.push(text(fl!("settings-profiles-loading"))),
            }
        }
        container(content)
            .padding(16)
            .width(Length::Fill)
            .style(canopa_ui::surface)
            .into()
    }
}
