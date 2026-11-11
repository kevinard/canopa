//! One source-bound window for the service's exact generation-bound question.
use super::file_operations::Event;
use super::*;
use crate::{
    operation_jobs::Question,
    operations::{UndoChoice, UndoConflict, UndoItem, UndoSource},
    transfers::TransferReply,
};
use canopa_theme::{ColorRole, UiKind};
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

pub(super) fn display_location(home: &Path, path: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(relative) if relative.as_os_str().is_empty() => "Home".into(),
        Ok(relative) => format!("Home / {}", relative.to_string_lossy().replace('/', " / ")),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

pub(super) fn entry_metadata(entry: &crate::browse::Entry) -> String {
    let modified = entry
        .modified
        .map(chrono::DateTime::<chrono::Local>::from)
        .map(|time| time.format("%b %-d, %Y · %H:%M:%S").to_string())
        .unwrap_or_else(|| "Unknown".into());
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
    format!("{kind}{bytes}\nModified {modified}")
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
            .align_x(iced::alignment::Horizontal::Right),
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

/// What a screen reader reads when the question opens, beside its title.
fn summary(dialog: &Dialog) -> String {
    use crate::operation_jobs::TransferQuestion;
    match &dialog.question {
        Question::Undo(conflict) => explanation(conflict.source).to_owned(),
        Question::Transfer(TransferQuestion::Conflict { source, .. })
        | Question::Recovery(TransferQuestion::Conflict { source, .. }) => format!(
            "{} “{}” already exists in the destination.",
            if source.is_directory() {
                "A folder named"
            } else {
                "An item named"
            },
            source.name()
        ),
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
        UndoSource::Trash => Some("Restoring from Trash"),
        UndoSource::PreviousDestination
        | UndoSource::ChangedRetained
        | UndoSource::UnavailableRetained
        | UndoSource::UnavailableMergeRetained => Some("Saved previous destination"),
        _ => None,
    };
    let (path, detail) = match &conflict.incoming {
        UndoItem::Available(entry) => (entry.path.as_path(), entry_metadata(entry)),
        UndoItem::Unavailable { path, message, .. } => {
            (path.as_path(), format!("Unavailable · {message}"))
        }
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
    if let Some(label) = logical {
        item = item.push(detail_text(label.into()));
    }
    if path != conflict.destination && logical.is_none() {
        item = item.push(detail_text(format!(
            "Destination · {}",
            display_location(&desktop.home, &conflict.destination)
        )));
    }
    let mut facts = column![
        chrome::dialog_heading(desktop, chrome::Icon::Recovery, "Undo needs a decision", ""),
        text(explanation(conflict.source))
            .font(font)
            .size(appearance.typography().body_size.px())
            .wrapping(text::Wrapping::WordOrGlyph),
        chrome::card(appearance, item),
    ]
    .spacing(12);
    for (label, entry, path) in [
        ("Existing item", &conflict.existing, None),
        (
            "Saved previous destination",
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
        facts = facts.push(detail_text(format!(
            "Return to · {}",
            display_location(&desktop.home, path)
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
        buttons = buttons.push(super::focus::button(
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
            decision_actions(buttons, None)
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Panel)));
    super::focus::group(body, order, Vec::new(), Some(control(dialog.window, 0)))
}
