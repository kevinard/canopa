//! Bounded bus connections and the shared observation pattern.
//!
//! Observers follow one contract: a worker sends `Ok(state)` for each new reading
//! and one `Err(())` when the service disappears or a read fails, then ends.
//! [`recovering`] turns such workers into a stream of [`ServiceState`] values
//! that reports unavailability once and retries with bounded backoff.
use crate::{Error, ServiceState};
use futures_util::{FutureExt, Stream, StreamExt, stream};
use std::{future::Future, pin::Pin, time::Duration};
use tokio::sync::mpsc;

/// The deadline for connecting and for ordinary property reads.
pub const DEADLINE: Duration = Duration::from_secs(2);

/// A boxed stream of service states, as returned by the `watch` functions.
pub type StateStream<T> = Pin<Box<dyn Stream<Item = ServiceState<T>> + Send + 'static>>;

/// Connect to the system bus within [`DEADLINE`].
pub async fn system() -> Result<zbus::Connection, Error> {
    Ok(tokio::time::timeout(DEADLINE, zbus::Connection::system()).await??)
}

/// Connect to the session bus within [`DEADLINE`].
pub async fn session() -> Result<zbus::Connection, Error> {
    Ok(tokio::time::timeout(DEADLINE, zbus::Connection::session()).await??)
}

/// The unique owner of a well-known name, or `None` when nobody owns it.
pub async fn owner(
    conn: &zbus::Connection,
    name: &str,
) -> Result<Option<zbus::names::OwnedUniqueName>, Error> {
    let dbus = zbus::fdo::DBusProxy::new(conn).await?;
    let name = zbus::names::BusName::try_from(name).map_err(|_| Error::Invalid("bus name"))?;
    match tokio::time::timeout(DEADLINE, dbus.get_name_owner(name)).await? {
        Ok(owner) => Ok(Some(owner)),
        Err(zbus::fdo::Error::NameHasNoOwner(_)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Recovering observation with bounded backoff. `spawn` starts one worker for
/// the sender it receives; the worker must stop when that sender closes. Each
/// retry gets a fresh channel, so a worker builds fresh proxies.
///
/// The stream yields [`ServiceState::Unavailable`] once per outage, then each
/// reading. It retries after 250 ms, doubling up to 30 s, and resets the
/// delay after a successful reading.
pub fn recovering<T, Spawn>(spawn: Spawn) -> StateStream<T>
where
    T: Send + 'static,
    Spawn: Fn(mpsc::UnboundedSender<Result<T, ()>>) + Send + 'static,
{
    let state = (
        spawn,
        None::<mpsc::UnboundedReceiver<Result<T, ()>>>,
        Duration::ZERO,
        false,
    );
    Box::pin(stream::unfold(
        state,
        |(spawn, mut rx, mut delay, mut unavailable)| async move {
            loop {
                if rx.is_none() {
                    tokio::time::sleep(delay).await;
                    let (tx, receiver) = mpsc::unbounded_channel();
                    spawn(tx);
                    rx = Some(receiver);
                }
                match rx.as_mut().expect("receiver initialized").recv().await {
                    Some(Ok(value)) => {
                        return Some((
                            ServiceState::Available(value),
                            (spawn, rx, Duration::ZERO, false),
                        ));
                    }
                    _ => {
                        rx = None;
                        delay = (delay * 2)
                            .max(Duration::from_millis(250))
                            .min(Duration::from_secs(30));
                        if !unavailable {
                            unavailable = true;
                            return Some((
                                ServiceState::Unavailable,
                                (spawn, rx, delay, unavailable),
                            ));
                        }
                    }
                }
            }
        },
    ))
}

/// A service can disappear without emitting a final property change. Run
/// `work` until it finishes, its subscriber leaves, or `service` changes owner.
pub async fn while_service_owned<T>(
    conn: &zbus::Connection,
    service: &str,
    tx: &mpsc::UnboundedSender<T>,
    work: impl Future,
) {
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

/// Signal-driven re-reading for services without a dedicated observer. The
/// worker resolves `service` to its unique owner, subscribes to every signal
/// from that owner, and calls `read` first and after each burst of signals.
/// Unchanged readings are not sent again.
pub async fn observe_signals<T, Read, Fut>(
    conn: zbus::Connection,
    service: &str,
    read: Read,
    tx: mpsc::UnboundedSender<Result<T, ()>>,
) where
    T: PartialEq + Clone,
    Read: Fn(zbus::Connection) -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    let result = async {
        let owner = owner(&conn, service).await?.ok_or(Error::Unavailable)?;
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(owner)?
            .build();
        // Subscribe before the first read so no change is lost in between.
        let mut signals = zbus::MessageStream::for_match_rule(rule, &conn, Some(64)).await?;
        let mut last = None;
        loop {
            let reading = tokio::time::timeout(DEADLINE, read(conn.clone())).await??;
            if last.as_ref() != Some(&reading) {
                if tx.send(Ok(reading.clone())).is_err() {
                    return Ok(());
                }
                last = Some(reading);
            }
            tokio::select! {
                _ = tx.closed() => return Ok(()),
                signal = signals.next() => {
                    if !matches!(signal, Some(Ok(_))) {
                        return Err(Error::Unavailable);
                    }
                }
            }
            // Coalesce a burst (for example one signal per property) into one read.
            while let Some(Some(Ok(_))) = signals.next().now_or_never() {}
        }
    }
    .await;
    if result.is_err() {
        let _ = tx.send(Err(()));
    }
}

/// [`observe_signals`] as a recovering stream that also ends each worker when
/// the service changes owner.
pub fn watch_service<T, Read, Fut>(
    conn: zbus::Connection,
    service: &'static str,
    read: Read,
) -> StateStream<T>
where
    T: PartialEq + Clone + Send + 'static,
    Read: Fn(zbus::Connection) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<T, Error>> + Send + 'static,
{
    recovering(move |tx| {
        let conn = conn.clone();
        let read = read.clone();
        tokio::spawn(async move {
            let work = observe_signals(conn.clone(), service, read, tx.clone());
            while_service_owned(&conn, service, &tx, work).await;
        });
    })
}

/// Replace control characters and bound text from another process.
pub(crate) fn bounded_text(text: &str, limit: usize) -> String {
    let mut out: String = text
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .take(limit)
        .collect();
    if text.chars().count() > limit {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn recovers_after_initial_failure_and_later_disconnect() {
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let mut stream = recovering(move |tx| {
            let attempt = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if attempt > 0 {
                let _ = tx.send(Ok(attempt));
            }
        });
        assert_eq!(stream.next().await, Some(ServiceState::Unavailable));
        assert_eq!(stream.next().await, Some(ServiceState::Available(1)));
        assert_eq!(stream.next().await, Some(ServiceState::Unavailable));
        assert_eq!(stream.next().await, Some(ServiceState::Available(2)));
    }

    #[test]
    fn bounded_text_replaces_controls_and_marks_truncation() {
        assert_eq!(bounded_text("a\tb\nc", 10), "a b\nc");
        assert_eq!(bounded_text("abcdef", 3), "abc…");
    }
}
