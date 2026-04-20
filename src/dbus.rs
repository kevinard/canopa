pub mod subscription;
pub mod networkmanager;
pub mod sni;
pub mod sni_watcher;
pub mod upower;
pub mod wireplumber;

/// Shared D-Bus connections, established once at startup (Story 5.1).
///
/// `zbus::Connection` is `Clone` + Arc-backed — cloning gives a reference to the
/// same underlying connection. Module subscriptions clone these handles; they
/// never create their own connections (AR5).
#[derive(Debug, Clone, Default)]
pub struct DbusConnections {
    /// Session bus handle for UPower and WirePlumber.
    /// `None` if the session bus was unavailable at startup.
    /// Used by Stories 5.2 (battery) and 5.4 (volume).
    pub(crate) session: Option<zbus::Connection>,
    /// System bus handle for NetworkManager.
    /// `None` if the system bus was unavailable at startup.
    /// Used by Story 5.3 (network).
    pub(crate) system: Option<zbus::Connection>,
}

impl DbusConnections {
    /// Attempt to connect to both buses.
    /// Never panics — unavailable buses yield `None` (UX-DR16 silent omission).
    async fn connect() -> Self {
        let session = zbus::Connection::session().await.ok();
        let system = zbus::Connection::system().await.ok();
        Self { session, system }
    }
}

/// Returns a one-shot Iced subscription that connects to both D-Bus buses and emits
/// `Message::DbusReady(connections)` exactly once, then terminates.
///
/// The unique ID `DbusInitId` prevents collision with `niri_subscription()` which
/// also uses `Subscription::run_with` with `()`. (Architecture rule: every subscription
/// uses a distinct ID via a named type.)
pub(crate) fn dbus_init_subscription() -> iced::Subscription<crate::app::Message> {
    use iced::futures::stream;

    #[derive(Hash)]
    struct DbusInitId;

    iced::Subscription::run_with(DbusInitId, |_: &DbusInitId| {
        stream::unfold(false, |done| async move {
            if done {
                return None; // Stream exhausted; Iced will not restart it.
            }
            let connections = DbusConnections::connect().await;
            Some((crate::app::Message::DbusReady(connections), true))
        })
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
