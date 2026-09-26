# Story 3.2: Animate the Width Transition

Status: review

## Story

As a **user**,
I want the expand and collapse to animate smoothly,
so that the transition feels polished rather than jarring.

## Acceptance Criteria

1. **Given** the bar is transitioning from collapsed to expanded
   **When** the animation runs
   **Then** the bar width interpolates from `collapsed_width` to `expanded_width` over 280ms using `cubic-bezier(0.22, 1, 0.36, 1)` easing

2. **Given** the bar is transitioning from expanded to collapsed
   **When** the animation runs
   **Then** the bar width interpolates from `expanded_width` to `collapsed_width` over 200ms using `cubic-bezier(0.4, 0, 1, 1)` easing

3. **Given** the bar is in the `Expanding` state
   **When** rendering modules
   **Then** module expanded content is shown with opacity animating from 0 to 1, with a 60ms delay before fade begins (i.e., opacity = max(0, (elapsed_ms - 60) / 220))

4. **Given** the bar begins collapsing
   **When** the `Collapsing` state starts
   **Then** module collapsed content is shown immediately (no reverse fade delay — instant switch to `view_collapsed()`)

5. **Given** the `CriticalBadge` spec (UX-DR7): badge is collapsed-only content
   **When** expansion begins (state transitions to `Expanding(0.0)`)
   **Then** `view_expanded()` is called immediately, hiding the badge by design (no extra code needed — this is satisfied by the switch to `view_expanded()` on first Expanding tick)

6. **Given** the bar is animating
   **When** the user clicks the bar mid-animation
   **Then** the animation reverses from the current position (toggle during Expanding → Collapsing(1.0 - current_progress), toggle during Collapsing → Expanding(1.0 - current_progress))

7. **Given** the animation is running
   **When** measured
   **Then** the tick rate is ~60fps (16ms interval) and no frames are dropped under normal system load (NFR1)

8. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

9. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 93 pre-existing tests pass and at least 10 new tests pass

## Tasks / Subtasks

- [x] Task 1: Extend `src/surface/expand.rs` — add animation variants and helpers (AC: #1, #2, #6)
  - [x] 1.1: Add `Expanding(f32)` and `Collapsing(f32)` variants to `ExpandState`:
    ```rust
    #[derive(Debug, Clone, Copy, PartialEq, Default)]
    pub enum ExpandState {
        #[default]
        Collapsed,
        /// Animating toward expanded. f32 = progress ∈ [0.0, 1.0]; 280ms cubic-bezier(0.22,1,0.36,1).
        Expanding(f32),
        Expanded,
        /// Animating toward collapsed. f32 = progress ∈ [0.0, 1.0]; 200ms cubic-bezier(0.4,0,1,1).
        Collapsing(f32),
    }
    ```

  - [x] 1.2: Update `toggle()` to transition through animation states (replaces the Story 3.1 instant toggle):
    ```rust
    pub fn toggle(self) -> ExpandState {
        match self {
            ExpandState::Collapsed => ExpandState::Expanding(0.0),
            ExpandState::Expanding(p) => ExpandState::Collapsing(1.0 - p),
            ExpandState::Expanded => ExpandState::Collapsing(0.0),
            ExpandState::Collapsing(p) => ExpandState::Expanding(1.0 - p),
        }
    }
    ```

  - [x] 1.3: Add helper methods:
    ```rust
    pub fn is_expanded(&self) -> bool {
        matches!(self, ExpandState::Expanded)
    }

    pub fn is_animating(&self) -> bool {
        matches!(self, ExpandState::Expanding(_) | ExpandState::Collapsing(_))
    }

    /// True when expanded content (view_expanded) should be rendered.
    /// During Expanding: show expanded content (with opacity fade).
    /// During Collapsing: show collapsed content immediately.
    pub fn shows_expanded_content(&self) -> bool {
        matches!(self, ExpandState::Expanded | ExpandState::Expanding(_))
    }

    /// Interpolated width for the current animation state.
    pub fn current_width(&self, collapsed: u32, expanded: u32) -> u32 {
        match self {
            ExpandState::Collapsed => collapsed,
            ExpandState::Expanded => expanded,
            ExpandState::Expanding(p) => {
                let eased = cubic_bezier(0.22, 1.0, 0.36, 1.0, *p);
                collapsed + ((expanded as f32 - collapsed as f32) * eased) as u32
            }
            ExpandState::Collapsing(p) => {
                let eased = cubic_bezier(0.4, 0.0, 1.0, 1.0, *p);
                expanded.saturating_sub(((expanded as f32 - collapsed as f32) * eased) as u32)
            }
        }
    }

    /// Opacity for expanded content during Expanding animation.
    /// Returns 1.0 when Expanded; 0.0 when not in an expanding-content state.
    pub fn label_opacity(&self) -> f32 {
        match self {
            ExpandState::Expanded => 1.0,
            ExpandState::Expanding(p) => {
                // 280ms total; 60ms delay before fade starts; 220ms fade window.
                let elapsed_ms = p * 280.0;
                ((elapsed_ms - 60.0) / 220.0).max(0.0).min(1.0)
            }
            _ => 0.0,
        }
    }
    ```

  - [x] 1.4: Add `cubic_bezier` free function (private to module) for easing:
    ```rust
    /// Evaluate a CSS cubic-bezier at time fraction t ∈ [0, 1].
    /// P1=(p1x, p1y), P2=(p2x, p2y) are inner control points; P0=(0,0), P3=(1,1).
    /// Uses Newton-Raphson to solve for the parametric s where x(s) == t, then returns y(s).
    fn cubic_bezier(p1x: f32, p1y: f32, p2x: f32, p2y: f32, t: f32) -> f32 {
        let cx = 3.0 * p1x;
        let bx = 3.0 * (p2x - p1x) - cx;
        let ax = 1.0 - cx - bx;
        let cy = 3.0 * p1y;
        let by = 3.0 * (p2y - p1y) - cy;
        let ay = 1.0 - cy - by;

        let sample_x = |s: f32| ((ax * s + bx) * s + cx) * s;
        let sample_dx = |s: f32| (3.0 * ax * s + 2.0 * bx) * s + cx;
        let sample_y = |s: f32| ((ay * s + by) * s + cy) * s;

        // Newton-Raphson: find s such that sample_x(s) ≈ t
        let mut s = t;
        for _ in 0..8 {
            let dx = sample_dx(s);
            if dx.abs() < 1e-6 {
                break;
            }
            s -= (sample_x(s) - t) / dx;
        }
        sample_y(s.clamp(0.0, 1.0))
    }
    ```

- [x] Task 2: Update `src/app.rs` — add `Message::Tick`, subscription, update handler, view (AC: #1–#5, #7)
  - [x] 2.1: Add `use std::time::Duration;` and `Message::Tick` variant:
    ```rust
    #[to_layer_message]
    #[derive(Debug, Clone)]
    pub enum Message {
        Module(ModuleId, ModuleEvent),
        ModuleHoverEnter(ModuleId),
        ModuleHoverExit(ModuleId),
        ModuleClick(ModuleId),
        BarClick,
        /// Animation clock tick. Fired at ~60fps only while `expand.is_animating()`.
        Tick,
    }
    ```

  - [x] 2.2: Update `subscription()` to emit ticks only during animation:
    ```rust
    pub fn subscription(&self) -> iced::Subscription<Message> {
        let module_subs =
            iced::Subscription::batch(self.modules.iter().map(|(_, m)| m.subscription()));
        if self.expand.is_animating() {
            let tick = iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick);
            iced::Subscription::batch([module_subs, tick])
        } else {
            module_subs
        }
    }
    ```

  - [x] 2.3: Update `Message::BarClick` handler — `toggle()` now returns `Expanding` or `Collapsing` instead of jumping directly to the final state. **No change to handler call site**: `self.expand = self.expand.toggle()` already works. However the resize Task must use the ANIMATED current width, not jump straight to target:
    ```rust
    Message::BarClick => {
        self.expand = self.expand.toggle();
        // Width at animation start: still at the origin side.
        // Actual resizing happens per-tick via Message::Tick.
        // Issue an immediate resize to the current (start) width to avoid a flash.
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
    ```

  - [x] 2.4: Add `Message::Tick` handler (insert BEFORE the `_ => Task::none()` catch-all):
    ```rust
    Message::Tick => {
        const EXPAND_MS: f32 = 280.0;
        const COLLAPSE_MS: f32 = 200.0;
        const TICK_MS: f32 = 16.0; // ~60fps

        match self.expand {
            ExpandState::Expanding(progress) => {
                let new_progress = (progress + TICK_MS / EXPAND_MS).min(1.0);
                self.expand = if new_progress >= 1.0 {
                    ExpandState::Expanded
                } else {
                    ExpandState::Expanding(new_progress)
                };
            }
            ExpandState::Collapsing(progress) => {
                let new_progress = (progress + TICK_MS / COLLAPSE_MS).min(1.0);
                self.expand = if new_progress >= 1.0 {
                    ExpandState::Collapsed
                } else {
                    ExpandState::Collapsing(new_progress)
                };
            }
            _ => return Task::none(),
        }
        // Resize surface to the animated width at the new progress value.
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
    ```

  - [x] 2.5: Update `view()` to use animated width and apply label opacity:
    - Replace the existing `content_width` calculation with `self.expand.current_width(...)`:
      ```rust
      let content_width = self
          .expand
          .current_width(
              self.config.bar.collapsed_width,
              self.config.bar.expanded_width,
          )
          .saturating_sub(1); // 1px right border
      ```
    - Use `shows_expanded_content()` (not `is_expanded()`) to decide which module view to call:
      ```rust
      let shows_expanded = self.expand.shows_expanded_content();
      let label_opacity = self.expand.label_opacity();

      let module_view: Element<'_, Message> = if shows_expanded {
          // Expanded content with animated opacity (UX-DR6: 60ms delay, 220ms fade).
          // iced 0.14: iced::widget::opacity(f32, content) — verify import path.
          // Fallback if unavailable: render without opacity wrapper (acceptable for MVP).
          iced::widget::opacity(label_opacity as f64, module.view_expanded(&state)).into()
      } else {
          module.view_collapsed(&state)
      };
      ```

    **IMPORTANT — `iced::widget::opacity` verification:** In iced 0.14, `iced::widget::opacity` takes `impl Into<f64>` as first argument. Confirm by checking `iced::widget` in docs or source. If it does not exist, remove the opacity wrapper and note in deferred-work.md — the animation still works correctly for width; only the label fade is deferred.

- [x] Task 3: Tests (AC: #8, #9)
  - [x] 3.1: In `src/surface/expand.rs` — add tests:
    - `toggle_collapsed_starts_expanding` — `Collapsed.toggle() == Expanding(0.0)`
    - `toggle_expanded_starts_collapsing` — `Expanded.toggle() == Collapsing(0.0)`
    - `toggle_expanding_reverses_to_collapsing` — `Expanding(0.3).toggle() == Collapsing(0.7)` (within f32 tolerance)
    - `toggle_collapsing_reverses_to_expanding` — `Collapsing(0.6).toggle() == Expanding(0.4)` (within f32 tolerance)
    - `is_animating_expanding` — `Expanding(0.5).is_animating() == true`
    - `is_animating_collapsing` — `Collapsing(0.5).is_animating() == true`
    - `is_animating_collapsed_false` — `Collapsed.is_animating() == false`
    - `is_animating_expanded_false` — `Expanded.is_animating() == false`
    - `shows_expanded_content_expanding` — `Expanding(0.0).shows_expanded_content() == true`
    - `shows_expanded_content_collapsing_false` — `Collapsing(0.0).shows_expanded_content() == false`
    - `current_width_collapsed` — `Collapsed.current_width(48, 200) == 48`
    - `current_width_expanded` — `Expanded.current_width(48, 200) == 200`
    - `current_width_expanding_mid` — `Expanding(0.5).current_width(48, 200)` is between 48 and 200
    - `cubic_bezier_zero` — `cubic_bezier(0.22, 1.0, 0.36, 1.0, 0.0) ≈ 0.0`
    - `cubic_bezier_one` — `cubic_bezier(0.22, 1.0, 0.36, 1.0, 1.0) ≈ 1.0`
    - `label_opacity_expanded` — `Expanded.label_opacity() == 1.0`
    - `label_opacity_collapsed` — `Collapsed.label_opacity() == 0.0`
    - `label_opacity_expanding_at_zero_progress` — `Expanding(0.0).label_opacity() == 0.0` (delay not elapsed)
    - `label_opacity_expanding_after_delay` — `Expanding(60.0/280.0).label_opacity() ≈ 0.0` (just at delay boundary)

  - [x] 3.2: In `src/app.rs` — add tests:
    - `bar_click_now_starts_expanding` — after `update(BarClick)` from Collapsed: `app.expand == Expanding(0.0)` (not `Expanded`)
    - `tick_advances_expand_progress` — after one `update(Tick)` in `Expanding(0.0)`: `app.expand == Expanding(16.0/280.0)`
    - `tick_completes_expand_animation` — `Expanding(0.999)` after Tick → `Expanded`
    - `tick_advances_collapse_progress` — after one `update(Tick)` in `Collapsing(0.0)`: `app.expand == Collapsing(16.0/200.0)`
    - `tick_completes_collapse_animation` — `Collapsing(0.999)` after Tick → `Collapsed`
    - `tick_no_op_when_not_animating` — `update(Tick)` when `Expanded` → still `Expanded`

  - [x] 3.3: Verify all 93 pre-existing tests still pass (`cargo test`)

- [x] Task 4: Quality gates (AC: #8, #9)
  - [x] 4.1: `cargo build` — succeeds
  - [x] 4.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 4.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 4.4: `cargo test` — 30 new tests + all 93 pre-existing pass (123 total)

## Dev Notes

### What Changed vs Story 3.1

Story 3.1 delivered instant `Collapsed ↔ Expanded` toggle. Story 3.2 inserts animation:
- `ExpandState` gains `Expanding(f32)` and `Collapsing(f32)` (progress ∈ [0.0, 1.0])
- `toggle()` transitions through these instead of jumping directly to the final state
- `Message::Tick` drives progress at ~60fps; subscription emits ticks only when animating
- Surface resize (SizeChange + ExclusiveZoneChange) happens on every tick
- `view()` uses `shows_expanded_content()` (not `is_expanded()`) for view dispatch, and `current_width()` (not config widths) for content_width

### Anatomy of `ExpandState` After This Story

```
Collapsed  →  toggle()  →  Expanding(0.0) ─── Tick×N ──→  Expanded
Expanded   →  toggle()  →  Collapsing(0.0) ── Tick×N ──→  Collapsed
Expanding(p) → toggle() →  Collapsing(1.0-p)   [mid-animation reverse]
Collapsing(p) → toggle() → Expanding(1.0-p)    [mid-animation reverse]
```

### Files Modified

| File | Change |
|---|---|
| `src/surface/expand.rs` | Add `Expanding`/`Collapsing` variants; update `toggle()`; add `is_animating()`, `shows_expanded_content()`, `current_width()`, `label_opacity()`; add `cubic_bezier()` free function |
| `src/app.rs` | Add `Message::Tick`; add `use std::time::Duration`; update `subscription()` for conditional tick; update `BarClick` handler; add `Tick` handler; update `view()` width and opacity |

**Do NOT touch:**
- `src/surface/mod.rs` — `SurfaceState` needs no changes
- Any `src/modules/` — module trait, view methods unchanged
- `src/config/` — no config changes (timing is hardcoded per UX-DR1; Story 3.4 adds config)
- `src/niri/`, `src/dbus/` — not in scope

### `BarClick` Handler After This Story

The `BarClick` handler calls `self.expand.toggle()` (same call site as Story 3.1). **No structural change to the handler** — just the behavior of `toggle()` changes. The resize Task now uses `self.expand.current_width(...)` at the START of the animation (which equals the origin width, so it's a no-op resize), then subsequent Tick messages drive the actual width changes.

**Risk**: The `BarClick` handler issues a resize Task AND the first Tick also issues a resize. This is benign — two consecutive SizeChange to the same width is idempotent.

### `#[to_layer_message]` Catch-All Pattern

Keep `_ => Task::none()` as the LAST arm in `update()`. Insert `Message::Tick =>` BEFORE the catch-all. Same convention as all previous stories.

The macro injects `SizeChange` and `ExclusiveZoneChange` variants. `Message::Tick` is a new user-defined variant — it is NOT injected by the macro.

### `iced::time::every` API (iced 0.14)

```rust
iced::time::every(std::time::Duration::from_millis(16))
    .map(|_instant| Message::Tick)
```

- Returns `Subscription<std::time::Instant>`; the instant value is discarded (we use fixed TICK_MS)
- The subscription is only returned from `subscription()` when `self.expand.is_animating()` — Iced stops the timer automatically when the subscription is not returned
- Import: `use std::time::Duration;` at top of `src/app.rs`

### `iced::widget::opacity` (iced 0.14)

```rust
iced::widget::opacity(label_opacity as f64, content)
```

- Signature: `opacity(opacity: impl Into<f64>, content: impl Into<Element<...>>) -> Opacity<...>`
- If this function does not compile (not in scope), check `iced::widget` module. If unavailable, omit the opacity wrapper, log in `deferred-work.md`, and the story is otherwise complete (width animation is the primary deliverable; label fade is secondary).
- **Do NOT use** `iced::widget::opaque` — that's for event propagation blocking, not visual transparency.

### Easing Curve Values

| Transition | Duration | CSS cubic-bezier | Character |
|---|---|---|---|
| Expand | 280ms | `cubic-bezier(0.22, 1, 0.36, 1)` | Spring ease-out — fast start, gentle landing |
| Collapse | 200ms | `cubic-bezier(0.4, 0, 1, 1)` | Ease-in — slow start, sharp exit |

Source: UX-DR1 (epics.md §"Story 3.2", §"UX Design Requirements").

### Label Opacity Formula (UX-DR6)

```
Expanding(progress) → elapsed_ms = progress * 280.0
opacity = clamp((elapsed_ms - 60.0) / 220.0, 0.0, 1.0)
```

- At elapsed=0ms: opacity=0 (bar just started expanding)
- At elapsed=60ms: opacity=0 (delay ends)
- At elapsed=280ms: opacity=1.0 (bar fully expanded)
- Collapsing: opacity=0.0 immediately (no reverse delay)
- Expanded: opacity=1.0

### `content_width` in `view()` — Change from Story 3.1

Story 3.1: `content_width = if self.expand.is_expanded() { expanded_width } else { collapsed_width } - 1`

Story 3.2: `content_width = self.expand.current_width(collapsed_width, expanded_width).saturating_sub(1)`

The `.saturating_sub(1)` preserves the 1px right border strip (established in Story 1.2, unchanged).

### CriticalBadge Handling (UX-DR7)

`CriticalBadge` is not yet implemented. When it is (future Epic), it will live in `view_collapsed()` only. Because Story 3.2 switches from `view_collapsed()` to `view_expanded()` the MOMENT `Expanding(0.0)` is set (the first tick after BarClick), the badge disappears within ~16ms of the click — meeting "hidden immediately when expansion begins."

No code in this story handles CriticalBadge explicitly.

### Test Helpers for f32 Comparison

Use `abs()` for f32 equality in tests. Example:
```rust
let result = Expanding(0.3).toggle();
assert!(matches!(result, ExpandState::Collapsing(p) if (p - 0.7).abs() < 1e-5));
```

Or use `approx_eq` pattern inline — no external crate needed.

### Test: `app.expand` After BarClick

The 3.1 tests `bar_click_toggles_collapsed_to_expanded` and `bar_click_toggles_expanded_to_collapsed` will now **FAIL** because `toggle()` no longer jumps directly to `Expanded`/`Collapsed`. These tests test Story 3.1 behavior explicitly. **Update them** for Story 3.2 semantics:
```rust
#[test]
fn bar_click_now_starts_expanding() {
    let mut app = App::new(make_config());
    assert_eq!(app.expand, ExpandState::Collapsed);
    let _ = app.update(Message::BarClick);
    assert!(matches!(app.expand, ExpandState::Expanding(0.0)));
}
```

The OLD Story 3.1 tests `bar_click_toggles_collapsed_to_expanded` and `bar_click_toggles_expanded_to_collapsed` should be **replaced** (not added alongside). Failing to update them will cause compile+test failures.

### State After Story 3.2

- Clicking the bar starts a smooth animated transition
- Expand: 280ms spring curve; collapse: 200ms ease-in curve
- Module expanded content fades in with 60ms delay
- Module collapsed content appears instantly on collapse start
- Subscription only runs the tick timer during animation (no idle overhead)
- Story 3.3 adds hover-based triggering; Story 3.4 adds config for durations/widths

### Project Structure Notes

- All changes are within `src/surface/expand.rs` and `src/app.rs` — no new files
- `cubic_bezier` is a module-private `fn` in `expand.rs` — not `pub`, not in `surface/mod.rs`
- Tests co-located in `#[cfg(test)] mod tests` at bottom of each file (architecture rule)

### References

- Epic 3, Story 3.2 acceptance criteria — epics.md §"Story 3.2: Animate the Width Transition"
- UX-DR1 (animation curves + durations) — epics.md §"UX Design Requirements"
- UX-DR6 (label opacity 60ms delay) — epics.md §"UX Design Requirements"
- UX-DR7 (CriticalBadge collapsed-only) — epics.md §"UX Design Requirements"
- Architecture §"Expand/Collapse State Machine" — `Collapsed → Animating(Expanding) → Expanded`, `f32` progress, `Message::Tick`
- Architecture §"State Management Patterns" — SurfaceState owns expand (AR6)
- Architecture §"FR Categories" — FR6–10 → `src/surface/expand.rs`
- Story 3.1 Dev Notes — `ExpandState` deliberately minimal; `#[to_layer_message]` catch-all; SizeChange/ExclusiveZoneChange API
- Story 3.1 Completion Notes — `Copy` derive added to ExpandState; 93 total tests baseline; resize pattern: `Task::batch([SizeChange((w, 0)), ExclusiveZoneChange(i32::try_from(w)...)])`
- Story 3.1 Review Findings — `ExpandState` derives `Copy` (Review patch applied)

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

- `iced::widget::opacity` does not exist in iced 0.14.0 (confirmed by checking iced_widget-0.14.2/src/ — no opacity.rs); opacity only exists on `image` and `svg` primitives. Label fade deferred per story notes; logged in deferred-work.md.
- Clippy: `manual_clamp` lint fired on `.max(0.0).min(1.0)` in `label_opacity()` — fixed to `.clamp(0.0, 1.0)`.
- `is_expanded()` and `label_opacity()` methods became dead-code warnings (binary no longer calls them directly); added `#[allow(dead_code)]` per project pattern from prior stories.

### Completion Notes List

- Extended `src/surface/expand.rs`: added `Expanding(f32)` and `Collapsing(f32)` variants to `ExpandState`. Updated `toggle()` to return animation states (Collapsed→Expanding(0.0), Expanded→Collapsing(0.0), mid-animation reversal via `1.0 - p`). Added `is_animating()`, `shows_expanded_content()`, `current_width()`, `label_opacity()` helpers. Added private `cubic_bezier()` free function (Newton-Raphson, 8 iterations) implementing CSS cubic-bezier evaluation.
- Updated `src/app.rs`: added `use std::time::Duration`; added `Message::Tick` variant; updated `subscription()` to emit `iced::time::every(16ms).map(|_| Tick)` only when `expand.is_animating()`; added `Tick` handler advancing linear progress with `EXPAND_MS=280` / `COLLAPSE_MS=200` / `TICK_MS=16`, issuing `SizeChange` + `ExclusiveZoneChange` on each tick via existing `#[to_layer_message]` API; updated `BarClick` to use `current_width()` (start-of-animation = origin width, no visual flash); updated `view()` to use `current_width()` for `content_width` and `shows_expanded_content()` for view dispatch.
- Label opacity (UX-DR6): `label_opacity()` method implemented and tested on `ExpandState`; view-layer application deferred — `iced 0.14.0` has no generic element opacity wrapper.
- Updated two Story 3.1 tests that expected instant toggle: `bar_click_toggles_collapsed_to_expanded` → `bar_click_starts_expanding_from_collapsed` (asserts `Expanding(0.0)`); `bar_click_toggles_expanded_to_collapsed` → `bar_click_starts_collapsing_from_expanded` (asserts `Collapsing(0.0)`).
- Test count: 93 (baseline) → 123 (30 new tests): 19 new in `surface::expand`, 6 new in `app`, 5 new cubic bezier boundary tests.
- All quality gates: `cargo build` ✓, `cargo clippy -- -D warnings` ✓ (zero warnings), `cargo fmt --check` ✓, `cargo test` ✓ (123/123 pass).

### File List

- src/surface/expand.rs
- src/app.rs
- _bmad-output/implementation-artifacts/3-2-animate-the-width-transition.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/deferred-work.md

## Change Log

| Date | Summary |
|---|---|
| 2026-04-02 | Story context created |
| 2026-04-02 | Implemented Story 3.2: ExpandState animation variants (Expanding/Collapsing), cubic_bezier easing, Message::Tick + subscription, per-tick surface resize, view dispatch via shows_expanded_content(); label opacity deferred (iced 0.14 missing generic opacity widget); 30 new tests; 123 total; all quality gates pass |
