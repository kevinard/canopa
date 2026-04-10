# Story 7.5: All Bar Params Per-Output Override

Status: review

## Story

As a user with monitors that serve different roles,
I want to override any bar parameter per output (edge, expand, expand-trigger, hover-delay, expand-keybind, as well as the existing size and modules),
So that each screen's bar can be independently configured without limits.

## Acceptance Criteria

1. **Given** an `output` block in config with `edge "top"`
   **When** niribar starts
   **Then** the bar on that output anchors to the top edge; bars on other outputs use the global edge

2. **Given** an `output` block with `expand #false`
   **When** the user interacts with that output's bar (click, hover)
   **Then** expand/collapse is disabled on that output only; other outputs behave normally

3. **Given** an `output` block with `expand-trigger "hover"` and/or `hover-delay 300`
   **When** the cursor enters that output's bar
   **Then** expand uses the per-output trigger and delay; other outputs use global settings

4. **Given** an `output` block with `expand-keybind "Ctrl+b"`
   **When** niribar starts
   **Then** the field is parsed and stored; in-bar keyboard shortcut uses the per-output keybind only if the per-output keybind is the first non-None keybind resolved for that surface (see Dev Notes for keybind limitation)

5. **Given** any new per-output field is set
   **When** `effective_bar_for()` is called for that output
   **Then** the per-output value is returned; absent fields inherit from global

6. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

7. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 351 pre-existing tests pass and at least 13 new tests pass (≥ 364 total)

## KDL Syntax

All remaining bar fields are now valid in any `output` block:

```kdl
bar {
    edge "left"
    collapsed-width 48
    expanded-width 200
    expand #true
    expand-trigger "click"
    hover-delay 0
    expand-keybind "Super+e"
    modules {
        top "clock"
        bottom "battery"
    }
}

output "DP-2" {
    edge "top"              // anchor to top instead of left
    collapsed-width 32
    expanded-width 0        // not applicable for horizontal bar (height only)
    expand #false           // always collapsed
    expand-trigger "hover"
    hover-delay 200
    expand-keybind "Ctrl+b"
    modules {
        top "clock"
        center "workspaces"
    }
}
```

Fields now available per output (in addition to `collapsed-width`, `expanded-width`, `modules`):

- `edge "left"|"right"|"top"|"bottom"` — which screen edge the bar anchors to
- `expand #true|#false` — whether expand/collapse is active
- `expand-trigger "click"|"hover"` — what triggers expand
- `hover-delay <ms>` — dwell time for hover trigger (u64)
- `expand-keybind "<keybind>"` — in-bar keyboard shortcut (string, same format as global)

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `src/config.rs` | **MODIFY** | Add 5 new `Option` fields to `OutputConfig`; update `effective_bar_for()` |
| `src/config/parse.rs` | **MODIFY** | Parse 5 new fields in `parse_output_sections()`; update unknown-key error message |
| `src/main.rs` | **MODIFY** | Extend `has_per_output_overrides` to include all 5 new fields |
| `src/app.rs` | **MODIFY** | Fix `spawn_output_window()` anchor; update `update()` handlers to use per-output expand/trigger/delay |

## Tasks / Subtasks

- [x] Task 1: Extend `OutputConfig` and update `effective_bar_for()` in `src/config.rs` (AC: #1–5)

  - [x] 1.1: Add 5 new optional fields to `OutputConfig`:

    ```rust
    #[derive(Debug, Clone)]
    pub struct OutputConfig {
        pub name: String,
        pub enabled: bool,
        pub collapsed_width: Option<u32>,
        pub expanded_width: Option<u32>,
        pub modules: Option<BarModules>,
        // New in Story 7.5:
        /// Override for bar.edge. `None` → inherit from global.
        pub edge: Option<Edge>,
        /// Override for bar.expand. `None` → inherit from global.
        pub expand: Option<bool>,
        /// Override for bar.expand-trigger. `None` → inherit from global.
        pub expand_trigger: Option<ExpandTrigger>,
        /// Override for bar.hover-delay. `None` → inherit from global.
        pub hover_delay: Option<u64>,
        /// Override for bar.expand-keybind. `None` → inherit from global.
        pub expand_keybind: Option<String>,
    }
    ```

  - [x] 1.2: Update `effective_bar_for()` to merge the 5 new fields:

    Replace the existing method body:

    ```rust
    pub fn effective_bar_for(&self, output_name: &str) -> BarConfig {
        let global = &self.bar;
        let Some(oc) = self.outputs.iter().find(|o| o.name == output_name) else {
            return global.clone();
        };
        BarConfig {
            collapsed_width: oc.collapsed_width.unwrap_or(global.collapsed_width),
            expanded_width: oc.expanded_width.unwrap_or(global.expanded_width),
            modules: oc.modules.clone().unwrap_or_else(|| global.modules.clone()),
            edge: oc.edge.clone().unwrap_or_else(|| global.edge.clone()),
            expand: oc.expand.unwrap_or(global.expand),
            expand_trigger: oc.expand_trigger.unwrap_or(global.expand_trigger),
            hover_delay: oc.hover_delay.unwrap_or(global.hover_delay),
            expand_keybind: oc.expand_keybind.clone().or_else(|| global.expand_keybind.clone()),
        }
    }
    ```

    Note: `expand_keybind` uses `.or_else()` because both sides are `Option<String>` — `oc.expand_keybind` is `Option<String>` (None = inherit, Some = override). `or_else` returns `oc.expand_keybind` if `Some`, otherwise falls back to `global.expand_keybind`.

- [x] Task 2: Extend `parse_output_sections()` in `src/config/parse.rs` (AC: #1–5)

  - [x] 2.1: Add 5 new local variables in `parse_output_sections` function body:

    ```rust
    let mut edge: Option<Edge> = None;
    let mut expand: Option<bool> = None;
    let mut expand_trigger: Option<ExpandTrigger> = None;
    let mut hover_delay: Option<u64> = None;
    let mut expand_keybind: Option<String> = None;
    ```

  - [x] 2.2: Add 5 new match arms before the `other =>` catch-all:

    ```rust
    "edge" => {
        let s = child
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "output {name:?}: edge: expected a string value."
                )
            })?;
        edge = Some(Edge::from_str(s)?);
    }
    "expand" => {
        let v = child
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_bool())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "output {name:?}: expand: expected a boolean value (#true or #false)."
                )
            })?;
        expand = Some(v);
    }
    "expand-trigger" => {
        let s = child
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "output {name:?}: expand-trigger: expected a string value."
                )
            })?;
        expand_trigger = Some(ExpandTrigger::from_str(s)?);
    }
    "hover-delay" => {
        let v = child
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_integer())
            .and_then(|v| u64::try_from(v).ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "output {name:?}: hover-delay: expected a non-negative integer."
                )
            })?;
        hover_delay = Some(v);
    }
    "expand-keybind" => {
        let s = child
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "output {name:?}: expand-keybind: expected a string value."
                )
            })?
            .to_string();
        expand_keybind = Some(s);
    }
    ```

  - [x] 2.3: Update the `other =>` error message to list all valid keys:

    ```rust
    other => {
        anyhow::bail!(
            "output {name:?}: unknown key {other:?}. \
             Expected: one of: enabled, edge, collapsed-width, expanded-width, \
             expand, expand-trigger, hover-delay, expand-keybind, modules."
        );
    }
    ```

  - [x] 2.4: Include new fields in the `result.push(OutputConfig { ... })` at the end:

    ```rust
    result.push(OutputConfig {
        name,
        enabled,
        collapsed_width,
        expanded_width,
        modules,
        edge,
        expand,
        expand_trigger,
        hover_delay,
        expand_keybind,
    });
    ```

- [x] Task 3: Update `has_per_output_overrides` in `src/main.rs` (AC: #1–4)

  Replace the existing `has_per_output_overrides` check at line 84:

  ```rust
  let has_per_output_overrides = config
      .outputs
      .iter()
      .any(|o| o.collapsed_width.is_some() || o.expanded_width.is_some() || o.modules.is_some());
  ```

  With:

  ```rust
  let has_per_output_overrides = config.outputs.iter().any(|o| {
      o.collapsed_width.is_some()
          || o.expanded_width.is_some()
          || o.modules.is_some()
          || o.edge.is_some()
          || o.expand.is_some()
          || o.expand_trigger.is_some()
          || o.hover_delay.is_some()
          || o.expand_keybind.is_some()
  });
  ```

- [x] Task 4: Fix `spawn_output_window()` and update `update()` handlers in `src/app.rs` (AC: #1–3)

  - [x] 4.1: Fix anchor computation in `spawn_output_window()` — currently uses global edge, must use per-output effective edge.

    Current code (lines 265–266):

    ```rust
    let anchor = config.bar.edge.anchor();
    let eff = config.effective_bar_for(name);
    ```

    Replace with (swap order so `eff` is computed first):

    ```rust
    let eff = config.effective_bar_for(name);
    let anchor = eff.edge.anchor();
    ```

    The `eff_size` computation on line 269 already uses `eff.edge.is_horizontal()` — this was correct but was inconsistent with the anchor using the global edge. After this fix, both anchor and size use the per-output effective edge.

  - [x] 4.2: Update `Message::BarClick(window)` handler to use per-output expand config:

    Replace:

    ```rust
    Message::BarClick(window) => {
        if !self.config.bar.expand {
            return Task::none();
        }
        if self.config.bar.expand_trigger == ExpandTrigger::Hover {
            return Task::none();
        }
        let s = self.surface_mut(window);
        s.expand = s.expand.toggle();
        self.overlay_size_change_for(window)
    }
    ```

    With:

    ```rust
    Message::BarClick(window) => {
        let output_name = self.surface(window).map(|s| s.output_name.clone()).unwrap_or_default();
        let bar = self.config.effective_bar_for(&output_name);
        if !bar.expand {
            return Task::none();
        }
        if bar.expand_trigger == ExpandTrigger::Hover {
            return Task::none();
        }
        let s = self.surface_mut(window);
        s.expand = s.expand.toggle();
        self.overlay_size_change_for(window)
    }
    ```

  - [x] 4.3: Update `Message::BarHoverEnter(window)` handler:

    Replace:

    ```rust
    Message::BarHoverEnter(window) => {
        let expand_enabled = self.config.bar.expand;
        let expand_trigger = self.config.bar.expand_trigger;
        let hover_delay = self.config.bar.hover_delay;
        let s = self.surface_mut(window);
    ```

    With:

    ```rust
    Message::BarHoverEnter(window) => {
        let output_name = self.surface(window).map(|s| s.output_name.clone()).unwrap_or_default();
        let bar = self.config.effective_bar_for(&output_name);
        let expand_enabled = bar.expand;
        let expand_trigger = bar.expand_trigger;
        let hover_delay = bar.hover_delay;
        let s = self.surface_mut(window);
    ```

  - [x] 4.4: Update `Message::BarHoverExit(window)` handler:

    Replace:

    ```rust
    Message::BarHoverExit(window) => {
        let expand_enabled = self.config.bar.expand;
        let expand_trigger = self.config.bar.expand_trigger;
        let s = self.surface_mut(window);
    ```

    With:

    ```rust
    Message::BarHoverExit(window) => {
        let output_name = self.surface(window).map(|s| s.output_name.clone()).unwrap_or_default();
        let bar = self.config.effective_bar_for(&output_name);
        let expand_enabled = bar.expand;
        let expand_trigger = bar.expand_trigger;
        let s = self.surface_mut(window);
    ```

  - [x] 4.5: Update hover tick handler to use per-output `hover_delay`.

    In `Message::Tick`, the loop over `window_ids` currently gets `s = self.surfaces.get_mut(&wid)` first, then reads `self.config.bar.hover_delay`. To avoid a borrow conflict with the new pattern, get the output name and effective config BEFORE the mutable borrow:

    Replace the loop header:

    ```rust
    for wid in window_ids {
        let s = self.surfaces.get_mut(&wid).unwrap();

        // Hover delay countdown — runs while waiting to start expand (Story 3.3).
        if let Some(elapsed) = s.hover_pending_ms {
            let new_elapsed = elapsed + TICK_MS;
            if new_elapsed >= self.config.bar.hover_delay as f32 {
    ```

    With:

    ```rust
    for wid in window_ids {
        // Resolve per-output hover_delay before taking a mutable borrow of the surface.
        let output_name = self.surfaces.get(&wid).map(|s| s.output_name.clone()).unwrap_or_default();
        let effective_hover_delay = self.config.effective_bar_for(&output_name).hover_delay;
        let s = self.surfaces.get_mut(&wid).unwrap();

        // Hover delay countdown — runs while waiting to start expand (Story 3.3).
        if let Some(elapsed) = s.hover_pending_ms {
            let new_elapsed = elapsed + TICK_MS;
            if new_elapsed >= effective_hover_delay as f32 {
    ```

- [x] Task 5: Write tests in `src/config.rs` and `src/config/parse.rs` (AC: #7)

  - [x] 5.1: Add tests for `effective_bar_for()` with new fields in `src/config.rs` tests block:

    ```rust
    #[test]
    fn effective_bar_for_applies_edge_override() {
        let mut config = Config::default();
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: Some(Edge::Top),
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None,
        }];
        let bar = config.effective_bar_for("DP-2");
        assert!(matches!(bar.edge, Edge::Top));
        // other fields inherit global defaults
        assert_eq!(bar.collapsed_width, config.bar.collapsed_width);
    }

    #[test]
    fn effective_bar_for_applies_expand_override() {
        let mut config = Config::default();
        config.bar.expand = true; // global: expand on
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: None,
            expand: Some(false), // per-output: expand off
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None,
        }];
        let bar = config.effective_bar_for("DP-2");
        assert!(!bar.expand);
        // global config unchanged
        let global_bar = config.effective_bar_for("unknown");
        assert!(global_bar.expand);
    }

    #[test]
    fn effective_bar_for_applies_expand_trigger_override() {
        let mut config = Config::default();
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: Some(ExpandTrigger::Hover),
            hover_delay: None,
            expand_keybind: None,
        }];
        let bar = config.effective_bar_for("DP-2");
        assert_eq!(bar.expand_trigger, ExpandTrigger::Hover);
    }

    #[test]
    fn effective_bar_for_applies_hover_delay_override() {
        let mut config = Config::default();
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: Some(300),
            expand_keybind: None,
        }];
        let bar = config.effective_bar_for("DP-2");
        assert_eq!(bar.hover_delay, 300);
    }

    #[test]
    fn effective_bar_for_applies_expand_keybind_override() {
        let mut config = Config::default();
        config.bar.expand_keybind = Some("Super+e".into());
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: Some("Ctrl+b".into()),
        }];
        let bar = config.effective_bar_for("DP-2");
        assert_eq!(bar.expand_keybind, Some("Ctrl+b".into()));
        // global still has Super+e
        let global_bar = config.effective_bar_for("other");
        assert_eq!(global_bar.expand_keybind, Some("Super+e".into()));
    }

    #[test]
    fn effective_bar_for_inherits_expand_keybind_when_not_overridden() {
        let mut config = Config::default();
        config.bar.expand_keybind = Some("Super+e".into());
        config.outputs = vec![OutputConfig {
            name: "DP-2".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: None,
            edge: None,
            expand: None,
            expand_trigger: None,
            hover_delay: None,
            expand_keybind: None, // not overridden → inherit
        }];
        let bar = config.effective_bar_for("DP-2");
        assert_eq!(bar.expand_keybind, Some("Super+e".into()));
    }
    ```

  - [x] 5.2: Add parsing tests in `src/config/parse.rs` tests block:

    ```rust
    #[test]
    fn output_edge_override_parses() {
        let config = parse(r#"output "DP-2" { edge "top" }"#).unwrap();
        assert!(matches!(config.outputs[0].edge, Some(Edge::Top)));
    }

    #[test]
    fn output_edge_invalid_errors() {
        let result = parse(r#"output "DP-2" { edge "diagonal" }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("edge"), "got: {msg}");
    }

    #[test]
    fn output_expand_override_parses() {
        let config = parse(r#"output "DP-2" { expand #false }"#).unwrap();
        assert_eq!(config.outputs[0].expand, Some(false));
    }

    #[test]
    fn output_expand_trigger_override_parses() {
        let config = parse(r#"output "DP-2" { expand-trigger "hover" }"#).unwrap();
        assert_eq!(config.outputs[0].expand_trigger, Some(ExpandTrigger::Hover));
    }

    #[test]
    fn output_hover_delay_override_parses() {
        let config = parse(r#"output "DP-2" { hover-delay 300 }"#).unwrap();
        assert_eq!(config.outputs[0].hover_delay, Some(300));
    }

    #[test]
    fn output_expand_keybind_override_parses() {
        let config = parse(r#"output "DP-2" { expand-keybind "Ctrl+b" }"#).unwrap();
        assert_eq!(config.outputs[0].expand_keybind, Some("Ctrl+b".into()));
    }

    #[test]
    fn output_new_fields_default_to_none_when_absent() {
        let config = parse(r#"output "DP-2" { enabled #true }"#).unwrap();
        let oc = &config.outputs[0];
        assert!(oc.edge.is_none());
        assert!(oc.expand.is_none());
        assert!(oc.expand_trigger.is_none());
        assert!(oc.hover_delay.is_none());
        assert!(oc.expand_keybind.is_none());
    }

    #[test]
    fn output_unknown_key_error_lists_all_valid_keys() {
        let result = parse(r#"output "DP-1" { zoom 2 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("edge"), "error should mention 'edge': {msg}");
        assert!(msg.contains("expand"), "error should mention 'expand': {msg}");
    }
    ```

## Dev Notes

### `Edge` and `ExpandTrigger` already have `from_str()` methods

`Edge::from_str(s: &str)` is already defined in `src/config.rs` (line ~235). `ExpandTrigger::from_str(s: &str)` is also defined (line ~280). Both are `pub(crate)`. Reuse them directly in the `parse_output_sections` match arms — do NOT re-implement parsing logic.

### `spawn_output_window()` anchor inconsistency (pre-existing)

Before this story, `spawn_output_window()` computed:

- `anchor` from `config.bar.edge.anchor()` (global)  
- `eff_size` using `eff.edge.is_horizontal()` (per-output)

These were equivalent since `edge` wasn't overridable. Task 4.1 fixes this by computing `eff` first, then using `eff.edge.anchor()` for both.

### Borrow checker pattern for `update()` handlers

The pattern used in Tasks 4.2–4.4 (get `output_name` first via immutable `surface()`, then drop the borrow, then get mutable borrow via `surface_mut()`) is safe because:

1. `self.surface(window)` borrows `self.surfaces` immutably; `clone()` copies the string; borrow released
2. `self.config.effective_bar_for(...)` borrows `self.config` immutably; released after binding
3. `self.surface_mut(window)` borrows `self.surfaces` mutably; no conflict

### Hover tick pattern (Task 4.5)

The tick handler iterates `window_ids` (a copied vec) and currently calls `self.surfaces.get_mut(&wid)` before reading `self.config`. The new pattern adds a `self.surfaces.get(&wid)` (immutable) call first to extract `output_name`, followed by `self.config.effective_bar_for(...)`, then the existing `get_mut` call. The borrow checker allows this because: immutable borrow ends before mutable borrow begins.

### `ToggleExpand` (IPC path) — keep global

`Message::ToggleExpand` is the IPC path (`niribar --toggle-expand`) that toggles ALL surfaces. It currently checks `if !self.config.bar.expand` (global). Do NOT change this to per-output — the IPC toggle is intentionally global. Users can use per-output `expand #false` to make a surface ignore toggle events (since `ToggleExpand` fires `FocusedToggleExpand` logic — verify this). Actually `ToggleExpand` calls internal logic; confirm at implementation time whether it respects `expand #false` per surface or not, and adjust accordingly.

### `expand_keybind` per-output: data model only

`init_expand_keybind` in `boot()` is a global `OnceLock` — initialized once from `config.bar.expand_keybind`. The per-output `expand_keybind` field IS stored and returned by `effective_bar_for()`, but the keyboard subscription uses the global keybind only.

**Behavior:** The per-output keybind overrides the value returned by `effective_bar_for()` (useful for config introspection and future refactoring), but does NOT change the actual keybind that fires `Message::ToggleFocusedExpand`. This is a known limitation and documented — do NOT attempt to refactor the OnceLock keyboard system in this story.

### `Edge` derives in `OutputConfig`

`Edge` already derives `Debug, Clone, Default`. It does NOT derive `Copy`. `expand_trigger: ExpandTrigger` derives `Copy`, so `.unwrap_or(global.expand_trigger)` works by copy. `edge: Edge` requires `.clone()` in `effective_bar_for()`, which is already done via `oc.edge.clone().unwrap_or_else(|| global.edge.clone())`.

### No changes to `validate()` for the 5 new fields

None of the 5 new per-output fields require new validation rules:

- `edge`: any value is valid (from_str already validates)
- `expand`: bool, always valid
- `expand_trigger`: from_str validates
- `hover_delay`: u64, always valid
- `expand_keybind`: string, always valid at parse time (runtime validity left to `init_expand_keybind`)

### `OutputConfig` struct literal sites

Any code that constructs `OutputConfig { ... }` (in tests) must be updated to include the 5 new fields. All pre-existing `OutputConfig` constructor calls in tests will need the new fields added with `None` values. Search for `OutputConfig {` in `src/config.rs` and `src/app.rs` tests.

### Test baseline: 351 → ≥ 364

Story 7.4 ended with 351 tests. Task 5 adds 6 config.rs tests + 9 parse.rs tests = 15 new tests → target ≥ 366.

### `pub(crate)` visibility for `from_str`

`Edge::from_str` and `ExpandTrigger::from_str` are `pub(crate)`. `parse.rs` is in the same crate (`src/config/parse.rs`), so they're accessible without any visibility changes.

## Previous Story Intelligence (Story 7.4)

- `OutputConfig` currently has: `name`, `enabled`, `collapsed_width: Option<u32>`, `expanded_width: Option<u32>`, `modules: Option<BarModules>`
- `effective_bar_for()` currently hardcodes `edge/expand/expand_trigger/hover_delay/expand_keybind` from global — this is the core change
- `parse_output_sections()` currently rejects `edge`, `expand`, etc. as unknown keys → update the error message to list all valid keys
- `spawn_output_window()` in `app.rs` (line ~265): `let anchor = config.bar.edge.anchor()` uses GLOBAL edge; must switch to per-output `eff.edge.anchor()` — compute `eff` BEFORE `anchor`
- `overlay_size_change_for()` ALREADY uses `effective_bar_for()` (updated in Story 7.4) — no change needed
- `update()` handlers (`BarClick`, `BarHoverEnter`, `BarHoverExit`, Tick) use `self.config.bar.*` for expand fields — need per-output pattern using `output_name` clone

## Definition of Done

- [x] `OutputConfig` has 5 new `Option` fields: `edge`, `expand`, `expand_trigger`, `hover_delay`, `expand_keybind`
- [x] `effective_bar_for()` merges all 5 new fields; absent fields inherit from global
- [x] `parse_output_sections()` parses all 5 new KDL keys; unknown-key error lists all valid keys
- [x] `has_per_output_overrides` in `main.rs` checks all 5 new fields
- [x] `spawn_output_window()` uses `eff.edge.anchor()` (not global edge)
- [x] `BarClick`, `BarHoverEnter`, `BarHoverExit` handlers use per-output `expand`/`expand_trigger`/`hover_delay`
- [x] Tick handler uses per-output `hover_delay`
- [x] All pre-existing `OutputConfig { ... }` test literals updated with new `None` fields
- [x] `cargo clippy -- -D warnings` clean
- [x] `cargo test` shows ≥ 364 tests passing

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

- Extended `OutputConfig` with 5 new `Option` fields: `edge`, `expand`, `expand_trigger`, `hover_delay`, `expand_keybind`
- Updated `effective_bar_for()` to merge all 5 new fields using per-output override or global fallback; `expand_keybind` uses `.or_else()` since both sides are `Option<String>`
- Extended `parse_output_sections()` with 5 new match arms and updated the unknown-key error message to list all valid keys including the new ones
- Updated `has_per_output_overrides` in `main.rs` to check all 5 new fields so manual output mode activates when any new field is set
- Fixed `spawn_output_window()` anchor: compute `eff` first, then use `eff.edge.anchor()` so per-output edge override is respected at surface creation
- Updated `BarClick`, `BarHoverEnter`, `BarHoverExit` handlers to resolve `output_name` + `effective_bar_for()` before mutable borrow (borrow-checker safe pattern)
- Updated Tick handler hover delay loop to resolve `effective_hover_delay` from immutable borrow before taking mutable borrow of surface
- Updated all pre-existing `OutputConfig { ... }` test literals (in `config.rs` and `app.rs`) to include 5 new `None` fields
- Added 6 new `effective_bar_for` tests in `config.rs` and 9 new parse tests in `parse.rs`
- `cargo clippy -- -D warnings`: zero warnings; `cargo test`: 377 tests pass (≥ 364 target)

### File List

- `src/config.rs`
- `src/config/parse.rs`
- `src/main.rs`
- `src/app.rs`

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-04-08 | 1.0 | Created story: extend per-output overrides to all bar params (edge, expand, expand-trigger, hover-delay, expand-keybind) | claude-sonnet-4-6 |
| 2026-04-10 | 1.1 | Implemented: 5 new OutputConfig fields, effective_bar_for() merge, parse support, main.rs guard, app.rs anchor/handler fixes, 15 new tests (377 total) | claude-sonnet-4-6 |
