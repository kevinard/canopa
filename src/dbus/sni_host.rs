use zbus::zvariant::OwnedObjectPath;

/// A live SNI tray item, fully resolved from D-Bus properties.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TrayItem {
    /// D-Bus service name, e.g. `"org.kde.konsole"` or `":1.42"`.
    pub service: String,
    /// Object path the item is registered at, e.g. `"/StatusNotifierItem"`.
    pub object_path: OwnedObjectPath,
    /// Resolved icon — Named preferred, Pixmap as fallback.
    pub icon: TrayIcon,
    /// Human-readable tooltip text (the `ToolTip.title` field), if any.
    pub tooltip: Option<String>,
    pub status: ItemStatus,
    /// D-Bus path for the context menu (`org.kde.DBusMenu`), if the item exposes one.
    pub menu_path: Option<OwnedObjectPath>,
}

/// Resolved icon data for a tray item.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TrayIcon {
    /// XDG icon theme name. Render via icon theme lookup at display time.
    Named(String),
    /// Raw ARGB32 pixmap data: list of (width, height, bytes) at different sizes.
    /// Bytes are ARGB big-endian; convert to RGBA before passing to iced.
    Pixmap(Vec<(i32, i32, Vec<u8>)>),
}

/// SNI item visibility status.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ItemStatus {
    /// Normal visible state.
    Active,
    /// Item requests to be hidden. Render nothing.
    Passive,
    /// Item requests attention (e.g. new notification).
    NeedsAttention,
}

impl ItemStatus {
    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "Passive" => ItemStatus::Passive,
            "NeedsAttention" => ItemStatus::NeedsAttention,
            _ => ItemStatus::Active,
        }
    }
}

// -- SNI item D-Bus proxy -----------------------------------------------------

#[zbus::proxy(
    interface = "org.kde.StatusNotifierItem",
    default_service = "org.kde.StatusNotifierItem",
    default_path = "/StatusNotifierItem"
)]
trait SniItem {
    fn activate(&self, x: i32, y: i32) -> zbus::Result<()>;
    fn secondary_activate(&self, x: i32, y: i32) -> zbus::Result<()>;
    fn scroll(&self, delta: i32, orientation: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn icon_name(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn icon_pixmap(&self) -> zbus::Result<Vec<(i32, i32, Vec<u8>)>>;

    /// Returns (icon_name, icon_pixmap, title, description).
    #[zbus(property)]
    fn tool_tip(&self) -> zbus::Result<(String, Vec<(i32, i32, Vec<u8>)>, String, String)>;

    #[zbus(property)]
    fn status(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn menu(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    #[zbus(signal)]
    fn new_icon(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn new_status(&self, status: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn new_tool_tip(&self) -> zbus::Result<()>;
}

// -- Watcher proxy (client side) ----------------------------------------------

#[zbus::proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
trait SniWatcher {
    fn register_status_notifier_host(&self, service: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> zbus::Result<Vec<String>>;

    #[zbus(signal)]
    fn status_notifier_item_registered(&self, service: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn status_notifier_item_unregistered(&self, service: String) -> zbus::Result<()>;
}

// -- Helpers ------------------------------------------------------------------

/// Convert ARGB big-endian bytes (SNI wire format) to RGBA bytes (iced format).
pub fn argb_to_rgba(argb: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(argb.len());
    for chunk in argb.chunks_exact(4) {
        // ARGB: chunk[0]=A, chunk[1]=R, chunk[2]=G, chunk[3]=B
        rgba.push(chunk[1]); // R
        rgba.push(chunk[2]); // G
        rgba.push(chunk[3]); // B
        rgba.push(chunk[0]); // A
    }
    rgba
}

/// Pick the pixmap whose width is closest to `target_size`.
/// Returns a reference to the chosen (width, height, bytes) tuple.
pub fn pick_best_pixmap(
    maps: &[(i32, i32, Vec<u8>)],
    target_size: i32,
) -> Option<&(i32, i32, Vec<u8>)> {
    maps.iter()
        .min_by_key(|(w, _, _)| (*w - target_size).unsigned_abs())
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
            let pixmaps = proxy.icon_pixmap().await.unwrap_or_default();
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

// -- Iced subscription --------------------------------------------------------

struct SniConnData(zbus::Connection);

impl std::hash::Hash for SniConnData {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "sni-subscription".hash(h);
    }
}

enum SniSubState {
    Init,
    Running {
        rx: tokio::sync::mpsc::UnboundedReceiver<Vec<TrayItem>>,
    },
    Done,
}

/// Returns an Iced subscription that starts the SNI watcher+host on `conn`
/// and emits `Message::TrayUpdated(Vec<TrayItem>)` on every change.
#[allow(dead_code)]
pub fn sni_subscription(conn: zbus::Connection) -> iced::Subscription<crate::app::Message> {
    iced::Subscription::run_with(SniConnData(conn), |data: &SniConnData| {
        let conn = data.0.clone();
        iced::futures::stream::unfold(SniSubState::Init, move |state| {
            let conn = conn.clone();
            async move {
                match state {
                    SniSubState::Init => {
                        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                        tokio::spawn(async move {
                            let _ = run_sni_task(conn, tx).await;
                        });
                        // Emit empty list immediately so the tray module starts in Loading state.
                        Some((
                            crate::app::Message::TrayUpdated(vec![]),
                            SniSubState::Running { rx },
                        ))
                    }
                    SniSubState::Running { mut rx } => match rx.recv().await {
                        Some(items) => Some((
                            crate::app::Message::TrayUpdated(items),
                            SniSubState::Running { rx },
                        )),
                        None => Some((
                            crate::app::Message::TrayUpdated(vec![]),
                            SniSubState::Done,
                        )),
                    },
                    SniSubState::Done => None,
                }
            }
        })
    })
}

/// Drives the SNI watcher and host.
///
/// 1. Starts the watcher (or falls back to connecting to an existing one).
/// 2. Registers niribar as a host.
/// 3. Fetches the initial item list.
/// 4. Listens for item registrations/unregistrations and name disappearances.
/// 5. Sends updated `Vec<TrayItem>` on `tx` on every change.
#[allow(dead_code)]
async fn run_sni_task(
    conn: zbus::Connection,
    tx: tokio::sync::mpsc::UnboundedSender<Vec<TrayItem>>,
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
    let own_name = conn.unique_name().map(|n| n.to_string()).unwrap_or_default();
    let _ = watcher_proxy.register_status_notifier_host(&own_name).await;

    // 3. Subscribe to item registration signals.
    let mut item_registered = match watcher_proxy.receive_status_notifier_item_registered().await {
        Ok(s) => s,
        Err(e) => return e,
    };
    let mut item_unregistered = match watcher_proxy.receive_status_notifier_item_unregistered().await {
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
    for service in initial_services {
        let path = zbus::zvariant::ObjectPath::try_from("/StatusNotifierItem").unwrap();
        if let Some(item) = fetch_item(&conn, &service, &path).await {
            items.insert(service, item);
        }
    }
    let snapshot: Vec<TrayItem> = items.values().cloned().collect();
    if tx.send(snapshot).is_err() {
        return zbus::Error::Failure("channel closed".into());
    }

    // 6. Event loop.
    loop {
        tokio::select! {
            Some(signal) = item_registered.next() => {
                if let Ok(args) = signal.args() {
                    let service = args.service.to_string();
                    let path = zbus::zvariant::ObjectPath::try_from("/StatusNotifierItem").unwrap();
                    if let Some(item) = fetch_item(&conn, &service, &path).await {
                        items.insert(service, item);
                        let snapshot: Vec<TrayItem> = items.values().cloned().collect();
                        if tx.send(snapshot).is_err() { break; }
                    }
                }
            }
            Some(signal) = item_unregistered.next() => {
                if let Ok(args) = signal.args() {
                    items.remove(args.service.as_str());
                    let snapshot: Vec<TrayItem> = items.values().cloned().collect();
                    if tx.send(snapshot).is_err() { break; }
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
                        if items.remove(&gone).is_some() {
                            let snapshot: Vec<TrayItem> = items.values().cloned().collect();
                            if tx.send(snapshot).is_err() { break; }
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
    fn argb_to_rgba_converts_channel_order() {
        // ARGB bytes: A=0xFF, R=0x11, G=0x22, B=0x33 -> RGBA: R=0x11, G=0x22, B=0x33, A=0xFF
        let argb = vec![0xFFu8, 0x11, 0x22, 0x33];
        let rgba = argb_to_rgba(&argb);
        assert_eq!(rgba, vec![0x11u8, 0x22, 0x33, 0xFF]);
    }

    #[test]
    fn argb_to_rgba_handles_empty() {
        assert_eq!(argb_to_rgba(&[]), Vec::<u8>::new());
    }

    #[test]
    fn pick_best_pixmap_prefers_closest_to_target() {
        let maps = vec![
            (16i32, 16i32, vec![0u8; 16 * 16 * 4]),
            (22i32, 22i32, vec![0u8; 22 * 22 * 4]),
            (32i32, 32i32, vec![0u8; 32 * 32 * 4]),
        ];
        let best = pick_best_pixmap(&maps, 20);
        assert_eq!(best.map(|b| b.0), Some(22));
    }

    #[test]
    fn pick_best_pixmap_returns_none_for_empty() {
        let maps: Vec<(i32, i32, Vec<u8>)> = vec![];
        assert!(pick_best_pixmap(&maps, 20).is_none());
    }

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
    fn item_status_from_str_needs_attention() {
        assert_eq!(ItemStatus::from_str("NeedsAttention"), ItemStatus::NeedsAttention);
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
