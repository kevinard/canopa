# Story 7.4: Per-Output Configuration Overrides

Status: review

## Story

As a **user with screens of different sizes or purposes**,
I want to override the bar configuration for specific outputs,
So that each screen's bar can have different modules, sizes, or positioning.

## Acceptance Criteria

1. **Given** the config specifies an `output "eDP-1"` block with overriding values (e.g., different module list, different collapsed width)
   **When** niribar starts
   **Then** the surface on that output uses the overridden values

2. **And** values not overridden in the output block inherit from the global config

3. **And** outputs without an explicit config block use the global defaults

4. **And** per-output config is validated at startup alongside global config — invalid overrides produce an FR37-format error

5. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

6. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 334 pre-existing tests pass and at least 12 new tests pass (≥ 346 total)

## KDL Syntax (What Users Write)

Per-output overrides are flat children of the existing `output "name" { ... }` block. Only the fields you want to override need to be present:

```kdl
output "eDP-1" {
    enabled #true
    collapsed-width 60
    expanded-width 250
    modules {
        top "clock" "battery"
        center "workspaces"
        bottom "volume"
    }
}

output "DP-1" {
    enabled #false
}

output "HDMI-1" {
    collapsed-width 40
    // no modules override → inherits global bar.modules
}
```

Fields available for per-output override:
- `collapsed-width <int>` — bar width in logical pixels when collapsed
- `expanded-width <int>` — bar width in logical pixels when expanded  
- `modules { top "..." center "..." bottom "..." }` — zone module layout (replaces global)

Fields NOT overridable per-output (global only):
- `edge`, `expand`, `expand-trigger`, `hover-delay`, `expand-keybind`

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `crates/niribar/src/config.rs` | **MODIFY** | Extend `OutputConfig` with optional override fields; add `Config::effective_bar_for()`; update `validate()` |
| `src/config/parse.rs` | **MODIFY** | Extend `parse_output_sections()` to parse new fields; update unknown-key guard |
| `crates/niribar/src/app.rs` | **MODIFY** | Update `view()` to use `effective_bar_for()`; update `boot()` to use per-output `collapsed_width` for manual window creation |

**No changes to:** `crates/niribar/src/surface.rs`, `crates/niribar/src/main.rs`, `crates/niribar/config/default.kdl`, or any module file.

## Tasks / Subtasks

- [x] Task 1: Extend `OutputConfig` and add `effective_bar_for()` in `crates/niribar/src/config.rs` (AC: #1–4)

  - [x] 1.1: Add optional override fields to `OutputConfig`:

    ```rust
    #[derive(Debug, Clone)]
    pub struct OutputConfig {
        pub name: String,
        pub enabled: bool,
        /// Override for bar.collapsed-width. None → inherit from global.
        pub collapsed_width: Option<u32>,
        /// Override for bar.expanded-width. None → inherit from global.
        pub expanded_width: Option<u32>,
        /// Override for bar.modules zone layout. None → inherit from global.
        pub modules: Option<BarModules>,
    }
    ```

  - [x] 1.2: Add `Config::effective_bar_for()` method (after the `Config` impl block or as a free function):

    ```rust
    impl Config {
        /// Return the effective `BarConfig` for the given output name.
        /// Per-output overrides take precedence; absent fields inherit from the global `bar`.
        /// Returns a clone of the global `bar` when the output has no config block.
        pub fn effective_bar_for(&self, output_name: &str) -> BarConfig {
            let global = &self.bar;
            let Some(oc) = self.outputs.iter().find(|o| o.name == output_name) else {
                return global.clone();
            };
            BarConfig {
                collapsed_width: oc.collapsed_width.unwrap_or(global.collapsed_width),
                expanded_width: oc.expanded_width.unwrap_or(global.expanded_width),
                modules: oc.modules.clone().unwrap_or_else(|| global.modules.clone()),
                // Non-overridable fields always inherit from global
                edge: global.edge.clone(),
                expand: global.expand,
                expand_trigger: global.expand_trigger,
                hover_delay: global.hover_delay,
                expand_keybind: global.expand_keybind.clone(),
            }
        }
    }
    ```

  - [x] 1.3: Update `validate()` to check per-output override constraints:

    Add after the duplicate output name check:
    ```rust
    // Validate per-output width overrides
    for oc in &config.outputs {
        if let Some(cw) = oc.collapsed_width {
            if cw < 2 {
                anyhow::bail!(
                    "output {:?}: collapsed-width: value {} is too small. \
                     Expected: 2 or greater.",
                    oc.name, cw
                );
            }
        }
        // effective collapsed width: per-output override OR global
        let effective_collapsed = oc.collapsed_width.unwrap_or(config.bar.collapsed_width);
        if let Some(ew) = oc.expanded_width {
            if ew < effective_collapsed {
                anyhow::bail!(
                    "output {:?}: expanded-width: value {} must be >= effective collapsed-width ({}). \
                     Expected: a width >= collapsed-width.",
                    oc.name, ew, effective_collapsed
                );
            }
        }
    }
    ```

- [x] Task 2: Extend `parse_output_sections()` in `src/config/parse.rs` (AC: #1–4)

  - [x] 2.1: Update the imports at the top of `parse.rs` — `BarModules` is already imported; no new imports needed.

  - [x] 2.2: Replace the `parse_output_sections` function body to also handle the new fields.

    Replace the match arm `other => { anyhow::bail!(...) }` to include the new keys, and parse `collapsed-width`, `expanded-width`, and `modules`:

    ```rust
    fn parse_output_sections(doc: &KdlDocument) -> anyhow::Result<Vec<OutputConfig>> {
        let mut result = Vec::new();
        for node in doc.nodes() {
            if node.name().value() != "output" {
                continue;
            }
            let name = node
                .entries()
                .iter()
                .find(|e| e.name().is_none())
                .and_then(|e| e.value().as_string())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "output: missing name argument. \
                         Expected: output \"<name>\" {{ ... }}, e.g. output \"DP-1\" {{ enabled #false }}"
                    )
                })?
                .to_string();

            let mut enabled = true;
            let mut collapsed_width: Option<u32> = None;
            let mut expanded_width: Option<u32> = None;
            let mut modules: Option<BarModules> = None;

            if let Some(children) = node.children() {
                for child in children.nodes() {
                    match child.name().value() {
                        "enabled" => {
                            enabled = child
                                .entries()
                                .iter()
                                .find(|e| e.name().is_none())
                                .and_then(|e| e.value().as_bool())
                                .ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "output {name:?}: enabled: expected a boolean value (#true or #false)."
                                    )
                                })?;
                        }
                        "collapsed-width" => {
                            let v = child
                                .entries()
                                .iter()
                                .find(|e| e.name().is_none())
                                .and_then(|e| e.value().as_integer())
                                .and_then(|v| u32::try_from(v).ok())
                                .ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "output {name:?}: collapsed-width: expected a positive integer."
                                    )
                                })?;
                            collapsed_width = Some(v);
                        }
                        "expanded-width" => {
                            let v = child
                                .entries()
                                .iter()
                                .find(|e| e.name().is_none())
                                .and_then(|e| e.value().as_integer())
                                .and_then(|v| u32::try_from(v).ok())
                                .ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "output {name:?}: expanded-width: expected a positive integer."
                                    )
                                })?;
                            expanded_width = Some(v);
                        }
                        "modules" => {
                            // Reuse parse_bar_modules: it takes &KdlDocument (the children of the
                            // modules node). The `modules {}` node here is a child of the output block,
                            // and its children are the zone nodes (top, center, bottom).
                            let mods = if let Some(zone_doc) = child.children() {
                                parse_bar_modules_from_doc(zone_doc)?
                            } else {
                                BarModules::default()
                            };
                            modules = Some(mods);
                        }
                        other => {
                            anyhow::bail!(
                                "output {name:?}: unknown key {other:?}. \
                                 Expected: one of: enabled, collapsed-width, expanded-width, modules."
                            );
                        }
                    }
                }
            }
            result.push(OutputConfig { name, enabled, collapsed_width, expanded_width, modules });
        }
        Ok(result)
    }
    ```

  - [x] 2.3: Refactor `parse_bar_modules` to extract a `parse_bar_modules_from_doc` helper so it can be reused for output blocks.

    The existing `parse_bar_modules` function takes `children: &kdl::KdlDocument` (the children of the bar node) and calls `children.get("modules")`. For output blocks, the `modules` node is the child node itself, and we already have its `KdlDocument` children.

    Add this helper function alongside `parse_bar_modules`:

    ```rust
    /// Parse `top`, `center`, `bottom` zone nodes from a `KdlDocument`.
    /// Called by both `parse_bar_modules` (bar.modules context) and the output modules override.
    fn parse_bar_modules_from_doc(zone_doc: &kdl::KdlDocument) -> anyhow::Result<BarModules> {
        let mut result = BarModules::default();
        for zone_node in zone_doc.nodes() {
            let zone_name = zone_node.name().value();
            let names: Vec<String> = zone_node
                .entries()
                .iter()
                .filter(|e| e.name().is_none())
                .filter_map(|e| e.value().as_string())
                .map(|s| s.to_string())
                .collect();
            match zone_name {
                "top" => result.top = names,
                "center" => result.center = names,
                "bottom" => result.bottom = names,
                other => anyhow::bail!(
                    "modules: unknown zone {other:?}. \
                     Expected: one of: top, center, bottom."
                ),
            }
        }
        Ok(result)
    }
    ```

    Update the existing `parse_bar_modules` to delegate to the new helper:

    ```rust
    fn parse_bar_modules(children: &kdl::KdlDocument) -> anyhow::Result<BarModules> {
        let Some(modules_node) = children.get("modules") else {
            return Ok(BarModules::default());
        };
        let Some(zone_doc) = modules_node.children() else {
            return Ok(BarModules::default());
        };
        parse_bar_modules_from_doc(zone_doc)
    }
    ```

- [x] Task 3: Update `App::view()` to use per-output effective bar config in `crates/niribar/src/app.rs` (AC: #1–3)

  - [x] 3.1: At the top of `view()`, replace direct `self.config.bar` accesses with a local `bar` binding:

    Replace:
    ```rust
    pub fn view(&self, window: iced::window::Id) -> Element<'_, Message> {
        let is_horizontal = self.config.bar.edge.is_horizontal();
        let surface = self.surface(window).cloned().unwrap_or_default();

        let surface_size = surface.expand.current_width(
            self.config.bar.collapsed_width,
            self.config.bar.expanded_width,
        );
    ```

    With:
    ```rust
    pub fn view(&self, window: iced::window::Id) -> Element<'_, Message> {
        let surface = self.surface(window).cloned().unwrap_or_default();
        // Resolve per-output overrides; falls back to global bar config for AllScreens mode
        // (output_name is "" when using StartMode::AllScreens — effective_bar_for returns global).
        let bar = self.config.effective_bar_for(&surface.output_name);
        let is_horizontal = bar.edge.is_horizontal();

        let surface_size = surface.expand.current_width(
            bar.collapsed_width,
            bar.expanded_width,
        );
    ```

  - [x] 3.2: Update the module rendering loop to filter by the effective per-output module list.

    Currently the loop iterates `&self.modules` and assigns to top/center/bottom based on the boot-time `zone`. Replace the loop and subsequent `self.config.bar.edge` accesses to use `bar`:

    Replace:
    ```rust
    for (zone, module) in &self.modules {
        ...
        match zone {
            Zone::Top => top.push(view),
            Zone::Center => center.push(view),
            Zone::Bottom => bottom.push(view),
        }
    }
    ```

    With:
    ```rust
    // Build a name→zone map from the effective (possibly per-output) module list.
    // This also acts as a filter: modules not in the effective list are skipped.
    let mut effective_zone: HashMap<String, Zone> = HashMap::new();
    for name in &bar.modules.top {
        effective_zone.insert(name.clone(), Zone::Top);
    }
    for name in &bar.modules.center {
        effective_zone.insert(name.clone(), Zone::Center);
    }
    for name in &bar.modules.bottom {
        effective_zone.insert(name.clone(), Zone::Bottom);
    }

    for (_boot_zone, module) in &self.modules {
        let module_name = module.id().to_string(); // ModuleId Display impl is the name string
        let Some(zone) = effective_zone.get(&module_name) else {
            continue; // not in this output's effective module list
        };
        ... // render as before
        match zone {
            Zone::Top => top.push(view),
            Zone::Center => center.push(view),
            Zone::Bottom => bottom.push(view),
        }
    }
    ```

  - [x] 3.3: Update remaining `self.config.bar.*` accesses in `view()` to use `bar.*`:

    Replace all occurrences of `self.config.bar.collapsed_width`, `self.config.bar.expanded_width`, and `self.config.bar.edge` **within the `view()` function** with `bar.collapsed_width`, `bar.expanded_width`, and `bar.edge`. (Do NOT change accesses in `update()` — those use global config intentionally.)

    Specific lines to update (in addition to those changed in 3.1):
    - The second `current_width()` call near the border layout code (`self.config.bar.collapsed_width`, `self.config.bar.expanded_width`)
    - `match self.config.bar.edge {` → `match bar.edge {`
    - `if self.config.bar.edge.is_horizontal() {` → `if bar.edge.is_horizontal() {` (the one in view's border sizing)

- [x] Task 4: Update `App::boot()` to use per-output `collapsed_width` for manual window creation in `crates/niribar/src/app.rs` (AC: #1–3)

  In `boot()`, the manual window creation block (when `allowed_outputs` is non-empty) currently uses `config.bar.collapsed_width` for all windows. Change it to use the per-output effective width:

  Replace:
  ```rust
  allowed_outputs
      .iter()
      .map(|name| {
          let settings = NewLayerShellSettings {
              anchor,
              exclusive_zone: Some(exclusive_zone),
              size,
              output_option: OutputOption::OutputName(name.clone()),
              namespace: Some(Self::namespace()),
              ..Default::default()
          };
          let (id, task) = Message::layershell_open(settings);
          pending_output_names.insert(id, name.clone());
          task
      })
      .collect()
  ```

  With:
  ```rust
  allowed_outputs
      .iter()
      .map(|name| {
          // Use per-output collapsed_width if configured (Story 7.4).
          // This ensures the exclusive zone and window size match the rendered bar width.
          let eff = config.effective_bar_for(name);
          let eff_collapsed = eff.collapsed_width;
          let eff_exclusive_zone = i32::try_from(eff_collapsed).unwrap_or(48);
          let eff_size = if eff.edge.is_horizontal() {
              Some((0, eff_collapsed))
          } else {
              Some((eff_collapsed, 0))
          };
          let settings = NewLayerShellSettings {
              anchor,
              exclusive_zone: Some(eff_exclusive_zone),
              size: eff_size,
              output_option: OutputOption::OutputName(name.clone()),
              namespace: Some(Self::namespace()),
              ..Default::default()
          };
          let (id, task) = Message::layershell_open(settings);
          pending_output_names.insert(id, name.clone());
          task
      })
      .collect()
  ```

  Note: `anchor` is still from the global `config.bar.edge` (edge is not overridable per-output), so that variable does not need to change. You can remove the now-unused `collapsed_width`, `exclusive_zone`, and `size` bindings in the `if !allowed_outputs.is_empty()` block.

- [x] Task 5: Write tests in `crates/niribar/src/config.rs` and `src/config/parse.rs` (AC: #6)

  - [x] 5.1: Add tests for `effective_bar_for()` in `crates/niribar/src/config.rs` tests block:

    ```rust
    #[test]
    fn effective_bar_for_unknown_output_returns_global() {
        let config = Config::default();
        let bar = config.effective_bar_for("DP-1");
        assert_eq!(bar.collapsed_width, config.bar.collapsed_width);
        assert_eq!(bar.expanded_width, config.bar.expanded_width);
    }

    #[test]
    fn effective_bar_for_empty_name_returns_global() {
        // AllScreens mode: output_name is "" — must return global defaults
        let config = Config::default();
        let bar = config.effective_bar_for("");
        assert_eq!(bar.collapsed_width, config.bar.collapsed_width);
    }

    #[test]
    fn effective_bar_for_applies_collapsed_width_override() {
        let mut config = Config::default();
        config.outputs = vec![OutputConfig {
            name: "eDP-1".into(),
            enabled: true,
            collapsed_width: Some(60),
            expanded_width: None,
            modules: None,
        }];
        let bar = config.effective_bar_for("eDP-1");
        assert_eq!(bar.collapsed_width, 60);
        // expanded_width inherits from global
        assert_eq!(bar.expanded_width, config.bar.expanded_width);
    }

    #[test]
    fn effective_bar_for_applies_modules_override() {
        let mut config = Config::default();
        let override_mods = BarModules {
            top: vec!["clock".into()],
            center: vec![],
            bottom: vec![],
        };
        config.outputs = vec![OutputConfig {
            name: "eDP-1".into(),
            enabled: true,
            collapsed_width: None,
            expanded_width: None,
            modules: Some(override_mods.clone()),
        }];
        let bar = config.effective_bar_for("eDP-1");
        assert_eq!(bar.modules.top, override_mods.top);
        assert!(bar.modules.center.is_empty());
    }

    #[test]
    fn effective_bar_for_partial_override_inherits_rest() {
        let mut config = Config::default();
        config.bar.collapsed_width = 48;
        config.bar.expanded_width = 200;
        config.outputs = vec![OutputConfig {
            name: "eDP-1".into(),
            enabled: true,
            collapsed_width: Some(60),  // only this is overridden
            expanded_width: None,
            modules: None,
        }];
        let bar = config.effective_bar_for("eDP-1");
        assert_eq!(bar.collapsed_width, 60);       // overridden
        assert_eq!(bar.expanded_width, 200);        // inherited
        assert_eq!(bar.modules.top, config.bar.modules.top);  // inherited
    }

    #[test]
    fn validate_per_output_collapsed_width_too_small_errors() {
        let config = Config {
            bar: BarConfig::default(),
            modules: vec![],
            outputs: vec![OutputConfig {
                name: "DP-1".into(),
                enabled: true,
                collapsed_width: Some(1),
                expanded_width: None,
                modules: None,
            }],
        };
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("DP-1"), "got: {msg}");
        assert!(msg.contains("collapsed-width"), "got: {msg}");
    }

    #[test]
    fn validate_per_output_expanded_less_than_collapsed_errors() {
        let config = Config {
            bar: BarConfig::default(),
            modules: vec![],
            outputs: vec![OutputConfig {
                name: "DP-1".into(),
                enabled: true,
                collapsed_width: Some(100),
                expanded_width: Some(50),
                modules: None,
            }],
        };
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("DP-1"), "got: {msg}");
        assert!(msg.contains("expanded-width"), "got: {msg}");
    }
    ```

  - [x] 5.2: Add parsing tests in `src/config/parse.rs` tests block:

    ```rust
    #[test]
    fn output_collapsed_width_override_parses() {
        let config = parse(r#"output "eDP-1" { collapsed-width 60 }"#).unwrap();
        assert_eq!(config.outputs[0].collapsed_width, Some(60));
    }

    #[test]
    fn output_expanded_width_override_parses() {
        let config = parse(r#"output "eDP-1" { expanded-width 300 }"#).unwrap();
        assert_eq!(config.outputs[0].expanded_width, Some(300));
    }

    #[test]
    fn output_modules_override_parses() {
        let kdl = r#"output "eDP-1" { modules { top "clock" "battery" } }"#;
        let config = parse(kdl).unwrap();
        let mods = config.outputs[0].modules.as_ref().unwrap();
        assert_eq!(mods.top, vec!["clock", "battery"]);
        assert!(mods.center.is_empty());
        assert!(mods.bottom.is_empty());
    }

    #[test]
    fn output_no_override_fields_are_none() {
        let config = parse(r#"output "eDP-1" { enabled #true }"#).unwrap();
        let oc = &config.outputs[0];
        assert!(oc.collapsed_width.is_none());
        assert!(oc.expanded_width.is_none());
        assert!(oc.modules.is_none());
    }

    #[test]
    fn output_unknown_key_still_errors_with_new_known_keys() {
        // The unknown-key error now lists the new keys too
        let result = parse(r#"output "DP-1" { zoom 2 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("collapsed-width"), "error should mention new keys: {msg}");
    }
    ```

## Dev Notes

### `ModuleId` Display / name accessor

`ModuleId` is the instance name string of a module. In `view()`, to get the string name for map lookup, use `module.id().to_string()` — `ModuleId` implements `Display` (its string is the name). Confirm by checking `src/modules/mod.rs` for the `ModuleId` type definition.

### `HashMap` import in `view()`

`view()` already has access to `HashMap` via the `use std::collections::HashMap;` at the top of `app.rs` (or via `std::collections::HashMap` inline). The `HashMap` from `std` is already imported — no new import needed.

### `effective_bar_for()` with empty output name (AllScreens mode)

When using `StartMode::AllScreens` (no output exclusions configured), `pending_output_names` is empty and `SurfaceState::output_name` is `""`. `effective_bar_for("")` finds no `OutputConfig` with `name == ""` and returns the global `BarConfig` — correct behavior, no special-casing needed.

### `anchor` variable in `boot()` manual window creation

After Task 4, the `anchor` variable computed before the `if !allowed_outputs.is_empty()` block still uses `config.bar.edge.anchor()`. Since `edge` is not per-output overridable, this is correct — all windows use the same edge/anchor from global config. The per-output `eff.edge` will always equal `config.bar.edge` since `effective_bar_for` always inherits `edge` from global. The `anchor` binding outside the loop remains unchanged.

### Unused variable warnings

After removing `collapsed_width`, `exclusive_zone`, and `size` from the outer scope in `boot()`, ensure there are no unused-variable warnings. Those bindings are now computed inside the `map()` closure per-output.

### `parse_bar_modules_from_doc` function name

This is a new private function in `parse.rs`. Name it clearly to distinguish it from `parse_bar_modules`. The `parse_bar_modules` function delegates to it so the bar-section path and the output-override path share the same zone-parsing logic.

### Test for `output_unknown_key_still_errors_with_new_known_keys`

The existing `output_unknown_key_errors` test uses `zoom 2` — it still passes (unknown key → error). The new test additionally asserts the error message mentions the new valid keys (`collapsed-width`), which confirms the error message was updated. Adjust the assertion if the actual error format differs.

### Clippy: `#[allow(dead_code)]` on `output_name`

`SurfaceState.output_name` carries `#[allow(dead_code)]`. After Story 7.4, `output_name` IS read in `view()` (via `surface.output_name`). Remove the `#[allow(dead_code)]` annotation from `output_name` in `crates/niribar/src/surface.rs` — it's no longer dead code. Also remove the `#[allow(dead_code)]` from `scale_factor` if it gets read too; otherwise leave it.

### `BarConfig` must derive/implement `Clone`

`effective_bar_for()` returns `BarConfig` by value. `BarConfig` is already `#[derive(Debug, Clone)]` — confirmed. `BarModules` is `#[derive(Debug, Clone, Default)]` — confirmed. No derive changes needed.

### Test count baseline

334 tests pass as of Story 7.3 implementation. Task 5 adds 12 new tests (7 in config.rs + 5 in parse.rs) → target ≥ 346.

## Previous Story Intelligence (Story 7.3)

- `OutputConfig { name: String, enabled: bool }` — adding 3 new optional fields in this story
- `SurfaceState.output_name` is populated from `pending_output_names` map in `surface_mut()` — Story 7.4 is the first story to READ this field in production code (`view()`)
- `#[allow(dead_code)]` on `SurfaceState.output_name` — remove it in this story (field is now used)
- `App::boot(config, allowed_outputs)` — Task 4 modifies the manual window creation loop inside
- `parse_output_sections()` currently errors on any unknown key beyond `enabled` — extend the match arm
- The unknown-key error message currently says `Expected: one of: enabled.` — update to list all valid keys
- `App.pending_output_names` is `pub` — accessible in tests if needed
- `parse_bar_modules()` at line 455 of `parse.rs` — reuse its zone-parsing logic via `parse_bar_modules_from_doc`
- All tests calling `App::boot(...)` already use `App::boot(config, vec![])` (updated in Story 7.3)
- The KDL boolean syntax is `#true`/`#false` (KDL v2) — confirmed by existing tests in `crates/niribar/src/config.rs`

## Definition of Done

- [x] `OutputConfig` has 3 new `Option` fields: `collapsed_width`, `expanded_width`, `modules`
- [x] `Config::effective_bar_for(output_name: &str) -> BarConfig` implemented and merges correctly
- [x] `validate()` checks per-output `collapsed_width >= 2` and `expanded_width >= effective_collapsed`
- [x] `parse_output_sections()` parses `collapsed-width`, `expanded-width`, `modules {}` from output blocks
- [x] Unknown key error message updated to list `enabled, collapsed-width, expanded-width, modules`
- [x] `parse_bar_modules_from_doc()` extracted as shared helper; `parse_bar_modules` delegates to it
- [x] `App::view()` uses `config.effective_bar_for(&surface.output_name)` for all bar config access
- [x] Module rendering loop in `view()` filters and re-zones by effective module list
- [x] `App::boot()` uses per-output `effective_bar_for()` for window size/exclusive_zone in manual creation path
- [x] `#[allow(dead_code)]` removed from `SurfaceState.output_name` in `crates/niribar/src/surface.rs`
- [x] `cargo clippy -- -D warnings` clean
- [x] `cargo test` shows ≥ 346 tests passing (actual: 351)

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

- Fixed collapsible `if` lint in `validate()` — clippy requires `if let Some(x) = ... && condition` form.
- Inline multi-child KDL on one line (`collapsed-width 60 expanded-width 250`) is not valid KDL; test fixed to use newline-separated children.

### Completion Notes List

- Extended `OutputConfig` with `collapsed_width: Option<u32>`, `expanded_width: Option<u32>`, `modules: Option<BarModules>`.
- Added `Config::effective_bar_for(output_name)` that returns a `BarConfig` merging per-output overrides with global bar config. Non-overridable fields (edge, expand, expand_trigger, hover_delay, expand_keybind) always inherit from global.
- Extended `validate()` to reject per-output `collapsed_width < 2` and `expanded_width < effective_collapsed`.
- Extended `parse_output_sections()` to parse `collapsed-width`, `expanded-width`, and `modules {}` child nodes.
- Extracted `parse_bar_modules_from_doc()` private helper; `parse_bar_modules` delegates to it — shared by bar section and output overrides.
- Updated `App::view()`: resolved per-output effective bar config at top of function; replaced module loop zone logic with `effective_zone` map built from `bar.modules`; all `self.config.bar.*` accesses replaced with `bar.*`.
- Updated `overlay_size_change_for()` to use per-output effective bar config for correct resize tasks.
- Updated `App::boot()` manual window creation loop to use per-output `collapsed_width` for each window's exclusive zone and size.
- Removed `#[allow(dead_code)]` from `SurfaceState.output_name` — now read in `view()`.
- Added 17 new tests (11 in `config.rs`, 6 in `parse.rs`). Final count: 351 tests.

### File List

- `crates/niribar/src/config.rs` — Extended `OutputConfig`; added `impl Config { effective_bar_for() }`; updated `validate()`; added 11 tests
- `src/config/parse.rs` — Extended `parse_output_sections()`; added `parse_bar_modules_from_doc()` helper; updated `parse_bar_modules()`; added 6 tests
- `crates/niribar/src/app.rs` — Updated `view()` and `overlay_size_change_for()` to use `effective_bar_for()`; updated `boot()` for per-output window sizing
- `crates/niribar/src/surface.rs` — Removed `#[allow(dead_code)]` from `output_name`; updated field doc comment

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-04-06 | 1.0 | Created story: per-output configuration overrides (collapsed-width, expanded-width, modules) | claude-sonnet-4-6 |
| 2026-04-06 | 1.1 | Implemented: OutputConfig extended, effective_bar_for(), view/boot updated, 17 new tests (351 total) | claude-sonnet-4-6 |
