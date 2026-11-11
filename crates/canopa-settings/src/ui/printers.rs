//! The Printers page: CUPS queues with their state, your default, a test
//! page and removal; print jobs with cancel; and driverless printers found on
//! the network to add.
use super::{Message, State, focus, live_page::LivePage, settings_group};
use crate::{
    live::{Event, LiveAction},
    printers::{PrinterAction, PrintersService, PrintersState, Queue, QueueState},
};
use iced::{
    Alignment, Element, Length,
    widget::{column, row, text},
};

pub(super) type PrintersPage = LivePage<PrintersService>;

pub(super) fn message(event: Event<PrintersService>) -> Message {
    Message::Printers(event)
}

fn apply(action: PrinterAction) -> Message {
    message(Event::Apply(action))
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
}

fn row_button<'a>(id: String, label: &'a str, message: Option<Message>) -> Element<'a, Message> {
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::button(label)
            .key(id.clone())
            .disabled(message.is_none())
            .on_click(message.clone()),
        focus::control(
            id,
            message.clone(),
            None,
            None,
            false,
            canopa_ui::action(text(label), message, canopa_ui::Action::Normal, false)
                .padding([6, 12]),
        ),
    )
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
            QueueState::Idle => "Ready".to_owned(),
            QueueState::Printing(job) => format!("Printing {job}"),
            QueueState::Stopped(reason) if reason.is_empty() => "Paused".to_owned(),
            QueueState::Stopped(reason) => format!("Paused: {reason}"),
        };
        if queue.default {
            status = format!("Your default · {status}");
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
        .width(Length::Fill);
        if !queue.location.is_empty() {
            details = details.push(muted(format!("Location: {}", queue.location)));
        }
        if !queue.uri.is_empty() {
            details = details.push(muted(queue.uri.clone()));
        }
        column![
            row![
                details,
                row_button(
                    format!("printers-{id}-default"),
                    "Make default",
                    (!queue.default).then(|| apply(PrinterAction::Default(queue.name.clone()))),
                ),
                row_button(
                    format!("printers-{id}-test"),
                    "Print test page",
                    Some(apply(PrinterAction::TestPage(queue.name.clone()))),
                ),
                row_button(
                    format!("printers-{id}-remove"),
                    "Remove",
                    Some(apply(PrinterAction::Remove(queue.name.clone()))),
                ),
            ]
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
        let mut queues = column![].spacing(12);
        if state.queues.is_empty() {
            queues = queues.push(muted("No printers are set up."));
        }
        for queue in &state.queues {
            queues = queues.push(self.queue_card(queue));
        }
        queues = queues.push(page.control_status("default", message));

        let mut jobs = column![].spacing(8);
        if state.jobs.is_empty() {
            jobs = jobs.push(muted("Nothing is waiting to print."));
        }
        for job in &state.jobs {
            let mine = job.user == state.user;
            jobs = jobs.push(
                column![
                    row![
                        column![
                            text(format!("Job {} on {}", job.number, job.queue)),
                            muted(format!(
                                "{} · {} kB",
                                if mine {
                                    "Yours".to_owned()
                                } else {
                                    job.user.clone()
                                },
                                job.size.div_ceil(1024)
                            )),
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                        row_button(
                            format!("printers-job-{}", key(&job.id)),
                            "Cancel",
                            (mine || state.helper)
                                .then(|| apply(PrinterAction::Cancel(job.clone()))),
                        ),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                    page.control_status(&PrinterAction::Cancel(job.clone()).control(), message),
                ]
                .spacing(4),
            );
        }

        let mut add = column![
            row![
                row_button(
                    "printers-search".into(),
                    if page.live.control("search") == crate::live::ControlStatus::Applying {
                        "Searching…"
                    } else {
                        "Search the network"
                    },
                    Some(apply(PrinterAction::Search)),
                ),
                muted("Finds printers that work without a driver (IPP Everywhere, AirPrint)."),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            page.control_status("search", message),
        ]
        .spacing(10);
        match &state.found {
            Some(found) if found.is_empty() => {
                add = add.push(muted(
                    "No new printers found. Check that the printer is on and on this network.",
                ));
            }
            Some(found) => {
                for printer in found {
                    add = add.push(
                        column![
                            row![
                                column![text(printer.name.clone()), muted(printer.uri.clone())]
                                    .spacing(2)
                                    .width(Length::Fill),
                                row_button(
                                    format!("printers-add-{}", key(&printer.uri)),
                                    "Add",
                                    Some(apply(PrinterAction::Add(printer.clone()))),
                                ),
                            ]
                            .spacing(10)
                            .align_y(Alignment::Center),
                            page.control_status(
                                &PrinterAction::Add(printer.clone()).control(),
                                message
                            ),
                        ]
                        .spacing(4),
                    );
                }
            }
            None => {}
        }
        add = add.push(muted(if state.helper {
            "Adding and removing printers asks for an administrator's permission."
        } else {
            "Adding and removing printers needs cups-pk-helper, or membership in CUPS's system group."
        }));
        column![
            settings_group("Printers", queues),
            settings_group("Print jobs", jobs),
            settings_group("Add a printer", add),
        ]
        .spacing(14)
        .into()
    }

    pub(super) fn printers_view(&self) -> Element<'_, Message> {
        self.printers
            .view(message, |state| self.printers_sections(state))
    }
}
