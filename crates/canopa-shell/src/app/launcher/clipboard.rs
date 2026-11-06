//! Clipboard interactions use the launcher search field and existing action menu.
use super::*;
use crate::launcher::clipboard::{Command, Content};
use canopa_launcher::clipboard::{self, Edit, Saved, Text};

pub(super) const PROVIDER: usize = usize::MAX - 6;

#[derive(Clone, Debug)]
pub enum Draft {
    New(Text),
    Rename { id: u64, name: String },
}

#[derive(Clone, Debug)]
pub(super) struct Naming {
    pub draft: Draft,
    pub previous_query: String,
}

impl Session {
    pub fn displays_clipboard(&self) -> bool {
        self.naming.is_some()
            || self.actions_mode.as_ref().is_some_and(|mode| {
                mode.result
                    .actions
                    .iter()
                    .any(|action| matches!(action.action, Action::Clipboard(_)))
            })
            || matches!(self.query_scope().0, Scope::Clipboard | Scope::Snippets)
    }

    pub fn clipboard_entries(&self, scope: Scope, query: &str) -> Vec<PanelMenuEntry> {
        if !self.clipboard.loaded {
            return Vec::new();
        }
        let mut rows = Vec::new();
        if scope == Scope::Clipboard {
            rows.extend(
                self.clipboard
                    .history
                    .iter()
                    .filter_map(|entry| {
                        let copy = named("Copy", Command::CopyHistory(entry.id));
                        let remove = named("Remove from history", Command::Forget(entry.id));
                        let id = format!("recent:{}", entry.id);
                        match &entry.content {
                            Content::Text(text) if matches_text(query, text.as_str()) => {
                                Some(self.clipboard_entry(
                                    id,
                                    text.preview(),
                                    "Recent copy",
                                    vec![
                                        copy,
                                        named(
                                            "Save as snippet…",
                                            Command::DraftFromHistory(entry.id),
                                        ),
                                        remove,
                                    ],
                                ))
                            }
                            Content::Image(image) => {
                                let description = image.description();
                                if !matches_text(query, &format!("Image {description}")) {
                                    return None;
                                }
                                let mut row = self.clipboard_entry(
                                    id,
                                    "Image".into(),
                                    &description,
                                    vec![copy, remove],
                                );
                                if let PanelMenuEntryTarget::Launcher(target) = &mut row.target {
                                    target.thumbnail = Some(image.thumbnail().clone());
                                }
                                Some(row)
                            }
                            _ => None,
                        }
                    })
                    .take(launcher::RESULT_LIMIT),
            );
            let clear = "Clear clipboard history";
            if !self.clipboard.history.is_empty() && matches_text(query, clear) {
                rows.push(self.clipboard_entry(
                    "clear".into(),
                    clear.into(),
                    "",
                    vec![named("Clear history", Command::Clear)],
                ));
            }
            let label = if self.clipboard.saved.options().recording {
                "Turn off clipboard history"
            } else {
                "Turn on clipboard history"
            };
            if matches_text(query, label) {
                rows.push(self.clipboard_entry(
                    "recording".into(),
                    label.into(),
                    "Keep recent copies for this session",
                    vec![named(
                        label,
                        Command::Save(Edit::Recording(!self.clipboard.saved.options().recording)),
                    )],
                ));
            }
        } else {
            rows.extend(
                self.clipboard
                    .saved
                    .snippets
                    .iter()
                    .filter_map(|entry| {
                        if !matches_text(query, &entry.name)
                            && !matches_text(query, entry.text.as_str())
                        {
                            return None;
                        }
                        Some(self.clipboard_entry(
                            format!("snippet:{}", entry.id),
                            entry.name.clone(),
                            &entry.text.preview(),
                            vec![
                                named("Copy", Command::CopySnippet(entry.id)),
                                named("Rename snippet…", Command::Rename(entry.id)),
                                named("Delete snippet", Command::Save(Edit::Delete(entry.id))),
                            ],
                        ))
                    })
                    .take(launcher::RESULT_LIMIT),
            );
            let label = "New snippet from clipboard";
            if matches_text(query, label) {
                rows.push(self.clipboard_entry(
                    "new".into(),
                    label.into(),
                    "",
                    vec![named("Name snippet…", Command::DraftFromClipboard)],
                ));
            }
        }
        rows
    }

    pub fn clipboard_status(&self, scope: Scope, query: &str) -> String {
        if let Some(error) = &self.clipboard.load_error {
            return error.clone();
        }
        if !self.clipboard.loaded {
            return "Loading clipboard preferences…".into();
        }
        if scope == Scope::Clipboard {
            if let Some(error) = self.clipboard.status.into_iter().flatten().next() {
                return error.into();
            }
            if !self.clipboard.saved.options().recording {
                return "History is off · type snip for saved snippets".into();
            }
            if query.is_empty() {
                let kinds = if self.clipboard.saved.options().images {
                    "text and images"
                } else {
                    "text"
                };
                return format!(
                    "Recording {kinds} for this session · Enter to copy · Tab for actions"
                );
            }
        } else if query.is_empty() {
            return "Saved snippets · Enter to copy · Tab to rename or delete".into();
        }
        if self.clipboard_entries(scope, query).is_empty() {
            "No matches".into()
        } else {
            "Enter to copy · Tab for actions".into()
        }
    }

    pub fn start_naming(&mut self, draft: Draft) {
        let previous_query = self
            .actions_mode
            .take()
            .map(|mode| mode.previous_query)
            .unwrap_or_else(|| self.query.clone());
        let name = match &draft {
            Draft::New(_) => String::new(),
            Draft::Rename { name, .. } => name.clone(),
        };
        self.naming = Some(Naming {
            draft,
            previous_query,
        });
        self.search_changed(&name);
    }

    pub fn naming_entry(&self) -> Option<Vec<PanelMenuEntry>> {
        let mode = self.naming.as_ref()?;
        let name = self.query.trim();
        if !clipboard::valid_name(name) {
            return Some(Vec::new());
        }
        let edit = match &mode.draft {
            Draft::New(text) => Edit::Add {
                name: name.into(),
                text: text.clone(),
            },
            Draft::Rename { id, .. } => Edit::Rename {
                id: *id,
                name: name.into(),
            },
        };
        Some(vec![self.clipboard_entry(
            "save".into(),
            format!("Save snippet “{name}”"),
            "",
            vec![named("Save snippet", Command::Save(edit))],
        )])
    }

    fn clipboard_entry(
        &self,
        id: String,
        label: String,
        description: &str,
        actions: Vec<NamedAction>,
    ) -> PanelMenuEntry {
        self.entry(
            PROVIDER,
            ResultItem {
                id,
                label,
                description: description.into(),
                icon: Some("edit-paste".into()),
                actions,
            },
        )
    }
}

fn matches_text(query: &str, value: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let value = value.to_lowercase();
    query
        .split_whitespace()
        .all(|word| value.contains(&word.to_lowercase()))
}

fn named(label: &str, command: Command) -> NamedAction {
    NamedAction {
        label: label.into(),
        action: Action::Clipboard(command),
    }
}

impl super::super::App {
    pub(super) fn run_clipboard_action(
        &mut self,
        target: Target,
        command: Command,
    ) -> iced::Task<Message> {
        let session = target.session;
        let generation = target.generation;
        let finish = move |result| {
            Message::Panel(PanelEvent::Launcher(Event::ClipboardFinished {
                session,
                generation,
                result,
            }))
        };
        let unavailable = || "That clipboard entry is no longer available".to_owned();
        let history = |id| {
            self.launcher_clipboard
                .view
                .history
                .iter()
                .find(|entry| entry.id == id)
        };
        let snippet = |id| {
            self.launcher_clipboard
                .view
                .saved
                .snippets
                .iter()
                .find(|entry| entry.id == id)
        };
        let copy = |text: Option<&Text>| match text {
            Some(text) => iced::clipboard::write(text.as_str().to_owned()).chain(iced::Task::done(
                Message::Panel(PanelEvent::Launcher(Event::ActionFinished {
                    session,
                    generation,
                    result: Ok(()),
                })),
            )),
            None => iced::Task::done(finish(Err(unavailable()))),
        };
        let result = match command {
            Command::CopyHistory(id) => {
                return match history(id).map(|entry| &entry.content) {
                    Some(Content::Text(text)) => copy(Some(text)),
                    Some(Content::Image(image)) => {
                        let image = image.clone();
                        iced::Task::perform(async move { image.copy().await }, move |result| {
                            Message::Panel(PanelEvent::Launcher(Event::ActionFinished {
                                session,
                                generation,
                                result,
                            }))
                        })
                    }
                    None => copy(None),
                };
            }
            Command::CopySnippet(id) => return copy(snippet(id).map(|entry| &entry.text)),
            Command::DraftFromHistory(id) => history(id)
                .and_then(|entry| match &entry.content {
                    Content::Text(text) => Some(Draft::New(text.clone())),
                    Content::Image(_) => None,
                })
                .map(Some)
                .ok_or_else(unavailable),
            Command::Rename(id) => snippet(id)
                .map(|entry| {
                    Some(Draft::Rename {
                        id,
                        name: entry.name.clone(),
                    })
                })
                .ok_or_else(unavailable),
            Command::DraftFromClipboard => {
                return iced::clipboard::read().map(move |value| {
                    finish(
                        value
                            .and_then(Text::new)
                            .map(|text| Some(Draft::New(text)))
                            .ok_or_else(|| {
                                "Copy some text up to 16 KiB before creating a snippet".into()
                            }),
                    )
                });
            }
            Command::Forget(id) => {
                self.launcher_clipboard.forget(id);
                Ok(None)
            }
            Command::Clear => {
                self.launcher_clipboard.clear();
                Ok(None)
            }
            Command::Save(edit) => {
                if self.launcher_clipboard.saving {
                    return iced::Task::done(finish(Err(
                        "A clipboard change is being saved".into()
                    )));
                }
                if !self.launcher_clipboard.view.loaded {
                    return iced::Task::done(finish(Err(
                        "Clipboard preferences are unavailable".into()
                    )));
                }
                self.launcher_clipboard.saving = true;
                return iced::Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            Saved::update(&clipboard::path()?, |saved| saved.edit(edit))
                        })
                        .await
                        .unwrap_or_else(|_| Err("Clipboard save stopped".into()))
                    },
                    move |result| Message::LauncherClipboardSaved {
                        session,
                        generation,
                        result,
                    },
                );
            }
        };
        iced::Task::done(finish(result))
    }
}
