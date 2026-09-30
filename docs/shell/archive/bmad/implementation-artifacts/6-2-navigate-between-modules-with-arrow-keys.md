# Story 6.2: Navigate Between Modules with Arrow Keys

Status: review

## Story

As a **user with the bar focused**,
I want to move between modules using arrow keys,
So that I can reach any module without a pointer.

## Acceptance Criteria

1. **Given** the bar has keyboard focus
   **When** the user presses the Down arrow key
   **Then** focus moves to the next module within the current zone
   **And** if the focused module is the last in its zone, nothing changes (no wrap)

2. **Given** the bar has keyboard focus
   **When** the user presses the Up arrow key
   **Then** focus moves to the previous module within the current zone
   **And** if the focused module is the first in its zone, nothing changes (no wrap)

3. **Given** the bar has keyboard focus
   **When** the user presses Tab
   **Then** focus moves to the first module in the next zone (cycle: top → center → bottom → top)
   **And** empty zones are skipped

4. **Given** the bar has keyboard focus
   **When** the user presses Shift+Tab
   **Then** focus moves to the first module in the previous zone (cycle: top → bottom → center → top)
   **And** empty zones are skipped

5. **Given** the bar does NOT have keyboard focus (focused_module is None)
   **When** any navigation key is pressed (Up/Down/Tab/Shift+Tab)
   **Then** nothing happens

6. **Given** the focused module is visually highlighted at all times
   **When** focus moves between modules
   **Then** only the newly focused module shows the `ModuleFrame` highlight (`#313244`, 4px radius)

7. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

8. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 237 pre-existing tests pass and at least 10 new tests pass (≥ 247 total)

## Navigation Model

`self.modules` is `Vec<(Zone, Box<dyn Module>)>` ordered exactly as configured. Navigation:

- **Down (FocusMoveNext)**: from current index, find next entry with same zone at a higher index. If none → no-op.
- **Up (FocusMovePrev)**: from current index, find previous entry with same zone at a lower index (search `[0..current_idx]` in reverse). If none → no-op.
- **Tab (FocusNextZone)**: find current zone, advance in zone cycle `[Top, Center, Bottom]` (wrapping), skip zones with no modules. Set focused_module to first module in found zone.
- **Shift+Tab (FocusPrevZone)**: same but cycle is `[Top, Bottom, Center]` (reverse-wrap: Top→Bottom→Center→Top).

**Zone cycle clarification:**
- Tab forward: Top→Center→Bottom→Top (index 0→1→2→0)
- Shift+Tab backward: Top→Bottom→Center→Top (index 0→2→1→0)

**Edge case: only one zone has modules** — Tab and Shift+Tab both wrap back to the first module in that zone.

**Focused module is always valid**: navigation keys do nothing when `focused_module.is_none()`.

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `crates/niribar/src/surface/input.rs` | **MODIFY** | Add ArrowDown, ArrowUp, Tab, Shift+Tab to `keyboard_focus_subscription()` |
| `crates/niribar/src/app.rs` | **MODIFY** | 4 new Message variants, 4 new update handlers, 4 new private nav methods, tests |

## Tasks / Subtasks

- [x] Task 1: Add navigation key messages to `Message` enum in `crates/niribar/src/app.rs`
  - [x] 1.1: Add after `EscapePressed` in the `#[to_layer_message]` enum block:
    ```rust
    /// Down arrow key pressed while bar is focused — move to next module in zone (Story 6.2).
    FocusMoveNext,
    /// Up arrow key pressed while bar is focused — move to previous module in zone (Story 6.2).
    FocusMovePrev,
    /// Tab pressed while bar is focused — jump to first module in next zone (Story 6.2).
    FocusNextZone,
    /// Shift+Tab pressed while bar is focused — jump to first module in previous zone (Story 6.2).
    FocusPrevZone,
    ```

- [x] Task 2: Extend `keyboard_focus_subscription()` in `crates/niribar/src/surface/input.rs`
  - [x] 2.1: Add four new arms to the `match event` in `keyboard_focus_subscription()`:
    ```rust
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(Named::ArrowDown),
        ..
    }) => Some(Message::FocusMoveNext),
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(Named::ArrowUp),
        ..
    }) => Some(Message::FocusMovePrev),
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(Named::Tab),
        modifiers,
        ..
    }) => {
        if modifiers.shift() {
            Some(Message::FocusPrevZone)
        } else {
            Some(Message::FocusNextZone)
        }
    }
    ```
    Note: `modifiers` is bound from the pattern — the closure is still non-capturing and coerces to `fn`.

- [x] Task 3: Add 4 private navigation methods to `App` in `crates/niribar/src/app.rs`
  - [x] 3.1: Add these methods to the `impl App` block (after the existing private helpers like `push_battery_state`):
    ```rust
    /// Returns the ID of the next module in the same zone as `current`, or None if at zone end.
    fn nav_next_in_zone(&self, current: &ModuleId) -> Option<ModuleId> {
        let idx = self.modules.iter().position(|(_, m)| m.id() == *current)?;
        let zone = &self.modules[idx].0;
        self.modules[idx + 1..]
            .iter()
            .find(|(z, _)| z == zone)
            .map(|(_, m)| m.id())
    }

    /// Returns the ID of the previous module in the same zone as `current`, or None if at zone start.
    fn nav_prev_in_zone(&self, current: &ModuleId) -> Option<ModuleId> {
        let idx = self.modules.iter().position(|(_, m)| m.id() == *current)?;
        let zone = &self.modules[idx].0;
        self.modules[..idx]
            .iter()
            .rev()
            .find(|(z, _)| z == zone)
            .map(|(_, m)| m.id())
    }

    /// Returns the ID of the first module in the next non-empty zone (Tab cycle: Top→Center→Bottom→Top).
    fn nav_next_zone(&self, current: &ModuleId) -> Option<ModuleId> {
        let idx = self.modules.iter().position(|(_, m)| m.id() == *current)?;
        let current_zone = &self.modules[idx].0;
        let cycle = [Zone::Top, Zone::Center, Zone::Bottom];
        let start = cycle.iter().position(|z| z == current_zone).unwrap_or(0);
        for step in 1..=3 {
            let next_zone = &cycle[(start + step) % 3];
            if let Some((_, m)) = self.modules.iter().find(|(z, _)| z == next_zone) {
                return Some(m.id());
            }
        }
        None
    }

    /// Returns the ID of the first module in the previous non-empty zone (Shift+Tab cycle: Top→Bottom→Center→Top).
    fn nav_prev_zone(&self, current: &ModuleId) -> Option<ModuleId> {
        let idx = self.modules.iter().position(|(_, m)| m.id() == *current)?;
        let current_zone = &self.modules[idx].0;
        // Reverse cycle: Top(0)→Bottom(2)→Center(1)→Top(0)
        let cycle = [Zone::Top, Zone::Bottom, Zone::Center];
        let start = cycle.iter().position(|z| z == current_zone).unwrap_or(0);
        for step in 1..=3 {
            let prev_zone = &cycle[(start + step) % 3];
            if let Some((_, m)) = self.modules.iter().find(|(z, _)| z == prev_zone) {
                return Some(m.id());
            }
        }
        None
    }
    ```

- [x] Task 4: Add 4 update handlers in `App::update()` in `crates/niribar/src/app.rs`
  - [x] 4.1: Add handlers before the `_ => Task::none()` catch-all:
    ```rust
    Message::FocusMoveNext => {
        if let Some(ref current) = self.focused_module.clone() {
            if let Some(next) = self.nav_next_in_zone(current) {
                self.focused_module = Some(next);
            }
        }
        Task::none()
    }
    Message::FocusMovePrev => {
        if let Some(ref current) = self.focused_module.clone() {
            if let Some(prev) = self.nav_prev_in_zone(current) {
                self.focused_module = Some(prev);
            }
        }
        Task::none()
    }
    Message::FocusNextZone => {
        if let Some(ref current) = self.focused_module.clone() {
            if let Some(next) = self.nav_next_zone(current) {
                self.focused_module = Some(next);
            }
        }
        Task::none()
    }
    Message::FocusPrevZone => {
        if let Some(ref current) = self.focused_module.clone() {
            if let Some(prev) = self.nav_prev_zone(current) {
                self.focused_module = Some(prev);
            }
        }
        Task::none()
    }
    ```

- [x] Task 5: Write tests in `crates/niribar/src/app.rs`
  - [x] 5.1: Add test helper `make_nav_app()` (top: clock+battery, center: network, bottom: workspaces):
    ```rust
    fn make_nav_app() -> App {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![
            ModuleConfig {
                name: "clock".into(),
                zone: Zone::Top,
                params: ModuleParams::Clock(ClockConfig::default()),
            },
            ModuleConfig {
                name: "battery".into(),
                zone: Zone::Top,
                params: ModuleParams::None,
            },
            ModuleConfig {
                name: "network".into(),
                zone: Zone::Center,
                params: ModuleParams::None,
            },
            ModuleConfig {
                name: "workspaces".into(),
                zone: Zone::Bottom,
                params: ModuleParams::None,
            },
        ];
        App::new(config)
    }
    ```
  - [x] 5.2: Test FocusMoveNext moves to next module in same zone:
    ```rust
    #[test]
    fn focus_move_next_advances_within_zone() {
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained); // focused = clock (first top)
        let _ = app.update(Message::FocusMoveNext);
        assert_eq!(app.focused_module, Some(ModuleId::new("battery"))); // second top
    }
    ```
  - [x] 5.3: Test FocusMoveNext at zone end does nothing:
    ```rust
    #[test]
    fn focus_move_next_at_zone_end_is_noop() {
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained);
        let _ = app.update(Message::FocusMoveNext); // clock → battery
        let _ = app.update(Message::FocusMoveNext); // battery → no-op (last in top)
        assert_eq!(app.focused_module, Some(ModuleId::new("battery")));
    }
    ```
  - [x] 5.4: Test FocusMovePrev moves to previous module in same zone:
    ```rust
    #[test]
    fn focus_move_prev_retreats_within_zone() {
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained);
        let _ = app.update(Message::FocusMoveNext); // → battery
        let _ = app.update(Message::FocusMovePrev); // → clock
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }
    ```
  - [x] 5.5: Test FocusMovePrev at zone start does nothing:
    ```rust
    #[test]
    fn focus_move_prev_at_zone_start_is_noop() {
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained); // focused = clock
        let _ = app.update(Message::FocusMovePrev); // clock is first in top — no-op
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }
    ```
  - [x] 5.6: Test FocusNextZone jumps to first module in next zone:
    ```rust
    #[test]
    fn focus_next_zone_jumps_to_center() {
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained); // top: clock
        let _ = app.update(Message::FocusNextZone);  // → center: network
        assert_eq!(app.focused_module, Some(ModuleId::new("network")));
    }
    ```
  - [x] 5.7: Test FocusNextZone wraps from bottom to top:
    ```rust
    #[test]
    fn focus_next_zone_wraps_bottom_to_top() {
        let mut app = make_nav_app();
        app.focused_module = Some(ModuleId::new("workspaces")); // bottom zone
        let _ = app.update(Message::FocusNextZone); // → top: clock
        assert_eq!(app.focused_module, Some(ModuleId::new("clock")));
    }
    ```
  - [x] 5.8: Test FocusPrevZone jumps to first module in previous zone:
    ```rust
    #[test]
    fn focus_prev_zone_from_top_jumps_to_bottom() {
        let mut app = make_nav_app();
        let _ = app.update(Message::BarFocusGained); // top: clock
        let _ = app.update(Message::FocusPrevZone);  // reverse cycle: top → bottom
        assert_eq!(app.focused_module, Some(ModuleId::new("workspaces")));
    }
    ```
  - [x] 5.9: Test FocusNextZone skips empty zone:
    ```rust
    #[test]
    fn focus_next_zone_skips_empty_zone() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        // Only top and bottom; center is empty.
        let mut config = Config::default();
        config.modules = vec![
            ModuleConfig {
                name: "clock".into(),
                zone: Zone::Top,
                params: ModuleParams::Clock(ClockConfig::default()),
            },
            ModuleConfig {
                name: "workspaces".into(),
                zone: Zone::Bottom,
                params: ModuleParams::None,
            },
        ];
        let mut app = App::new(config);
        let _ = app.update(Message::BarFocusGained); // top: clock
        let _ = app.update(Message::FocusNextZone);  // skip empty center → bottom
        assert_eq!(app.focused_module, Some(ModuleId::new("workspaces")));
    }
    ```
  - [x] 5.10: Test navigation keys when not focused are no-ops:
    ```rust
    #[test]
    fn navigation_keys_when_not_focused_are_noop() {
        let mut app = make_nav_app();
        // Bar not focused — focused_module is None
        let _ = app.update(Message::FocusMoveNext);
        let _ = app.update(Message::FocusMovePrev);
        let _ = app.update(Message::FocusNextZone);
        let _ = app.update(Message::FocusPrevZone);
        assert!(app.focused_module.is_none());
    }
    ```

## Critical Implementation Notes

### Modifiers in non-capturing closure
`modifiers` is bound from the destructured event pattern, not captured from outside the closure. The closure `|event, _status, _window|` in `keyboard_focus_subscription()` remains non-capturing and coerces to `fn`. The `modifiers.shift()` call inside the match arm is valid.

### Zone cycle for nav_prev_zone
Shift+Tab goes to the **previous** zone. The AC says top→center→bottom is the Tab order, so Shift+Tab should be the reverse: bottom→center→top. But re-read: "Shift+Tab moves to the first module in the previous zone". From Top, the previous zone is Bottom (wrapping). From Center, previous is Top. From Bottom, previous is Center. This gives the reverse cycle `[Top, Bottom, Center]` (step 1 from Top(0) → index 1 = Bottom, step 1 from Bottom(1) → index 2 = Center, step 1 from Center(2) → index 0 = Top).

### Clone trick for focused_module borrow
In the update handlers, `self.focused_module.clone()` avoids a borrow conflict when calling `self.nav_*` methods (which borrow `self.modules`). This is the standard Rust pattern when you need to read a field then mutate another.

### No visual changes needed in view()
Story 6.1 already wired `focused_module` into `view()` via `hovered || focused`. As long as `focused_module` is updated correctly, the visual highlight follows automatically.

### ModuleParams::None for battery/network/workspaces in tests
All non-button/non-script module constructors handle `ModuleParams::None` with default values (see `battery_ctor`, `network_ctor`, `workspaces_ctor`). Safe to use in tests.

### ModuleId uniqueness in tests
Use different module types (clock, battery, network, workspaces) for test fixtures to ensure each has a unique ID. Never use two modules of the same type in a test without understanding they share the same `ModuleId`.

## Previous Story Learnings (Story 6.1)

- `keyboard_focus_subscription()` uses `iced::event::listen_with` with a non-capturing closure — extend the match arms in the same closure
- `focused_module: Option<ModuleId>` is already on `App` — navigation handlers just update it
- The `_ => Task::none()` catch-all in `update()` is where new handlers go before it
- Test pattern: `App::new(config)` then `app.update(Message::BarFocusGained)` sets initial focus
- `make_config()` returns default config with no modules — use it for "unfocused" tests

## Definition of Done

- [x] 4 new `Message` variants added (`FocusMoveNext`, `FocusMovePrev`, `FocusNextZone`, `FocusPrevZone`)
- [x] `keyboard_focus_subscription()` emits all 4 new messages on correct key presses
- [x] 4 private nav methods on `App` (`nav_next_in_zone`, `nav_prev_in_zone`, `nav_next_zone`, `nav_prev_zone`)
- [x] 4 update handlers in `App::update()`
- [x] `cargo clippy -- -D warnings` clean
- [x] `cargo test` shows ≥ 247 tests passing

## File List

- `crates/niribar/src/app.rs` — added 4 `Message` variants, 4 private nav methods, 4 update handlers, 11 new tests (including helper)
- `crates/niribar/src/surface/input.rs` — extended `keyboard_focus_subscription()` with ArrowDown, ArrowUp, Tab/Shift+Tab arms
- `_bmad-output/implementation-artifacts/6-2-navigate-between-modules-with-arrow-keys.md` — story file (this file)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` — status updated to review

## Dev Agent Record

### Completion Notes

Implemented Story 6.2 (Navigate Between Modules with Arrow Keys) in full:

- **Task 1**: Added `FocusMoveNext`, `FocusMovePrev`, `FocusNextZone`, `FocusPrevZone` to `Message` enum in `crates/niribar/src/app.rs`
- **Task 2**: Extended `keyboard_focus_subscription()` in `crates/niribar/src/surface/input.rs` with ArrowDown→FocusMoveNext, ArrowUp→FocusMovePrev, Tab→FocusNextZone, Shift+Tab→FocusPrevZone
- **Task 3**: Added 4 private nav methods to `impl App`: `nav_next_in_zone`, `nav_prev_in_zone`, `nav_next_zone`, `nav_prev_zone`
- **Task 4**: Added 4 update handlers in `App::update()` before the catch-all `_ => Task::none()`
- **Task 5**: Added `make_nav_app()` helper + 10 tests covering all navigation behaviours (5.2–5.10 + extra `focus_prev_zone_from_center_jumps_to_top`)

Clippy fix: collapsed nested `if let` blocks to `if let … && let …` form as required by `-D warnings`.

All 247 tests pass, zero clippy warnings.

## Change Log

- 2026-04-05: Implemented Story 6.2 — arrow-key navigation between modules. Added 4 Message variants, keyboard subscription arms, 4 nav methods, 4 update handlers, 10 new tests. Total: 247 tests passing, clippy clean.
