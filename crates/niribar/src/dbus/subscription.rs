//! Recovering subscriptions with bounded backoff. Backends must stop when the
//! receiver closes; each retry gets a fresh channel and fresh service proxies.
use iced::futures::{Stream, stream};
use std::{pin::Pin, time::Duration};
use tokio::sync::mpsc;

pub(super) fn build<T, M, Spawn, OnOk, OnErr>(
    spawn: Spawn,
    on_ok: OnOk,
    on_err: OnErr,
) -> Pin<Box<dyn Stream<Item = M> + Send + 'static>>
where
    T: Send + 'static,
    M: Send + 'static,
    Spawn: Fn(mpsc::UnboundedSender<Result<T, ()>>) + Send + 'static,
    OnOk: Fn(T) -> M + Send + 'static,
    OnErr: Fn() -> M + Send + 'static,
{
    let state = (
        spawn,
        on_ok,
        on_err,
        None::<mpsc::UnboundedReceiver<Result<T, ()>>>,
        Duration::ZERO,
        false,
    );
    Box::pin(stream::unfold(
        state,
        |(spawn, on_ok, on_err, mut rx, mut delay, mut unavailable)| async move {
            loop {
                if rx.is_none() {
                    tokio::time::sleep(delay).await;
                    let (tx, receiver) = mpsc::unbounded_channel();
                    spawn(tx);
                    rx = Some(receiver);
                }
                match rx.as_mut().expect("receiver initialized").recv().await {
                    Some(Ok(value)) => {
                        let message = on_ok(value);
                        return Some((message, (spawn, on_ok, on_err, rx, Duration::ZERO, false)));
                    }
                    _ => {
                        rx = None;
                        delay = (delay * 2)
                            .max(Duration::from_millis(250))
                            .min(Duration::from_secs(30));
                        if !unavailable {
                            unavailable = true;
                            let message = on_err();
                            return Some((message, (spawn, on_ok, on_err, rx, delay, unavailable)));
                        }
                    }
                }
            }
        },
    ))
}

/// A service can disappear without emitting a final property change. Watch its
/// owner as well as its data, and cancel the backend promptly with its subscriber.
pub(super) async fn while_service_owned<T>(
    conn: &zbus::Connection,
    service: &str,
    tx: &mpsc::UnboundedSender<T>,
    work: impl std::future::Future,
) {
    use iced::futures::StreamExt;
    let Ok(dbus) = zbus::fdo::DBusProxy::new(conn).await else {
        return;
    };
    let Ok(mut owners) = dbus.receive_name_owner_changed().await else {
        return;
    };
    tokio::pin!(work);
    loop {
        tokio::select! {
            _ = tx.closed() => return,
            _ = &mut work => return,
            signal = owners.next() => {
                let Some(signal) = signal else { return };
                if let Ok(args) = signal.args()
                    && args.name().as_str() == service
                { return; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::futures::StreamExt;
    #[tokio::test]
    async fn recovers_after_initial_failure_and_later_disconnect() {
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let mut stream = build(
            move |tx| {
                let attempt = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if attempt > 0 {
                    let _ = tx.send(Ok(attempt));
                }
            },
            Some,
            || None,
        );
        assert_eq!(stream.next().await, Some(None));
        assert_eq!(stream.next().await, Some(Some(1)));
        assert_eq!(stream.next().await, Some(None));
        assert_eq!(stream.next().await, Some(Some(2)));
    }
}
