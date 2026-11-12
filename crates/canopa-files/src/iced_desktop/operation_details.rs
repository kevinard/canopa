//! Read-only inspection of one operation; the worker remains its sole executor.
use super::file_operations::Event;
use super::*;
use crate::operation_jobs::{JobSnapshot, Phase, TransferProgress, TransferReport};
use iced::{
    Length,
    widget::{column, container, responsive, row, scrollable, text},
};

const PAGE_SIZE: usize = 20;

pub(super) struct Dialog {
    pub window: window::Id,
    pub parent: window::Id,
    pub job: u64,
    pub section: Option<Section>,
    pub page: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Section {
    Completed,
    Failed,
    Skipped,
    Remaining,
    Sources,
}

impl Section {
    const ALL: [Self; 5] = [
        Self::Completed,
        Self::Failed,
        Self::Skipped,
        Self::Remaining,
        Self::Sources,
    ];

    fn label(self, count: usize) -> String {
        match self {
            Self::Completed => fl!("files-details-completed", count = count),
            Self::Failed => fl!("files-details-failed", count = count),
            Self::Skipped => fl!("files-details-skipped", count = count),
            Self::Remaining => fl!("files-details-remaining", count = count),
            Self::Sources => fl!("files-details-sources", count = count),
        }
    }

    fn compact_label(self, count: usize) -> String {
        match self {
            Self::Completed => fl!("files-details-completed-short", count = count),
            Self::Failed => fl!("files-details-failed-short", count = count),
            Self::Skipped => fl!("files-details-skipped-short", count = count),
            Self::Remaining => fl!("files-details-remaining-short", count = count),
            Self::Sources => fl!("files-details-sources-short", count = count),
        }
    }

    fn count(self, report: &TransferReport) -> usize {
        match self {
            Self::Completed => report.destinations.len(),
            Self::Failed => report.failures.len(),
            Self::Skipped => report.skipped.len(),
            Self::Remaining => report.remaining.len(),
            Self::Sources => report.source_remainders.len(),
        }
    }

    fn initial(report: &TransferReport) -> Self {
        [Self::Failed, Self::Remaining, Self::Sources]
            .into_iter()
            .find(|section| section.count(report) > 0)
            .unwrap_or(Self::Completed)
    }

    fn entry(self, report: &TransferReport, index: usize) -> String {
        let paths = match self {
            Self::Failed => {
                let failure = &report.failures[index];
                return fl!(
                    "files-details-failure",
                    source = display(&failure.source),
                    destination = display(&failure.destination),
                    error = bounded(&failure.message, false)
                );
            }
            Self::Completed => &report.destinations,
            Self::Skipped => &report.skipped,
            Self::Remaining => &report.remaining,
            Self::Sources => &report.source_remainders,
        };
        display(&paths[index])
    }
}

fn control(window: window::Id, index: usize) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-operation-details-{window:?}-{index}"))
}

fn scroll(window: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-operation-details-{window:?}-scroll"))
}

fn section_scroll(window: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-operation-details-{window:?}-sections"))
}

fn bounded(label: &str, more: bool) -> String {
    let mut chars = label.chars();
    let mut label: String = chars.by_ref().take(4096).collect();
    if more || chars.next().is_some() {
        label.push_str(&fl!("files-details-truncated"));
    }
    label
}

fn display(path: &Path) -> String {
    let bytes = path.as_os_str().as_encoded_bytes();
    let end = bytes.len().min(4096 * 4);
    bounded(&String::from_utf8_lossy(&bytes[..end]), end < bytes.len())
}

pub(super) fn progress<'a>(
    job: &'a JobSnapshot,
    appearance: &'a canopa_theme::Appearance,
    home: &Path,
) -> Option<Element<'a, Message>> {
    if !matches!(job.phase, Phase::Running | Phase::Waiting) {
        return None;
    }
    let progress = job.progress.as_ref()?;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().label_size.px();
    let name = bounded(
        &progress
            .path
            .file_name()
            .unwrap_or(progress.path.as_os_str())
            .to_string_lossy(),
        false,
    );
    let title = if job.phase == Phase::Waiting {
        fl!("files-progress-paused-at", name = name)
    } else if progress.verifying {
        fl!("files-progress-checking", name = name)
    } else {
        fl!("files-progress-transferring", name = name)
    };
    let mut description = vec![progress_counts(progress)];
    description.extend(progress_rate(progress));
    let fraction = progress_fraction(progress);
    let node = super::accessibility_ui::Node::progress(title.clone(), fraction.map(f64::from))
        .key(format!("files-progress-{}", job.id))
        .description(description.join(", "));
    let counts = text(progress_counts(progress))
        .font(font)
        .size(size)
        .wrapping(text::Wrapping::WordOrGlyph);
    // While totals are still counting the progress is indeterminate.
    let counts: Element<'a, Message> = if fraction.is_none() {
        super::accessibility_ui::semantic(node.clone(), counts)
    } else {
        counts.into()
    };
    Some(
        column![
            chrome::section_title(appearance, title).wrapping(text::Wrapping::WordOrGlyph),
            text(bounded(
                &super::question_dialog::display_location(home, &progress.path),
                false
            ))
            .font(font)
            .size(size)
            .style(canopa_ui::muted)
            .wrapping(text::Wrapping::WordOrGlyph),
            fraction.map(|fraction| super::accessibility_ui::semantic(
                node.clone(),
                iced::widget::progress_bar(0.0..=1.0, fraction)
                    .girth(4)
                    .style(iced::widget::progress_bar::primary)
            )),
            counts,
            progress_rate(progress).map(|rate| {
                text(rate)
                    .font(font)
                    .size(size)
                    .style(canopa_ui::muted)
                    .wrapping(text::Wrapping::WordOrGlyph)
            }),
        ]
        .spacing(6)
        .into(),
    )
}

/// Weighted like the remaining-time estimate: content plus per-item work.
fn progress_fraction(progress: &TransferProgress) -> Option<f32> {
    let total = progress.total.filter(|total| total.complete)?;
    const ITEM: f64 = 256.0 * 1024.0;
    let work = |items: u64, bytes: u64| items as f64 * ITEM + bytes as f64;
    let all = work(total.items, total.bytes);
    (all > 0.0)
        .then(|| (work(progress.completed as u64, progress.bytes) / all).clamp(0.0, 1.0) as f32)
}

/// "X of Y items · A of B", or the running counts while the scan continues.
pub(super) fn progress_counts(progress: &TransferProgress) -> String {
    let done = progress.completed as u64;
    let size = super::listing_ui::size;
    match progress.total {
        Some(total) if total.complete => fl!(
            "files-progress-counts",
            done = done,
            total = total.items.max(done),
            bytes = size(progress.bytes),
            total_bytes = size(total.bytes.max(progress.bytes))
        ),
        Some(total) => fl!(
            "files-progress-counting",
            done = done,
            total = total.items.max(done),
            bytes = size(progress.bytes)
        ),
        None => fl!(
            "files-progress-running-counts",
            done = done,
            bytes = size(progress.bytes)
        ),
    }
}

/// "12.0 MiB/s · about 2 min left" once the estimate has settled.
pub(super) fn progress_rate(progress: &TransferProgress) -> Option<String> {
    let rate = progress.bytes_per_second.map(|rate| {
        fl!(
            "files-progress-rate",
            size = super::listing_ui::size(rate as u64)
        )
    });
    let remaining = progress.remaining.map(|left| {
        let seconds = left.as_secs();
        let rounded = match seconds {
            0..5 => return fl!("files-progress-few-seconds-left"),
            5..60 => seconds.div_ceil(5) * 5,
            60..3600 => seconds.div_ceil(60) * 60,
            _ => seconds,
        };
        fl!(
            "files-progress-time-left",
            duration = canopa_i18n::format::duration(rounded)
        )
    });
    match (rate, remaining) {
        (Some(rate), Some(left)) => Some(fl!(
            "files-progress-rate-and-time",
            rate = rate,
            left = left
        )),
        (rate, left) => rate.or(left),
    }
}

impl Desktop {
    pub(super) fn show_operation_details(&mut self, job: u64) -> Task<Message> {
        let Some(snapshot) = self
            .file_operations
            .snapshot
            .jobs
            .iter()
            .find(|entry| entry.id == job)
        else {
            return Task::none();
        };
        let height = if snapshot.transfer.is_some() {
            440.0
        } else {
            310.0
        };
        if let Some(dialog) = &mut self.file_operations.details {
            if dialog.job != job {
                dialog.section = None;
                dialog.page = 0;
            }
            dialog.job = job;
            return iced::widget::operation::scroll_to(
                scroll(dialog.window),
                scrollable::AbsoluteOffset {
                    x: None,
                    y: Some(0.0),
                },
            )
            .chain(window::gain_focus(dialog.window));
        }
        let Some(parent) = self.operations_window else {
            return self.show_operations();
        };
        let (id, opened) = window::open_transient(
            parent,
            window::Settings {
                size: Size::new(680.0, height),
                min_size: Some(Size::new(320.0, 240.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Files".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        self.file_operations.details = Some(Dialog {
            window: id,
            parent,
            job,
            section: None,
            page: 0,
        });
        opened.map(move |opened| Message::FileOperation(Event::DetailsOpened(id, opened)))
    }

    pub(super) fn operation_details_opened(
        &mut self,
        id: window::Id,
        opened: Option<window::Id>,
    ) -> Task<Message> {
        if self
            .file_operations
            .details
            .as_ref()
            .is_none_or(|dialog| dialog.window != id)
        {
            return window::close(id);
        }
        if opened.is_none() {
            self.file_operations.details = None;
            self.file_operations.error = Some(fl!("files-details-failed-to-open"));
            return self.exit_if_empty();
        }
        iced::widget::operation::focus(control(id, 0)).chain(window::gain_focus(id))
    }

    pub(super) fn operation_details_section(
        &mut self,
        window: window::Id,
        job: u64,
        section: Section,
    ) -> Task<Message> {
        let available = self
            .file_operations
            .snapshot
            .jobs
            .iter()
            .any(|entry| entry.id == job && entry.transfer.is_some());
        let Some(dialog) = self
            .file_operations
            .details
            .as_mut()
            .filter(|dialog| dialog.window == window && dialog.job == job && available)
        else {
            return Task::none();
        };
        dialog.section = Some(section);
        dialog.page = 0;
        iced::widget::operation::scroll_to(
            scroll(window),
            scrollable::AbsoluteOffset {
                x: None,
                y: Some(0.0),
            },
        )
    }

    pub(super) fn operation_details_page(
        &mut self,
        window: window::Id,
        job: u64,
        section: Section,
        page: usize,
    ) -> Task<Message> {
        let Some(report) = self
            .file_operations
            .snapshot
            .jobs
            .iter()
            .find(|entry| entry.id == job)
            .and_then(|entry| entry.transfer.as_ref())
        else {
            return Task::none();
        };
        let Some(dialog) = self.file_operations.details.as_mut().filter(|dialog| {
            dialog.window == window
                && dialog.job == job
                && dialog.section.unwrap_or_else(|| Section::initial(report)) == section
        }) else {
            return Task::none();
        };
        let last = section.count(report).saturating_sub(1) / PAGE_SIZE;
        let page = page.min(last);
        let next = page > dialog.page;
        dialog.page = page;
        let focused = if page == 0 || next && page < last {
            7
        } else {
            6
        };
        iced::widget::operation::scroll_to(
            scroll(window),
            scrollable::AbsoluteOffset {
                x: None,
                y: Some(0.0),
            },
        )
        .chain(iced::widget::operation::focus(control(window, focused)))
    }

    pub(super) fn close_operation_details(&mut self, id: window::Id) -> Task<Message> {
        if self
            .file_operations
            .details
            .as_ref()
            .is_none_or(|dialog| dialog.window != id)
        {
            return Task::none();
        }
        let dialog = self.file_operations.details.take().unwrap();
        let focus = if self.operations_window == Some(dialog.parent) {
            window::gain_focus(dialog.parent)
        } else {
            Task::none()
        };
        focus.chain(window::close(id)).chain(self.exit_if_empty())
    }

    pub(super) fn close_operation_details_parent(&mut self, parent: window::Id) -> Task<Message> {
        if self
            .file_operations
            .details
            .as_ref()
            .is_some_and(|dialog| dialog.parent == parent)
        {
            let dialog = self.file_operations.details.take().unwrap();
            return window::close(dialog.window);
        }
        Task::none()
    }

    pub(super) fn operation_details_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || modifiers.control() && key.as_ref() == keyboard::Key::Character("w")
        {
            return self.close_operation_details(id);
        }
        super::scrolling::key(scroll(id), key, modifiers)
    }
}

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    responsive(move |size| contents(desktop, dialog, size)).into()
}

fn contents<'a>(desktop: &'a Desktop, dialog: &'a Dialog, viewport: Size) -> Element<'a, Message> {
    let compact = viewport.height < 300.0;
    let padding = if compact { 8.0 } else { 16.0 };
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let small = appearance.typography().label_size.px();
    let title: Element<'_, Message> = if compact {
        chrome::section_title(appearance, fl!("files-transfer-details-title")).into()
    } else {
        chrome::dialog_heading(
            desktop,
            chrome::Icon::Activity,
            fl!("files-transfer-details-title"),
            "",
        )
    };
    let mut facts = column![title].spacing(12);
    let mut sections = row![].spacing(8);
    let mut pagination = None;
    let mut review = None;
    let mut order = vec![control(dialog.window, 0)];
    if let Some(job) = desktop
        .file_operations
        .snapshot
        .jobs
        .iter()
        .find(|job| job.id == dialog.job)
    {
        let mut overview = column![
            row![
                chrome::section_title(
                    appearance,
                    bounded(&crate::history_labels::title(&job.title), false)
                )
                .width(Length::Fill)
                .wrapping(text::Wrapping::WordOrGlyph),
                super::file_operation_rows::phase_badge(appearance, job)
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
        ]
        .spacing(10);
        if let Some(progress) = progress(job, appearance, &desktop.home) {
            overview = overview.push(progress);
        }
        let destination = match job.decision.as_ref().map(|decision| &decision.question) {
            Some(crate::operation_jobs::Question::Transfer(
                crate::operation_jobs::TransferQuestion::Conflict { destination, .. },
            )) => destination.path.parent(),
            Some(crate::operation_jobs::Question::Transfer(
                crate::operation_jobs::TransferQuestion::Unavailable { destination, .. },
            )) => Some(destination.as_path()),
            _ => job
                .transfer
                .as_ref()
                .map(|report| report.destination.as_path()),
        };
        if let Some(destination) = destination {
            let path = super::question_dialog::display_location(&desktop.home, destination);
            overview = overview.push(
                text(fl!("files-undo-destination", location = path))
                    .font(font)
                    .size(small)
                    .style(canopa_ui::muted)
                    .wrapping(text::Wrapping::WordOrGlyph),
            );
        }
        if job.decision.is_some() {
            let id = control(dialog.window, 8);
            order.push(id.clone());
            let message = Message::FileOperation(Event::Review(job.id));
            review = Some(super::focus::button(
                id,
                Some(message.clone()),
                appearance,
                canopa_ui::styled_action(
                    text(fl!("files-review-decision")).font(font).size(small),
                    Some(message),
                    canopa_ui::Action::Primary,
                    false,
                    appearance,
                ),
            ));
        }
        facts = facts.push(chrome::card(appearance, overview));
        for message in [job.detail.as_ref(), job.error.as_ref()]
            .into_iter()
            .flatten()
        {
            facts = facts.push(
                text(bounded(message, false))
                    .font(font)
                    .size(small)
                    .wrapping(text::Wrapping::WordOrGlyph),
            );
        }
        if let Some(report) = &job.transfer {
            let section = dialog.section.unwrap_or_else(|| Section::initial(report));
            let count = section.count(report);
            let last = count.saturating_sub(1) / PAGE_SIZE;
            let page = dialog.page.min(last);
            let start = page * PAGE_SIZE;
            let end = start.saturating_add(PAGE_SIZE).min(count);
            facts = facts.push(
                chrome::section_title(appearance, section.label(count))
                    .wrapping(text::Wrapping::WordOrGlyph),
            );
            if count == 0 {
                facts = facts.push(
                    text(fl!("files-details-section-empty"))
                        .font(font)
                        .size(small),
                );
            }
            for index in start..end {
                facts = facts.push(
                    text(fl!(
                        "files-details-entry",
                        number = index.saturating_add(1),
                        entry = section.entry(report, index)
                    ))
                    .font(font)
                    .size(small)
                    .wrapping(text::Wrapping::WordOrGlyph),
                );
            }
            for (index, choice) in Section::ALL.into_iter().enumerate() {
                let id = control(dialog.window, index + 1);
                order.push(id.clone());
                let message = Message::FileOperation(Event::DetailsSection(
                    dialog.window,
                    dialog.job,
                    choice,
                ));
                sections = sections.push(super::focus::button(
                    id,
                    Some(message.clone()),
                    appearance,
                    canopa_ui::styled_action(
                        text(if compact {
                            choice.compact_label(choice.count(report))
                        } else {
                            choice.label(choice.count(report))
                        })
                        .font(font)
                        .size(small),
                        Some(message),
                        if section == choice {
                            canopa_ui::Action::Normal
                        } else {
                            canopa_ui::Action::Quiet
                        },
                        false,
                        appearance,
                    ),
                ));
            }
            let mut controls = row![
                text(if count == 0 {
                    fl!("files-status-items", count = 0_usize)
                } else {
                    fl!(
                        "files-details-page",
                        first = start.saturating_add(1),
                        last = end,
                        count = count
                    )
                })
                .font(font)
                .size(small)
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center);
            for (index, label, target) in [
                (6, fl!("files-previous"), (page > 0).then(|| page - 1)),
                (7, fl!("files-next"), (page < last).then_some(page + 1)),
            ] {
                let id = control(dialog.window, index);
                let message = target.map(|page| {
                    Message::FileOperation(Event::DetailsPage(
                        dialog.window,
                        dialog.job,
                        section,
                        page,
                    ))
                });
                if message.is_some() {
                    order.push(id.clone());
                }
                controls = controls.push(super::focus::button(
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
                ));
            }
            pagination = Some(controls.wrap());
        }
    } else {
        facts = facts.push(text(fl!("files-details-gone")).font(font).size(small));
    }
    let message = Message::FileOperation(Event::DetailsClose(dialog.window));
    let close = super::focus::button(
        control(dialog.window, 0),
        Some(message.clone()),
        appearance,
        canopa_ui::styled_action(
            text(fl!("files-close")).font(font).size(small),
            Some(message),
            canopa_ui::Action::Normal,
            false,
            appearance,
        ),
    );
    let reveal = if compact {
        Section::ALL
            .into_iter()
            .enumerate()
            .map(|(index, _)| {
                (
                    control(dialog.window, index + 1),
                    section_scroll(dialog.window),
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    // Keep this tree identical across reflow: control focus belongs to widget
    // state. Horizontal scrolling gives the compact row unconstrained width;
    // normal-height wrapping is bounded to the container's padded viewport.
    let scrollbar = if compact && order.len() > 1 {
        scrollable::Scrollbar::new()
            .width(6.0)
            .scroller_width(6.0)
            .spacing(4.0)
    } else {
        scrollable::Scrollbar::new()
    };
    let sections = scrollable(
        sections
            .width(if compact {
                Length::Shrink
            } else {
                Length::Fixed((viewport.width - padding * 2.0).max(0.0))
            })
            .wrap(),
    )
    .id(section_scroll(dialog.window))
    .direction(scrollable::Direction::Horizontal(scrollbar))
    .width(Length::Fill);
    let appearance = appearance.clone();
    let body = container(
        column![
            scrollable(facts)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new()
                        .width(6)
                        .scroller_width(6)
                        .spacing(7),
                ))
                .id(scroll(dialog.window))
                .width(Length::Fill)
                .height(Length::Fill),
            sections,
            pagination,
            iced::widget::rule::horizontal(1).style(chrome::divider),
            row![iced::widget::space().width(Length::Fill), close, review,]
                .spacing(8)
                .align_y(iced::Alignment::Center)
        ]
        .spacing(if compact { 6 } else { 12 }),
    )
    .padding(padding)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| chrome::window_surface(&appearance, true));
    super::focus::group(body, order, reveal, Some(control(dialog.window, 0)))
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::operation_jobs::TransferTotal;
    use std::time::Duration;

    fn progress(total: Option<TransferTotal>) -> TransferProgress {
        TransferProgress {
            completed: 3,
            bytes: 2048,
            total,
            ..TransferProgress::default()
        }
    }

    #[test]
    fn counts_show_totals_after_counting_and_a_floor_while_counting() {
        assert_eq!(progress_counts(&progress(None)), "3 items · 2.0 KiB");
        let mut total = TransferTotal {
            items: 10,
            bytes: 4 * 1024 * 1024,
            complete: false,
        };
        assert_eq!(
            progress_counts(&progress(Some(total))),
            "3 of 10+ items · 2.0 KiB · counting…"
        );
        total.complete = true;
        assert_eq!(
            progress_counts(&progress(Some(total))),
            "3 of 10 items · 2.0 KiB of 4.0 MiB"
        );
        let fraction = progress_fraction(&progress(Some(total))).unwrap();
        assert!(fraction > 0.0 && fraction < 1.0);
        assert!(progress_fraction(&progress(None)).is_none());
    }

    #[test]
    fn rate_and_remaining_time_are_rounded() {
        let mut shown = progress(None);
        assert_eq!(progress_rate(&shown), None);
        shown.bytes_per_second = Some(12.0 * 1024.0 * 1024.0);
        assert_eq!(progress_rate(&shown).unwrap(), "12.0 MiB/s");
        shown.remaining = Some(Duration::from_secs(91));
        assert_eq!(
            progress_rate(&shown).unwrap(),
            "12.0 MiB/s · about 2 min left"
        );
        shown.remaining = Some(Duration::from_secs(12));
        assert_eq!(
            progress_rate(&shown).unwrap(),
            "12.0 MiB/s · about 15 s left"
        );
    }
}
