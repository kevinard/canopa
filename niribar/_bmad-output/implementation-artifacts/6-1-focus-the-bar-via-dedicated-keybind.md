# Story 6.1: Focus the Bar via Dedicated Keybind

Status: review

## Story

As a **user who prefers keyboard-driven workflows**,
I want to press a keybind to focus the bar,
So that I can navigate and interact with it without touching the mouse.

## Acceptance Criteria

1. **Given** the bar is running and unfocused
   **When** the user's compositor keybind focuses the bar (e.g. niri: `focus-layer-surface`)
   **Then** the bar surface receives keyboard focus
   **And** the first module in the top zone is highlighted with the `ModuleFrame` focus state (same rounded-rect fill as hover: `#313244`, 4px radius)
   **And** focus state (`focused_module`) is stored in `SurfaceState` (and mirrored in `App` for single-surface use, per existing expand/hovered_module pattern)

2. **Given** the bar has keyboard focus
   **When** the user presses Escape
   **Then** the bar releases keyboard focus (sends `KeyboardInteractivityChange(None)` to Wayland)
   **And** the compositor returns focus to the previously focused window
   **And** `KeyboardInteractivity::OnDemand` is restored once focus is lost (so the bar can be focused again)

3. **Given** the bar has keyboard focus
   **When** the surface receives a `Window::Unfocused` event (focus lost for any reason)
   **Then** `focused_module` is cleared
   **And** `KeyboardInteractivity::OnDemand` is re-enabled

4. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

5. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 229 pre-existing tests pass and at least 7 new tests pass (≥ 236 total)

## Technical Background: iced_layershell 0.17.1 Keyboard API

**Critical knowledge — do NOT guess these:**

### KeyboardInteractivity
`KeyboardInteractivity` is re-exported from `iced_layershell::reexport::KeyboardInteractivity`.
Variants: `None` (no focus ever), `Exclusive` (lock-screen-style), `OnDemand` (normal focus).
`LayerShellSettings::default()` already sets `keyboard_interactivity: KeyboardInteractivity::OnDemand` — so the bar can receive focus without any `main.rs` changes.

### KeyboardInteractivityChange message
The `#[to_layer_message]` macro (non-multi mode) auto-generates this variant into the `Message` enum:
```rust
KeyboardInteractivityChange(iced_layershell::reexport::KeyboardInteractivity)
```
This already exists — do NOT add it manually to the enum. Send it via `Task::done(Message::KeyboardInteractivityChange(...))`.

### Window focus events
`iced_layershell` converts Wayland focus events to iced events:
- `WindowEvent::Focused` → `iced::Event::Window(iced::window::Event::Focused)`
- `WindowEvent::Unfocus` → `iced::Event::Window(iced::window::Event::Unfocused)`

Subscribe via `iced::event::listen_with` (in `iced::event` module, re-exported from `iced_futures`).

### Keyboard events
`iced::event::listen_with` also delivers keyboard events:
- `iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })`
- Escape key: `iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)`

### listen_with signature
```rust
pub fn listen_with<Message>(
    f: fn(Event, event::Status, window::Id) -> Option<Message>,
) -> Subscription<Message>
```
Receives all events (except `RedrawRequested`) regardless of capture status. Use for window and keyboard events.

## Architecture Constraints (AR6 — MUST follow)

- `focused_module: Option<ModuleId>` belongs in `SurfaceState` (already has placeholder comment)
- For single-surface (Story 6.x), also mirror as `focused_module: Option<ModuleId>` in `App` — same pattern as `expand: ExpandState` and `hovered_module: Option<ModuleId>`
- **Never** put focus state in `SharedState`
- Keyboard focus logic lives in `src/surface/input.rs` (new file, per architecture file map)

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `src/surface/input.rs` | **CREATE** | `keyboard_focus_subscription()` fn returning iced Subscription |
| `src/surface/mod.rs` | **MODIFY** | Uncomment/add `focused_module: Option<ModuleId>` to `SurfaceState`; update `new()` and tests |
| `src/app.rs` | **MODIFY** | New messages, new `App` field, update handlers, subscription, view |

## Tasks / Subtasks

- [x] Task 1: Create `src/surface/input.rs` with keyboard focus subscription
  - [x] 1.1: Create the file with `keyboard_focus_subscription()` that returns `iced::Subscription<crate::app::Message>`:
    ```rust
    use iced::event::listen_with;
    use iced::{Event, Subscription};
    use iced::window;
    use iced::keyboard::{self, key::Named};

    use crate::app::Message;

    /// Subscribe to keyboard focus events (Window::Focused, Window::Unfocused) and
    /// Escape key presses. Returns messages that update bar keyboard focus state.
    pub fn keyboard_focus_subscription() -> Subscription<Message> {
        listen_with(|event, _status, _window| match event {
            Event::Window(window::Event::Focused) => Some(Message::BarFocusGained),
            Event::Window(window::Event::Unfocused) => Some(Message::BarFocusLost),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(Named::Escape),
                ..
            }) => Some(Message::EscapePressed),
            _ => None,
        })
    }
    ```
  - [x] 1.2: Add `pub mod input;` to `src/surface/mod.rs`

- [x] Task 2: Update `src/surface/mod.rs` — add `focused_module` to `SurfaceState`
  - [x] 2.1: Replace the comment `// focused_module: Option<ModuleId> — added in Story 6.1` with the actual field:
    ```rust
    use crate::modules::ModuleId;
    // ... existing imports ...
    
    pub struct SurfaceState {
        pub output_name: String,
        pub expand: ExpandState,
        pub focused_module: Option<ModuleId>,
    }
    ```
  - [x] 2.2: Update `SurfaceState::new()` to initialize `focused_module: None`
  - [x] 2.3: Add/update tests for `SurfaceState`:
    ```rust
    #[test]
    fn surface_state_focused_module_starts_none() {
        let state = SurfaceState::new("DP-1".into());
        assert!(state.focused_module.is_none());
    }
    ```

- [x] Task 3: Add new `Message` variants to `App` in `src/app.rs`
  - [x] 3.1: Add three new variants to the `Message` enum BEFORE the `_ => Task::none()` catch-all (add in `update()` order for readability). In `src/app.rs`, inside the `#[to_layer_message] #[derive(Debug, Clone)] pub enum Message { ... }` block, add after `DbusReady`:
    ```rust
    /// Layer surface received keyboard focus (Story 6.1).
    BarFocusGained,
    /// Layer surface lost keyboard focus (Story 6.1).
    BarFocusLost,
    /// Escape key pressed while bar has keyboard focus (Story 6.1).
    EscapePressed,
    ```

- [x] Task 4: Add `focused_module` field to `App` struct in `src/app.rs`
  - [x] 4.1: Add field after `hovered_module`:
    ```rust
    /// Which module currently has keyboard focus; None when bar is unfocused.
    /// Moves to per-surface SurfaceState in Story 7.1.
    pub focused_module: Option<ModuleId>,
    ```
  - [x] 4.2: Initialize in `App::new()` after `hovered_module: None`:
    ```rust
    focused_module: None,
    ```

- [x] Task 5: Implement message handlers in `App::update()` in `src/app.rs`
  - [x] 5.1: Add handler for `Message::BarFocusGained` — finds first module in top zone, falls back to center then bottom:
    ```rust
    Message::BarFocusGained => {
        // Highlight first module in top zone (AC #1); fall back to center/bottom.
        let first = self.modules.iter()
            .find(|(z, _)| *z == Zone::Top)
            .or_else(|| self.modules.iter().find(|(z, _)| *z == Zone::Center))
            .or_else(|| self.modules.iter().find(|(z, _)| *z == Zone::Bottom))
            .map(|(_, m)| m.id());
        self.focused_module = first;
        Task::none()
    }
    ```
  - [x] 5.2: Add handler for `Message::BarFocusLost` — clears focus state and re-enables OnDemand:
    ```rust
    Message::BarFocusLost => {
        self.focused_module = None;
        // Restore OnDemand so the bar can be focused again (AC #3).
        use iced_layershell::reexport::KeyboardInteractivity;
        Task::done(Message::KeyboardInteractivityChange(KeyboardInteractivity::OnDemand))
    }
    ```
  - [x] 5.3: Add handler for `Message::EscapePressed` — releases focus only when bar is focused:
    ```rust
    Message::EscapePressed => {
        if self.focused_module.is_none() {
            return Task::none();
        }
        // Tell compositor to remove keyboard focus from this surface (AC #2).
        // BarFocusLost fires in response and restores OnDemand.
        use iced_layershell::reexport::KeyboardInteractivity;
        Task::done(Message::KeyboardInteractivityChange(KeyboardInteractivity::None))
    }
    ```
  - [x] 5.4: Add the import at the top of `src/app.rs` (if not already present — check first): `use crate::surface::input::keyboard_focus_subscription;`
    And add to the top-level `use crate::surface::...` imports: `use crate::surface::frame::module_frame;` (already present — no change needed).

- [x] Task 6: Wire subscription in `App::subscription()` in `src/app.rs`
  - [x] 6.1: Add `let focus_sub = keyboard_focus_subscription();` near the top of `subscription()`
  - [x] 6.2: Add `focus_sub` to both `Subscription::batch` call sites (the animating branch and the non-animating branch)

- [x] Task 7: Update `App::view()` to highlight focused module
  - [x] 7.1: In the module-rendering loop inside `view()`, after the existing hover check, add focus check and combine:
    ```rust
    let hovered = self.hovered_module.as_ref() == Some(&id);
    let focused = self.focused_module.as_ref() == Some(&id);
    // module_frame treats hover and keyboard focus identically (UX-DR3)
    let view: Element<'_, Message> = mouse_area(module_frame(module_view, hovered || focused))
    ```
    Replace the existing line:
    ```rust
    let view: Element<'_, Message> = mouse_area(module_frame(module_view, hovered))
    ```
    with:
    ```rust
    let focused = self.focused_module.as_ref() == Some(&id);
    let view: Element<'_, Message> = mouse_area(module_frame(module_view, hovered || focused))
    ```
    (Keep the existing `hovered` line as-is; only add the `focused` line and update the `module_frame` call.)

- [x] Task 8: Add `pub mod input;` in `src/surface/mod.rs`
  - [x] 8.1: Add at the top of `src/surface/mod.rs` alongside existing `pub mod expand;` etc.:
    ```rust
    pub mod input;
    ```

- [x] Task 9: Write tests in `src/app.rs` (append to the existing `#[cfg(test)] mod tests` block)
  - [x] 9.1: Test `BarFocusGained` sets `focused_module` to first top-zone module:
    ```rust
    #[test]
    fn bar_focus_gained_sets_first_top_module() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        use crate::modules::Zone;
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: Zone::Top,
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
        let mut app = App::new(config);
        let _ = app.update(Message::BarFocusGained);
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }
    ```
  - [x] 9.2: Test `BarFocusLost` clears `focused_module`:
    ```rust
    #[test]
    fn bar_focus_lost_clears_focused_module() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        use crate::modules::Zone;
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: Zone::Top,
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
        let mut app = App::new(config);
        let _ = app.update(Message::BarFocusGained);
        assert!(app.focused_module.is_some());
        let _ = app.update(Message::BarFocusLost);
        assert!(app.focused_module.is_none());
    }
    ```
  - [x] 9.3: Test `EscapePressed` when bar is focused produces a task (not Task::none):
    ```rust
    #[test]
    fn escape_pressed_when_focused_produces_task() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        use crate::modules::Zone;
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: Zone::Top,
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
        let mut app = App::new(config);
        let _ = app.update(Message::BarFocusGained);
        assert!(app.focused_module.is_some());
        // EscapePressed should return a non-none task (KeyboardInteractivityChange(None))
        // We can't inspect the task directly, but we can verify it doesn't immediately clear state
        // (state is cleared by BarFocusLost which fires after the Wayland round-trip)
        let _ = app.update(Message::EscapePressed);
        // focused_module is still set — cleared by BarFocusLost, not EscapePressed
        assert!(app.focused_module.is_some());
    }
    ```
  - [x] 9.4: Test `EscapePressed` when bar is NOT focused is a no-op:
    ```rust
    #[test]
    fn escape_pressed_when_not_focused_is_noop() {
        let mut app = App::new(make_config());
        assert!(app.focused_module.is_none());
        let _ = app.update(Message::EscapePressed);
        assert!(app.focused_module.is_none());
    }
    ```
  - [x] 9.5: Test `BarFocusGained` with no top-zone modules falls back to center:
    ```rust
    #[test]
    fn bar_focus_gained_falls_back_to_center_if_no_top_zone() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        use crate::modules::Zone;
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: Zone::Center,
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
        let mut app = App::new(config);
        let _ = app.update(Message::BarFocusGained);
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }
    ```
  - [x] 9.6: Test `BarFocusGained` with no modules at all is a no-op (no panic):
    ```rust
    #[test]
    fn bar_focus_gained_with_no_modules_is_noop() {
        let mut app = App::new(make_config());
        let _ = app.update(Message::BarFocusGained);
        assert!(app.focused_module.is_none());
    }
    ```
  - [x] 9.7: Test `focused_module` starts None in App::new:
    ```rust
    #[test]
    fn focused_module_starts_none() {
        let app = App::new(make_config());
        assert!(app.focused_module.is_none());
    }
    ```

## Critical Implementation Notes

### The KeyboardInteractivityChange variant is ALREADY generated by the macro
`#[to_layer_message]` auto-injects `KeyboardInteractivityChange(iced_layershell::reexport::KeyboardInteractivity)` into `Message`. Do NOT add it manually — that would cause a compile error (duplicate variant).

### Import pattern for KeyboardInteractivity
Use inside the handler functions only to avoid bringing it into scope everywhere:
```rust
use iced_layershell::reexport::KeyboardInteractivity;
```

### listen_with vs keyboard::listen
Use `iced::event::listen_with` (NOT `iced::keyboard::listen`) because we need both window focus events AND keyboard events in one subscription. `keyboard::listen` only delivers keyboard events with `Status::Ignored`.

### Focus-release sequence
Escape → `KeyboardInteractivityChange(None)` (Wayland round-trip) → compositor gives focus to prev window → `WindowEvent::Unfocus` → `BarFocusLost` → clears state + `KeyboardInteractivityChange(OnDemand)`.
Do NOT try to restore `OnDemand` in the same update tick as `None` — the order of two `Task::done` in a `Task::batch` is not guaranteed to produce separate Wayland commits.

### UX-DR3: Identical visual treatment
`module_frame` already renders the `#313244` background with 4px radius for hover. Pass `hovered || focused` — do NOT add a separate `focused` parameter to `module_frame`. The visual treatment is identical, so combining into one boolean is correct.

### SurfaceState placeholder comment
`src/surface/mod.rs` line 14 already has `// focused_module: Option<ModuleId> — added in Story 6.1`. Replace this comment with the actual field — do not leave both.

### ModuleId import in surface/mod.rs
`ModuleId` is in `crate::modules`. Add `use crate::modules::ModuleId;` at the top of `src/surface/mod.rs`.

### Zone enum import in app.rs
`Zone` is already imported via `use crate::modules::{..., Zone, ...}` — check the existing import and add `Zone` if missing.

### No config changes needed
The keybind is configured in the user's compositor (niri: `bind Mod+b { focus-layer-surface; }`). niribar has no config for this. Do NOT add any config fields for the focus keybind.

### No main.rs changes needed
`LayerShellSettings::default()` already sets `keyboard_interactivity: KeyboardInteractivity::OnDemand`. The bar can already receive keyboard focus. Verify this by checking `..Default::default()` in `main.rs` `LayerShellSettings` constructor.

## Previous Story Context (Story 5.4 patterns)

- Event subscription pattern: `iced::Subscription::batch([..., new_sub])` — follow the same pattern in `App::subscription()`
- `Task::done(Message::XyzChange(...))` pattern — same as `Task::done(Message::SizeChange(...))` used in expand/collapse
- Test structure: all tests in `#[cfg(test)] mod tests` at the bottom of `src/app.rs` — add new tests there
- `make_config()` helper already exists in tests — reuse it
- `ModuleId::new("clock")` pattern — use for test assertions

## Definition of Done

- [x] `src/surface/input.rs` exists with `keyboard_focus_subscription()`
- [x] `SurfaceState.focused_module` field is uncommented/added
- [x] `App.focused_module` field initialized to `None`
- [x] `Message::BarFocusGained`, `Message::BarFocusLost`, `Message::EscapePressed` handled in `update()`
- [x] `keyboard_focus_subscription()` wired into `App::subscription()`
- [x] `view()` passes `hovered || focused` to `module_frame`
- [x] `cargo clippy -- -D warnings` clean
- [x] `cargo test` shows ≥ 236 tests passing (237 actual)

## Dev Agent Record

### File List

- `src/surface/input.rs` — **CREATED**: `keyboard_focus_subscription()` using `iced::event::listen_with`
- `src/surface/mod.rs` — **MODIFIED**: added `pub mod input;`, `focused_module: Option<ModuleId>` field to `SurfaceState`, updated `new()` and tests
- `src/app.rs` — **MODIFIED**: new messages (`BarFocusGained`, `BarFocusLost`, `EscapePressed`), new `App.focused_module` field, three new update handlers, `keyboard_focus_subscription` import and wiring, `view()` uses `hovered || focused`

### Completion Notes

Implemented keyboard focus receipt for the niribar layer-shell surface. The bar already had `KeyboardInteractivity::OnDemand` by default (via `LayerShellSettings::default()`), so no `main.rs` changes were needed — the user just needs to bind `focus-layer-surface` in their niri config.

Focus state is tracked in `App.focused_module` (mirrors future `SurfaceState.focused_module`). On `BarFocusGained`, the first module in the top zone is highlighted; falls back to center then bottom. Escape sends `KeyboardInteractivityChange(None)` to release focus; `BarFocusLost` re-enables `OnDemand` after the Wayland round-trip. Visual treatment uses the existing `module_frame` with `hovered || focused` — identical appearance to hover per UX-DR3.

237 tests pass (229 pre-existing + 8 new). `cargo clippy -- -D warnings` clean.

### Change Log

- 2026-04-05: Story 6.1 implemented — bar keyboard focus via `focus-layer-surface` compositor keybind, Escape to release focus, `focused_module` highlight on first top-zone module.
