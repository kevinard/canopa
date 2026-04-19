# App Actor Split Implementation Plan

> **For agentic workers:** DO NOT IMPLEMENT THIS FILE YET. This is the deferred Phase 5 split from `2026-04-19-maintainability-refactor.md`. Before implementation, rewrite this file into a full task-by-task plan using `superpowers:writing-plans`, then implement with `superpowers:subagent-driven-development` or `superpowers:executing-plans`.

**Goal:** Decompose `App` into ownership-focused runtime components (`SharedContext`, `SurfaceManager`, `MenuSystem`, `ModuleRuntime`, `NiriState`, `TrayState`) while preserving iced's single-threaded update loop and all observable behavior.

**Architecture:** `App::update` becomes a shallow router over domain messages. Each sub-struct owns its own state and returns cross-domain effects as `Task<Message>` instead of calling another sub-struct directly. This keeps the actor-style boundary compiler-enforced without adding channels, locks, threads, or runtime dependencies.

**Tech Stack:** Rust 2024 edition, `iced` 0.14 with `iced_layershell`, `zbus` 5.x, `tokio`, `niri-ipc`.

---

## Status

This plan is intentionally deferred because the first maintainability branch now stops after Phases 1–4 plus cleanup. Do not treat the task list below as execution-ready code instructions. The previous review found the old Phase 5 too abstract; the next planning pass must replace every field move, message mapping, and view/update handoff with concrete code snippets and verification steps.

## Target App Shape

```rust
pub struct App {
    config: Config,
    shared: SharedContext,
    surfaces: SurfaceManager,
    menu: MenuSystem,
    modules: ModuleRuntime,
    niri: NiriState,
    tray: TrayState,
    perf: Option<perf::PerfTracker>,
}
```

`App::update` should end with a small router:

```rust
pub fn update(&mut self, message: Message) -> iced::Task<Message> {
    match message {
        Message::Surface(event) => self.surfaces.update(event, &self.shared, &self.config),
        Message::Menu(event) => self.menu.update(event),
        Message::Modules(event) => self.modules.update(event, &self.shared, &self.niri),
        Message::Niri(event) => self.niri.update(event),
        Message::Tray(event) => self.tray.update(event),
        Message::DbusReady(dbus) => self.handle_dbus_ready(dbus),
        Message::PerfReport => self.handle_perf_report(),
        other => self.handle_remaining_cross_cutting_message(other),
    }
}
```

The final implementation plan must decide whether `DbusReady`, performance reporting, config reload, IPC commands, and window creation remain direct `App` responsibilities or become domain events.

## Required Pre-Implementation Inventory

Before writing implementation tasks, inspect and record the current exact ownership of:

- `src/app.rs`: `SharedState`, `App` fields, `Message`, `App::new`, `App::update`, tests at the bottom of the file.
- `src/app/animation.rs`, `src/app/surfaces.rs`, `src/app/layout.rs`, `src/app/focus.rs`: per-window state and layer-shell operations.
- `src/app/menu.rs`, `src/app/navigation.rs`, `src/app/view.rs`, `src/surface/menu_popup.rs`: popup menu lifecycle, rendering, hover/focus state, keyboard navigation.
- `src/app/tray.rs`, `src/modules/tray.rs`, `src/dbus/sni/`: SNI item state, tray icon dynamic modules, menu requests.
- `src/app/niri.rs`, `src/niri.rs`, `src/niri/subscription.rs`: workspace/focused-window state and subscription events.
- `src/app/module_store.rs`, `src/app/module_state.rs`, `src/app/module_events.rs`, `src/app/dynamic_modules.rs`, `src/app/widget_ops.rs`, `src/app/subscriptions.rs`: module store/state ownership, dynamic module rebuilds, interaction handling, subscriptions.

## Planned Extraction Order

### 1. SharedContext

Split the current `SharedState` instead of blindly replacing it. Current mutable module and niri state must remain available until `ModuleRuntime` and `NiriState` own them. The execution plan must name each field's destination.

Expected final responsibility:

```rust
pub struct SharedContext {
    pub dbus: crate::dbus::DbusConnections,
}
```

Add theme or clock fields only if the current code actually has such read-only handles when this work starts.

### 2. SurfaceManager

Own every window and every popup window. Move `App.surfaces`, `pending_surface_keys`, `manual_outputs`, and popup-window lifecycle out of `App` only after the execution plan maps every method that reads or mutates those fields.

Expected final responsibility:

```rust
pub struct SurfaceManager {
    surfaces: std::collections::HashMap<iced::window::Id, crate::surface::SurfaceState>,
    pending_surface_keys: std::collections::HashMap<iced::window::Id, (String, String)>,
    manual_outputs: bool,
    popups: std::collections::HashMap<iced::window::Id, PopupMeta>,
    last_cursor: Option<(iced::window::Id, iced::Point)>,
}
```

The execution plan must specify how layer-shell `Task<Message>` values are created after this move.

### 3. MenuSystem

Own menu open/closed state, hover/focus state, navigation, and menu rendering. It must not own popup windows; it requests them through `Message::Surface(SurfaceEvent::OpenPopup { parent, content, anchor })`.

The execution plan must define:

- The exact `MenuModel` shape that preserves the existing `crate::surface::menu_popup::MenuTree` behavior.
- The activation token format for DBusMenu item clicks.
- The event that connects a newly opened popup window back to the menu id.
- The tests that move from `src/app.rs` and `src/surface/menu_popup.rs`.

### 4. NiriState

Own workspace list and focused-window state. The execution plan must account for current `crate::niri::NiriState` to avoid creating a confusing duplicate with the same name.

Possible final name if the crate-level type still exists: `AppNiriState`.

### 5. TrayState

Own only SNI data source state: current tray items and menu data translation. It must not create windows and must not mutate dynamic tray icon modules directly; it emits `Message::Modules(ModuleEvent::TrayItemsChanged(items))`.

The execution plan must specify how DBusMenu fetch/update results from `dbus::sni::menu` are converted into `MenuModel`.

### 6. ModuleRuntime

Own `ModuleStore`, per-module `ModuleState`, module subscriptions, dynamic workspace/tray module rebuilding, and module interaction handling.

This should be extracted last because it receives follow-up events from `NiriState` and `TrayState`.

## Verification Gate

Every extraction commit must pass:

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

Manual Wayland smoke tests are required after `SurfaceManager`, `MenuSystem`, `TrayState`, and `ModuleRuntime` land:

- Bar renders on every configured output.
- Hover/click expand toggle works.
- Tray icons add/remove as apps start and stop.
- Right-click tray menu opens, keyboard navigation works, click-outside closes, and menu items execute.
- Workspace switching updates focus highlight.
- Focused-window title updates.
- Battery, network, volume, clock, script, and button modules still update/respond.

## Required Rewrite Before Implementation

The next planning pass must convert this deferred outline into bite-sized tasks with:

- Exact file paths and field moves.
- Exact `Message` enum transitions.
- Concrete code snippets with no placeholder macros, ellipses, or comment-only bodies.
- Test moves or new tests for each state owner.
- One commit per handler group or field-owner transition.
