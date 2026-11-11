//! Transfer choices use captured metadata and the shared operation question window.
use super::{
    file_operations::Event,
    question_dialog::{
        Dialog, control, decision_actions, display_location, entry_metadata, scroll,
    },
    *,
};
use crate::{
    operation_jobs::{Question, TransferQuestion},
    transfers::{ConflictChoice, TransferReply},
};
use canopa_theme::{ColorRole, UiKind};
use iced::{
    Length,
    widget::{checkbox, column, container, row, scrollable, text},
};

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    let (Question::Transfer(question) | Question::Recovery(question)) = &dialog.question else {
        unreachable!()
    };
    let recovery = matches!(dialog.question, Question::Recovery(_));
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let small = appearance.typography().label_size.px();
    let mut facts = column![].spacing(12);
    let mut choices = vec![("Cancel", TransferReply::Cancel)];
    match question {
        TransferQuestion::Conflict {
            source,
            destination,
            ..
        } => {
            let explanation = if question.can_merge() {
                "Merge combines these folders and asks about individual collisions. Keep both renames the incoming folder. Skip leaves it untouched."
            } else if recovery {
                "“Keep both” copies the saved item under a new name. “Skip” leaves both items untouched."
            } else if question.can_replace() {
                "Replace saves the existing file for Undo. Keep both renames the incoming file. Skip leaves both untouched."
            } else {
                "This name is already used by a different kind of item. Keep both gives the incoming item another name. Skip leaves both items untouched."
            };
            let text_size = appearance.typography().body_size.px();
            let filename = source.name().into_owned();
            let path = |entry: &crate::browse::Entry| {
                display_location(&desktop.home, entry.path.parent().unwrap_or(&entry.path))
            };
            let original = if recovery {
                "Saved version in Recovery".into()
            } else {
                path(source)
            };
            let pair = |left: Element<'a, Message>, right: Element<'a, Message>| {
                row![
                    container(left).width(Length::Fill),
                    container(right).width(Length::Fill)
                ]
                .spacing(18)
            };
            let field = |value: String| {
                text(value)
                    .font(font)
                    .size(small)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .into()
            };
            let details = |entry: &crate::browse::Entry| {
                text(entry_metadata(entry))
                    .font(font)
                    .size(small)
                    .style(canopa_ui::muted)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .into()
            };
            facts = facts
                .push(chrome::dialog_heading(
                    desktop,
                    if source.is_directory() {
                        chrome::Icon::Folder
                    } else {
                        chrome::Icon::Documents
                    },
                    dialog.title(),
                    "",
                ))
                .push(
                    chrome::section_title(appearance, filename)
                        .wrapping(text::Wrapping::WordOrGlyph),
                )
                .push(
                    text(explanation)
                        .font(font)
                        .wrapping(text::Wrapping::WordOrGlyph)
                        .size(text_size),
                )
                .push(chrome::card(
                    appearance,
                    column![
                        pair(
                            chrome::section_title(appearance, "Incoming").into(),
                            chrome::section_title(appearance, "Existing").into()
                        ),
                        pair(field(original), field(path(destination))),
                        pair(details(source), details(destination)),
                    ]
                    .spacing(7),
                ));
            let resolve = |choice| TransferReply::Resolve {
                choice,
                apply_to_remaining: false,
            };
            if !recovery && question.can_replace() {
                choices.push(("Replace", resolve(ConflictChoice::Replace)));
            }
            if question.can_merge() {
                choices.push(("Merge", resolve(ConflictChoice::Merge)));
            }
            choices.extend([
                ("Keep both", resolve(ConflictChoice::KeepBoth)),
                ("Skip", resolve(ConflictChoice::Skip)),
            ]);
        }
        TransferQuestion::Unavailable {
            destination,
            message,
        } => {
            facts = facts
                .push(chrome::dialog_heading(desktop, chrome::Icon::Drive, dialog.title(), "The transfer is paused"))
                .push(text("Restore access to the destination, then choose Retry. Cancel keeps the unfinished source files.")
                    .font(font).size(appearance.typography().body_size.px())
                    .wrapping(text::Wrapping::WordOrGlyph))
                .push(chrome::card(
                    appearance,
                    column![
                        chrome::section_title(appearance, "Destination"),
                        text(display_location(&desktop.home, destination)).font(font).size(small)
                            .wrapping(text::Wrapping::WordOrGlyph),
                        text(format!("System details · {message}")).font(font).size(small).style(canopa_ui::muted)
                            .wrapping(text::Wrapping::WordOrGlyph),
                    ].spacing(6),
                ));
            choices.push(("Retry", TransferReply::Retry));
        }
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
                .wrapping(text::Wrapping::WordOrGlyph)
                .size(small),
        );
    }
    if desktop.file_operations.storage_error.is_some() {
        choices.truncate(1);
    }
    let mut order: Vec<_> = (0..choices.len())
        .map(|index| control(dialog.window, index))
        .collect();
    let mut buttons = row![].spacing(8);
    for (index, (label, choice)) in choices.iter().enumerate() {
        let message = Message::FileOperation(Event::TransferAnswer(
            dialog.job,
            dialog.generation,
            *choice,
        ));
        buttons = buttons.push(super::focus::button(
            control(dialog.window, index),
            Some(message.clone()),
            appearance,
            canopa_ui::styled_action(
                text(*label)
                    .font(font)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .size(small)
                    .color_maybe(
                        (*label == "Replace").then(|| chrome::ink(appearance, ColorRole::Danger)),
                    ),
                Some(message),
                match *label {
                    "Retry" | "Keep both" => canopa_ui::Action::Primary,
                    "Replace" | "Merge" => canopa_ui::Action::Normal,
                    _ => canopa_ui::Action::Quiet,
                },
                false,
                appearance,
            ),
        ));
    }
    let mut scope = None;
    if let TransferQuestion::Conflict { source, .. } = question
        && desktop.file_operations.storage_error.is_none()
    {
        let id = control(dialog.window, choices.len());
        order.push(id.clone());
        let message = Message::FileOperation(Event::ToggleApply(dialog.job, dialog.generation));
        let toggled = message.clone();
        scope = Some(super::focus::check_box(
            id,
            dialog.apply,
            Some(message),
            appearance,
            checkbox(dialog.apply)
                .label(if source.is_directory() {
                    "Apply to remaining folder conflicts"
                } else {
                    "Apply to remaining file conflicts"
                })
                .font(font)
                .text_wrapping(text::Wrapping::WordOrGlyph)
                .text_size(small)
                .style(canopa_ui::checkbox_style)
                .on_toggle(move |_| toggled.clone()),
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
            decision_actions(buttons, scope)
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Panel)));
    super::focus::group(body, order, Vec::new(), Some(control(dialog.window, 0)))
}
