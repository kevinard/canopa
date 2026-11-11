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

/// All properties of `interface` on `service`'s object at `path`, read
/// through the well-known name, which starts an activatable service.
pub async fn properties(
    conn: &zbus::Connection,
    service: &str,
    path: &str,
    interface: &str,
) -> Result<std::collections::HashMap<String, zbus::zvariant::OwnedValue>, Error> {
    let properties = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(service.to_owned())?
        .path(path.to_owned())?
        .build()
        .await?;
    let interface =
        zbus::names::InterfaceName::try_from(interface.to_owned()).map_err(zbus::Error::from)?;
    Ok(tokio::time::timeout(DEADLINE, properties.get_all(interface)).await??)
}

/// Observation of a service that systemd starts on demand and that exits
/// when idle (timedated, hostnamed, localed): an absent owner is not an
/// outage. The worker follows `PropertiesChanged` at `path` from the
/// well-known name, and re-reads when the owner changes and every `every`.
/// It reports unavailability only when `read` fails.
pub fn watch_on_demand<T, Read, Fut>(
    conn: zbus::Connection,
    service: &'static str,
    path: &'static str,
    every: Duration,
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
            let result = async {
                let rule = zbus::MatchRule::builder()
                    .msg_type(zbus::message::Type::Signal)
                    .sender(service)?
                    .path(path)?
                    .interface("org.freedesktop.DBus.Properties")?
                    .member("PropertiesChanged")?
                    .build();
                let mut signals =
                    zbus::MessageStream::for_match_rule(rule, &conn, Some(16)).await?;
                // When the service exits, read again at once: that starts it
                // again, or reports that it is gone.
                let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
                let mut owners = dbus
                    .receive_name_owner_changed_with_args(&[(0, service)])
                    .await?;
                let mut last = None;
                loop {
                    let reading = tokio::time::timeout(DEADLINE * 2, read(conn.clone())).await??;
                    if last.as_ref() != Some(&reading) {
                        if tx.send(Ok(reading.clone())).is_err() {
                            return Ok::<(), Error>(());
                        }
                        last = Some(reading);
                    }
                    tokio::select! {
                        _ = tx.closed() => return Ok(()),
                        _ = tokio::time::sleep(every) => {}
                        signal = signals.next() => {
                            if signal.is_none() {
                                return Err(Error::Unavailable);
                            }
                        }
                        owner = owners.next() => {
                            if owner.is_none() {
                                return Err(Error::Unavailable);
                            }
                        }
                    }
                }
            }
            .await;
            if result.is_err() {
                let _ = tx.send(Err(()));
            }
        });
    })
}

/// How long a privileged call may wait, including a polkit password prompt.
pub const AUTHORIZATION_DEADLINE: Duration = Duration::from_secs(300);

/// Call `method` allowing polkit to ask for a password, within
/// [`AUTHORIZATION_DEADLINE`]. A cancelled or refused prompt is
/// [`Error::NotAuthorized`].
pub async fn call_interactive<B, R>(
    proxy: &zbus::Proxy<'_>,
    method: &str,
    body: &B,
) -> Result<R, Error>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    let reply = tokio::time::timeout(
        AUTHORIZATION_DEADLINE,
        proxy.call_with_flags::<_, _, R>(
            method,
            zbus::proxy::MethodFlags::AllowInteractiveAuth.into(),
            body,
        ),
    )
    .await??;
    reply.ok_or(Error::Refused("the service sent no reply".into()))
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
