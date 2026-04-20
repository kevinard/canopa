//! Generic builder for D-Bus subscription streams.
//!
//! Each D-Bus subscription in this crate follows the same three-state shape:
//! spawn a task that pushes updates onto an mpsc channel, emit a mapped
//! `Message` for each update, and terminate after the channel closes.
//!
//! This module encapsulates that shape so each D-Bus backend only supplies
//! (a) the task body and (b) the message-mapping closures.

use iced::futures::{Stream, stream};
use std::{pin::Pin, sync::Arc};
use tokio::sync::mpsc;

type SpawnFn<T> = Box<dyn FnOnce(mpsc::UnboundedSender<Result<T, ()>>) + Send>;

/// Internal state for the generated stream.
#[allow(dead_code)]
enum SubState<T, M> {
    Init {
        spawn: Option<SpawnFn<T>>,
        on_ok: Arc<dyn Fn(T) -> M + Send + Sync>,
        on_err: Arc<dyn Fn() -> M + Send + Sync>,
    },
    Running {
        rx: mpsc::UnboundedReceiver<Result<T, ()>>,
        on_ok: Arc<dyn Fn(T) -> M + Send + Sync>,
        on_err: Arc<dyn Fn() -> M + Send + Sync>,
    },
    Done,
}

/// Build a boxed iced subscription stream from:
/// - `spawn`: starts the backend task, taking the `tx` side of the channel.
/// - `on_ok`: maps a successful update into the outgoing `Message`.
/// - `on_err`: produces the single "unavailable" message emitted before termination.
///
/// The returned stream emits one `Message` per update, followed by a single
/// `on_err` message if the backend task exits or the channel closes.
#[allow(dead_code)]
pub(super) fn build<T, M, Spawn, OnOk, OnErr>(
    spawn: Spawn,
    on_ok: OnOk,
    on_err: OnErr,
) -> Pin<Box<dyn Stream<Item = M> + Send + 'static>>
where
    T: Send + 'static,
    M: Send + 'static,
    Spawn: FnOnce(mpsc::UnboundedSender<Result<T, ()>>) + Send + 'static,
    OnOk: Fn(T) -> M + Send + Sync + 'static,
    OnErr: Fn() -> M + Send + Sync + 'static,
{
    let init = SubState::Init {
        spawn: Some(Box::new(spawn)),
        on_ok: Arc::new(on_ok),
        on_err: Arc::new(on_err),
    };

    Box::pin(stream::unfold(init, |state| async move {
        match state {
            SubState::Init {
                spawn,
                on_ok,
                on_err,
            } => {
                let (tx, mut rx) = mpsc::unbounded_channel();
                if let Some(f) = spawn {
                    f(tx);
                }
                match rx.recv().await {
                    Some(Ok(value)) => {
                        let msg = on_ok(value);
                        Some((msg, SubState::Running { rx, on_ok, on_err }))
                    }
                    _ => Some((on_err(), SubState::Done)),
                }
            }
            SubState::Running {
                mut rx,
                on_ok,
                on_err,
            } => match rx.recv().await {
                Some(Ok(value)) => {
                    let msg = on_ok(value);
                    Some((msg, SubState::Running { rx, on_ok, on_err }))
                }
                _ => Some((on_err(), SubState::Done)),
            },
            SubState::Done => None,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::futures::StreamExt;

    #[tokio::test(flavor = "current_thread")]
    async fn emits_ok_messages_then_terminates_with_err() {
        let mut stream = build::<u32, &'static str, _, _, _>(
            |tx| {
                tokio::spawn(async move {
                    let _ = tx.send(Ok(1));
                    let _ = tx.send(Ok(2));
                    // dropping tx closes the channel and triggers on_err
                });
            },
            |n| match n {
                1 => "one",
                2 => "two",
                _ => "other",
            },
            || "unavailable",
        );
        assert_eq!(stream.next().await, Some("one"));
        assert_eq!(stream.next().await, Some("two"));
        assert_eq!(stream.next().await, Some("unavailable"));
        assert_eq!(stream.next().await, None);
    }
}
