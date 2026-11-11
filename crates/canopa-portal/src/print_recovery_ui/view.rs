use super::*;
use iced::{
    Length,
    widget::{column, container, scrollable, text},
};

fn action<'a>(label: String, message: Option<Message>, primary: bool) -> Element<'a, Message> {
    canopa_ui::action(text(label), message, canopa_ui::Action::Normal, primary).into()
}
fn status(status: RecoveryStatus) -> String {
    match status {
        RecoveryStatus::Busy => fl!("portal-recovery-status-busy"),
        RecoveryStatus::Missing => fl!("portal-recovery-status-missing"),
        RecoveryStatus::Incomplete => fl!("portal-recovery-status-incomplete"),
        RecoveryStatus::Interrupted => fl!("portal-recovery-status-interrupted"),
        RecoveryStatus::Uncertain => fl!("portal-recovery-status-uncertain"),
        RecoveryStatus::Published => fl!("portal-recovery-status-published"),
        RecoveryStatus::Unavailable => fl!("portal-recovery-status-unavailable"),
    }
}
fn explanation(status: RecoveryStatus) -> String {
    match status {
        RecoveryStatus::Busy => fl!("portal-recovery-explain-busy"),
        RecoveryStatus::Missing => fl!("portal-recovery-explain-missing"),
        RecoveryStatus::Incomplete => fl!("portal-recovery-explain-incomplete"),
        RecoveryStatus::Interrupted => fl!("portal-recovery-explain-interrupted"),
        RecoveryStatus::Uncertain => fl!("portal-recovery-explain-uncertain"),
        RecoveryStatus::Published => fl!("portal-recovery-explain-published"),
        RecoveryStatus::Unavailable => fl!("portal-recovery-explain-unavailable"),
    }
}

pub(super) fn view(app: &Recovery) -> Element<'_, Message> {
    if let Screen::Picker(picker) = &app.screen {
        let generation = app.generation;
        return picker
            .view()
            .map(move |message| Message::Picker(generation, message));
    }
    let content: Element<'_, Message> = match app.screen {
        Screen::Preparing => column![
            text(fl!("portal-recovery-preparing")).size(22),
            text(fl!("portal-recovery-files-stay")),
            action(
                canopa_ui::direction::back_label(),
                Some(Message::Back),
                false
            )
        ]
        .spacing(16)
        .into(),
        Screen::Saving => column![
            text(fl!("portal-recovery-saving")).size(22),
            text(fl!("portal-recovery-saving-hint"))
        ]
        .spacing(16)
        .into(),
        Screen::CheckingPrinters => column![
            text(fl!("portal-recovery-checking")).size(22),
            text(fl!("portal-recovery-checking-hint"))
        ]
        .spacing(16)
        .into(),
        Screen::RemovingPrinter => column![
            text(fl!("portal-recovery-removing")).size(22),
            text(fl!("portal-recovery-removing-hint"))
        ]
        .spacing(16)
        .into(),
        Screen::PrinterDismiss => {
            let title = app
                .printer()
                .map(printer_title)
                .unwrap_or_else(|| fl!("portal-recovery-printed-document"));
            column![
                text(fl!("portal-recovery-remove-printer-question")).size(24),
                text(title).size(18),
                text(fl!("portal-recovery-remove-printer-explain")),
                text(fl!("portal-recovery-remove-printer-effect")),
                canopa_ui::direction::row([
                    action(fl!("portal-recovery-keep"), Some(Message::Keep), true),
                    action(
                        fl!("portal-recovery-remove-record"),
                        Some(Message::DismissPrinter),
                        false
                    ),
                ])
                .spacing(10)
            ]
            .spacing(18)
            .into()
        }
        Screen::Dismiss => {
            let folder = app
                .item()
                .map(|item| label(&item.directory))
                .unwrap_or_default();
            column![
                text(fl!("portal-recovery-remove-work-question")).size(24),
                text(fl!("portal-recovery-remove-work-explain")),
                text(folder).size(14),
                canopa_ui::direction::row([
                    action(fl!("portal-recovery-keep"), Some(Message::Keep), true),
                    action(
                        fl!("portal-recovery-remove-from-list"),
                        Some(Message::Dismiss),
                        false
                    ),
                ])
                .spacing(10)
            ]
            .spacing(18)
            .into()
        }
        Screen::List => {
            let mut entries = column![].spacing(8).width(Length::Fill);
            for item in &app.items {
                let path = PathBuf::from(std::ffi::OsString::from_vec(item.destination.clone()));
                let title = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| fl!("portal-recovery-printed-file"));
                let selected = app.selected.as_ref() == Some(&item.id);
                entries = entries.push(
                    canopa_ui::action(
                        column![text(title).size(16), text(status(item.status)).size(12)]
                            .spacing(5),
                        app.editable().then(|| Message::Select(item.id.clone())),
                        canopa_ui::Action::Normal,
                        selected,
                    )
                    .width(Length::Fill),
                );
            }
            let details: Element<'_, Message> = if let Some(item) = app.item() {
                column![
                    text(status(item.status)).size(22),
                    text(explanation(item.status)),
                    text(fl!("portal-recovery-original-destination")).size(13),
                    text(label(&item.destination)).size(14),
                    action(
                        fl!("portal-recovery-save-document"),
                        (app.editable() && available(item, RecoveryBody::Document))
                            .then_some(Message::Save(RecoveryBody::Document)),
                        true
                    ),
                    action(
                        fl!("portal-recovery-save-displaced"),
                        (app.editable() && available(item, RecoveryBody::Displaced))
                            .then_some(Message::Save(RecoveryBody::Displaced)),
                        false
                    ),
                    text(fl!("portal-recovery-displaced-hint")).size(13),
                    text(fl!(
                        "portal-recovery-size-limit",
                        size = canopa_i18n::format::size(128 * 1024 * 1024)
                    ))
                    .size(13),
                    text(fl!("portal-recovery-retained-folder")).size(13),
                    text(label(&item.directory)).size(14),
                    canopa_ui::direction::row([
                        action(
                            fl!("portal-recovery-open-folder"),
                            app.editable().then_some(Message::OpenFolder),
                            false
                        ),
                        action(
                            fl!("portal-recovery-remove-from-list-ellipsis"),
                            (app.editable() && dismissible(item)).then_some(Message::AskDismiss),
                            false
                        ),
                    ])
                    .spacing(8)
                ]
                .spacing(12)
                .width(Length::Fill)
                .into()
            } else {
                column![
                    text(if app.busy {
                        fl!("portal-recovery-loading")
                    } else if app.error.is_some() || app.file_load_error.is_some() {
                        fl!("portal-recovery-unavailable")
                    } else {
                        fl!("portal-recovery-empty")
                    })
                    .size(22),
                    text(fl!("portal-recovery-empty-hint"))
                ]
                .spacing(12)
                .into()
            };
            // Embedded scrollbars reserve width instead of covering path text.
            let file_body = canopa_ui::direction::row([
                scrollable(entries)
                    .spacing(6)
                    .width(Length::FillPortion(2))
                    .into(),
                scrollable(details)
                    .spacing(6)
                    .width(Length::FillPortion(3))
                    .into(),
            ])
            .spacing(22)
            .height(Length::Fill);
            let body = if app.tab == Tab::Printers {
                printer_body(app)
            } else {
                file_body.into()
            };
            let tabs = canopa_ui::direction::row([
                canopa_ui::action(
                    text(fl!("portal-recovery-files-tab", count = app.items.len())),
                    app.editable().then_some(Message::Tab(Tab::Files)),
                    canopa_ui::Action::Normal,
                    app.tab == Tab::Files,
                )
                .into(),
                canopa_ui::action(
                    text(fl!(
                        "portal-recovery-printers-tab",
                        count = app.printers.len()
                    )),
                    app.editable().then_some(Message::Tab(Tab::Printers)),
                    canopa_ui::Action::Normal,
                    app.tab == Tab::Printers,
                )
                .into(),
            ])
            .spacing(8);
            let mut layout = column![
                canopa_ui::direction::row([
                    text(fl!("portal-recovery-title"))
                        .size(28)
                        .width(Length::Fill)
                        .into(),
                    action(
                        fl!("portal-print-refresh"),
                        app.editable().then_some(Message::Refresh),
                        false
                    ),
                ])
                .spacing(10),
                tabs,
                body
            ]
            .spacing(18);
            let load_error = if app.tab == Tab::Printers {
                &app.printer_load_error
            } else {
                &app.file_load_error
            };
            if let Some(error) = load_error {
                layout = layout.push(text(error));
            }
            if let Some(error) = &app.error {
                layout = layout.push(text(error));
            }
            if let Some(notice) = &app.notice {
                layout = layout.push(text(notice));
            }
            layout
                .push(action(fl!("portal-close"), Some(Message::Close), false))
                .into()
        }
        Screen::Picker(_) => unreachable!(),
    };
    container(content)
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(canopa_ui::surface)
        .into()
}

fn printer_title(item: &PrinterRecoveryItem) -> String {
    if item.title.trim().is_empty() {
        fl!("portal-recovery-printed-document")
    } else {
        item.title.clone()
    }
}

fn printer_status(status: PrinterRecoveryStatus) -> String {
    match status {
        PrinterRecoveryStatus::Busy => fl!("portal-recovery-status-busy"),
        PrinterRecoveryStatus::Unconfirmed => fl!("portal-recovery-printer-unconfirmed"),
        PrinterRecoveryStatus::Accepted => fl!("portal-recovery-printer-accepted"),
        PrinterRecoveryStatus::Unavailable => fl!("portal-recovery-printer-unavailable"),
    }
}

fn printer_explanation(status: PrinterRecoveryStatus) -> String {
    match status {
        PrinterRecoveryStatus::Busy => fl!("portal-recovery-printer-explain-busy"),
        PrinterRecoveryStatus::Unconfirmed => fl!("portal-recovery-printer-explain-unconfirmed"),
        PrinterRecoveryStatus::Accepted => fl!("portal-recovery-printer-explain-accepted"),
        PrinterRecoveryStatus::Unavailable => fl!("portal-recovery-printer-explain-unavailable"),
    }
}

fn printer_body(app: &Recovery) -> Element<'_, Message> {
    let mut entries = column![].spacing(8).width(Length::Fill);
    for item in &app.printers {
        entries = entries.push(
            canopa_ui::action(
                column![
                    text(printer_title(item)).size(16),
                    text(&item.destination.queue).size(13),
                    text(printer_status(item.status)).size(12)
                ]
                .spacing(5),
                app.editable()
                    .then(|| Message::SelectPrinter(item.id.clone())),
                canopa_ui::Action::Normal,
                app.selected_printer.as_ref() == Some(&item.id),
            )
            .width(Length::Fill),
        );
    }
    let mut details = column![].spacing(12).width(Length::Fill);
    if let Some(item) = app.printer() {
        let destination = match &item.destination.instance {
            Some(instance) => format!("{} / {}", item.destination.queue, instance),
            None => item.destination.queue.clone(),
        };
        details = details
            .push(text(printer_status(item.status)).size(22))
            .push(text(printer_title(item)).size(18))
            .push(text(printer_explanation(item.status)))
            .push(text(fl!("portal-recovery-printer")).size(13))
            .push(text(destination))
            .push(text(fl!("portal-recovery-server")).size(13))
            .push(text(&item.server).size(14))
            .push(
                text(
                    item.job
                        .map(|id| fl!("portal-recovery-job", id = id.to_string()))
                        .unwrap_or_else(|| fl!("portal-recovery-job-unknown")),
                )
                .size(14),
            )
            .push(action(
                fl!("portal-recovery-remove-record-ellipsis"),
                (app.editable() && item.dismissible()).then_some(Message::AskPrinterDismiss),
                false,
            ));
    } else {
        details = details
            .push(
                text(if app.busy {
                    fl!("portal-recovery-printers-loading")
                } else if app.printer_load_error.is_some() {
                    fl!("portal-recovery-printers-unavailable")
                } else {
                    fl!("portal-recovery-printers-empty")
                })
                .size(22),
            )
            .push(text(fl!("portal-recovery-printers-empty-hint")));
    }
    details = details
        .push(text(fl!("portal-recovery-check-all")).size(18))
        .push(text(fl!("portal-recovery-check-all-hint")))
        .push(action(
            fl!("portal-recovery-check-cleanup"),
            (app.editable() && !app.printers.is_empty() && app.printer_load_error.is_none())
                .then_some(Message::CheckPrinters),
            true,
        ))
        .push(text(fl!("portal-recovery-record-limit", count = 16)).size(13));
    canopa_ui::direction::row([
        scrollable(entries)
            .spacing(6)
            .width(Length::FillPortion(2))
            .into(),
        scrollable(details)
            .spacing(6)
            .width(Length::FillPortion(3))
            .into(),
    ])
    .spacing(22)
    .height(Length::Fill)
    .into()
}
