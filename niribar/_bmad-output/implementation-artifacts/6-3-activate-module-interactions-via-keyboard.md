# Story 6.3: Activate Module Interactions via Keyboard

Status: review

## Story

As a **user navigating the bar with the keyboard**,
I want to trigger module actions (click, scroll) from the keyboard,
so that I can interact with any module without switching to the mouse.

## Acceptance Criteria

1. **Given** the bar has keyboard focus and a module is highlighted
   **When** the user presses Enter or Space
   **Then** the module's primary action fires (equivalent to a pointer click — e.g., button executes its `on_click` command)

2. **Given** the bar has keyboard focus and the focused module is a scrollable module (e.g., volume)
   **When** the user presses the Down arrow key
   **Then** the scroll-down (decrement) action fires instead of moving focus to the next module

3. **Given** the bar has keyboard focus and the focused module is a scrollable module (e.g., volume)
   **When** the user presses the Up arrow key
   **Then** the scroll-up (increment) action fires instead of moving focus to the previous module

4. **Given** the bar has keyboard focus and the focused module is NOT scrollable (e.g., clock)
   **When** the user presses Up or Down arrow
   **Then** normal focus navigation occurs (unchanged behaviour from Story 6.2)

5. **Given** the bar does NOT have keyboard focus (`focused_module` is None)
   **When** Enter or Space is pressed
   **Then** nothing happens

6. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

7. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 248 pre-existing tests pass and at least 10 new tests pass (≥ 258 total)

## Navigation Model Clarification (from Story 6.2 as implemented)

The actual implementation of 6.2 diverged from the story plan. The real methods:

- **`nav_next`** — advances globally (wraps last → first), NOT zone-bounded
- **`nav_prev`** — retreats globally (wraps first → last), NOT zone-bounded
- **`nav_next_zone`** / **`nav_prev_zone`** — Tab/Shift+Tab zone cycling, unchanged

Story 6.3 must modify the `FocusMoveNext` and `FocusMovePrev` handlers (which call `nav_next`/`nav_prev`) to check scroll priority first.

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `src/surface/input.rs` | **MODIFY** | Add `Enter` and `Space` arms to `keyboard_focus_subscription()` → `Message::KeyboardActivate` |
| `src/app.rs` | **MODIFY** | 1 new `Message` variant; 1 new handler; modify 2 existing handlers (`FocusMoveNext`, `FocusMovePrev`) for scroll priority |

## Tasks / Subtasks

- [x] Task 1: Add `Message::KeyboardActivate` variant to `Message` enum in `src/app.rs` (AC: #1, #5)
  - [x] 1.1: Add after `FocusPrevZone` in the `#[to_layer_message]` enum block:
    ```rust
    /// Enter or Space pressed while bar is focused — activate focused module (Story 6.3).
    KeyboardActivate,
    ```

- [x] Task 2: Add Enter and Space key arms to `keyboard_focus_subscription()` in `src/surface/input.rs` (AC: #1)
  - [x] 2.1: Add two arms before the `_ => None` catchall:
    ```rust
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(Named::Enter),
        ..
    }) => Some(Message::KeyboardActivate),
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(Named::Space),
        ..
    }) => Some(Message::KeyboardActivate),
    ```
    Note: `Named::Space` is `iced::keyboard::key::Named::Space`. Both `Enter` and `Space` map to the same message — they are interchangeable activators.

- [x] Task 3: Add `KeyboardActivate` update handler in `App::update()` in `src/app.rs` (AC: #1, #5)
  - [x] 3.1: Add before the `_ => Task::none()` catchall (after `FocusPrevZone` handler):
    ```rust
    Message::KeyboardActivate => {
        if let Some(ref id) = self.focused_module.clone() {
            for (_, module) in &self.modules {
                if module.id() == *id {
                    if let Some(msg) = module.on_interaction(InteractionEvent::Clicked) {
                        return Task::done(msg);
                    }
                    break;
                }
            }
        }
        Task::none()
    }
    ```
    Pattern follows `Message::ModuleClick` handler at line 183. `ButtonModule::on_interaction(Clicked)` spawns the command as a side effect and returns `None` — the `if let Some` branch won't fire for buttons, but the side effect (spawn) still runs.

- [x] Task 4: Modify `FocusMoveNext` handler in `App::update()` for scroll priority (AC: #2, #4)
  - [x] 4.1: Replace the body of `Message::FocusMoveNext =>` (lines ~548–562) with scroll-priority logic:
    ```rust
    Message::FocusMoveNext => {
        if self.focused_module.is_none() {
            return Task::done(Message::BarFocusGained);
        }
        // Scroll priority: if the focused module handles ScrollDown, fire that instead of navigating.
        if let Some(ref id) = self.focused_module.clone() {
            for (_, m) in &self.modules {
                if m.id() == *id {
                    if let Some(msg) = m.on_interaction(InteractionEvent::ScrollDown) {
                        return Task::done(msg);
                    }
                    break;
                }
            }
        }
        // No scroll handler — navigate to next module.
        if let Some(ref current) = self.focused_module.clone()
            && let Some(next) = self.nav_next(current)
        {
            self.focused_module = Some(next);
        }
        Task::none()
    }
    ```

- [x] Task 5: Modify `FocusMovePrev` handler in `App::update()` for scroll priority (AC: #3, #4)
  - [x] 5.1: Replace the body of `Message::FocusMovePrev =>` (lines ~563–573) with scroll-priority logic:
    ```rust
    Message::FocusMovePrev => {
        if self.focused_module.is_none() {
            return Task::done(Message::BarFocusGained);
        }
        // Scroll priority: if the focused module handles ScrollUp, fire that instead of navigating.
        if let Some(ref id) = self.focused_module.clone() {
            for (_, m) in &self.modules {
                if m.id() == *id {
                    if let Some(msg) = m.on_interaction(InteractionEvent::ScrollUp) {
                        return Task::done(msg);
                    }
                    break;
                }
            }
        }
        // No scroll handler — navigate to previous module.
        if let Some(ref current) = self.focused_module.clone()
            && let Some(prev) = self.nav_prev(current)
        {
            self.focused_module = Some(prev);
        }
        Task::none()
    }
    ```

- [x] Task 6: Write tests in `src/app.rs` (AC: #7)
  - [x] 6.1: Reuse `make_nav_app()` from Story 6.2 tests (already present — top: clock+battery, center: network, bottom: workspaces). Add a volume-containing fixture:
    ```rust
    fn make_interact_app() -> App {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams, VolumeConfig};
        let mut config = Config::default();
        config.modules = vec![
            ModuleConfig {
                name: "clock".into(),
                zone: Zone::Top,
                params: ModuleParams::Clock(ClockConfig::default()),
            },
            ModuleConfig {
                name: "volume".into(),
                zone: Zone::Top,
                params: ModuleParams::Volume(VolumeConfig::default()),
            },
            ModuleConfig {
                name: "button".into(),
                zone: Zone::Center,
                params: ModuleParams::Button(crate::config::ButtonConfig {
                    label: "test".into(),
                    icon: ">".into(),
                    on_click: "true".into(), // harmless command
                }),
            },
        ];
        App::new(config)
    }
    ```
  - [x] 6.2: Test `KeyboardActivate` when not focused is a no-op:
    ```rust
    #[test]
    fn keyboard_activate_when_not_focused_is_noop() {
        let mut app = make_interact_app();
        // focused_module is None — no state change
        let task = app.update(Message::KeyboardActivate);
        assert!(app.focused_module.is_none());
        // Task::none() has no pending work; just verify no panic
        let _ = task;
    }
    ```
  - [x] 6.3: Test `KeyboardActivate` on a module with no click handler returns `Task::none()` (clock has no `on_interaction`):
    ```rust
    #[test]
    fn keyboard_activate_on_non_interactive_module_is_noop() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // focuses clock (first top)
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
        let _ = app.update(Message::KeyboardActivate);
        // clock.on_interaction(Clicked) returns None → focused_module unchanged
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }
    ```
  - [x] 6.4: Test Down arrow on volume fires scroll (not navigation):
    ```rust
    #[test]
    fn focus_move_next_on_volume_fires_scroll_down() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock (first Top)
        let _ = app.update(Message::FocusMoveNext);   // → volume (second Top, scrollable)
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
        // Now Down arrow on volume should fire scroll, not advance focus.
        let _ = app.update(Message::FocusMoveNext);
        // Focus stays on volume (scroll fired, not navigation)
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
    }
    ```
  - [x] 6.5: Test Up arrow on volume fires scroll (not navigation):
    ```rust
    #[test]
    fn focus_move_prev_on_volume_fires_scroll_up() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        let _ = app.update(Message::FocusMoveNext);   // → volume
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
        let _ = app.update(Message::FocusMovePrev);
        // Focus stays on volume (ScrollUp fired, not navigation backward)
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
    }
    ```
  - [x] 6.6: Test Down arrow on non-scrollable module navigates as before:
    ```rust
    #[test]
    fn focus_move_next_on_non_scrollable_navigates() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        // clock has no on_interaction(ScrollDown) → navigation fires
        let _ = app.update(Message::FocusMoveNext);
        assert_eq!(app.focused_module, Some(ModuleId::new("volume")));
    }
    ```
  - [x] 6.7: Test Up arrow on non-scrollable module navigates as before:
    ```rust
    #[test]
    fn focus_move_prev_on_non_scrollable_navigates() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        let _ = app.update(Message::FocusMoveNext);   // → volume
        let _ = app.update(Message::FocusMoveNext);   // volume scrolls — still on volume
        // Tab to button (next zone)
        let _ = app.update(Message::FocusNextZone);   // → button (center zone)
        assert_eq!(app.focused_module, Some(ModuleId::new("button")));
        // button has no on_interaction(ScrollUp) → navigates backward
        let _ = app.update(Message::FocusMovePrev);
        // Wraps/navigates — focus leaves button
        assert_ne!(app.focused_module, Some(ModuleId::new("button")));
    }
    ```
  - [x] 6.8: Test Tab still works normally (no regression):
    ```rust
    #[test]
    fn tab_zone_navigation_unaffected() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock (Top)
        let _ = app.update(Message::FocusNextZone);  // → button (Center, first non-empty next zone)
        assert_eq!(app.focused_module, Some(ModuleId::new("button")));
    }
    ```
  - [x] 6.9: Test `KeyboardActivate` while focused on button does not panic and side-effect fires:
    ```rust
    #[test]
    fn keyboard_activate_on_button_does_not_panic() {
        let mut app = make_interact_app();
        let _ = app.update(Message::BarFocusGained); // → clock
        let _ = app.update(Message::FocusNextZone);   // → button (center)
        assert_eq!(app.focused_module, Some(ModuleId::new("button")));
        // ButtonModule::on_interaction(Clicked) spawns "true" — side effect, returns None.
        let _ = app.update(Message::KeyboardActivate);
        // No panic; focused_module unchanged
        assert_eq!(app.focused_module, Some(ModuleId::new("button")));
    }
    ```
  - [x] 6.10: Test that FocusMoveNext on None (bar re-focus fallback) still delegates to BarFocusGained:
    ```rust
    #[test]
    fn focus_move_next_on_unfocused_delegates_to_bar_focus_gained() {
        let mut app = make_interact_app();
        // focused_module is None — should delegate to BarFocusGained
        assert!(app.focused_module.is_none());
        // FocusMoveNext when not focused → BarFocusGained task returned
        // We can't inspect Task internals, but ensure no panic and bar becomes focused after
        // simulating the delegated message:
        let _ = app.update(Message::FocusMoveNext);
        // Actually exercise the delegation path by manually following through:
        let _ = app.update(Message::BarFocusGained);
        assert!(app.focused_module.is_some());
    }
    ```

## Critical Implementation Notes

### Scroll priority in `FocusMoveNext`/`FocusMovePrev`
`on_interaction(ScrollDown)` on `VolumeModule` constructs a `VolumeAdjust` message — no side effects, safe to call as a check. For modules that return `None` (clock, battery, network, workspaces, button), the check is a no-op. The `break` after the matching module is found ensures we don't iterate further.

### `ButtonModule::on_interaction(Clicked)` has side effects but returns `None`
When `KeyboardActivate` hits a focused button module, `on_interaction(Clicked)` spawns the `on_click` shell command (via `std::process::Command::spawn()`) as a side effect and returns `None`. The handler correctly uses `if let Some(msg) = ...` — when None is returned, `Task::none()` is returned. This is correct: the button action already fired as a side effect.

### `Named::Space` import
`iced::keyboard::key::Named::Space` is already in scope via the `use iced::keyboard::key::Named;` import at the top of `src/surface/input.rs` (same file as existing key arms). No new import needed.

### `InteractionEvent` import in `app.rs`
`InteractionEvent` is already imported in `src/app.rs` at line 19: `use crate::modules::{InteractionEvent, ...}`. No new import needed.

### Clone borrow pattern
The `self.focused_module.clone()` pattern is already used in all existing nav handlers. Follow the same pattern in Task 3/4/5.

### `make_interact_app()` module configuration
- `ModuleParams::Button` requires a `ButtonConfig` struct. Check `src/config/mod.rs` or `src/config/parse.rs` for the exact field names (`label`, `icon`, `on_click`).
- `VolumeConfig::default()` gives `step: 5` (default per config).
- Volume module ID is always `"volume"` (set in `VolumeModule::new()`).
- Button module ID is always `"button"` (set in `ButtonModule::new()`).
- Clock module ID is always `"clock"`.

### `to_layer_message` attribute
The `Message` enum uses `#[to_layer_message]` derive macro. `KeyboardActivate` should be placed after `FocusPrevZone` in the enum, following the same pattern as all other navigation-related variants added in Story 6.2.

### Test for scroll fires VolumeAdjust
In tests 6.4/6.5, we assert that focus doesn't move (stays on `"volume"`). We can't inspect the `Task` to verify `VolumeAdjust` was queued — but the invariant is: if scroll fired, focus didn't move. This is a valid proxy.

## Previous Story Learnings (Story 6.2)

- The actual nav methods are `nav_next`/`nav_prev` (global wrap), NOT `nav_next_in_zone`/`nav_prev_in_zone`
- `keyboard_focus_subscription()` uses `listen_with` with a non-capturing closure — add new key arms in the same closure
- `focused_module.is_none()` guard + delegation to `BarFocusGained` is the pattern for all key handlers
- Test pattern: `App::new(config)` → `app.update(Message::BarFocusGained)` sets initial focus to first Top module
- `make_nav_app()` (top: clock+battery, center: network, bottom: workspaces) already exists — reuse for regression tests; add `make_interact_app()` for interaction-specific tests
- The `_ => Task::none()` catch-all is at line ~596; new handlers go before it

## Definition of Done

- [x] `Message::KeyboardActivate` variant added to `Message` enum
- [x] `keyboard_focus_subscription()` emits `KeyboardActivate` on Enter and Space
- [x] `KeyboardActivate` update handler fires `on_interaction(Clicked)` on focused module
- [x] `FocusMoveNext` checks `on_interaction(ScrollDown)` before navigating
- [x] `FocusMovePrev` checks `on_interaction(ScrollUp)` before navigating
- [x] `cargo clippy -- -D warnings` clean
- [x] `cargo test` shows ≥ 258 tests passing

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

Implemented Story 6.3 (Activate Module Interactions via Keyboard) in full:

- **Task 1**: Added `KeyboardActivate` to `Message` enum in `src/app.rs` (after `FocusPrevZone`)
- **Task 2**: Extended `keyboard_focus_subscription()` in `src/surface/input.rs` with Enter → `KeyboardActivate` and Space → `KeyboardActivate` arms
- **Task 3**: Added `KeyboardActivate` handler in `App::update()` — iterates modules to find focused one and calls `on_interaction(Clicked)`; button module spawns command as side effect and returns None
- **Task 4**: Modified `FocusMoveNext` handler — scroll priority check: calls `on_interaction(ScrollDown)` on focused module; if Some (volume), fires `VolumeAdjust`; if None, navigates with `nav_next`
- **Task 5**: Modified `FocusMovePrev` handler — same scroll priority for `ScrollUp`
- **Task 6**: Added `make_interact_app()` fixture (clock/volume/button) + 10 new tests covering all ACs

All 258 tests pass (248 pre-existing + 10 new), zero clippy warnings.

### File List

- `src/app.rs` — added `KeyboardActivate` Message variant, handler, modified `FocusMoveNext`/`FocusMovePrev` for scroll priority, 10 new tests + `make_interact_app()` helper
- `src/surface/input.rs` — added Enter and Space arms to `keyboard_focus_subscription()`
- `_bmad-output/implementation-artifacts/6-3-activate-module-interactions-via-keyboard.md` — story file (this file)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` — status updated to review

## Change Log

- 2026-04-05: Implemented Story 6.3 — keyboard activation of module interactions. Added `KeyboardActivate` message (Enter/Space), scroll-priority logic in `FocusMoveNext`/`FocusMovePrev` (volume scrolls instead of navigating), `KeyboardActivate` handler. 10 new tests. Total: 258 tests passing, clippy clean.
- 2026-04-05: Corrected interaction model — replaced scroll-priority-on-arrow with explicit `module_activated: bool` state. Arrow keys now always navigate unless module is activated; Enter/Space activates scrollable modules or fires click; Escape deactivates back to navigation. Added `module_activated` to `App` struct, updated EscapePressed, BarFocusGained/Lost, FocusNextZone/FocusPrevZone. Tests rewritten to cover two-mode model. 259 tests passing, clippy clean.
