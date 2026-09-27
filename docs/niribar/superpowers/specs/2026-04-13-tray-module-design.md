# Tray Module Design

**Date:** 2026-04-13
**Author:** Kevin

## Summary

Add a `tray` module that renders StatusNotifierItem (SNI) tray icons in the bar. niribar acts as its own StatusNotifierWatcher and StatusNotifierHost. All SNI and DBusMenu D-Bus work is isolated to the `dbus/` layer; rendering lives in `modules/tray.rs`; the context menu popup is a generic reusable surface in `surface/menu_popup.rs`. The SharedState/SurfaceState boundary is strictly observed throughout.

The entire SNI D-Bus system (watcher + host) is only started if at least one bar section in the parsed config contains a `tray` module.

---

## New Files

```
crates/niribar/src/dbus/sni_watcher.rs
src/dbus/sni_host.rs
crates/niribar/src/modules/tray.rs
crates/niribar/src/surface/menu_popup.rs
```

---

## Section 1: Architecture Overview

Four layers, one file each:

| File | Role |
|------|------|
| `crates/niribar/src/dbus/sni_watcher.rs` | Registers `org.kde.StatusNotifierWatcher` on the session bus. Accepts `RegisterStatusNotifierItem` calls from apps. Emits `ItemRegistered` / `ItemUnregistered` signals. |
| `src/dbus/sni_host.rs` | Registers as `org.kde.StatusNotifierHost`. Subscribes to watcher signals. Proxies each live item to fetch icon/tooltip/status/menu path. Maintains `Vec<TrayItem>`. Exposes an `iced::Subscription`. |
| `crates/niribar/src/modules/tray.rs` | Implements `Module` trait. Renders icon row or column depending on bar edge. Routes pointer and scroll events to host. Emits `OpenMenu(MenuTree)` for right-clicks. |
| `crates/niribar/src/surface/menu_popup.rs` | Generic layer-shell popup surface. Accepts a `MenuTree`. Emits `MenuItemSelected(id)` or `MenuClosed`. No knowledge of SNI or DBusMenu. |

**Startup sequence:**
1. Config fully parsed (AR7) — check if any bar section contains `module "tray"`
2. If yes: start watcher (registers `org.kde.StatusNotifierWatcher`), start host (registers as host, begins listening)
3. Apps register items -> host fetches properties -> subscription emits `TrayUpdated(Vec<TrayItem>)`
4. Right-click on icon -> tray module emits `OpenMenu(MenuTree)` -> app spawns popup SurfaceState
5. Menu item selected -> popup emits `MenuItemSelected(id)` -> app routes to sni_host -> D-Bus `Event("clicked")` call -> popup closes

---

## Section 2: D-Bus Layer

### Shared types (defined in `sni_host.rs`, public)

```rust
pub struct TrayItem {
    pub service: String,
    pub object_path: OwnedObjectPath,
    pub icon: TrayIcon,
    pub tooltip: Option<String>,
    pub status: ItemStatus,
    pub menu_path: Option<OwnedObjectPath>,
}

pub enum TrayIcon {
    Named(String),                        // XDG icon theme name
    Pixmap(Vec<(i32, i32, Vec<u8>)>),     // list of (width, height, ARGB data)
}

pub enum ItemStatus {
    Active,
    Passive,
    NeedsAttention,
}
```

### `sni_watcher.rs`

Implements `org.kde.StatusNotifierWatcher` via zbus:

- **Methods:** `RegisterStatusNotifierItem(service: String)`, `RegisterStatusNotifierHost(service: String)`
- **Properties:** `IsStatusNotifierHostRegistered`, `ProtocolVersion` (always 0), `RegisteredStatusNotifierItems`
- **Signals:** `StatusNotifierItemRegistered(String)`, `StatusNotifierItemUnregistered(String)`, `StatusNotifierHostRegistered`
- Watches registered item D-Bus names; removes items when their owner disappears

### `sni_host.rs`

- Registers as `org.kde.StatusNotifierHost` with the watcher
- Subscribes to watcher `StatusNotifierItemRegistered` / `StatusNotifierItemUnregistered` signals
- For each new item: creates a zbus proxy, fetches `IconName`, `IconPixmap`, `ToolTip`, `Title`, `Status`, `Menu`
- Subscribes to item signals: `NewIcon`, `NewStatus`, `NewToolTip`
- Icon resolution: prefers `IconName` if non-empty -> `TrayIcon::Named`; falls back to `IconPixmap` -> `TrayIcon::Pixmap`
- Exposes `iced::Subscription` that emits `Vec<TrayItem>` on any change

---

## Section 3: Tray Module (`crates/niribar/src/modules/tray.rs`)

Implements `Module` trait.

**Layout:**
- `bar.edge` is `left` or `right` -> icons in a `Column`
- `bar.edge` is `top` or `bottom` -> icons in a `Row`
- `Edge` is passed to `TrayModule` at construction time from parsed config context
- Both `view_collapsed` and `view_expanded` render the same icon layout (icons only, no labels)
- `Passive` items hidden entirely — no gap, no placeholder (UX-DR16)

**Icon rendering:**
- `TrayIcon::Named(name)` — resolve path via XDG icon theme when `TrayItem` is received or updated, render as `svg` or `image`
- `TrayIcon::Pixmap(pixmaps)` — pick size closest to configured icon size, convert ARGB data to `iced::widget::image::Handle::from_rgba`, render as `image`

**`NeedsAttention` state:** small accent-colored dot overlaid on icon, consistent with `CriticalBadge` pattern (UX-DR7)

**Interactions** — each icon in a `mouse_area`:
| Interaction | Message emitted |
|-------------|----------------|
| Left click | `TrayActivate { service, x, y }` |
| Middle click | `TraySecondaryActivate { service, x, y }` |
| Right click | `OpenMenu(MenuTree)` |
| Scroll | `TrayScroll { service, delta, orientation }` |

**Config:**
```kdl
module "tray" {}
```

No parameters for MVP. Zone placement is controlled by position within the bar section config, identical to all other modules.

---

## Section 4: Generic Menu Popup (`crates/niribar/src/surface/menu_popup.rs`)

A reusable layer-shell popup surface. No SNI or DBusMenu knowledge.

### Generic types (public)

```rust
pub struct MenuTree {
    pub items: Vec<MenuItem>,
    pub anchor: (i32, i32),   // screen position — popup placed adjacent to this point
}

pub struct MenuItem {
    pub id: String,            // opaque — caller interprets on selection
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    pub icon_name: Option<String>,
    pub children: Vec<MenuItem>,
    pub is_separator: bool,
}
```

### Popup surface

- Receives `MenuTree` on spawn; stores it as `SurfaceState`
- Renders flat vertical list; separators as thin horizontal rules; disabled items in `color.overlay0`
- Submenus: no nesting for MVP — children of items ignored at this stage
- Positioned adjacent to `anchor` point
- Only one popup open at a time — opening a second closes the first

**Lifecycle messages emitted:**
- `MenuItemSelected(id)` — user selected an item; caller maps `id` to its action
- `MenuClosed` — dismissed without selection (Escape, outside click)

### Tray module's use of the popup

- On right-click: `sni_host` fetches DBusMenu layout via `GetLayout`, calls `AboutToShow`, converts to `MenuTree` with item IDs encoding DBusMenu integer IDs; tray module emits `OpenMenu(MenuTree)`
- `sni_host` subscribes to `LayoutUpdated` / `ItemsPropertiesUpdated` on the item while the popup is open; if layout changes, a new `OpenMenu(MenuTree)` replaces the current popup
- On `MenuItemSelected(id)`: `app.rs` routes to `sni_host`, which looks up `TrayItem.menu_path` by service name and calls `Event(id, "clicked", ...)` on that path
- On `MenuClosed`: `app.rs` routes to `sni_host`, which calls `Event(root_id, "closed", ...)`

### SharedState / SurfaceState boundary

- Tray module (SharedState) emits `OpenMenu(MenuTree)` and stops — no reference to popup surface
- `app.rs` receives message, creates popup `SurfaceState` with the `MenuTree`
- Popup `SurfaceState` holds its own copy of `MenuTree`; never reaches into SharedState
- `MenuItemSelected(id)` flows through `app.rs` back to sni_host (SharedState) for the D-Bus call
- No module holds a reference to any surface; no surface holds a reference to any module

---

## Section 5: Config, Message Routing, and Initialization

### Conditional initialization

After config is fully parsed, `app.rs` checks whether any bar section contains `module "tray"`. If yes: watcher started, host started, host subscription wired up. If no: nothing registered, no D-Bus names claimed.

### New messages

```
TrayUpdated(Vec<TrayItem>)              SharedState update from sni_host subscription
TrayActivate { service, x, y }          module -> sni_host calls Activate(x, y)
TraySecondaryActivate { service, x, y } module -> sni_host calls SecondaryActivate(x, y)
TrayScroll { service, delta, orientation } module -> sni_host calls Scroll(delta, orientation)
OpenMenu(MenuTree)                       module -> app spawns popup SurfaceState
MenuItemSelected(id)                     popup -> app -> sni_host calls Event("clicked")
MenuClosed                               popup -> app tears down popup SurfaceState
```

### Edge orientation

`TrayModule` receives `Edge` at construction from its parsed config context. No runtime querying of surface state.

---

## Section 6: Error Handling and Degradation

| Failure | Behavior |
|---------|----------|
| Another watcher already holds `org.kde.StatusNotifierWatcher` | Log conflict; fall back to host-only mode connecting to existing watcher. Tray still functions. |
| Item proxy fails (app registers then disappears) | Drop item silently. D-Bus name watch detects disappearance; item removed. |
| Named icon not found in theme; pixmap data malformed | Fall back to `application-x-executable`. If that also fails, render blank placeholder at icon size — layout does not shift. |
| DBusMenu `GetLayout` fails | Do not spawn popup. Right-click falls back to calling `Activate` instead. |
| Watcher service disappears at runtime | Clear `Vec<TrayItem>`. Tray renders empty. Attempt re-register on next D-Bus reconnect. |
| No D-Bus session bus | Tray module renders nothing. No crash. Follows FR21 graceful degradation. |
