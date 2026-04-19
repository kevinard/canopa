use crate::modules::network::{NetworkConnectivity, NetworkEvent};
use tokio::sync::mpsc;

// ── NetworkManager proxy ─────────────────────────────────────────────────

/// Main NetworkManager proxy.
#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    /// Connectivity: 0=Unknown, 1=None, 2=Limited, 3=Portal, 4=Full
    #[zbus(property)]
    fn connectivity(&self) -> zbus::Result<u32>;

    /// Path to primary active connection, or "/" when no connection.
    #[zbus(property)]
    fn primary_connection(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

/// Active connection proxy for reading the profile name.
#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/"   // placeholder; always use builder to set real path
)]
trait NMActiveConnection {
    /// Human-readable connection profile name (e.g. "Home WiFi").
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
}

// ── systemd-networkd proxy ───────────────────────────────────────────────

/// systemd-networkd Manager proxy.
#[zbus::proxy(
    interface = "org.freedesktop.network1.Manager",
    default_service = "org.freedesktop.network1",
    default_path = "/org/freedesktop/network1"
)]
trait NetworkdManager {
    /// "online" | "partial" | "offline"
    #[zbus(property)]
    fn online_state(&self) -> zbus::Result<String>;

    /// Returns (index: i32, name: String, path: OwnedObjectPath) per link.
    fn list_links(&self) -> zbus::Result<Vec<(i32, String, zbus::zvariant::OwnedObjectPath)>>;
}

/// Convert NM Connectivity u32 to NetworkConnectivity.
fn nm_connectivity(value: u32) -> NetworkConnectivity {
    match value {
        4 => NetworkConnectivity::Full,
        2 | 3 => NetworkConnectivity::Limited,
        _ => NetworkConnectivity::None, // 0=Unknown, 1=None
    }
}

/// Convert networkd OnlineState string to NetworkConnectivity.
fn networkd_connectivity(state: &str) -> NetworkConnectivity {
    match state {
        "online" => NetworkConnectivity::Full,
        "partial" => NetworkConnectivity::Limited,
        _ => NetworkConnectivity::None, // "offline" or unknown
    }
}

struct NetworkConnData(zbus::Connection);

impl std::hash::Hash for NetworkConnData {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "network-subscription".hash(h);
    }
}

fn build_network_stream(
    data: &NetworkConnData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>> {
    let conn = data.0.clone();
    crate::dbus::subscription::build::<(NetworkConnectivity, Option<String>), crate::app::Message, _, _, _>(
        move |tx| {
            tokio::spawn(async move {
                let _ = run_network_task(conn, tx).await;
            });
        },
        |(connectivity, label)| {
            crate::app::Message::Network(NetworkEvent::StateChanged { connectivity, label })
        },
        || crate::app::Message::Network(NetworkEvent::Unavailable),
    )
}

/// Returns an Iced subscription for network state.
/// Tries NetworkManager first; falls back to systemd-networkd; emits Unavailable if both absent.
pub fn network_subscription(conn: zbus::Connection) -> iced::Subscription<crate::app::Message> {
    iced::Subscription::run_with(NetworkConnData(conn), build_network_stream)
}

async fn run_network_task(
    conn: zbus::Connection,
    tx: mpsc::UnboundedSender<Result<(NetworkConnectivity, Option<String>), ()>>,
) -> zbus::Error {
    // Auto-detect: try NetworkManager first, fall back to systemd-networkd.
    let dbus = match zbus::fdo::DBusProxy::new(&conn).await {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(Err(()));
            return e;
        }
    };

    let has_nm = dbus
        .name_has_owner("org.freedesktop.NetworkManager".try_into().unwrap())
        .await
        .unwrap_or(false);
    let has_nd = dbus
        .name_has_owner("org.freedesktop.network1".try_into().unwrap())
        .await
        .unwrap_or(false);

    if has_nm {
        run_nm_task(conn, tx).await
    } else if has_nd {
        run_networkd_task(conn, tx).await
    } else {
        let _ = tx.send(Err(()));
        zbus::Error::Failure("no network backend available".into())
    }
}

async fn run_nm_task(
    conn: zbus::Connection,
    tx: mpsc::UnboundedSender<Result<(NetworkConnectivity, Option<String>), ()>>,
) -> zbus::Error {
    let proxy = match NetworkManagerProxy::new(&conn).await {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(Err(()));
            return e;
        }
    };

    // Send initial state.
    match read_nm_state(&proxy, &conn).await {
        Ok(data) => {
            if tx.send(Ok(data)).is_err() {
                return zbus::Error::Failure("channel closed".into());
            }
        }
        Err(e) => {
            let _ = tx.send(Err(()));
            return e;
        }
    }

    // Watch Connectivity and PrimaryConnection property changes.
    let mut conn_stream = proxy.receive_connectivity_changed().await;
    let mut pc_stream = proxy.receive_primary_connection_changed().await;

    use iced::futures::StreamExt;
    loop {
        tokio::select! {
            _ = conn_stream.next() => {}
            _ = pc_stream.next()   => {}
        }
        match read_nm_state(&proxy, &conn).await {
            Ok(data) => {
                if tx.send(Ok(data)).is_err() {
                    break;
                }
            }
            Err(_) => {
                let _ = tx.send(Err(()));
                break;
            }
        }
    }
    zbus::Error::Failure("NM stream ended".into())
}

async fn read_nm_state(
    proxy: &NetworkManagerProxy<'_>,
    conn: &zbus::Connection,
) -> zbus::Result<(NetworkConnectivity, Option<String>)> {
    let connectivity = nm_connectivity(proxy.connectivity().await?);
    let label = read_nm_label(proxy, conn).await;
    Ok((connectivity, label))
}

/// Get NM profile name from PrimaryConnection → Id.
/// Returns None gracefully if no primary connection or query fails.
async fn read_nm_label(proxy: &NetworkManagerProxy<'_>, conn: &zbus::Connection) -> Option<String> {
    let path = proxy.primary_connection().await.ok()?;
    // "/" means no active primary connection.
    if path.as_str() == "/" {
        return None;
    }
    let active = NMActiveConnectionProxy::builder(conn)
        .path(path)
        .ok()?
        .build()
        .await
        .ok()?;
    active.id().await.ok()
}

async fn run_networkd_task(
    conn: zbus::Connection,
    tx: mpsc::UnboundedSender<Result<(NetworkConnectivity, Option<String>), ()>>,
) -> zbus::Error {
    let proxy = match NetworkdManagerProxy::new(&conn).await {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(Err(()));
            return e;
        }
    };

    // Send initial state.
    match read_networkd_state(&proxy).await {
        Ok(data) => {
            if tx.send(Ok(data)).is_err() {
                return zbus::Error::Failure("channel closed".into());
            }
        }
        Err(e) => {
            let _ = tx.send(Err(()));
            return e;
        }
    }

    // Watch OnlineState property changes on the Manager.
    let mut online_stream = proxy.receive_online_state_changed().await;

    use iced::futures::StreamExt;
    loop {
        if online_stream.next().await.is_none() {
            break;
        }
        match read_networkd_state(&proxy).await {
            Ok(data) => {
                if tx.send(Ok(data)).is_err() {
                    break;
                }
            }
            Err(_) => {
                let _ = tx.send(Err(()));
                break;
            }
        }
    }
    zbus::Error::Failure("networkd stream ended".into())
}

async fn read_networkd_state(
    proxy: &NetworkdManagerProxy<'_>,
) -> zbus::Result<(NetworkConnectivity, Option<String>)> {
    let online_state = proxy.online_state().await?;
    let connectivity = networkd_connectivity(&online_state);
    // Get label: first non-loopback link name from ListLinks.
    let label = proxy.list_links().await.ok().and_then(|links| {
        links
            .into_iter()
            .filter(|(_, name, _)| name != "lo")
            .map(|(_, name, _)| name)
            .next()
    });
    Ok((connectivity, label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nm_connectivity_4_is_full() {
        assert_eq!(nm_connectivity(4), NetworkConnectivity::Full);
    }

    #[test]
    fn nm_connectivity_3_is_limited() {
        assert_eq!(nm_connectivity(3), NetworkConnectivity::Limited);
    }

    #[test]
    fn nm_connectivity_2_is_limited() {
        assert_eq!(nm_connectivity(2), NetworkConnectivity::Limited);
    }

    #[test]
    fn nm_connectivity_1_is_none() {
        assert_eq!(nm_connectivity(1), NetworkConnectivity::None);
    }

    #[test]
    fn nm_connectivity_0_is_none() {
        assert_eq!(nm_connectivity(0), NetworkConnectivity::None);
    }

    #[test]
    fn networkd_online_state_online_is_full() {
        assert_eq!(networkd_connectivity("online"), NetworkConnectivity::Full);
    }

    #[test]
    fn networkd_online_state_partial_is_limited() {
        assert_eq!(
            networkd_connectivity("partial"),
            NetworkConnectivity::Limited
        );
    }

    #[test]
    fn networkd_online_state_offline_is_none() {
        assert_eq!(networkd_connectivity("offline"), NetworkConnectivity::None);
    }

    #[test]
    fn networkd_unknown_online_state_is_none() {
        assert_eq!(networkd_connectivity("unknown"), NetworkConnectivity::None);
    }
}
