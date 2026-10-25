use super::file_operations::Event;
use super::*;
use crate::operation_jobs::Phase;
use iced::{
    Length,
    widget::{column, container, row, text},
};

fn control(window: window::Id, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-operations-{window:?}-{name}"))
}

pub(super) fn rows(
    desktop: &Desktop,
    window: window::Id,
) -> (Element<'_, Message>, Vec<iced::widget::Id>) {
    let files = &desktop.file_operations;
    let appearance = &desktop.configuration.appearance;
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let font = canopa_ui::font(appearance);
    let mut order = Vec::new();
    let mut button = |label: String, name: String, event: Option<Event>| {
        let id = control(window, &name);
        let message = event.map(Message::FileOperation);
        if message.is_some() {
            order.push(id.clone());
        }
        super::focus::control(
            id,
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(label).font(font).size(small),
                message,
                canopa_ui::Action::Quiet,
                false,
                appearance,
            ),
        )
    };
    let undo = files.snapshot.undo.as_ref();
    let mut body = column![
        text("File operations").font(font).size(size),
        row![
            button(
                undo.map_or_else(|| "Nothing to Undo".into(), |undo| undo.label.clone()),
                "undo".into(),
                undo.filter(|_| files.ready() && !files.has_active())
                    .map(|undo| Event::Undo(window, undo.id))
            ),
            button(
                "Recovery".into(),
                "recovery".into(),
                files.service.as_ref().map(|_| Event::Recovery)
            ),
            button(
                "Latest".into(),
                "newest".into(),
                files.before.map(|_| Event::Newest)
            ),
            button(
                "Older".into(),
                "older".into(),
                files.has_older().then_some(Event::Older)
            ),
        ]
        .spacing(8)
        .wrap(),
    ]
    .spacing(10);
    if files.starting() {
        body = body.push(text("Preparing file operations…").font(font).size(small));
    }
    for error in [
        &files.error,
        &files.storage_error,
        &files.snapshot.error,
        &files.notification_error,
    ]
    .into_iter()
    .flatten()
    {
        body = body.push(text(error).font(font).size(small));
    }
    if files.snapshot.jobs.is_empty() {
        body = body.push(text("No file operations yet.").font(font).size(small));
    }
    for job in files.page() {
        let mut content = column![
            text(&job.title).font(font).size(size),
            text(file_operations::status(job.phase))
                .font(font)
                .size(small)
        ]
        .spacing(6);
        if let Some(detail) = &job.detail {
            content = content.push(text(detail).font(font).size(small));
        }
        if let Some(progress) = super::operation_details::progress(job, appearance) {
            content = content.push(progress);
        }
        for error in [job.error.as_ref(), files.notification_errors.get(&job.id)]
            .into_iter()
            .flatten()
        {
            content = content.push(text(error).font(font).size(small));
        }
        let mut controls = row![].spacing(8);
        if job.progress.is_some() || job.transfer.is_some() {
            controls = controls.push(button(
                "Details".into(),
                format!("details-{}", job.id),
                Some(Event::Details(job.id)),
            ));
        }
        if matches!(job.phase, Phase::Queued | Phase::Running | Phase::Waiting) {
            controls = controls.push(button(
                "Cancel".into(),
                format!("cancel-{}", job.id),
                Some(Event::Cancel(job.id)),
            ));
        }
        if job.decision.is_some() {
            controls = controls.push(button(
                "Review".into(),
                format!("review-{}", job.id),
                Some(Event::Review(job.id)),
            ));
        }
        if job.phase.needs_save() {
            controls = controls.push(button(
                "Retry Save".into(),
                format!("save-{}", job.id),
                Some(Event::RetrySave(job.id)),
            ));
        }
        for (enabled, label, part) in [
            (
                job.transfer_actions.retry,
                "Retry failed items",
                crate::operation_jobs::TransferPart::Failed,
            ),
            (
                job.transfer_actions.remaining,
                "Continue remaining work",
                crate::operation_jobs::TransferPart::Remaining,
            ),
        ] {
            if enabled {
                controls = controls.push(button(
                    label.into(),
                    format!("transfer-{part:?}-{}", job.id),
                    Some(Event::ResumeTransfer(job.id, part)),
                ));
            }
        }
        if job.transfer_actions.abandon {
            controls = controls.push(button(
                "Abandon unfinished work".into(),
                format!("abandon-{}", job.id),
                Some(Event::AbandonTransfer(job.id)),
            ));
        }
        if job.dismissible {
            controls = controls.push(button(
                "Dismiss request".into(),
                format!("dismiss-{}", job.id),
                Some(Event::DismissRequest(job.id)),
            ));
            content = content.push(
                text(
                    "Dismiss releases this saved request. Completed work and Undo stay available.",
                )
                .font(font)
                .size(small),
            );
        }
        for (enabled, label, part) in [
            (
                job.recovery_actions.retry,
                "Retry failed Recovery copies",
                crate::operation_jobs::RecoveryPart::Failed,
            ),
            (
                job.recovery_actions.remaining,
                "Continue Recovery copy",
                crate::operation_jobs::RecoveryPart::Remaining,
            ),
        ] {
            if enabled {
                controls = controls.push(button(
                    label.into(),
                    format!("recovery-{part:?}-{}", job.id),
                    Some(Event::ResumeRecovery(job.id, part)),
                ));
            }
        }
        if job.paths.first().and_then(|path| path.parent()).is_some() {
            controls = controls.push(button(
                "Open folder".into(),
                format!("open-{}", job.id),
                Some(Event::OpenFolder(job.id)),
            ));
        }
        content = content.push(controls.wrap());
        body = body.push(super::row_state::keyed_row(
            view::RowKey::FilesystemJob(job.id),
            container(content).padding(12).width(Length::Fill),
        ));
    }
    (body.into(), order)
}
