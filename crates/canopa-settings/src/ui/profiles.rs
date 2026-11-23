use super::{Message, State, kit, pick, settings_group, task};
use crate::{
    display::{Monitor, TrialHandle},
    profiles::{Book, Switching},
    session::Paths,
};
use anyhow::{Context, Result};
use canopa_ui::direction;
use iced::{
    Element, Task,
    widget::{column, text},
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
        let mut monitors = self
            .monitors
            .iter()
            .cloned()
            .zip(&self.positions)
            .map(|(mut monitor, (x, y))| {
                monitor.x = x
                    .trim()
                    .parse()
                    .with_context(|| fl!("settings-displays-position-invalid"))?;
                monitor.y = y
                    .trim()
                    .parse()
                    .with_context(|| fl!("settings-displays-position-invalid"))?;
                Ok(monitor)
            })
            .collect::<Result<Vec<_>>>()?;
        // The arrangement starts at 0, 0, however the positions were typed.
        let enabled = || monitors.iter().filter(|m| m.enabled);
        let left = enabled().map(|m| m.x).min().unwrap_or(0);
        let top = enabled().map(|m| m.y).min().unwrap_or(0);
        for monitor in &mut monitors {
            monitor.x -= left;
            monitor.y -= top;
        }
        Ok(monitors)
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
        if !self.paths.active() {
            return settings_group(
                fl!("settings-profiles"),
                kit::caption(fl!("settings-profiles-needs-session")),
            );
        }
        let mut rows: Vec<Element<'_, Message>> = Vec::new();
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
                rows.push(kit::row(
                    fl!("settings-profiles-when-changed"),
                    None,
                    // Save appears once the choice differs from the saved one.
                    direction::row(
                        [Some(pick(
                            "display-profile-switching",
                            Switching::ALL
                                .map(|policy| (policy, policy.to_string()))
                                .to_vec(),
                            Some(editor.switching),
                            |value| message(Action::Switching(value)),
                        ))]
                        .into_iter()
                        .chain([(book.switching != editor.switching).then(|| {
                            self.treated_action(
                                "settings-button-save-switching-policy",
                                fl!("settings-profiles-save-short"),
                                enabled.then(|| message(Action::SaveSwitching)),
                                kit::Treatment::Primary,
                            )
                        })])
                        .flatten(),
                    )
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                ));
                // Only the actions that apply to the chosen profile.
                let mut chosen: Vec<Element<'_, Message>> = vec![super::sized_pick(
                    "saved-display-profile",
                    names
                        .iter()
                        .map(|name| (name.clone(), name.clone()))
                        .collect(),
                    editor.selected.clone(),
                    200.0,
                    |value| message(Action::Select(value)),
                )];
                if enabled && !self.refreshing && matches!(compatible, Some(Ok(_))) {
                    chosen.push(self.action(
                        "settings-button-test-saved-profile",
                        fl!("settings-profiles-test-short"),
                        Some(message(Action::Test)),
                        false,
                    ));
                }
                if enabled && selected.is_some() {
                    chosen.push(self.treated_action(
                        "settings-button-delete-selected-profile",
                        fl!("settings-profiles-delete-short"),
                        Some(message(Action::Delete)),
                        super::kit::Treatment::Destructive,
                    ));
                }
                let mut saved = column![kit::row(
                    fl!("settings-profiles-saved-label"),
                    None,
                    direction::row(chosen)
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                )]
                .spacing(6);
                if let Some(Err(error)) = compatible {
                    saved = saved.push(kit::caption(fl!(
                        "settings-profiles-incompatible",
                        error = format!("{error:#}")
                    )));
                }
                rows.push(saved.into());
                // Save appears once the layout has a name.
                let can_save = enabled && !self.refreshing && !self.monitors.is_empty();
                rows.push(kit::row(
                    fl!("settings-profiles-save-layout-label"),
                    None,
                    direction::row([
                        super::bare_field(
                            "display-profile-name",
                            fl!("settings-profiles-new-name"),
                            &fl!("settings-profiles-new-name"),
                            &editor.title,
                            |value| message(Action::Title(value)),
                            iced::Length::Fixed(200.),
                        ),
                        self.action(
                            "settings-button-save-layout-as-profile",
                            fl!("settings-profiles-save-short"),
                            (can_save && !editor.title.trim().is_empty())
                                .then(|| message(Action::SaveLayout)),
                            false,
                        ),
                    ])
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                ));
            }
            Some(Err(error)) => rows.push(text(error).into()),
            None => rows.push(kit::caption(fl!("settings-profiles-loading"))),
        }
        super::noted_group(
            fl!("settings-profiles"),
            kit::rows(rows),
            [fl!("settings-profiles-intro")],
        )
    }
}
