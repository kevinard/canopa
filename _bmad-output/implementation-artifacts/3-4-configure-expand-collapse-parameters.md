# Story 3.4: Configure Expand/Collapse Parameters

Status: review

## Story

As a **user**,
I want to customize the bar's collapse width, expanded width, and animation timing,
so that the bar fits my screen setup and personal preference.

## Acceptance Criteria

1. **Given** the config file specifies `bar { collapsed-width N }` (or omits it)
   **When** niribar starts
   **Then** the bar uses N as its collapsed width (default 48 when omitted)

2. **Given** the config file specifies `bar { expanded-width N }` (or omits it)
   **When** niribar starts
   **Then** the bar uses N as its expanded width (default 200 when omitted)

3. **Given** the config file specifies `bar { animation { expand-ms N } }` (or omits it)
   **When** niribar starts
   **Then** the expand animation uses N milliseconds (default 280 when omitted)

4. **Given** the config file specifies `bar { animation { collapse-ms N } }` (or omits it)
   **When** niribar starts
   **Then** the collapse animation uses N milliseconds (default 200 when omitted)

5. **Given** all expand/collapse config fields are omitted
   **When** niribar starts
   **Then** the bar uses 48px collapsed, 200px expanded, 280ms expand, 200ms collapse

6. **Given** a negative value is provided for `bar.collapsed-width` or `bar.expanded-width`
   **When** niribar starts
   **Then** an error is produced matching the FR37 format: `"bar.<field>: ..."` containing `"Expected:"`

7. **Given** a negative value is provided for `bar.animation.expand-ms` or `bar.animation.collapse-ms`
   **When** niribar starts
   **Then** an error is produced matching the FR37 format: `"bar.animation.<field>: ..."` containing `"Expected:"`

8. **Given** `output "eDP-1" { animation { expand-ms 150 } }` is in config
   **When** niribar starts
   **Then** the bar on eDP-1 uses 150ms expand animation
   **And** the bar on all other outputs uses the global expand duration

9. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

10. **Given** niribar is built
    **When** `cargo test` is run
    **Then** all 385 pre-existing tests pass and at least 14 new tests pass

## Tasks / Subtasks

- [x] Task 1: Add `AnimationConfig` struct and extend `BarConfig` (AC: #3, #4, #5)
  - [x] 1.1: In `src/config.rs`, add `AnimationConfig` struct immediately before `BarConfig`:
    ```rust
    /// Animation timing for expand/collapse transitions.
    #[derive(Debug, Clone, PartialEq)]
    pub struct AnimationConfig {
        /// Duration of the expand animation in milliseconds. Default: 280.0.
        pub expand_ms: f32,
        /// Duration of the collapse animation in milliseconds. Default: 200.0.
        pub collapse_ms: f32,
    }

    impl Default for AnimationConfig {
        fn default() -> Self {
            AnimationConfig {
                expand_ms: 280.0,
                collapse_ms: 200.0,
            }
        }
    }
    ```

  - [x] 1.2: Add `animation: AnimationConfig` field to `BarConfig` in `src/config.rs` (after `expand_keybind`, before `modules`):
    ```rust
    pub struct BarConfig {
        pub edge: Edge,
        pub collapsed_width: u32,
        pub expanded_width: u32,
        pub expand: bool,
        pub expand_trigger: ExpandTrigger,
        pub hover_delay: u64,
        pub expand_keybind: Option<String>,
        /// Animation timing for expand/collapse transitions.
        pub animation: AnimationConfig,
        pub modules: BarModules,
    }
    ```
    Update `Default for BarConfig` to include `animation: AnimationConfig::default()`.

  - [x] 1.3: Update `effective_bar_for()` in `src/config.rs` — add `animation` to the `BarConfig { ... }` constructor, preferring the per-output override when present:
    ```rust
    BarConfig {
        collapsed_width: oc.collapsed_width.unwrap_or(global.collapsed_width),
        expanded_width: oc.expanded_width.unwrap_or(global.expanded_width),
        modules: oc.modules.clone().unwrap_or_else(|| global.modules.clone()),
        edge: oc.edge.clone().unwrap_or_else(|| global.edge.clone()),
        expand: oc.expand.unwrap_or(global.expand),
        expand_trigger: oc.expand_trigger.unwrap_or(global.expand_trigger),
        hover_delay: oc.hover_delay.unwrap_or(global.hover_delay),
        expand_keybind: oc.expand_keybind.clone().or_else(|| global.expand_keybind.clone()),
        animation: oc.animation.clone().unwrap_or_else(|| global.animation.clone()),  // ← new
    }
    ```

  - [x] 1.4: Add `animation: Option<AnimationConfig>` field to `OutputConfig` in `src/config.rs` (after `expand_keybind`, before any closing brace):
    ```rust
    pub struct OutputConfig {
        // ... existing fields ...
        pub expand_keybind: Option<String>,
        /// Per-output animation override. `None` means inherit from global `bar.animation`.
        pub animation: Option<AnimationConfig>,
    }
    ```
    Initialize to `None` in the output parser constructor.

- [x] Task 2: Add validation for `collapsed-width` and `expanded-width` in bar section (AC: #6)
  - [x] 2.1: In `src/config/parse.rs`, replace the silent-fallback width parsing in `parse_bar_section()` with error-producing validation. Current (silent fallback):
    ```rust
    let collapsed_width = children
        .get_arg("collapsed-width")
        .and_then(|v| v.as_integer())
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(defaults.collapsed_width);
    ```
    Replace with:
    ```rust
    let collapsed_width = match children.get_arg("collapsed-width") {
        None => defaults.collapsed_width,
        Some(v) => {
            let n = v.as_integer().ok_or_else(|| anyhow::anyhow!(
                "bar.collapsed-width: expected a positive integer. \
                 Expected: a width in logical pixels, e.g. 48."
            ))?;
            u32::try_from(n).map_err(|_| anyhow::anyhow!(
                "bar.collapsed-width: expected a positive integer. \
                 Expected: a width in logical pixels, e.g. 48."
            ))?
        }
    };
    ```
    Apply the same pattern for `expanded_width`:
    ```rust
    let expanded_width = match children.get_arg("expanded-width") {
        None => defaults.expanded_width,
        Some(v) => {
            let n = v.as_integer().ok_or_else(|| anyhow::anyhow!(
                "bar.expanded-width: expected a positive integer. \
                 Expected: a width in logical pixels, e.g. 200."
            ))?;
            u32::try_from(n).map_err(|_| anyhow::anyhow!(
                "bar.expanded-width: expected a positive integer. \
                 Expected: a width in logical pixels, e.g. 200."
            ))?
        }
    };
    ```

- [x] Task 3: Parse `bar.animation` block in `src/config/parse.rs` (AC: #3, #4, #5, #7, #8)
  - [x] 3.1: Add a shared private function `parse_animation_node()` that takes a KDL node and an error-path prefix. This is called both from the bar section and the output section:
    ```rust
    /// Parse an `animation { expand-ms N collapse-ms N }` KDL node.
    /// `prefix` is used in error messages, e.g. `"bar"` or `r#"output "eDP-1""#`.
    fn parse_animation_node(
        anim_node: &kdl::KdlNode,
        prefix: &str,
    ) -> anyhow::Result<AnimationConfig> {
        let Some(anim_children) = anim_node.children() else {
            return Ok(AnimationConfig::default());
        };
        let defaults = AnimationConfig::default();

        let expand_ms = match anim_children.get_arg("expand-ms") {
            None => defaults.expand_ms,
            Some(v) => {
                let n = v.as_integer().ok_or_else(|| anyhow::anyhow!(
                    "{prefix}.animation.expand-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 280."
                ))?;
                u32::try_from(n).map_err(|_| anyhow::anyhow!(
                    "{prefix}.animation.expand-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 280."
                ))? as f32
            }
        };

        let collapse_ms = match anim_children.get_arg("collapse-ms") {
            None => defaults.collapse_ms,
            Some(v) => {
                let n = v.as_integer().ok_or_else(|| anyhow::anyhow!(
                    "{prefix}.animation.collapse-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 200."
                ))?;
                u32::try_from(n).map_err(|_| anyhow::anyhow!(
                    "{prefix}.animation.collapse-ms: expected a positive integer. \
                     Expected: a duration in milliseconds, e.g. 200."
                ))? as f32
            }
        };

        Ok(AnimationConfig { expand_ms, collapse_ms })
    }
    ```

  - [x] 3.2: In `parse_bar_section()` (after `expand_keybind`, before `parse_bar_modules`), look up the animation node and call `parse_animation_node`:
    ```rust
    let animation = match children.get("animation") {
        None => AnimationConfig::default(),
        Some(node) => parse_animation_node(node, "bar")?,
    };
    ```
    Add `animation` to the `BarConfig { ... }` constructor at the end of `parse_bar_section`.

  - [x] 3.3: In the output section parser's `match child.name().value()` loop, add an `"animation"` arm after `"expand-keybind"`:
    ```rust
    "animation" => {
        animation = Some(parse_animation_node(child, &format!("output {name:?}"))?);
    }
    ```
    Add `let mut animation: Option<AnimationConfig> = None;` to the output parser locals (alongside the existing `let mut collapsed_width`, etc.), and include `animation` in the `OutputConfig { ... }` constructor at the end of output parsing.

  - [x] 3.4: Add `use crate::config::AnimationConfig;` to the imports in `src/config/parse.rs` (alongside the existing `use crate::config::*` or equivalent).

- [x] Task 4: Use configured animation durations in `src/app.rs` Tick handler (AC: #3, #4, #8)
  - [x] 4.1: In `Message::Tick` in `src/app.rs`, replace the two hardcoded constants with per-surface config lookups. The handler already does `effective_bar_for(&output_name)` to get `hover_delay`. Extend the same resolved-bar block to also extract animation durations:

    Current:
    ```rust
    let effective_hover_delay = self.config.effective_bar_for(&output_name).hover_delay;
    let s = self.surfaces.get_mut(&wid).unwrap();
    ```
    Replace with:
    ```rust
    let effective_bar = self.config.effective_bar_for(&output_name);
    let effective_hover_delay = effective_bar.hover_delay;
    let effective_expand_ms = effective_bar.animation.expand_ms;
    let effective_collapse_ms = effective_bar.animation.collapse_ms;
    let s = self.surfaces.get_mut(&wid).unwrap();
    ```

  - [x] 4.2: Remove the two now-unused constants at the top of the Tick arm:
    ```rust
    // DELETE THESE:
    const EXPAND_MS: f32 = 280.0;
    const COLLAPSE_MS: f32 = 200.0;
    ```
    Replace all uses of `EXPAND_MS` → `effective_expand_ms` and `COLLAPSE_MS` → `effective_collapse_ms` in the match arm below.

- [x] Task 5: Tests (AC: #9, #10)
  - [x] 5.1: In `src/config/parse.rs` tests — add:
    - `parse_animation_expand_ms` — `bar { animation { expand-ms 400 } }` → `config.bar.animation.expand_ms == 400.0`
    - `parse_animation_collapse_ms` — `bar { animation { collapse-ms 100 } }` → `config.bar.animation.collapse_ms == 100.0`
    - `parse_animation_defaults_when_omitted` — no animation block → `expand_ms == 280.0`, `collapse_ms == 200.0`
    - `parse_animation_node_no_children_uses_defaults` — `bar { animation }` → defaults (280.0/200.0)
    - `parse_animation_negative_expand_ms_errors` — `bar { animation { expand-ms -1 } }` → `is_err()` and error contains `"bar.animation.expand-ms"`
    - `parse_animation_negative_collapse_ms_errors` — `bar { animation { collapse-ms -1 } }` → error contains `"bar.animation.collapse-ms"`
    - `parse_negative_collapsed_width_errors` — `bar { collapsed-width -5 }` → error contains `"bar.collapsed-width"` and `"Expected:"`
    - `parse_negative_expanded_width_errors` — `bar { expanded-width -10 }` → error contains `"bar.expanded-width"` and `"Expected:"`
    - `parse_output_animation_expand_ms_override` — `output "eDP-1" { animation { expand-ms 150 } }` → `outputs[0].animation == Some(AnimationConfig { expand_ms: 150.0, collapse_ms: 200.0 })`
    - `parse_output_animation_collapse_ms_override` — `output "DP-1" { animation { collapse-ms 80 } }` → `outputs[0].animation.as_ref().unwrap().collapse_ms == 80.0`
    - `parse_output_animation_none_when_omitted` — `output "eDP-1" { }` → `outputs[0].animation == None`
    - `effective_bar_uses_output_animation_override` — build a `Config` with global `animation.expand_ms = 280.0` and an `OutputConfig` with `animation = Some(AnimationConfig { expand_ms: 150.0, collapse_ms: 200.0 })` → `effective_bar_for("eDP-1").animation.expand_ms == 150.0` and other output falls back to 280.0

  - [x] 5.2: In `src/app.rs` tests — add (use the existing `make_config()` helper; set `config.bar.animation.expand_ms`/`collapse_ms` directly):
    - `tick_uses_custom_expand_duration` — set `animation.expand_ms = 100.0`, state `Expanding(0.0)`, Tick → progress equals `16.0 / 100.0`
    - `tick_uses_custom_collapse_duration` — set `animation.collapse_ms = 50.0`, state `Collapsing(0.0)`, Tick → progress equals `16.0 / 50.0`

- [x] Task 6: Quality gates (AC: #9, #10)
  - [x] 6.1: `cargo build` — succeeds
  - [x] 6.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 6.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 6.4: `cargo test` — 14 new tests + all 385 pre-existing pass (399 total)

## Dev Notes

### State After Story 3.3

When story 3.3 was completed:
- `BarConfig` has: `edge`, `collapsed_width`, `expanded_width`, `expand`, `expand_trigger`, `hover_delay`, `expand_keybind`, `modules`
- `collapsed_width` (default 48) and `expanded_width` (default 200) are already parsed from KDL config via `bar { collapsed-width N expanded-width N }`
- Animation durations are **hardcoded** in `src/app.rs` as `const EXPAND_MS: f32 = 280.0` and `const COLLAPSE_MS: f32 = 200.0` inside the `Message::Tick` arm
- 385 tests pass (app.rs: 124, parse.rs: 64, others: 197)

Story 3-4 adds `AnimationConfig` and makes those constants configurable. It does NOT touch the `ExpandState` enum, `toggle()`, `is_animating()`, `current_width()`, or `label_opacity()` — those are correct and tested.

### Key Insight: `collapsed_width` and `expanded_width` Already Parse, But Without Validation

The current `parse_bar_section()` uses `.and_then(|v| u32::try_from(v).ok()).unwrap_or(defaults.xxx)` — this **silently falls back** to the default when a negative value is given. Story 3-4 requires that negative values produce a hard error (per AC #6 and FR37). Change these to return `Err(...)` instead of silently falling back.

The **output section** parser (for `output "XYZ" { collapsed-width N }`) already validates properly with `.ok_or_else(...)` — follow that exact pattern for the bar section.

### `AnimationConfig` Placement

Place `AnimationConfig` in `src/config.rs` immediately before `BarConfig` (follows the project pattern: supporting types defined before the struct that uses them). There is no separate `src/config/animation.rs` — keep it in the same file as `BarConfig`.

### Per-output `AnimationConfig` Override

`OutputConfig` gets `animation: Option<AnimationConfig>`. `effective_bar_for()` uses `oc.animation.clone().unwrap_or_else(|| global.animation.clone())` — same `Option`-fallback pattern used for all other output overrides.

The `parse_animation_node()` helper is shared between the bar section parser and the output section parser. It takes the KDL node directly (not the parent document) and a `prefix: &str` for error messages. This avoids duplicating validation logic:
- Bar section: `parse_animation_node(node, "bar")?` → errors say `"bar.animation.expand-ms: ..."`
- Output section: `parse_animation_node(child, &format!("output {name:?}"))` → errors say `"output \"eDP-1\".animation.expand-ms: ..."`

In the output section parser's `match child.name().value()` loop, add the `"animation"` arm alongside the existing arms (`"enabled"`, `"collapsed-width"`, `"expanded-width"`, `"edge"`, `"expand"`, `"expand-trigger"`, `"hover-delay"`, `"expand-keybind"`, `"modules"`). Also add `animation` to the unknown-key error message list at the bottom of that match.

### Tick Handler Pattern — Per-Surface Resolution

The Tick handler (in `src/app.rs`, `Message::Tick` arm) already resolves `effective_bar_for(&output_name)` per surface for `hover_delay`. Extend this same resolved bar object to also read `animation.expand_ms` and `animation.collapse_ms`. The effective bar lookup:

```rust
let effective_bar = self.config.effective_bar_for(&output_name);
let effective_hover_delay = effective_bar.hover_delay;
let effective_expand_ms = effective_bar.animation.expand_ms;
let effective_collapse_ms = effective_bar.animation.collapse_ms;
```

This call is **before** `self.surfaces.get_mut(&wid)` (which takes a mutable borrow). Do NOT try to do both calls simultaneously — the existing code already handles this ordering correctly.

After this change, `const EXPAND_MS` and `const COLLAPSE_MS` are completely removed. There should be no remaining references to those symbols.

### Error Message Format (FR37)

All error messages in this story must follow FR37: `"<field.path>: <description>. Expected: <expectation>."`:
- `"bar.collapsed-width: expected a positive integer. Expected: a width in logical pixels, e.g. 48."`
- `"bar.expanded-width: expected a positive integer. Expected: a width in logical pixels, e.g. 200."`
- `"bar.animation.expand-ms: expected a positive integer. Expected: a duration in milliseconds, e.g. 280."`
- `"bar.animation.collapse-ms: expected a positive integer. Expected: a duration in milliseconds, e.g. 200."`

Note that tests validate `error.contains("bar.collapsed-width")` and `error.contains("Expected:")` — both substrings must be present in the error message.

### Test Pattern for Existing Tick Tests

The existing Tick tests in `src/app.rs` (e.g., `tick_advances_expand_animation`, `tick_completes_expand_animation`) use `make_config()` which sets `animation: AnimationConfig::default()` (280ms/200ms). These tests will continue to work correctly since the defaults are unchanged.

For the new `tick_uses_custom_expand_duration` test, mutate the config before constructing the App:
```rust
let mut config = make_config();
config.bar.animation.expand_ms = 100.0;
let mut app = App::new(config);
// put surface into Expanding(0.0) state...
// send Tick...
// assert progress == 16.0 / 100.0
```

Follow the exact App construction pattern used by the existing Tick tests — check `tick_advances_expand_animation` as the template.

### `AnimationConfig` must derive `Clone` and `PartialEq`

`BarConfig` derives `Clone` (used by `effective_bar_for`). `AnimationConfig` must also derive `Clone`. The parse tests that assert `outputs[0].animation == Some(AnimationConfig { ... })` require `PartialEq`. The `#[derive]` line: `#[derive(Debug, Clone, PartialEq)]`.

### No `default.kdl` Changes

Do NOT add `animation { ... }` to `config/default.kdl`. The defaults (280ms/200ms) work correctly without an explicit animation block. Adding it would clutter the default config with values users rarely change — consistent with the decision not to add `hover-delay` to defaults in Story 3.3.

### File Locations

| What | Where |
|---|---|
| `AnimationConfig` struct + `Default` impl | `src/config.rs` — before `BarConfig` |
| `BarConfig.animation` field + Default update | `src/config.rs` — in `BarConfig` struct and `Default` |
| `OutputConfig.animation: Option<AnimationConfig>` | `src/config.rs` — in `OutputConfig` struct |
| `effective_bar_for()` animation merge | `src/config.rs` — `Config::effective_bar_for` |
| `parse_animation_node()` shared helper | `src/config/parse.rs` — new private function |
| Width validation (collapsed-width, expanded-width) | `src/config/parse.rs` — `parse_bar_section()` |
| Animation parsing: bar section | `src/config/parse.rs` — `parse_bar_section()` |
| Animation parsing: output section | `src/config/parse.rs` — output match loop, `"animation"` arm |
| `use crate::config::AnimationConfig;` | `src/config/parse.rs` — imports |
| Tick handler: remove constants, add per-surface lookup | `src/app.rs` — `Message::Tick` arm |
| Tests | `src/config/parse.rs` (12 new) and `src/app.rs` (2 new) |

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

None — implementation completed without errors. No clippy warnings; all 399 tests pass first run after fixing a Python script side-effect that accidentally inserted `animation: None,` into struct field declarations (quickly detected by compiler errors, fixed cleanly).

### Completion Notes List

- Added `AnimationConfig { expand_ms: f32, collapse_ms: f32 }` struct to `src/config.rs` (before `BarConfig`) with `#[derive(Debug, Clone, PartialEq)]` and `Default` impl (280.0ms / 200.0ms).
- Added `animation: AnimationConfig` field to `BarConfig`; updated `Default` impl.
- Added `animation: Option<AnimationConfig>` to `OutputConfig` for per-output override; initialized to `None` in parser constructor.
- Updated `effective_bar_for()` to merge animation with `oc.animation.clone().unwrap_or_else(|| global.animation.clone())` — same Option-fallback pattern as all other per-output overrides.
- In `parse_bar_section()`: replaced silent-fallback `collapsed_width` and `expanded_width` parsing with hard errors matching FR37 format ("bar.collapsed-width: ... Expected: ...").
- Added `parse_animation_node(anim_node, prefix)` shared helper function — takes the KDL node directly and a string prefix for error messages; called from bar section with prefix `"bar"` and from output section with `format!("output {name:?}")`.
- Bar section: `match children.get("animation") { None => default, Some(node) => parse_animation_node(node, "bar")? }`.
- Output section: added `"animation"` arm to match loop calling `parse_animation_node(child, &format!("output {name:?}"))`. Updated unknown-key error message to include `"animation"`.
- Added `AnimationConfig` to imports in `src/config/parse.rs`.
- Removed `const EXPAND_MS` and `const COLLAPSE_MS` from `Message::Tick` handler in `src/app.rs`. Extended the existing `effective_bar_for` call (already used for `hover_delay`) to also extract `animation.expand_ms` and `animation.collapse_ms` per surface.
- All 15 existing `OutputConfig` test literals updated with `animation: None`.
- 14 new tests: 12 in `src/config/parse.rs` (animation parsing, width validation, output override, effective_bar), 2 in `src/app.rs` (custom expand/collapse durations in Tick handler).
- Quality gates: `cargo build` ✓, `cargo clippy -- -D warnings` ✓ (zero warnings), `cargo fmt --check` ✓, `cargo test` ✓ (399/399).

### File List

- src/config.rs
- src/config/parse.rs
- src/app.rs
- _bmad-output/implementation-artifacts/3-4-configure-expand-collapse-parameters.md
- _bmad-output/implementation-artifacts/sprint-status.yaml

## Change Log

| Date | Summary |
|---|---|
| 2026-04-12 | Story context created |
| 2026-04-12 | Implemented: AnimationConfig struct, BarConfig/OutputConfig animation fields, effective_bar_for merge, bar section validation, parse_animation_node shared helper, output section animation arm, Tick handler config-driven durations; 14 new tests; 399 total; all quality gates pass |
