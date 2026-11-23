//! A hidden debug-build page that runs the live page pattern on the fake
//! service. Open it with `canopa-settings live-example`.
use super::{Message, State, adjust, live_page::LivePage, settings_group, toggle};
use crate::live::{
    Event,
    fake::{Action, Service},
};
use iced::{
    Element,
    widget::{column, row, text},
};
use std::time::Duration;

pub(super) type Example = LivePage<Service>;

/// Changes made to the simulated service itself, not through the page.
#[derive(Clone, Debug)]
pub(super) enum Simulate {
    Running(bool),
    RefuseNext,
    Slow(bool),
}

pub(super) fn message(event: Event<Service>) -> Message {
    Message::LiveExample(event)
}

impl State {
    pub(super) fn simulate(&mut self, simulate: Simulate) {
        let service = &self.live_example.service;
        match simulate {
            Simulate::Running(running) => service.set_running(running),
            Simulate::RefuseNext => service.refuse(1),
            Simulate::Slow(slow) => service.set_delay(if slow {
                Duration::from_millis(1500)
            } else {
                Duration::ZERO
            }),
        }
    }

    pub(super) fn live_example_view(&self) -> Element<'_, Message> {
        let page = &self.live_example;
        let service = &page.service;
        let body = page.view(message, |state| {
            // Show a change being applied rather than the old value.
            let level = match page.live.pending("level") {
                Some(Action::Level(level)) => *level,
                _ => state.level,
            };
            settings_group(
                "Example service",
                column![
                    toggle(
                        "live-example-enabled",
                        "Example switch",
                        state.enabled,
                        message(Event::Apply(Action::Enabled(!state.enabled))),
                    ),
                    page.control_status("enabled", message),
                    text(format!("Example level: {level}%")),
                    adjust(
                        "live-example-level",
                        0.0..=100.,
                        f64::from(level),
                        5.,
                        |value| message(Event::Apply(Action::Level(value as u8))),
                    ),
                    page.control_status("level", message),
                ]
                .spacing(10),
            )
        });
        let slow = !service.delay().is_zero();
        column![
            text("Changes apply at once, without Save. This page exists only in debug builds."),
            body,
            settings_group(
                "Simulation",
                column![
                    text("These controls change the simulated service directly.")
                        .size(13)
                        .style(canopa_ui::muted),
                    row![
                        self.action(
                            "settings-button-toggle-service",
                            if service.running() {
                                "Stop the service"
                            } else {
                                "Start the service"
                            },
                            Some(Message::Simulate(Simulate::Running(!service.running()))),
                            false,
                        ),
                        self.action(
                            "settings-button-refuse-the-next-change",
                            "Refuse the next change",
                            Some(Message::Simulate(Simulate::RefuseNext)),
                            false,
                        ),
                    ]
                    .spacing(12),
                    toggle(
                        "live-example-slow",
                        "Slow replies",
                        slow,
                        Message::Simulate(Simulate::Slow(!slow)),
                    ),
                ]
                .spacing(12),
            ),
        ]
        .spacing(14)
        .into()
    }
}
