//! The live service page pattern in the window: runs [`Live`] effects
//! against the service, follows its updates while the page is open, and draws
//! the loading, unavailable and per-control states with the shared controls.
use super::{Message, button, focus, settings_group};
use crate::live::{ControlStatus, Effect, Event, Live, LiveService, ServiceError, Status};
use canopa_ui::accessibility::{Node, semantic};
use futures_util::{StreamExt, stream::BoxStream};
use iced::{
    Alignment, Element, Length, Subscription, Task,
    widget::{column, container, row, text},
};
use std::{hash::Hash, sync::Arc, time::Duration};

/// How long to wait before reconnecting after the service goes away.
const RECONNECT: Duration = Duration::from_secs(2);

/// One live page: its service and its state machine.
pub(super) struct LivePage<S: LiveService> {
    pub(super) service: Arc<S>,
    pub(super) live: Live<S>,
}

/// Identifies a page's update subscription by its service instance.
struct Service<S>(Arc<S>);

impl<S> Hash for Service<S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as *const () as usize).hash(state);
    }
}

enum Phase<S: LiveService> {
    Connect,
    Follow(BoxStream<'static, Result<S::State, ServiceError>>),
    Wait,
}

/// Load, then follow updates; when the service goes away, say so and
/// reconnect after a pause.
fn follow<S: LiveService>(service: &Service<S>) -> BoxStream<'static, Event<S>> {
    let service = service.0.clone();
    futures_util::stream::unfold(Phase::<S>::Connect, move |mut phase| {
        let service = service.clone();
        async move {
            loop {
                match phase {
                    Phase::Wait => {
                        tokio::time::sleep(RECONNECT).await;
                        phase = Phase::Connect;
                    }
                    Phase::Connect => {
                        // Subscribe first so no change between the two is lost.
                        let updates = service.updates();
                        let loaded = service.snapshot().await;
                        let next = if loaded.is_ok() {
                            Phase::Follow(updates)
                        } else {
                            Phase::Wait
                        };
                        return Some((Event::Loaded(loaded), next));
                    }
                    Phase::Follow(mut updates) => {
                        return Some(match updates.next().await {
                            Some(Ok(state)) => (Event::Changed(Ok(state)), Phase::Follow(updates)),
                            Some(Err(error)) => (Event::Changed(Err(error)), Phase::Wait),
                            None => (
                                Event::Changed(Err(ServiceError::Unavailable(format!(
                                    "{} stopped.",
                                    service.name()
                                )))),
                                Phase::Wait,
                            ),
                        });
                    }
                }
            }
        }
    })
    .boxed()
}

impl<S: LiveService> LivePage<S> {
    pub(super) fn new(service: S) -> Self {
        Self {
            service: Arc::new(service),
            live: Live::default(),
        }
    }

    /// Update the page and run the resulting service call.
    pub(super) fn update(
        &mut self,
        event: Event<S>,
        map: fn(Event<S>) -> Message,
    ) -> Task<Message> {
        match self.live.update(event) {
            None => Task::none(),
            Some(Effect::Load) => Task::perform(self.service.snapshot(), move |result| {
                map(Event::Loaded(result))
            }),
            Some(Effect::Apply(action)) => {
                let call = self.service.apply(action.clone());
                Task::perform(call, move |result| map(Event::Applied(action, result)))
            }
        }
    }

    /// Follow the service while the page is shown. `map` must be a function
    /// item, such as an enum variant, not a pointer or a capturing closure.
    pub(super) fn subscription<F>(&self, map: F) -> Subscription<Message>
    where
        F: Fn(Event<S>) -> Message + Clone + Send + 'static,
    {
        Subscription::run_with(Service(self.service.clone()), follow::<S>).map(map)
    }

    /// The page body: progress, "not available" with Try again, or `ready`.
    pub(super) fn view<'a>(
        &'a self,
        map: fn(Event<S>) -> Message,
        ready: impl FnOnce(&'a S::State) -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        match self.live.status() {
            Status::Loading => {
                let connecting = format!("Connecting to {}…", self.service.name());
                semantic(
                    Node::status(connecting.clone())
                        .key("settings-live-loading")
                        .busy(true),
                    text(connecting).style(canopa_ui::muted),
                )
            }
            Status::Unavailable(reason) => settings_group(
                "Service not available",
                column![
                    {
                        let message = format!(
                            "{} is not installed or not running. {reason}",
                            self.service.name()
                        );
                        semantic(
                            Node::status(message.clone()).key("settings-live-unavailable"),
                            text(message).style(canopa_ui::muted),
                        )
                    },
                    button(
                        "settings-button-try-again",
                        "Try again",
                        Some(map(Event::Reload)),
                        canopa_ui::Action::Normal,
                        Length::Shrink,
                        false,
                    ),
                ]
                .spacing(12),
            ),
            Status::Ready(state) => ready(state),
        }
    }

    /// A control's progress or error, shown under it. The element stays in
    /// the tree when idle so focus and typed text in later controls survive.
    pub(super) fn control_status(
        &self,
        control: &str,
        map: fn(Event<S>) -> Message,
    ) -> Element<'_, Message> {
        let status = format!("settings-live-{control}-status");
        let content: Element<'_, Message> = match self.live.control(control) {
            ControlStatus::Idle => {
                // An empty live region: the next state is announced when it appears.
                return semantic(
                    Node::status("").key(status),
                    container(text("")).height(Length::Fixed(1.)),
                );
            }
            ControlStatus::Applying => semantic(
                Node::status("Applying…").key(status).busy(true),
                text("Applying…").size(13).style(canopa_ui::muted),
            ),
            ControlStatus::Failed(error) => row![
                semantic(
                    Node::alert(error.to_owned()).key(status),
                    text(error.to_owned()).size(13).width(Length::Fill)
                ),
                control_button(
                    format!("settings-live-{control}-retry"),
                    "Retry",
                    map(Event::Retry(control.into())),
                    canopa_ui::Action::Normal,
                ),
                control_button(
                    format!("settings-live-{control}-dismiss"),
                    "Dismiss",
                    map(Event::Dismiss(control.into())),
                    canopa_ui::Action::Quiet,
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        };
        container(content).into()
    }
}

/// A button with a focus id unique to its control.
fn control_button<'a>(
    id: String,
    label: &'a str,
    message: Message,
    treatment: canopa_ui::Action,
) -> Element<'a, Message> {
    semantic(
        Node::button(label)
            .key(id.clone())
            .on_click(Some(message.clone())),
        focus::control(
            id,
            Some(message.clone()),
            None,
            None,
            false,
            canopa_ui::action(text(label), Some(message), treatment, false).padding([6, 12]),
        ),
    )
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use crate::live::fake;

    #[tokio::test]
    async fn pages_follow_the_service_and_reconnect_after_it_returns() {
        let service = Arc::new(fake::Service::default());
        let mut events = follow(&Service(service.clone()));
        assert!(matches!(events.next().await, Some(Event::Loaded(Ok(_)))));
        service.external(fake::State {
            enabled: false,
            level: 3,
        });
        assert!(matches!(
            events.next().await,
            Some(Event::Changed(Ok(fake::State { level: 3, .. })))
        ));
        service.set_running(false);
        assert!(matches!(
            events.next().await,
            Some(Event::Changed(Err(ServiceError::Unavailable(_))))
        ));
        service.set_running(true);
        assert!(matches!(events.next().await, Some(Event::Loaded(Ok(_)))));
    }
}
