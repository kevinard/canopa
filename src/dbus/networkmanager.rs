use crate::modules::network::{NetworkConnectivity, NetworkEvent};
use tokio::sync::mpsc;

#[cfg(test)]
#[path = "networkmanager_tests.rs"]
mod transport_tests;

// ── NetworkManager proxy ─────────────────────────────────────────────────

/// Main NetworkManager proxy.
#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    /// Connectivity: 0=Unknown, 1=None, 2=Portal, 3=Limited, 4=Full
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
        3 => NetworkConnectivity::Limited,
        2 => NetworkConnectivity::Portal,
        1 => NetworkConnectivity::None,
        _ => NetworkConnectivity::Unknown,
    }
}

/// Convert networkd OnlineState string to NetworkConnectivity.
fn networkd_connectivity(state: &str) -> NetworkConnectivity {
    match state {
        "online" => NetworkConnectivity::Connected,
        "partial" => NetworkConnectivity::Limited,
        "offline" => NetworkConnectivity::None,
        _ => NetworkConnectivity::Unknown,
    }
}

struct NetworkConnData(zbus::Connection);

impl std::hash::Hash for NetworkConnData {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "network-subscription".hash(h);
        self.0.unique_name().hash(h);
        self.0.server_guid().hash(h);
    }
}

fn build_network_stream(
    data: &NetworkConnData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>> {
    let conn = data.0.clone();
    crate::dbus::subscription::build::<
        (NetworkConnectivity, Option<String>),
        crate::app::Message,
        _,
        _,
        _,
    >(
        move |tx| {
            let conn = conn.clone();
            tokio::spawn(async move {
                let work = run_network_task(conn.clone(), tx.clone());
                tokio::select! { _ = tx.closed() => {}, _ = work => {} }
            });
        },
        |(connectivity, label)| {
            crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Network(
                NetworkEvent::StateChanged {
                    connectivity,
                    label,
                },
            ))
        },
        || {
            crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Network(
                NetworkEvent::Unavailable,
            ))
        },
    )
}

/// Returns an Iced subscription for network state.
/// Tries NetworkManager first; falls back to systemd-networkd; emits Unavailable if both absent.
pub(crate) fn network_subscription(
    conn: zbus::Connection,
) -> iced::Subscription<crate::app::Message> {
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
        {
            crate::dbus::subscription::while_service_owned(
                &conn,
                "org.freedesktop.NetworkManager",
                &tx,
                run_nm_task(conn.clone(), tx.clone()),
            )
            .await;
            zbus::Error::Failure("NetworkManager disconnected".into())
        }
    } else if has_nd {
        {
            crate::dbus::subscription::while_service_owned(
                &conn,
                "org.freedesktop.network1",
                &tx,
                run_networkd_task(conn.clone(), tx.clone()),
            )
            .await;
            zbus::Error::Failure("networkd disconnected".into())
        }
    } else {
        let _ = tx.send(Err(()));
        zbus::Error::Failure("no network backend available".into())
    }
}

async fn network_signals(
    conn: &zbus::Connection,
    service: &str,
) -> zbus::Result<zbus::MessageStream> {
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(service)?
        .build();
    zbus::MessageStream::for_match_rule(rule, conn, Some(64)).await
}

async fn run_nm_task(
    conn: zbus::Connection,
    tx: mpsc::UnboundedSender<Result<(NetworkConnectivity, Option<String>), ()>>,
) -> zbus::Error {
    use iced::futures::StreamExt;
    let result = async {
        // Subscribe before the first snapshot, including active-profile renames.
        let mut changes = network_signals(&conn, "org.freedesktop.NetworkManager").await?;
        let proxy = NetworkManagerProxy::builder(&conn)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        loop {
            let data = read_nm_state(&proxy, &conn).await?;
            if tx.send(Ok(data)).is_err() || changes.next().await.transpose()?.is_none() {
                break;
            }
        }
        Ok::<_, zbus::Error>(())
    }
    .await;
    let _ = tx.send(Err(()));
    result
        .err()
        .unwrap_or_else(|| zbus::Error::Failure("NM stream ended".into()))
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
    use iced::futures::StreamExt;
    let result = async {
        // Link state, additions and removals can change the label without changing
        // the manager's OnlineState. Observe the daemon's whole signal stream.
        let mut changes = network_signals(&conn, "org.freedesktop.network1").await?;
        let proxy = NetworkdManagerProxy::builder(&conn)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        loop {
            let data = read_networkd_state(&proxy, &conn).await?;
            if tx.send(Ok(data)).is_err() || changes.next().await.transpose()?.is_none() {
                break;
            }
        }
        Ok::<_, zbus::Error>(())
    }
    .await;
    let _ = tx.send(Err(()));
    result
        .err()
        .unwrap_or_else(|| zbus::Error::Failure("networkd stream ended".into()))
}

#[zbus::proxy(
    interface = "org.freedesktop.network1.Link",
    default_service = "org.freedesktop.network1",
    default_path = "/"
)]
trait NetworkdLink {
    #[zbus(property)]
    fn operational_state(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn online_state(&self) -> zbus::Result<String>;
}

async fn read_networkd_state(
    proxy: &NetworkdManagerProxy<'_>,
    conn: &zbus::Connection,
) -> zbus::Result<(NetworkConnectivity, Option<String>)> {
    let connectivity = networkd_connectivity(&proxy.online_state().await?);
    let mut labels = Vec::new();
    for (_, name, path) in proxy.list_links().await? {
        if name == "lo" {
            continue;
        }
        let link = NetworkdLinkProxy::builder(conn)
            .path(path)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        let online = link.online_state().await.unwrap_or_default();
        let state = link.operational_state().await.unwrap_or_default();
        if online == "online" && matches!(state.as_str(), "routable" | "degraded") {
            labels.push(name);
        }
    }
    // Networkd doesn't identify one primary connection. Report all usable links,
    // deterministically, instead of guessing from ListLinks iteration order.
    labels.sort();
    labels.dedup();
    let label = (!labels.is_empty()).then(|| labels.join(", "));
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
    fn nm_connectivity_2_is_portal() {
        assert_eq!(nm_connectivity(2), NetworkConnectivity::Portal);
    }

    #[test]
    fn nm_connectivity_1_is_none() {
        assert_eq!(nm_connectivity(1), NetworkConnectivity::None);
    }

    #[test]
    fn nm_connectivity_0_is_unknown() {
        assert_eq!(nm_connectivity(0), NetworkConnectivity::Unknown);
    }

    #[test]
    fn networkd_online_state_online_is_connected() {
        assert_eq!(
            networkd_connectivity("online"),
            NetworkConnectivity::Connected
        );
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
    fn networkd_unknown_online_state_is_unknown() {
        assert_eq!(
            networkd_connectivity("unknown"),
            NetworkConnectivity::Unknown
        );
    }
}
