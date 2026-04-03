# Story 3.1: Bar Collapses and Expands on Explicit Trigger

Status: done

## Story

As a **user**,
I want to click the bar to toggle it between its narrow and wide states,
so that I can see full module labels when I need them and reclaim screen space when I don't.

## Acceptance Criteria

1. **Given** the bar is in collapsed state (48px wide)
   **When** the user clicks anywhere on the bar surface
   **Then** the bar expands to its configured expanded width (200px default)
   **And** the iced_layershell surface is resized to `config.bar.expanded_width`

2. **Given** the bar is in expanded state
   **When** the user clicks anywhere on the bar surface
   **Then** the bar collapses to the configured collapsed width (48px default)
   **And** the iced_layershell surface is resized back to `config.bar.collapsed_width`

3. **Given** the bar is toggled
   **When** the state changes
   **Then** the same action always inverts the current state (toggle model — UX-DR13)

4. **Given** the bar is in collapsed state
   **When** rendering
   **Then** each module's `view_collapsed()` is called

5. **Given** the bar is in expanded state
   **When** rendering
   **Then** each module's `view_expanded()` is called

6. **Given** the expand/collapse state
   **When** stored
   **Then** it is stored in `SurfaceState` (not `SharedState`) — architecture rule AR6

7. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

8. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 79 pre-existing tests pass and at least 8 new tests pass

## Tasks / Subtasks

- [x] Task 1: Create `src/surface/expand.rs` (new file) — `ExpandState` (AC: #3, #6)
  - [x] 1.1: Define `ExpandState` enum with two variants for Story 3.1 (Story 3.2 adds `Animating`):
    ```rust
    /// Expand/collapse state for a single bar surface.
    /// Animation logic (Animating variant, f32 progress) is added in Story 3.2.
    /// This enum is stored in SurfaceState, never in SharedState (AR6).
    #[derive(Debug, Clone, PartialEq, Default)]
    pub enum ExpandState {
        #[default]
        Collapsed,
        Expanded,
        // Animating(f32) — added in Story 3.2; story 3.1 transitions are instant
    }

    impl ExpandState {
        /// Toggle between Collapsed and Expanded.
        /// Story 3.2 will refine this to emit Animating(...) instead of instant transition.
        pub fn toggle(self) -> ExpandState {
            match self {
                ExpandState::Collapsed => ExpandState::Expanded,
                ExpandState::Expanded => ExpandState::Collapsed,
            }
        }

        pub fn is_expanded(&self) -> bool {
            matches!(self, ExpandState::Expanded)
        }
    }
    ```
  - [x] 1.2: Add `pub mod expand;` to `src/surface/mod.rs`

- [x] Task 2: Update `SurfaceState` in `src/surface/mod.rs` (AC: #6)
  - [x] 2.1: Replace the stub `SurfaceState` (which had `// expand: ExpandState — added in Story 3.1` comment):
    ```rust
    use crate::surface::expand::ExpandState;

    /// Per-output render state. Tracks surface-specific data (expand/collapse, focus).
    /// Shared module data lives in SharedState (established in Story 2.1), never here.
    /// Multi-surface support (HashMap<SurfaceId, SurfaceState>) deferred to Story 7.1.
    pub struct SurfaceState {
        pub output_name: String,
        pub expand: ExpandState,
        // focused_module: Option<ModuleId> — added in Story 6.1
    }

    impl SurfaceState {
        pub fn new(output_name: String) -> Self {
            SurfaceState {
                output_name,
                expand: ExpandState::default(), // starts Collapsed
            }
        }
    }
    ```
  - [x] 2.2: Remove or update `#[allow(dead_code)]` now that fields are used

- [x] Task 3: Update `src/app.rs` — expand state, new message, update(), view() (AC: #1, #2, #3, #4, #5)
  - [x] 3.1: Add `expand: ExpandState` field to `App` struct:
    ```rust
    pub struct App {
        pub config: Config,
        pub shared: SharedState,
        pub modules: Vec<(Zone, Box<dyn Module>)>,
        pub hovered_module: Option<ModuleId>,
        /// Current expand state. Single-surface for Stories 3.1–6.x;
        /// moves to SurfaceState per-output in Story 7.1.
        pub expand: ExpandState,
    }
    ```
    Update `App::new()` to initialize: `expand: ExpandState::default()`

  - [x] 3.2: Add `use crate::surface::expand::ExpandState;` import to `src/app.rs`

  - [x] 3.3: Add `BarClick` variant to the `Message` enum:
    ```rust
    #[to_layer_message]
    #[derive(Debug, Clone)]
    pub enum Message {
        Module(ModuleId, ModuleEvent),
        ModuleHoverEnter(ModuleId),
        ModuleHoverExit(ModuleId),
        ModuleClick(ModuleId),
        BarClick,   // ← new: fires when user clicks bar surface (Story 3.1 toggle trigger)
    }
    ```

  - [x] 3.4: Handle `Message::BarClick` in `update()` — toggle state AND resize surface.

    **CRITICAL — Dynamic Surface Resize:**
    iced_layershell 0.17.1 (with iced 0.14) requires returning a `Task<Message>` from `update()` to resize the layer-shell surface dynamically. The exact API must be confirmed:

    **Option A — iced `window::resize` (most likely for iced 0.14):**
    ```rust
    use iced::window;
    // in update():
    Message::BarClick => {
        self.expand = self.expand.clone().toggle();
        let new_width = if self.expand.is_expanded() {
            self.config.bar.expanded_width
        } else {
            self.config.bar.collapsed_width
        };
        window::resize(window::Id::MAIN, iced::Size::new(new_width as f32, 0.0))
    }
    ```

    **Option B — iced_layershell-specific resize command:**
    If `window::resize` does not work with layer shells (layer shell surfaces are not standard iced windows), look at `iced_layershell` for a resize API. Check:
    - `iced_layershell::commands` module for a `resize` or `set_size` function
    - The `#[to_layer_message]` macro may inject a resize variant; check crate docs/examples
    - Reference: bar-rs (github.com/faervan/bar-rs) for how it handles surface resizing

    Regardless of which API: the Task returned from `update()` must change both the surface width
    AND the exclusive zone (the desktop space reserved by the bar). The exclusive zone should
    match the surface width so the compositor allocates the correct amount of screen real estate.

    Add `Message::BarClick` arm BEFORE the `_ => Task::none()` catch-all.

  - [x] 3.5: Update `view()` to use `self.expand`:
    - When `ExpandState::Collapsed`: use `collapsed_width - 1` as content width, call `view_collapsed()`
    - When `ExpandState::Expanded`: use `expanded_width - 1` as content width, call `view_expanded()`
    - Wrap the entire bar view in a `mouse_area` with `.on_press(Message::BarClick)`:
    ```rust
    pub fn view(&self) -> Element<'_, Message> {
        let content_width = if self.expand.is_expanded() {
            self.config.bar.expanded_width.saturating_sub(1)
        } else {
            self.config.bar.collapsed_width.saturating_sub(1)
        };

        // ... build top/center/bottom zones ...
        for (zone, module) in &self.modules {
            let state = ...;
            let id = module.id();
            let hovered = self.hovered_module.as_ref() == Some(&id);
            let module_view = if self.expand.is_expanded() {
                module.view_expanded(&state)
            } else {
                module.view_collapsed(&state)
            };
            let view: Element<'_, Message> =
                mouse_area(module_frame(module_view, hovered))
                    .on_enter(Message::ModuleHoverEnter(id.clone()))
                    .on_exit(Message::ModuleHoverExit(id.clone()))
                    .on_press(Message::ModuleClick(id))
                    .into();
            // ... push to zone ...
        }

        let bar_content = row![
            container(zone_layout(top, center, bottom))
                .width(content_width)
                .height(Length::Fill),
            // 1px right border
            container(Space::new())
                .width(1)
                .height(Length::Fill)
                .style(|_theme| container::Style {
                    background: Some(iced::Background::Color(Color::from_rgb8(0x45, 0x47, 0x5a))),
                    ..Default::default()
                }),
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        // Wrap entire bar in mouse_area for click-to-toggle (Story 3.1 AC #1/#2)
        mouse_area(bar_content)
            .on_press(Message::BarClick)
            .into()
    }
    ```

    **Note on click propagation:** The outer `mouse_area(bar_content).on_press(Message::BarClick)`
    will fire when ANY part of the bar is clicked, including module areas. Module mouse areas
    (inner) also fire `Message::ModuleClick(id)`. This means:
    - Clicking a module → BOTH `ModuleClick` AND `BarClick` fire (module interaction + bar toggle)
    - Clicking bar background → only `BarClick` fires
    This dual-fire behavior is acceptable for Story 3.1; Story 3.3 will refine trigger behavior.

- [x] Task 4: Tests (AC: #7, #8)
  - [x] 4.1: In `src/surface/expand.rs` tests:
    - `expand_state_default_is_collapsed` — `ExpandState::default() == ExpandState::Collapsed`
    - `expand_state_toggle_collapsed_to_expanded` — `ExpandState::Collapsed.toggle() == ExpandState::Expanded`
    - `expand_state_toggle_expanded_to_collapsed` — `ExpandState::Expanded.toggle() == ExpandState::Collapsed`
    - `expand_state_is_expanded_true` — `ExpandState::Expanded.is_expanded() == true`
    - `expand_state_is_expanded_false` — `ExpandState::Collapsed.is_expanded() == false`
  - [x] 4.2: In `src/surface/mod.rs` tests:
    - `surface_state_new_starts_collapsed` — `SurfaceState::new("HDMI-1".into()).expand == ExpandState::Collapsed`
    - `surface_state_expand_field_is_set` — `SurfaceState { output_name: "DP-1".into(), expand: ExpandState::Expanded }.expand.is_expanded()`
  - [x] 4.3: In `src/app.rs` tests:
    - `app_starts_collapsed` — `App::new(Config::default()).expand == ExpandState::Collapsed`
    - `bar_click_toggles_collapsed_to_expanded` — `update(Message::BarClick)` when `expand == Collapsed` → `app.expand == Expanded`
    - `bar_click_toggles_expanded_to_collapsed` — second `update(Message::BarClick)` → `app.expand == Collapsed`
  - [x] 4.4: Verify all 79 pre-existing tests still pass (`cargo test`)

- [x] Task 5: Quality gates (AC: #7, #8)
  - [x] 5.1: `cargo build` — succeeds
  - [x] 5.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 5.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 5.4: `cargo test` — 14 new tests + all 79 pre-existing pass (93 total)

## Dev Notes

### ExpandState Is Deliberately Minimal for Story 3.1

The architecture defines: `Collapsed → Animating(Expanding) → Expanded` with an `f32` progress
value advanced by `Message::Tick`. Story 3.1 delivers INSTANT toggle only. Do NOT add:
- `Animating` variant (Story 3.2)
- `Message::Tick` handling (Story 3.2)
- `easing` or `duration` fields (Story 3.2)

The `toggle()` method goes directly `Collapsed ↔ Expanded`. Story 3.2 will intercept the toggle
to insert `Animating` instead, and Story 3.3 adds hover-based triggering.

### CRITICAL: Dynamic Surface Resize (iced_layershell 0.17.1)

This story requires the iced_layershell surface to resize when the expand state toggles.
The initial size is set in `main.rs` via `LayerShellSettings { size: Some((collapsed_width, 0)) }`.

**What to look for in iced_layershell 0.17.1:**
1. `iced_layershell::commands` module — check for `resize(width, height)` or `set_size`
2. The `#[to_layer_message]` macro injects LayerShell action variants — inspect if it adds
   a size-change variant (e.g., something like `Message::Size(u32, u32)`)
3. For iced 0.14 compatibility: `iced::window::resize(window::Id::MAIN, Size)` may work
4. **Reference implementation:** bar-rs (github.com/faervan/bar-rs) uses the same iced_layershell
   stack and implements expand/collapse — study how it resizes surfaces

**Exclusive zone must also be updated:**
When the bar resizes, `exclusive_zone` (the Wayland reserved desktop space) must match the
new width. Look for `iced_layershell` API to update `exclusive_zone` dynamically.
If there is no API for this in 0.17.1, file a note in deferred-work.md and leave exclusive
zone at `collapsed_width` for now (bar will render at correct visual width regardless).

**Deferred: `expanded_width` validation:**
`validate()` in `src/config/mod.rs` does NOT check `expanded_width >= collapsed_width`.
This was noted as deferred in Story 1.4 review. Add it in this story:
```rust
if config.bar.expanded_width < config.bar.collapsed_width {
    anyhow::bail!(
        "bar.expanded-width: value {} must be >= bar.collapsed-width ({}). \
         Expected: a width greater than or equal to the collapsed width.",
        config.bar.expanded_width, config.bar.collapsed_width
    );
}
```
This is safe to add now as Story 3.1 defines expand/collapse semantics.

### State Location: App vs SurfaceState (Architecture Story Arc)

The architecture defines `SurfaceState` per output in `HashMap<SurfaceId, SurfaceState>`.
Story 3.1 defers multi-surface support to Epic 7, so `expand: ExpandState` lives directly
on `App` as a top-level field. **Do NOT add `surfaces: HashMap<SurfaceId, SurfaceState>` yet** —
that's Story 7.1 territory and would require routing messages by surface ID.

`SurfaceState.expand` is used for the `SurfaceState::new()` constructor so the type is
ready for Story 7.1 to wire in.

### `view_expanded()` Was Already Implemented (Deferred from Story 2.5)

From the deferred work notes:
> **`view_expanded` not called from `app.view()`** — `src/app.rs`: `view_expanded` is fully
> implemented but unreachable; `app.view()` only calls `view_collapsed`. Expand/collapse wiring
> is Story 3.1.

All modules (clock, script, button) already implement both `view_collapsed()` and
`view_expanded()`. Story 3.1 just needs to call the right one based on `self.expand`.

Module view method signatures (established in Stories 2.3–2.5):
```rust
fn view_collapsed(&self, state: &ModuleState) -> Element<'_, Message>
fn view_expanded(&self, state: &ModuleState) -> Element<'_, Message>
```

Both take `&self` and `state: &ModuleState`. The `state` is fetched from
`self.shared.modules.get(&module.id()).cloned().unwrap_or_default()` — same as now.

### `#[to_layer_message]` Macro Caveat

The `#[to_layer_message]` macro on `Message` injects LayerShell variants. Keep the
`_ => Task::none()` catch-all in `app.update()`. New `BarClick` arm must be added BEFORE
the catch-all — same pattern as all previous stories.

### `main.rs` — No Changes Required

`main.rs` sets `size: Some((collapsed_width, 0))` as initial size. This starts the bar
collapsed. Story 3.1's `Message::BarClick` handler in `update()` resizes dynamically.
`exclusive_zone` in `main.rs` stays as `collapsed_width` at startup; it updates at runtime
via the resize Task (or is deferred per the note above).

### Module Click vs Bar Click Interaction

`Message::ModuleClick(id)` fires from the module-level `mouse_area.on_press(...)`.
`Message::BarClick` fires from the outer `mouse_area(bar_content).on_press(...)`.

When a module is clicked, BOTH messages fire (inner then outer mouse_area). This means:
- Button click → spawns command (`on_interaction(Clicked)`) AND toggles bar width
- Clock click → bar toggles (clock has no `on_interaction` override — returns `None`)

This is expected behavior for Story 3.1. Future UX refinement (e.g., separate bar-background
click from module click) deferred until Story 3.3 or later.

### No Changes to `src/modules/` or `src/config/`

Story 3.1 is purely surface-layer wiring:
- `src/surface/expand.rs` — new
- `src/surface/mod.rs` — extend existing
- `src/app.rs` — extend existing

Do NOT touch:
- Any `src/modules/` files — all module views are already implemented
- `src/config/` except the `validate()` addition noted above
- `src/niri/` — Stories 4.x
- `src/dbus/` — Stories 5.x

### File Locations

| What | Where |
|---|---|
| `ExpandState` enum | `src/surface/expand.rs` — **new file** |
| `SurfaceState.expand` field | `src/surface/mod.rs` — extend existing |
| `App.expand`, `Message::BarClick`, `BarClick` handler, view update | `src/app.rs` |
| `expanded_width` validation | `src/config/mod.rs` — extend `validate()` |

### State After Story 3.1

When complete:
- Clicking anywhere on the bar toggles between 48px and 200px width
- `ExpandState::Collapsed` → `view_collapsed()` called for each module
- `ExpandState::Expanded` → `view_expanded()` called for each module
- `SurfaceState` has `expand: ExpandState` field (ready for Story 7.1)
- Architecture expand state machine stubbed: Collapsed ↔ Expanded (Story 3.2 adds animation)

### References

- Epic 3, Story 3.1 (epics.md §"Story 3.1"): user story + acceptance criteria
- Architecture: §"Expand/Collapse State Machine" — `Collapsed → Animating → Expanded`, `f32` progress
- Architecture: §"State Management Patterns" — `SurfaceState` owns expand (AR6)
- Architecture: §"FR Categories" — FR6–10 → `src/surface/expand.rs`
- UX Design: UX-DR1 (animation curves — Story 3.2 not 3.1), UX-DR10 (48px / 200px defaults), UX-DR13 (toggle model)
- Architecture: §"Project Structure" — `src/surface/expand.rs` already planned
- Story 2.5 dev notes: `#[to_layer_message]` catch-all pattern, `view_expanded` deferral note
- Story 1.4 deferred work: `expanded_width` validation deferred to Story 3.1

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

None — implementation completed without errors. One clippy fix needed: `SurfaceState` and `SurfaceState::new` are not yet wired into the binary (that's Story 7.1), so `#[allow(dead_code)]` was applied to the struct and the `new()` method — same pattern used in earlier stories for types waiting on future wiring.

### Completion Notes List

- Created `src/surface/expand.rs` (new file): `ExpandState { Collapsed, Expanded }` with `toggle()` (Collapsed ↔ Expanded, Story 3.2 adds Animating) and `is_expanded()`. Derives `Debug, Clone, PartialEq, Default`.
- Updated `src/surface/mod.rs`: added `pub mod expand;`, replaced `SurfaceState` stub with `expand: ExpandState` field and `SurfaceState::new(output_name)` constructor. `#[allow(dead_code)]` retained on struct and `new()` since `SurfaceState` is not yet used in the binary (wired in Story 7.1).
- Updated `src/app.rs`: added `expand: ExpandState` field to `App` (initialized to `ExpandState::Collapsed`); added `Message::BarClick`; added `BarClick` handler in `update()` — toggles expand state, then returns `Task::batch([Message::SizeChange((w, 0)), Message::ExclusiveZoneChange(w as i32)])` via `#[to_layer_message]`-injected variants; updated `view()` to call `view_expanded()` or `view_collapsed()` based on `self.expand`, use correct content width, and wrap bar in `mouse_area(...).on_press(Message::BarClick)`.
- Updated `src/config/mod.rs`: added `expanded_width >= collapsed_width` validation to `validate()` (deferred from Story 1.4 review). Added 2 tests for this constraint.
- Resize API discovery: `#[to_layer_message]` macro (non-multi mode) injects `SizeChange((u32, u32))` and `ExclusiveZoneChange(i32)` variants directly into `Message`. Pattern: `Task::done(Message::SizeChange((width, 0)))`.
- 14 new tests across 4 files: 5 in `surface/expand.rs`, 3 in `surface/mod.rs`, 3 in `app.rs` (Story 3.1 AC tests), 2 in `config/mod.rs` (expanded_width validation). 93 total tests pass; zero clippy warnings; `cargo fmt --check` passes.

### File List

- src/surface/expand.rs (new)
- src/surface/mod.rs
- src/app.rs
- src/config/mod.rs
- _bmad-output/implementation-artifacts/3-1-bar-collapses-and-expands-on-explicit-trigger.md
- _bmad-output/implementation-artifacts/sprint-status.yaml

### Review Findings

- [x] [Review][Patch] Unchecked u32→i32 cast in ExclusiveZoneChange [src/app.rs]
- [x] [Review][Patch] ExpandState missing Copy derive — unnecessary clone at toggle call site [src/surface/expand.rs]
- [x] [Review][Defer] Task::batch dispatch ordering for SizeChange/ExclusiveZoneChange is non-guaranteed [src/app.rs] — deferred, pre-existing
- [x] [Review][Defer] Height=0 magic value in SizeChange undocumented at call site [src/app.rs] — deferred, pre-existing
- [x] [Review][Defer] Equal-width toggle fires redundant resize when expanded_width == collapsed_width [src/app.rs] — deferred, pre-existing
- [x] [Review][Defer] BarClick handler tests assert expand state only, not Task return value [src/app.rs] — deferred, pre-existing
- [x] [Review][Defer] ModuleClick + BarClick double-fire: every module click also toggles bar expand/collapse [src/app.rs] — deferred, spec-documented; refine in Story 3.3
- [x] [Review][Defer] SurfaceState.expand / App.expand divergence risk at Story 7.1 migration [src/surface/mod.rs] — deferred; SurfaceState.expand is a scaffold stuck at Collapsed; Story 7.1 must not adopt it blindly

## Change Log

| Date | Summary |
|---|---|
| 2026-04-02 | Story context created |
| 2026-04-02 | Implemented Story 3.1: ExpandState (expand.rs new file), SurfaceState.expand, App.expand + Message::BarClick, SizeChange/ExclusiveZoneChange resize via to_layer_message, view_expanded wiring; expanded_width validation; 14 new tests; 93 total; all quality gates pass |
