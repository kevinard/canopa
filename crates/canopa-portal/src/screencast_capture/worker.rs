use super::*;
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::io::{BufRead, Read, Write};
use zbus::zvariant::{DynamicType, OwnedObjectPath, OwnedValue, Value};

pub(super) async fn call<B, R>(
    connection: &Connection,
    owner: &str,
    path: &str,
    interface: &str,
    method: &str,
    body: &B,
    timeout: Duration,
) -> anyhow::Result<R>
where
    B: serde::Serialize + DynamicType,
    R: serde::de::DeserializeOwned + zbus::zvariant::Type,
{
    let reply = tokio::time::timeout(
        timeout,
        connection.call_method(Some(owner), path, Some(interface), method, body),
    )
    .await??;
    Ok(reply.body().deserialize()?)
}

struct Watch(tokio::task::JoinHandle<()>);
impl Drop for Watch {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn observe(
    connection: &Connection,
    input: &Input,
    stop: watch::Sender<bool>,
) -> anyhow::Result<Watch> {
    let bus = zbus::fdo::DBusProxy::new(connection).await?;
    let mut backend = bus
        .receive_name_owner_changed_with_args(&[(0, input.backend.as_str())])
        .await?;
    let mut caller = bus
        .receive_name_owner_changed_with_args(&[(0, input.caller.as_str())])
        .await?;
    let mut compositor = bus
        .receive_name_owner_changed_with_args(&[(0, SERVICE)])
        .await?;
    // Subscribe first. Never activate or switch to a replacement compositor.
    anyhow::ensure!(
        bus.get_name_owner(SERVICE.try_into()?).await?.as_str() == input.compositor
            && bus
                .name_has_owner(input.backend.as_str().try_into()?)
                .await?
            && bus
                .name_has_owner(input.caller.as_str().try_into()?)
                .await?,
        "Capture owner unavailable"
    );
    Ok(Watch(tokio::spawn(async move {
        tokio::select! {
            _ = backend.next() => {},
            _ = caller.next() => {},
            _ = compositor.next() => {},
        }
        stop.send_replace(true);
    })))
}

async fn cancelled(receiver: &mut watch::Receiver<bool>) {
    if !*receiver.borrow() {
        let _ = receiver.changed().await;
    }
}

/// A successful ready callback transfers live streams, not their cleanup owner.
/// This future remains in the separate worker until Stop has been attempted.
pub(super) async fn run(
    connection: &Connection,
    input: Input,
    stop: watch::Sender<bool>,
    ready: impl FnOnce(Vec<Stream>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    input.validate()?;
    let mut stopped = stop.subscribe();
    let _watch =
        tokio::time::timeout(CALL_TIME, observe(connection, &input, stop.clone())).await??;
    anyhow::ensure!(!*stopped.borrow(), "Capture cancelled");
    // A worker killed before Stop cannot clean up its own recording. Recheck
    // the pinned compositor's creator-lifetime contract before any mutation.
    require_owner_cleanup(connection, &input.compositor).await?;
    let mapped = monitor::supported(connection, &input.compositor).await?;
    if input
        .sources
        .iter()
        .any(|source| matches!(source, Source::Monitor(monitor) if monitor.identity().is_some()))
    {
        anyhow::ensure!(mapped, "Capture monitor identities unavailable");
    }
    // Keep awaiting the creation reply after caller/pipe loss so a returned
    // session can still be stopped. A timed-out mutation can remain ambiguous.
    anyhow::ensure!(!*stopped.borrow(), "Capture cancelled");
    let session: OwnedObjectPath = call(
        connection,
        &input.compositor,
        PATH,
        SERVICE,
        "CreateSession",
        &(std::collections::HashMap::<&str, Value<'_>>::new(),),
        Duration::from_secs(30),
    )
    .await?;
    let result = async {
        anyhow::ensure!(!*stopped.borrow(), "Capture cancelled");
        let proxy = zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
            .destination(input.compositor.as_str())?
            .path(session.as_str())?
            .interface(SESSION)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        let mut closed = tokio::time::timeout(CALL_TIME, proxy.receive_signal("Closed")).await??;
        let mut startup_stop = stopped.clone();
        let end = std::time::Instant::now() + Duration::from_secs(20);
        let startup = async {
            let mut waiting = FuturesUnordered::new();
            for (index, source) in input.sources.iter().enumerate() {
                anyhow::ensure!(!*startup_stop.borrow(), "Capture cancelled");
                let mut properties = std::collections::HashMap::from([(
                    "cursor-mode",
                    Value::from(input.cursor.compositor_value()),
                )]);
                if let Source::Monitor(monitor) = source
                    && let Some(identity) = monitor.identity()
                {
                    properties.insert("canopa-source-id", Value::from(identity));
                }
                let stream: OwnedObjectPath = match source {
                    Source::Monitor(monitor) => {
                        call(
                            connection,
                            &input.compositor,
                            &session,
                            SESSION,
                            "RecordMonitor",
                            &(monitor.name(), properties),
                            remaining(end)?,
                        )
                        .await?
                    }
                    Source::Window(id) => {
                        properties.insert("window-id", Value::from(*id));
                        properties.insert("canopa-source-id", Value::from(*id));
                        call(
                            connection,
                            &input.compositor,
                            &session,
                            SESSION,
                            "RecordWindow",
                            &(properties,),
                            remaining(end)?,
                        )
                        .await?
                    }
                };
                let proxy = zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
                    .destination(input.compositor.clone())?
                    .path(stream)?
                    .interface(STREAM)?
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await?;
                let mut added = tokio::time::timeout(remaining(end)?, proxy.receive_signal("PipeWireStreamAdded")).await??;
                let source = source.clone();
                let path = proxy.path().to_owned();
                let compositor = input.compositor.as_str();
                let session = session.clone();
                waiting.push(async move {
                    let message = added
                        .next()
                        .await
                        .ok_or_else(|| anyhow::anyhow!("Capture stream disappeared"))?;
                    let (node,): (u32,) = message.body().deserialize()?;
                    anyhow::ensure!(node != 0 && node != u32::MAX, "Invalid capture node");
                    let (geometry, mapping) = stream_metadata(connection, compositor, session.as_str(), path.as_str(), &source, mapped, end).await?;
                    Ok::<_, anyhow::Error>((
                        index,
                        Stream {
                            node,
                            source,
                            geometry,
                            mapping,
                        },
                    ))
                });
            }
            anyhow::ensure!(!*startup_stop.borrow(), "Capture cancelled");
            let _: () = call(
                connection,
                &input.compositor,
                &session,
                SESSION,
                "Start",
                &(),
                remaining(end)?,
            )
            .await?;
            let mut streams = vec![None; input.sources.len()];
            let mut nodes = HashSet::new();
            loop {
                let next = tokio::select! {
                    biased;
                    _ = cancelled(&mut startup_stop) => anyhow::bail!("Capture cancelled"),
                    next = tokio::time::timeout(end.saturating_duration_since(std::time::Instant::now()), waiting.next()) => next?,
                };
                let Some(stream) = next else { break };
                let (index, stream) = stream?;
                anyhow::ensure!(nodes.insert(stream.node), "Duplicate capture node");
                streams[index] = Some(stream);
            }
            Ok::<_, anyhow::Error>(streams.into_iter().map(Option::unwrap).collect())
        };
        tokio::pin!(startup);
        let streams = tokio::select! {
            result = &mut startup => result?,
            _ = closed.next() => {
                stop.send_replace(true);
                // Finish an in-flight mutation before Stop so cancellation
                // cannot create a stream after its session was retired.
                startup.await?
            },
        };
        anyhow::ensure!(!*stopped.borrow(), "Capture cancelled");
        ready(streams)?;
        tokio::select! {
            _ = cancelled(&mut stopped) => {},
            _ = closed.next() => {},
        }
        Ok(())
    }
    .await;
    // Always stop the original unique owner, including partial/failed startup.
    let cleanup: anyhow::Result<()> = call(
        connection,
        &input.compositor,
        &session,
        SESSION,
        "Stop",
        &(),
        CALL_TIME,
    )
    .await;
    if let Err(error) = cleanup {
        // A compositor can retire the object itself before our Stop arrives.
        let gone = error.downcast_ref::<zbus::Error>().is_some_and(|e| matches!(e, zbus::Error::MethodError(name, _, _) if name.as_str() == "org.freedesktop.DBus.Error.UnknownObject" || name.as_str() == "org.freedesktop.DBus.Error.NameHasNoOwner" || name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown"));
        anyhow::ensure!(gone, "Capture cleanup unconfirmed");
    }
    result
}

async fn stream_metadata(
    connection: &Connection,
    owner: &str,
    session: &str,
    path: &str,
    source: &Source,
    mapped: bool,
    end: std::time::Instant,
) -> anyhow::Result<(Option<Geometry>, Option<Mapping>)> {
    let monitor = matches!(source, Source::Monitor(_));
    if !mapped && !monitor {
        return Ok((None, None));
    }
    let value: OwnedValue = call(
        connection,
        owner,
        path,
        "org.freedesktop.DBus.Properties",
        "Get",
        &(STREAM, "Parameters"),
        remaining(end)?,
    )
    .await?;
    let mut parameters = std::collections::HashMap::<String, OwnedValue>::try_from(value)?;
    let position = parameters
        .remove("position")
        .ok_or_else(|| anyhow::anyhow!("Missing capture position"))?
        .try_into()?;
    let size = parameters
        .remove("size")
        .ok_or_else(|| anyhow::anyhow!("Missing capture size"))?
        .try_into()?;
    let geometry = Geometry { position, size };
    anyhow::ensure!(geometry.valid(), "Invalid capture geometry");
    let mapping = if mapped {
        if let Some(expected) = bindings::source_identity(source) {
            let actual = u64::try_from(
                parameters
                    .remove("canopa-source-id")
                    .ok_or_else(|| anyhow::anyhow!("Missing capture source identity"))?,
            )?;
            anyhow::ensure!(actual == expected, "Capture source identity changed");
            let mapping_id = String::try_from(
                parameters
                    .remove("mapping-id")
                    .ok_or_else(|| anyhow::anyhow!("Missing capture mapping identity"))?,
            )?;
            let mapping = Mapping {
                session: session.into(),
                stream: path.into(),
                source_id: actual,
                mapping_id,
                size,
                physical_scale: parameters
                    .remove("canopa-physical-scale")
                    .map(f64::try_from)
                    .transpose()?,
            };
            anyhow::ensure!(mapping.valid(), "Invalid capture mapping");
            Some(mapping)
        } else {
            None
        }
    } else {
        None
    };
    Ok((monitor.then_some(geometry), mapping))
}

fn remaining(end: std::time::Instant) -> anyhow::Result<Duration> {
    let remaining = end.saturating_duration_since(std::time::Instant::now());
    anyhow::ensure!(!remaining.is_zero(), "Capture start timed out");
    Ok(CALL_TIME.min(remaining))
}

pub(super) fn stdio() -> anyhow::Result<()> {
    let mut line = Vec::new();
    std::io::stdin()
        .lock()
        .take(FRAME_BYTES as u64 + 1)
        .read_until(b'\n', &mut line)?;
    anyhow::ensure!(
        line.len() <= FRAME_BYTES && line.last() == Some(&b'\n'),
        "Invalid capture worker input"
    );
    let input: Input = serde_json::from_slice(&line)?;
    input.validate()?;
    let (stop, _) = watch::channel(false);
    let control = stop.clone();
    std::thread::spawn(move || {
        let mut byte = [0];
        // EOF, unexpected control bytes and read errors all end the lease.
        let _ = std::io::stdin().read(&mut byte);
        control.send_replace(true);
    });
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            use tokio::signal::unix::{SignalKind, signal};
            let mut term = signal(SignalKind::terminate())?;
            let mut interrupt = signal(SignalKind::interrupt())?;
            let control = stop.clone();
            let _signals = Watch(tokio::spawn(async move {
                tokio::select! { _ = term.recv() => {}, _ = interrupt.recv() => {} }
                control.send_replace(true);
            }));
            let connection = tokio::time::timeout(CALL_TIME, Connection::session()).await??;
            run(&connection, input, stop, |streams| {
                let mut bytes = serde_json::to_vec(&streams)?;
                anyhow::ensure!(bytes.len() < RESPONSE_BYTES, "Capture response too large");
                bytes.push(b'\n');
                std::io::stdout().write_all(&bytes)?;
                std::io::stdout().flush()?;
                Ok(())
            })
            .await
        })
}
