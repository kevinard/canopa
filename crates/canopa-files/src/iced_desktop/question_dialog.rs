//! One source-bound window for the service's exact generation-bound question.
use super::file_operations::Event;
use super::*;
use crate::{
    operation_jobs::Question,
    operations::{UndoChoice, UndoConflict, UndoItem, UndoSource},
    transfers::TransferReply,
};
use canopa_theme::ColorRole;
use iced::{
    Length,
    widget::{column, container, scrollable, text},
};

pub(super) struct Dialog {
    pub window: window::Id,
    pub parent: window::Id,
    pub job: u64,
    pub generation: u64,
    pub question: Question,
    pub apply: bool,
    pub error: Option<String>,
}

pub(super) enum Reply {
    Undo(UndoChoice),
    Transfer(TransferReply),
}
impl Dialog {
    pub fn title(&self) -> String {
        match &self.question {
            Question::Undo(_) => fl!("files-question-undo"),
            Question::Recovery(_) => fl!("files-question-recovery"),
            Question::Transfer(crate::operation_jobs::TransferQuestion::Conflict {
                folders: true,
                ..
            }) => fl!("files-question-folder-conflict"),
            Question::Transfer(crate::operation_jobs::TransferQuestion::Conflict { .. }) => {
                fl!("files-question-file-conflict")
            }
            Question::Transfer(crate::operation_jobs::TransferQuestion::Unavailable { .. }) => {
                fl!("files-question-destination-unavailable")
            }
        }
    }
}

pub(super) fn scroll(window: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-question-{window:?}-facts"))
}

pub(super) fn control(window: window::Id, index: usize) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-question-{window:?}-{index}"))
}

fn choices(conflict: &UndoConflict) -> Vec<(String, UndoChoice)> {
    use UndoChoice::*;
    use UndoSource::*;
    let mut choices = vec![(fl!("files-cancel"), Cancel)];
    match conflict.source {
        Current | Trash | PreviousDestination => {
            if conflict.can_merge() {
                choices.push((fl!("files-conflict-merge"), Merge));
            }
            if conflict.can_replace() {
                choices.push((fl!("files-conflict-replace"), Replace));
            }
            choices.extend([
                (fl!("files-conflict-keep-both"), KeepBoth),
                (fl!("files-conflict-skip"), Skip),
            ]);
        }
        NonemptyFolder | ChangedCreatedFolder => choices.extend([
            (fl!("files-undo-keep-item"), KeepItem),
            (fl!("files-conflict-skip"), Skip),
        ]),
        UnavailableCreatedFolder => choices.extend([
            (fl!("files-undo-finish-step"), FinishStep),
            (fl!("files-conflict-skip"), Skip),
        ]),
        MergeRemainder => choices.extend([
            (fl!("files-undo-move-remaining"), Merge),
            (fl!("files-undo-keep-remaining"), KeepItem),
            (fl!("files-conflict-skip"), Skip),
        ]),
        MergeSource | MergeDestination => choices.extend([
            (fl!("files-undo-finish-merge"), FinishStep),
            (fl!("files-conflict-skip"), Skip),
        ]),
        ChangedCopy => {
            if conflict.can_move_to_recovery() {
                choices.push((fl!("files-undo-move-to-recovery"), MoveToRecovery));
            }
            choices.extend([
                (fl!("files-undo-keep-item"), KeepItem),
                (fl!("files-conflict-skip"), Skip),
            ]);
        }
        ChangedRetained => {
            if conflict.can_use_saved_version() {
                choices.push((fl!("files-undo-use-saved-version"), UseSavedVersion));
            }
            choices.extend([
                (fl!("files-undo-keep-item"), KeepItem),
                (fl!("files-conflict-skip"), Skip),
            ]);
        }
        ChangedMove | ChangedCrossMove => {
            if conflict.can_return_item() {
                choices.push((fl!("files-undo-return-item"), ReturnItem));
            }
            choices.extend([
                (fl!("files-undo-keep-item"), KeepItem),
                (fl!("files-conflict-skip"), Skip),
            ]);
        }
        UnavailableMove | UnavailableCrossMove | UnavailableRetained | UnavailableMergeRetained => {
            if conflict.can_restore_previous_destination() {
                choices.push((
                    fl!("files-undo-restore-previous"),
                    RestorePreviousDestination,
                ));
            }
            if conflict.can_return_item() {
                choices.push((fl!("files-undo-return-item"), ReturnItem));
            }
            choices.extend([
                (fl!("files-undo-finish-step"), FinishStep),
                (fl!("files-conflict-skip"), Skip),
            ]);
        }
    }
    choices
}

fn explanation(source: UndoSource) -> String {
    use UndoSource::*;
    match source {
        Current => fl!("files-undo-explain-current"),
        Trash => fl!("files-undo-explain-trash"),
        PreviousDestination => fl!("files-undo-explain-previous-destination"),
        NonemptyFolder => fl!("files-undo-explain-nonempty-folder"),
        ChangedCreatedFolder => fl!("files-undo-explain-changed-created-folder"),
        UnavailableCreatedFolder => fl!("files-undo-explain-unavailable-created-folder"),
        MergeRemainder => fl!("files-undo-explain-merge-remainder"),
        MergeSource | MergeDestination => fl!("files-undo-explain-merge"),
        ChangedCopy => fl!("files-undo-explain-changed-copy"),
        ChangedRetained => fl!("files-undo-explain-changed-retained"),
        ChangedMove | ChangedCrossMove => fl!("files-undo-explain-changed-move"),
        UnavailableMove | UnavailableCrossMove => fl!("files-undo-explain-unavailable-move"),
        UnavailableRetained | UnavailableMergeRetained => {
            fl!("files-undo-explain-unavailable-retained")
        }
    }
}

pub(super) fn display_location(home: &Path, path: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(relative) if relative.as_os_str().is_empty() => fl!("files-place-home"),
        Ok(relative) => fl!(
            "files-home-path",
            path = relative.to_string_lossy().replace('/', " › ")
        ),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

pub(super) fn entry_metadata(entry: &crate::browse::Entry) -> String {
    let modified = entry
        .modified
        .and_then(super::listing_ui::local_time)
        .map(super::listing_ui::date_time_seconds)
        .unwrap_or_else(|| fl!("files-unknown"));
    let kind = match entry.kind {
        crate::browse::EntryKind::Directory => fl!("files-kind-folder"),
        crate::browse::EntryKind::File => fl!("files-kind-file"),
        crate::browse::EntryKind::Symlink => fl!("files-kind-link"),
        crate::browse::EntryKind::Other => fl!("files-kind-special"),
    };
    if entry.kind == crate::browse::EntryKind::Directory {
        fl!(
            "files-entry-metadata-folder",
            kind = kind,
            modified = modified
        )
    } else {
        fl!(
            "files-entry-metadata",
            kind = kind,
            bytes = entry.size,
            modified = modified
        )
    }
}

pub(super) fn decision_actions<'a>(
    buttons: iced::widget::Row<'a, Message>,
    scope: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![
        scope,
        iced::widget::rule::horizontal(1).style(chrome::divider),
        container(buttons.wrap())
            .width(Length::Fill)
            .align_x(canopa_ui::direction::end()),
    ]
    .spacing(8)
    .into()
}

impl Desktop {
    pub(super) fn auto_file_question(&mut self) -> Task<Message> {
        if self.file_operations.question.is_some() {
            return Task::none();
        }
        let Some((id, generation)) = self.file_operations.snapshot.jobs.iter().find_map(|job| {
            job.decision
                .as_ref()
                .map(|decision| (job.id, decision.generation))
        }) else {
            return Task::none();
        };
        if self
            .file_operations
            .seen_questions
            .contains(&(id, generation))
        {
            return Task::none();
        }
        if self.focused_window == self.operations_window && self.operations_window.is_some()
            || self
                .focused_window
                .is_some_and(|focused| self.file_owner(id) == Some(focused))
            || self
                .recovery_destination
                .as_ref()
                .is_some_and(|d| d.job == Some(id) && self.focused_window == Some(d.window))
        {
            return self.show_file_question(id, false);
        }
        Task::none()
    }

    pub(super) fn show_file_question(&mut self, job: u64, manual: bool) -> Task<Message> {
        if let Some(dialog) = &self.file_operations.question {
            return window::gain_focus(dialog.window);
        }
        let Some(decision) = self
            .file_operations
            .snapshot
            .jobs
            .iter()
            .find(|entry| entry.id == job)
            .and_then(|entry| entry.decision.clone())
        else {
            return Task::none();
        };
        let parent = if manual || self.focused_window == self.operations_window {
            self.operations_window
        } else {
            self.file_owner(job).filter(|id| {
                self.windows.contains_key(id)
                    || self
                        .trash_window
                        .as_ref()
                        .is_some_and(|trash| trash.window == *id)
                    || self
                        .recovery_window
                        .as_ref()
                        .is_some_and(|w| w.window == *id)
            })
        };
        let Some(parent) = parent else {
            return self.show_operations();
        };
        let height = match &decision.question {
            Question::Undo(conflict)
                if conflict.existing.is_none()
                    && conflict.saved_previous.is_none()
                    && conflict.return_destination.is_none() =>
            {
                295.0
            }
            Question::Undo(_) => 395.0,
            Question::Transfer(crate::operation_jobs::TransferQuestion::Unavailable { .. }) => {
                310.0
            }
            Question::Recovery(_) => 397.0,
            _ => 380.0,
        };
        let (window, opened) = window::open_transient(
            parent,
            window::Settings {
                size: Size::new(640.0, height),
                min_size: Some(Size::new(320.0, 240.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Files".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        self.file_operations
            .seen_questions
            .insert((job, decision.generation));
        self.file_operations.question = Some(Dialog {
            window,
            parent,
            job,
            generation: decision.generation,
            question: decision.question,
            apply: false,
            error: None,
        });
        opened.map(move |opened| Message::FileOperation(Event::QuestionOpened(window, opened)))
    }

    pub(super) fn file_question_opened(
        &mut self,
        id: window::Id,
        opened: Option<window::Id>,
    ) -> Task<Message> {
        if self
            .file_operations
            .question
            .as_ref()
            .is_none_or(|dialog| dialog.window != id)
        {
            return window::close(id);
        }
        if opened.is_none() {
            self.file_operations.question = None;
            self.file_operations.error = Some(fl!("files-question-failed"));
            return Task::none();
        }
        iced::widget::operation::focus(control(id, 0)).chain(window::gain_focus(id))
    }

    pub(super) fn sync_file_question(&mut self) -> Task<Message> {
        if let Some(dialog) = &self.file_operations.question {
            let current = self
                .file_operations
                .snapshot
                .jobs
                .iter()
                .find(|job| job.id == dialog.job)
                .and_then(|job| job.decision.as_ref())
                .is_some_and(|decision| decision.generation == dialog.generation);
            if !current {
                let id = dialog.window;
                self.file_operations.question = None;
                return window::close(id);
            }
        }
        Task::none()
    }

    pub(super) fn answer_file_question(
        &mut self,
        job: u64,
        generation: u64,
        choice: Reply,
    ) -> Task<Message> {
        let Some(dialog) = self
            .file_operations
            .question
            .as_mut()
            .filter(|dialog| dialog.job == job && dialog.generation == generation)
        else {
            return Task::none();
        };
        let Some(service) = &self.file_operations.service else {
            return Task::none();
        };
        let result = match (&dialog.question, choice) {
            (Question::Undo(_), Reply::Undo(choice)) => service.resolve(job, generation, choice),
            (Question::Transfer(_) | Question::Recovery(_), Reply::Transfer(mut choice)) => {
                if let TransferReply::Resolve {
                    apply_to_remaining, ..
                } = &mut choice
                {
                    *apply_to_remaining = dialog.apply;
                }
                service.resolve_transfer(job, generation, choice)
            }
            _ => return Task::none(),
        };
        match result {
            Ok(()) => {
                let id = dialog.window;
                let parent = dialog.parent;
                self.file_operations.question = None;
                let focus = if self.windows.contains_key(&parent)
                    || self.operations_window == Some(parent)
                    || self
                        .trash_window
                        .as_ref()
                        .is_some_and(|trash| trash.window == parent)
                    || self
                        .recovery_window
                        .as_ref()
                        .is_some_and(|w| w.window == parent)
                {
                    window::gain_focus(parent)
                } else {
                    Task::none()
                };
                focus.chain(window::close(id))
            }
            Err(error) => {
                dialog.error = Some(error.to_string());
                Task::none()
            }
        }
    }

    pub(super) fn close_file_question(&mut self, id: window::Id) -> Task<Message> {
        if let Some(dialog) = self
            .file_operations
            .question
            .as_ref()
            .filter(|dialog| dialog.window == id)
        {
            let reply = match &dialog.question {
                Question::Undo(_) => Reply::Undo(UndoChoice::Cancel),
                Question::Transfer(_) | Question::Recovery(_) => {
                    Reply::Transfer(TransferReply::Cancel)
                }
            };
            return self.answer_file_question(dialog.job, dialog.generation, reply);
        }
        Task::none()
    }
    pub(super) fn file_question_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || modifiers.control() && key.as_ref() == keyboard::Key::Character("w")
        {
            return self.close_file_question(id);
        }
        super::scrolling::key(scroll(id), key, modifiers)
    }
}

/// What a screen reader reads when the question opens, beside its title.
fn summary(dialog: &Dialog) -> String {
    use crate::operation_jobs::TransferQuestion;
    match &dialog.question {
        Question::Undo(conflict) => explanation(conflict.source),
        Question::Transfer(TransferQuestion::Conflict { source, .. })
        | Question::Recovery(TransferQuestion::Conflict { source, .. }) => {
            let name = source.name().into_owned();
            if source.is_directory() {
                fl!("files-conflict-folder-exists", name = name)
            } else {
                fl!("files-conflict-item-exists", name = name)
            }
        }
        Question::Transfer(TransferQuestion::Unavailable { message, .. })
        | Question::Recovery(TransferQuestion::Unavailable { message, .. }) => message.clone(),
    }
}

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    super::accessibility_ui::dialog(
        "files-question-dialog",
        dialog.title(),
        summary(dialog),
        true,
        content(desktop, dialog),
    )
}

fn content<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    let Question::Undo(conflict) = &dialog.question else {
        return super::transfer_dialog::view(desktop, dialog);
    };
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let small = appearance.typography().label_size.px();
    let logical = match conflict.source {
        UndoSource::Trash => Some(fl!("files-undo-restoring-from-trash")),
        UndoSource::PreviousDestination
        | UndoSource::ChangedRetained
        | UndoSource::UnavailableRetained
        | UndoSource::UnavailableMergeRetained => Some(fl!("files-undo-saved-previous")),
        _ => None,
    };
    let (path, detail) = match &conflict.incoming {
        UndoItem::Available(entry) => (entry.path.as_path(), entry_metadata(entry)),
        UndoItem::Unavailable { path, message, .. } => (
            path.as_path(),
            fl!("files-undo-unavailable", error = message.clone()),
        ),
    };
    let name = |path: &Path| {
        path.file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned()
    };
    let detail_text = |value: String| {
        text(value)
            .font(font)
            .size(small)
            .style(canopa_ui::muted)
            .wrapping(text::Wrapping::WordOrGlyph)
    };
    let mut item = column![
        chrome::section_title(
            appearance,
            name(if logical.is_some() {
                &conflict.destination
            } else {
                path
            })
        ),
        detail_text(detail),
        detail_text(display_location(
            &desktop.home,
            if logical.is_some() {
                &conflict.destination
            } else {
                path
            }
        )),
    ]
    .spacing(5);
    if let Some(label) = &logical {
        item = item.push(detail_text(label.clone()));
    }
    if path != conflict.destination && logical.is_none() {
        item = item.push(detail_text(fl!(
            "files-undo-destination",
            location = display_location(&desktop.home, &conflict.destination)
        )));
    }
    let mut facts = column![
        chrome::dialog_heading(
            desktop,
            chrome::Icon::Recovery,
            fl!("files-question-undo"),
            ""
        ),
        text(explanation(conflict.source))
            .font(font)
            .size(appearance.typography().body_size.px())
            .wrapping(text::Wrapping::WordOrGlyph),
        chrome::card(appearance, item),
    ]
    .spacing(12);
    for (label, entry, path) in [
        (fl!("files-undo-existing-item"), &conflict.existing, None),
        (
            fl!("files-undo-saved-previous"),
            &conflict.saved_previous,
            Some(conflict.destination.as_path()),
        ),
    ] {
        if let Some(entry) = entry {
            facts = facts.push(chrome::card(
                appearance,
                column![
                    chrome::section_title(appearance, label),
                    detail_text(display_location(&desktop.home, path.unwrap_or(&entry.path))),
                    detail_text(entry_metadata(entry)),
                ]
                .spacing(5),
            ));
        }
    }
    if let Some(path) = &conflict.return_destination {
        facts = facts.push(detail_text(fl!(
            "files-undo-return-to",
            location = display_location(&desktop.home, path)
        )));
    }
    if let Some(error) = desktop
        .file_operations
        .storage_error
        .as_ref()
        .or(dialog.error.as_ref())
    {
        facts = facts.push(
            text(error)
                .font(font)
                .size(small)
                .color(chrome::ink(appearance, ColorRole::Danger))
                .wrapping(text::Wrapping::WordOrGlyph),
        );
    }
    let choices = if desktop.file_operations.storage_error.is_some() {
        vec![(fl!("files-cancel"), UndoChoice::Cancel)]
    } else {
        choices(conflict)
    };
    let order: Vec<_> = choices
        .iter()
        .enumerate()
        .map(|(index, _)| control(dialog.window, index))
        .collect();
    let mut buttons = Vec::new();
    for (index, (label, choice)) in choices.into_iter().enumerate() {
        let message = Message::FileOperation(Event::Answer(dialog.job, dialog.generation, choice));
        buttons.push(super::focus::button(
            control(dialog.window, index),
            Some(message.clone()),
            appearance,
            canopa_ui::styled_action(
                text(label)
                    .font(font)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .size(small),
                Some(message),
                if matches!(choice, UndoChoice::Cancel | UndoChoice::Skip) {
                    canopa_ui::Action::Quiet
                } else {
                    canopa_ui::Action::Normal
                },
                false,
                appearance,
            ),
        ));
    }
    let appearance = appearance.clone();
    let body = container(
        column![
            scrollable(facts)
                .id(scroll(dialog.window))
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new()
                        .width(6)
                        .scroller_width(6)
                        .spacing(4),
                ))
                .height(Length::Fill),
            decision_actions(canopa_ui::direction::row(buttons).spacing(8), None)
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| chrome::window_surface(&appearance, true));
    super::focus::group(body, order, Vec::new(), Some(control(dialog.window, 0)))
}
