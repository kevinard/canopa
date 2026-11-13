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
use canopa_theme::ColorRole;
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
    use canopa_ui::Action;
    // Caption, reply, button style and whether the caption warns of data loss.
    let mut choices = vec![(
        fl!("files-cancel"),
        TransferReply::Cancel,
        Action::Quiet,
        false,
    )];
    match question {
        TransferQuestion::Conflict {
            source,
            destination,
            ..
        } => {
            let explanation = if question.can_merge() {
                fl!("files-conflict-merge-explanation")
            } else if recovery {
                fl!("files-conflict-recovery-explanation")
            } else if question.can_replace() {
                fl!("files-conflict-replace-explanation")
            } else {
                fl!("files-conflict-kind-explanation")
            };
            let text_size = appearance.typography().body_size.px();
            let filename = source.name().into_owned();
            let path = |entry: &crate::browse::Entry| {
                display_location(&desktop.home, entry.path.parent().unwrap_or(&entry.path))
            };
            let original = if recovery {
                fl!("files-conflict-saved-version")
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
                            chrome::section_title(appearance, fl!("files-conflict-incoming"))
                                .into(),
                            chrome::section_title(appearance, fl!("files-conflict-existing"))
                                .into()
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
                choices.push((
                    fl!("files-conflict-replace"),
                    resolve(ConflictChoice::Replace),
                    Action::Normal,
                    true,
                ));
            }
            if question.can_merge() {
                choices.push((
                    fl!("files-conflict-merge"),
                    resolve(ConflictChoice::Merge),
                    Action::Normal,
                    false,
                ));
            }
            choices.extend([
                (
                    fl!("files-conflict-keep-both"),
                    resolve(ConflictChoice::KeepBoth),
                    Action::Primary,
                    false,
                ),
                (
                    fl!("files-conflict-skip"),
                    resolve(ConflictChoice::Skip),
                    Action::Quiet,
                    false,
                ),
            ]);
        }
        TransferQuestion::Unavailable {
            destination,
            message,
        } => {
            facts = facts
                .push(chrome::dialog_heading(
                    desktop,
                    chrome::Icon::Drive,
                    dialog.title(),
                    fl!("files-transfer-paused"),
                ))
                .push(
                    text(fl!("files-transfer-unavailable-explanation"))
                        .font(font)
                        .size(appearance.typography().body_size.px())
                        .wrapping(text::Wrapping::WordOrGlyph),
                )
                .push(chrome::card(
                    appearance,
                    column![
                        chrome::section_title(appearance, fl!("files-destination")),
                        text(display_location(&desktop.home, destination))
                            .font(font)
                            .size(small)
                            .wrapping(text::Wrapping::WordOrGlyph),
                        text(fl!("files-system-details", details = message.clone()))
                            .font(font)
                            .size(small)
                            .style(canopa_ui::muted)
                            .wrapping(text::Wrapping::WordOrGlyph),
                    ]
                    .spacing(6),
                ));
            choices.push((
                fl!("files-retry"),
                TransferReply::Retry,
                Action::Primary,
                false,
            ));
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
    let mut buttons = Vec::new();
    for (index, (label, choice, kind, danger)) in choices.iter().enumerate() {
        let message = Message::FileOperation(Event::TransferAnswer(
            dialog.job,
            dialog.generation,
            *choice,
        ));
        buttons.push(super::focus::button(
            control(dialog.window, index),
            Some(message.clone()),
            appearance,
            canopa_ui::styled_action(
                text(label.clone())
                    .font(font)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .size(small)
                    .color_maybe(danger.then(|| chrome::ink(appearance, ColorRole::Danger))),
                Some(message),
                *kind,
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
                    fl!("files-conflict-apply-folders")
                } else {
                    fl!("files-conflict-apply-files")
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
                .style(canopa_ui::scrollbar)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new()
                        .width(6)
                        .scroller_width(6)
                        .spacing(4),
                ))
                .height(Length::Fill),
            decision_actions(canopa_ui::direction::row(buttons).spacing(8), scope)
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| chrome::window_surface(&appearance, true));
    super::focus::group(body, order, Vec::new(), Some(control(dialog.window, 0)))
}
