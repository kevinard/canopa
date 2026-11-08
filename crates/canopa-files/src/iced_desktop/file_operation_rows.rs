use super::file_operations::Event;
use super::*;
use crate::operation_jobs::Phase;
use canopa_theme::ColorRole;
use iced::{
    Length,
    widget::{column, container, row, space, text},
};

fn control(window: window::Id, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-operations-{window:?}-{name}"))
}

pub(super) fn rows(
    desktop: &Desktop,
    window: window::Id,
    wide: bool,
) -> (
    Element<'_, Message>,
    Vec<iced::widget::Id>,
    Element<'_, Message>,
) {
    let files = &desktop.file_operations;
    let appearance = &desktop.configuration.appearance;
    let small = appearance.typography().label_size.px();
    let font = canopa_ui::font(appearance);
    let undo = files.snapshot.undo.as_ref();
    let undo_event = || {
        undo.filter(|_| files.ready() && !files.has_active())
            .map(|head| Event::Undo(window, head.id))
    };
    let mut order = Vec::new();
    if undo_event().is_some() {
        order.push(control(window, "undo"));
    }
    let mut button = |label: String, name: String, event: Option<Event>| {
        let id = control(window, &name);
        let message = event.map(Message::FileOperation);
        if message.is_some() && name != "undo" {
            order.push(id.clone());
        }
        let label = text(label).font(font).size(small).color_maybe(
            name.starts_with("open-")
                .then(|| chrome::ink(appearance, ColorRole::Accent)),
        );
        let content: Element<'_, Message> = if name == "recovery" {
            row![
                desktop.icons.chrome(chrome::Icon::Recovery, 15.0, 0.9),
                label
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .into()
        } else if name.starts_with("open-") {
            row![
                label,
                desktop.icons.chrome(chrome::Icon::Forward, 12.0, 0.86)
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            label.into()
        };
        super::focus::control(
            id,
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                content,
                message,
                if name.starts_with("open-") {
                    canopa_ui::Action::Quiet
                } else {
                    canopa_ui::Action::Normal
                },
                false,
                appearance,
            ),
        )
    };
    let page = files.page();
    let undo_row = undo.and_then(|head| {
        page.iter()
            .find(|job| report_operation(job) == Some(head.id))
            .map(|job| job.id)
    });
    let mut tools = row![].spacing(8);
    if let Some(head) = undo.filter(|_| undo_row.is_none()) {
        tools = tools.push(button(head.label.clone(), "undo".into(), undo_event()));
    }
    let recovery = button(
        "Recovery".into(),
        "recovery".into(),
        files.service.as_ref().map(|_| Event::Recovery),
    );
    let count = page.len();
    let count_label = if count == 1 {
        "1 operation".into()
    } else {
        format!("{count} operations")
    };
    let mut body = column![
        row![
            chrome::section_title(appearance, "File history"),
            space().width(Length::Fill),
            text(if count < 2 || !wide {
                count_label
            } else {
                format!("{count_label}  ·  Latest first")
            })
            .font(font)
            .size(small)
            .style(canopa_ui::muted),
            tools.wrap(),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(10);
    if files.before.is_some() || files.has_older() {
        body = body.push(
            row![
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
            .align_y(iced::Alignment::Center)
            .wrap(),
        );
    }
    if files.starting() {
        body = body.push(text("Preparing file operations…").font(font).size(small));
    }
    for error in [&files.error, &files.storage_error, &files.snapshot.error]
        .into_iter()
        .flatten()
    {
        body = body.push(text(error).font(font).size(small));
    }
    if let Some(error) = &files.notification_error {
        body = body.push(text(error).font(font).size(small).style(canopa_ui::muted));
    }
    if page.is_empty() && files.ready() {
        body = body.push(chrome::card(
            appearance,
            row![
                desktop.icons.chrome(chrome::Icon::Activity, 21.0, 0.9),
                column![
                    chrome::section_title(appearance, "No file operations yet"),
                    text("File changes made in Files will appear here.")
                        .font(font)
                        .size(small)
                        .style(canopa_ui::muted)
                        .wrapping(text::Wrapping::WordOrGlyph),
                ]
                .spacing(4),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        ));
    }
    let mut history = column![].spacing(0);
    let today = chrono::Local::now().date_naive();
    for (index, job) in page.into_iter().enumerate() {
        let renamed = job.title.strip_prefix("rename ");
        let created = job.title.strip_prefix("create folder ");
        let icon = if renamed.is_some() {
            chrome::Icon::Edit
        } else if created.is_some() {
            chrome::Icon::Folder
        } else if job.trash.is_some() {
            chrome::Icon::Trash
        } else if job.restore.is_some() || job.recovery.is_some() {
            chrome::Icon::Recovery
        } else if job.transfer.is_some() || job.progress.is_some() {
            chrome::Icon::Folder
        } else {
            chrome::Icon::Activity
        };
        let identity = container(desktop.icons.chrome(icon, 18.0, 0.9))
            .padding(8)
            .style(move |_| chrome::highlight(appearance, ColorRole::AccentSurface));
        let mut title = job.title.clone();
        if let Some(initial) = title.get_mut(..1) {
            initial.make_ascii_uppercase();
        }
        let folder = job.paths.first().and_then(|path| path.parent());
        let mut subtitle = job.detail.clone();
        if let Some(report) = job.trash.as_ref().filter(|report| {
            report.failures.is_empty()
                && report.remaining.is_empty()
                && !report.cancelled
                && report.interrupted.is_none()
        }) {
            if let [path] = report.completed.as_slice()
                && let Some(name) = path.file_name()
            {
                title = "Moved to Trash".into();
                subtitle = Some(name.to_string_lossy().into_owned());
            }
        } else if let Some(change) = renamed {
            // Completed jobs retain the destination path. Match its whole name
            // at the end of the receipt, since either name can contain " to ".
            let names = job.paths.last().and_then(|destination| {
                let current = destination.file_name()?.to_string_lossy();
                let previous = change.strip_suffix(&format!(" to {current}"))?;
                Some((previous, current))
            });
            if let Some((previous, current)) = names {
                title = format!("Renamed to {current}");
                subtitle = Some(format!("Previously {previous}"));
            } else {
                title = "Renamed".into();
                subtitle = Some(change.to_owned());
            }
        } else if let Some(name) = created {
            title = "Created folder".into();
            subtitle = Some(name.to_owned());
        }
        let location = folder
            .filter(|_| renamed.is_some() || created.is_some() || title == "Moved to Trash")
            .map(|path| super::question_dialog::display_location(&desktop.home, path));
        let muted = Some(chrome::ink(appearance, ColorRole::Muted));
        let mut labels =
            column![chrome::section_title(appearance, title).wrapping(text::Wrapping::WordOrGlyph)]
                .spacing(3)
                .width(Length::Fill);
        if let Some(detail) = subtitle {
            labels = labels.push(chrome::single_line(detail, font, small + 1.0, muted, true));
        }
        let accepted: Option<Element<'_, Message>> = job
            .accepted_at
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
            .map(|time| {
                let local = time.with_timezone(&chrono::Local);
                let visible = if local.date_naive() == today {
                    local.format("%H:%M").to_string()
                } else {
                    local.format("%b %-d · %H:%M").to_string()
                };
                iced::widget::tooltip(
                    text(visible)
                        .font(font)
                        .size(small + 1.0)
                        .style(canopa_ui::muted),
                    text(format!(
                        "Request accepted {}",
                        local.format("%b %-d, %Y · %H:%M:%S")
                    ))
                    .font(font)
                    .size(small),
                    iced::widget::tooltip::Position::Bottom,
                )
                .into()
            });
        if let Some(location) = location {
            labels = labels.push(chrome::single_line(
                location,
                font,
                small + 1.0,
                muted,
                true,
            ));
        }
        let status = column![
            phase_badge(appearance, job),
            accepted.map(|time| container(time).padding(iced::Padding {
                left: 8.0,
                ..iced::Padding::ZERO
            })),
        ]
        .align_x(iced::Alignment::Start);
        let mut header = row![identity, labels]
            .spacing(10)
            .align_y(iced::Alignment::Center);
        let mut content = column![].spacing(6);
        if let Some(progress) = super::operation_details::progress(job, appearance, &desktop.home) {
            content = content.push(progress);
        }
        if let Some(error) = &job.error {
            content = content.push(text(error).font(font).size(small));
        }
        if let Some(error) = files.notification_errors.get(&job.id) {
            content = content.push(text(error).font(font).size(small).style(canopa_ui::muted));
        }
        let mut controls = row![].spacing(8);
        let row_undo = undo_row == Some(job.id);
        let mut undo = row_undo.then(|| button("Undo".into(), "undo".into(), undo_event()));
        if !wide {
            controls = controls.push(undo.take());
        }
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
        let folder = folder.map(|_| {
            button(
                "Open folder".into(),
                format!("open-{}", job.id),
                Some(Event::OpenFolder(job.id)),
            )
        });
        if wide {
            header = header
                .push(container(status).width(106).align_x(iced::Alignment::Start))
                .push(
                    container(undo.unwrap_or_else(|| space().into()))
                        .width(56)
                        .align_x(iced::Alignment::End),
                )
                .push(
                    container(folder.unwrap_or_else(|| space().into()))
                        .width(106)
                        .align_x(iced::Alignment::End),
                );
        } else {
            header = header.push(status);
            controls = controls.push(folder);
        }
        let actions = container(row![space().width(44), controls.wrap()]);
        let content = column![header, content, actions].spacing(3);
        if index > 0 {
            history = history.push(iced::widget::rule::horizontal(1).style(chrome::divider));
        }
        history = history.push(super::row_state::keyed_row(
            view::RowKey::FilesystemJob(job.id),
            container(content).padding([11, 12]).width(Length::Fill),
        ));
    }
    if count > 0 {
        body = body.push(container(history).width(Length::Fill).style(move |_| {
            chrome::highlight(appearance, ColorRole::Raised).border(iced::Border {
                width: 1.0,
                color: chrome::ink(appearance, ColorRole::Border).scale_alpha(0.44),
                radius: 10.0.into(),
            })
        }));
    }
    (body.into(), order, recovery)
}

fn report_operation(job: &crate::operation_jobs::JobSnapshot) -> Option<u64> {
    job.trash
        .as_ref()
        .and_then(|report| report.operation)
        .or_else(|| job.restore.as_ref().and_then(|report| report.operation))
        .or_else(|| job.transfer.as_ref().and_then(|report| report.operation))
        .or_else(|| {
            job.recovery
                .as_ref()
                .and_then(|report| report.transfer.operation)
        })
}

pub(super) fn phase_badge<'a>(
    appearance: &'a canopa_theme::Appearance,
    job: &crate::operation_jobs::JobSnapshot,
) -> Element<'a, Message> {
    let completed = job.phase == Phase::Finished
        && job.error.is_none()
        && job
            .transfer
            .iter()
            .map(Arc::as_ref)
            .chain(job.recovery.iter().map(|report| &report.transfer))
            .all(|report| {
                report.failures.is_empty()
                    && report.remaining.is_empty()
                    && report.skipped.is_empty()
                    && !report.cancelled
                    && !report.needs_attention
            })
        && job.trash.as_ref().is_none_or(|report| {
            report.failures.is_empty()
                && report.remaining.is_empty()
                && !report.cancelled
                && report.interrupted.is_none()
        })
        && job
            .recovery
            .as_ref()
            .is_none_or(|report| report.error.is_none() && !report.interrupted)
        && job.restore.as_ref().is_none_or(|report| {
            report.error.is_none() && !report.cancelled && !report.interrupted
        });
    let role = match job.phase {
        Phase::Failed | Phase::StartSaveFailed | Phase::SaveFailed => ColorRole::Danger,
        Phase::Waiting | Phase::AwaitingReview | Phase::Interrupted | Phase::Stopped => {
            ColorRole::Warning
        }
        Phase::Running | Phase::Queued => ColorRole::Accent,
        Phase::Finished if completed => ColorRole::Success,
        Phase::Finished => ColorRole::Muted,
    };
    let ink = chrome::ink(appearance, role);
    container(
        row![
            text(if completed { "✓" } else { "●" })
                .font(canopa_ui::font(appearance))
                .size(if completed { 13 } else { 8 })
                .color(ink),
            text(if completed {
                "Completed"
            } else {
                file_operations::status(job.phase)
            })
            .font(canopa_ui::font(appearance))
            .size(appearance.typography().label_size.px())
            .color_maybe(completed.then(|| chrome::ink(appearance, ColorRole::Muted))),
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center),
    )
    .max_width(145)
    .padding([5, 8])
    .style(move |_| {
        container::Style::default()
            .background(ink.scale_alpha(if completed { 0.0 } else { 0.13 }))
            .border(iced::Border::default().rounded(7))
    })
    .into()
}
