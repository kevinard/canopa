pub mod networkmanager;
pub mod sni;
pub mod sni_watcher;
pub mod subscription;
pub mod upower;
pub mod wireplumber;

/// Shared connections. A persistent subscription reconnects missing buses and
/// publishes a replacement handle when a bus restarts.
#[derive(Debug, Clone, Default)]
pub struct DbusConnections {
    pub(crate) session: Option<zbus::Connection>,
    pub(crate) system: Option<zbus::Connection>,
}

async fn connect_bus(session: bool) -> Option<zbus::Connection> {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        if session {
            zbus::Connection::session().await
        } else {
            zbus::Connection::system().await
        }
    })
    .await
    .ok()
    .and_then(Result::ok)
}

async fn disconnected(stream: &mut Option<zbus::MessageStream>) {
    use iced::futures::StreamExt;
    let Some(stream) = stream else {
        return std::future::pending().await;
    };
    while let Some(Ok(_)) = stream.next().await {}
}

pub(crate) fn dbus_init_subscription() -> iced::Subscription<crate::app::Message> {
    #[derive(Hash)]
    struct DbusInitId;
    iced::Subscription::run_with(DbusInitId, |_: &DbusInitId| {
        iced::futures::stream::unfold(
            (DbusConnections::default(), None, None, true),
            |(mut connections, mut session_stream, mut system_stream, mut first)| async move {
                loop {
                    let (session, system) = tokio::join!(
                        async {
                            if connections.session.is_none() {
                                connect_bus(true).await
                            } else {
                                None
                            }
                        },
                        async {
                            if connections.system.is_none() {
                                connect_bus(false).await
                            } else {
                                None
                            }
                        }
                    );
                    let changed = session.is_some() || system.is_some();
                    if let Some(conn) = session {
                        session_stream = Some(zbus::MessageStream::from(&conn));
                        connections.session = Some(conn);
                    }
                    if let Some(conn) = system {
                        system_stream = Some(zbus::MessageStream::from(&conn));
                        connections.system = Some(conn);
                    }
                    if first || changed {
                        return Some((
                            crate::app::Message::DbusReady(connections.clone()),
                            (connections, session_stream, system_stream, false),
                        ));
                    }
                    first = false;
                    // Healthy connections wait on transport closure, without periodic RPCs.
                    tokio::select! {
                        _ = disconnected(&mut session_stream) => { connections.session = None; session_stream = None; },
                        _ = disconnected(&mut system_stream) => { connections.system = None; system_stream = None; },
                        _ = tokio::time::sleep(std::time::Duration::from_secs(2)), if connections.session.is_none() || connections.system.is_none() => continue,
                    }
                    return Some((
                        crate::app::Message::DbusReady(connections.clone()),
                        (connections, session_stream, system_stream, false),
                    ));
                }
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbus_connections_default_session_is_none() {
        assert!(DbusConnections::default().session.is_none());
    }

    #[test]
    fn dbus_connections_default_system_is_none() {
        assert!(DbusConnections::default().system.is_none());
    }
}
