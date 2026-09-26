# Story 3.3: Hover-Triggered Expand/Collapse

Status: review

## Story

As a **user who wants the bar to open automatically**,
I want the bar to expand when my pointer enters it and collapse when it leaves,
so that I don't need to click to see full module labels.

## Acceptance Criteria

1. **Given** `bar.expand-trigger = "hover"` is set in config
   **When** the pointer enters the bar surface
   **Then** the bar begins expanding after the configured `bar.hover-delay` (default 0ms)

2. **Given** `bar.expand-trigger = "hover"` is set in config
   **When** the pointer leaves the bar surface
   **Then** the bar begins collapsing

3. **Given** `bar.expand-trigger = "click"` (the default)
   **When** the pointer enters or exits the bar surface
   **Then** hover has no effect on expand state (click behavior is unchanged)

4. **Given** `bar.hover-delay` is set to N milliseconds
   **When** the pointer enters the bar and `expand-trigger = "hover"`
   **Then** the expand animation does not begin until N ms have elapsed
   **And** if the pointer exits before N ms elapse, the pending expand is cancelled

5. **Given** `bar.expand-trigger = "hover"`
   **When** the pointer exits mid-animation (bar is Expanding or Collapsing)
   **Then** the animation reverses via the existing `toggle()` mid-animation reversal

6. **Given** `bar.expand-trigger = "hover"`
   **When** the pointer clicks the bar
   **Then** the click does NOT additionally toggle the bar (hover is the sole trigger)

7. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

8. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 123 pre-existing tests pass and at least 10 new tests pass

## Tasks / Subtasks

- [x] Task 1: Add `ExpandTrigger` enum and new `BarConfig` fields (AC: #1, #3, #4, #6)
  - [x] 1.1: In `src/config/mod.rs`, add `ExpandTrigger` enum after the `Edge` enum:
    ```rust
    /// Determines what user action triggers bar expand/collapse.
    /// Hover and click are mutually exclusive — when hover is set, click has no effect.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub enum ExpandTrigger {
        #[default]
        Click,
        Hover,
    }

    impl ExpandTrigger {
        pub(crate) fn from_str(s: &str) -> anyhow::Result<ExpandTrigger> {
            match s {
                "click" => Ok(ExpandTrigger::Click),
                "hover" => Ok(ExpandTrigger::Hover),
                other => anyhow::bail!(
                    "bar.expand-trigger: unknown value {other:?}. \
                     Expected: one of: click, hover."
                ),
            }
        }
    }
    ```

  - [x] 1.2: Add two new fields to `BarConfig` (after `expanded_width`):
    ```rust
    pub struct BarConfig {
        pub edge: Edge,
        pub collapsed_width: u32,
        pub expanded_width: u32,
        /// What triggers expand/collapse. Default: Click.
        pub expand_trigger: ExpandTrigger,
        /// Dwell delay before hover expands the bar (ms). Only used when expand_trigger = Hover.
        pub hover_delay: u64,
    }

    impl Default for BarConfig {
        fn default() -> Self {
            BarConfig {
                edge: Edge::Left,
                collapsed_width: 48,
                expanded_width: 200,
                expand_trigger: ExpandTrigger::Click,
                hover_delay: 0,
            }
        }
    }
    ```

  - [x] 1.3: Update `parse_bar_section()` in `src/config/parse.rs` — add after the `expanded_width` parse block, following the exact same patterns already in the file:
    ```rust
    let expand_trigger = match children.get("expand-trigger").and_then(|n| n.entries().first()) {
        None => defaults.expand_trigger,
        Some(v) => {
            let s = v.value().as_string().ok_or_else(|| {
                anyhow::anyhow!(
                    "bar.expand-trigger: expected a string value. \
                     Expected: one of: click, hover."
                )
            })?;
            ExpandTrigger::from_str(s)?
        }
    };

    let hover_delay = children
        .get_arg("hover-delay")
        .and_then(|v| v.as_integer())
        .and_then(|v| u64::try_from(v).ok())
        .unwrap_or(defaults.hover_delay);
    ```
    Add `expand_trigger` and `hover_delay` to the `BarConfig { ... }` constructor at the end of `parse_bar_section`.

  - [x] 1.4: Add `expand-trigger "click"` to `config/default.kdl` under the `bar` block (before the closing brace). Do NOT add `hover-delay` to the default config — it is only relevant when trigger is hover and the 0ms default needs no mention.

- [x] Task 2: Extend `App` — new fields and messages (AC: #1, #2, #3, #4, #5, #6)
  - [x] 2.1: Add `use crate::config::ExpandTrigger;` to the imports in `src/app.rs`.

  - [x] 2.2: Add two fields to `App` struct (after `expand`):
    ```rust
    pub struct App {
        pub config: Config,
        pub shared: SharedState,
        pub modules: Vec<(Zone, Box<dyn Module>)>,
        pub hovered_module: Option<ModuleId>,
        pub expand: ExpandState,
        /// Whether the pointer is currently inside the bar surface (for hover trigger logic).
        pub bar_hovered: bool,
        /// Hover dwell elapsed (ms). `Some(elapsed)` = waiting for hover_delay before expanding.
        pub hover_pending_ms: Option<f32>,
    }
    ```
    Initialize in `App::new()`: `bar_hovered: false, hover_pending_ms: None`

  - [x] 2.3: Add two new variants to `Message` (after `BarClick`):
    ```rust
    /// Pointer entered bar surface; may trigger expand when expand_trigger = Hover.
    BarHoverEnter,
    /// Pointer left bar surface; may trigger collapse when expand_trigger = Hover.
    BarHoverExit,
    ```

  - [x] 2.4: Add `Message::BarHoverEnter` handler in `update()` — insert BEFORE the `_ => Task::none()` catch-all:
    ```rust
    Message::BarHoverEnter => {
        self.bar_hovered = true;
        if self.config.bar.expand_trigger != ExpandTrigger::Hover {
            return Task::none();
        }
        // Only expand from Collapsed; ignore if already expanded or animating upward.
        match self.expand {
            ExpandState::Collapsed => {
                if self.config.bar.hover_delay == 0 {
                    self.expand = self.expand.toggle(); // → Expanding(0.0)
                    let w = self.expand.current_width(
                        self.config.bar.collapsed_width,
                        self.config.bar.expanded_width,
                    );
                    Task::batch([
                        Task::done(Message::SizeChange((w, 0))),
                        Task::done(Message::ExclusiveZoneChange(
                            i32::try_from(w).unwrap_or(i32::MAX),
                        )),
                    ])
                } else {
                    self.hover_pending_ms = Some(0.0);
                    Task::none()
                }
            }
            ExpandState::Collapsing(_) => {
                // Pointer re-entered while collapsing — reverse back to expanding.
                self.expand = self.expand.toggle();
                Task::none()
            }
            _ => Task::none(), // Already expanded or expanding — no action
        }
    }
    ```

  - [x] 2.5: Add `Message::BarHoverExit` handler (after `BarHoverEnter`, before `_ => Task::none()`):
    ```rust
    Message::BarHoverExit => {
        self.bar_hovered = false;
        if self.config.bar.expand_trigger != ExpandTrigger::Hover {
            return Task::none();
        }
        // Cancel any pending hover delay.
        self.hover_pending_ms = None;
        // Collapse if bar is expanded or currently expanding.
        match self.expand {
            ExpandState::Expanded | ExpandState::Expanding(_) => {
                self.expand = self.expand.toggle(); // → Collapsing(0.0) or Collapsing(1.0-p)
                let w = self.expand.current_width(
                    self.config.bar.collapsed_width,
                    self.config.bar.expanded_width,
                );
                Task::batch([
                    Task::done(Message::SizeChange((w, 0))),
                    Task::done(Message::ExclusiveZoneChange(
                        i32::try_from(w).unwrap_or(i32::MAX),
                    )),
                ])
            }
            _ => Task::none(), // Already collapsed or collapsing — no action
        }
    }
    ```

  - [x] 2.6: Update `Message::BarClick` handler to be a no-op when expand_trigger = Hover (AC #6):
    ```rust
    Message::BarClick => {
        // Click has no effect when hover is the expand trigger (hover is the sole trigger).
        if self.config.bar.expand_trigger == ExpandTrigger::Hover {
            return Task::none();
        }
        // Existing logic unchanged: toggle and resize.
        self.expand = self.expand.toggle();
        let w = self.expand.current_width( ... );
        Task::batch([ ... ])
    }
    ```

  - [x] 2.7: Extend `subscription()` to emit Tick when hover delay is pending:
    ```rust
    pub fn subscription(&self) -> iced::Subscription<Message> {
        let module_subs =
            iced::Subscription::batch(self.modules.iter().map(|(_, m)| m.subscription()));
        if self.expand.is_animating() || self.hover_pending_ms.is_some() {
            let tick = iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick);
            iced::Subscription::batch([module_subs, tick])
        } else {
            module_subs
        }
    }
    ```

  - [x] 2.8: Extend `Message::Tick` handler to process hover delay countdown. Add this block at the TOP of the `Tick` arm, BEFORE the existing `match self.expand { ... }` block:
    ```rust
    Message::Tick => {
        const EXPAND_MS: f32 = 280.0;
        const COLLAPSE_MS: f32 = 200.0;
        const TICK_MS: f32 = 16.0;

        // Hover delay countdown — runs while waiting to start expand.
        if let Some(elapsed) = self.hover_pending_ms {
            let new_elapsed = elapsed + TICK_MS;
            if new_elapsed >= self.config.bar.hover_delay as f32 {
                // Delay expired: start expanding.
                self.hover_pending_ms = None;
                self.expand = ExpandState::Expanding(0.0);
                // Return the initial resize (no visual change at progress=0).
                let w = self.expand.current_width(
                    self.config.bar.collapsed_width,
                    self.config.bar.expanded_width,
                );
                return Task::batch([
                    Task::done(Message::SizeChange((w, 0))),
                    Task::done(Message::ExclusiveZoneChange(
                        i32::try_from(w).unwrap_or(i32::MAX),
                    )),
                ]);
            } else {
                self.hover_pending_ms = Some(new_elapsed);
                return Task::none(); // Still counting down
            }
        }

        // Existing animation progress logic (unchanged from Story 3.2):
        match self.expand {
            ExpandState::Expanding(progress) => { ... }
            ExpandState::Collapsing(progress) => { ... }
            _ => return Task::none(),
        }
        // ... resize task ...
    }
    ```

- [x] Task 3: Update `view()` — add hover detection to the outer mouse_area (AC: #1, #2)
  - [x] 3.1: In `view()`, extend the final `mouse_area` call to include enter/exit events:
    ```rust
    // Before (Story 3.2):
    mouse_area(bar_content).on_press(Message::BarClick).into()

    // After (Story 3.3):
    mouse_area(bar_content)
        .on_press(Message::BarClick)
        .on_enter(Message::BarHoverEnter)
        .on_exit(Message::BarHoverExit)
        .into()
    ```
    The outer mouse_area already exists. Only the two `.on_enter`/`.on_exit` calls are new.

- [x] Task 4: Tests (AC: #7, #8)
  - [x] 4.1: In `src/config/parse.rs` tests:
    - `parse_expand_trigger_hover` — `bar { expand-trigger "hover" }` → `ExpandTrigger::Hover`
    - `parse_expand_trigger_click` — `bar { expand-trigger "click" }` → `ExpandTrigger::Click`
    - `parse_hover_delay` — `bar { hover-delay 200 }` → `hover_delay == 200`
    - `parse_unknown_expand_trigger_errors` — `bar { expand-trigger "double-click" }` → error starts with `"bar.expand-trigger:"`
    - `parse_expand_trigger_defaults_to_click` — no expand-trigger in config → `ExpandTrigger::Click`

  - [x] 4.2: In `src/app.rs` tests (use a `make_hover_config()` helper that returns a `Config` with `expand_trigger: ExpandTrigger::Hover, hover_delay: 0`):
    - `hover_enter_click_trigger_does_nothing` — BarHoverEnter on click-trigger app → expand stays Collapsed
    - `hover_exit_click_trigger_does_nothing` — BarHoverExit on click-trigger app (expanded) → expand stays Expanded
    - `hover_enter_hover_trigger_zero_delay_starts_expanding` — BarHoverEnter on hover-trigger, hover_delay=0 → Expanding(0.0)
    - `hover_exit_when_expanded_starts_collapsing` — BarHoverExit when Expanded → Collapsing(0.0)
    - `hover_exit_when_expanding_reverses` — BarHoverExit when Expanding(0.3) → Collapsing(0.7)
    - `hover_exit_when_collapsed_does_nothing` — BarHoverExit when Collapsed → stays Collapsed
    - `hover_enter_with_delay_sets_pending` — BarHoverEnter, hover_delay=100 → hover_pending_ms = Some(0.0)
    - `hover_exit_cancels_pending` — BarHoverEnter (pending), then BarHoverExit → hover_pending_ms = None, expand = Collapsed
    - `tick_triggers_expand_when_delay_elapsed` — set hover_pending_ms = Some(90.0), hover_delay=100, Tick → Expanding(0.0), pending = None
    - `bar_click_noop_when_hover_trigger` — BarClick on hover-trigger app → expand stays Collapsed

- [x] Task 5: Quality gates (AC: #7, #8)
  - [x] 5.1: `cargo build` — succeeds
  - [x] 5.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 5.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 5.4: `cargo test` — 17 new tests + all 123 pre-existing pass (140 total)

## Dev Notes

### What Exists After Story 3.2

The animation state machine is fully in place. Do NOT touch:
- `ExpandState` variants (Collapsed, Expanding, Collapsing, Expanded) — story 3.3 only adds a trigger mechanism; the enum itself is unchanged
- `toggle()`, `is_animating()`, `shows_expanded_content()`, `current_width()`, `label_opacity()` — all correct and tested
- `Message::Tick` animation logic — only the hover delay countdown block is added at the top
- `subscription()` condition — extend with `|| self.hover_pending_ms.is_some()`, not replace

### Hover = Mouse Area, Not Wayland Input Region

Story 3.3 uses iced's `mouse_area` widget enter/exit events — NOT Wayland input region APIs. The outer `mouse_area` that already wraps the entire bar content for `BarClick` (added in Story 3.1) is extended with `.on_enter()`/`.on_exit()`. No Wayland-level input region changes are needed at this scope.

Iced's `mouse_area`:
- `.on_enter(msg)` — fires when pointer first enters the widget bounds (transition from outside to inside)
- `.on_exit(msg)` — fires when pointer leaves the widget bounds (transition from inside to outside)
- The outer `mouse_area` spans the entire bar surface, so these events correctly fire on bar surface entry/exit

### ExpandTrigger Placement

Put `ExpandTrigger` in `src/config/mod.rs` immediately after the `Edge` enum (follows the project's pattern: related enums live in config/mod.rs near the config struct that uses them). Follow the exact `Edge::from_str()` pattern for `ExpandTrigger::from_str()`:
- Error message prefix: `"bar.expand-trigger: ..."` (matches FR37 error format)
- Contains "Expected:" in the message (tested in Story 1.4 validation tests)
- Struct derives: `Debug, Clone, Default, PartialEq`

### Trigger Mutual Exclusivity (AC #6)

When `expand_trigger = Hover`, clicking the bar MUST NOT toggle expand state. The `BarClick` handler guards with `if self.config.bar.expand_trigger == ExpandTrigger::Hover { return Task::none(); }`. This is necessary because:
- The outer `mouse_area` always fires `BarClick` on press — there's no way to suppress it at widget level
- Without the guard, a click-while-hovering would double-toggle the state: expand (hover) + collapse (click) = net no-op but with a visual glitch

### Hover Delay Implementation Pattern

`hover_pending_ms: Option<f32>` on `App`:
- `None` — no hover delay countdown in progress
- `Some(elapsed)` — pointer is inside bar, counting up toward `config.bar.hover_delay`

The Tick handler processes hover delay BEFORE animation, with an early return on both countdown-still-running and delay-triggered cases. This prevents hover delay from conflicting with animation ticks.

When the delay expires, set `self.expand = ExpandState::Expanding(0.0)` directly (not via `toggle()`) to avoid double-toggling if state changed. Then return the initial resize task. The next Tick will naturally pick up animation progress since `is_animating()` is now true.

### Mid-Animation Hover Events

- **BarHoverExit during Expanding**: `toggle()` → `Collapsing(1.0 - p)`. Works correctly via existing reversal logic.
- **BarHoverEnter during Collapsing**: `toggle()` → `Expanding(1.0 - p)`. Bar reverses back toward expanded.
- **BarHoverEnter when Expanded**: no-op (bar is already where it should be).
- **BarHoverEnter when Expanding**: no-op (already going the right direction).
- **BarHoverExit when Collapsed**: no-op (bar is already where it should be).
- **BarHoverExit when Collapsing**: no-op (already going the right direction).

### Dead Code on `expanded_width`

Story 3.2's `current_width()` already uses `config.bar.expanded_width`, so the `#[allow(dead_code)]` that existed in Story 3.1 was removed in Story 3.2. Story 3.3 continues using both widths — no dead code concern.

### `make_hover_config()` Test Helper Pattern

Add alongside the existing `make_config()` test helper in `src/app.rs`:
```rust
#[cfg(test)]
fn make_hover_config() -> Config {
    let mut config = make_config();
    config.bar.expand_trigger = ExpandTrigger::Hover;
    config.bar.hover_delay = 0;
    config
}
```
Use `make_hover_config()` for hover-trigger tests; keep `make_config()` (expand_trigger=Click) for click-trigger tests.

### `bar_hovered` Field

`bar_hovered: bool` tracks whether the pointer is currently inside the bar. It is set in both `BarHoverEnter` and `BarHoverExit` handlers. It is used only to keep a consistent view of pointer-in-bar state — not directly used in any logic besides being set. Future stories (e.g., Story 6.x keyboard navigation) may use this field to decide behavior. Setting it in both handlers is correct; it ensures the field stays accurate even if events arrive out of expected order.

### Default Config Update

`config/default.kdl` should gain `expand-trigger "click"` inside the `bar` block. This makes the default explicit and documents the option for users who read the default config:
```kdl
bar {
    edge "left"
    collapsed-width 48
    expanded-width 200
    expand-trigger "click"
}
```
Do NOT add `hover-delay` to the default config file — it is only meaningful for hover trigger and clutters the defaults with a no-op value.

### No Changes to `src/surface/expand.rs`

Story 3.3 adds no new methods or variants to `ExpandState`. All hover trigger logic lives in `src/app.rs` update handlers. Do not add hover-awareness to `ExpandState` itself — the expand state machine describes geometry and animation state, not input trigger semantics.

### State After Story 3.3

When complete:
- `config.bar.expand-trigger = "hover"` enables hover-based expand/collapse
- `config.bar.hover-delay = N` adds a dwell delay before expand begins
- Click trigger and hover trigger are mutually exclusive
- Mid-animation hover events correctly reverse via existing toggle() reversal logic
- Subscription emits Tick during hover delay countdown (same channel as animation ticks)
- Story 3.4 will add configurable animation durations and width parameters

### References

- Epic 3 (epics.md §"Story 3.3"): user story + acceptance criteria
- UX Design: §"Journey 2: Expand/Collapse" — hover dwell opt-in, mutual exclusivity with keybind
- Architecture: §"Expand/Collapse State Machine" — existing state machine (unchanged)
- Architecture: §"State Management Patterns" — AR6 (SurfaceState owns expand)
- Story 3.1 dev notes: outer mouse_area pattern, BarClick handler, #[to_layer_message] catch-all
- Story 3.2 dev notes: Tick handler structure, subscription conditional, ExpandState helpers
- Story 3.2 completion notes: label_opacity deferred, #[allow(dead_code)] pattern for unused methods
- FR36: hover trigger as opt-in (epics.md)

### File Locations

| What | Where |
|---|---|
| `ExpandTrigger` enum | `src/config/mod.rs` — after `Edge` enum |
| `ExpandTrigger::from_str()` | `src/config/mod.rs` — in impl block |
| `BarConfig.expand_trigger`, `.hover_delay` | `src/config/mod.rs` — extend `BarConfig` struct and `Default` |
| Parse `expand-trigger`, `hover-delay` | `src/config/parse.rs` — in `parse_bar_section()` |
| `config/default.kdl` | Add `expand-trigger "click"` to bar block |
| `Message::BarHoverEnter`, `BarHoverExit` | `src/app.rs` — in `Message` enum |
| `App.bar_hovered`, `App.hover_pending_ms` | `src/app.rs` — in `App` struct |
| Handlers, subscription, Tick update | `src/app.rs` — in `update()` and `subscription()` |
| `view()` outer mouse_area | `src/app.rs` — add `.on_enter()` / `.on_exit()` |
| Tests | `src/config/parse.rs` and `src/app.rs` |

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

None — implementation completed without errors. No clippy warnings; all 140 tests pass first run.

### Completion Notes List

- Added `ExpandTrigger { Click, Hover }` enum to `src/config/mod.rs` (after `Edge` enum) with `from_str()` following `Edge::from_str()` pattern (error prefix "bar.expand-trigger:", contains "Expected:").
- Added `expand_trigger: ExpandTrigger` and `hover_delay: u64` fields to `BarConfig`; updated `Default` impl (Click, 0ms).
- Removed stale `#[allow(dead_code)]` comment on `expanded_width` — field is actively used via `current_width()`.
- Updated `parse_bar_section()` in `src/config/parse.rs` to parse `expand-trigger` (string → `ExpandTrigger::from_str`) and `hover-delay` (integer → u64) using the same `get_arg` / fallback-to-default pattern as existing fields.
- Added `expand-trigger "click"` to `config/default.kdl` bar block to make the default explicit.
- Added `use crate::config::ExpandTrigger;` import to `src/app.rs`.
- Added `bar_hovered: bool` and `hover_pending_ms: Option<f32>` fields to `App`; initialized to `false` / `None`.
- Added `Message::BarHoverEnter` and `Message::BarHoverExit` variants to `Message` enum.
- `BarClick` handler: added early return `Task::none()` when `expand_trigger == Hover` (click is the sole trigger for click mode; hover is sole trigger for hover mode).
- `BarHoverEnter` handler: sets `bar_hovered = true`; if trigger ≠ Hover, returns immediately; if Collapsed with delay=0, calls `toggle()` → Expanding(0.0) + resize task; if Collapsed with delay>0, sets `hover_pending_ms = Some(0.0)`; if Collapsing, calls `toggle()` to reverse; otherwise no-op.
- `BarHoverExit` handler: sets `bar_hovered = false`; if trigger ≠ Hover, returns immediately; clears `hover_pending_ms`; if Expanded or Expanding, calls `toggle()` → Collapsing + resize task; otherwise no-op.
- `subscription()`: condition changed from `expand.is_animating()` to `expand.is_animating() || hover_pending_ms.is_some()` — Tick also fires during hover countdown.
- `Tick` handler: added hover delay block at top (before animation match); if `hover_pending_ms` is Some, advances elapsed by TICK_MS; if new_elapsed >= hover_delay, sets expand = Expanding(0.0), clears pending, returns resize task; otherwise updates elapsed and returns early.
- `view()`: outer `mouse_area` extended with `.on_enter(Message::BarHoverEnter).on_exit(Message::BarHoverExit)`.
- Tests: 6 new in `parse.rs` (expand-trigger, hover-delay parse/default/error), 11 new in `app.rs` (hover trigger behavior, pending delay, tick countdown, bar_click no-op). 17 new total → 140 total tests, all pass.
- All quality gates: `cargo build` ✓, `cargo clippy -- -D warnings` ✓ (zero warnings), `cargo fmt --check` ✓, `cargo test` ✓ (140/140).

### File List

- src/config/mod.rs
- src/config/parse.rs
- config/default.kdl
- src/app.rs
- _bmad-output/implementation-artifacts/3-3-hover-triggered-expand-collapse.md
- _bmad-output/implementation-artifacts/sprint-status.yaml

## Change Log

| Date | Summary |
|---|---|
| 2026-04-03 | Story context created |
| 2026-04-03 | Implemented Story 3.3: ExpandTrigger enum, hover_delay config, BarHoverEnter/Exit messages and handlers, hover_pending_ms countdown, subscription extended, Tick handler extended, view() updated; 17 new tests; 140 total; all quality gates pass |
