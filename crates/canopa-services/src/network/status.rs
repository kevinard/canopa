//! Connectivity and connection label from the bundle's status owner.
use super::{Connectivity, NetworkStatus};
use crate::{ServiceState, bus::StateStream};
use futures_util::StreamExt;
use tokio::sync::mpsc;

#[cfg(test)]
#[path = "status_tests.rs"]
mod transport_tests;

type Sender = mpsc::UnboundedSender<Result<NetworkStatus, ()>>;

/// Observe network status on the system bus with recovery. Switches between
/// NetworkManager and systemd-networkd as their ownership changes and yields
/// [`ServiceState::Unavailable`] while neither runs.
pub fn watch(conn: zbus::Connection) -> StateStream<NetworkStatus> {
    crate::bus::recovering(move |tx| {
        let conn = conn.clone();
        tokio::spawn(async move {
            let work = observe(conn.clone(), tx.clone());
            tokio::select! { _ = tx.closed() => {}, _ = work => {} }
        });
    })
}

/// One-shot status read from the current status owner.
pub async fn read(conn: &zbus::Connection) -> Result<ServiceState<NetworkStatus>, crate::Error> {
    let Some(owner) = super::bundle::Bundle::resolve(conn).await?.status else {
        return Ok(ServiceState::Unavailable);
    };
    let status = if owner.kind == super::bundle::Kind::NetworkManager {
        let proxy = NetworkManagerProxy::builder(conn)
            .destination(owner.unique)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        read_nm_state(&proxy, conn).await?
    } else {
        let proxy = NetworkdManagerProxy::builder(conn)
            .destination(owner.unique)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        read_networkd_state(&proxy, conn).await?
    };
    Ok(ServiceState::Available(status))
}

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

    #[zbus(property)]
    fn primary_connection_type(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn active_connections(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;
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

    #[zbus(property)]
    fn vpn(&self) -> zbus::Result<bool>;

    #[zbus(property, name = "Type")]
    fn kind(&self) -> zbus::Result<String>;
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

/// Convert NM Connectivity u32 to Connectivity.
fn nm_connectivity(value: u32) -> Connectivity {
    match value {
        4 => Connectivity::Full,
        3 => Connectivity::Limited,
        2 => Connectivity::Portal,
        1 => Connectivity::None,
        _ => Connectivity::Unknown,
    }
}

/// Convert networkd OnlineState string to Connectivity.
fn networkd_connectivity(state: &str) -> Connectivity {
    match state {
        "online" => Connectivity::Connected,
        "partial" => Connectivity::Limited,
        "offline" => Connectivity::None,
        _ => Connectivity::Unknown,
    }
}

/// The observation worker behind [`watch`]: sends a reading per change and one
/// `Err(())` per outage, and ends when `tx` closes or the bus closes.
pub async fn observe(conn: zbus::Connection, tx: Sender) -> zbus::Error {
    use super::bundle::{Kind, Watch};
    let mut watch = match Watch::new(&conn).await {
        Ok(watch) => watch,
        Err(error) => {
            let _ = tx.send(Err(()));
            return error;
        }
    };
    loop {
        let selected = watch.current().status;
        let work = async {
            match selected.as_ref() {
                Some(owner) if owner.kind == Kind::NetworkManager => {
                    run_nm_task(conn.clone(), owner.unique.clone(), tx.clone()).await;
                }
                Some(owner) => {
                    run_networkd_task(conn.clone(), owner.unique.clone(), tx.clone()).await;
                }
                None => {
                    let _ = tx.send(Err(()));
                    std::future::pending::<()>().await;
                }
            }
        };
        tokio::pin!(work);
        loop {
            tokio::select! {
                _ = tx.closed() => return zbus::Error::Failure("network observer closed".into()),
                result = watch.changed() => {
                    if result.is_err() { let _ = tx.send(Err(())); return zbus::Error::Failure("network bus closed".into()); }
                    if watch.current().status != selected { break; }
                }
                _ = &mut work => {
                    // Let the subscription's bounded retry recover transient property failures.
                    return zbus::Error::Failure("network observation failed".into());
                }
            }
        }
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

async fn run_nm_task(conn: zbus::Connection, owner: String, tx: Sender) -> zbus::Error {
    let result = async {
        // Subscribe before the first snapshot, including active-profile renames.
        let mut changes = network_signals(&conn, &owner).await?;
        let proxy = NetworkManagerProxy::builder(&conn)
            .destination(owner)?
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
) -> zbus::Result<NetworkStatus> {
    let connectivity = nm_connectivity(proxy.connectivity().await?);
    let label = read_nm_label(proxy, conn).await;
    // Older daemons and fakes may not have these; they only add detail.
    let kind = proxy
        .primary_connection_type()
        .await
        .ok()
        .and_then(|kind| super::NetworkKind::from_nm(&kind));
    let mut vpn = Vec::new();
    for path in proxy
        .active_connections()
        .await
        .unwrap_or_default()
        .into_iter()
        .take(64)
    {
        let Ok(builder) = NMActiveConnectionProxy::builder(conn)
            .destination(proxy.inner().destination().to_string())
            .and_then(|builder| builder.path(path))
        else {
            continue;
        };
        let Ok(active) = builder
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
        else {
            continue;
        };
        let is_vpn = active.vpn().await.unwrap_or(false)
            || active.kind().await.is_ok_and(|kind| kind == "wireguard");
        if is_vpn && let Ok(id) = active.id().await {
            vpn.push(crate::bus::bounded_text(&id, 64));
        }
    }
    vpn.sort();
    Ok(NetworkStatus {
        connectivity,
        label,
        kind,
        vpn,
    })
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
        .destination(proxy.inner().destination().to_string())
        .ok()?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .path(path)
        .ok()?
        .build()
        .await
        .ok()?;
    active.id().await.ok()
}

async fn run_networkd_task(conn: zbus::Connection, owner: String, tx: Sender) -> zbus::Error {
    let result = async {
        // Link state, additions and removals can change the label without changing
        // the manager's OnlineState. Observe the daemon's whole signal stream.
        let mut changes = network_signals(&conn, &owner).await?;
        let proxy = NetworkdManagerProxy::builder(&conn)
            .destination(owner)?
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
) -> zbus::Result<NetworkStatus> {
    let connectivity = networkd_connectivity(&proxy.online_state().await?);
    let mut labels = Vec::new();
    for (_, name, path) in proxy.list_links().await? {
        if name == "lo" {
            continue;
        }
        let link = NetworkdLinkProxy::builder(conn)
            .destination(proxy.inner().destination().to_string())?
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
    Ok(NetworkStatus {
        connectivity,
        label,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nm_connectivity_4_is_full() {
        assert_eq!(nm_connectivity(4), Connectivity::Full);
    }

    #[test]
    fn nm_connectivity_3_is_limited() {
        assert_eq!(nm_connectivity(3), Connectivity::Limited);
    }

    #[test]
    fn nm_connectivity_2_is_portal() {
        assert_eq!(nm_connectivity(2), Connectivity::Portal);
    }

    #[test]
    fn nm_connectivity_1_is_none() {
        assert_eq!(nm_connectivity(1), Connectivity::None);
    }

    #[test]
    fn nm_connectivity_0_is_unknown() {
        assert_eq!(nm_connectivity(0), Connectivity::Unknown);
    }

    #[test]
    fn networkd_online_state_online_is_connected() {
        assert_eq!(networkd_connectivity("online"), Connectivity::Connected);
    }

    #[test]
    fn networkd_online_state_partial_is_limited() {
        assert_eq!(networkd_connectivity("partial"), Connectivity::Limited);
    }

    #[test]
    fn networkd_online_state_offline_is_none() {
        assert_eq!(networkd_connectivity("offline"), Connectivity::None);
    }

    #[test]
    fn networkd_unknown_online_state_is_unknown() {
        assert_eq!(networkd_connectivity("unknown"), Connectivity::Unknown);
    }
}
