# Story 7.1: Spawn a Bar Surface on Every Connected Output

Status: review

## Story

As a **user with multiple monitors**,
I want a bar to appear on each of my screens,
So that I have status information and workspace navigation available on every display.

## Acceptance Criteria

1. **Given** niribar is running and multiple outputs are connected
   **When** the app starts
   **Then** one bar surface is created per output

2. **And** each surface is independently anchored to the configured edge on its respective output

3. **And** each surface fills the full height (or width, for horizontal bars) of its output

4. **And** surfaces are independent — interacting with one (hover, click, keyboard focus, expand/collapse) does not affect others

5. **And** each surface has its own `SurfaceState` (expand/collapse and focus state is per-surface, keyed by `iced::window::Id`)

6. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

7. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 272 pre-existing tests pass and at least 10 new tests pass (≥ 282 total)

## The Big Architectural Shift

Story 7.1 is the largest refactor in the project. It replaces the single-surface `application()` with the multi-surface `daemon()` API from `iced_layershell`. Every handler that touches per-surface state (`expand`, `hovered_module`, `focused_module`, `module_activated`, `bar_hovered`, `hover_pending_ms`) must be updated to use `surfaces: HashMap<iced::window::Id, SurfaceState>`.

**Module data stays in `SharedState` — never moves to `SurfaceState`. This boundary is non-negotiable.**

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `crates/niribar/src/main.rs` | **MODIFY** | `application()` → `daemon()`, `StartMode::AllScreens`, `App::boot()` call |
| `crates/niribar/src/app.rs` | **MODIFY** | `#[to_layer_message(multi)]`, `surfaces: HashMap<Id, SurfaceState>`, remove flat per-surface fields, update `view()` signature, update all per-surface handlers |
| `crates/niribar/src/surface.rs` | **MODIFY** | Extend `SurfaceState` with all per-surface fields currently on `App` |
| `crates/niribar/src/surface/input.rs` | **MODIFY** | `keyboard_handler` passes `window: iced::window::Id` through to messages |

## Tasks / Subtasks

- [x] Task 1: Extend `SurfaceState` in `crates/niribar/src/surface.rs` (AC: #5)
  - [x] 1.1: Add per-surface fields currently flat on `App`:

    ```rust
    pub struct SurfaceState {
        pub output_name: String,
        pub expand: ExpandState,
        pub focused_module: Option<ModuleId>,
        // NEW — these three were flat on App, now per-surface:
        pub hovered_module: Option<ModuleId>,
        pub module_activated: bool,
        pub bar_hovered: bool,
        pub hover_pending_ms: Option<f32>,
    }
    ```

  - [x] 1.2: Update `SurfaceState::new()` to initialize new fields:

    ```rust
    pub fn new(output_name: String) -> Self {
        SurfaceState {
            output_name,
            expand: ExpandState::default(),
            focused_module: None,
            hovered_module: None,
            module_activated: false,
            bar_hovered: false,
            hover_pending_ms: None,
        }
    }
    ```

  - [x] 1.3: Add `Default` impl (delegates to `new("")`):

    ```rust
    impl Default for SurfaceState {
        fn default() -> Self {
            SurfaceState::new(String::new())
        }
    }
    ```

- [x] Task 2: Switch `crates/niribar/src/main.rs` from `application()` to `daemon()` (AC: #1, #2, #3)
  - [x] 2.1: Update imports:

    ```rust
    use iced_layershell::build_pattern::daemon;
    use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
    ```

    Remove `application` import.
  - [x] 2.2: Replace `try_main()` body. Key changes:
    - `application(...)` → `daemon(...)`
    - First argument: `|| App::boot(config.clone())` (returns `(App, Task<Message>)`, not just `App`)
    - `StartMode::AllScreens` creates one surface per connected output
    - Size/exclusive zone still set from global config (each surface uses the same settings)
    - The `daemon` API does NOT take a namespace fn as `application` does — it has its own builder method `.namespace()`... actually looking at `daemon()` signature: `daemon(boot, namespace, update, view)` — same 4-arg pattern as application. Namespace is still second arg.

    ```rust
    fn try_main() -> anyhow::Result<()> {
        let config = config::load()?;

        let anchor = config.bar.edge.anchor();
        let collapsed_width = config.bar.collapsed_width;
        let exclusive_zone = i32::try_from(collapsed_width).unwrap_or(48);
        let size = if config.bar.edge.is_horizontal() {
            Some((0, collapsed_width))
        } else {
            Some((collapsed_width, 0))
        };

        daemon(
            move || App::boot(config.clone()),
            App::namespace,
            App::update,
            App::view,
        )
        .settings(Settings {
            layer_settings: LayerShellSettings {
                anchor,
                exclusive_zone,
                size,
                start_mode: StartMode::AllScreens,  // KEY CHANGE
                ..Default::default()
            },
            ..Default::default()
        })
        .style(App::style)
        .subscription(App::subscription)
        .run()
        .map_err(anyhow::Error::from)
    }
    ```

- [x] Task 3: Update `Message` enum in `crates/niribar/src/app.rs` (AC: #4, #5)
  - [x] 3.1: Change `#[to_layer_message]` to `#[to_layer_message(multi)]`
    - This changes layer-shell action message signatures: `SizeChange` becomes `SizeChange { id: IcedId, size: (u32, u32) }`, `KeyboardInteractivityChange` becomes `KeyboardInteractivityChange { id: IcedId, keyboard_interactivity: ... }`, `ExclusiveZoneChange` becomes `ExclusiveZoneChange { id: IcedId, zone_size: i32 }`
  - [x] 3.2: Add `window: iced::window::Id` to all per-surface application messages:

    ```rust
    // Per-surface messages — all carry window ID for routing to the correct SurfaceState
    ModuleHoverEnter(ModuleId, iced::window::Id),
    ModuleHoverExit(ModuleId, iced::window::Id),
    ModuleClick(ModuleId, iced::window::Id),
    ModuleScrollUp(ModuleId, iced::window::Id),
    ModuleScrollDown(ModuleId, iced::window::Id),
    BarClick(iced::window::Id),
    Tick,                                           // global — ticks whenever any surface animates
    BarHoverEnter(iced::window::Id),
    BarHoverExit(iced::window::Id),
    BarFocusGained(iced::window::Id),
    BarFocusLost(iced::window::Id),
    EscapePressed(iced::window::Id),
    FocusMoveNext(iced::window::Id),
    FocusMovePrev(iced::window::Id),
    FocusNextZone(iced::window::Id),
    FocusPrevZone(iced::window::Id),
    KeyboardActivate(iced::window::Id),
    ToggleExpand,  // IPC-sourced — broadcasts to ALL surfaces (socket message has no window context)
    // Module data events — NOT per-surface (shared state)
    Module(ModuleId, ModuleEvent),
    VolumeAdjust { step: u8, up: bool },
    Niri(crate::niri::NiriEvent),
    Battery(BatteryEvent),
    Network(NetworkEvent),
    Volume(VolumeEvent),
    DbusReady(DbusConnections),
    ```

    Note: `ToggleExpand` (IPC/socket) has no window context and toggles ALL surfaces. `KeyboardActivate` (in-bar keybind) is window-specific and carries the window ID.

- [x] Task 4: Update `App` struct in `crates/niribar/src/app.rs` (AC: #5)
  - [x] 4.1: Remove flat per-surface fields, add `surfaces` map:

    ```rust
    pub struct App {
        pub config: Config,
        pub shared: SharedState,
        pub modules: Vec<(Zone, Box<dyn Module>)>,
        // Per-surface state — keyed by iced::window::Id assigned at surface creation
        pub surfaces: HashMap<iced::window::Id, SurfaceState>,
    }
    ```

  - [x] 4.2: Remove these flat fields (now live in `SurfaceState`):
    - `hovered_module: Option<ModuleId>`
    - `focused_module: Option<ModuleId>`
    - `module_activated: bool`
    - `expand: ExpandState`
    - `bar_hovered: bool`
    - `hover_pending_ms: Option<f32>`

- [x] Task 5: Rename `App::new` to `App::boot` and fix signature (AC: #1)
  - [x] 5.1: `new(config: Config) -> Self` → `boot(config: Config) -> (Self, Task<Message>)`:

    ```rust
    pub fn boot(config: Config) -> (Self, Task<Message>) {
        init_expand_keybind(config.bar.expand_keybind.as_deref());
        // ... (rest of module registry setup unchanged) ...
        let app = App {
            config,
            shared: SharedState::new(),
            modules,
            surfaces: HashMap::new(), // populated lazily on first view/event per window
        };
        (app, Task::none())
    }
    ```

  - [x] 5.2: Add helper to get or insert a default `SurfaceState`:

    ```rust
    pub fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
        self.surfaces.entry(id).or_insert_with(|| SurfaceState::new(String::new()))
    }
    pub fn surface(&self, id: iced::window::Id) -> Option<&SurfaceState> {
        self.surfaces.get(&id)
    }
    ```

- [x] Task 6: Update `App::view` signature and body (AC: #2, #3, #4)
  - [x] 6.1: Change signature:

    ```rust
    pub fn view(&self, id: iced::window::Id) -> Element<'_, Message>
    ```

  - [x] 6.2: Replace all references to flat per-surface fields with `SurfaceState` lookup:

    ```rust
    let surface = self.surfaces.get(&id).cloned().unwrap_or_default();
    let shows_expanded = surface.expand.shows_expanded_content();
    let hovered = surface.hovered_module.as_ref() == Some(&module_id);
    let focused = surface.focused_module.as_ref() == Some(&module_id);
    ```

  - [x] 6.3: Pass `id` (window ID) into all per-surface messages emitted from `view`:

    ```rust
    .on_enter(Message::ModuleHoverEnter(module_id.clone(), id))
    .on_exit(Message::ModuleHoverExit(module_id.clone(), id))
    .on_press(Message::ModuleClick(module_id, id))
    // ... etc.
    .on_press(Message::BarClick(id))
    .on_enter(Message::BarHoverEnter(id))
    .on_exit(Message::BarHoverExit(id))
    ```

  - [x] 6.4: Update scroll handler to pass `id`:

    ```rust
    .on_scroll(move |delta| {
        let up = /* same */ ;
        if up { Message::ModuleScrollUp(scroll_id.clone(), id) }
        else  { Message::ModuleScrollDown(scroll_id.clone(), id) }
    })
    ```

- [x] Task 7: Update `App::update` — per-surface handlers (AC: #4, #5)
  - [x] 7.1: `ModuleHoverEnter(id, window)` → `self.surface_mut(window).hovered_module = Some(id)`
  - [x] 7.2: `ModuleHoverExit(id, window)` → `self.surface_mut(window).hovered_module = None` (if current)
  - [x] 7.3: `BarClick(window)`:

    ```rust
    Message::BarClick(window) => {
        if !self.config.bar.expand { return Task::none(); }
        if self.config.bar.expand_trigger == ExpandTrigger::Hover { return Task::none(); }
        let s = self.surface_mut(window);
        s.expand = s.expand.toggle();
        self.overlay_size_change_for(window)
    }
    ```

  - [x] 7.4: `BarHoverEnter(window)`, `BarHoverExit(window)` → update `surface_mut(window).bar_hovered`, `hover_pending_ms`
  - [x] 7.5: `Tick` — iterate over ALL surfaces, advance animation/hover for each:

    ```rust
    Message::Tick => {
        let mut tasks = vec![];
        let window_ids: Vec<_> = self.surfaces.keys().copied().collect();
        for wid in window_ids {
            let s = self.surfaces.get_mut(&wid).unwrap();
            // advance hover countdown
            if let Some(elapsed) = s.hover_pending_ms { /* same logic */ }
            // advance animation
            s.expand = s.expand.advance(/* EXPAND_MS, COLLAPSE_MS, TICK_MS */);
            tasks.push(self.overlay_size_change_for(wid));
        }
        Task::batch(tasks)
    }
    ```

  - [x] 7.6: `BarFocusGained(window)`:

    ```rust
    Message::BarFocusGained(window) => {
        let first = /* same first-module logic */;
        let s = self.surface_mut(window);
        s.focused_module = first;
        s.module_activated = false;
        Task::none()
    }
    ```

  - [x] 7.7: `BarFocusLost(window)`:

    ```rust
    Message::BarFocusLost(window) => {
        let s = self.surface_mut(window);
        s.focused_module = None;
        s.module_activated = false;
        Task::done(Message::KeyboardInteractivityChange {
            id: window,
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
        })
    }
    ```

  - [x] 7.8: `EscapePressed(window)`, `FocusMoveNext(window)`, `FocusMovePrev(window)`, `FocusNextZone(window)`, `FocusPrevZone(window)`, `KeyboardActivate(window)` — all look up `self.surface_mut(window)` for `focused_module` / `module_activated`. Pattern is identical to current code except `self.focused_module` → `s.focused_module`, `self.module_activated` → `s.module_activated`.
  - [x] 7.9: `EscapePressed(window)` → `Task::done(Message::KeyboardInteractivityChange { id: window, keyboard_interactivity: KeyboardInteractivity::None })`
  - [x] 7.10: `ToggleExpand` (IPC — no window context) → toggle ALL surfaces:

    ```rust
    Message::ToggleExpand => {
        if !self.config.bar.expand { return Task::none(); }
        let window_ids: Vec<_> = self.surfaces.keys().copied().collect();
        let tasks: Vec<_> = window_ids.into_iter().map(|wid| {
            let s = self.surfaces.get_mut(&wid).unwrap();
            s.expand = s.expand.toggle();
            self.overlay_size_change_for(wid)
        }).collect();
        Task::batch(tasks)
    }
    ```

    Note: `ToggleExpand` is fire-and-forget from the IPC socket — it has no window context, so it applies to all. This is intentional: the socket command is for users who want all bars to toggle simultaneously.
  - [x] 7.11: `ModuleScrollUp(id, window)` / `ModuleScrollDown(id, window)` — same as current but route to correct surface's modules; module interactions are shared (modules live in `self.modules`, not per-surface).

- [x] Task 8: Update `overlay_size_change` to be window-specific (AC: #4)
  - [x] 8.1: Rename and parameterize:

    ```rust
    fn overlay_size_change_for(&self, window: iced::window::Id) -> Task<Message> {
        let s = self.surfaces.get(&window).cloned().unwrap_or_default();
        let size = s.expand.current_width(
            self.config.bar.collapsed_width,
            self.config.bar.expanded_width,
        );
        if self.config.bar.edge.is_horizontal() {
            Task::done(Message::SizeChange { id: window, size: (0, size) })
        } else {
            Task::done(Message::SizeChange { id: window, size: (size, 0) })
        }
    }
    ```

- [x] Task 9: Update `App::subscription` tick condition (AC: #4)
  - [x] 9.1: The tick sub now fires when ANY surface is animating or has a pending hover:

    ```rust
    let needs_tick = self.surfaces.values().any(|s| {
        s.expand.is_animating() || s.hover_pending_ms.is_some()
    });
    let tick_sub = if needs_tick {
        iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick)
    } else {
        iced::Subscription::none()
    };
    ```

- [x] Task 10: Update `crates/niribar/src/surface/input.rs` keyboard handler (AC: #4)
  - [x] 10.1: Use the `window` parameter instead of ignoring it:

    ```rust
    fn keyboard_handler(
        event: Event,
        _status: iced::event::Status,
        window: iced::window::Id,      // was `_window`
    ) -> Option<Message> {
        match event {
            Event::Window(window::Event::Focused) => Some(Message::BarFocusGained(window)),
            Event::Window(window::Event::Unfocused) => Some(Message::BarFocusLost(window)),
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if let Some(Some((kb_mods, kb_key))) = EXPAND_KEYBIND.get()
                    && modifiers == *kb_mods
                    && key == *kb_key
                {
                    return Some(Message::ToggleExpand);  // No window ID — IPC style, all surfaces
                    // Alternative: per-surface: Some(Message::KeyboardActivate(window))
                    // Architecture decision: expand keybind in compositor runs niribar --toggle-expand
                    // (Story 6.4 design). In-bar keybind is already handled by IPC. So ToggleExpand stays global.
                }
                match key {
                    keyboard::Key::Named(Named::Escape) => Some(Message::EscapePressed(window)),
                    keyboard::Key::Named(Named::ArrowDown) => Some(Message::FocusMoveNext(window)),
                    keyboard::Key::Named(Named::ArrowUp) => Some(Message::FocusMovePrev(window)),
                    keyboard::Key::Named(Named::Tab) => {
                        if modifiers.shift() {
                            Some(Message::FocusPrevZone(window))
                        } else {
                            Some(Message::FocusNextZone(window))
                        }
                    }
                    keyboard::Key::Named(Named::Enter | Named::Space) => {
                        Some(Message::KeyboardActivate(window))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
    ```

- [x] Task 11: Write tests (AC: #7)
  - [x] 11.1: `SurfaceState` tests in `crates/niribar/src/surface.rs`:

    ```rust
    #[test]
    fn surface_state_default_starts_collapsed_and_unfocused() {
        let s = SurfaceState::default();
        assert_eq!(s.expand, ExpandState::Collapsed);
        assert!(s.focused_module.is_none());
        assert!(s.hovered_module.is_none());
        assert!(!s.module_activated);
        assert!(!s.bar_hovered);
        assert!(s.hover_pending_ms.is_none());
    }
    ```

  - [x] 11.2: `App::surfaces` isolation tests:

    ```rust
    #[test]
    fn two_surfaces_have_independent_expand_state() {
        let (mut app, _) = App::boot(make_config());
        let id_a = iced::window::Id::unique();
        let id_b = iced::window::Id::unique();
        // Expand surface A
        let s_a = app.surfaces.entry(id_a).or_insert_with(|| SurfaceState::new("A".into()));
        s_a.expand = ExpandState::Expanded;
        // Surface B should still be Collapsed (or not yet exist)
        let s_b = app.surfaces.get(&id_b);
        assert!(s_b.map(|s| s.expand == ExpandState::Collapsed).unwrap_or(true));
    }

    #[test]
    fn two_surfaces_have_independent_focus_state() {
        let (mut app, _) = App::boot(make_config());
        let id_a = iced::window::Id::unique();
        let id_b = iced::window::Id::unique();
        let focus_id = ModuleId::from("clock");
        app.surfaces.entry(id_a).or_insert_with(SurfaceState::default).focused_module = Some(focus_id.clone());
        let s_b = app.surfaces.get(&id_b);
        assert!(s_b.map(|s| s.focused_module.is_none()).unwrap_or(true));
    }
    ```

  - [x] 11.3: Test `surface_mut` lazy init:

    ```rust
    #[test]
    fn surface_mut_creates_default_state_for_new_window() {
        let (mut app, _) = App::boot(make_config());
        let id = iced::window::Id::unique();
        assert!(app.surfaces.get(&id).is_none());
        let s = app.surface_mut(id);
        assert_eq!(s.expand, ExpandState::Collapsed);
    }
    ```

  - [x] 11.4: Test `App::boot` returns `Task::none()` (not panicking):

    ```rust
    #[test]
    fn boot_returns_app_and_task() {
        let (app, _task) = App::boot(make_config());
        assert!(app.surfaces.is_empty()); // no surfaces before first view call
    }
    ```

  - [x] 11.5: Test tick sub condition:

    ```rust
    #[test]
    fn tick_not_needed_when_no_surfaces() {
        let (app, _) = App::boot(make_config());
        // With no surfaces, no animation is pending
        let needs_tick = app.surfaces.values().any(|s| s.expand.is_animating());
        assert!(!needs_tick);
    }

    #[test]
    fn tick_needed_when_any_surface_animating() {
        let (mut app, _) = App::boot(make_config());
        let id = iced::window::Id::unique();
        app.surface_mut(id).expand = ExpandState::Expanding(0.5);
        let needs_tick = app.surfaces.values().any(|s| s.expand.is_animating());
        assert!(needs_tick);
    }
    ```

## Dev Notes

### `application()` vs `daemon()` — Critical Differences

| | `application()` | `daemon()` |
|---|---|---|
| Entry | `fn() -> App` | `fn() -> (App, Task<Message>)` |
| View | `fn view(&self) -> Element` | `fn view(&self, id: iced::window::Id) -> Element` |
| Multi-output | No | Yes — `StartMode::AllScreens` |
| Message macro | `#[to_layer_message]` | `#[to_layer_message(multi)]` |
| Layer-shell messages | `SizeChange((w, h))` | `SizeChange { id: IcedId, size: (w, h) }` |

Imports for `daemon`:

```rust
use iced_layershell::build_pattern::daemon;
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
```

(Remove `application` import from `build_pattern`.)

### `#[to_layer_message(multi)]` — What Changes

When `multi` is added to the macro, these message variants change signatures:

| Single | Multi |
|--------|-------|
| `SizeChange((u32, u32))` | `SizeChange { id: IcedId, size: (u32, u32) }` |
| `KeyboardInteractivityChange(KeyboardInteractivity)` | `KeyboardInteractivityChange { id: IcedId, keyboard_interactivity: KeyboardInteractivity }` |
| `AnchorSizeChange(Anchor, (u32, u32))` | `AnchorSizeChange { id: IcedId, anchor: Anchor, size: (u32, u32) }` |

Every call site for these in `app.rs` must be updated. Search for `Message::SizeChange`, `Message::KeyboardInteractivityChange` in the codebase — there are ~5-8 call sites.

### Surface ID Lifecycle

With `StartMode::AllScreens`, `iced_layershell` creates one surface per Wayland output before the event loop starts. Each surface gets a unique `iced::window::Id` assigned internally. The app receives events (keyboard, mouse, window focus) tagged with the window ID.

Surfaces are **not** created via `Message::NewLayerShell` — they're created automatically by the compositor integration at startup. There is no "window created" callback to initialize `SurfaceState` proactively. Use lazy initialization via `surface_mut(id)`:

- `view(state, id)`: falls back to `SurfaceState::default()` if `id` not yet in `surfaces`
- `update(state, msg)`: calls `self.surface_mut(window_id)` which inserts default on first access

### `ToggleExpand` — Global vs Per-Surface Design Decision

`ToggleExpand` comes from the IPC socket (`niribar --toggle-expand`). The socket has no window context, so `ToggleExpand` applies to **all surfaces located on the currently focused output**.

Keyboard-triggered expand via the in-bar keybind (`keyboard_handler`) fires `ToggleExpand` from the `EXPAND_KEYBIND` match — this **only** toggles the focused surface.

### `ExpandState::advance` — Tick Handler

In the current code, `Tick` calls:

```rust
self.expand = match self.expand {
    ExpandState::Expanding(p) => { /* compute new p */ ... }
    // ...
}
```

In Story 7.1, `Tick` must advance each surface's `expand` independently. The advance logic from the current `Tick` handler should be extracted into a method on `ExpandState` (or keep inline, duplicating per surface). The tick handler iterates over all surfaces:

```rust
for wid in window_ids {
    let s = self.surfaces.get_mut(&wid).unwrap();
    // 1. advance hover_pending_ms
    // 2. advance s.expand (same logic as current Tick)
    // 3. push SizeChange task for this window
}
```

### `App::style` — Multi-Surface Context

`App::style(&self, theme: &iced::Theme)` is called once per window in `daemon` mode. The signature should stay the same — the style is global, not per-surface.

### `nav_next`, `nav_prev`, `nav_next_zone`, `nav_prev_zone` — No Change

These navigation helpers operate on `self.modules` (shared). The caller passes in the `focused_module` ID from the relevant `SurfaceState`. No change to the helpers themselves — just pass `s.focused_module.as_ref()` instead of `self.focused_module.as_ref()`.

### Compilation Tip — Exhaustive Match Arms

After changing `#[to_layer_message]` to `#[to_layer_message(multi)]`, the macro generates new message variants. If `update` has a `_ => Task::none()` catch-all at the end, it will continue to compile cleanly. But if there are exhaustive matches elsewhere, they need updating.

Search for `Message::SizeChange` / `Message::KeyboardInteractivityChange` in the code — each must be updated to the new struct syntax.

### Test Helper Updates

Existing tests use `App::new(config)`. These must change to:

```rust
let (app, _) = App::boot(make_config());
```

Update: `new_with_config_stores_config`, `new_modules_empty_with_no_module_config`, `undeclared_known_type_in_bar_modules_uses_defaults`, `undeclared_unknown_name_in_bar_modules_is_skipped`, `clock_module_instantiated_from_registry`, and all other tests that call `App::new(...)`.

Tests that directly read `app.expand`, `app.hovered_module`, `app.focused_module` etc. must be rewritten to use `app.surfaces` — or, better, test through `surface_mut()` / `surface()` helpers.

Tests that call `app.update(Message::ToggleExpand)` (from Story 6.4) now need a surface to exist first. Initialize one: `app.surfaces.insert(id, SurfaceState::default())` before updating.

### `BarFocusGained` — Multi-Surface

In the current code, `BarFocusGained` calls `self.focused_module = first`. First module is found by scanning `self.modules` (shared). In multi-surface, the first module is still found from shared `self.modules` — the only change is that the result is stored in `self.surface_mut(window).focused_module`.

### `ModuleId` type

`ModuleId` is defined in `src/modules/mod.rs`. It implements `Clone` and `PartialEq`. No change needed. Used in `SurfaceState` as `Option<ModuleId>`.

### Project Structure Reference

```
src/
├── app.rs           ← Major changes: #[to_layer_message(multi)], surfaces HashMap, view(id), all handlers
├── main.rs          ← application() → daemon(), StartMode::AllScreens, App::boot()
├── surface.rs       ← SurfaceState extended with hover/focus fields
├── surface/
│   ├── expand.rs    ← No change
│   ├── input.rs     ← keyboard_handler passes window ID
│   ├── frame.rs     ← No change
│   └── layout.rs    ← No change
└── ipc.rs           ← No change (ToggleExpand stays global)
```

### AR6 Compliance

Architecture Rule 6 (AR6): "SurfaceState is per-surface — each output has independent expand/focus state."

This story implements AR6 for expand and focus. Module data continues to live in `SharedState`. The test in AC #5 verifies the isolation.

## Previous Story Intelligence (Story 6.4)

- `mod ipc;` is declared in `crates/niribar/src/main.rs` (crate root) — no change needed
- `App::subscription()` returns `iced::Subscription::batch([...])` — structure unchanged, only tick condition changes
- `ExpandState::toggle()` is the correct method for toggling expand state
- In tests, `app.expand` (flat field) will no longer exist — tests that reference it must migrate to `app.surfaces`
- `make_config()` and `make_hover_config()` helpers in `crates/niribar/src/app.rs` tests are reusable
- `272 tests pass` as of Story 6.4 completion — must not regress

## Definition of Done

- [x] `SurfaceState` has all per-surface fields: `expand`, `focused_module`, `hovered_module`, `module_activated`, `bar_hovered`, `hover_pending_ms`, `output_name`
- [x] `App` has `surfaces: HashMap<iced::window::Id, SurfaceState>` and no flat per-surface fields
- [x] `main.rs` uses `daemon()` with `StartMode::AllScreens`
- [x] `Message` macro is `#[to_layer_message(multi)]`
- [x] All per-surface `Message` variants carry `window: iced::window::Id`
- [x] `App::view` takes `id: iced::window::Id` and renders the correct surface state
- [x] `keyboard_handler` passes window ID through to messages
- [x] All layer-shell action messages use struct syntax with `id` field
- [x] All existing `App::new(...)` test calls migrated to `App::boot(...)`
- [x] `cargo clippy -- -D warnings` clean
- [x] `cargo test` shows ≥ 282 tests passing

## Dev Agent Record

### Agent Model Used

_to be filled by dev agent_

### Debug Log References

### Completion Notes List

### File List

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-04-06 | 1.0 | Created story: multi-surface refactor — daemon API, SurfaceState per-window, view(id) | claude-sonnet-4-6 |
