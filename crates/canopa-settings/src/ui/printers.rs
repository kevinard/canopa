//! The Printers page: CUPS queues with their state, your default, a test
//! page and removal; print jobs with cancel; and driverless printers found on
//! the network to add.
use super::{Message, State, focus, kit, live_page::LivePage, settings_group};
use crate::{
    live::{Event, LiveAction},
    printers::{PrinterAction, PrintersService, PrintersState, Queue, QueueState},
};
use canopa_ui::direction;
use iced::{
    Alignment, Element, Length,
    widget::{column, text},
};

pub(super) type PrintersPage = LivePage<PrintersService>;

pub(super) fn message(event: Event<PrintersService>) -> Message {
    Message::Printers(event)
}

fn apply(action: PrinterAction) -> Message {
    message(Event::Apply(action))
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(13).style(canopa_ui::muted).into()
}

fn treated_button<'a>(
    id: String,
    label: String,
    message: Option<Message>,
    treatment: super::kit::Treatment,
) -> Element<'a, Message> {
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::button(label.clone())
            .key(id.clone())
            .disabled(message.is_none())
            .on_click(message.clone()),
        focus::control(
            id,
            message.clone(),
            None,
            None,
            false,
            iced::widget::button(text(label))
                .on_press_maybe(message)
                .style(move |theme, status| super::kit::button_style(theme, status, treatment))
                .padding([7, 14]),
        ),
    )
}

fn row_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
    treated_button(id, label, message, super::kit::Treatment::Secondary)
}
/// An action that removes or forgets something.
fn danger_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
    treated_button(id, label, message, super::kit::Treatment::Destructive)
}

fn key(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

impl State {
    fn queue_card<'a>(&'a self, queue: &'a Queue) -> Element<'a, Message> {
        let page = &self.printers;
        let id = key(&queue.name);
        let mut status = match &queue.state {
            QueueState::Idle => fl!("settings-printers-ready"),
            QueueState::Printing(job) => fl!("settings-printers-printing", job = job.as_str()),
            QueueState::Stopped(reason) if reason.is_empty() => fl!("settings-printers-paused"),
            // CUPS's own reason.
            QueueState::Stopped(reason) => {
                fl!("settings-printers-paused-because", reason = reason.as_str())
            }
        };
        if queue.default {
            status = fl!("settings-printers-default-status", status = status);
        }
        let mut details = column![
            text(if queue.description.is_empty() {
                queue.name.clone()
            } else {
                queue.description.clone()
            }),
            muted(status),
        ]
        .spacing(2)
        .align_x(direction::start())
        .width(Length::Fill);
        if !queue.location.is_empty() {
            details = details.push(muted(fl!(
                "settings-printers-location",
                location = queue.location.as_str()
            )));
        }
        if !queue.uri.is_empty() {
            details = details.push(muted(queue.uri.clone()));
        }
        column![
            direction::row([
                details.into(),
                row_button(
                    format!("printers-{id}-default"),
                    fl!("settings-printers-make-default"),
                    (!queue.default).then(|| apply(PrinterAction::Default(queue.name.clone()))),
                ),
                row_button(
                    format!("printers-{id}-test"),
                    fl!("settings-printers-test-page"),
                    Some(apply(PrinterAction::TestPage(queue.name.clone()))),
                ),
                danger_button(
                    format!("printers-{id}-remove"),
                    fl!("settings-printers-remove"),
                    Some(apply(PrinterAction::Remove(queue.name.clone()))),
                ),
            ])
            .spacing(8)
            .align_y(Alignment::Center),
            page.control_status(
                &PrinterAction::Remove(queue.name.clone()).control(),
                message
            ),
        ]
        .spacing(4)
        .into()
    }

    fn printers_sections<'a>(&'a self, state: &'a PrintersState) -> Element<'a, Message> {
        let page = &self.printers;
        let mut queues: Vec<Element<'_, Message>> = Vec::new();
        if state.queues.is_empty() {
            queues.push(super::kit::empty(fl!("settings-printers-none")));
        }
        for queue in &state.queues {
            queues.push(self.queue_card(queue));
        }
        let queues = column![kit::rows(queues), page.control_status("default", message)].spacing(4);

        let mut jobs: Vec<Element<'_, Message>> = Vec::new();
        if state.jobs.is_empty() {
            jobs.push(super::kit::empty(fl!("settings-printers-no-jobs")));
        }
        for job in &state.jobs {
            let mine = job.user == state.user;
            jobs.push(
                column![
                    direction::row([
                        column![
                            text(fl!(
                                "settings-printers-job",
                                number = job.number.to_string(),
                                queue = job.queue.as_str()
                            )),
                            muted(if mine {
                                fl!(
                                    "settings-printers-job-mine",
                                    size = canopa_i18n::format::size(job.size)
                                )
                            } else {
                                fl!(
                                    "settings-printers-job-of",
                                    user = job.user.as_str(),
                                    size = canopa_i18n::format::size(job.size)
                                )
                            }),
                        ]
                        .spacing(2)
                        .align_x(direction::start())
                        .width(Length::Fill)
                        .into(),
                        row_button(
                            format!("printers-job-{}", key(&job.id)),
                            fl!("settings-cancel"),
                            (mine || state.helper)
                                .then(|| apply(PrinterAction::Cancel(job.clone()))),
                        ),
                    ])
                    .spacing(10)
                    .align_y(Alignment::Center),
                    page.control_status(&PrinterAction::Cancel(job.clone()).control(), message),
                ]
                .spacing(4)
                .into(),
            );
        }
        let jobs = kit::rows(jobs);

        let mut add: Vec<Element<'_, Message>> = vec![
            column![
                kit::row(
                    fl!("settings-printers-search-label"),
                    Some(fl!("settings-printers-search-hint")),
                    treated_button(
                        "printers-search".into(),
                        if page.live.control("search") == crate::live::ControlStatus::Applying {
                            fl!("settings-printers-searching")
                        } else {
                            fl!("settings-printers-search")
                        },
                        Some(apply(PrinterAction::Search)),
                        // With nothing set up, searching is the page's next step.
                        if state.queues.is_empty() {
                            super::kit::Treatment::Primary
                        } else {
                            super::kit::Treatment::Secondary
                        },
                    ),
                ),
                page.control_status("search", message),
            ]
            .spacing(4)
            .into(),
        ];
        match &state.found {
            Some(found) if found.is_empty() => {
                add.push(super::kit::empty(fl!("settings-printers-none-found")));
            }
            Some(found) => {
                for printer in found {
                    add.push(
                        column![
                            direction::row([
                                column![text(printer.name.clone()), muted(printer.uri.clone())]
                                    .spacing(2)
                                    .align_x(direction::start())
                                    .width(Length::Fill)
                                    .into(),
                                row_button(
                                    format!("printers-add-{}", key(&printer.uri)),
                                    fl!("settings-printers-add"),
                                    Some(apply(PrinterAction::Add(printer.clone()))),
                                ),
                            ])
                            .spacing(10)
                            .align_y(Alignment::Center),
                            page.control_status(
                                &PrinterAction::Add(printer.clone()).control(),
                                message
                            ),
                        ]
                        .spacing(4)
                        .into(),
                    );
                }
            }
            None => {}
        }
        let helper = if state.helper {
            fl!("settings-printers-helper-hint")
        } else {
            fl!("settings-printers-no-helper-hint")
        };
        // With nothing set up, finding a printer is the next step: it joins
        // the empty list instead of waiting in a section further down.
        if state.queues.is_empty() {
            let mut first = vec![Element::from(queues)];
            first.extend(add);
            return column![
                super::noted_group(fl!("settings-printers"), kit::rows(first), [helper]),
                settings_group(fl!("settings-printers-jobs"), jobs),
            ]
            .spacing(28)
            .into();
        }
        column![
            settings_group(fl!("settings-printers"), queues),
            super::noted_group(fl!("settings-printers-add-title"), kit::rows(add), [helper]),
            settings_group(fl!("settings-printers-jobs"), jobs),
        ]
        .spacing(28)
        .into()
    }

    pub(super) fn printers_view(&self) -> Element<'_, Message> {
        self.printers
            .view(message, |state| self.printers_sections(state))
    }
}
