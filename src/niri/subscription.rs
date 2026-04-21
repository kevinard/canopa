use iced::futures::stream;
use niri_ipc::socket::Socket;
use niri_ipc::{Event, Request};
use tokio::sync::mpsc;

use crate::app::Message;
use crate::niri::NiriEvent;

#[derive(Hash)]
struct NiriSubscriptionKey;

fn should_emit_event(event: &Event) -> bool {
    !matches!(event, Event::WindowFocusTimestampChanged { .. })
}

/// Internal subscription state machine.
enum NiriSubState {
    /// Not yet connected; will attempt connection on next poll.
    Disconnected,
    /// Connected; receiving events from the background thread via channel.
    Running(mpsc::UnboundedReceiver<Result<Event, String>>),
    /// Terminal state — no Niri socket or connection dropped. Stream ends here.
    Done,
}

/// Returns an Iced Subscription that connects to the Niri IPC socket and emits
/// `Message::NiriEvent(NiriEvent)` for each event from the compositor.
///
/// Emits `NiriEvent::Unavailable` (once) and terminates if the socket is unreachable.
/// Uses a single OS thread for blocking I/O, bridged to async via mpsc channel (AR4).
pub fn niri_subscription() -> iced::Subscription<Message> {
    iced::Subscription::run_with(NiriSubscriptionKey, |_: &NiriSubscriptionKey| {
        stream::unfold(NiriSubState::Disconnected, |state| async move {
            match state {
                NiriSubState::Disconnected => {
                    // Attempt connection on a blocking thread (socket open may block briefly).
                    let result = tokio::task::spawn_blocking(|| -> Option<Socket> {
                        let mut socket = Socket::connect().ok()?;
                        let _ = socket.send(Request::EventStream).ok()?;
                        Some(socket)
                    })
                    .await;

                    match result {
                        Ok(Some(socket)) => {
                            // Connected — hand off blocking read loop to an OS thread.
                            let (tx, rx) = mpsc::unbounded_channel();
                            std::thread::spawn(move || {
                                let mut read_event = socket.read_events();
                                loop {
                                    match read_event() {
                                        Ok(event) => {
                                            if tx.send(Ok(event)).is_err() {
                                                break; // Receiver dropped — bar shutting down
                                            }
                                        }
                                        Err(e) => {
                                            let _ = tx.send(Err(e.to_string()));
                                            break;
                                        }
                                    }
                                }
                            });
                            Some((
                                Message::NiriEvent(NiriEvent::Connected),
                                NiriSubState::Running(rx),
                            ))
                        }
                        _ => {
                            // No Niri socket — graceful degradation (FR21, AC #2).
                            // Do NOT log to stderr. Emit once and terminate.
                            Some((Message::NiriEvent(NiriEvent::Unavailable), NiriSubState::Done))
                        }
                    }
                }
                NiriSubState::Running(mut rx) => loop {
                    match rx.recv().await {
                        Some(Ok(event)) if should_emit_event(&event) => {
                            break Some((
                                Message::NiriEvent(NiriEvent::Event(event)),
                                NiriSubState::Running(rx),
                            ));
                        }
                        Some(Ok(_)) => {}
                        _ => {
                            // Channel closed or error — treat as disconnection.
                            break Some((
                                Message::NiriEvent(NiriEvent::Unavailable),
                                NiriSubState::Done,
                            ));
                        }
                    }
                },
                NiriSubState::Done => None, // Stream exhausted; Iced will not restart it.
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn niri_subscription_filters_focus_timestamp_events() {
        assert!(!should_emit_event(&Event::WindowFocusTimestampChanged {
            id: 1,
            focus_timestamp: None,
        }));
    }

    #[test]
    fn niri_subscription_keeps_focus_identity_events() {
        assert!(should_emit_event(&Event::WindowFocusChanged {
            id: Some(1)
        }));
    }
}
