use zbus::zvariant::OwnedObjectPath;

pub use super::menu::{fetch_menu, send_menu_event};
use super::pixmap::{
    MAX_RETAINED_PIXMAP_BYTES, MAX_RETAINED_PIXMAP_COUNT, retain_pixmaps_with_budget,
};
pub(crate) use super::pixmap::{argb_to_rgba, pick_best_pixmap};
use super::proxy::*;

/// A live SNI tray item, fully resolved from D-Bus properties.
#[derive(Debug, Clone)]
pub struct TrayItem {
    /// D-Bus service name, e.g. `"org.kde.konsole"` or `":1.42"`.
    pub service: String,
    /// Object path the item is registered at, e.g. `"/StatusNotifierItem"`.
    pub object_path: OwnedObjectPath,
    /// Resolved icon — Named preferred, Pixmap as fallback.
    pub icon: TrayIcon,
    /// Human-readable tooltip text (the `ToolTip.title` field), if any.
    /// Populated from D-Bus but not yet rendered; will be shown on hover in a future story.
    #[allow(dead_code)]
    pub tooltip: Option<String>,
    pub status: ItemStatus,
    /// D-Bus path for the context menu (`org.kde.DBusMenu`), if the item exposes one.
    pub menu_path: Option<OwnedObjectPath>,
}

/// Resolved icon data for a tray item.
#[derive(Debug, Clone)]
pub enum TrayIcon {
    /// XDG icon theme name. Render via icon theme lookup at display time.
    Named(String),
    /// Raw ARGB32 pixmap data: list of (width, height, bytes) at different sizes.
    /// Bytes are ARGB big-endian; convert to RGBA before passing to iced.
    Pixmap(Vec<SniPixmap>),
}

/// SNI item visibility status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStatus {
    /// Normal visible state.
    Active,
    /// Item requests to be hidden. Render nothing.
    Passive,
    /// Item requests attention (e.g. new notification).
    NeedsAttention,
}

impl ItemStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "Passive" => ItemStatus::Passive,
            "NeedsAttention" => ItemStatus::NeedsAttention,
            _ => ItemStatus::Active,
        }
    }
}

/// Fetch the current state of one SNI item, building a `TrayItem`.
///
/// Returns `None` on any proxy error (item may have disappeared).
pub async fn fetch_item(
    conn: &zbus::Connection,
    service: &str,
    object_path: &zbus::zvariant::ObjectPath<'_>,
) -> Option<TrayItem> {
    let proxy = SniItemProxy::builder(conn)
        .destination(service)
        .ok()?
        .path(object_path)
        .ok()?
        .build()
        .await
        .ok()?;

    let icon = {
        let name = proxy.icon_name().await.unwrap_or_default();
        if !name.is_empty() {
            TrayIcon::Named(name)
        } else {
            let pixmaps = retain_pixmaps_with_budget(
                proxy.icon_pixmap().await.unwrap_or_default(),
                MAX_RETAINED_PIXMAP_COUNT,
                MAX_RETAINED_PIXMAP_BYTES,
            );
            TrayIcon::Pixmap(pixmaps)
        }
    };

    let tooltip = proxy
        .tool_tip()
        .await
        .ok()
        .map(|(_, _, title, _)| title)
        .filter(|t| !t.is_empty());

    let status = proxy
        .status()
        .await
        .map(|s| ItemStatus::from_str(&s))
        .unwrap_or(ItemStatus::Active);

    let menu_path = proxy.menu().await.ok();

    Some(TrayItem {
        service: service.to_string(),
        object_path: zbus::zvariant::OwnedObjectPath::try_from(object_path.as_str()).ok()?,
        icon,
        tooltip,
        status,
        menu_path,
    })
}

// -- D-Bus call helpers -------------------------------------------------------

/// Call `Activate(0, 0)` on an SNI item. Errors are silently ignored.
pub async fn call_activate(
    conn: &zbus::Connection,
    service: &str,
    object_path: &zbus::zvariant::OwnedObjectPath,
) -> zbus::Result<()> {
    let proxy = SniItemProxy::builder(conn)
        .destination(service)?
        .path(object_path.as_ref())?
        .build()
        .await?;
    proxy.activate(0, 0).await
}

/// Call `SecondaryActivate(0, 0)` on an SNI item.
pub async fn call_secondary_activate(
    conn: &zbus::Connection,
    service: &str,
    object_path: &zbus::zvariant::OwnedObjectPath,
) -> zbus::Result<()> {
    let proxy = SniItemProxy::builder(conn)
        .destination(service)?
        .path(object_path.as_ref())?
        .build()
        .await?;
    proxy.secondary_activate(0, 0).await
}

/// Call `Scroll(delta, orientation)` on an SNI item.
pub async fn call_scroll(
    conn: &zbus::Connection,
    service: &str,
    object_path: &zbus::zvariant::OwnedObjectPath,
    delta: i32,
    orientation: &str,
) -> zbus::Result<()> {
    let proxy = SniItemProxy::builder(conn)
        .destination(service)?
        .path(object_path.as_ref())?
        .build()
        .await?;
    proxy.scroll(delta, orientation).await
}

/// Incremental tray change event emitted by the SNI subscription.
#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// Full snapshot: replaces all current tray icons (sent once on startup/reset).
    Snapshot(Vec<TrayItem>),
    /// A new item became visible (Active or NeedsAttention).
    ItemAdded(TrayItem),
    /// An item was removed or became Passive; keyed by the D-Bus service string.
    ItemRemoved(String),
}

/// Parse a registered service string into (service, object_path).
///
/// The SNI watcher stores items as either:
/// - A well-known name: `"org.kde.konsole"` → service=`"org.kde.konsole"`, path=`"/StatusNotifierItem"`
/// - A unique name + path: `":1.42/StatusNotifierItem"` → service=`":1.42"`, path=`"/StatusNotifierItem"`
fn parse_service_string(s: &str) -> (&str, &str) {
    // Find the first '/' that is not at position 0 (unique names start with ':').
    // Well-known names contain only dots and alphanumerics, no slashes.
    if let Some(slash_pos) = s[1..].find('/') {
        let slash_pos = slash_pos + 1; // adjust for the skipped leading char
        (&s[..slash_pos], &s[slash_pos..])
    } else {
        (s, "/StatusNotifierItem")
    }
}

/// Drives the SNI watcher and host.
///
/// 1. Starts the watcher (or falls back to connecting to an existing one).
/// 2. Registers niribar as a host.
/// 3. Fetches the initial item list.
/// 4. Listens for item registrations/unregistrations and name disappearances.
/// 5. Sends updated `Vec<TrayItem>` on `tx` on every change.
pub(super) async fn run_sni_task(
    conn: zbus::Connection,
    tx: tokio::sync::mpsc::UnboundedSender<TrayEvent>,
) -> zbus::Error {
    use iced::futures::StreamExt;
    use std::collections::HashMap;

    // 1. Start or connect to watcher.
    let watcher_items_opt = crate::dbus::sni_watcher::start_watcher(&conn).await;
    let _watcher_items = match watcher_items_opt {
        Ok(opt) => opt,
        Err(e) => return e,
    };

    // 2. Connect to the watcher and register as host.
    let watcher_proxy = match SniWatcherProxy::new(&conn).await {
        Ok(p) => p,
        Err(e) => return e,
    };
    let own_name = conn
        .unique_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let _ = watcher_proxy.register_status_notifier_host(&own_name).await;

    // 3. Subscribe to item registration signals.
    let mut item_registered = match watcher_proxy
        .receive_status_notifier_item_registered()
        .await
    {
        Ok(s) => s,
        Err(e) => return e,
    };
    let mut item_unregistered = match watcher_proxy
        .receive_status_notifier_item_unregistered()
        .await
    {
        Ok(s) => s,
        Err(e) => return e,
    };

    // 4. Subscribe to NameOwnerChanged to detect service disappearances.
    let dbus_proxy = match zbus::fdo::DBusProxy::new(&conn).await {
        Ok(p) => p,
        Err(e) => return e,
    };
    let mut name_changes = match dbus_proxy.receive_name_owner_changed().await {
        Ok(s) => s,
        Err(e) => return e,
    };

    // 5. Fetch initial item list.
    let initial_services = watcher_proxy
        .registered_status_notifier_items()
        .await
        .unwrap_or_default();
    let mut items: HashMap<String, TrayItem> = HashMap::new();
    for service_str in initial_services {
        let (svc, path_str) = parse_service_string(&service_str);
        if let Ok(path) = zbus::zvariant::ObjectPath::try_from(path_str)
            && let Some(item) = fetch_item(&conn, svc, &path).await
        {
            items.insert(service_str, item);
        }
    }
    let snapshot: Vec<TrayItem> = items.values().cloned().collect();
    if tx.send(TrayEvent::Snapshot(snapshot)).is_err() {
        return zbus::Error::Failure("channel closed".into());
    }

    // 6. Event loop.
    loop {
        tokio::select! {
            Some(signal) = item_registered.next() => {
                if let Ok(args) = signal.args() {
                    let service_str = args.service.to_string();
                    let (svc, path_str) = parse_service_string(&service_str);
                    if let Ok(path) = zbus::zvariant::ObjectPath::try_from(path_str)
                        && let Some(item) = fetch_item(&conn, svc, &path).await {
                            // If this service was already known, remove the old entry first.
                            if items.remove(&service_str).is_some()
                                && tx
                                    .send(TrayEvent::ItemRemoved(service_str.clone()))
                                    .is_err()
                            {
                                break;
                            }
                            items.insert(service_str, item.clone());
                            if tx.send(TrayEvent::ItemAdded(item)).is_err() { break; }
                        }
                }
            }
            Some(signal) = item_unregistered.next() => {
                if let Ok(args) = signal.args() {
                    let service_str = args.service.to_string();
                    if items.remove(&service_str).is_some()
                        && tx.send(TrayEvent::ItemRemoved(service_str)).is_err()
                    {
                        break;
                    }
                }
            }
            Some(signal) = name_changes.next() => {
                if let Ok(args) = signal.args() {
                    // A service with no new owner has disappeared.
                    let new_owner_empty = args.new_owner()
                        .as_deref()
                        .map(|o| o.is_empty())
                        .unwrap_or(true);
                    if new_owner_empty {
                        let gone = args.name().to_string();
                        if items.remove(&gone).is_some()
                            && tx.send(TrayEvent::ItemRemoved(gone)).is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }
    }
    zbus::Error::Failure("sni task ended".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_item_has_service_and_status() {
        let item = TrayItem {
            service: "org.kde.konsole".into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("utilities-terminal".into()),
            tooltip: None,
            status: ItemStatus::Active,
            menu_path: None,
        };
        assert_eq!(item.service, "org.kde.konsole");
        assert!(matches!(item.status, ItemStatus::Active));
    }

    #[test]
    fn item_status_from_str_passive() {
        assert_eq!(ItemStatus::from_str("Passive"), ItemStatus::Passive);
    }

    #[test]
    fn parse_service_string_well_known_name() {
        let (svc, path) = parse_service_string("org.kde.konsole");
        assert_eq!(svc, "org.kde.konsole");
        assert_eq!(path, "/StatusNotifierItem");
    }

    #[test]
    fn parse_service_string_unique_name_with_path() {
        let (svc, path) = parse_service_string(":1.42/StatusNotifierItem");
        assert_eq!(svc, ":1.42");
        assert_eq!(path, "/StatusNotifierItem");
    }

    #[test]
    fn parse_service_string_unique_name_custom_path() {
        let (svc, path) = parse_service_string(":1.99/org/kde/tray");
        assert_eq!(svc, ":1.99");
        assert_eq!(path, "/org/kde/tray");
    }

    #[test]
    fn item_status_from_str_needs_attention() {
        assert_eq!(
            ItemStatus::from_str("NeedsAttention"),
            ItemStatus::NeedsAttention
        );
    }

    #[test]
    fn item_status_from_str_unknown_is_active() {
        assert_eq!(ItemStatus::from_str("SomeUnknown"), ItemStatus::Active);
    }

    #[test]
    fn tray_icon_named_stores_string() {
        let icon = TrayIcon::Named("konsole".into());
        match icon {
            TrayIcon::Named(n) => assert_eq!(n, "konsole"),
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn tray_icon_pixmap_stores_vec() {
        let icon = TrayIcon::Pixmap(vec![(22, 22, vec![0u8; 22 * 22 * 4])]);
        match icon {
            TrayIcon::Pixmap(v) => assert_eq!(v[0].0, 22),
            _ => panic!("expected Pixmap"),
        }
    }
}
