//! Transfer choices use captured metadata and the shared operation question window.
use super::{
    file_operations::Event,
    question_dialog::{Dialog, control, describe, scroll},
    *,
};
use crate::{
    operation_jobs::{Question, TransferQuestion},
    transfers::{ConflictChoice, TransferReply},
};
use canopa_theme::UiKind;
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
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let mut facts = column![
        text(dialog.title())
            .font(font)
            .wrapping(text::Wrapping::WordOrGlyph)
            .size(size)
    ]
    .spacing(12);
    let mut choices = vec![("Cancel", TransferReply::Cancel)];
    match question {
        TransferQuestion::Conflict {
            source,
            destination,
            ..
        } => {
            let explanation = if question.can_merge() {
                "Merge combines these folders and asks about individual file collisions. Keep both gives the incoming folder another name. Skip leaves the incoming folder untouched."
            } else if recovery {
                "This destination already contains an item. Recovery keeps existing work. Keep both copies the saved item under another name; Skip leaves both items untouched."
            } else if question.can_replace() {
                "This name is already in use. Replace keeps the previous destination for Undo. Keep both gives the incoming file another name. Skip leaves both versions untouched."
            } else {
                "This name is already used by a different kind of item. Keep both gives the incoming item another name. Skip leaves both items untouched."
            };
            facts = facts
                .push(
                    text(explanation)
                        .font(font)
                        .wrapping(text::Wrapping::WordOrGlyph)
                        .size(small),
                )
                .push(
                    text(format!("Incoming\n{}", describe(source, None)))
                        .font(font)
                        .wrapping(text::Wrapping::WordOrGlyph)
                        .size(small),
                )
                .push(
                    text(format!("Existing\n{}", describe(destination, None)))
                        .font(font)
                        .wrapping(text::Wrapping::WordOrGlyph)
                        .size(small),
                );
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
            facts = facts.push(text(format!("{}\n{message}", destination.to_string_lossy())).font(font).wrapping(text::Wrapping::WordOrGlyph).size(small))
                .push(text("Restore access to the original destination, then retry. Completed work remains recorded; Cancel keeps the unfinished source files.").font(font).wrapping(text::Wrapping::WordOrGlyph).size(small));
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
        buttons = buttons.push(super::focus::control(
            control(dialog.window, index),
            Some(message.clone()),
            appearance,
            canopa_ui::styled_action(
                text(*label)
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
    let mut actions = column![buttons.wrap()].spacing(10);
    if matches!(question, TransferQuestion::Conflict { .. })
        && desktop.file_operations.storage_error.is_none()
    {
        let id = control(dialog.window, choices.len());
        order.push(id.clone());
        let message = Message::FileOperation(Event::ToggleApply(dialog.job, dialog.generation));
        let toggled = message.clone();
        actions = actions.push(super::focus::control(
            id,
            Some(message),
            appearance,
            checkbox(dialog.apply)
                .label("Apply to remaining conflicts of this kind")
                .font(font)
                .text_wrapping(text::Wrapping::WordOrGlyph)
                .text_size(small)
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
            actions
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Panel)));
    super::focus::group(body, order, Vec::new(), Some(control(dialog.window, 0)))
}
