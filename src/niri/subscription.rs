use iced::futures::stream;
use niri_ipc::socket::SOCKET_PATH_ENV;
use niri_ipc::{Event, Request, Response};
use std::io::{self, BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use tokio::sync::mpsc;

use crate::app::Message;
use crate::app::messages::AppNiriEvent;
use crate::niri::NiriEvent;

#[derive(Hash)]
struct NiriSubscriptionKey;

fn should_emit_event(event: &Event) -> bool {
    !matches!(event, Event::WindowFocusTimestampChanged { .. })
}

#[derive(Debug)]
enum DecodedNiriEvent {
    Event(Event),
    Ignored,
}

fn decode_event_line(line: &str) -> Result<DecodedNiriEvent, String> {
    match serde_json::from_str::<Event>(line) {
        Ok(event) => Ok(DecodedNiriEvent::Event(event)),
        Err(error) if looks_like_unknown_event_variant(line, &error) => {
            Ok(DecodedNiriEvent::Ignored)
        }
        Err(error) => Err(error.to_string()),
    }
}

fn looks_like_unknown_event_variant(line: &str, error: &serde_json::Error) -> bool {
    if !error.to_string().contains("unknown variant") {
        return false;
    }

    matches!(
        serde_json::from_str::<serde_json::Value>(line),
        Ok(serde_json::Value::Object(map)) if map.len() == 1
    )
}

fn connect_event_stream() -> io::Result<BufReader<UnixStream>> {
    let socket_path = std::env::var_os(SOCKET_PATH_ENV).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{SOCKET_PATH_ENV} is not set, are you running this within niri?"),
        )
    })?;
    let stream = UnixStream::connect(std::path::PathBuf::from(socket_path))?;
    let mut reader = BufReader::new(stream);

    let mut request = serde_json::to_string(&Request::EventStream).unwrap();
    request.push('\n');
    reader.get_mut().write_all(request.as_bytes())?;

    let mut reply_line = String::new();
    reader.read_line(&mut reply_line)?;
    let reply = serde_json::from_str::<niri_ipc::Reply>(&reply_line)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    match reply {
        Ok(Response::Handled) => {
            let _ = reader.get_mut().shutdown(Shutdown::Write);
            Ok(reader)
        }
        Ok(other) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected EventStream response: {other:?}"),
        )),
        Err(message) => Err(io::Error::new(io::ErrorKind::ConnectionRefused, message)),
    }
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
                    let result =
                        tokio::task::spawn_blocking(|| -> Option<BufReader<UnixStream>> {
                            connect_event_stream().ok()
                        })
                        .await;

                    match result {
                        Ok(Some(mut reader)) => {
                            // Connected — hand off blocking read loop to an OS thread.
                            let (tx, rx) = mpsc::unbounded_channel();
                            std::thread::spawn(move || {
                                let mut line = String::new();
                                loop {
                                    line.clear();
                                    match reader.read_line(&mut line) {
                                        Ok(0) => {
                                            let _ = tx.send(Err("niri event stream closed".into()));
                                            break;
                                        }
                                        Ok(_) => match decode_event_line(&line) {
                                            Ok(DecodedNiriEvent::Event(event)) => {
                                                if tx.send(Ok(event)).is_err() {
                                                    break; // Receiver dropped — bar shutting down
                                                }
                                            }
                                            Ok(DecodedNiriEvent::Ignored) => {}
                                            Err(error) => {
                                                let _ = tx.send(Err(error));
                                                break;
                                            }
                                        },
                                        Err(error) => {
                                            let _ = tx.send(Err(error.to_string()));
                                            break;
                                        }
                                    }
                                }
                            });
                            Some((
                                Message::Niri(AppNiriEvent::Ipc(NiriEvent::Connected)),
                                NiriSubState::Running(rx),
                            ))
                        }
                        _ => {
                            // No Niri socket — graceful degradation (FR21, AC #2).
                            // Do NOT log to stderr. Emit once and terminate.
                            Some((
                                Message::Niri(AppNiriEvent::Ipc(NiriEvent::Unavailable)),
                                NiriSubState::Done,
                            ))
                        }
                    }
                }
                NiriSubState::Running(mut rx) => loop {
                    match rx.recv().await {
                        Some(Ok(event)) if should_emit_event(&event) => {
                            break Some((
                                Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(event))),
                                NiriSubState::Running(rx),
                            ));
                        }
                        Some(Ok(_)) => {}
                        _ => {
                            // Channel closed or error — treat as disconnection.
                            break Some((
                                Message::Niri(AppNiriEvent::Ipc(NiriEvent::Unavailable)),
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

    #[test]
    fn niri_subscription_ignores_unknown_event_stream_variants() {
        assert!(matches!(
            decode_event_line(r#"{"FutureNiriEvent":{"value":true}}"#),
            Ok(DecodedNiriEvent::Ignored)
        ));
    }

    #[test]
    fn niri_subscription_decodes_known_event_stream_variants() {
        assert!(matches!(
            decode_event_line(r#"{"WindowFocusChanged":{"id":7}}"#),
            Ok(DecodedNiriEvent::Event(Event::WindowFocusChanged {
                id: Some(7)
            }))
        ));
    }
}
