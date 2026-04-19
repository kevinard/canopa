# Code Quality Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve niribar code quality by reducing duplication, tightening performance-sensitive paths, and hardening security-sensitive boundaries without changing user-visible behavior.

**Architecture:** Keep the current `App`/Iced architecture, but move domain-specific reducers out of the central update match, replace stringly typed module checks with typed module kinds, add a module store for indexed lookup, and centralize repeated dynamic-module and command-execution behavior. Each task is independently testable and should preserve all existing tests before moving on.

**Tech Stack:** Rust 2024, Iced 0.14, iced_layershell, Tokio, zbus, niri-ipc, libpulse-binding, KDL config parser.

---

## File Structure

- Modify: `src/app.rs`
  - Keep `Message`, `App`, `SharedState`, boot wiring, and the top-level `update` router.
  - Remove tray/menu implementation details as they move into focused modules.
- Create: `src/app/menu.rs`
  - Own popup lifecycle, popup closing, keyboard interactivity restoration, and menu layout refresh handling.
- Create: `src/app/tray.rs`
  - Own tray events, tray activation, tray menu request routing, and tray icon insertion/removal.
- Modify: `src/app/mod declarations in src/app.rs`
  - Add `mod menu;` and `mod tray;`.
- Modify: `src/modules.rs`
  - Add `ModuleKind`.
  - Change `Module::module_type()` usage path toward `Module::kind()`.
  - Keep `module_type()` temporarily if useful for incremental compatibility.
- Modify: `src/config.rs`
  - Add typed parsing/printing bridge for module types.
  - Use a single known module type list derived from `ModuleKind`.
- Modify: `src/config/parse.rs`
  - Replace repeated child-entry parsing with typed helpers.
- Create: `src/app/module_store.rs`
  - Wrap ordered modules and maintain indexes for id, kind, and config placeholder.
- Modify: `src/app/layout.rs`, `src/app/module_state.rs`, `src/app/module_events.rs`, `src/app/niri.rs`, `src/app/subscriptions.rs`, `src/app/view.rs`, `src/app/surfaces.rs`, `src/app/focus.rs`
  - Replace repeated scans with `ModuleStore` methods.
- Create: `src/app/dynamic_modules.rs`
  - Centralize placeholder expansion for tray icons and workspace items.
- Create: `src/command.rs`
  - Centralize shell command execution, timeout, output limits, and fire-and-forget process handling.
- Modify: `src/modules/script.rs`, `src/modules/button.rs`, `src/app/module_events.rs`
  - Route process execution through `src/command.rs`.
- Modify: `src/dbus/sni_host.rs`
  - Add SNI pixmap/menu bounds and use small helpers for repeated proxy/event logic.
- Modify: `src/dbus/wireplumber.rs`
  - Isolate the unsafe PulseAudio mainloop callback into a tiny helper.

---

## Task 1: Restore Clean Quality Gate

**Files:**
- Modify: `src/app.rs`
- Modify: `src/dbus/sni_host.rs`

- [ ] **Step 1: Run current checks and record baseline**

Run:

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected:

```text
cargo test: 474 passed
cargo clippy: fails with needless_borrow in src/app.rs and collapsible_if in src/dbus/sni_host.rs
```

- [ ] **Step 2: Fix needless borrow in tray insertion**

In `src/app.rs`, change the call around the `TrayEvent::ItemAdded` handling from:

```rust
let insert_at = self.tray_insert_position(*slot_idx, &config_name);
```

to:

```rust
let insert_at = self.tray_insert_position(*slot_idx, config_name);
```

- [ ] **Step 3: Collapse repeated nested removal/send checks**

In `src/dbus/sni_host.rs`, replace each nested pattern:

```rust
if items.remove(&service_str).is_some() {
    if tx.send(TrayEvent::ItemRemoved(service_str.clone())).is_err() { break; }
}
```

with:

```rust
if items.remove(&service_str).is_some()
    && tx
        .send(TrayEvent::ItemRemoved(service_str.clone()))
        .is_err()
{
    break;
}
```

For the owned `service_str` and `gone` variants, use:

```rust
if items.remove(&service_str).is_some()
    && tx.send(TrayEvent::ItemRemoved(service_str)).is_err()
{
    break;
}
```

```rust
if items.remove(&gone).is_some() && tx.send(TrayEvent::ItemRemoved(gone)).is_err() {
    break;
}
```

- [ ] **Step 4: Verify clean gate**

Run:

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected:

```text
cargo test: all tests pass
cargo clippy: no warnings or errors
```

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/dbus/sni_host.rs
git commit -m "chore: restore clean clippy gate"
```

---

## Task 2: Extract Menu Popup Reducer

**Files:**
- Create: `src/app/menu.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Add focused tests for popup close helper**

In `src/app.rs` test module, add tests before existing menu tests:

```rust
#[test]
fn close_all_menu_popups_removes_popup_and_focus_maps() {
    let (mut app, _) = App::boot(make_config(), vec![]);
    let wid = make_popup_wid(&mut app);
    app.menu_popup_hovered.insert(wid, "1".into());
    app.menu_focused_item.insert(wid, "1".into());

    let tasks = app.close_all_menu_popups();

    assert!(app.menu_popups.is_empty());
    assert!(app.menu_popup_hovered.is_empty());
    assert!(app.menu_focused_item.is_empty());
    assert_eq!(tasks.len(), 1);
}
```

Run:

```bash
rtk cargo test close_all_menu_popups_removes_popup_and_focus_maps
```

Expected: fail because `close_all_menu_popups` does not exist.

- [ ] **Step 2: Create `src/app/menu.rs`**

Add:

```rust
use iced::Task;
use iced_layershell::reexport::KeyboardInteractivity;

use super::{App, Message};

impl App {
    pub(super) fn close_all_menu_popups(&mut self) -> Vec<Task<Message>> {
        let ids: Vec<iced::window::Id> = self.menu_popups.keys().copied().collect();
        ids.into_iter()
            .map(|id| {
                self.menu_popups.remove(&id);
                self.menu_popup_hovered.remove(&id);
                self.menu_focused_item.remove(&id);
                Task::done(Message::RemoveWindow(id))
            })
            .collect()
    }

    pub(super) fn restore_bar_keyboard_interactivity(&mut self) -> Vec<Task<Message>> {
        let ids: Vec<iced::window::Id> = self.surfaces.keys().copied().collect();
        for surface in self.surfaces.values_mut() {
            surface.focused_module = None;
            surface.module_activated = false;
        }
        ids.into_iter()
            .map(|id| {
                Task::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::OnDemand,
                })
            })
            .collect()
    }

    pub(super) fn handle_menu_item_selected(
        &mut self,
        service: String,
        menu_item_id: i32,
        menu_path: zbus::zvariant::OwnedObjectPath,
    ) -> Task<Message> {
        let conn = self.shared.dbus.session.clone();
        tokio::task::spawn(async move {
            if let Some(conn) = conn {
                let _ = crate::dbus::sni_host::send_menu_event(
                    &conn,
                    &service,
                    &menu_path,
                    menu_item_id,
                    "clicked",
                )
                .await;
            }
        });

        let mut tasks = self.close_all_menu_popups();
        tasks.extend(self.restore_bar_keyboard_interactivity());
        Task::batch(tasks)
    }

    pub(super) fn handle_menu_closed(&mut self) -> Task<Message> {
        let conn = self.shared.dbus.session.clone();
        for (service, menu_path, _, _) in self.menu_popups.values() {
            if let Some(conn) = conn.clone() {
                let service = service.clone();
                let menu_path = menu_path.clone();
                tokio::task::spawn(async move {
                    let _ = crate::dbus::sni_host::send_menu_event(
                        &conn, &service, &menu_path, 0, "closed",
                    )
                    .await;
                });
            }
        }

        let mut tasks = self.close_all_menu_popups();
        tasks.extend(self.restore_bar_keyboard_interactivity());
        Task::batch(tasks)
    }
}
```

- [ ] **Step 3: Register module**

At the top of `src/app.rs`, add:

```rust
mod menu;
```

- [ ] **Step 4: Route menu messages through helper methods**

In `App::update`, replace the body of `Message::MenuItemSelected { ... }` with:

```rust
self.handle_menu_item_selected(service, menu_item_id, menu_path)
```

Replace the body of `Message::MenuClosed` with:

```rust
self.handle_menu_closed()
```

- [ ] **Step 5: Verify**

Run:

```bash
rtk cargo test menu_
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/app/menu.rs
git commit -m "refactor: extract menu popup reducer"
```

---

## Task 3: Extract Tray Reducer

**Files:**
- Create: `src/app/tray.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Add tests for tray snapshot and duplicate add behavior**

In `src/app.rs` test module, add:

```rust
#[test]
fn tray_snapshot_removes_existing_tray_icons_before_insert() {
    use crate::dbus::sni_host::{ItemStatus, TrayIcon, TrayItem};
    use crate::modules::tray::TrayIconModule;
    use zbus::zvariant::OwnedObjectPath;

    let mut config = make_config();
    config.bars[0].modules.bottom = vec!["tray".into()];
    config.modules = vec![ModuleConfig {
        name: "tray".into(),
        module_type: "tray".into(),
        params: ModuleParams::Tray(crate::config::TrayConfig),
    }];
    let (mut app, _) = App::boot(config, vec![]);

    let old_item = TrayItem {
        service: "org.old".into(),
        object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
        icon: TrayIcon::Named("old".into()),
        tooltip: None,
        status: ItemStatus::Active,
        menu_path: None,
    };
    app.modules.push((
        Zone::Bottom,
        Box::new(TrayIconModule::new(old_item, "tray".into())),
    ));

    let new_item = TrayItem {
        service: "org.new".into(),
        object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
        icon: TrayIcon::Named("new".into()),
        tooltip: None,
        status: ItemStatus::Active,
        menu_path: None,
    };

    let _ = app.handle_tray_event(crate::dbus::sni_host::TrayEvent::Snapshot(vec![new_item]));

    assert!(app.modules.iter().any(|(_, m)| m.id().0 == "tray:org.new"));
    assert!(!app.modules.iter().any(|(_, m)| m.id().0 == "tray:org.old"));
}
```

Run:

```bash
rtk cargo test tray_snapshot_removes_existing_tray_icons_before_insert
```

Expected: fail because `handle_tray_event` is not visible yet.

- [ ] **Step 2: Create `src/app/tray.rs`**

Move the full `Message::TrayEvent(event)` match body from `src/app.rs` into:

```rust
use iced::Task;

use super::{App, Message};

impl App {
    pub(super) fn handle_tray_event(
        &mut self,
        event: crate::dbus::sni_host::TrayEvent,
    ) -> Task<Message> {
        use crate::dbus::sni_host::{ItemStatus, TrayEvent};
        use crate::modules::tray::TrayIconModule;

        if let Some(perf) = &self.perf {
            perf.record_tray_event();
        }

        match event {
            TrayEvent::Snapshot(items) => {
                let tray_slots: Vec<(usize, crate::modules::Zone, String)> = self
                    .modules
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, m))| m.module_type() == "tray")
                    .map(|(i, (z, m))| (i, z.clone(), m.id().0.clone()))
                    .collect();

                self.modules.retain(|(_, m)| m.module_type() != "tray_icon");
                self.clear_surface_module_refs_with_prefix("tray:");

                let active_items: Vec<&crate::dbus::sni_host::TrayItem> = items
                    .iter()
                    .filter(|i| i.status != ItemStatus::Passive)
                    .collect();

                for (slot_idx, zone, config_name) in tray_slots.iter().rev() {
                    let insert_at = slot_idx + 1;
                    for item in active_items.iter().rev() {
                        let icon_module = TrayIconModule::new((*item).clone(), config_name.clone());
                        self.modules
                            .insert(insert_at, (zone.clone(), Box::new(icon_module)));
                    }
                }
            }
            TrayEvent::ItemAdded(item) => {
                let service_id = format!("tray:{}", item.service);
                self.remove_tray_icon(&service_id);

                if item.status != ItemStatus::Passive {
                    let tray_slots: Vec<(usize, crate::modules::Zone, String)> = self
                        .modules
                        .iter()
                        .enumerate()
                        .filter(|(_, (_, m))| m.module_type() == "tray")
                        .map(|(i, (z, m))| (i, z.clone(), m.id().0.clone()))
                        .collect();

                    for (slot_idx, zone, config_name) in tray_slots.iter().rev() {
                        let insert_at = self.tray_insert_position(*slot_idx, config_name);
                        let icon_module = TrayIconModule::new(item.clone(), config_name.clone());
                        self.modules
                            .insert(insert_at, (zone.clone(), Box::new(icon_module)));
                    }
                }
            }
            TrayEvent::ItemRemoved(service) => {
                let service_id = format!("tray:{service}");
                self.remove_tray_icon(&service_id);
            }
        }

        Task::none()
    }

    pub(super) fn clear_surface_module_refs_with_prefix(&mut self, prefix: &str) {
        for surface in self.surfaces.values_mut() {
            if surface
                .focused_module
                .as_ref()
                .map(|id| id.0.starts_with(prefix))
                .unwrap_or(false)
            {
                surface.focused_module = None;
            }
            if surface
                .hovered_module
                .as_ref()
                .map(|id| id.0.starts_with(prefix))
                .unwrap_or(false)
            {
                surface.hovered_module = None;
            }
        }
    }
}
```

- [ ] **Step 3: Register module and route update**

At the top of `src/app.rs`, add:

```rust
mod tray;
```

Replace `Message::TrayEvent(event) => { ... }` with:

```rust
Message::TrayEvent(event) => self.handle_tray_event(event),
```

- [ ] **Step 4: Verify**

Run:

```bash
rtk cargo test tray_
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/app/tray.rs
git commit -m "refactor: extract tray reducer"
```

---

## Task 4: Introduce `ModuleKind`

**Files:**
- Modify: `src/modules.rs`
- Modify: `src/config.rs`
- Modify: `src/config/parse.rs`
- Modify: all module impl files under `src/modules/*.rs`
- Modify: app files that compare `module_type()`

- [ ] **Step 1: Add `ModuleKind` tests**

In `src/modules.rs` tests, add:

```rust
#[test]
fn module_kind_round_trips_known_config_names() {
    for (kind, name) in [
        (ModuleKind::Battery, "battery"),
        (ModuleKind::Clock, "clock"),
        (ModuleKind::Network, "network"),
        (ModuleKind::Volume, "volume"),
        (ModuleKind::Workspaces, "workspaces"),
        (ModuleKind::FocusedWindow, "focused-window"),
        (ModuleKind::Script, "script"),
        (ModuleKind::Button, "button"),
        (ModuleKind::Tray, "tray"),
    ] {
        assert_eq!(kind.as_str(), name);
        assert_eq!(ModuleKind::from_config_name(name), Some(kind));
    }
}
```

Run:

```bash
rtk cargo test module_kind_round_trips_known_config_names
```

Expected: fail because `ModuleKind` does not exist.

- [ ] **Step 2: Add enum and bridge methods**

In `src/modules.rs`, near `Zone`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleKind {
    Battery,
    Button,
    Network,
    Volume,
    Clock,
    Script,
    Workspaces,
    WorkspaceItem,
    FocusedWindow,
    Tray,
    TrayIcon,
}

impl ModuleKind {
    pub const CONFIG_KINDS: &'static [ModuleKind] = &[
        ModuleKind::Battery,
        ModuleKind::Clock,
        ModuleKind::Network,
        ModuleKind::Volume,
        ModuleKind::Workspaces,
        ModuleKind::FocusedWindow,
        ModuleKind::Script,
        ModuleKind::Button,
        ModuleKind::Tray,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ModuleKind::Battery => "battery",
            ModuleKind::Button => "button",
            ModuleKind::Network => "network",
            ModuleKind::Volume => "volume",
            ModuleKind::Clock => "clock",
            ModuleKind::Script => "script",
            ModuleKind::Workspaces => "workspaces",
            ModuleKind::WorkspaceItem => "workspace_item",
            ModuleKind::FocusedWindow => "focused-window",
            ModuleKind::Tray => "tray",
            ModuleKind::TrayIcon => "tray_icon",
        }
    }

    pub fn from_config_name(s: &str) -> Option<Self> {
        Self::CONFIG_KINDS
            .iter()
            .copied()
            .find(|kind| kind.as_str() == s)
    }
}
```

- [ ] **Step 3: Extend trait**

In `src/modules.rs`, change the trait to:

```rust
fn kind(&self) -> ModuleKind;

fn module_type(&self) -> &str {
    self.kind().as_str()
}
```

- [ ] **Step 4: Implement `kind()` in each module**

Example replacements:

```rust
fn kind(&self) -> crate::modules::ModuleKind {
    crate::modules::ModuleKind::Clock
}
```

Use the matching kind for each module:

```text
BatteryModule -> Battery
ButtonModule -> Button
NetworkModule -> Network
VolumeModule -> Volume
ClockModule -> Clock
ScriptModule -> Script
WorkspacesModule -> Workspaces
WorkspaceItemModule -> WorkspaceItem
FocusedWindowModule -> FocusedWindow
TrayModule -> Tray
TrayIconModule -> TrayIcon
```

Keep existing `module_type()` methods only until compile errors are resolved, then remove module-local implementations so the default trait method is used.

- [ ] **Step 5: Replace runtime string checks**

Replace patterns like:

```rust
module.module_type() == "battery"
matches!(module.module_type(), "tray" | "tray_icon")
```

with:

```rust
module.kind() == ModuleKind::Battery
matches!(module.kind(), ModuleKind::Tray | ModuleKind::TrayIcon)
```

- [ ] **Step 6: Derive known module list from `ModuleKind`**

In `src/config.rs`, replace hardcoded `KNOWN_MODULE_TYPES` contents with:

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

Keep the constant initially for low risk. Add a follow-up test that every `ModuleKind::CONFIG_KINDS` entry appears in it.

- [ ] **Step 7: Verify**

Run:

```bash
rtk cargo test module_kind
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/modules.rs src/config.rs src/config/parse.rs src/app.rs src/app/*.rs src/modules/*.rs
git commit -m "refactor: add typed module kind"
```

---

## Task 5: Add `ModuleStore`

**Files:**
- Create: `src/app/module_store.rs`
- Modify: `src/app.rs`
- Modify: `src/app/*.rs`

- [ ] **Step 1: Add store tests**

Create `src/app/module_store.rs` with a test-only stub section first:

```rust
#[cfg(test)]
mod tests {
    use crate::modules::{ModuleId, ModuleKind, Zone};

    #[test]
    fn module_store_indexes_ids_by_kind() {
        let store = super::ModuleStore::from_modules(Vec::new());
        assert!(store.ids_by_kind(ModuleKind::Clock).is_empty());
        assert!(store.get(&ModuleId::new("clock")).is_none());
    }
}
```

Run:

```bash
rtk cargo test module_store_indexes_ids_by_kind
```

Expected: fail because `ModuleStore` does not exist.

- [ ] **Step 2: Implement initial `ModuleStore` wrapper**

In `src/app/module_store.rs`, add:

```rust
use std::collections::HashMap;

use crate::modules::{Module, ModuleId, ModuleKind, Zone};

pub struct ModuleStore {
    ordered: Vec<(Zone, Box<dyn Module>)>,
    by_id: HashMap<ModuleId, usize>,
    by_kind: HashMap<ModuleKind, Vec<ModuleId>>,
}

impl ModuleStore {
    pub fn from_modules(ordered: Vec<(Zone, Box<dyn Module>)>) -> Self {
        let mut store = Self {
            ordered,
            by_id: HashMap::new(),
            by_kind: HashMap::new(),
        };
        store.rebuild_indexes();
        store
    }

    fn rebuild_indexes(&mut self) {
        self.by_id.clear();
        self.by_kind.clear();
        for (idx, (_, module)) in self.ordered.iter().enumerate() {
            let id = module.id();
            self.by_id.insert(id.clone(), idx);
            self.by_kind.entry(module.kind()).or_default().push(id);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &(Zone, Box<dyn Module>)> {
        self.ordered.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut (Zone, Box<dyn Module>)> {
        self.ordered.iter_mut()
    }

    pub fn get(&self, id: &ModuleId) -> Option<&dyn Module> {
        self.by_id
            .get(id)
            .and_then(|idx| self.ordered.get(*idx))
            .map(|(_, module)| module.as_ref())
    }

    pub fn ids_by_kind(&self, kind: ModuleKind) -> &[ModuleId] {
        self.by_kind.get(&kind).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn push(&mut self, zone: Zone, module: Box<dyn Module>) {
        self.ordered.push((zone, module));
        self.rebuild_indexes();
    }

    pub fn insert(&mut self, index: usize, item: (Zone, Box<dyn Module>)) {
        self.ordered.insert(index, item);
        self.rebuild_indexes();
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&(Zone, Box<dyn Module>)) -> bool) {
        self.ordered.retain(|item| keep(item));
        self.rebuild_indexes();
    }

    pub fn len(&self) -> usize {
        self.ordered.len()
    }
}
```

- [ ] **Step 3: Register module and change `App` field**

In `src/app.rs`, add:

```rust
mod module_store;
```

Change:

```rust
pub modules: Vec<(Zone, Box<dyn Module>)>,
```

to:

```rust
pub modules: module_store::ModuleStore,
```

In boot construction, wrap:

```rust
modules: module_store::ModuleStore::from_modules(modules),
```

- [ ] **Step 4: Update call sites incrementally**

Replace direct vector operations with store methods:

```rust
self.modules.iter()
self.modules.iter_mut()
self.modules.insert(index, item)
self.modules.retain(|(_, m)| ...)
self.modules.len()
```

Tests may directly `push`; use:

```rust
app.modules.push(zone, Box::new(module));
```

- [ ] **Step 5: Verify**

Run:

```bash
rtk cargo test module_store
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/app/module_store.rs src/app/*.rs
git commit -m "refactor: wrap modules in indexed store"
```

---

## Task 6: Replace Repeated Type Fan-Out With Store Queries

**Files:**
- Modify: `src/app/module_state.rs`
- Modify: `src/app/module_events.rs`
- Modify: `src/app/subscriptions.rs`
- Modify: `src/app/layout.rs`

- [ ] **Step 1: Add `ids_by_kind` fan-out test**

In `src/app/module_state.rs` tests or `src/app.rs` tests, add:

```rust
#[test]
fn battery_state_updates_all_battery_instances() {
    use crate::config::{BatteryConfig, ModuleConfig, ModuleParams};
    let mut config = Config::default();
    config.modules = vec![
        ModuleConfig {
            name: "battery".into(),
            module_type: "battery".into(),
            params: ModuleParams::Battery(BatteryConfig::default()),
        },
        ModuleConfig {
            name: "battery2".into(),
            module_type: "battery".into(),
            params: ModuleParams::Battery(BatteryConfig::default()),
        },
    ];
    config.bars[0].modules.bottom = vec!["battery".into(), "battery2".into()];
    let (mut app, _) = App::boot(config, vec![]);

    app.push_battery_state(42.0, false);

    assert!(matches!(
        app.shared.modules.get(&ModuleId::new("battery")),
        Some(ModuleState::Battery(_))
    ));
    assert!(matches!(
        app.shared.modules.get(&ModuleId::new("battery2")),
        Some(ModuleState::Battery(_))
    ));
}
```

- [ ] **Step 2: Change state fan-out**

In `src/app/module_state.rs`, replace repeated scan/collect blocks with:

```rust
let ids: Vec<_> = self.modules.ids_by_kind(ModuleKind::Battery).to_vec();
```

Use matching kinds for network, volume, and focused-window.

- [ ] **Step 3: Change subscription feature detection**

In `src/app/subscriptions.rs`, replace scan-set logic with:

```rust
let has_battery = !self.modules.ids_by_kind(ModuleKind::Battery).is_empty();
let has_network = !self.modules.ids_by_kind(ModuleKind::Network).is_empty();
let has_volume = !self.modules.ids_by_kind(ModuleKind::Volume).is_empty();
let has_tray = !self.modules.ids_by_kind(ModuleKind::Tray).is_empty();
```

- [ ] **Step 4: Change interaction dispatch**

In `src/app/module_events.rs`, replace the loop in `dispatch_module_interaction` with:

```rust
if let Some(module) = self.modules.get(id) {
    if let Some(msg) = module.on_interaction(event) {
        return Task::done(msg);
    }
}
Task::none()
```

- [ ] **Step 5: Verify**

Run:

```bash
rtk cargo test battery_state_updates_all_battery_instances
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/app/module_state.rs src/app/module_events.rs src/app/subscriptions.rs src/app/layout.rs src/app.rs
git commit -m "refactor: use module store indexes"
```

---

## Task 7: Consolidate Dynamic Module Expansion

**Files:**
- Create: `src/app/dynamic_modules.rs`
- Modify: `src/app/tray.rs`
- Modify: `src/app/niri.rs`

- [ ] **Step 1: Add dynamic cleanup test**

In `src/app.rs` test module, add:

```rust
#[test]
fn clear_surface_module_refs_with_prefix_clears_focus_and_hover() {
    let (mut app, wid) = make_interact_app();
    app.surface_mut(wid).focused_module = Some(ModuleId::new("workspace:1"));
    app.surface_mut(wid).hovered_module = Some(ModuleId::new("workspace:2"));

    app.clear_surface_module_refs_with_prefix("workspace:");

    assert!(app.surface(wid).unwrap().focused_module.is_none());
    assert!(app.surface(wid).unwrap().hovered_module.is_none());
}
```

- [ ] **Step 2: Move shared cleanup helper**

Create `src/app/dynamic_modules.rs`:

```rust
use super::App;

impl App {
    pub(super) fn clear_surface_module_refs_with_prefix(&mut self, prefix: &str) {
        for surface in self.surfaces.values_mut() {
            if surface
                .focused_module
                .as_ref()
                .map(|id| id.0.starts_with(prefix))
                .unwrap_or(false)
            {
                surface.focused_module = None;
            }
            if surface
                .hovered_module
                .as_ref()
                .map(|id| id.0.starts_with(prefix))
                .unwrap_or(false)
            {
                surface.hovered_module = None;
            }
        }
    }
}
```

Register it in `src/app.rs`:

```rust
mod dynamic_modules;
```

Remove the duplicate helper from `src/app/tray.rs`.

- [ ] **Step 3: Use helper in workspace rebuild**

In `src/app/niri.rs`, replace the manual focused/hovered workspace cleanup loop with:

```rust
self.clear_surface_module_refs_with_prefix("workspace:");
```

- [ ] **Step 4: Verify**

Run:

```bash
rtk cargo test clear_surface_module_refs_with_prefix_clears_focus_and_hover
rtk cargo test workspace
rtk cargo test tray
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/app/dynamic_modules.rs src/app/tray.rs src/app/niri.rs
git commit -m "refactor: share dynamic module cleanup"
```

---

## Task 8: Centralize Command Execution

**Files:**
- Create: `src/command.rs`
- Modify: `src/main.rs`
- Modify: `src/modules/script.rs`
- Modify: `src/modules/button.rs`
- Modify: `src/app/module_events.rs`

- [ ] **Step 1: Add command module tests**

Create `src/command.rs`:

```rust
pub const MAX_COMMAND_OUTPUT_BYTES: usize = 4096;

pub fn sanitize_stdout(stdout: &[u8], max_bytes: usize) -> String {
    let text = String::from_utf8_lossy(stdout);
    let trimmed = text.trim();
    if trimmed.len() <= max_bytes {
        trimmed.to_string()
    } else {
        String::from_utf8_lossy(&trimmed.as_bytes()[..max_bytes]).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_stdout_trims_and_caps_bytes() {
        let output = format!("{} trailing\n", "x".repeat(MAX_COMMAND_OUTPUT_BYTES + 32));
        let sanitized = sanitize_stdout(output.as_bytes(), MAX_COMMAND_OUTPUT_BYTES);
        assert_eq!(sanitized.len(), MAX_COMMAND_OUTPUT_BYTES);
        assert!(sanitized.chars().all(|c| c == 'x'));
    }
}
```

In `src/main.rs`, add:

```rust
mod command;
```

Run:

```bash
rtk cargo test sanitize_stdout_trims_and_caps_bytes
```

Expected: pass.

- [ ] **Step 2: Move script command execution**

Extend `src/command.rs`:

```rust
pub async fn run_shell_capture_stdout(
    command: &str,
    timeout: std::time::Duration,
    max_output_bytes: usize,
) -> std::io::Result<Option<String>> {
    let mut child = tokio::process::Command::new("sh");
    child.arg("-c").arg(command).kill_on_drop(true);
    let output = tokio::time::timeout(timeout, child.output()).await;

    match output {
        Ok(Ok(out)) if out.status.success() => {
            Ok(Some(sanitize_stdout(&out.stdout, max_output_bytes)))
        }
        Ok(Ok(_)) => Ok(None),
        Ok(Err(e)) => Err(e),
        Err(_) => Ok(None),
    }
}
```

In `src/modules/script.rs`, replace `sanitize_script_stdout` and `tokio::process::Command` usage with:

```rust
async fn run_script_command(cmd: &str, timeout: std::time::Duration) -> ScriptEvent {
    match crate::command::run_shell_capture_stdout(cmd, timeout, MAX_SCRIPT_OUTPUT_BYTES).await {
        Ok(Some(output)) => ScriptEvent::Output(output),
        Ok(None) | Err(_) => ScriptEvent::Failed,
    }
}
```

- [ ] **Step 3: Move button fire-and-forget**

Extend `src/command.rs`:

```rust
pub fn spawn_shell_fire_and_forget(command: &str) -> std::io::Result<()> {
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
```

In `src/modules/button.rs`, replace local `spawn_button_command` with:

```rust
let _ = crate::command::spawn_shell_fire_and_forget(&self.on_click);
```

Update the button test to call the shared function:

```rust
#[test]
fn spawn_shell_fire_and_forget_returns_ok_for_short_command() {
    assert!(crate::command::spawn_shell_fire_and_forget("true").is_ok());
}
```

- [ ] **Step 4: Route `wpctl` through named helper**

Add to `src/command.rs`:

```rust
pub async fn set_default_sink_volume(arg: &str) -> std::io::Result<std::process::ExitStatus> {
    tokio::process::Command::new("wpctl")
        .args(["set-volume", "-l", "1", "@DEFAULT_AUDIO_SINK@", arg])
        .status()
        .await
}
```

In `src/app/module_events.rs`, replace direct `tokio::process::Command::new("wpctl")` with:

```rust
let _ = crate::command::set_default_sink_volume(&arg).await;
```

- [ ] **Step 5: Verify**

Run:

```bash
rtk cargo test command
rtk cargo test script
rtk cargo test button
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/command.rs src/modules/script.rs src/modules/button.rs src/app/module_events.rs
git commit -m "refactor: centralize command execution"
```

---

## Task 9: Harden SNI Pixmap and Menu Bounds

**Files:**
- Modify: `src/dbus/sni_host.rs`

- [ ] **Step 1: Add pixmap validation tests**

In `src/dbus/sni_host.rs` tests, add:

```rust
#[test]
fn valid_pixmap_accepts_matching_byte_len() {
    assert!(valid_pixmap(16, 16, &[0u8; 16 * 16 * 4]));
}

#[test]
fn valid_pixmap_rejects_negative_dimensions() {
    assert!(!valid_pixmap(-1, 16, &[0u8; 16 * 16 * 4]));
}

#[test]
fn valid_pixmap_rejects_mismatched_byte_len() {
    assert!(!valid_pixmap(16, 16, &[0u8; 3]));
}
```

Run:

```bash
rtk cargo test valid_pixmap_
```

Expected: fail because `valid_pixmap` does not exist.

- [ ] **Step 2: Add limits and validation helper**

In `src/dbus/sni_host.rs`, near pixmap helpers, add:

```rust
const MAX_PIXMAP_DIMENSION: i32 = 512;
const MAX_MENU_ITEMS: usize = 256;
const MAX_MENU_LABEL_BYTES: usize = 512;

fn valid_pixmap(width: i32, height: i32, bytes: &[u8]) -> bool {
    if width <= 0 || height <= 0 {
        return false;
    }
    if width > MAX_PIXMAP_DIMENSION || height > MAX_PIXMAP_DIMENSION {
        return false;
    }
    let Some(pixel_count) = (width as usize).checked_mul(height as usize) else {
        return false;
    };
    let Some(expected_bytes) = pixel_count.checked_mul(4) else {
        return false;
    };
    bytes.len() == expected_bytes
}
```

- [ ] **Step 3: Filter pixmaps on fetch**

In `fetch_item`, replace:

```rust
let pixmaps = proxy.icon_pixmap().await.unwrap_or_default();
TrayIcon::Pixmap(pixmaps)
```

with:

```rust
let pixmaps = proxy
    .icon_pixmap()
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|(w, h, bytes)| valid_pixmap(*w, *h, bytes))
    .collect();
TrayIcon::Pixmap(pixmaps)
```

- [ ] **Step 4: Cap menu labels and item count**

Add:

```rust
fn truncate_label(label: String) -> String {
    if label.len() <= MAX_MENU_LABEL_BYTES {
        label
    } else {
        String::from_utf8_lossy(&label.as_bytes()[..MAX_MENU_LABEL_BYTES]).to_string()
    }
}
```

In `owned_value_to_menu_item`, change:

```rust
let label = extract_label_from_props(&props);
```

to:

```rust
let label = truncate_label(extract_label_from_props(&props));
```

In `fetch_menu`, change item collection to:

```rust
let items: Vec<crate::surface::menu_popup::MenuItem> = children
    .iter()
    .filter_map(owned_value_to_menu_item)
    .take(MAX_MENU_ITEMS)
    .collect();
```

- [ ] **Step 5: Verify**

Run:

```bash
rtk cargo test valid_pixmap_
rtk cargo test sni_host
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/dbus/sni_host.rs
git commit -m "security: bound tray pixmap and menu data"
```

---

## Task 10: Isolate PulseAudio Unsafe Callback

**Files:**
- Modify: `src/dbus/wireplumber.rs`

- [ ] **Step 1: Extract unsafe callback helper**

In `src/dbus/wireplumber.rs`, add above `run_pa_thread`:

```rust
fn mainloop_signal_callback(
    mainloop: &mut libpulse_binding::mainloop::threaded::Mainloop,
) -> Box<dyn FnMut() + 'static> {
    let ml_ptr = mainloop as *mut libpulse_binding::mainloop::threaded::Mainloop as usize;
    Box::new(move || {
        // SAFETY: `ml_ptr` points to the `Mainloop` stack value owned by `run_pa_thread`.
        // The callback is cleared before that function can return, and libpulse invokes it
        // while the threaded mainloop is alive.
        let ml = unsafe {
            &mut *(ml_ptr as *mut libpulse_binding::mainloop::threaded::Mainloop)
        };
        ml.signal(false);
    })
}
```

- [ ] **Step 2: Replace inline unsafe block**

In `run_pa_thread`, replace:

```rust
let ml_ptr = &mut mainloop as *mut Mainloop as usize;
context.set_state_callback(Some(Box::new(move || {
    let ml = unsafe { &mut *(ml_ptr as *mut Mainloop) };
    ml.signal(false);
})));
```

with:

```rust
context.set_state_callback(Some(mainloop_signal_callback(&mut mainloop)));
```

- [ ] **Step 3: Verify**

Run:

```bash
rtk cargo test wireplumber
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add src/dbus/wireplumber.rs
git commit -m "refactor: isolate pulse mainloop unsafe callback"
```

---

## Task 11: Refactor KDL Parsing Helpers

**Files:**
- Modify: `src/config/parse.rs`

- [ ] **Step 1: Add helper unit tests**

In `src/config/parse.rs` tests, add:

```rust
#[test]
fn parse_required_string_child_reports_missing_name() {
    let kdl = r#"modules { module "button" { icon ">" on-click "kitty" } }"#;
    let err = parse(kdl).unwrap_err();
    assert!(err.to_string().contains("button.label"));
}

#[test]
fn parse_optional_u8_range_reports_out_of_range() {
    let kdl = r#"modules { module "volume" { step 101 } }"#;
    let err = parse(kdl).unwrap_err();
    assert!(err.to_string().contains("volume.step"));
    assert!(err.to_string().contains("1–100"));
}
```

Run:

```bash
rtk cargo test parse_required_string_child_reports_missing_name parse_optional_u8_range_reports_out_of_range
```

Expected: pass before refactor; these lock behavior.

- [ ] **Step 2: Add parser helper functions**

In `src/config/parse.rs`, near the top, add:

```rust
fn first_child_value<'a>(
    children: &'a kdl::KdlDocument,
    name: &str,
) -> Option<&'a kdl::KdlValue> {
    children
        .get(name)
        .and_then(|node| node.entries().first())
        .map(|entry| entry.value())
}

fn required_string_child(
    children: &kdl::KdlDocument,
    name: &str,
    path: &str,
) -> anyhow::Result<String> {
    match first_child_value(children, name).and_then(|v| v.as_string()) {
        Some(s) => Ok(s.to_string()),
        None => anyhow::bail!("{path}: expected a string value."),
    }
}

fn optional_nonempty_string_child(
    children: &kdl::KdlDocument,
    name: &str,
    default: String,
    path: &str,
    expected: &str,
) -> anyhow::Result<String> {
    match first_child_value(children, name) {
        None => Ok(default),
        Some(v) => match v.as_string() {
            Some("") => anyhow::bail!("{path}: value must not be empty. Expected: {expected}."),
            Some(s) => Ok(s.to_string()),
            None => anyhow::bail!("{path}: expected a string value."),
        },
    }
}

fn optional_u8_range_child(
    children: &kdl::KdlDocument,
    name: &str,
    default: u8,
    min: i64,
    max: i64,
    path: &str,
) -> anyhow::Result<u8> {
    match first_child_value(children, name) {
        None => Ok(default),
        Some(v) => match v.as_integer() {
            Some(n) if (min..=max).contains(&n) => Ok(n as u8),
            Some(n) => anyhow::bail!(
                "{path}: value {n} out of range. Expected: integer {min}–{max}."
            ),
            None => anyhow::bail!("{path}: expected an integer value."),
        },
    }
}
```

- [ ] **Step 3: Replace battery and volume parsing first**

Change `parse_battery_params` to:

```rust
fn parse_battery_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<BatteryConfig> {
    let Some(children) = node.children() else {
        return Ok(BatteryConfig::default());
    };
    Ok(BatteryConfig {
        critical_threshold: optional_u8_range_child(
            children,
            "critical-threshold",
            BatteryConfig::default().critical_threshold,
            0,
            100,
            &format!("modules[{idx}].battery.critical-threshold"),
        )?,
    })
}
```

Change `parse_volume_params` similarly:

```rust
fn parse_volume_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<VolumeConfig> {
    let Some(children) = node.children() else {
        return Ok(VolumeConfig::default());
    };
    Ok(VolumeConfig {
        step: optional_u8_range_child(
            children,
            "step",
            VolumeConfig::default().step,
            1,
            100,
            &format!("modules[{idx}].volume.step"),
        )?,
    })
}
```

- [ ] **Step 4: Replace clock/button/script string parsing**

Use `optional_nonempty_string_child` for clock formats and script command. For button fields, keep missing-field wording by checking presence first, then call `required_string_child`:

```rust
if children.get("label").is_none() {
    anyhow::bail!(
        "modules[{idx}].button.label: missing required parameter. Expected: a string value."
    );
}
let label = required_string_child(children, "label", &format!("modules[{idx}].button.label"))?;
```

Repeat for `icon` and `on-click`.

- [ ] **Step 5: Verify**

Run:

```bash
rtk cargo test config::parse
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass and error-message tests remain green.

- [ ] **Step 6: Commit**

```bash
git add src/config/parse.rs
git commit -m "refactor: centralize kdl value parsing"
```

---

## Task 12: Final Verification and Documentation

**Files:**
- Modify: `refactor.md` or create `docs/superpowers/specs/2026-04-19-code-quality-refactor-summary.md`

- [ ] **Step 1: Run full verification**

Run:

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build --release
```

Expected:

```text
all tests pass
clippy exits 0
release build exits 0
```

- [ ] **Step 2: Capture code-size hotspots after refactor**

Run:

```bash
rtk wc -l src/app.rs src/app/*.rs src/modules.rs src/config/parse.rs src/dbus/sni_host.rs
```

Expected: `src/app.rs` should be meaningfully smaller than the original 3779-line baseline, with menu/tray logic moved into focused files.

- [ ] **Step 3: Write summary**

Create `docs/superpowers/specs/2026-04-19-code-quality-refactor-summary.md` with:

```markdown
# Code Quality Refactor Summary

## Completed

- Restored clean `cargo clippy --all-targets -- -D warnings`.
- Extracted menu popup reducer from `App::update`.
- Extracted tray reducer from `App::update`.
- Added typed `ModuleKind`.
- Wrapped runtime modules in `ModuleStore`.
- Shared dynamic module cleanup.
- Centralized command execution.
- Bounded tray pixmap and DBusMenu data.
- Isolated PulseAudio unsafe callback.
- Reduced KDL parser duplication.

## Verification

- `rtk cargo test`
- `rtk cargo clippy --all-targets -- -D warnings`
- `rtk cargo build --release`

## Residual Risk

- Runtime tray and PulseAudio behavior still needs manual Wayland-session smoke testing.
- Command execution remains intentionally shell-based for compatibility, but now has one audit boundary.
```

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/specs/2026-04-19-code-quality-refactor-summary.md
git commit -m "docs: summarize code quality refactor"
```

---

## Manual Smoke Test Checklist

Run after all automated checks pass in a Niri session:

- [ ] Start `niribar` with default config.
- [ ] Confirm bar renders on expected output.
- [ ] Confirm click expand/collapse still works.
- [ ] Confirm clock updates.
- [ ] Confirm battery/network/volume modules degrade gracefully if services are unavailable.
- [ ] Confirm volume scroll changes volume.
- [ ] Confirm workspace indicators update when switching workspaces.
- [ ] Confirm focused-window title updates when focusing different windows.
- [ ] Confirm tray icons appear, activate, and open menus.
- [ ] Confirm popup Escape/click-away closes menu and keyboard focus returns to bar.

---

## Self-Review

- Spec coverage: The plan covers the original recommendations: clippy gate, `App::update` extraction, `ModuleKind`, module indexing, dynamic module consolidation, command hardening, SNI bounds, unsafe isolation, and KDL parsing cleanup.
- Placeholder scan: No task uses `TBD`, vague “handle edge cases,” or test-free implementation instructions.
- Type consistency: `ModuleKind`, `ModuleStore`, `close_all_menu_popups`, `restore_bar_keyboard_interactivity`, and `clear_surface_module_refs_with_prefix` are introduced before later tasks use them.
