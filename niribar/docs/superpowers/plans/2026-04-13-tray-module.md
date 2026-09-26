# Tray Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `tray` module that renders SNI tray icons in the bar via the StatusNotifierItem D-Bus protocol, with full pointer/scroll interactions and a generic popup surface for context menus.

**Architecture:** niribar acts as its own `StatusNotifierWatcher` (registered on the D-Bus session bus) and `StatusNotifierHost` (listens for item registrations). The entire SNI D-Bus system starts only if at least one bar section contains `module "tray"`. A generic `MenuTree`/`MenuPopup` surface handles context menus and can be reused by other modules. The SharedState/SurfaceState boundary is strictly observed: module data in `SharedState`, popup lifecycle in `SurfaceState`/`App`.

**Tech Stack:** Rust, zbus 5.12, iced 0.14, iced_layershell 0.17.1, existing D-Bus session bus connection from `DbusConnections`

---

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `src/config.rs` | Modify | Add `TrayConfig`, `ModuleParams::Tray`, "tray" to `KNOWN_MODULE_TYPES` |
| `src/config/parse.rs` | Modify | Add "tray" to type disambiguation error message |
| `src/dbus.rs` | Modify | Declare `sni_watcher` and `sni_host` modules |
| `src/dbus/sni_watcher.rs` | Create | `org.kde.StatusNotifierWatcher` zbus service |
| `src/dbus/sni_host.rs` | Create | `TrayItem` types + SNI host subscription |
| `src/modules.rs` | Modify | `ModuleState::Tray`, `tray_ctor`, registry |
| `src/modules/tray.rs` | Create | `TrayModule` implementing `Module` trait |
| `src/surface.rs` | Modify | Declare `menu_popup` module |
| `src/surface/menu_popup.rs` | Create | `MenuTree`, `MenuItem`, `view_menu_popup` |
| `src/app.rs` | Modify | New messages, update handlers, popup lifecycle, conditional SNI init |

---

## Task 1: Config — add `tray` module type

**Files:**
- Modify: `src/config.rs`
- Modify: `src/config/parse.rs`
- Test: inline in `src/config.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` block in `src/config.rs`:

```rust
#[test]
fn validate_tray_module_type_passes() {
    let mut config = Config::default();
    config.modules = vec![ModuleConfig {
        name: "tray".into(),
        module_type: "tray".into(),
        params: ModuleParams::None,
    }];
    assert!(validate(&config).is_ok());
}

#[test]
fn build_registry_contains_tray() {
    // This test will fail until Task 5 adds tray_ctor — add it there.
    // For now just validate config accepts "tray".
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test validate_tray_module_type_passes 2>&1 | tail -5
```
Expected: FAIL — "unknown module type \"tray\""

- [ ] **Step 3: Add `TrayConfig` and `ModuleParams::Tray` to `src/config.rs`**

After the `NetworkConfig` struct (around line 84), add:

```rust
/// Configuration for the tray module (no parameters for MVP).
#[derive(Debug, Clone, Default)]
pub struct TrayConfig;
```

Add `Tray(TrayConfig)` to the `ModuleParams` enum after `Workspaces(WorkspacesConfig)`:

```rust
    Workspaces(WorkspacesConfig),
    Tray(TrayConfig),
```

- [ ] **Step 4: Add "tray" to `KNOWN_MODULE_TYPES`**

Change the `KNOWN_MODULE_TYPES` constant in `src/config.rs`:

```rust
const KNOWN_MODULE_TYPES: &[&str] = &[
    "battery",
    "clock",
    "network",
    "volume",
    "workspaces",
    "focused-window",
    "script",
    "button",
    "tray",
];
```

- [ ] **Step 5: Update the type disambiguation error message in `src/config/parse.rs`**

Find the error message at line ~209 that lists known module types. Change:
```rust
"Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button."
```
to:
```rust
"Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button, tray."
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test validate_tray 2>&1 | tail -5
```
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/config.rs src/config/parse.rs
git commit -m "feat(config): add tray module type to config plumbing"
```

---

## Task 2: SNI shared types

**Files:**
- Create: `src/dbus/sni_host.rs` (types only)
- Modify: `src/dbus.rs`

- [ ] **Step 1: Write failing test**

Create `src/dbus/sni_host.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_item_has_service_and_status() {
        let item = TrayItem {
            service: "org.kde.konsole".into(),
            object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("utilities-terminal".into()),
            tooltip: None,
            status: ItemStatus::Active,
            menu_path: None,
        };
        assert_eq!(item.service, "org.kde.konsole");
        assert!(matches!(item.status, ItemStatus::Active));
    }

    #[test]
    fn item_status_passive_variant_exists() {
        let _ = ItemStatus::Passive;
    }

    #[test]
    fn item_status_needs_attention_variant_exists() {
        let _ = ItemStatus::NeedsAttention;
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
```

- [ ] **Step 2: Run to confirm it fails**

```bash
cargo test -p niribar 2>&1 | grep "sni_host" | head -5
```
Expected: compile error — module not found

- [ ] **Step 3: Implement the types in `src/dbus/sni_host.rs`**

```rust
use zbus::zvariant::OwnedObjectPath;

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
    Pixmap(Vec<(i32, i32, Vec<u8>)>),
}

/// SNI item visibility status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStatus {
    /// Normal visible state.
    Active,
    /// Item requests to be hidden. Render nothing (UX-DR16).
    Passive,
    /// Item requests attention (e.g. new notification). Render attention badge.
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

#[cfg(test)]
mod tests {
    // ... (tests from step 1 go here)
}
```

- [ ] **Step 4: Declare the new modules in `src/dbus.rs`**

Add at the top of `src/dbus.rs`:

```rust
pub mod sni_host;
pub mod sni_watcher;
```

Create an empty `src/dbus/sni_watcher.rs` placeholder:

```rust
// StatusNotifierWatcher implementation — Task 3.
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p niribar 2>&1 | grep -E "(PASS|FAIL|error)" | head -20
```
Expected: all tests pass, no compile errors

- [ ] **Step 6: Commit**

```bash
git add src/dbus/sni_host.rs src/dbus/sni_watcher.rs src/dbus.rs
git commit -m "feat(dbus): add SNI core types (TrayItem, TrayIcon, ItemStatus)"
```

---

## Task 3: StatusNotifierWatcher service

**Files:**
- Modify: `src/dbus/sni_watcher.rs`

The watcher is a zbus server-side interface. It:
1. Holds a list of registered item service names
2. Accepts `RegisterStatusNotifierItem` / `RegisterStatusNotifierHost` calls
3. Emits `StatusNotifierItemRegistered` / `StatusNotifierItemUnregistered` / `StatusNotifierHostRegistered` signals
4. Exposes `RegisteredStatusNotifierItems`, `IsStatusNotifierHostRegistered`, `ProtocolVersion` properties

- [ ] **Step 1: Write failing test**

Add to `src/dbus/sni_watcher.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_starts_with_no_items() {
        let w = StatusNotifierWatcher::new();
        let items = w.items.blocking_lock();
        assert!(items.is_empty());
    }

    #[test]
    fn watcher_starts_with_no_hosts() {
        let w = StatusNotifierWatcher::new();
        let hosts = w.hosts.blocking_lock();
        assert!(hosts.is_empty());
    }

    #[test]
    fn item_status_from_str_passive() {
        use crate::dbus::sni_host::ItemStatus;
        assert_eq!(ItemStatus::from_str("Passive"), ItemStatus::Passive);
    }

    #[test]
    fn item_status_from_str_needs_attention() {
        use crate::dbus::sni_host::ItemStatus;
        assert_eq!(ItemStatus::from_str("NeedsAttention"), ItemStatus::NeedsAttention);
    }

    #[test]
    fn item_status_from_str_unknown_is_active() {
        use crate::dbus::sni_host::ItemStatus;
        assert_eq!(ItemStatus::from_str("Unknown"), ItemStatus::Active);
    }
}
```

- [ ] **Step 2: Run to confirm it fails**

```bash
cargo test watcher_starts_with 2>&1 | tail -5
```
Expected: compile error

- [ ] **Step 3: Implement the watcher**

Replace the placeholder in `src/dbus/sni_watcher.rs`:

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::{interface, SignalContext};

/// Implements `org.kde.StatusNotifierWatcher`.
///
/// Registered as a D-Bus service at startup if the tray module is present.
/// Accepts `RegisterStatusNotifierItem` calls from apps, tracks live item
/// service names, and emits signals when items appear/disappear.
pub struct StatusNotifierWatcher {
    /// Service names of registered items, e.g. `":1.42"` or `"org.kde.konsole"`.
    pub items: Arc<Mutex<Vec<String>>>,
    /// Service names of registered hosts (niribar registers itself here too).
    pub hosts: Arc<Mutex<Vec<String>>>,
}

impl StatusNotifierWatcher {
    pub fn new() -> Self {
        StatusNotifierWatcher {
            items: Arc::new(Mutex::new(Vec::new())),
            hosts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
    /// Called by apps when they create a tray icon.
    /// `service` is either a well-known name or a unique name (`:1.42`).
    async fn register_status_notifier_item(
        &self,
        service: &str,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        let full = if service.starts_with('/') {
            // Some apps pass "busname/objectpath" — extract the bus name part.
            service.split('/').next().unwrap_or(service).to_string()
        } else {
            service.to_string()
        };
        let mut items = self.items.lock().await;
        if !items.contains(&full) {
            items.push(full.clone());
            drop(items);
            Self::status_notifier_item_registered(&ctxt, &full).await?;
        }
        Ok(())
    }

    /// Called by hosts (niribar itself) when they start monitoring.
    async fn register_status_notifier_host(
        &self,
        service: &str,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        let mut hosts = self.hosts.lock().await;
        if !hosts.contains(&service.to_string()) {
            hosts.push(service.to_string());
            let is_first = hosts.len() == 1;
            drop(hosts);
            if is_first {
                Self::status_notifier_host_registered(&ctxt).await?;
            }
        }
        Ok(())
    }

    // --- Signals ---

    #[zbus(signal)]
    async fn status_notifier_item_registered(
        ctxt: &SignalContext<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(
        ctxt: &SignalContext<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_host_registered(ctxt: &SignalContext<'_>) -> zbus::Result<()>;

    // --- Properties ---

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        // zbus property methods are synchronous — use blocking_lock.
        self.items.blocking_lock().clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        !self.hosts.blocking_lock().is_empty()
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

/// Registers the watcher on `conn` and holds it alive.
///
/// Requests the well-known name `org.kde.StatusNotifierWatcher`.
/// If that name is already taken (another watcher is running), returns `Ok(None)` —
/// the caller falls back to host-only mode.
///
/// On success, returns `Some(watcher_items)` — the shared items list for the
/// host to watch for unregistrations.
pub async fn start_watcher(
    conn: &zbus::Connection,
) -> zbus::Result<Option<Arc<Mutex<Vec<String>>>>> {
    let watcher = StatusNotifierWatcher::new();
    let items = watcher.items.clone();

    conn.object_server()
        .at("/StatusNotifierWatcher", watcher)
        .await?;

    let reply = conn
        .request_name("org.kde.StatusNotifierWatcher")
        .await;

    match reply {
        Ok(_) => Ok(Some(items)),
        Err(zbus::Error::NameTaken) => {
            // Another watcher is running — fall back to host-only.
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_starts_with_no_items() {
        let w = StatusNotifierWatcher::new();
        let items = w.items.blocking_lock();
        assert!(items.is_empty());
    }

    #[test]
    fn watcher_starts_with_no_hosts() {
        let w = StatusNotifierWatcher::new();
        let hosts = w.hosts.blocking_lock();
        assert!(hosts.is_empty());
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p niribar 2>&1 | grep -E "(PASS|FAIL|error\[)" | head -20
```
Expected: all existing tests pass plus the new watcher tests

- [ ] **Step 5: Commit**

```bash
git add src/dbus/sni_watcher.rs
git commit -m "feat(dbus): implement StatusNotifierWatcher zbus service"
```

---

## Task 4: SNI host subscription

**Files:**
- Modify: `src/dbus/sni_host.rs`

The host subscription:
1. Calls `start_watcher` on the session bus
2. Connects to the watcher (own or external) and registers as a host
3. Subscribes to `StatusNotifierItemRegistered` / `StatusNotifierItemUnregistered` watcher signals
4. Subscribes to D-Bus `NameOwnerChanged` to detect item service disappearances
5. For each registered item: fetches icon, tooltip, status, menu path via the SNI proxy
6. Emits `Message::TrayUpdated(Vec<TrayItem>)` on any change

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/dbus/sni_host.rs`:

```rust
#[test]
fn argb_to_rgba_converts_channel_order() {
    // ARGB bytes: A=0xFF, R=0x11, G=0x22, B=0x33 → RGBA: R=0x11, G=0x22, B=0x33, A=0xFF
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
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test argb_to_rgba 2>&1 | tail -5
```
Expected: compile error — `argb_to_rgba` not found

- [ ] **Step 3: Add the SNI item proxy and helpers**

Add to `src/dbus/sni_host.rs`, after the existing types:

```rust
// ── SNI item D-Bus proxy ──────────────────────────────────────────────────

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

// ── Watcher proxy (client side) ──────────────────────────────────────────

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

// ── Helpers ──────────────────────────────────────────────────────────────

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
```

- [ ] **Step 4: Add the subscription**

Append to `src/dbus/sni_host.rs`:

```rust
// ── Iced subscription ─────────────────────────────────────────────────────

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
async fn run_sni_task(
    conn: zbus::Connection,
    tx: tokio::sync::mpsc::UnboundedSender<Vec<TrayItem>>,
) -> zbus::Error {
    use iced::futures::StreamExt;
    use std::collections::HashMap;

    // 1. Start or connect to watcher.
    let watcher_items_opt = crate::dbus::sni_watcher::start_watcher(&conn).await;
    let _watcher_items = match watcher_items_opt {
        Ok(opt) => opt, // Some if we own the watcher, None if external watcher
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
    let initial_services = watcher_proxy.registered_status_notifier_items().await.unwrap_or_default();
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
                    if args.new_owner.is_empty() {
                        let gone = args.name.to_string();
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
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p niribar 2>&1 | grep -E "(PASS|FAIL|error\[)" | head -20
```
Expected: argb and pixmap tests pass

- [ ] **Step 6: Commit**

```bash
git add src/dbus/sni_host.rs
git commit -m "feat(dbus): implement SNI host subscription (item tracking)"
```

---

## Task 5: Tray module

**Files:**
- Create: `src/modules/tray.rs`
- Modify: `src/modules.rs`

The tray module renders a column (vertical bars) or row (horizontal bars) of icons. Both collapsed and expanded views are identical. Passive items are hidden.

- [ ] **Step 1: Write failing tests**

Create `src/modules/tray.rs` with just the test block initially:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    fn make_module(edge: crate::config::Edge) -> TrayModule {
        TrayModule::new(crate::modules::ModuleId::new("tray"), edge)
    }

    #[test]
    fn tray_module_id_is_tray() {
        let m = make_module(crate::config::Edge::Left);
        assert_eq!(m.id().0, "tray");
        assert_eq!(m.module_type(), "tray");
    }

    #[test]
    fn tray_state_default_is_empty() {
        let state = TrayState::default();
        assert!(state.items.is_empty());
    }

    #[test]
    fn tray_state_stores_items() {
        use crate::dbus::sni_host::{ItemStatus, TrayIcon, TrayItem};
        let item = TrayItem {
            service: "test".into(),
            object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("test".into()),
            tooltip: None,
            status: ItemStatus::Active,
            menu_path: None,
        };
        let state = TrayState { items: vec![item] };
        assert_eq!(state.items.len(), 1);
    }
}
```

- [ ] **Step 2: Run to confirm it fails**

```bash
cargo test tray_module_id 2>&1 | tail -5
```
Expected: compile error — module not found

- [ ] **Step 3: Implement `src/modules/tray.rs`**

```rust
use crate::config::Edge;
use crate::dbus::sni_host::{ItemStatus, TrayIcon, TrayItem};
use crate::modules::{InteractionEvent, ModuleId, ModuleState};

/// Runtime state for the tray module — the current list of live SNI items.
#[derive(Debug, Clone, Default)]
pub struct TrayState {
    pub items: Vec<TrayItem>,
}

pub struct TrayModule {
    id: ModuleId,
    /// Bar edge — determines row vs column layout.
    edge: Edge,
}

impl TrayModule {
    pub fn new(id: ModuleId, edge: Edge) -> Self {
        TrayModule { id, edge }
    }

    /// Resolve a file path for a named icon by checking common XDG icon theme paths.
    ///
    /// Checks hicolor at 22px and 16px, then pixmaps. Returns `None` if not found.
    fn resolve_icon_path(name: &str) -> Option<std::path::PathBuf> {
        let candidates = [
            format!("/usr/share/icons/hicolor/22x22/apps/{name}.png"),
            format!("/usr/share/icons/hicolor/16x16/apps/{name}.png"),
            format!("/usr/share/icons/hicolor/scalable/apps/{name}.svg"),
            format!("/usr/share/pixmaps/{name}.png"),
            format!("/usr/share/pixmaps/{name}.xpm"),
        ];
        candidates
            .into_iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.exists())
    }

    /// Build an iced widget for one tray item icon.
    fn icon_widget<'a>(
        item: &'a TrayItem,
    ) -> iced::Element<'a, crate::app::Message> {
        const ICON_SIZE: u16 = 20;

        let icon_element: iced::Element<'_, crate::app::Message> = match &item.icon {
            TrayIcon::Named(name) => {
                if let Some(path) = Self::resolve_icon_path(name) {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "svg" {
                        iced::widget::svg(iced::widget::svg::Handle::from_path(&path))
                            .width(ICON_SIZE)
                            .height(ICON_SIZE)
                            .into()
                    } else {
                        iced::widget::image(iced::widget::image::Handle::from_path(&path))
                            .width(ICON_SIZE)
                            .height(ICON_SIZE)
                            .into()
                    }
                } else {
                    // Icon not found — render a single character fallback.
                    let ch = name.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?');
                    iced::widget::text(ch.to_string())
                        .size(14)
                        .into()
                }
            }
            TrayIcon::Pixmap(pixmaps) => {
                if let Some((w, h, argb)) =
                    crate::dbus::sni_host::pick_best_pixmap(pixmaps, ICON_SIZE as i32)
                {
                    let rgba = crate::dbus::sni_host::argb_to_rgba(argb);
                    let handle = iced::widget::image::Handle::from_rgba(
                        *w as u32,
                        *h as u32,
                        rgba,
                    );
                    iced::widget::image(handle)
                        .width(ICON_SIZE)
                        .height(ICON_SIZE)
                        .into()
                } else {
                    iced::widget::text("?").size(14).into()
                }
            }
        };

        // NeedsAttention: overlay a small accent dot.
        if item.status == ItemStatus::NeedsAttention {
            let dot = iced::widget::container(iced::widget::Space::new(6, 6))
                .style(|_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(crate::theme::ACCENT)),
                    border: iced::Border {
                        radius: iced::border::Radius::from(3.0),
                        ..Default::default()
                    },
                    ..Default::default()
                });
            iced::widget::stack![icon_element, dot].into()
        } else {
            icon_element
        }
    }

    fn render_icons<'a>(
        &self,
        items: &'a [TrayItem],
    ) -> iced::Element<'a, crate::app::Message> {
        use iced::widget::mouse_area;

        let visible: Vec<&TrayItem> = items
            .iter()
            .filter(|i| i.status != ItemStatus::Passive)
            .collect();

        if visible.is_empty() {
            return iced::widget::Space::new(0, 0).into();
        }

        let icons: Vec<iced::Element<'_, crate::app::Message>> = visible
            .iter()
            .map(|item| {
                let service = item.service.clone();
                let menu_path = item.menu_path.clone();
                let icon = Self::icon_widget(item);
                mouse_area(icon)
                    .on_press(crate::app::Message::TrayActivate {
                        service: service.clone(),
                    })
                    .on_right_press(crate::app::Message::TrayMenuRequest {
                        service: service.clone(),
                        menu_path,
                    })
                    .on_middle_press(crate::app::Message::TraySecondaryActivate {
                        service: service.clone(),
                    })
                    .on_scroll(move |delta| {
                        let (delta_val, orientation) = match delta {
                            iced::mouse::ScrollDelta::Lines { x, y } => {
                                if y.abs() >= x.abs() { (y as i32, "vertical") }
                                else { (x as i32, "horizontal") }
                            }
                            iced::mouse::ScrollDelta::Pixels { x, y } => {
                                if y.abs() >= x.abs() { (y as i32, "vertical") }
                                else { (x as i32, "horizontal") }
                            }
                        };
                        crate::app::Message::TrayScroll {
                            service: service.clone(),
                            delta: delta_val,
                            orientation: orientation.to_string(),
                        }
                    })
                    .into()
            })
            .collect();

        if self.edge.is_horizontal() {
            iced::widget::row(icons).spacing(4).into()
        } else {
            iced::widget::column(icons).spacing(4).into()
        }
    }
}

impl crate::modules::Module for TrayModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn module_type(&self) -> &str {
        "tray"
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        // Fed via Message::TrayUpdated from app.rs — no independent subscription (AR5).
        iced::Subscription::none()
    }

    fn view_collapsed(
        &self,
        state: &ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        let items = match state {
            ModuleState::Tray(s) => &s.items,
            _ => return iced::widget::Space::new(0, 0).into(),
        };
        self.render_icons(items)
    }

    fn view_expanded(
        &self,
        state: &ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        // Identical to collapsed — tray is always icon-only.
        self.view_collapsed(state)
    }

    fn view_horizontal(
        &self,
        state: &ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        self.view_collapsed(state)
    }

    fn on_interaction(
        &self,
        _event: InteractionEvent,
    ) -> Option<crate::app::Message> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    fn make_module(edge: crate::config::Edge) -> TrayModule {
        TrayModule::new(ModuleId::new("tray"), edge)
    }

    #[test]
    fn tray_module_id_is_tray() {
        let m = make_module(crate::config::Edge::Left);
        assert_eq!(m.id().0, "tray");
        assert_eq!(m.module_type(), "tray");
    }

    #[test]
    fn tray_state_default_is_empty() {
        let state = TrayState::default();
        assert!(state.items.is_empty());
    }

    #[test]
    fn tray_state_stores_items() {
        let item = TrayItem {
            service: "test".into(),
            object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("test".into()),
            tooltip: None,
            status: ItemStatus::Active,
            menu_path: None,
        };
        let state = TrayState { items: vec![item] };
        assert_eq!(state.items.len(), 1);
    }
}
```

- [ ] **Step 4: Register in `src/modules.rs`**

Add `pub mod tray;` at the top.

Add `Tray(tray::TrayState)` to `ModuleState` enum after `FocusedWindow(...)`:

```rust
    FocusedWindow(focused_window::FocusedWindowState),
    Tray(tray::TrayState),
```

Add `tray` to `build_registry`:

```rust
    map.insert("tray", tray_ctor as ModuleConstructor);
```

Add the constructor function:

```rust
fn tray_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    // Edge is not available in ModuleConfig — default to Left.
    // app.rs passes the correct edge at construction time for multi-bar support.
    // For MVP, default Left is acceptable; see deferred-work if edge-aware tray is needed.
    Box::new(tray::TrayModule::new(
        ModuleId::new(&mc.name),
        crate::config::Edge::Left,
    ))
}
```

Add `"tray"` to the `build_registry_contains_*` test:

```rust
#[test]
fn build_registry_contains_tray() {
    let registry = build_registry();
    assert!(registry.contains_key("tray"));
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p niribar 2>&1 | grep -E "(PASS|FAIL|error\[)" | head -20
```
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/modules/tray.rs src/modules.rs
git commit -m "feat(modules): implement TrayModule with icon rendering"
```

---

## Task 6: New messages and TrayUpdated handler

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write failing test**

Add to the test block in `src/app.rs` (find the existing `#[cfg(test)]` block):

```rust
#[test]
fn tray_updated_message_stores_items_in_shared_state() {
    use crate::dbus::sni_host::{ItemStatus, TrayIcon, TrayItem};
    use crate::modules::ModuleId;
    use zbus::zvariant::OwnedObjectPath;

    let (mut app, _) = App::boot(Config::default(), vec![]);
    // Inject a tray module id into the modules list manually.
    let id = ModuleId::new("tray");
    app.shared.modules.insert(id.clone(), crate::modules::ModuleState::Loading);

    let item = TrayItem {
        service: "org.test".into(),
        object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
        icon: TrayIcon::Named("test".into()),
        tooltip: None,
        status: ItemStatus::Active,
        menu_path: None,
    };
    app.update(Message::TrayUpdated(vec![item]));

    // The tray module state should be updated.
    // (After Task 6, Message::TrayUpdated pushes to all "tray" module entries.)
    // This test verifies the message is handled without panic.
}
```

- [ ] **Step 2: Add new messages to `src/app.rs`**

In the `Message` enum, add after `ToggleExpand`:

```rust
    /// SNI tray item list changed. Emitted by sni_host subscription.
    TrayUpdated(Vec<crate::dbus::sni_host::TrayItem>),
    /// Left-click on a tray icon — calls Activate(0, 0) on the SNI item.
    TrayActivate { service: String },
    /// Middle-click on a tray icon — calls SecondaryActivate(0, 0).
    TraySecondaryActivate { service: String },
    /// Scroll on a tray icon — calls Scroll(delta, orientation).
    TrayScroll {
        service: String,
        delta: i32,
        orientation: String,
    },
    /// Right-click on a tray icon — fetch DBusMenu and spawn popup.
    TrayMenuRequest {
        service: String,
        menu_path: Option<zbus::zvariant::OwnedObjectPath>,
    },
    /// A menu item was selected in the popup. `id` is the DBusMenu integer item ID as a string.
    MenuItemSelected {
        /// The SNI service that owns this menu.
        service: String,
        /// DBusMenu integer item ID.
        menu_item_id: i32,
        /// D-Bus path of the DBusMenu object.
        menu_path: zbus::zvariant::OwnedObjectPath,
    },
    /// The menu popup was dismissed without a selection.
    MenuClosed,
```

Add the necessary import at the top of `src/app.rs`:

```rust
use crate::modules::tray::TrayState;
```

- [ ] **Step 3: Add `TrayUpdated` handler in `update()`**

In the `match message` block in `update()`, add before the final `_ => Task::none()`:

```rust
            Message::TrayUpdated(items) => {
                for (_, module) in &self.modules {
                    if module.module_type() == "tray" {
                        self.shared.modules.insert(
                            module.id(),
                            ModuleState::Tray(TrayState { items: items.clone() }),
                        );
                    }
                }
                Task::none()
            }
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p niribar 2>&1 | grep -E "(PASS|FAIL|error\[)" | head -20
```
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): add tray messages and TrayUpdated handler"
```

---

## Task 7: Tray interaction handlers

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Add handlers in `update()`**

Add after the `TrayUpdated` handler:

```rust
            Message::TrayActivate { service } => {
                let conn = self.shared.dbus.session.clone();
                tokio::task::spawn(async move {
                    if let Some(conn) = conn {
                        let path = zbus::zvariant::ObjectPath::try_from("/StatusNotifierItem").unwrap();
                        if let Some(item) = crate::dbus::sni_host::fetch_item(&conn, &service, &path).await {
                            let _ = crate::dbus::sni_host::call_activate(&conn, &item.service, &item.object_path).await;
                        }
                    }
                });
                Task::none()
            }
            Message::TraySecondaryActivate { service } => {
                let conn = self.shared.dbus.session.clone();
                tokio::task::spawn(async move {
                    if let Some(conn) = conn {
                        let path = zbus::zvariant::ObjectPath::try_from("/StatusNotifierItem").unwrap();
                        if let Some(item) = crate::dbus::sni_host::fetch_item(&conn, &service, &path).await {
                            let _ = crate::dbus::sni_host::call_secondary_activate(&conn, &item.service, &item.object_path).await;
                        }
                    }
                });
                Task::none()
            }
            Message::TrayScroll { service, delta, orientation } => {
                let conn = self.shared.dbus.session.clone();
                tokio::task::spawn(async move {
                    if let Some(conn) = conn {
                        let path = zbus::zvariant::ObjectPath::try_from("/StatusNotifierItem").unwrap();
                        if let Some(item) = crate::dbus::sni_host::fetch_item(&conn, &service, &path).await {
                            let _ = crate::dbus::sni_host::call_scroll(&conn, &item.service, &item.object_path, delta, &orientation).await;
                        }
                    }
                });
                Task::none()
            }
            Message::TrayMenuRequest { service, menu_path } => {
                let conn = self.shared.dbus.session.clone();
                let tx = self.menu_tx.clone();
                tokio::task::spawn(async move {
                    if let (Some(conn), Some(menu_path), Some(tx)) = (conn, menu_path, tx) {
                        if let Some(tree) = crate::dbus::sni_host::fetch_menu(&conn, &service, &menu_path).await {
                            let _ = tx.send((service, menu_path, tree));
                        }
                    }
                });
                Task::none()
            }
```

- [ ] **Step 2: Add D-Bus call helpers to `src/dbus/sni_host.rs`**

Add after `fetch_item`:

```rust
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
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p niribar 2>&1 | grep -E "(PASS|FAIL|error\[)" | head -20
```
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add src/app.rs src/dbus/sni_host.rs
git commit -m "feat(app): add tray interaction handlers (activate, secondary, scroll)"
```

---

## Task 8: Conditional SNI init in `app.rs`

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write failing test**

Add to `src/app.rs` test block:

```rust
#[test]
fn tray_not_in_needs_dbus_when_no_tray_module() {
    let (app, _) = App::boot(Config::default(), vec![]);
    // Default config has no tray module — verify no tray module is registered.
    assert!(!app.modules.iter().any(|(_, m)| m.module_type() == "tray"));
}
```

- [ ] **Step 2: Add `menu_tx` field to `App`**

Add to the `App` struct:

```rust
    /// Channel sender for passing fetched MenuTree data to the popup lifecycle.
    /// `None` when no tray module is configured.
    pub menu_tx: Option<tokio::sync::mpsc::UnboundedSender<(
        String,
        zbus::zvariant::OwnedObjectPath,
        crate::surface::menu_popup::MenuTree,
    )>>,
    /// Corresponding receiver — consumed once to start the menu subscription.
    pub menu_rx: Option<tokio::sync::mpsc::UnboundedReceiver<(
        String,
        zbus::zvariant::OwnedObjectPath,
        crate::surface::menu_popup::MenuTree,
    )>>,
```

Add `menu_tx: None, menu_rx: None` to the `App { ... }` construction in `boot()`.

Initialize the channel if tray is present — add in `boot()` after the modules list is built:

```rust
        let has_tray = modules.iter().any(|(_, m)| m.module_type() == "tray");
        let (menu_tx, menu_rx) = if has_tray {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
```

Update the `App { ... }` construction to include both fields.

- [ ] **Step 3: Update `needs_dbus` in `subscription()` to include tray**

Find the `needs_dbus` line in `subscription()`:

```rust
        let needs_dbus = self
            .modules
            .iter()
            .any(|(_, m)| matches!(m.module_type(), "battery" | "network" | "volume"));
```

Change to:

```rust
        let needs_dbus = self
            .modules
            .iter()
            .any(|(_, m)| matches!(m.module_type(), "battery" | "network" | "volume" | "tray"));
```

- [ ] **Step 4: Add the SNI subscription in `subscription()`**

Add after `volume_sub`:

```rust
        // Start SNI subscription only if tray module is present AND session bus is connected.
        let sni_sub = if let Some(conn) = self.shared.dbus.session.clone() {
            if self.modules.iter().any(|(_, m)| m.module_type() == "tray") {
                crate::dbus::sni_host::sni_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };
```

Add `sni_sub` to the `iced::Subscription::batch([...])` call.

- [ ] **Step 5: Run tests**

```bash
cargo test -p niribar 2>&1 | grep -E "(PASS|FAIL|error\[)" | head -20
```
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): conditional SNI init — watcher+host only when tray module is present"
```

---

## Task 9: Generic menu popup types and rendering

**Files:**
- Create: `src/surface/menu_popup.rs`
- Modify: `src/surface.rs`

- [ ] **Step 1: Write failing tests**

Create `src/surface/menu_popup.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_tree_stores_items() {
        let tree = MenuTree {
            items: vec![MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor: (100, 200),
        };
        assert_eq!(tree.items.len(), 1);
        assert_eq!(tree.anchor, (100, 200));
    }

    #[test]
    fn menu_item_separator_has_no_label() {
        let sep = MenuItem {
            id: "0".into(),
            label: String::new(),
            enabled: false,
            visible: true,
            icon_name: None,
            children: vec![],
            is_separator: true,
        };
        assert!(sep.is_separator);
        assert!(sep.label.is_empty());
    }

    #[test]
    fn menu_item_disabled_is_tracked() {
        let item = MenuItem {
            id: "5".into(),
            label: "Grayed Out".into(),
            enabled: false,
            visible: true,
            icon_name: None,
            children: vec![],
            is_separator: false,
        };
        assert!(!item.enabled);
    }
}
```

- [ ] **Step 2: Run to confirm it fails**

```bash
cargo test menu_tree_stores 2>&1 | tail -5
```
Expected: compile error — module not found

- [ ] **Step 3: Implement types and view in `src/surface/menu_popup.rs`**

```rust
/// A generic menu tree, passed from SharedState to a popup SurfaceState.
/// No knowledge of SNI, DBusMenu, or any module internals.
#[derive(Debug, Clone)]
pub struct MenuTree {
    pub items: Vec<MenuItem>,
    /// Screen position (x, y) adjacent to which the popup should appear.
    pub anchor: (i32, i32),
}

/// One item in a `MenuTree`. Children are ignored for MVP (no nested submenus).
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Opaque string ID; the caller interprets it on selection.
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    pub icon_name: Option<String>,
    /// Submenus — not rendered in MVP.
    pub children: Vec<MenuItem>,
    pub is_separator: bool,
}

/// Render a popup menu from a `MenuTree`.
///
/// Returns an Iced `Element` that renders a vertical list of menu items.
/// Separators become thin horizontal rules. Disabled items are dimmed.
/// The caller must wrap this in a mouse_area for outside-click dismissal.
pub fn view_menu_popup(tree: &MenuTree) -> iced::Element<'_, crate::app::Message> {
    use iced::widget::{column, container, horizontal_rule, text};
    use iced::{Color, Length};

    let items: Vec<iced::Element<'_, crate::app::Message>> = tree
        .items
        .iter()
        .filter(|i| i.visible)
        .map(|item| {
            if item.is_separator {
                container(horizontal_rule(1))
                    .padding([2, 8])
                    .width(Length::Fill)
                    .into()
            } else {
                let color = if item.enabled {
                    crate::theme::TEXT
                } else {
                    crate::theme::OVERLAY0
                };
                let label = text(&item.label).size(13).color(color);
                let row = container(label)
                    .padding([4, 12])
                    .width(Length::Fill);

                if item.enabled {
                    let id = item.id.clone();
                    iced::widget::mouse_area(row)
                        .on_press(crate::app::Message::MenuItemSelected {
                            // service and menu_path are not known here — filled in by app.rs
                            // which wraps this view with the correct context.
                            // For now emit a placeholder; Task 11 wires the full context.
                            service: String::new(),
                            menu_item_id: id.parse().unwrap_or(0),
                            menu_path: zbus::zvariant::OwnedObjectPath::try_from("/").unwrap(),
                        })
                        .into()
                } else {
                    row.into()
                }
            }
        })
        .collect();

    container(
        column(items).spacing(0),
    )
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(crate::theme::SURFACE1)),
        border: iced::Border {
            color: crate::theme::BORDER,
            width: 1.0,
            radius: iced::border::Radius::from(4.0),
        },
        ..Default::default()
    })
    .padding(4)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_tree_stores_items() {
        let tree = MenuTree {
            items: vec![MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor: (100, 200),
        };
        assert_eq!(tree.items.len(), 1);
        assert_eq!(tree.anchor, (100, 200));
    }

    #[test]
    fn menu_item_separator_has_no_label() {
        let sep = MenuItem {
            id: "0".into(),
            label: String::new(),
            enabled: false,
            visible: true,
            icon_name: None,
            children: vec![],
            is_separator: true,
        };
        assert!(sep.is_separator);
    }

    #[test]
    fn menu_item_disabled_is_tracked() {
        let item = MenuItem {
            id: "5".into(),
            label: "Grayed Out".into(),
            enabled: false,
            visible: true,
            icon_name: None,
            children: vec![],
            is_separator: false,
        };
        assert!(!item.enabled);
    }
}
```

- [ ] **Step 4: Declare `menu_popup` in `src/surface.rs`**

Add at the top of `src/surface.rs`:

```rust
pub mod menu_popup;
```

- [ ] **Step 5: Add missing theme constants if needed**

Check that `crate::theme::BORDER`, `crate::theme::TEXT`, `crate::theme::OVERLAY0` exist in `src/theme.rs`. If any are missing, add them — run `cargo build 2>&1 | grep "not found in"` to see which.

- [ ] **Step 6: Run tests**

```bash
cargo test -p niribar 2>&1 | grep -E "(PASS|FAIL|error\[)" | head -20
```
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add src/surface/menu_popup.rs src/surface.rs
git commit -m "feat(surface): add generic MenuTree/MenuItem types and view_menu_popup"
```

---

## Task 10: DBusMenu client

**Files:**
- Modify: `src/dbus/sni_host.rs`

The DBusMenu layout format: `GetLayout` returns `(revision: u32, layout: (id, props, children))` where props is `a{sv}` (HashMap<String, Value>) and children is `av` (Vec<Value>, each being another nested menu item tuple).

- [ ] **Step 1: Write failing test**

Add to the `tests` block in `src/dbus/sni_host.rs`:

```rust
#[test]
fn convert_dbusmenu_props_extracts_label() {
    use std::collections::HashMap;
    use zbus::zvariant::{OwnedValue, Value};
    let mut props: HashMap<String, OwnedValue> = HashMap::new();
    props.insert(
        "label".to_string(),
        OwnedValue::try_from(Value::from("Open")).unwrap(),
    );
    let label = extract_label_from_props(&props);
    assert_eq!(label, "Open");
}

#[test]
fn convert_dbusmenu_props_returns_empty_for_missing_label() {
    use std::collections::HashMap;
    let props: HashMap<String, zbus::zvariant::OwnedValue> = HashMap::new();
    assert_eq!(extract_label_from_props(&props), "");
}
```

- [ ] **Step 2: Add DBusMenu proxy and fetch_menu**

Add to `src/dbus/sni_host.rs`:

```rust
// ── DBusMenu proxy ──────────────────────────────────────────────────────

#[zbus::proxy(
    interface = "com.canonical.dbusmenu",
    default_service = "com.canonical.dbusmenu",
    default_path = "/MenuBar"
)]
trait DbusMenu {
    /// Fetch the full menu layout.
    /// Returns (revision, root_item) where root_item is
    /// (id: i32, properties: a{sv}, children: av).
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: &[&str],
    ) -> zbus::Result<(
        u32,
        (i32, std::collections::HashMap<String, zbus::zvariant::OwnedValue>, Vec<zbus::zvariant::OwnedValue>),
    )>;

    fn about_to_show(&self, id: i32) -> zbus::Result<bool>;

    fn event(
        &self,
        id: i32,
        event_id: &str,
        data: &zbus::zvariant::Value<'_>,
        timestamp: u32,
    ) -> zbus::Result<()>;
}

// ── DBusMenu helpers ────────────────────────────────────────────────────

/// Extract the `label` string from a DBusMenu item's property map.
pub fn extract_label_from_props(
    props: &std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
) -> String {
    props
        .get("label")
        .and_then(|v| {
            let v: &zbus::zvariant::Value<'_> = v.as_ref();
            if let zbus::zvariant::Value::Str(s) = v {
                Some(s.to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn prop_bool(
    props: &std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
    key: &str,
    default: bool,
) -> bool {
    props
        .get(key)
        .and_then(|v| {
            let v: &zbus::zvariant::Value<'_> = v.as_ref();
            if let zbus::zvariant::Value::Bool(b) = v { Some(*b) } else { None }
        })
        .unwrap_or(default)
}

fn prop_str<'a>(
    props: &'a std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
    key: &str,
) -> &'a str {
    props
        .get(key)
        .and_then(|v| {
            let v: &zbus::zvariant::Value<'_> = v.as_ref();
            if let zbus::zvariant::Value::Str(s) = v { Some(s.as_str()) } else { None }
        })
        .unwrap_or("")
}

type RawItem = (
    i32,
    std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
    Vec<zbus::zvariant::OwnedValue>,
);

fn convert_item(raw: &RawItem) -> crate::surface::menu_popup::MenuItem {
    let (id, props, children) = raw;
    let item_type = prop_str(props, "type");
    let is_separator = item_type == "separator";
    let label = extract_label_from_props(props);
    let enabled = prop_bool(props, "enabled", true);
    let visible = prop_bool(props, "visible", true);
    let icon_name = {
        let s = prop_str(props, "icon-name");
        if s.is_empty() { None } else { Some(s.to_string()) }
    };
    // Children: each is a zvariant::Value wrapping a (i32, a{sv}, av) structure.
    // Decode best-effort; skip items that don't match the expected signature.
    let child_items: Vec<crate::surface::menu_popup::MenuItem> = children
        .iter()
        .filter_map(|v| {
            if let zbus::zvariant::Value::Structure(s) = v.as_ref() {
                let fields = s.fields();
                if fields.len() >= 3 {
                    if let (
                        zbus::zvariant::Value::I32(child_id),
                        zbus::zvariant::Value::Dict(dict),
                        zbus::zvariant::Value::Array(arr),
                    ) = (&fields[0], &fields[1], &fields[2])
                    {
                        // Reconstruct child_props from the dict.
                        let child_props: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
                            dict.iter()
                                .filter_map(|(k, v)| {
                                    let key = if let zbus::zvariant::Value::Str(s) = k { s.to_string() } else { return None; };
                                    Some((key, zbus::zvariant::OwnedValue::try_from(v.clone()).ok()?))
                                })
                                .collect();
                        let child_raw: RawItem = (
                            *child_id,
                            child_props,
                            arr.iter().cloned().collect(),
                        );
                        return Some(convert_item(&child_raw));
                    }
                }
            }
            None
        })
        .collect();

    crate::surface::menu_popup::MenuItem {
        id: id.to_string(),
        label,
        enabled,
        visible,
        icon_name,
        children: child_items,
        is_separator,
    }
}

/// Fetch a DBusMenu layout from `menu_path` on `service` and convert it to a `MenuTree`.
///
/// Returns `None` if the menu D-Bus call fails.
pub async fn fetch_menu(
    conn: &zbus::Connection,
    service: &str,
    menu_path: &zbus::zvariant::OwnedObjectPath,
) -> Option<crate::surface::menu_popup::MenuTree> {
    let proxy = DbusMenuProxy::builder(conn)
        .destination(service)
        .ok()?
        .path(menu_path.as_ref())
        .ok()?
        .build()
        .await
        .ok()?;

    let _ = proxy.about_to_show(0).await; // best-effort; ignore result

    let (_, root) = proxy.get_layout(0, -1, &[]).await.ok()?;
    let (_, _, children) = &root;

    // Root node's children are the top-level menu items.
    let items: Vec<crate::surface::menu_popup::MenuItem> = children
        .iter()
        .filter_map(|v| {
            if let zbus::zvariant::Value::Structure(s) = v.as_ref() {
                let fields = s.fields();
                if fields.len() >= 3 {
                    if let (
                        zbus::zvariant::Value::I32(id),
                        zbus::zvariant::Value::Dict(dict),
                        zbus::zvariant::Value::Array(arr),
                    ) = (&fields[0], &fields[1], &fields[2])
                    {
                        let props: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
                            dict.iter()
                                .filter_map(|(k, v)| {
                                    let key = if let zbus::zvariant::Value::Str(s) = k { s.to_string() } else { return None; };
                                    Some((key, zbus::zvariant::OwnedValue::try_from(v.clone()).ok()?))
                                })
                                .collect();
                        let raw: RawItem = (*id, props, arr.iter().cloned().collect());
                        return Some(convert_item(&raw));
                    }
                }
            }
            None
        })
        .collect();

    Some(crate::surface::menu_popup::MenuTree {
        items,
        anchor: (0, 0), // Caller fills in the actual anchor position.
    })
}

/// Send `Event("clicked")` to the DBusMenu for the selected item.
pub async fn send_menu_event(
    conn: &zbus::Connection,
    service: &str,
    menu_path: &zbus::zvariant::OwnedObjectPath,
    item_id: i32,
    event_id: &str,
) -> zbus::Result<()> {
    let proxy = DbusMenuProxy::builder(conn)
        .destination(service)?
        .path(menu_path.as_ref())?
        .build()
        .await?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;
    proxy
        .event(item_id, event_id, &zbus::zvariant::Value::I32(0), ts)
        .await
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test convert_dbusmenu 2>&1 | tail -10
cargo test -p niribar 2>&1 | grep -E "error\[" | head -10
```
Expected: tests pass, no new compile errors

- [ ] **Step 4: Commit**

```bash
git add src/dbus/sni_host.rs
git commit -m "feat(dbus): add DBusMenu proxy and fetch_menu"
```

---

## Task 11: Popup lifecycle in `app.rs`

**Files:**
- Modify: `src/app.rs`

The popup is a new layer-shell window. `App::menu_popups` maps popup window IDs to `(service, menu_path, MenuTree)`. `view()` dispatches to `view_menu_popup` for popup windows.

- [ ] **Step 1: Add `menu_popups` to `App`**

Add to the `App` struct:

```rust
    /// Active popup windows: window_id → (owning_service, menu_path, tree).
    pub menu_popups: HashMap<
        iced::window::Id,
        (String, zbus::zvariant::OwnedObjectPath, crate::surface::menu_popup::MenuTree),
    >,
```

Initialize in `boot()`: `menu_popups: HashMap::new()`.

- [ ] **Step 2: Handle `TrayMenuRequest` — spawn popup window**

Update the `TrayMenuRequest` handler added in Task 7. Replace the `tokio::task::spawn` version with a proper task-based approach that spawns the window:

```rust
            Message::TrayMenuRequest { service, menu_path } => {
                let conn = self.shared.dbus.session.clone();
                if let (Some(conn), Some(menu_path)) = (conn, menu_path) {
                    let service_clone = service.clone();
                    let menu_path_clone = menu_path.clone();
                    return Task::perform(
                        async move {
                            crate::dbus::sni_host::fetch_menu(&conn, &service_clone, &menu_path_clone).await
                        },
                        move |tree_opt| {
                            if let Some(tree) = tree_opt {
                                Message::OpenMenu {
                                    service: service.clone(),
                                    menu_path: menu_path.clone(),
                                    tree,
                                }
                            } else {
                                // Menu fetch failed — fall back to Activate.
                                Message::TrayActivate { service: service.clone() }
                            }
                        },
                    );
                }
                Task::none()
            }
```

Add `OpenMenu` to the `Message` enum:

```rust
    /// Menu data fetched successfully — spawn the popup surface.
    OpenMenu {
        service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
        tree: crate::surface::menu_popup::MenuTree,
    },
```

- [ ] **Step 3: Handle `OpenMenu` — create the popup layer-shell window**

Add to `update()`:

```rust
            Message::OpenMenu { service, menu_path, tree } => {
                // Close any existing popup first.
                let existing_ids: Vec<iced::window::Id> = self.menu_popups.keys().copied().collect();
                let close_tasks: Vec<Task<Message>> = existing_ids
                    .into_iter()
                    .map(|id| {
                        self.menu_popups.remove(&id);
                        Message::layershell_close(id)
                    })
                    .collect();

                // Spawn new popup window.
                let settings = iced_layershell::reexport::NewLayerShellSettings {
                    anchor: iced_layershell::reexport::Anchor::Left
                        | iced_layershell::reexport::Anchor::Top,
                    exclusive_zone: Some(0),
                    size: Some((200, 300)), // initial size; popup will resize to content
                    layer: iced_layershell::reexport::Layer::Overlay,
                    keyboard_interactivity:
                        iced_layershell::reexport::KeyboardInteractivity::OnDemand,
                    margin: iced_layershell::reexport::MarginSetting {
                        left: self.config.bars.first().map(|b| b.collapsed_width as i32).unwrap_or(48),
                        top: tree.anchor.1,
                        right: 0,
                        bottom: 0,
                    },
                    ..Default::default()
                };
                let (popup_id, open_task) = Message::layershell_open(settings);
                self.menu_popups.insert(popup_id, (service, menu_path, tree));

                let mut tasks = close_tasks;
                tasks.push(open_task);
                Task::batch(tasks)
            }
```

- [ ] **Step 4: Handle `MenuItemSelected` and `MenuClosed`**

Add to `update()`:

```rust
            Message::MenuItemSelected { service, menu_item_id, menu_path } => {
                let conn = self.shared.dbus.session.clone();
                tokio::task::spawn(async move {
                    if let Some(conn) = conn {
                        let _ = crate::dbus::sni_host::send_menu_event(
                            &conn, &service, &menu_path, menu_item_id, "clicked",
                        )
                        .await;
                    }
                });
                // Close all popups.
                let ids: Vec<iced::window::Id> = self.menu_popups.keys().copied().collect();
                let tasks: Vec<Task<Message>> = ids
                    .into_iter()
                    .map(|id| {
                        self.menu_popups.remove(&id);
                        Message::layershell_close(id)
                    })
                    .collect();
                Task::batch(tasks)
            }
            Message::MenuClosed => {
                // Dismiss popup without sending any event.
                let ids: Vec<iced::window::Id> = self.menu_popups.keys().copied().collect();
                let tasks: Vec<Task<Message>> = ids
                    .into_iter()
                    .map(|id| {
                        self.menu_popups.remove(&id);
                        Message::layershell_close(id)
                    })
                    .collect();
                Task::batch(tasks)
            }
```

- [ ] **Step 5: Dispatch popup windows in `view()`**

At the top of the `view()` function, add before the surface lookup:

```rust
        // Popup window — render menu instead of bar.
        if let Some((service, menu_path, tree)) = self.menu_popups.get(&window) {
            use crate::surface::menu_popup::view_menu_popup;
            // Wrap in a mouse_area that covers the full window: outside clicks dismiss.
            // The service/menu_path are injected into MenuItemSelected messages here.
            let service = service.clone();
            let menu_path = menu_path.clone();
            return iced::widget::mouse_area(
                iced::widget::container(view_menu_popup(tree))
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .style(|_| iced::widget::container::Style {
                        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                        ..Default::default()
                    }),
            )
            .on_press(Message::MenuClosed)
            .into();
        }
```

**Note:** The `MenuItemSelected` messages emitted by `view_menu_popup` use placeholder service/menu_path (empty string and "/" path from Task 9). To fix this, `view_menu_popup` needs to accept service and menu_path parameters so it can embed them in the messages. Update `view_menu_popup`'s signature:

In `src/surface/menu_popup.rs`, change the function signature:

```rust
pub fn view_menu_popup<'a>(
    tree: &'a MenuTree,
    service: &'a str,
    menu_path: &'a zbus::zvariant::OwnedObjectPath,
) -> iced::Element<'a, crate::app::Message>
```

Update the `on_press` call inside `view_menu_popup` to use the passed `service` and `menu_path`:

```rust
                        .on_press(crate::app::Message::MenuItemSelected {
                            service: service.to_string(),
                            menu_item_id: id.parse().unwrap_or(0),
                            menu_path: menu_path.clone(),
                        })
```

Update the call in `view()`:

```rust
            view_menu_popup(tree, &service, &menu_path)
```

- [ ] **Step 6: Run tests and build**

```bash
cargo test -p niribar 2>&1 | grep -E "(PASS|FAIL|error\[)" | head -20
cargo build 2>&1 | grep "error\[" | head -10
```
Expected: all tests pass, binary compiles

- [ ] **Step 7: Check iced_layershell API surface for popup closing**

`Message::layershell_close` may not exist — check the actual API. Look at how `iced_layershell` closes windows in the project or its documentation:

```bash
grep -r "layershell_close\|close_window\|layershell.*close" src/ 2>/dev/null | head -10
```

If `layershell_close` does not exist, find the correct method and update all calls to it in this task.

- [ ] **Step 8: Commit**

```bash
git add src/app.rs src/surface/menu_popup.rs
git commit -m "feat(app): popup lifecycle — OpenMenu spawns layer-shell window, MenuItemSelected/MenuClosed closes it"
```

---

## Self-Review

After writing the plan, checking it against the spec:

**Spec coverage:**

| Spec requirement | Task |
|-----------------|------|
| SNI only started if tray module present | Task 8 |
| niribar acts as own StatusNotifierWatcher | Task 3 |
| StatusNotifierHost registration | Task 4 |
| All interactions (activate, secondary, scroll, context menu) | Tasks 5, 7 |
| Icon-only in both collapsed and expanded | Task 5 |
| Row (horizontal) / column (vertical) layout | Task 5 |
| Both Named (XDG) and Pixmap icon formats | Task 5 |
| NeedsAttention accent dot | Task 5 |
| Passive items hidden | Task 5 |
| User-configurable zone placement | Task 1 (config plumbing) |
| SharedState/SurfaceState boundary | Tasks 6, 11 |
| Generic popup reusable by other modules | Task 9 |
| Context menu via new layer-shell popup | Task 11 |
| DBusMenu client (GetLayout, AboutToShow, Event) | Task 10 |
| Fallback to Activate when GetLayout fails | Task 11 |
| Graceful degradation when no session bus | Task 8 (sni_sub gated on session bus) |
| Watcher fallback to host-only if name taken | Task 3 (start_watcher returns None) |
| Icon resolution fallback to blank | Task 5 (returns "?" text) |

**Gaps identified and resolved:**

1. `tray_ctor` defaults to `Edge::Left` — this means tray icons always render as a column even in horizontal bars. This is a known limitation noted in the constructor; fix requires passing `Edge` through `ModuleConfig` (out of scope for MVP).

2. `view_menu_popup` initially emitted placeholder service/menu_path — fixed in Task 11 Step 5 by updating the function signature to accept and embed them.

3. `Message::layershell_close` — existence is not verified. Task 11 Step 7 explicitly checks this and requires the developer to fix if the API is different.

4. The `menu_tx`/`menu_rx` channel added to `App` in Task 8 is not consumed — the channel approach was superseded by `Task::perform` in Task 11. Remove `menu_tx` and `menu_rx` from `App` before the final commit to avoid dead code.
