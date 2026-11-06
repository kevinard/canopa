use super::*;
use iced::{
    Length,
    widget::{column, container, row, scrollable, text},
};

fn action<'a>(label: &'a str, message: Option<Message>, primary: bool) -> Element<'a, Message> {
    canopa_ui::action(text(label), message, canopa_ui::Action::Normal, primary).into()
}
fn status(status: RecoveryStatus) -> &'static str {
    match status {
        RecoveryStatus::Busy => "Still in use",
        RecoveryStatus::Missing => "Folder unavailable",
        RecoveryStatus::Incomplete => "Incomplete copy",
        RecoveryStatus::Interrupted => "Interrupted",
        RecoveryStatus::Uncertain => "Result unconfirmed",
        RecoveryStatus::Published => "Previously published",
        RecoveryStatus::Unavailable => "Couldn’t check files",
    }
}
fn explanation(status: RecoveryStatus) -> &'static str {
    match status {
        RecoveryStatus::Busy => {
            "A print process may still be using these files. Refresh after it exits."
        }
        RecoveryStatus::Missing => {
            "The retained folder may have moved or become unavailable. Its last known location is shown below."
        }
        RecoveryStatus::Incomplete => {
            "A complete document could not be established. You can inspect any retained files in their folder."
        }
        RecoveryStatus::Interrupted => {
            "Printing was interrupted. You can save a copy of the retained data to a new destination."
        }
        RecoveryStatus::Uncertain => {
            "The print result could not be confirmed. Check the original destination before saving another copy."
        }
        RecoveryStatus::Published => {
            "The document was previously published. You can review the retained data and save another copy if needed."
        }
        RecoveryStatus::Unavailable => {
            "The recorded files changed or could not be checked. You can inspect their last known folder."
        }
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
        Screen::Preparing => column![text("Preparing recovered copy…").size(22), text("Retained files will stay in place."), action("Back", Some(Message::Back), false)].spacing(16).into(),
        Screen::Saving => column![text("Saving recovered copy…").size(22), text("Retained files will stay in place. If you close this window, check your chosen destination before trying again.")].spacing(16).into(),
        Screen::CheckingPrinters => column![text("Checking printer jobs…").size(22), text("Only identified jobs without confirmed acceptance may be cancelled. Documents are never sent again.")].spacing(16).into(),
        Screen::RemovingPrinter => column![text("Removing printer record…").size(22), text("The printer job will be left unchanged.")].spacing(16).into(),
        Screen::PrinterDismiss => {
            let title = app.printer().map(printer_title).unwrap_or("Printed document");
            column![text("Remove this printer record?").size(24),
                text(title).size(18),
                text("Check your printer before printing another copy. This removes only Canopa’s record; it does not cancel the printer job or send the document again."),
                text("Canopa will stop checking this record automatically. Removing it frees one place in the recovery list."),
                row![action("Keep in list", Some(Message::Keep), true), action("Remove record", Some(Message::DismissPrinter), false)].spacing(10)
            ].spacing(18).into()
        }
        Screen::Dismiss => {
            let folder = app.item().map(|item| label(&item.directory)).unwrap_or_default();
            column![text("Remove this work from the list?").size(24),
                text("Its files will stay in place. Keep the folder location if you want to find them later."),
                text(folder).size(14), row![action("Keep in list", Some(Message::Keep), true), action("Remove from list", Some(Message::Dismiss), false)].spacing(10)
            ].spacing(18).into()
        }
        Screen::List => {
            let mut entries = column![].spacing(8).width(Length::Fill);
            for item in &app.items {
                let path = PathBuf::from(std::ffi::OsString::from_vec(item.destination.clone()));
                let title = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| "Printed file".into());
                let selected = app.selected.as_ref() == Some(&item.id);
                entries = entries.push(canopa_ui::action(
                    column![text(title).size(16), text(status(item.status)).size(12)].spacing(5),
                    app.editable().then(|| Message::Select(item.id.clone())),
                    canopa_ui::Action::Normal, selected,
                ).width(Length::Fill));
            }
            let details: Element<'_, Message> = if let Some(item) = app.item() {
                column![
                    text(status(item.status)).size(22), text(explanation(item.status)),
                    text("Original destination").size(13), text(label(&item.destination)).size(14),
                    action("Save document copy…", (app.editable() && available(item, RecoveryBody::Document)).then_some(Message::Save(RecoveryBody::Document)), true),
                    action("Save file moved aside…", (app.editable() && available(item, RecoveryBody::Displaced)).then_some(Message::Save(RecoveryBody::Displaced)), false),
                    text("A file moved aside may contain unrelated data. Saving a copy leaves it in place.").size(13),
                    text("Copies up to 128 MiB can be saved here. Use the folder for other files.").size(13),
                    text("Retained folder").size(13), text(label(&item.directory)).size(14),
                    row![action("Open folder", app.editable().then_some(Message::OpenFolder), false), action("Remove from list…", (app.editable() && dismissible(item)).then_some(Message::AskDismiss), false)].spacing(8)
                ].spacing(12).width(Length::Fill).into()
            } else {
                column![text(if app.busy { "Loading retained work…" } else if app.error.is_some() || app.file_load_error.is_some() { "Retained work unavailable" } else { "No retained print work" }).size(22), text("Interrupted print files will appear here when they can be found.")].spacing(12).into()
            };
            // Embedded scrollbars reserve width instead of covering path text.
            let file_body = row![
                scrollable(entries).spacing(6).width(Length::FillPortion(2)),
                scrollable(details).spacing(6).width(Length::FillPortion(3))
            ].spacing(22).height(Length::Fill);
            let body = if app.tab == Tab::Printers { printer_body(app) } else { file_body.into() };
            let tabs = row![
                canopa_ui::action(text(format!("Files ({})", app.items.len())), app.editable().then_some(Message::Tab(Tab::Files)), canopa_ui::Action::Normal, app.tab == Tab::Files),
                canopa_ui::action(text(format!("Printers ({})", app.printers.len())), app.editable().then_some(Message::Tab(Tab::Printers)), canopa_ui::Action::Normal, app.tab == Tab::Printers)
            ].spacing(8);
            let mut layout = column![row![text("Print recovery").size(28).width(Length::Fill), action("Refresh", app.editable().then_some(Message::Refresh), false)].spacing(10),
                tabs, body].spacing(18);
            let load_error = if app.tab == Tab::Printers { &app.printer_load_error } else { &app.file_load_error };
            if let Some(error) = load_error { layout = layout.push(text(error)); }
            if let Some(error) = &app.error { layout = layout.push(text(error)); }
            if let Some(notice) = &app.notice { layout = layout.push(text(notice)); }
            layout.push(action("Close", Some(Message::Close), false)).into()
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

fn printer_title(item: &PrinterRecoveryItem) -> &str {
    if item.title.trim().is_empty() {
        "Printed document"
    } else {
        &item.title
    }
}

fn printer_status(status: PrinterRecoveryStatus) -> &'static str {
    match status {
        PrinterRecoveryStatus::Busy => "Still in use",
        PrinterRecoveryStatus::Unconfirmed => "Print result unconfirmed",
        PrinterRecoveryStatus::Accepted => "Previously accepted",
        PrinterRecoveryStatus::Unavailable => "Couldn’t check record",
    }
}

fn printer_explanation(status: PrinterRecoveryStatus) -> &'static str {
    match status {
        PrinterRecoveryStatus::Busy => {
            "A print process is still using this record. Refresh after it finishes."
        }
        PrinterRecoveryStatus::Unconfirmed => {
            "Canopa could not confirm this print result. Check your printer before printing another copy."
        }
        PrinterRecoveryStatus::Accepted => {
            "The printer service previously accepted this document. It may still be printing. Removing this record leaves the job unchanged."
        }
        PrinterRecoveryStatus::Unavailable => {
            "This record could not be checked. Refresh before removing it."
        }
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
            .push(text("Printer").size(13))
            .push(text(destination))
            .push(text("Print server").size(13))
            .push(text(&item.server).size(14))
            .push(
                text(
                    item.job
                        .map(|id| format!("Job {id}"))
                        .unwrap_or_else(|| "Job number unavailable".into()),
                )
                .size(14),
            )
            .push(action(
                "Remove record…",
                (app.editable() && item.dismissible()).then_some(Message::AskPrinterDismiss),
                false,
            ));
    } else {
        details = details
            .push(
                text(if app.busy {
                    "Loading printer records…"
                } else if app.printer_load_error.is_some() {
                    "Printer records unavailable"
                } else {
                    "No unresolved printer records"
                })
                .size(22),
            )
            .push(text(
                "Printer records with an uncertain outcome will appear here.",
            ));
    }
    details = details.push(text("Check all printer records").size(18))
        .push(text("Cleanup cancels only identified jobs whose acceptance was not confirmed. Previously accepted jobs are left alone. Documents are never sent again."))
        .push(action("Check and clean up", (app.editable() && !app.printers.is_empty() && app.printer_load_error.is_none()).then_some(Message::CheckPrinters), true))
        .push(text("Up to 16 records can be kept. Unconfirmed records stay until cleanup or your explicit removal.").size(13));
    row![
        scrollable(entries).spacing(6).width(Length::FillPortion(2)),
        scrollable(details).spacing(6).width(Length::FillPortion(3))
    ]
    .spacing(22)
    .height(Length::Fill)
    .into()
}
