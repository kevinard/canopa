use iced::futures::stream;
use niri_ipc::socket::SOCKET_PATH_ENV;
use niri_ipc::{Event, Request, Response};
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

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

async fn connect_event_stream(path: &std::path::Path) -> io::Result<BufReader<UnixStream>> {
    let stream = UnixStream::connect(path).await?;
    let mut reader = BufReader::new(stream);
    let mut request = serde_json::to_string(&Request::EventStream).unwrap();
    request.push('\n');
    reader.get_mut().write_all(request.as_bytes()).await?;
    let mut reply_line = String::new();
    reader.read_line(&mut reply_line).await?;
    let reply = serde_json::from_str::<niri_ipc::Reply>(&reply_line)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    match reply {
        Ok(Response::Handled) => Ok(reader),
        other => Err(io::Error::other(format!(
            "unexpected EventStream response: {other:?}"
        ))),
    }
}

fn event_stream(path: Option<PathBuf>) -> impl iced::futures::Stream<Item = Message> {
    stream::unfold(
        (path, None::<BufReader<UnixStream>>, Duration::ZERO, false),
        |(path, mut reader, mut delay, mut unavailable)| async move {
            loop {
                if reader.is_none() {
                    tokio::time::sleep(delay).await;
                    let connected = match &path {
                        Some(path) => {
                            tokio::time::timeout(Duration::from_secs(2), connect_event_stream(path))
                                .await
                                .ok()
                                .and_then(Result::ok)
                        }
                        None => None,
                    };
                    if let Some(connected) = connected {
                        return Some((
                            Message::Niri(AppNiriEvent::Ipc(NiriEvent::Connected)),
                            (path, Some(connected), Duration::ZERO, false),
                        ));
                    }
                } else {
                    let mut line = String::new();
                    if let Ok(count) = reader.as_mut().unwrap().read_line(&mut line).await
                        && count > 0
                    {
                        match decode_event_line(&line) {
                            Ok(DecodedNiriEvent::Event(event)) if should_emit_event(&event) => {
                                return Some((
                                    Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(event))),
                                    (path, reader, delay, false),
                                ));
                            }
                            Ok(_) => continue,
                            Err(_) => {}
                        }
                    }
                    reader = None;
                }
                delay = (delay * 2)
                    .max(Duration::from_millis(250))
                    .min(Duration::from_secs(30));
                if !unavailable {
                    unavailable = true;
                    return Some((
                        Message::Niri(AppNiriEvent::Ipc(NiriEvent::Unavailable)),
                        (path, reader, delay, unavailable),
                    ));
                }
            }
        },
    )
}

/// Async socket ownership follows the subscription, including cancellation.
pub fn niri_subscription() -> iced::Subscription<Message> {
    iced::Subscription::run_with(NiriSubscriptionKey, |_: &NiriSubscriptionKey| {
        event_stream(std::env::var_os(SOCKET_PATH_ENV).map(PathBuf::from))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reconnects_after_initial_absence_and_after_eof() {
        use iced::futures::StreamExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("niri.sock");
        let events = event_stream(Some(path.clone()));
        tokio::pin!(events);
        assert!(matches!(
            events.next().await,
            Some(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Unavailable)))
        ));
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (socket, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(socket);
                let mut request = String::new();
                reader.read_line(&mut request).await.unwrap();
                assert!(matches!(
                    serde_json::from_str::<Request>(&request).unwrap(),
                    Request::EventStream
                ));
                reader
                    .get_mut()
                    .write_all(b"{\"Ok\":\"Handled\"}\n{\"WindowFocusChanged\":{\"id\":7}}\n")
                    .await
                    .unwrap();
            }
        });
        tokio::time::timeout(Duration::from_secs(3), async {
            for _ in 0..2 {
                assert!(matches!(
                    events.next().await,
                    Some(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Connected)))
                ));
                assert!(matches!(
                    events.next().await,
                    Some(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
                        Event::WindowFocusChanged { id: Some(7) }
                    ))))
                ));
                assert!(matches!(
                    events.next().await,
                    Some(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Unavailable)))
                ));
            }
        })
        .await
        .unwrap();
        server.await.unwrap();
    }

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
