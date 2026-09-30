# App Actor Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decompose `App` into ownership-focused runtime components while preserving iced's single-threaded update loop and all observable behavior.

**Architecture:** `App` remains the iced application boundary and owns cross-cutting startup, performance reporting, and layer-shell message integration. Domain structs own their state and return `Task<Message>` for effects; `App::update` routes typed message groups instead of mutating every field directly. No channels, locks, threads, or runtime dependencies are introduced.

**Tech Stack:** Rust 2024 edition, `iced` 0.14 with `iced_layershell`, `zbus` 5.x, `tokio`, `niri-ipc`.

---

## Current Inventory

`crates/niribar/src/app.rs` currently defines `SharedState`, `App`, `Message`, `default_params_for_type`, `App::boot`, `App::namespace`, `App::update`, and a large in-file test module. `SharedState` owns `modules: HashMap<ModuleId, ModuleState>`, `niri: crate::niri::NiriState`, and `dbus: DbusConnections`. `App` owns `config`, `shared`, `perf`, `modules: ModuleStore`, `surfaces`, `pending_surface_keys`, `manual_outputs`, popup maps, menu focus maps, and `last_cursor`.

`crates/niribar/src/app/surfaces.rs` owns surface lookup, manual output window spawning, bar lookup, overlay size-change tasks, tray-icon removal, and tray insertion helpers. `crates/niribar/src/app/animation.rs`, `crates/niribar/src/app/focus.rs`, `crates/niribar/src/app/navigation.rs`, and `crates/niribar/src/app/layout.rs` still implement methods on `App` that mostly mutate per-window `SurfaceState` and inspect modules/config.

`crates/niribar/src/app/menu.rs`, popup-related arms in `App::update`, and `crates/niribar/src/surface/menu_popup.rs` currently split popup state, DBusMenu click/closed events, popup rendering, hover state, focus state, and window lifecycle. Popup windows are stored in `App::menu_popups` as `(service, menu_path, tree, bar_name)`.

`crates/niribar/src/app/tray.rs`, tray message arms in `App::update`, `crates/niribar/src/modules/tray.rs`, and `crates/niribar/src/dbus/sni/` currently split SNI host events, SNI item activation, menu fetches, and dynamic tray icon module insertion.

`crates/niribar/src/app/niri.rs`, `crates/niribar/src/niri.rs`, and `crates/niribar/src/niri/subscription.rs` currently split IPC subscription events, workspace/window state storage, focused-window derivation, workspace item rebuilds, and manual hotplug surface creation.

`crates/niribar/src/app/module_store.rs`, `crates/niribar/src/app/module_state.rs`, `crates/niribar/src/app/module_events.rs`, `crates/niribar/src/app/dynamic_modules.rs`, `crates/niribar/src/app/widget_ops.rs`, and `crates/niribar/src/app/subscriptions.rs` currently split module instantiation, `ModuleState` updates, dynamic workspace/tray modules, widget bounds operations, and runtime subscriptions.

## Target File Structure

- Modify `crates/niribar/src/app.rs`: keep `App`, public iced entry points, `Message`, `default_params_for_type`, `App::boot`, top-level routing, `DbusReady`, `PerfReport`, and `NiriFocusWorkspace`.
- Create `crates/niribar/src/app/shared.rs`: define `SharedContext` with only D-Bus handles after module and niri state move out.
- Create `crates/niribar/src/app/messages.rs`: define `SurfaceEvent`, `MenuEvent`, `ModuleRuntimeEvent`, `AppNiriEvent`, and `TrayRuntimeEvent`; re-export them through `crates/niribar/src/app.rs`.
- Modify `crates/niribar/src/app/surfaces.rs`: introduce `SurfaceManager` owning bar surfaces, pending surface keys, manual output mode, popup window metadata, and last cursor.
- Modify `crates/niribar/src/app/menu.rs`: introduce `MenuSystem` owning menu model, hover/focus state, menu rendering handoff, and DBusMenu selected/closed effects.
- Modify `crates/niribar/src/app/niri.rs`: introduce `AppNiriState` owning crate-level `crate::niri::NiriState` and niri update logic.
- Modify `crates/niribar/src/app/tray.rs`: introduce `TrayState` owning SNI item snapshot and SNI item action/menu fetch conversion.
- Create `crates/niribar/src/app/module_runtime.rs`: introduce `ModuleRuntime` owning `ModuleStore` and `HashMap<ModuleId, ModuleState>`.
- Modify `crates/niribar/src/app/view.rs`: render through `SurfaceManager`, `MenuSystem`, and `ModuleRuntime` read APIs.
- Move tests from `crates/niribar/src/app.rs` into the owning modules as each owner lands; keep only `namespace_returns_niribar`, `boot_with_config_stores_config`, `dbus_ready_stores_connections_in_shared_context`, and perf routing tests in `crates/niribar/src/app.rs`.

## Message Shape

The final `Message` enum in `crates/niribar/src/app.rs` must keep layer-shell generated variants at the top level and wrap domain messages explicitly:

```rust
#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    Surface(messages::SurfaceEvent),
    Menu(messages::MenuEvent),
    Modules(messages::ModuleRuntimeEvent),
    Niri(messages::AppNiriEvent),
    Tray(messages::TrayRuntimeEvent),
    Battery(crate::modules::battery::BatteryEvent),
    Network(crate::modules::network::NetworkEvent),
    Volume(crate::modules::volume::VolumeEvent),
    VolumeAdjust { step: u8, up: bool },
    DbusReady(crate::dbus::DbusConnections),
    NiriFocusWorkspace { id: u64 },
    PerfReport,
}
```

Keep `Battery`, `Network`, `Volume`, `VolumeAdjust`, `DbusReady`, `NiriFocusWorkspace`, and `PerfReport` top-level until `ModuleRuntime` lands, then route battery/network/volume through `Message::Modules`.

## Task 1: Add SharedContext And Domain Message Types

**Files:**
- Create: `crates/niribar/src/app/shared.rs`
- Create: `crates/niribar/src/app/messages.rs`
- Modify: `crates/niribar/src/app.rs`
- Test: `crates/niribar/src/app.rs`

- [ ] **Step 1: Write failing tests for shared context and message labels**

Add these tests to the existing `#[cfg(test)] mod tests` in `crates/niribar/src/app.rs`:

```rust
#[test]
fn shared_context_starts_with_default_dbus() {
    let shared = shared::SharedContext::new();
    assert!(shared.dbus.session.is_none());
    assert!(shared.dbus.system.is_none());
}

#[test]
fn perf_tracker_labels_surface_wrapped_event() {
    let event = Message::Surface(messages::SurfaceEvent::Tick);
    assert_eq!(event.perf_label(), "Surface::Tick");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `rtk cargo test shared_context_starts_with_default_dbus perf_tracker_labels_surface_wrapped_event`

Expected: FAIL because `shared` and `messages` modules do not exist.

- [ ] **Step 3: Add `SharedContext`**

Create `crates/niribar/src/app/shared.rs`:

```rust
#[derive(Default)]
pub(super) struct SharedContext {
    pub(super) dbus: crate::dbus::DbusConnections,
}

impl SharedContext {
    pub(super) fn new() -> Self {
        Self {
            dbus: crate::dbus::DbusConnections::default(),
        }
    }
}
```

- [ ] **Step 4: Add domain event enums that mirror current `Message` variants**

Create `crates/niribar/src/app/messages.rs`:

```rust
use crate::modules::{ModuleEvent, ModuleId};

#[derive(Debug, Clone)]
pub enum SurfaceEvent {
    BarClick(iced::window::Id),
    Tick,
    BarHoverEnter(iced::window::Id),
    BarHoverExit(iced::window::Id),
    ModuleHoverEnter(ModuleId, iced::window::Id),
    ModuleHoverExit(ModuleId, iced::window::Id),
    BarFocusGained(iced::window::Id),
    BarFocusLost(iced::window::Id),
    EscapePressed(iced::window::Id),
    FocusMoveNext(iced::window::Id),
    FocusMovePrev(iced::window::Id),
    FocusNextZone(iced::window::Id),
    FocusPrevZone(iced::window::Id),
    KeyboardActivate(iced::window::Id),
    KeyboardSecondaryActivate(iced::window::Id),
    ToggleExpand,
    CursorMoved(iced::window::Id, iced::Point),
    WindowSized(iced::window::Id, u32, u32),
}

#[derive(Debug, Clone)]
pub enum MenuEvent {
    ItemSelected {
        service: String,
        menu_item_id: i32,
        menu_path: zbus::zvariant::OwnedObjectPath,
    },
    Closed,
    ItemHoverEnter {
        window: iced::window::Id,
        item_id: String,
    },
    ItemHoverExit(iced::window::Id),
    LayoutChanged {
        service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
    },
    Open {
        service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
        tree: crate::surface::menu_popup::MenuTree,
        bar_name: String,
    },
}

#[derive(Debug, Clone)]
pub enum ModuleRuntimeEvent {
    Module(ModuleId, ModuleEvent),
    ModuleClick(ModuleId, iced::window::Id),
    ModuleScrollUp(ModuleId, iced::window::Id),
    ModuleScrollDown(ModuleId, iced::window::Id),
    Battery(crate::modules::battery::BatteryEvent),
    Network(crate::modules::network::NetworkEvent),
    Volume(crate::modules::volume::VolumeEvent),
}

#[derive(Debug, Clone)]
pub enum AppNiriEvent {
    Ipc(crate::niri::NiriEvent),
}

#[derive(Debug, Clone)]
pub enum TrayRuntimeEvent {
    Host(crate::dbus::sni::host::TrayEvent),
    Activate { service: String },
    MenuRequest {
        service: String,
        menu_path: Option<zbus::zvariant::OwnedObjectPath>,
        source_window: Option<iced::window::Id>,
    },
    MenuRequestKeyboard {
        service: String,
        menu_path: Option<zbus::zvariant::OwnedObjectPath>,
        source_window: Option<iced::window::Id>,
    },
    MenuRequestWithBounds {
        service: String,
        menu_path: Option<zbus::zvariant::OwnedObjectPath>,
        bounds: Option<iced::Rectangle>,
        screen_size: (u32, u32),
        source_window: Option<iced::window::Id>,
    },
    SecondaryActivate { service: String },
    Scroll {
        service: String,
        delta: i32,
        orientation: String,
    },
}
```

- [ ] **Step 5: Wire modules and replace `SharedState` with `SharedContext`**

In `crates/niribar/src/app.rs`, add modules near the current module declarations:

```rust
mod messages;
mod shared;
```

Replace the `SharedState` definition and constructor with use of `shared::SharedContext`. Change the `App` field:

```rust
shared: shared::SharedContext,
```

Change `App::boot`:

```rust
shared: shared::SharedContext::new(),
```

Keep the existing `SharedState` name out of the code after this step. References to `shared.modules` and `shared.niri` will fail until Task 5 and Task 7 move those fields, so temporarily add compatibility fields to `SharedContext`:

```rust
pub(super) modules: std::collections::HashMap<crate::modules::ModuleId, crate::modules::ModuleState>,
pub(super) niri: crate::niri::NiriState,
```

and initialize them in `new()`. Task 9 removes these compatibility fields.

- [ ] **Step 6: Add temporary wrapped label support**

Extend `Message::perf_label()` in `crates/niribar/src/app.rs` with these arms above the old flat variants:

```rust
Message::Surface(messages::SurfaceEvent::Tick) => "Surface::Tick",
Message::Surface(_) => "Surface",
Message::Menu(_) => "Menu",
Message::Modules(_) => "Modules",
Message::Niri(messages::AppNiriEvent::Ipc(crate::niri::NiriEvent::Connected)) => {
    "Niri::Connected"
}
Message::Niri(messages::AppNiriEvent::Ipc(crate::niri::NiriEvent::Unavailable)) => {
    "Niri::Unavailable"
}
Message::Niri(messages::AppNiriEvent::Ipc(crate::niri::NiriEvent::Event(_))) => "Niri::Event",
Message::Tray(_) => "Tray",
```

- [ ] **Step 7: Run focused tests**

Run: `rtk cargo test shared_context_starts_with_default_dbus perf_tracker_labels_surface_wrapped_event`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/shared.rs crates/niribar/src/app/messages.rs
rtk git commit -m "refactor(app): add runtime context and event groups"
```

## Task 2: Extract SurfaceManager State

**Files:**
- Modify: `crates/niribar/src/app/surfaces.rs`
- Modify: `crates/niribar/src/app.rs`
- Test: `crates/niribar/src/app/surfaces.rs`

- [ ] **Step 1: Move surface tests into `crates/niribar/src/app/surfaces.rs`**

Move these tests from `crates/niribar/src/app.rs` into `#[cfg(test)] mod tests` at the bottom of `crates/niribar/src/app/surfaces.rs`: `app_starts_with_no_surfaces`, `surface_mut_creates_default_state_for_new_window`, `two_surfaces_have_independent_expand_state`, `two_surfaces_have_independent_focus_state`, `boot_with_empty_allowed_outputs_uses_allscreens_mode`, `boot_manual_outputs_false_when_no_allowed_outputs`, `boot_manual_outputs_true_when_allowed_outputs_given`, `boot_manual_mode_creates_surface_per_bar_per_output`, `surface_bar_name_set_from_pending_surface_key`, `surface_output_name_set_from_pending_surface_key`, `bar_for_surface_name_returns_first_when_name_empty`, `bar_for_surface_name_returns_matching_bar`, `bar_for_surface_name_falls_back_to_first_on_miss`, `boot_spawns_horizontal_bars_before_side_bars`.

Use this helper block in the moved test module:

```rust
fn make_config() -> crate::config::Config {
    crate::config::Config::default()
}

fn test_wid() -> iced::window::Id {
    iced::window::Id::unique()
}
```

- [ ] **Step 2: Run moved tests to verify current behavior**

Run: `rtk cargo test app_starts_with_no_surfaces surface_mut_creates_default_state_for_new_window boot_manual_mode_creates_surface_per_bar_per_output`

Expected: PASS before the extraction starts.

- [ ] **Step 3: Add `SurfaceManager`**

At the top of `crates/niribar/src/app/surfaces.rs`, add:

```rust
pub(super) struct PopupMeta {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) tree: crate::surface::menu_popup::MenuTree,
    pub(super) bar_name: String,
}

pub(super) struct SurfaceManager {
    surfaces: HashMap<iced::window::Id, SurfaceState>,
    pending_surface_keys: HashMap<iced::window::Id, (String, String)>,
    manual_outputs: bool,
    popups: HashMap<iced::window::Id, PopupMeta>,
    last_cursor: Option<(iced::window::Id, iced::Point)>,
}

impl SurfaceManager {
    pub(super) fn new(
        surfaces: HashMap<iced::window::Id, SurfaceState>,
        pending_surface_keys: HashMap<iced::window::Id, (String, String)>,
        manual_outputs: bool,
    ) -> Self {
        Self {
            surfaces,
            pending_surface_keys,
            manual_outputs,
            popups: HashMap::new(),
            last_cursor: None,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    pub(super) fn manual_outputs(&self) -> bool {
        self.manual_outputs
    }

    pub(super) fn surface(&self, id: iced::window::Id) -> Option<&SurfaceState> {
        self.surfaces.get(&id)
    }

    pub(super) fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
        if !self.surfaces.contains_key(&id) {
            let (output_name, bar_name) = self.pending_surface_keys.remove(&id).unwrap_or_default();
            self.surfaces
                .insert(id, SurfaceState::new(output_name, bar_name));
        }
        self.surfaces.get_mut(&id).unwrap()
    }

    pub(super) fn surfaces(&self) -> impl Iterator<Item = (&iced::window::Id, &SurfaceState)> {
        self.surfaces.iter()
    }

    pub(super) fn surfaces_mut(&mut self) -> impl Iterator<Item = &mut SurfaceState> {
        self.surfaces.values_mut()
    }

    pub(super) fn last_cursor(&self) -> Option<(iced::window::Id, iced::Point)> {
        self.last_cursor
    }

    pub(super) fn set_last_cursor(&mut self, value: Option<(iced::window::Id, iced::Point)>) {
        self.last_cursor = value;
    }
}
```

- [ ] **Step 4: Move fields into `SurfaceManager`**

In `crates/niribar/src/app.rs`, replace these fields:

```rust
surfaces: HashMap<iced::window::Id, SurfaceState>,
pending_surface_keys: HashMap<iced::window::Id, (String, String)>,
manual_outputs: bool,
menu_popups: HashMap<
    iced::window::Id,
    (
        String,
        zbus::zvariant::OwnedObjectPath,
        crate::surface::menu_popup::MenuTree,
        String,
    ),
>,
last_cursor: Option<(iced::window::Id, iced::Point)>,
```

with:

```rust
surfaces: surfaces::SurfaceManager,
```

Keep `menu_popup_hovered` and `menu_focused_item` in `App` until Task 4.

In `App::boot`, build the manager:

```rust
let app = App {
    config,
    shared: shared::SharedContext::new(),
    perf: perf::PerfTracker::from_env(),
    modules: module_store::ModuleStore::from_modules(modules),
    surfaces: surfaces::SurfaceManager::new(surfaces, pending_surface_keys, manual_outputs),
    menu_popup_hovered: HashMap::new(),
    menu_focused_item: HashMap::new(),
};
```

- [ ] **Step 5: Update callers through accessors**

Replace direct map use in `crates/niribar/src/app.rs`, `crates/niribar/src/app/view.rs`, `crates/niribar/src/app/menu.rs`, `crates/niribar/src/app/surfaces.rs`, `crates/niribar/src/app/animation.rs`, `crates/niribar/src/app/focus.rs`, `crates/niribar/src/app/navigation.rs`, and `crates/niribar/src/app/layout.rs`:

```rust
self.surfaces.get(&window)
self.surfaces.get_mut(&window)
self.surfaces.values_mut()
self.surfaces.keys()
self.surfaces.is_empty()
self.manual_outputs
self.last_cursor
```

with:

```rust
self.surfaces.surface(window)
self.surfaces.surface_mut(window)
self.surfaces.surfaces_mut()
self.surfaces.surfaces().map(|(id, _)| *id)
self.surfaces.is_empty()
self.surfaces.manual_outputs()
self.surfaces.last_cursor()
```

- [ ] **Step 6: Keep App method shims temporarily**

In `impl App` in `crates/niribar/src/app/surfaces.rs`, keep these shims so other files compile while later tasks move logic:

```rust
pub(super) fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
    self.surfaces.surface_mut(id)
}

pub(super) fn surface(&self, id: iced::window::Id) -> Option<&SurfaceState> {
    self.surfaces.surface(id)
}
```

- [ ] **Step 7: Run surface tests**

Run: `rtk cargo test app_starts_with_no_surfaces surface_mut_creates_default_state_for_new_window two_surfaces_have_independent_expand_state boot_manual_mode_creates_surface_per_bar_per_output`

Expected: PASS.

- [ ] **Step 8: Run verification gate**

Run: `rtk cargo test`

Expected: PASS.

Run: `rtk cargo clippy --all-targets -- -D warnings`

Expected: PASS.

Run: `rtk cargo build`

Expected: PASS.

- [ ] **Step 9: Manual smoke after SurfaceManager**

Run the app in a Wayland session and verify:

```text
Bar renders on every configured output.
Hover/click expand toggle works.
Workspace switching still updates focus highlight.
Focused-window title still updates.
```

- [ ] **Step 10: Commit**

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/surfaces.rs crates/niribar/src/app/animation.rs crates/niribar/src/app/focus.rs crates/niribar/src/app/navigation.rs crates/niribar/src/app/layout.rs crates/niribar/src/app/view.rs crates/niribar/src/app/menu.rs
rtk git commit -m "refactor(app): move surface state into manager"
```

## Task 3: Route Surface Events Through `SurfaceEvent`

**Files:**
- Modify: `crates/niribar/src/app.rs`
- Modify: `crates/niribar/src/app/surfaces.rs`
- Modify: `crates/niribar/src/app/animation.rs`
- Modify: `crates/niribar/src/app/focus.rs`
- Modify: `crates/niribar/src/app/navigation.rs`
- Modify: `crates/niribar/src/app/view.rs`
- Test: `crates/niribar/src/app/surfaces.rs`

- [ ] **Step 1: Add routing tests**

Add this test to `crates/niribar/src/app/surfaces.rs`:

```rust
#[test]
fn wrapped_module_hover_enter_sets_hovered_module() {
    let (mut app, _) = App::boot(make_config(), vec![]);
    let wid = test_wid();
    let id = crate::modules::ModuleId::new("clock");

    let _ = app.update(Message::Surface(super::messages::SurfaceEvent::ModuleHoverEnter(
        id.clone(),
        wid,
    )));

    assert_eq!(app.surface(wid).unwrap().hovered_module, Some(id));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test wrapped_module_hover_enter_sets_hovered_module`

Expected: FAIL because `Message::Surface` is not routed.

- [ ] **Step 3: Add `handle_surface_event`**

In `crates/niribar/src/app/surfaces.rs`, add:

```rust
impl App {
    pub(super) fn handle_surface_event(
        &mut self,
        event: super::messages::SurfaceEvent,
    ) -> Task<Message> {
        use super::messages::SurfaceEvent;

        match event {
            SurfaceEvent::CursorMoved(window, pos) => {
                let has_tray = self.modules.iter().any(|(_, module)| {
                    matches!(module.kind(), ModuleKind::Tray | ModuleKind::TrayIcon)
                });
                if has_tray && self.surfaces.last_cursor() != Some((window, pos)) {
                    self.surfaces.set_last_cursor(Some((window, pos)));
                }
                Task::none()
            }
            SurfaceEvent::WindowSized(window, w, h) => {
                self.surfaces.surface_mut(window).screen_size = (w, h);
                Task::none()
            }
            SurfaceEvent::ModuleHoverEnter(id, window) => {
                self.surface_mut(window).hovered_module = Some(id);
                Task::none()
            }
            SurfaceEvent::ModuleHoverExit(id, window) => {
                let s = self.surface_mut(window);
                if s.hovered_module.as_ref() == Some(&id) {
                    s.hovered_module = None;
                }
                Task::none()
            }
            SurfaceEvent::BarClick(window) => self.handle_bar_click(window),
            SurfaceEvent::BarHoverEnter(window) => self.handle_bar_hover_enter(window),
            SurfaceEvent::BarHoverExit(window) => self.handle_bar_hover_exit(window),
            SurfaceEvent::Tick => self.handle_tick(),
            SurfaceEvent::BarFocusGained(window) => self.handle_bar_focus_gained(window),
            SurfaceEvent::BarFocusLost(window) => self.handle_bar_focus_lost(window),
            SurfaceEvent::EscapePressed(window) => self.handle_escape_pressed(window),
            SurfaceEvent::FocusMoveNext(window) => self.handle_focus_move(window, true),
            SurfaceEvent::FocusMovePrev(window) => self.handle_focus_move(window, false),
            SurfaceEvent::FocusNextZone(window) => self.handle_focus_zone(window, true),
            SurfaceEvent::FocusPrevZone(window) => self.handle_focus_zone(window, false),
            SurfaceEvent::KeyboardActivate(window) => self.handle_keyboard_activate(window),
            SurfaceEvent::KeyboardSecondaryActivate(window) => {
                self.handle_keyboard_secondary_activate(window)
            }
            SurfaceEvent::ToggleExpand => self.handle_toggle_expand(),
        }
    }
}
```

- [ ] **Step 4: Route `Message::Surface`**

In `App::update`, add this match arm before the flat surface arms:

```rust
Message::Surface(event) => self.handle_surface_event(event),
```

- [ ] **Step 5: Convert view and module callbacks**

In `crates/niribar/src/app/view.rs`, replace surface-related messages:

```rust
Message::ModuleHoverEnter(id.clone(), window)
Message::ModuleHoverExit(id.clone(), window)
```

with:

```rust
Message::Surface(messages::SurfaceEvent::ModuleHoverEnter(id.clone(), window))
Message::Surface(messages::SurfaceEvent::ModuleHoverExit(id.clone(), window))
```

Keep `Message::ModuleClick`, `Message::ModuleScrollUp`, and `Message::ModuleScrollDown` flat until Task 8.

- [ ] **Step 6: Convert subscription outputs**

In `crates/niribar/src/app/subscriptions.rs`, change keyboard focus and animation subscription mappings so they emit wrapped surface messages. For example:

```rust
Message::Surface(super::messages::SurfaceEvent::Tick)
```

and:

```rust
Message::Surface(super::messages::SurfaceEvent::ToggleExpand)
```

- [ ] **Step 7: Remove flat surface arms from `App::update`**

After all call sites compile, remove these flat arms from `Message` and `App::update`: `BarClick`, `Tick`, `BarHoverEnter`, `BarHoverExit`, `ModuleHoverEnter`, `ModuleHoverExit`, `BarFocusGained`, `BarFocusLost`, `EscapePressed`, `FocusMoveNext`, `FocusMovePrev`, `FocusNextZone`, `FocusPrevZone`, `KeyboardActivate`, `KeyboardSecondaryActivate`, `ToggleExpand`, `CursorMoved`, and `WindowSized`.

- [ ] **Step 8: Run focused tests**

Run: `rtk cargo test wrapped_module_hover_enter_sets_hovered_module hover_enter_sets_hovered_module tick_advances_expand_progress focus_move_next_advances_to_next_module`

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/surfaces.rs crates/niribar/src/app/animation.rs crates/niribar/src/app/focus.rs crates/niribar/src/app/navigation.rs crates/niribar/src/app/view.rs crates/niribar/src/app/subscriptions.rs
rtk git commit -m "refactor(app): route surface events"
```

## Task 4: Extract MenuSystem And Popup Ownership

**Files:**
- Modify: `crates/niribar/src/app/menu.rs`
- Modify: `crates/niribar/src/app/surfaces.rs`
- Modify: `crates/niribar/src/app/view.rs`
- Modify: `crates/niribar/src/app.rs`
- Test: `crates/niribar/src/app/menu.rs`
- Test: `crates/niribar/src/surface/menu_popup.rs`

- [ ] **Step 1: Move menu tests into `crates/niribar/src/app/menu.rs`**

Move these tests from `crates/niribar/src/app.rs` into `crates/niribar/src/app/menu.rs`: `menu_item_hover_enter_sets_hovered_item`, `menu_item_hover_exit_clears_hovered_item`, `menu_item_hover_enter_different_popups_are_independent`, `menu_item_hover_enter_on_second_item_replaces_first_without_exit`, `escape_in_popup_fires_menu_closed`, `arrow_down_in_popup_focuses_first_item`, `arrow_up_in_popup_wraps_to_last`, `popup_skips_separators_and_disabled_items`, `menu_closed_clears_bar_focus_on_all_surfaces`, `close_all_menu_popups_removes_popup_and_focus_maps`.

Keep the existing tests in `crates/niribar/src/surface/menu_popup.rs`; do not move them unless a compile error forces helper visibility changes.

- [ ] **Step 2: Run moved tests before extraction**

Run: `rtk cargo test menu_item_hover_enter_sets_hovered_item close_all_menu_popups_removes_popup_and_focus_maps popup_size_single_item`

Expected: PASS.

- [ ] **Step 3: Define menu model and activation token**

At the top of `crates/niribar/src/app/menu.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MenuActivation {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) item_id: i32,
}

#[derive(Debug, Clone)]
pub(super) struct MenuModel {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) tree: crate::surface::menu_popup::MenuTree,
    pub(super) bar_name: String,
}

impl MenuModel {
    pub(super) fn activation_for(&self, item_id: i32) -> MenuActivation {
        MenuActivation {
            service: self.service.clone(),
            menu_path: self.menu_path.clone(),
            item_id,
        }
    }
}
```

The activation token format is exactly `(service, menu_path, item_id)`; do not encode this into a string.

- [ ] **Step 4: Add `MenuSystem`**

In `crates/niribar/src/app/menu.rs`, add:

```rust
#[derive(Default)]
pub(super) struct MenuSystem {
    hovered: std::collections::HashMap<iced::window::Id, String>,
    focused: std::collections::HashMap<iced::window::Id, String>,
}

impl MenuSystem {
    pub(super) fn hovered_item(&self, window: iced::window::Id) -> Option<&str> {
        self.hovered.get(&window).map(String::as_str)
    }

    pub(super) fn focused_item(&self, window: iced::window::Id) -> Option<&str> {
        self.focused.get(&window).map(String::as_str)
    }

    pub(super) fn hover_enter(&mut self, window: iced::window::Id, item_id: String) {
        self.hovered.insert(window, item_id);
    }

    pub(super) fn hover_exit(&mut self, window: iced::window::Id) {
        self.hovered.remove(&window);
    }

    pub(super) fn clear_window(&mut self, window: iced::window::Id) {
        self.hovered.remove(&window);
        self.focused.remove(&window);
    }
}
```

- [ ] **Step 5: Move popup windows into `SurfaceManager`**

Add these methods to `SurfaceManager` in `crates/niribar/src/app/surfaces.rs`:

```rust
pub(super) fn popup(&self, window: iced::window::Id) -> Option<&PopupMeta> {
    self.popups.get(&window)
}

pub(super) fn popup_values(&self) -> impl Iterator<Item = &PopupMeta> {
    self.popups.values()
}

pub(super) fn popup_ids(&self) -> Vec<iced::window::Id> {
    self.popups.keys().copied().collect()
}

pub(super) fn insert_popup(&mut self, id: iced::window::Id, meta: PopupMeta) {
    self.popups.insert(id, meta);
}

pub(super) fn remove_popup(&mut self, id: iced::window::Id) -> Option<PopupMeta> {
    self.popups.remove(&id)
}
```

- [ ] **Step 6: Add `menu` field to `App`**

In `App`, replace:

```rust
menu_popup_hovered: HashMap<iced::window::Id, String>,
menu_focused_item: HashMap<iced::window::Id, String>,
```

with:

```rust
menu: menu::MenuSystem,
```

Initialize it in `App::boot`:

```rust
menu: menu::MenuSystem::default(),
```

- [ ] **Step 7: Route `MenuEvent`**

Add this to `crates/niribar/src/app/menu.rs`:

```rust
impl App {
    pub(super) fn handle_menu_event(
        &mut self,
        event: super::messages::MenuEvent,
    ) -> Task<Message> {
        use super::messages::MenuEvent;

        match event {
            MenuEvent::ItemSelected {
                service,
                menu_item_id,
                menu_path,
            } => self.handle_menu_item_selected(service, menu_item_id, menu_path),
            MenuEvent::Closed => self.handle_menu_closed(),
            MenuEvent::ItemHoverEnter { window, item_id } => {
                self.menu.hover_enter(window, item_id);
                Task::none()
            }
            MenuEvent::ItemHoverExit(window) => {
                self.menu.hover_exit(window);
                Task::none()
            }
            MenuEvent::LayoutChanged { service, menu_path } => {
                self.handle_menu_layout_changed(service, menu_path)
            }
            MenuEvent::Open {
                service,
                menu_path,
                tree,
                bar_name,
            } => self.open_menu_popup(service, menu_path, tree, bar_name),
        }
    }
}
```

Add `Message::Menu(event) => self.handle_menu_event(event),` to `App::update`.

- [ ] **Step 8: Move open and layout handlers out of `App::update` arms**

Move the current `Message::OpenMenu` arm body into:

```rust
pub(super) fn open_menu_popup(
    &mut self,
    service: String,
    menu_path: zbus::zvariant::OwnedObjectPath,
    tree: crate::surface::menu_popup::MenuTree,
    bar_name: String,
) -> Task<Message> {
    let existing_ids = self.surfaces.popup_ids();
    let mut tasks: Vec<Task<Message>> = existing_ids
        .into_iter()
        .map(|id| {
            self.surfaces.remove_popup(id);
            self.menu.clear_window(id);
            Task::done(Message::RemoveWindow(id))
        })
        .collect();

    let settings = iced_layershell::reexport::NewLayerShellSettings {
        anchor: iced_layershell::reexport::Anchor::Left
            | iced_layershell::reexport::Anchor::Right
            | iced_layershell::reexport::Anchor::Top
            | iced_layershell::reexport::Anchor::Bottom,
        exclusive_zone: Some(-1),
        size: Some((0, 0)),
        layer: iced_layershell::reexport::Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
        ..Default::default()
    };
    let (popup_id, open_task) = Message::layershell_open(settings);
    self.surfaces.insert_popup(
        popup_id,
        super::surfaces::PopupMeta {
            service,
            menu_path,
            tree,
            bar_name,
        },
    );
    tasks.push(open_task);
    Task::batch(tasks)
}
```

Move the current `Message::MenuLayoutChanged` arm body into:

```rust
pub(super) fn handle_menu_layout_changed(
    &mut self,
    service: String,
    menu_path: zbus::zvariant::OwnedObjectPath,
) -> Task<Message> {
    let conn = self.shared.dbus.session.clone();
    let existing = self
        .surfaces
        .popup_values()
        .find(|popup| popup.service == service && popup.menu_path == menu_path);
    let (anchor, screen_size, existing_bar_name) = match existing {
        Some(popup) => (popup.tree.anchor, popup.tree.screen_size, popup.bar_name.clone()),
        None => return Task::none(),
    };
    if let Some(conn) = conn {
        let svc = service.clone();
        let mp = menu_path.clone();
        return Task::perform(
            async move { crate::dbus::sni::host::fetch_menu(&conn, &svc, &mp).await },
            move |tree_opt| {
                if let Some(mut tree) = tree_opt {
                    tree.anchor = anchor;
                    tree.screen_size = screen_size;
                    Message::Menu(super::messages::MenuEvent::Open {
                        service,
                        menu_path,
                        tree,
                        bar_name: existing_bar_name,
                    })
                } else {
                    Message::Menu(super::messages::MenuEvent::Closed)
                }
            },
        );
    }
    Task::none()
}
```

- [ ] **Step 9: Update popup rendering**

In `crates/niribar/src/app/view.rs`, replace popup lookup and hover/focus lookup with:

```rust
if let Some(popup) = self.surfaces.popup(window) {
    use crate::surface::menu_popup::view_menu_popup;
    let hovered_item = self.menu.hovered_item(window);
    let focused_item = self.menu.focused_item(window);
    let menu = view_menu_popup(
        &popup.tree,
        popup.service.as_str(),
        &popup.menu_path,
        window,
        hovered_item,
        focused_item,
    );
    let bar = Self::bar_for_surface_name(&self.config.bars, &popup.bar_name);

    let bar_size = bar.collapsed_width as f32;
    let (popup_w, popup_h) = crate::surface::menu_popup::popup_size(&popup.tree);
    let screen_w = popup.tree.screen_size.0 as f32;
    let screen_h = popup.tree.screen_size.1 as f32;
    let raw_cursor_x = popup.tree.anchor.0 as f32;
    let raw_cursor_y = popup.tree.anchor.1 as f32;
    let cursor_x = if screen_w > 0.0 {
        raw_cursor_x.min(screen_w - popup_w as f32).max(0.0)
    } else {
        raw_cursor_x
    };
    let cursor_y = if screen_h > 0.0 {
        raw_cursor_y.min(screen_h - popup_h as f32).max(0.0)
    } else {
        raw_cursor_y
    };

    let (h_align, v_align, padding) = match &bar.edge {
        crate::config::Edge::Left => (
            iced::alignment::Horizontal::Left,
            iced::alignment::Vertical::Top,
            iced::Padding {
                top: cursor_y,
                left: bar_size,
                ..Default::default()
            },
        ),
        crate::config::Edge::Right => (
            iced::alignment::Horizontal::Right,
            iced::alignment::Vertical::Top,
            iced::Padding {
                top: cursor_y,
                right: bar_size,
                ..Default::default()
            },
        ),
        crate::config::Edge::Top => (
            iced::alignment::Horizontal::Left,
            iced::alignment::Vertical::Top,
            iced::Padding {
                top: bar_size,
                left: cursor_x,
                ..Default::default()
            },
        ),
        crate::config::Edge::Bottom => (
            iced::alignment::Horizontal::Left,
            iced::alignment::Vertical::Bottom,
            iced::Padding {
                bottom: bar_size,
                left: cursor_x,
                ..Default::default()
            },
        ),
    };

    let menu_sized = iced::widget::container(menu).width(popup_w).height(popup_h);

    return iced::widget::mouse_area(
        iced::widget::container(menu_sized)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .align_x(h_align)
            .align_y(v_align)
            .padding(padding),
    )
    .on_press(Message::Menu(super::messages::MenuEvent::Closed))
    .into();
}
```

Use `Message::Menu(messages::MenuEvent::Closed)` for click-outside close.

- [ ] **Step 10: Remove flat menu variants**

Remove `MenuItemSelected`, `MenuClosed`, `MenuItemHoverEnter`, `MenuItemHoverExit`, `MenuLayoutChanged`, and `OpenMenu` from `Message`. Replace each constructor with the matching `Message::Menu(messages::MenuEvent::{ItemSelected, Closed, ItemHoverEnter, ItemHoverExit, LayoutChanged, Open})` constructor.

- [ ] **Step 11: Run menu tests and verification gate**

Run: `rtk cargo test menu_item_hover_enter_sets_hovered_item close_all_menu_popups_removes_popup_and_focus_maps popup_size_single_item`

Expected: PASS.

Run: `rtk cargo test`

Expected: PASS.

Run: `rtk cargo clippy --all-targets -- -D warnings`

Expected: PASS.

Run: `rtk cargo build`

Expected: PASS.

- [ ] **Step 12: Manual smoke after MenuSystem**

Verify:

```text
Right-click tray menu opens.
Keyboard navigation moves through enabled menu rows.
Disabled rows and separators are skipped.
Click-outside closes the popup.
Menu item clicks execute.
```

- [ ] **Step 13: Commit**

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/menu.rs crates/niribar/src/app/surfaces.rs crates/niribar/src/app/view.rs crates/niribar/src/surface/menu_popup.rs
rtk git commit -m "refactor(app): move menu state into system"
```

## Task 5: Extract AppNiriState

**Files:**
- Modify: `crates/niribar/src/app/niri.rs`
- Modify: `crates/niribar/src/app.rs`
- Modify: `crates/niribar/src/app/view.rs`
- Test: `crates/niribar/src/app/niri.rs`

- [ ] **Step 1: Move niri tests into `crates/niribar/src/app/niri.rs`**

Move the Story 4.1, Story 4.2, and Story 4.4 tests from `crates/niribar/src/app.rs` into `crates/niribar/src/app/niri.rs`: from `niri_connected_event_sets_connected` through `focused_window_state_not_updated_without_focused_window_module`, plus helpers `make_workspaces_app`, `make_workspace`, `make_focused_window_app`, and `make_window`.

- [ ] **Step 2: Run moved tests before extraction**

Run: `rtk cargo test niri_connected_event_sets_connected niri_workspaces_changed_stores_workspaces focused_window_state_updated_on_windows_changed`

Expected: PASS.

- [ ] **Step 3: Add `AppNiriState`**

In `crates/niribar/src/app/niri.rs`, add:

```rust
pub(super) struct AppNiriState {
    state: crate::niri::NiriState,
}

impl AppNiriState {
    pub(super) fn new() -> Self {
        Self {
            state: crate::niri::NiriState::default(),
        }
    }

    pub(super) fn state(&self) -> &crate::niri::NiriState {
        &self.state
    }

    pub(super) fn state_mut(&mut self) -> &mut crate::niri::NiriState {
        &mut self.state
    }
}
```

- [ ] **Step 4: Move niri field out of `SharedContext`**

In `App`, add:

```rust
niri: niri::AppNiriState,
```

Initialize it:

```rust
niri: niri::AppNiriState::new(),
```

Replace references:

```rust
self.shared.niri
```

with:

```rust
self.niri.state()
```

for reads and:

```rust
self.niri.state_mut()
```

for mutation.

- [ ] **Step 5: Route wrapped niri events**

In `crates/niribar/src/app/niri.rs`, add:

```rust
impl App {
    pub(super) fn handle_app_niri_event(
        &mut self,
        event: super::messages::AppNiriEvent,
    ) -> Task<Message> {
        match event {
            super::messages::AppNiriEvent::Ipc(event) => self.handle_niri_event(event),
        }
    }
}
```

In `App::update`, replace the flat `Message::Niri(event)` route with:

```rust
Message::Niri(event) => self.handle_app_niri_event(event),
```

In `crates/niribar/src/app/subscriptions.rs`, map the niri subscription to:

```rust
Message::Niri(super::messages::AppNiriEvent::Ipc(event))
```

- [ ] **Step 6: Update view focused-window derivation**

In `crates/niribar/src/app/view.rs`, replace `self.shared.niri` reads with `self.niri.state()`, for example:

```rust
match self.niri.state().focused_window_id {
    None => self.niri.state().active_window_title_for_output(&surface.output_name),
    Some(_) => {
        let focused_output = self.niri.state().focused_output();
        if focused_output.is_none() || focused_output == Some(surface.output_name.as_str()) {
            fw.title.clone()
        } else {
            self.niri.state().active_window_title_for_output(&surface.output_name)
        }
    }
}
```

- [ ] **Step 7: Remove compatibility niri field**

Remove `niri: crate::niri::NiriState` from `SharedContext`.

- [ ] **Step 8: Run niri tests and verification gate**

Run: `rtk cargo test niri_connected_event_sets_connected niri_workspaces_changed_stores_workspaces workspace_activated_updates_focused_pill focused_window_state_updated_on_windows_changed`

Expected: PASS.

Run: `rtk cargo test`

Expected: PASS.

Run: `rtk cargo clippy --all-targets -- -D warnings`

Expected: PASS.

Run: `rtk cargo build`

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/niri.rs crates/niribar/src/app/view.rs crates/niribar/src/app/subscriptions.rs crates/niribar/src/app/shared.rs
rtk git commit -m "refactor(app): move niri state into runtime owner"
```

## Task 6: Extract TrayState For SNI Source Data

**Files:**
- Modify: `crates/niribar/src/app/tray.rs`
- Modify: `crates/niribar/src/app.rs`
- Modify: `crates/niribar/src/app/messages.rs`
- Test: `crates/niribar/src/app/tray.rs`

- [ ] **Step 1: Move tray tests into `crates/niribar/src/app/tray.rs`**

Move these tests from `crates/niribar/src/app.rs` into `crates/niribar/src/app/tray.rs`: `tray_not_in_needs_dbus_when_no_tray_module`, `tray_updated_message_stores_items_in_shared_state`, `tray_snapshot_removes_existing_tray_icons_before_insert`, `tray_snapshot_handles_multiple_placeholders_after_existing_icons`, and `tray_keyboard_menu_request_without_source_window_is_noop`.

- [ ] **Step 2: Add failing routing test**

Add to `crates/niribar/src/app/tray.rs`:

```rust
#[test]
fn wrapped_tray_snapshot_updates_runtime_icons() {
    let mut config = crate::config::Config::default();
    config.bars[0].modules.bottom = vec!["tray".into()];
    let (mut app, _) = App::boot(config, vec![]);

    let item = crate::dbus::sni::host::TrayItem {
        service: "org.example.App".to_string(),
        object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
        title: "Example".to_string(),
        status: crate::dbus::sni::host::ItemStatus::Active,
        icon_name: Some("example".to_string()),
        icon_pixmap: Vec::new(),
        menu: None,
    };

    let _ = app.update(Message::Tray(super::messages::TrayRuntimeEvent::Host(
        crate::dbus::sni::host::TrayEvent::Snapshot(vec![item]),
    )));

    assert!(app.modules.iter().any(|(_, module)| module.id().0 == "tray:org.example.App"));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `rtk cargo test wrapped_tray_snapshot_updates_runtime_icons`

Expected: FAIL because `Message::Tray` is not routed.

- [ ] **Step 4: Add `TrayState`**

In `crates/niribar/src/app/tray.rs`, add:

```rust
#[derive(Default)]
pub(super) struct TrayState {
    items: std::collections::BTreeMap<String, crate::dbus::sni::host::TrayItem>,
}

impl TrayState {
    pub(super) fn apply_host_event(
        &mut self,
        event: &crate::dbus::sni::host::TrayEvent,
    ) -> Vec<crate::dbus::sni::host::TrayItem> {
        use crate::dbus::sni::host::{ItemStatus, TrayEvent};
        match event {
            TrayEvent::Snapshot(items) => {
                self.items.clear();
                for item in items {
                    if item.status != ItemStatus::Passive {
                        self.items.insert(item.service.clone(), item.clone());
                    }
                }
            }
            TrayEvent::ItemAdded(item) => {
                if item.status == ItemStatus::Passive {
                    self.items.remove(&item.service);
                } else {
                    self.items.insert(item.service.clone(), item.clone());
                }
            }
            TrayEvent::ItemRemoved(service) => {
                self.items.remove(service);
            }
        }
        self.items.values().cloned().collect()
    }
}
```

- [ ] **Step 5: Add `tray` field to `App`**

In `App`, add:

```rust
tray: tray::TrayState,
```

Initialize it:

```rust
tray: tray::TrayState::default(),
```

- [ ] **Step 6: Route tray events**

In `crates/niribar/src/app/tray.rs`, add:

```rust
impl App {
    pub(super) fn handle_tray_runtime_event(
        &mut self,
        event: super::messages::TrayRuntimeEvent,
    ) -> Task<Message> {
        use super::messages::TrayRuntimeEvent;

        match event {
            TrayRuntimeEvent::Host(event) => self.handle_tray_event(event),
            TrayRuntimeEvent::Activate { service } => self.handle_tray_activate(service),
            TrayRuntimeEvent::MenuRequest {
                service,
                menu_path,
                source_window,
            } => self.handle_tray_menu_request(service, menu_path, source_window),
            TrayRuntimeEvent::MenuRequestKeyboard {
                service,
                menu_path,
                source_window,
            } => self.handle_tray_menu_request(service, menu_path, source_window),
            TrayRuntimeEvent::MenuRequestWithBounds {
                service,
                menu_path,
                bounds,
                screen_size,
                source_window,
            } => self.handle_tray_menu_request_with_bounds(
                service,
                menu_path,
                bounds,
                screen_size,
                source_window,
            ),
            TrayRuntimeEvent::SecondaryActivate { service } => {
                self.handle_tray_secondary_activate(service)
            }
            TrayRuntimeEvent::Scroll {
                service,
                delta,
                orientation,
            } => self.handle_tray_scroll(service, delta, orientation),
        }
    }
}
```

Move the bodies from the current `App::update` tray arms into methods with these signatures:

```rust
pub(super) fn handle_tray_activate(&self, service: String) -> Task<Message>
pub(super) fn handle_tray_secondary_activate(&self, service: String) -> Task<Message>
pub(super) fn handle_tray_scroll(&self, service: String, delta: i32, orientation: String) -> Task<Message>
pub(super) fn handle_tray_menu_request(
    &self,
    service: String,
    menu_path: Option<zbus::zvariant::OwnedObjectPath>,
    source_window: Option<iced::window::Id>,
) -> Task<Message>
pub(super) fn handle_tray_menu_request_with_bounds(
    &self,
    service: String,
    menu_path: Option<zbus::zvariant::OwnedObjectPath>,
    bounds: Option<iced::Rectangle>,
    screen_size: (u32, u32),
    source_window: Option<iced::window::Id>,
) -> Task<Message>
```

In the menu-fetch result mapper, return:

```rust
Message::Menu(super::messages::MenuEvent::Open {
    service: service.clone(),
    menu_path: menu_path.clone(),
    tree,
    bar_name: bar_name.clone(),
})
```

and use:

```rust
Message::Tray(super::messages::TrayRuntimeEvent::Activate {
    service: service.clone(),
})
```

for the fallback.

- [ ] **Step 7: Update `handle_tray_event` to update source data first**

At the start of `handle_tray_event`, add:

```rust
let active_items = self.tray.apply_host_event(&event);
```

Use `active_items` for the `Snapshot` case instead of filtering the event-local vector. Preserve existing dynamic tray icon insertion behavior in this task; `ModuleRuntime` will own it in Task 8.

- [ ] **Step 8: Replace flat tray variants**

Remove flat `TrayEvent`, `TrayActivate`, `TrayMenuRequest`, `TrayMenuRequestKeyboard`, `TrayMenuRequestWithBounds`, `TraySecondaryActivate`, and `TrayScroll` from `Message`. Replace all call sites in `crates/niribar/src/modules/tray.rs`, `crates/niribar/src/app/focus.rs`, and `crates/niribar/src/app/subscriptions.rs` with the matching `Message::Tray(messages::TrayRuntimeEvent::{Host, Activate, MenuRequest, MenuRequestKeyboard, MenuRequestWithBounds, SecondaryActivate, Scroll})` constructor.

- [ ] **Step 9: Run tray tests and verification gate**

Run: `rtk cargo test wrapped_tray_snapshot_updates_runtime_icons tray_snapshot_removes_existing_tray_icons_before_insert tray_keyboard_menu_request_without_source_window_is_noop`

Expected: PASS.

Run: `rtk cargo test`

Expected: PASS.

Run: `rtk cargo clippy --all-targets -- -D warnings`

Expected: PASS.

Run: `rtk cargo build`

Expected: PASS.

- [ ] **Step 10: Manual smoke after TrayState**

Verify:

```text
Tray icons add/remove as apps start and stop.
Left-click tray icon activates the item.
Middle-click tray icon sends secondary activation.
Scroll over tray icon sends SNI scroll.
Right-click tray menu still opens.
```

- [ ] **Step 11: Commit**

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/tray.rs crates/niribar/src/app/messages.rs crates/niribar/src/modules/tray.rs crates/niribar/src/app/focus.rs crates/niribar/src/app/subscriptions.rs
rtk git commit -m "refactor(app): move tray source state into owner"
```

## Task 7: Extract ModuleRuntime

**Files:**
- Create: `crates/niribar/src/app/module_runtime.rs`
- Modify: `crates/niribar/src/app.rs`
- Modify: `crates/niribar/src/app/module_events.rs`
- Modify: `crates/niribar/src/app/module_state.rs`
- Modify: `crates/niribar/src/app/dynamic_modules.rs`
- Modify: `crates/niribar/src/app/layout.rs`
- Modify: `crates/niribar/src/app/view.rs`
- Modify: `crates/niribar/src/app/niri.rs`
- Modify: `crates/niribar/src/app/tray.rs`
- Modify: `crates/niribar/src/app/subscriptions.rs`
- Test: `crates/niribar/src/app/module_runtime.rs`

- [ ] **Step 1: Move module tests into `crates/niribar/src/app/module_runtime.rs`**

Move module construction and module state tests from `crates/niribar/src/app.rs` into the new file: `boot_modules_empty_with_no_module_config`, `battery_state_updates_all_battery_instances`, `undeclared_known_type_in_bar_modules_uses_defaults`, `undeclared_unknown_name_in_bar_modules_is_skipped`, `clock_module_instantiated_from_registry`, `script_module_instantiated_from_registry`, `script_output_event_updates_state`, `repeated_clock_tick_keeps_existing_state`, `repeated_volume_state_keeps_existing_state`, `button_module_instantiated_from_registry`, `button_ready_event_updates_state`, `module_click_no_match_does_not_panic`, `script_failed_event_retains_previous_state`, battery/network/volume state tests, workspace rebuild dynamic module tests, and tray dynamic icon tests.

- [ ] **Step 2: Add failing wrapped module event test**

Add to `crates/niribar/src/app/module_runtime.rs`:

```rust
#[test]
fn wrapped_script_output_updates_runtime_state() {
    let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
    let id = crate::modules::ModuleId::new("script");

    let _ = app.update(Message::Modules(super::messages::ModuleRuntimeEvent::Module(
        id.clone(),
        crate::modules::ModuleEvent::Script(crate::modules::script::ScriptEvent::Output(
            "hello".into(),
        )),
    )));

    match app.modules.state(&id) {
        Some(crate::modules::ModuleState::Script(s)) => assert_eq!(s.output, "hello"),
        other => panic!("expected Script state, got {:?}", other),
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `rtk cargo test wrapped_script_output_updates_runtime_state`

Expected: FAIL because `ModuleRuntime` does not exist and `app.modules.state()` is unavailable.

- [ ] **Step 4: Add `ModuleRuntime`**

Create `crates/niribar/src/app/module_runtime.rs`:

```rust
use std::collections::HashMap;

use crate::modules::{Module, ModuleId, ModuleState, Zone};

pub(super) struct ModuleRuntime {
    store: super::module_store::ModuleStore,
    states: HashMap<ModuleId, ModuleState>,
}

impl ModuleRuntime {
    pub(super) fn from_modules(modules: Vec<(Zone, Box<dyn Module>)>) -> Self {
        Self {
            store: super::module_store::ModuleStore::from_modules(modules),
            states: HashMap::new(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.store.len()
    }

    pub(super) fn state(&self, id: &ModuleId) -> Option<&ModuleState> {
        self.states.get(id)
    }

    pub(super) fn set_state(&mut self, id: ModuleId, state: ModuleState) {
        if self.states.get(&id) != Some(&state) {
            self.states.insert(id, state);
        }
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &(Zone, Box<dyn Module>)> {
        self.store.iter()
    }

    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = &mut (Zone, Box<dyn Module>)> {
        self.store.iter_mut()
    }

    pub(super) fn push(&mut self, zone: Zone, module: Box<dyn Module>) {
        self.store.push(zone, module);
    }

    pub(super) fn insert(&mut self, index: usize, item: (Zone, Box<dyn Module>)) {
        self.store.insert(index, item);
    }

    pub(super) fn retain<F>(&mut self, keep: F)
    where
        F: FnMut(&(Zone, Box<dyn Module>)) -> bool,
    {
        self.store.retain(keep);
    }

    pub(super) fn ids_by_kind(&self, kind: crate::modules::ModuleKind) -> &[ModuleId] {
        self.store.ids_by_kind(kind)
    }

    pub(super) fn get(&self, id: &ModuleId) -> Option<&dyn Module> {
        self.store.get(id)
    }
}
```

Add `mod module_runtime;` to `crates/niribar/src/app.rs`.

- [ ] **Step 5: Move module fields**

In `App`, replace:

```rust
modules: module_store::ModuleStore,
```

with:

```rust
modules: module_runtime::ModuleRuntime,
```

Initialize it:

```rust
modules: module_runtime::ModuleRuntime::from_modules(modules),
```

Replace `self.shared.modules.get(&id)` with `self.modules.state(&id)` and replace `self.shared.modules.insert(id, state)` with `self.modules.set_state(id, state)`.

- [ ] **Step 6: Route `ModuleRuntimeEvent`**

In `crates/niribar/src/app/module_events.rs`, add:

```rust
impl App {
    pub(super) fn handle_module_runtime_event(
        &mut self,
        event: super::messages::ModuleRuntimeEvent,
    ) -> Task<Message> {
        use super::messages::ModuleRuntimeEvent;

        match event {
            ModuleRuntimeEvent::Module(id, event) => self.handle_module_event(id, event),
            ModuleRuntimeEvent::ModuleClick(id, _) => {
                self.dispatch_module_interaction(&id, InteractionEvent::Clicked)
            }
            ModuleRuntimeEvent::ModuleScrollUp(id, _) => {
                self.dispatch_module_interaction(&id, InteractionEvent::ScrollUp)
            }
            ModuleRuntimeEvent::ModuleScrollDown(id, _) => {
                self.dispatch_module_interaction(&id, InteractionEvent::ScrollDown)
            }
            ModuleRuntimeEvent::Battery(event) => self.handle_battery_event(event),
            ModuleRuntimeEvent::Network(event) => self.handle_network_event(event),
            ModuleRuntimeEvent::Volume(event) => self.handle_volume_event(event),
        }
    }
}
```

Add to `App::update`:

```rust
Message::Modules(event) => self.handle_module_runtime_event(event),
```

- [ ] **Step 7: Replace flat module message constructors**

Replace `Message::Module`, `Message::ModuleClick`, `Message::ModuleScrollUp`, `Message::ModuleScrollDown`, top-level `Message::Battery`, `Message::Network`, and `Message::Volume` constructors with the matching `Message::Modules(messages::ModuleRuntimeEvent::{Module, ModuleClick, ModuleScrollUp, ModuleScrollDown, Battery, Network, Volume})` constructor. Update `crates/niribar/src/app/view.rs`, modules' subscriptions, and tests.

Keep `Message::VolumeAdjust` top-level because it executes an external command and does not mutate module state directly.

- [ ] **Step 8: Move dynamic module rebuild ownership**

Keep the existing methods on `App` for this task, but change their internal storage calls to `self.modules.*`. The methods are `rebuild_workspace_item_modules`, `handle_tray_event`, `remove_tray_icon`, `tray_insert_position`, `clear_surface_module_refs_with_prefix`, and `effective_module_zones_for_bar`.

Do not introduce a second dynamic rebuild abstraction in this task.

- [ ] **Step 9: Remove compatibility module state field**

Remove `modules: HashMap<ModuleId, ModuleState>` from `SharedContext`.

- [ ] **Step 10: Run module tests and verification gate**

Run: `rtk cargo test wrapped_script_output_updates_runtime_state script_output_event_updates_state battery_state_changed_updates_module workspace_rebuild_handles_multiple_placeholders_after_existing_items tray_snapshot_handles_multiple_placeholders_after_existing_icons`

Expected: PASS.

Run: `rtk cargo test`

Expected: PASS.

Run: `rtk cargo clippy --all-targets -- -D warnings`

Expected: PASS.

Run: `rtk cargo build`

Expected: PASS.

- [ ] **Step 11: Manual smoke after ModuleRuntime**

Verify:

```text
Battery, network, volume, clock, script, and button modules still update/respond.
Workspace switching updates focus highlight.
Focused-window title updates.
Tray icons still add/remove.
```

- [ ] **Step 12: Commit**

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/module_runtime.rs crates/niribar/src/app/module_events.rs crates/niribar/src/app/module_state.rs crates/niribar/src/app/dynamic_modules.rs crates/niribar/src/app/layout.rs crates/niribar/src/app/view.rs crates/niribar/src/app/niri.rs crates/niribar/src/app/tray.rs crates/niribar/src/app/subscriptions.rs crates/niribar/src/app/shared.rs
rtk git commit -m "refactor(app): move modules into runtime owner"
```

## Task 8: Make `App::update` A Shallow Router

**Files:**
- Modify: `crates/niribar/src/app.rs`
- Modify: `crates/niribar/src/app/messages.rs`
- Test: `crates/niribar/src/app.rs`

- [ ] **Step 1: Add router regression test**

Add to `crates/niribar/src/app.rs` tests:

```rust
#[test]
fn app_update_routes_wrapped_tick_to_surface_owner() {
    let (mut app, _) = App::boot(make_config(), vec![]);
    let wid = test_wid();
    app.surface_mut(wid).expand = crate::surface::expand::ExpandState::Expanding(0.0);

    let _ = app.update(Message::Surface(messages::SurfaceEvent::Tick));

    assert!(matches!(
        app.surface(wid).unwrap().expand,
        crate::surface::expand::ExpandState::Expanding(progress) if progress > 0.0
    ));
}
```

- [ ] **Step 2: Run test**

Run: `rtk cargo test app_update_routes_wrapped_tick_to_surface_owner`

Expected: PASS.

- [ ] **Step 3: Reduce `App::update` to routing**

In `crates/niribar/src/app.rs`, replace the long `match message` body with:

```rust
match message {
    Message::Surface(event) => self.handle_surface_event(event),
    Message::Menu(event) => self.handle_menu_event(event),
    Message::Modules(event) => self.handle_module_runtime_event(event),
    Message::Niri(event) => self.handle_app_niri_event(event),
    Message::Tray(event) => self.handle_tray_runtime_event(event),
    Message::VolumeAdjust { step, up } => self.handle_volume_adjust(step, up),
    Message::DbusReady(connections) => {
        self.shared.dbus = connections;
        Task::none()
    }
    Message::NiriFocusWorkspace { id } => {
        tokio::task::spawn_blocking(move || {
            if let Ok(mut socket) = niri_ipc::socket::Socket::connect() {
                let _ = socket.send(niri_ipc::Request::Action(
                    niri_ipc::Action::FocusWorkspace {
                        reference: niri_ipc::WorkspaceReferenceArg::Id(id),
                    },
                ));
            }
        });
        Task::none()
    }
    Message::PerfReport => {
        if let Some(perf) = &self.perf {
            eprintln!(
                "{}",
                crate::app::perf::format_snapshot(&perf.snapshot_and_reset())
            );
        }
        Task::none()
    }
    _ => Task::none(),
}
```

The `_ => Task::none()` arm remains for generated layer-shell variants only.

- [ ] **Step 4: Tighten `perf_label`**

Replace old flat labels with wrapped labels. Keep layer-shell fallback:

```rust
Message::Surface(super::messages::SurfaceEvent::Tick) => "Surface::Tick",
Message::Surface(super::messages::SurfaceEvent::BarClick(_)) => "Surface::BarClick",
Message::Surface(super::messages::SurfaceEvent::BarHoverEnter(_)) => "Surface::BarHoverEnter",
Message::Menu(_) => "Menu",
Message::Modules(super::messages::ModuleRuntimeEvent::Module(_, ModuleEvent::Clock(_))) => {
    "Module::Clock"
}
Message::Modules(super::messages::ModuleRuntimeEvent::Module(_, ModuleEvent::Script(_))) => {
    "Module::Script"
}
Message::Modules(super::messages::ModuleRuntimeEvent::Module(_, ModuleEvent::Button(_))) => {
    "Module::Button"
}
Message::Niri(super::messages::AppNiriEvent::Ipc(crate::niri::NiriEvent::Connected)) => {
    "Niri::Connected"
}
Message::Tray(_) => "Tray",
Message::DbusReady(_) => "DbusReady",
Message::PerfReport => "PerfReport",
_ => "LayerShell",
```

Add the specific niri IPC event labels that existed before by matching `AppNiriEvent::Ipc(crate::niri::NiriEvent::Event(event))` and reusing the old inner match.

- [ ] **Step 5: Remove obsolete imports and fields**

Remove unused imports from `crates/niribar/src/app.rs`: direct `HashMap` if no longer needed, `SurfaceState`, direct `NiriState`, direct battery/network/volume event imports if wrapped. Run `rtk cargo clippy --all-targets -- -D warnings` and remove every unused import it reports.

- [ ] **Step 6: Run full verification gate**

Run: `rtk cargo test`

Expected: PASS.

Run: `rtk cargo clippy --all-targets -- -D warnings`

Expected: PASS.

Run: `rtk cargo build`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/messages.rs
rtk git commit -m "refactor(app): make update a domain router"
```

## Task 9: Final Cleanup And Ownership Boundaries

**Files:**
- Modify: `crates/niribar/src/app.rs`
- Modify: `crates/niribar/src/app/shared.rs`
- Modify: `crates/niribar/src/app/surfaces.rs`
- Modify: `crates/niribar/src/app/menu.rs`
- Modify: `crates/niribar/src/app/niri.rs`
- Modify: `crates/niribar/src/app/tray.rs`
- Modify: `crates/niribar/src/app/module_runtime.rs`
- Modify: `crates/niribar/src/app/view.rs`
- Test: all touched modules

- [ ] **Step 1: Assert final `App` shape**

Change `App` in `crates/niribar/src/app.rs` to exactly this ownership shape, allowing `pub(super)` visibility only where tests require it:

```rust
pub(crate) struct App {
    config: Config,
    shared: shared::SharedContext,
    surfaces: surfaces::SurfaceManager,
    menu: menu::MenuSystem,
    modules: module_runtime::ModuleRuntime,
    niri: niri::AppNiriState,
    tray: tray::TrayState,
    perf: Option<perf::PerfTracker>,
}
```

- [ ] **Step 2: Assert final `SharedContext` shape**

Make `crates/niribar/src/app/shared.rs` exactly:

```rust
#[derive(Default)]
pub(super) struct SharedContext {
    pub(super) dbus: crate::dbus::DbusConnections,
}

impl SharedContext {
    pub(super) fn new() -> Self {
        Self {
            dbus: crate::dbus::DbusConnections::default(),
        }
    }
}
```

- [ ] **Step 3: Remove temporary App shims only if all callers have moved**

Search:

Run: `rtk rg -n "fn surface_mut|fn surface\\(|self\\.surface_mut\\(|self\\.surface\\(" crates/niribar/src/app.rs crates/niribar/src/app`

Expected: only intentional `App` shims and necessary callers remain.

If every caller can use `self.surfaces.surface_mut` and `self.surfaces.surface` directly, remove the shims from `crates/niribar/src/app/surfaces.rs`. If doing so creates large unrelated churn, keep the shims and document them with:

```rust
// Thin compatibility accessors while App remains the iced root.
```

- [ ] **Step 4: Remove dead flat variants**

Search:

Run: `rtk rg -n "Message::(BarClick|Tick|BarHoverEnter|BarHoverExit|ModuleHoverEnter|ModuleHoverExit|ModuleClick|ModuleScrollUp|ModuleScrollDown|TrayEvent|TrayActivate|MenuClosed|OpenMenu|Battery\\(|Network\\(|Volume\\()" src`

Expected: no matches except comments in historical docs. Replace any source match with the wrapped event form before continuing.

- [ ] **Step 5: Run formatting and verification**

Run: `rtk cargo fmt`

Expected: command exits 0.

Run: `rtk cargo test`

Expected: PASS.

Run: `rtk cargo clippy --all-targets -- -D warnings`

Expected: PASS.

Run: `rtk cargo build`

Expected: PASS.

- [ ] **Step 6: Final manual smoke**

Verify:

```text
Bar renders on every configured output.
Hover/click expand toggle works.
Tray icons add/remove as apps start and stop.
Right-click tray menu opens, keyboard navigation works, click-outside closes, and menu items execute.
Workspace switching updates focus highlight.
Focused-window title updates.
Battery, network, volume, clock, script, and button modules still update/respond.
```

- [ ] **Step 7: Commit**

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/shared.rs crates/niribar/src/app/surfaces.rs crates/niribar/src/app/menu.rs crates/niribar/src/app/niri.rs crates/niribar/src/app/tray.rs crates/niribar/src/app/module_runtime.rs crates/niribar/src/app/view.rs
rtk git commit -m "refactor(app): finalize runtime ownership split"
```

## Final Verification

Run the complete gate after the final cleanup commit:

```bash
rtk cargo fmt --check
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

Expected: every command exits 0.

Run this source scan:

```bash
rtk rg -n "SharedState|shared\\.modules|shared\\.niri|Message::OpenMenu|Message::MenuClosed|Message::TrayEvent|Message::Module\\(" src
```

Expected: no matches in Rust source.

## Self-Review

Spec coverage: this plan maps every requested owner. `SharedContext` ends with D-Bus handles only. `SurfaceManager` owns bar surfaces, pending surface keys, manual output mode, popup windows, and cursor anchor. `MenuSystem` owns menu hover/focus and menu model behavior while popup windows live in `SurfaceManager`. `AppNiriState` avoids a duplicate crate-level `NiriState` name. `TrayState` owns SNI item source data and emits menu/module effects through existing message routing. `ModuleRuntime` owns `ModuleStore`, per-module state, dynamic workspace/tray module rebuilding touch points, and module interactions.

Placeholder scan: no task uses TBD, TODO, omitted bodies, or comment-only bodies as implementation instructions. Where a body is moved from existing code, the exact target signature and message conversion are named.

Type consistency: final owner names are `SharedContext`, `SurfaceManager`, `MenuSystem`, `AppNiriState`, `TrayState`, and `ModuleRuntime`. Wrapped message names are `SurfaceEvent`, `MenuEvent`, `ModuleRuntimeEvent`, `AppNiriEvent`, and `TrayRuntimeEvent`.
