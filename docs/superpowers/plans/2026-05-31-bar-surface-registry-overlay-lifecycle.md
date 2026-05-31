# Bar Surface Registry Overlay Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the **Bar Surface Registry** the deep Module that owns **Panel**, **Menu**, and **Tooltip** overlay lifecycle rules.

**Architecture:** Move overlay exclusivity, closing-window tracking, pending **Menu** replacement, and overlay window classification into `SurfaceManager`. Keep `PanelRuntime` focused on **Panel Runtime** interaction state, and keep `overlay_adapter` as the Adapter that translates typed Registry effects into Iced tasks, D-Bus menu-close notifications, Menu focus cleanup, and **Panel Runtime** clearing.

**Tech Stack:** Rust 2024, iced, iced_layershell, zbus, existing `rtk cargo test` suite.

---

## File Structure

- Modify: `src/app/surfaces.rs`
  - Deepen `SurfaceManager` into the **Bar Surface Registry** for overlay lifecycle.
  - Add originating **Bar Surface Identity** to **Menu** metadata.
  - Track closing **Panel** and closing **Menu** windows in the Registry.
  - Return typed `RegistryEffect` values instead of plain `RemoveWindow` for overlay closures.
  - Answer window role classification for `surface_input_router`.
- Modify: `src/app/overlay_adapter.rs`
  - Translate typed Registry effects into `Task<Message>`.
  - Clear **Panel Runtime** when a `PanelClosed` effect is applied.
  - Clear Menu hover/focus and send D-Bus `closed` notifications when `MenuClosed` is applied.
- Modify: `src/app/panel_runtime.rs`
  - Remove closing **Panel** window tracking.
  - Keep **Panel Runtime** state only: active content/session, search focus, scroll, Module Detail Sessions, and Dmenu responder storage.
- Modify: `src/app/panel.rs`
  - Stop manually marking closing Panel windows.
  - Let the Registry discard pending **Menu** replacement when a **Panel** opens.
  - Let the Adapter clear **Panel Runtime** from typed Registry effects.
- Modify: `src/app/menu.rs`
  - Pass the originating **Bar Surface** window into **Menu** open requests.
  - Stop manually sending D-Bus `closed` events before Registry replacement; the Adapter sends them from `MenuClosed` effects.
  - Let pending **Menu** replacement live behind the Registry Interface.
- Modify: `src/app/messages.rs`
  - Add `origin_window` to `MenuEvent::Open`.
- Modify: `src/app/tray.rs`
  - Preserve the source **Bar Surface** when turning a tray **Menu** request into `MenuEvent::Open`.
- Modify: `src/app/surface_input_router.rs`
  - Keep the router pure. It should receive a role from the Registry, not construct one from app internals.

## Task 1: Add Red Registry Tests For Overlay Lifecycle Rules

**Files:**
- Modify: `src/app/surfaces.rs`

- [ ] **Step 1: Add a helper for menu metadata with a Bar Surface Identity**

Inside `#[cfg(test)] mod tests` in `src/app/surfaces.rs`, add:

```rust
fn popup_meta_for_identity(identity: BarSurfaceIdentity, service: &str) -> PopupMeta {
    PopupMeta {
        service: service.into(),
        menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
        tree: crate::surface::menu_popup::MenuTree {
            items: Vec::new(),
            anchor: (0, 0),
            screen_size: (1920, 1080),
        },
        origin: identity,
    }
}
```

Expected initial compile result: fail because `PopupMeta` still has `bar_name`, not `origin`.

- [ ] **Step 2: Add a failing test for Panel opening rules**

Add this test inside `src/app/surfaces.rs` tests:

```rust
#[test]
fn opening_panel_closes_menu_and_tooltip_discards_pending_menu_and_suspends_keyboard() {
    let origin = test_wid();
    let panel = test_wid();
    let old_menu = test_wid();
    let tooltip = test_wid();
    let identity = BarSurfaceIdentity::new("main", Some("DP-1".into()));
    let mut registry = SurfaceManager::new(
        [(origin, SurfaceState::new("DP-1".into(), "main".into()))].into(),
        HashMap::new(),
        true,
    );
    registry.insert_popup(old_menu, popup_meta_for_identity(identity.clone(), "org.old"));
    registry.insert_tooltip_popup(TooltipPopup {
        window: tooltip,
        target_window: origin,
        target_module: crate::modules::ModuleId::new("clock"),
        tree: tooltip_tree(crate::config::Edge::Top),
    });
    registry.defer_menu_for_test(popup_meta_for_identity(identity, "org.pending"));

    let effects = registry.open_panel_overlay(
        origin,
        panel,
        PanelMeta {
            tree: panel_tree(origin),
            search_input_id: iced::widget::Id::from("search"),
        },
    );

    assert!(registry.popup(old_menu).is_none());
    assert!(registry.tooltip_popup(tooltip).is_none());
    assert!(!registry.has_pending_menu_for_test());
    assert_eq!(
        effects,
        vec![
            RegistryEffect::MenuClosed {
                window: old_menu,
                meta: popup_meta_for_identity(
                    BarSurfaceIdentity::new("main", Some("DP-1".into())),
                    "org.old"
                ),
            },
            RegistryEffect::TooltipClosed { window: tooltip },
            RegistryEffect::SuspendBarKeyboardInteractivity,
        ]
    );
    assert!(registry.panel_popup(panel).is_some());
}
```

Expected initial result: fail because `defer_menu_for_test`, `has_pending_menu_for_test`, `RegistryEffect::MenuClosed`, and `RegistryEffect::TooltipClosed` do not exist.

- [ ] **Step 3: Add a failing test for Menu replacement sequencing**

Add:

```rust
#[test]
fn opening_menu_while_menu_is_live_defers_replacement_until_old_window_closes() {
    let old_menu = test_wid();
    let identity = BarSurfaceIdentity::new("main", Some("DP-1".into()));
    let old_meta = popup_meta_for_identity(identity.clone(), "org.old");
    let new_meta = popup_meta_for_identity(identity, "org.new");
    let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new(), true);
    registry.insert_popup(old_menu, old_meta.clone());

    let effects = registry.open_menu_overlay(test_wid(), new_meta.clone());

    assert!(registry.popup(old_menu).is_none());
    assert!(registry.has_pending_menu_for_test());
    assert_eq!(
        effects,
        vec![RegistryEffect::MenuClosed {
            window: old_menu,
            meta: old_meta,
        }]
    );

    let followup = registry.window_closed(old_menu);

    assert_eq!(followup, vec![RegistryEffect::PendingMenuReady(new_meta)]);
    assert!(!registry.has_pending_menu_for_test());
}
```

Expected initial result: fail because `window_closed` returns `Option<ClosedRegisteredWindow>` and pending readiness is not an effect.

- [ ] **Step 4: Add a failing test for Bar Surface Identity scoped cleanup**

Add:

```rust
#[test]
fn removing_bar_surface_closes_only_overlays_for_that_bar_surface_identity() {
    let removed_window = test_wid();
    let kept_window = test_wid();
    let removed_menu = test_wid();
    let kept_menu = test_wid();
    let mut registry = SurfaceManager::new(
        [
            (
                removed_window,
                SurfaceState::new("DP-1".into(), "main".into()),
            ),
            (
                kept_window,
                SurfaceState::new("HDMI-A-1".into(), "main".into()),
            ),
        ]
        .into(),
        HashMap::new(),
        true,
    );
    registry.insert_popup(
        removed_menu,
        popup_meta_for_identity(
            BarSurfaceIdentity::new("main", Some("DP-1".into())),
            "org.removed",
        ),
    );
    registry.insert_popup(
        kept_menu,
        popup_meta_for_identity(
            BarSurfaceIdentity::new("main", Some("HDMI-A-1".into())),
            "org.kept",
        ),
    );

    let effects = registry.reconcile_bar_surfaces(vec![DesiredBarSurface {
        identity: BarSurfaceIdentity::new("main", Some("HDMI-A-1".into())),
        bar_index: 0,
    }]);

    assert!(effects.contains(&BarSurfaceLifecycleEffect::RemoveWindow(removed_window)));
    assert!(registry.popup(removed_menu).is_none());
    assert!(registry.popup(kept_menu).is_some());
}
```

Expected initial result: fail because cleanup currently matches **Menu** overlays by **Bar Name**, so both `main` menus are closed.

- [ ] **Step 5: Run the focused red tests**

Run:

```bash
rtk cargo test app::surfaces::tests::opening_panel_closes_menu_and_tooltip_discards_pending_menu_and_suspends_keyboard -- --nocapture
rtk cargo test app::surfaces::tests::opening_menu_while_menu_is_live_defers_replacement_until_old_window_closes -- --nocapture
rtk cargo test app::surfaces::tests::removing_bar_surface_closes_only_overlays_for_that_bar_surface_identity -- --nocapture
```

Expected: fail to compile on the new `PopupMeta::origin` field and typed `RegistryEffect` variants.

## Task 2: Add Registry Metadata And Typed Effects

**Files:**
- Modify: `src/app/surfaces.rs`

- [ ] **Step 1: Replace `PopupMeta::bar_name` with `PopupMeta::origin`**

In `src/app/surfaces.rs`, change `PopupMeta` to:

```rust
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PopupMeta {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) tree: crate::surface::menu_popup::MenuTree,
    pub(super) origin: BarSurfaceIdentity,
}
```

- [ ] **Step 2: Add typed Registry effects**

Replace `RegistryEffect` with:

```rust
#[derive(Debug, Clone, PartialEq)]
pub(super) enum RegistryEffect {
    PanelClosed {
        window: iced::window::Id,
    },
    MenuClosed {
        window: iced::window::Id,
        meta: PopupMeta,
    },
    TooltipClosed {
        window: iced::window::Id,
    },
    PendingMenuReady(PopupMeta),
    SuspendBarKeyboardInteractivity,
    RestoreBarKeyboardInteractivity,
}
```

- [ ] **Step 3: Track closing overlays in the Registry**

Add fields to `SurfaceManager`:

```rust
pub(super) closing_panel_popups: HashSet<iced::window::Id>,
pub(super) closing_menu_popups: HashMap<iced::window::Id, PopupMeta>,
```

Remove the old `closing_menu_popups: HashSet<iced::window::Id>` field.

Initialize them in `SurfaceManager::new`:

```rust
closing_panel_popups: HashSet::new(),
closing_menu_popups: HashMap::new(),
```

- [ ] **Step 4: Add test-only pending helpers**

In the `impl SurfaceManager` block:

```rust
#[cfg(test)]
pub(super) fn defer_menu_for_test(&mut self, meta: PopupMeta) {
    self.pending_menu_popup = Some(meta);
}

#[cfg(test)]
pub(super) fn has_pending_menu_for_test(&self) -> bool {
    self.pending_menu_popup.is_some()
}
```

- [ ] **Step 5: Run the red tests again**

Run:

```bash
rtk cargo test app::surfaces::tests::opening_panel_closes_menu_and_tooltip_discards_pending_menu_and_suspends_keyboard -- --nocapture
```

Expected: still fail because close helpers still return `RemoveWindow` and do not carry metadata.

## Task 3: Implement Registry-Owned Overlay Closure Rules

**Files:**
- Modify: `src/app/surfaces.rs`

- [ ] **Step 1: Replace close helpers with typed closures**

Replace `close_panel_overlays_without_restore`, `close_menu_overlays`, `close_tooltip_overlays`, and `close_tooltips_for_surface` with helpers that return typed effects:

```rust
fn close_panel_overlays_without_restore(&mut self) -> Vec<RegistryEffect> {
    self.panel_popups
        .drain()
        .map(|(window, _)| {
            self.closing_panel_popups.insert(window);
            RegistryEffect::PanelClosed { window }
        })
        .collect()
}

pub(super) fn close_menu_overlays(&mut self) -> Vec<RegistryEffect> {
    self.popups
        .drain()
        .map(|(window, meta)| {
            self.closing_menu_popups.insert(window, meta.clone());
            RegistryEffect::MenuClosed { window, meta }
        })
        .collect()
}

fn close_tooltip_overlays(&mut self) -> Vec<RegistryEffect> {
    self.tooltip_popups
        .drain()
        .map(|(window, _)| RegistryEffect::TooltipClosed { window })
        .collect()
}

fn close_tooltips_for_surface(
    &mut self,
    target_surface: iced::window::Id,
) -> Vec<RegistryEffect> {
    let closing: Vec<_> = self
        .tooltip_popups
        .iter()
        .filter_map(|(window, popup)| {
            (popup.target_window == target_surface).then_some(*window)
        })
        .collect();
    for window in &closing {
        self.tooltip_popups.remove(window);
    }
    closing
        .into_iter()
        .map(|window| RegistryEffect::TooltipClosed { window })
        .collect()
}
```

- [ ] **Step 2: Make Panel opening discard pending Menu replacement**

Update `open_panel_overlay`:

```rust
pub(super) fn open_panel_overlay(
    &mut self,
    origin_surface: iced::window::Id,
    panel_window: iced::window::Id,
    meta: PanelMeta,
) -> Vec<RegistryEffect> {
    self.pending_menu_popup = None;
    let mut effects = self.close_tooltips_for_surface(origin_surface);
    effects.extend(self.close_menu_overlays());
    effects.extend(self.close_panel_overlays_without_restore());
    self.panel_popups.insert(panel_window, meta);
    effects.push(RegistryEffect::SuspendBarKeyboardInteractivity);
    effects
}
```

- [ ] **Step 3: Keep one-slot pending Menu replacement behind the Registry Interface**

Update `open_menu_overlay`:

```rust
pub(super) fn open_menu_overlay(
    &mut self,
    menu_window: iced::window::Id,
    meta: PopupMeta,
) -> Vec<RegistryEffect> {
    let mut effects = self.close_tooltip_overlays();
    effects.extend(self.close_panel_overlays_without_restore());
    if self.has_any_menu_popup() {
        self.pending_menu_popup = Some(meta);
        effects.extend(self.close_menu_overlays());
        return effects;
    }

    self.popups.insert(menu_window, meta);
    effects
}
```

- [ ] **Step 4: Return typed effects when Menu window close makes pending Menu ready**

Replace `window_closed` with:

```rust
pub(super) fn window_closed(&mut self, window: iced::window::Id) -> Vec<RegistryEffect> {
    if self.panel_popups.remove(&window).is_some() {
        return vec![RegistryEffect::PanelClosed { window }];
    }
    if let Some(meta) = self.popups.remove(&window) {
        return vec![RegistryEffect::MenuClosed { window, meta }];
    }
    if self.closing_panel_popups.remove(&window) {
        return Vec::new();
    }
    if self.closing_menu_popups.remove(&window).is_some() {
        return self
            .take_ready_pending_menu_popup()
            .map(RegistryEffect::PendingMenuReady)
            .into_iter()
            .collect();
    }
    if self.tooltip_popups.remove(&window).is_some() {
        return vec![RegistryEffect::TooltipClosed { window }];
    }
    Vec::new()
}
```

- [ ] **Step 5: Run Registry lifecycle tests**

Run:

```bash
rtk cargo test app::surfaces::tests::opening_panel_closes_menu_and_tooltip_discards_pending_menu_and_suspends_keyboard -- --nocapture
rtk cargo test app::surfaces::tests::opening_menu_while_menu_is_live_defers_replacement_until_old_window_closes -- --nocapture
```

Expected: pass after test helpers are adjusted for exact `PopupMeta` equality. Existing compile errors remain in Adapter/App call sites.

## Task 4: Move Closing Panel Tracking Out Of Panel Runtime

**Files:**
- Modify: `src/app/panel_runtime.rs`
- Modify: `src/app/surfaces.rs`
- Modify: `src/app/overlay_adapter.rs`
- Modify: `src/app/panel.rs`

- [ ] **Step 1: Remove closing Panel state from `PanelRuntime`**

In `src/app/panel_runtime.rs`, remove:

```rust
closing_windows: HashSet<iced::window::Id>,
```

Remove these methods:

```rust
pub(super) fn mark_closing_window(&mut self, window: iced::window::Id) {
    self.closing_windows.insert(window);
}

pub(super) fn is_closing_window(&self, window: iced::window::Id) -> bool {
    self.closing_windows.contains(&window)
}

pub(super) fn forget_closing_window(&mut self, window: iced::window::Id) {
    self.closing_windows.remove(&window);
}
```

Remove the `closing_windows_are_tracked_by_window_id` test from `src/app/panel_runtime.rs`.

- [ ] **Step 2: Add Registry closing Panel lookup**

In `src/app/surfaces.rs`, add:

```rust
pub(super) fn is_closing_panel_popup(&self, window: iced::window::Id) -> bool {
    self.closing_panel_popups.contains(&window)
}
```

- [ ] **Step 3: Stop manually marking closing Panels**

In `src/app/panel.rs`, remove calls to:

```rust
self.panel.mark_closing_window(window);
```

In `src/app/overlay_adapter.rs`, remove calls to:

```rust
self.panel.mark_closing_window(window);
```

In `src/app/surfaces.rs`, remove calls to:

```rust
self.panel.mark_closing_window(window);
self.panel.forget_closing_window(window);
```

- [ ] **Step 4: Run compile to expose remaining call sites**

Run:

```bash
rtk cargo test app::panel_runtime::tests -- --nocapture
```

Expected: compile errors identify any remaining references to removed `PanelRuntime` closing-window methods.

## Task 5: Convert The Adapter To Typed Registry Effects

**Files:**
- Modify: `src/app/overlay_adapter.rs`
- Modify: `src/app/menu.rs`
- Modify: `src/app/panel.rs`
- Modify: `src/app/surfaces.rs`

- [ ] **Step 1: Add D-Bus closed-event helper to the Adapter**

In `src/app/overlay_adapter.rs`, add:

```rust
fn send_menu_closed_event(
    conn: Option<zbus::Connection>,
    meta: super::surfaces::PopupMeta,
) {
    if let Some(conn) = conn {
        tokio::task::spawn(async move {
            let _ = crate::dbus::sni::host::send_menu_event(
                &conn,
                &meta.service,
                &meta.menu_path,
                0,
                "closed",
            )
            .await;
        });
    }
}
```

- [ ] **Step 2: Replace `apply_registry_effects` match arms**

Replace `apply_registry_effects` with:

```rust
pub(super) fn apply_registry_effects(
    &mut self,
    effects: Vec<RegistryEffect>,
) -> Vec<iced::Task<super::Message>> {
    let mut tasks = Vec::new();
    for effect in effects {
        match effect {
            RegistryEffect::PanelClosed { window } => {
                self.panel.clear();
                self.surfaces.remove_panel_popup(window);
                tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
            }
            RegistryEffect::MenuClosed { window, meta } => {
                self.surfaces.remove_popup(window);
                self.menu.clear_window(window);
                send_menu_closed_event(self.shared.dbus.session.clone(), meta);
                tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
            }
            RegistryEffect::TooltipClosed { window } => {
                self.surfaces.remove_tooltip_popup(window);
                tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
            }
            RegistryEffect::PendingMenuReady(meta) => {
                tasks.push(self.open_ready_menu_popup(meta));
            }
            RegistryEffect::SuspendBarKeyboardInteractivity => {
                tasks.extend(self.suspend_bar_keyboard_interactivity());
            }
            RegistryEffect::RestoreBarKeyboardInteractivity => {
                tasks.extend(self.restore_bar_keyboard_interactivity());
            }
        }
    }
    tasks
}
```

- [ ] **Step 3: Replace `open_ready_pending_menu_popup` with explicit ready-meta opening**

In `src/app/menu.rs`, replace:

```rust
pub(super) fn open_ready_pending_menu_popup(&mut self) -> Task<Message>
```

with:

```rust
pub(super) fn open_ready_menu_popup(&mut self, meta: super::surfaces::PopupMeta) -> Task<Message> {
    let bar = Self::bar_for_surface_identity(&self.config.bars, &meta.origin);
    let settings = menu_popup_window_settings(&bar.edge, bar.collapsed_width, &meta.tree);
    let (popup_id, open_task) = Message::popup_open(settings);
    self.surfaces.register_ready_menu_overlay(popup_id, meta);
    open_task
}
```

Add the lookup helper in `src/app/surfaces.rs`:

```rust
pub(super) fn bar_for_surface_identity<'a>(
    bars: &'a [crate::config::BarConfig],
    identity: &BarSurfaceIdentity,
) -> &'a crate::config::BarConfig {
    Self::bar_for_surface_name(bars, &identity.bar_name)
}
```

- [ ] **Step 4: Update surface window closed handling to apply Registry effects**

In `src/app/surfaces.rs`, update `apply_surface_input_effect`:

```rust
SurfaceInputEffect::WindowClosed(window)
| SurfaceInputEffect::ClosingPanelWindowClosed(window)
| SurfaceInputEffect::MenuWindowClosed(window) => {
    let effects = self.surfaces.window_closed(window);
    Task::batch(self.apply_registry_effects(effects))
}
```

- [ ] **Step 5: Run focused Adapter and Menu tests**

Run:

```bash
rtk cargo test app::overlay_adapter::tests -- --nocapture
rtk cargo test app::menu::tests::opening_menu_while_menu_is_open_waits_for_old_window_closed -- --nocapture
rtk cargo test app::menu::tests::pending_menu_opens_after_old_menu_window_closed -- --nocapture
```

Expected: pass after tests are updated for `PopupMeta::origin` and typed Registry effects.

## Task 6: Propagate Menu Origin Window And Bar Surface Identity

**Files:**
- Modify: `src/app/messages.rs`
- Modify: `src/app/tray.rs`
- Modify: `src/app/menu.rs`
- Modify: `src/app/surfaces.rs`
- Modify: tests in `src/app/menu.rs`, `src/app/view.rs`, `src/app/overlay_adapter.rs`, `src/app/surfaces.rs`

- [ ] **Step 1: Add origin window to Menu open messages**

Change `MenuEvent::Open` in `src/app/messages.rs`:

```rust
Open {
    service: String,
    menu_path: OwnedObjectPath,
    tree: MenuTree,
    origin_window: iced::window::Id,
},
```

- [ ] **Step 2: Preserve origin window in tray Menu open path**

In `src/app/tray.rs`, inside `handle_tray_menu_request_with_bounds`, require a source **Bar Surface**:

```rust
let Some(origin_window) = source_window.or_else(|| self.surfaces.last_cursor().map(|(w, _)| w))
else {
    return Task::none();
};
```

Use `origin_window` for `screen_size`, `bar_name`, and the emitted `MenuEvent::Open`:

```rust
Message::Menu(messages::MenuEvent::Open {
    service: service.clone(),
    menu_path: menu_path.clone(),
    tree,
    origin_window,
})
```

- [ ] **Step 3: Resolve Menu origin identity in menu open handler**

In `src/app/menu.rs`, update the `MenuEvent::Open` match arm:

```rust
MenuEvent::Open {
    service,
    menu_path,
    tree,
    origin_window,
} => self.open_menu_popup(service, menu_path, tree, origin_window),
```

Update `open_menu_popup`:

```rust
pub(super) fn open_menu_popup(
    &mut self,
    service: String,
    menu_path: zbus::zvariant::OwnedObjectPath,
    tree: crate::surface::menu_popup::MenuTree,
    origin_window: iced::window::Id,
) -> Task<Message> {
    self.spawn_menu_popup(super::surfaces::PopupMeta {
        service,
        menu_path,
        tree,
        origin: self
            .surfaces
            .bar_surface_identity_for_window(origin_window)
            .unwrap_or_else(super::surfaces::BarSurfaceIdentity::all_screens),
    })
}
```

Add to `SurfaceManager`:

```rust
pub(super) fn bar_surface_identity_for_window(
    &self,
    window: iced::window::Id,
) -> Option<BarSurfaceIdentity> {
    self.surfaces
        .get(&window)
        .map(BarSurfaceIdentity::from_surface_state)
}
```

- [ ] **Step 4: Update popup positioning to use identity**

In `src/app/menu.rs`, change both popup-open paths to use:

```rust
let bar = Self::bar_for_surface_identity(&self.config.bars, &meta.origin);
```

- [ ] **Step 5: Update tests constructing `PopupMeta`**

Replace test literals like:

```rust
bar_name: String::new(),
```

with:

```rust
origin: super::super::surfaces::BarSurfaceIdentity::all_screens(),
```

Use `BarSurfaceIdentity::new("main", Some("DP-1".into()))` when a test is about a concrete **Bar Surface**.

- [ ] **Step 6: Run compile-focused tests**

Run:

```bash
rtk cargo test app::menu::tests -- --nocapture
rtk cargo test app::tray::tests::tray_keyboard_menu_request_without_source_window_is_noop -- --nocapture
```

Expected: pass. If the tray no-source-window test fails, keep the no-op behavior by returning `Task::none()` before `MenuEvent::Open`.

## Task 7: Move Window Role Classification Behind The Registry

**Files:**
- Modify: `src/app/surfaces.rs`
- Modify: `src/app/surface_input_router.rs`

- [ ] **Step 1: Add Registry role classification**

In `src/app/surfaces.rs`, add:

```rust
pub(super) fn window_role(
    &self,
    window: Option<iced::window::Id>,
) -> super::surface_input_router::WindowRole {
    let Some(window) = window else {
        return super::surface_input_router::WindowRole::Unknown;
    };

    if self.closing_panel_popups.contains(&window) {
        return super::surface_input_router::WindowRole::ClosingPanel;
    }
    if self.tooltip_popups.contains_key(&window) {
        return super::surface_input_router::WindowRole::Tooltip;
    }
    if self.panel_popups.contains_key(&window) {
        return super::surface_input_router::WindowRole::Panel;
    }
    if self.popups.contains_key(&window) || self.closing_menu_popups.contains_key(&window) {
        return super::surface_input_router::WindowRole::Menu;
    }
    if self.surfaces.contains_key(&window) {
        return super::surface_input_router::WindowRole::BarSurface;
    }
    super::surface_input_router::WindowRole::Unknown
}
```

- [ ] **Step 2: Update `App::window_role_for_surface_input`**

In `src/app/surfaces.rs`, replace the body with:

```rust
self.surfaces.window_role(window)
```

- [ ] **Step 3: Remove snapshot classification if unused**

If `WindowRoleSnapshot` and `classify_window` are now unused outside router tests, keep them only if the tests still provide useful pure-router coverage. Otherwise remove `WindowRoleSnapshot`, `classify_window`, and tests that only assert snapshot priority.

- [ ] **Step 4: Add Registry role tests**

In `src/app/surfaces.rs` tests, add:

```rust
#[test]
fn registry_classifies_closing_panel_windows() {
    let window = test_wid();
    let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new(), true);
    registry.closing_panel_popups.insert(window);

    assert_eq!(
        registry.window_role(Some(window)),
        super::surface_input_router::WindowRole::ClosingPanel
    );
}

#[test]
fn registry_classifies_closing_menu_windows_as_menu_for_input_routing() {
    let window = test_wid();
    let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new(), true);
    registry.closing_menu_popups.insert(
        window,
        popup_meta_for_identity(BarSurfaceIdentity::all_screens(), "org.test"),
    );

    assert_eq!(
        registry.window_role(Some(window)),
        super::surface_input_router::WindowRole::Menu
    );
}
```

- [ ] **Step 5: Run router and Registry role tests**

Run:

```bash
rtk cargo test app::surface_input_router::tests -- --nocapture
rtk cargo test app::surfaces::tests::registry_classifies_closing_panel_windows -- --nocapture
rtk cargo test app::surfaces::tests::registry_classifies_closing_menu_windows_as_menu_for_input_routing -- --nocapture
```

Expected: pass.

## Task 8: Update Bar Surface Removal To Close Overlays By Identity

**Files:**
- Modify: `src/app/surfaces.rs`

- [ ] **Step 1: Change stale Bar Surface cleanup to match exact identity**

In `close_overlays_for_bar_surface`, replace **Menu** cleanup by `bar_name` with exact origin identity:

```rust
let closing_menus: Vec<_> = self
    .popups
    .iter()
    .filter_map(|(window, meta)| (&meta.origin == identity).then_some(*window))
    .collect();
for window in &closing_menus {
    if let Some(meta) = self.popups.remove(window) {
        self.closing_menu_popups.insert(*window, meta);
    }
}
effects.extend(closing_menus.into_iter().filter_map(|window| {
    self.closing_menu_popups
        .get(&window)
        .cloned()
        .map(|meta| RegistryEffect::MenuClosed { window, meta })
}));
```

- [ ] **Step 2: Convert overlay effects for Bar Surface lifecycle**

In `reconcile_bar_surfaces`, convert typed overlay close effects to `BarSurfaceLifecycleEffect::RemoveWindow` only after preserving Registry metadata:

```rust
effects.extend(
    self.close_overlays_for_bar_surface(window, &identity)
        .into_iter()
        .filter_map(|effect| match effect {
            RegistryEffect::PanelClosed { window }
            | RegistryEffect::MenuClosed { window, .. }
            | RegistryEffect::TooltipClosed { window } => {
                Some(BarSurfaceLifecycleEffect::RemoveWindow(window))
            }
            RegistryEffect::SuspendBarKeyboardInteractivity
            | RegistryEffect::RestoreBarKeyboardInteractivity
            | RegistryEffect::PendingMenuReady(_) => None,
        }),
);
```

- [ ] **Step 3: Run identity cleanup test**

Run:

```bash
rtk cargo test app::surfaces::tests::removing_bar_surface_closes_only_overlays_for_that_bar_surface_identity -- --nocapture
```

Expected: pass.

## Task 9: Clean Up Compatibility Helpers And Run The Suite

**Files:**
- Modify: `src/app/surfaces.rs`
- Modify: `src/app/menu.rs`
- Modify: `src/app/panel.rs`
- Modify: `src/app/overlay_adapter.rs`
- Modify: `src/app/panel_runtime.rs`
- Modify: `src/app/messages.rs`
- Modify: `src/app/tray.rs`
- Modify: `src/app/surface_input_router.rs`

- [ ] **Step 1: Search for removed shallow Interface usage**

Run:

```bash
rtk rg -n "RemoveWindow\\(|mark_closing_window|is_closing_window|forget_closing_window|closing_menu_popups\\.contains|pending_menu_popup|bar_name:" src/app src/modules
```

Expected: no references to removed PanelRuntime closing methods. Remaining `pending_menu_popup` references are inside `SurfaceManager` only. Remaining `bar_name:` literals are for `PanelTree` or `SurfaceState`, not `PopupMeta`.

- [ ] **Step 2: Run focused overlay tests**

Run:

```bash
rtk cargo test app::surfaces::tests -- --nocapture
rtk cargo test app::menu::tests -- --nocapture
rtk cargo test app::panel::tests::close_all_panels_closes_tracked_and_untracked_panel_popups -- --nocapture
rtk cargo test app::panel::tests::stale_events_from_closing_panel_window_do_not_create_bar_surface_state -- --nocapture
```

Expected: pass.

- [ ] **Step 3: Run full test suite**

Run:

```bash
rtk cargo test
```

Expected: pass.

- [ ] **Step 4: Commit**

Run:

```bash
rtk git add src/app/surfaces.rs src/app/overlay_adapter.rs src/app/panel_runtime.rs src/app/panel.rs src/app/menu.rs src/app/messages.rs src/app/tray.rs src/app/surface_input_router.rs docs/superpowers/plans/2026-05-31-bar-surface-registry-overlay-lifecycle.md
rtk git commit -m "refactor: deepen bar surface registry overlay lifecycle"
```

Expected: commit succeeds with only this refactor and plan file staged.

## Self-Review

- Spec coverage: the plan covers the agreed scope: Registry-owned Panel/Menu/Tooltip exclusivity, pending Menu replacement, Menu origin by **Bar Surface Identity**, typed close effects, Adapter-owned Iced/D-Bus translation, PanelRuntime closing-tracking removal, and Registry window role classification. Dmenu Selection Request response rules are explicitly excluded.
- Placeholder scan: no task uses placeholder markers, copy-forward shorthand, or unspecified edge handling.
- Type consistency: `PopupMeta::origin`, `RegistryEffect::{PanelClosed, MenuClosed, TooltipClosed, PendingMenuReady}`, `SurfaceManager::window_closed`, and `SurfaceManager::window_role` are used consistently across later tasks.
