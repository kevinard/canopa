//! One source-bound window for the service's exact generation-bound question.
use super::file_operations::Event;
use super::*;
use crate::{
    operation_jobs::Question,
    operations::{UndoChoice, UndoConflict, UndoItem, UndoSource},
    transfers::TransferReply,
};
use canopa_theme::UiKind;
use iced::{
    Length,
    widget::{column, container, row, scrollable, text},
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
    pub fn title(&self) -> &'static str {
        match &self.question {
            Question::Undo(_) => "Undo needs a decision",
            Question::Recovery(_) => "Recovery copy needs a decision",
            Question::Transfer(crate::operation_jobs::TransferQuestion::Conflict {
                folders: true,
                ..
            }) => "Folder conflict",
            Question::Transfer(crate::operation_jobs::TransferQuestion::Conflict { .. }) => {
                "File conflict"
            }
            Question::Transfer(crate::operation_jobs::TransferQuestion::Unavailable { .. }) => {
                "Destination unavailable"
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

fn choices(conflict: &UndoConflict) -> Vec<(&'static str, UndoChoice)> {
    use UndoChoice::*;
    use UndoSource::*;
    let mut choices = vec![("Cancel", Cancel)];
    match conflict.source {
        Current | Trash | PreviousDestination => {
            if conflict.can_merge() {
                choices.push(("Merge", Merge));
            }
            if conflict.can_replace() {
                choices.push(("Replace", Replace));
            }
            choices.extend([("Keep both", KeepBoth), ("Skip", Skip)]);
        }
        NonemptyFolder | ChangedCreatedFolder => {
            choices.extend([("Keep item", KeepItem), ("Skip", Skip)])
        }
        UnavailableCreatedFolder => choices.extend([("Finish step", FinishStep), ("Skip", Skip)]),
        MergeRemainder => choices.extend([
            ("Move remaining items", Merge),
            ("Keep remaining items", KeepItem),
            ("Skip", Skip),
        ]),
        MergeSource | MergeDestination => {
            choices.extend([("Finish this Merge", FinishStep), ("Skip", Skip)])
        }
        ChangedCopy => {
            if conflict.can_move_to_recovery() {
                choices.push(("Move changed item to Recovery", MoveToRecovery));
            }
            choices.extend([("Keep item", KeepItem), ("Skip", Skip)]);
        }
        ChangedRetained => {
            if conflict.can_use_saved_version() {
                choices.push(("Use saved version", UseSavedVersion));
            }
            choices.extend([("Keep item", KeepItem), ("Skip", Skip)]);
        }
        ChangedMove | ChangedCrossMove => {
            if conflict.can_return_item() {
                choices.push(("Return item", ReturnItem));
            }
            choices.extend([("Keep item", KeepItem), ("Skip", Skip)]);
        }
        UnavailableMove | UnavailableCrossMove | UnavailableRetained | UnavailableMergeRetained => {
            if conflict.can_restore_previous_destination() {
                choices.push(("Restore previous destination", RestorePreviousDestination));
            }
            if conflict.can_return_item() {
                choices.push(("Return item", ReturnItem));
            }
            choices.extend([("Finish step", FinishStep), ("Skip", Skip)]);
        }
    }
    choices
}

fn explanation(source: UndoSource) -> &'static str {
    use UndoSource::*;
    match source {
        Current => {
            "Undo would return this item to a name that is already in use. Review both versions before choosing. Replace retains the displaced item in Recovery; Keep both returns under a new name. Skip leaves this step available for later."
        }
        Trash => {
            "An item from Trash needs to return to a name that is already in use. Review both versions. Keep both restores it under another name; Skip keeps it available for later."
        }
        PreviousDestination => {
            "Undo is restoring a saved previous destination, but its original name is in use. Review both versions. Replacements remain protected in Recovery."
        }
        NonemptyFolder => {
            "This created folder now contains later work. Keep item leaves the folder and its contents in place and finishes its creation step. Skip keeps the step available for later."
        }
        ChangedCreatedFolder => {
            "The created folder has changed, moved or been replaced. Keep item leaves the current item untouched and finishes this creation step. Skip lets you decide later."
        }
        UnavailableCreatedFolder => {
            "The created folder cannot be inspected. Restore its location or access before trying again. Finish step leaves files untouched and gives up this removal step; Skip keeps it for later."
        }
        MergeRemainder => {
            "Items remain in the merged destination. Move remaining items continues the return while handling new conflicts. Keep remaining items leaves them here and finishes this branch. Skip lets you decide later."
        }
        MergeSource | MergeDestination => {
            "A folder used by this partial Merge has moved, changed or cannot be inspected. Restore its original location or access to continue. Finish this Merge leaves files in place and preserves saved previous destinations in Recovery. Already returned items stay where they are."
        }
        ChangedCopy => {
            "This copied item has changed since the copy. Move changed item to Recovery preserves its newer contents before undoing the copy. Keep item leaves it here and finishes its copy step; Skip lets you decide later."
        }
        ChangedRetained => {
            "The saved version has changed. Review the saved and current items before choosing to use it. Keep item preserves the current item and finishes this step, retaining protected versions in Recovery."
        }
        ChangedMove | ChangedCrossMove => {
            "The moved item or its destination has changed. Return item returns the reviewed current item and preserves any saved previous destination. Keep item leaves the current item in place and finishes this step. Skip keeps it for later."
        }
        UnavailableMove | UnavailableCrossMove => {
            "The moved item cannot be inspected. Restore its location or access before continuing. Restore previous destination returns only that saved version when available. Finish step leaves the moved item untouched and preserves protected data in Recovery."
        }
        UnavailableRetained | UnavailableMergeRetained => {
            "A saved previous version cannot be inspected. Restore its location or access to retry. Return item performs only the available return when offered. Finish step leaves current files untouched and retains protected saved data in Recovery."
        }
    }
}

pub(super) fn describe(entry: &crate::browse::Entry, label: Option<&str>) -> String {
    let modified = entry
        .modified
        .map(chrono::DateTime::<chrono::Local>::from)
        .map(|time| time.format("%Y-%m-%d %H:%M:%S %Z").to_string())
        .unwrap_or_else(|| "Unknown modification time".into());
    let kind = match entry.kind {
        crate::browse::EntryKind::Directory => "Folder",
        crate::browse::EntryKind::File => "File",
        crate::browse::EntryKind::Symlink => "Link",
        crate::browse::EntryKind::Other => "Special item",
    };
    let bytes = if entry.kind == crate::browse::EntryKind::Directory {
        String::new()
    } else {
        format!(" · {} bytes", entry.size)
    };
    format!(
        "{}\n{kind}{bytes} · {modified}",
        label.map_or_else(|| entry.path.to_string_lossy(), std::borrow::Cow::Borrowed)
    )
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
        let (window, opened) = window::open_transient(
            parent,
            window::Settings {
                size: Size::new(640.0, 520.0),
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
            self.file_operations.error = Some(
                "Could not open the operation question. Review the operation to try again.".into(),
            );
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

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    let Question::Undo(conflict) = &dialog.question else {
        return super::transfer_dialog::view(desktop, dialog);
    };
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let logical = match conflict.source {
        UndoSource::Trash => Some(format!(
            "{} (from Trash)",
            conflict.destination.to_string_lossy()
        )),
        UndoSource::PreviousDestination
        | UndoSource::ChangedRetained
        | UndoSource::UnavailableRetained
        | UndoSource::UnavailableMergeRetained => Some(format!(
            "{} (saved previous destination)",
            conflict.destination.to_string_lossy()
        )),
        _ => None,
    };
    let incoming = match &conflict.incoming {
        UndoItem::Available(entry) => describe(entry, logical.as_deref()),
        UndoItem::Unavailable { path, message, .. } => {
            format!(
                "{}\nUnavailable: {message}",
                logical
                    .as_deref()
                    .map_or_else(|| path.to_string_lossy(), std::borrow::Cow::Borrowed)
            )
        }
    };
    let mut facts = column![
        text("Undo needs a decision")
            .size(size)
            .font(font)
            .wrapping(text::Wrapping::WordOrGlyph),
        text(explanation(conflict.source))
            .size(small)
            .font(font)
            .wrapping(text::Wrapping::WordOrGlyph),
        text(format!("Reviewed item\n{incoming}"))
            .size(small)
            .font(font)
            .wrapping(text::Wrapping::WordOrGlyph),
        text(format!(
            "Destination\n{}",
            conflict.destination.to_string_lossy()
        ))
        .size(small)
        .font(font)
        .wrapping(text::Wrapping::WordOrGlyph),
    ]
    .spacing(12);
    for (label, entry, path) in [
        ("Existing item", &conflict.existing, None),
        (
            "Saved previous destination",
            &conflict.saved_previous,
            Some(conflict.destination.to_string_lossy()),
        ),
    ] {
        if let Some(entry) = entry {
            facts = facts.push(
                text(format!("{label}\n{}", describe(entry, path.as_deref())))
                    .size(small)
                    .font(font)
                    .wrapping(text::Wrapping::WordOrGlyph),
            );
        }
    }
    if let Some(path) = &conflict.return_destination {
        facts = facts.push(
            text(format!("Return to\n{}", path.to_string_lossy()))
                .size(small)
                .font(font)
                .wrapping(text::Wrapping::WordOrGlyph),
        );
    }
    facts = facts.push(
        container(
            text(
                desktop
                    .file_operations
                    .storage_error
                    .as_deref()
                    .or(dialog.error.as_deref())
                    .unwrap_or(""),
            )
            .size(small)
            .font(font)
            .wrapping(text::Wrapping::WordOrGlyph),
        )
        .height(40),
    );
    let choices = if desktop.file_operations.storage_error.is_some() {
        vec![("Cancel", UndoChoice::Cancel)]
    } else {
        choices(conflict)
    };
    let order: Vec<_> = choices
        .iter()
        .enumerate()
        .map(|(index, _)| control(dialog.window, index))
        .collect();
    let mut buttons = row![].spacing(8);
    for (index, (label, choice)) in choices.into_iter().enumerate() {
        let message = Message::FileOperation(Event::Answer(dialog.job, dialog.generation, choice));
        buttons = buttons.push(super::focus::control(
            control(dialog.window, index),
            Some(message.clone()),
            appearance,
            canopa_ui::styled_action(
                text(label)
                    .font(font)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .size(small),
                Some(message),
                canopa_ui::Action::Quiet,
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
            buttons.wrap()
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Panel)));
    super::focus::group(body, order, Vec::new(), Some(control(dialog.window, 0)))
}
