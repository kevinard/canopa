# Story 9.1: Named Bar Sections with Output Targeting

Status: ready-for-dev

## Story

As a user who wants flexible multi-bar layouts,
I want to define named `bar "name" { }` sections with an `output` field,
so that I can place multiple independent bars on the same output or target a bar to specific outputs only.

## Acceptance Criteria

1. **Given** a config with `bar "main" { output "all" ... }` (or `output` field omitted, defaulting to "all")
   **When** niribar starts
   **Then** one surface for "main" appears on every connected output

2. **Given** a config with two bar sections that both target the same output (e.g. `output "all"` on both, or one `output "all"` and one `output "DP-1"`)
   **When** niribar starts on that output
   **Then** two surfaces are created for that output, each using its own bar's settings (edge, width, modules, etc.)

3. **Given** a config with `bar "side" { output "DP-1" ... }`
   **When** niribar starts and "DP-1" is connected
   **Then** a surface for "side" appears only on "DP-1"; other outputs do not receive this bar

4. **Given** a config with `bar "side" { output "DP-1" ... }`
   **When** niribar starts and "DP-1" is not connected
   **Then** no surface is created for "side" and niribar starts without error

5. **Given** two bar sections with the same name
   **When** niribar starts
   **Then** a config error is reported: duplicate bar name

6. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

7. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 402 pre-existing tests pass and at least 13 new tests pass (≥ 415 total)

## KDL Syntax

### New Format

```kdl
// Single bar on all outputs — equivalent to the old unnamed bar {}
bar "main" {
    // output "all"   ← optional; "all" is the default when field is omitted
    edge "left"
    collapsed-width 48
    expanded-width 200
    expand #true
    expand-trigger "click"
    hover-delay 0
    animation {
        expand-ms 280
        collapse-ms 200
    }
    modules {
        top "workspaces"
        center "focused-window"
        bottom "volume" "network" "battery" "clock"
    }
}

// A second bar appearing only on DP-1, at the top edge
bar "top-strip" {
    output "DP-1"
    edge "top"
    collapsed-width 32
    expanded-width 32
    expand #false
    modules {
        center "clock"
    }
}
```

### Migration from Old Format

Old config:
```kdl
bar {
    edge "left"
    collapsed-width 48
}
output "DP-1" {
    edge "top"
    collapsed-width 32
}
```

New equivalent:
```kdl
bar "main" {
    edge "left"
    collapsed-width 48
}
bar "main-dp1" {
    output "DP-1"
    edge "top"
    collapsed-width 32
}
```

To suppress a bar on a specific output, simply don't define a bar section targeting it (or ensure no `output "all"` bars exist).

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `src/config.rs` | **MODIFY** | Add `name: String` + `output: String` to `BarConfig`; replace `Config { bar, outputs }` with `Config { bars, modules }`; remove `OutputConfig`; replace `effective_bar_for()` with `bars_for_output()`; update `validate()` and `Default` impls |
| `src/config/parse.rs` | **MODIFY** | Replace `parse_bar_section()` + `parse_output_sections()` with `parse_bar_sections()`; add migration error guards for unnamed `bar {}` and `output "name" {}` nodes; update top-level unknown-node error |
| `src/config/defaults.rs` | **NO CHANGE** | Embeds `config/default.kdl` via `include_str!` — updating the .kdl file is sufficient |
| `config/default.kdl` | **MODIFY** | Rename `bar { }` → `bar "main" { }`; remove `output` example comments; add `output "all"` note; update per-output-override comments to show new `bar "name" { output "..." }` pattern |
| `src/surface.rs` | **MODIFY** | Add `bar_name: String` field to `SurfaceState`; update `new()`, `Default`, and existing tests |
| `src/app.rs` | **MODIFY** | Rename `pending_output_names` → `pending_surface_keys: HashMap<Id, (String, String)>`; update `boot()` to spawn one window per (output, bar) pair; update `surface_mut()` to set `bar_name`; update `view()` to look up bar by `surface.bar_name`; update `open_new_output_windows()` to spawn all matching bars for a new output |

## Tasks / Subtasks

- [ ] Task 1: Update `BarConfig`, `Config`, remove `OutputConfig` in `src/config.rs` (AC: #1–7)

  - [ ] 1.1 Add `name: String` and `output: String` to `BarConfig` (insert after the struct opening, before `edge`):

    ```rust
    pub struct BarConfig {
        /// Instance name — unique identifier for this bar (e.g. "main", "top-strip").
        pub name: String,
        /// Wayland output name this bar targets, or `"all"` to appear on every output.
        /// Default: `"all"`.
        pub output: String,
        pub edge: Edge,
        // ... all existing fields unchanged
    }
    ```

    Update `BarConfig::default()`:
    ```rust
    impl Default for BarConfig {
        fn default() -> Self {
            BarConfig {
                name: "main".to_string(),
                output: "all".to_string(),
                edge: Edge::Left,
                collapsed_width: 48,
                expanded_width: 200,
                expand: true,
                expand_trigger: ExpandTrigger::Click,
                hover_delay: 0,
                expand_keybind: None,
                animation: AnimationConfig::default(),
                modules: BarModules::default(),
            }
        }
    }
    ```

  - [ ] 1.2 Replace `Config { bar: BarConfig, outputs: Vec<OutputConfig> }` with `Config { bars: Vec<BarConfig>, modules: Vec<ModuleConfig> }`. Remove the `OutputConfig` struct entirely.

    ```rust
    #[derive(Debug, Clone)]
    pub struct Config {
        pub bars: Vec<BarConfig>,
        pub modules: Vec<ModuleConfig>,
    }

    impl Default for Config {
        fn default() -> Self {
            Config {
                bars: vec![BarConfig::default()],
                modules: vec![],
            }
        }
    }
    ```

    Note: `Config` can no longer derive `Default` automatically (because `BarConfig` no longer derives `Default` trivially with the new fields) — implement it manually as above.

  - [ ] 1.3 Replace `effective_bar_for()` with `bars_for_output()`:

    Remove `Config::effective_bar_for()` entirely. Add:

    ```rust
    impl Config {
        /// Return all bar configs that should appear on the given output.
        ///
        /// A bar with `output == "all"` appears on every connected output.
        /// A bar with a specific output name appears only on that output.
        /// In AllScreens mode (`output_name == ""`), returns all bars with `output == "all"`.
        pub fn bars_for_output(&self, output_name: &str) -> Vec<&BarConfig> {
            self.bars
                .iter()
                .filter(|b| b.output == "all" || b.output == output_name)
                .collect()
        }
    }
    ```

  - [ ] 1.4 Update `validate()`. Replace the per-output-override validation block with:

    ```rust
    // Require at least one bar section
    if config.bars.is_empty() {
        anyhow::bail!(
            "config must contain at least one bar section, e.g. bar \"main\" {{ ... }}"
        );
    }

    // Duplicate bar name check
    let mut seen_names = std::collections::HashSet::new();
    for bar in &config.bars {
        if !seen_names.insert(bar.name.as_str()) {
            anyhow::bail!(
                "bar {:?}: duplicate name. Each bar section must have a unique name.",
                bar.name
            );
        }
    }

    // Per-bar width and expand constraints
    for bar in &config.bars {
        if bar.collapsed_width < 2 {
            anyhow::bail!(
                "bar {:?}: collapsed-width: value {} is too small. Expected: 2 or greater.",
                bar.name, bar.collapsed_width
            );
        }
        if bar.expanded_width < bar.collapsed_width {
            anyhow::bail!(
                "bar {:?}: expanded-width: value {} must be >= collapsed-width ({}). \
                 Expected: a width >= collapsed-width.",
                bar.name, bar.expanded_width, bar.collapsed_width
            );
        }
    }
    ```

    Remove all references to `config.bar` and `config.outputs` from `validate()`. The module-name validation loop that references `config.bar.modules` should now iterate over all `config.bars` — collect all referenced module names across all bars before checking.

    Updated module-name reference check:
    ```rust
    // Collect all module names referenced in any bar's zones
    let mut referenced: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for bar in &config.bars {
        for name in bar.modules.top.iter()
            .chain(bar.modules.center.iter())
            .chain(bar.modules.bottom.iter())
        {
            referenced.insert(name.as_str());
        }
    }
    // ... then check each referenced name against declared modules (unchanged logic)
    ```

- [ ] Task 2: Update parsing in `src/config/parse.rs` (AC: #1–7)

  - [ ] 2.1 Remove `parse_bar_section()` and `parse_output_sections()`. Add `parse_bar_sections(doc: &KdlDocument) -> anyhow::Result<Vec<BarConfig>>`:

    ```rust
    fn parse_bar_sections(doc: &KdlDocument) -> anyhow::Result<Vec<BarConfig>> {
        let mut bars = Vec::new();
        for node in doc.nodes() {
            if node.name().value() != "bar" {
                continue;
            }
            let name = node
                .entries()
                .iter()
                .find(|e| e.name().is_none())
                .and_then(|e| e.value().as_string())
                .ok_or_else(|| {
                    anyhow::anyhow!("bar: expected a name argument, e.g. bar \"main\" {{ ... }}")
                })?
                .to_string();

            let bar = parse_single_bar_node(node, &name)?;
            bars.push(bar);
        }

        Ok(bars)
    }
    ```

  - [ ] 2.2 Write `parse_single_bar_node(node: &KdlNode, name: &str) -> anyhow::Result<BarConfig>`.

    This function parses a single `bar "name" { }` node's children into a `BarConfig`. It starts from `BarConfig { name: name.to_string(), ..BarConfig::default() }` and applies child overrides.

    Parse the `output` field as a String (default "all"):
    ```rust
    "output" => {
        output = child
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "bar {name:?}: output: expected a string value, e.g. output \"DP-1\" or output \"all\"."
                )
            })?
            .to_string();
    }
    ```

    All other fields (`edge`, `collapsed-width`, `expanded-width`, `expand`, `expand-trigger`, `hover-delay`, `expand-keybind`, `animation`, `modules`) are parsed the same way they were in the old `parse_bar_section()`. Move that logic into this function, adjusting error messages to include the bar name: `"bar {name:?}: collapsed-width: ..."`.

    Unknown keys error:
    ```rust
    other => {
        anyhow::bail!(
            "bar {name:?}: unknown key {other:?}. Expected: one of: \
             output, edge, collapsed-width, expanded-width, expand, expand-trigger, \
             hover-delay, expand-keybind, animation, modules."
        );
    }
    ```

  - [ ] 2.3 Update the top-level `parse()` function. Remove the `output` arm from the known-node match (it's no longer a valid top-level node):

    ```rust
    pub fn parse(input: &str) -> anyhow::Result<Config> {
        let doc: KdlDocument = input.parse()?;
        let bars = parse_bar_sections(&doc)?;
        let modules = parse_module_sections(&doc)?;
        // Validate unknown top-level nodes
        for node in doc.nodes() {
            match node.name().value() {
                "bar" | "modules" => {}
                other => anyhow::bail!(
                    "unknown top-level section {other:?}. Expected: bar, modules."
                ),
            }
        }
        let config = Config { bars, modules };
        validate(&config)?;
        Ok(config)
    }
    ```

    Note: `parse_module_sections` is the existing function — check the exact name in `parse.rs` and use it unchanged.

  - [ ] 2.4 Remove `parse_bar_modules` helper duplication check: `parse_bar_modules_from_doc` was introduced in Story 7.4 and is used for zone parsing. It is still needed inside `parse_single_bar_node` for the `modules { }` child. Keep it.

- [ ] Task 3: Update default config files (AC: #1)

  - [ ] 3.1 Update `config/default.kdl`:
    - Rename `bar {` → `bar "main" {`
    - Add a comment above the `modules {` child: `// output "all"   ← default; omit or set to an output name like "DP-1"`
    - Remove the `// ─── Per-output overrides ────` comment block at the bottom
    - Replace it with a new comment block showing the new multi-bar syntax:

    ```kdl
    // ─── Additional bars ─────────────────────────────────────────────────────────
    // Add more bar sections to place additional bars on specific outputs.
    // Each bar section is independent — different edge, width, modules, etc.
    // The `output` field controls which Wayland output the bar appears on.
    // Run `niri msg outputs` to list connected output names.
    //
    // Example: a minimal top bar only on a secondary monitor
    // bar "top-strip" {
    //     output "DP-2"
    //     edge "top"
    //     collapsed-width 32
    //     expanded-width 32
    //     expand #false
    //     modules {
    //         center "clock"
    //     }
    // }
    ```

- [ ] Task 4: Add `bar_name` to `SurfaceState` in `src/surface.rs` (AC: #1–4)

  - [ ] 4.1 Add `pub bar_name: String` field to `SurfaceState`, after `output_name`:

    ```rust
    pub struct SurfaceState {
        /// Wayland output name for this surface. Empty ("") in AllScreens mode.
        pub output_name: String,
        /// Name of the `BarConfig` driving this surface (matches `BarConfig::name`).
        /// Empty ("") in AllScreens mode — app.rs falls back to `config.bars[0]`.
        pub bar_name: String,
        // ... existing fields unchanged
    }
    ```

  - [ ] 4.2 Update `SurfaceState::new()` to accept `bar_name`:

    ```rust
    pub fn new(output_name: String, bar_name: String) -> Self {
        SurfaceState {
            output_name,
            bar_name,
            scale_factor: 1.0,
            expand: ExpandState::default(),
            focused_module: None,
            hovered_module: None,
            module_activated: false,
            bar_hovered: false,
            hover_pending_ms: None,
        }
    }
    ```

  - [ ] 4.3 Update `SurfaceState::default()`:

    ```rust
    impl Default for SurfaceState {
        fn default() -> Self {
            SurfaceState::new(String::new(), String::new())
        }
    }
    ```

  - [ ] 4.4 Update existing tests in `src/surface.rs` that call `SurfaceState::new(...)` with one argument to pass two arguments: `SurfaceState::new("output".into(), "main".into())`. Update the struct literal test to include `bar_name: "main".into()`.

- [ ] Task 5: Update `App` surface spawning and view in `src/app.rs` (AC: #1–4)

  - [ ] 5.1 Rename `pending_output_names: HashMap<iced::window::Id, String>` to `pending_surface_keys: HashMap<iced::window::Id, (String, String)>` (type: `(output_name, bar_name)`). Update the field declaration, initialization, and all call sites throughout `app.rs`.

  - [ ] 5.2 Update the start-mode decision in `boot()`:

    Current logic: use AllScreens if no exclusions and no per-output overrides; otherwise Background.

    New logic:
    ```rust
    // Use AllScreens only for the simple single-bar-all-outputs case:
    // one bar section with output == "all". This preserves compatibility with
    // non-Niri compositors (no IPC needed for output enumeration).
    let use_allscreens = config.bars.len() == 1 && config.bars[0].output == "all";
    ```

    When `use_allscreens` is true: start mode is AllScreens (unchanged from before). When false: start mode is Background (manual window creation via Niri IPC).

  - [ ] 5.3 Update the manual window creation block in `boot()` (the Background path):

    Replace the current single-window-per-output loop with a (output, bar) nested loop:

    ```rust
    // For each connected output, spawn one surface per matching bar
    let tasks: Vec<Task<Message>> = allowed_outputs
        .iter()
        .flat_map(|output_name| {
            config.bars_for_output(output_name)
                .into_iter()
                .map(|bar| {
                    let anchor = bar.edge.anchor();
                    let exclusive_zone = i32::try_from(bar.collapsed_width).unwrap_or(48);
                    let size = if bar.edge.is_horizontal() {
                        Some((0, bar.collapsed_width))
                    } else {
                        Some((bar.collapsed_width, 0))
                    };
                    let settings = NewLayerShellSettings {
                        anchor,
                        exclusive_zone: Some(exclusive_zone),
                        size,
                        output_option: OutputOption::OutputName(output_name.clone()),
                        namespace: Some(Self::namespace()),
                        ..Default::default()
                    };
                    let (id, task) = Message::layershell_open(settings);
                    pending_surface_keys.insert(id, (output_name.clone(), bar.name.clone()));
                    task
                })
                .collect::<Vec<_>>()
        })
        .collect();
    Task::batch(tasks)
    ```

    Note: `allowed_outputs` is still populated by querying Niri IPC and filtering enabled outputs, unchanged from before. The only change is iterating bars per output.

  - [ ] 5.4 Update `surface_mut()` (or the location where `pending_output_names` was consumed to create `SurfaceState`) to use `pending_surface_keys`:

    Where currently:
    ```rust
    let output_name = self.pending_output_names.remove(&id).unwrap_or_default();
    SurfaceState::new(output_name)
    ```

    Replace with:
    ```rust
    let (output_name, bar_name) = self.pending_surface_keys.remove(&id).unwrap_or_default();
    SurfaceState::new(output_name, bar_name)
    ```

  - [ ] 5.5 Update `App::view()` to look up bar config by `surface.bar_name`:

    Replace:
    ```rust
    let bar = self.config.effective_bar_for(&surface.output_name);
    ```

    With:
    ```rust
    // Look up the bar config by name; fall back to first bar if name is empty (AllScreens mode).
    let bar = self.config.bars.iter()
        .find(|b| b.name == surface.bar_name)
        .cloned()
        .unwrap_or_else(|| self.config.bars.first().cloned().unwrap_or_default());
    ```

  - [ ] 5.6 Update `open_new_output_windows()` (hotplug for Background mode) to spawn all matching bars for each newly detected output, not just one surface. Apply the same (output, bar) loop pattern from 5.3. The deduplication guard (`self.surfaces.values().any(|s| s.output_name == name)`) needs to change to check for `(output_name, bar_name)` pairs:

    ```rust
    // Only spawn if this (output, bar) pair doesn't already have a surface
    let already_open = self.surfaces.values()
        .any(|s| s.output_name == *output_name && s.bar_name == bar.name);
    if !already_open {
        // spawn as in 5.3
    }
    ```

  - [ ] 5.7 Remove all references to `config.bar`, `config.outputs`, and `Config::effective_bar_for()` from `app.rs`. Any remaining `self.config.bar.*` accesses in `update()` must be replaced — check whether they can use `self.config.bars[0]` (global defaults) or should use `bars_for_output()`.

    Specifically: `update()` uses `self.config.bar.expand_trigger`, `self.config.bar.hover_delay`, `self.config.bar.expand_keybind` and similar for event handling. Since these are now per-bar, and `update()` receives a window ID, look up the bar via the surface's `bar_name`:

    ```rust
    // In update(), when you need bar config for a specific window:
    let surface = self.surface(window).cloned().unwrap_or_default();
    let bar = self.config.bars.iter()
        .find(|b| b.name == surface.bar_name)
        .cloned()
        .unwrap_or_else(|| self.config.bars.first().cloned().unwrap_or_default());
    ```

- [ ] Task 6: Write tests (AC: #9)

  - [ ] 6.1 Tests for `Config::bars_for_output()` in `src/config.rs`:

    ```rust
    #[test]
    fn bars_for_output_all_matches_any_output() {
        let config = Config::default(); // single bar "main" with output "all"
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].name, "main");
    }

    #[test]
    fn bars_for_output_specific_name_matches_only_that_output() {
        let mut config = Config::default();
        config.bars = vec![
            BarConfig { name: "main".into(), output: "all".into(), ..BarConfig::default() },
            BarConfig { name: "side".into(), output: "DP-1".into(), ..BarConfig::default() },
        ];
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars.len(), 2); // both "main" (all) and "side" (DP-1)
        let bars_edp = config.bars_for_output("eDP-1");
        assert_eq!(bars_edp.len(), 1); // only "main"
    }

    #[test]
    fn bars_for_output_no_all_bars_specific_output_not_connected() {
        let mut config = Config::default();
        config.bars = vec![
            BarConfig { name: "side".into(), output: "DP-1".into(), ..BarConfig::default() },
        ];
        let bars = config.bars_for_output("eDP-1");
        assert!(bars.is_empty());
    }

    #[test]
    fn bars_for_output_empty_string_matches_only_all() {
        // AllScreens mode: output_name is ""
        let config = Config::default();
        let bars = config.bars_for_output("");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].output, "all");
    }

    #[test]
    fn validate_duplicate_bar_name_errors() {
        let config = Config {
            bars: vec![
                BarConfig { name: "main".into(), output: "all".into(), ..BarConfig::default() },
                BarConfig { name: "main".into(), output: "DP-1".into(), ..BarConfig::default() },
            ],
            modules: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("duplicate name"), "got: {err}");
    }

    #[test]
    fn validate_bar_collapsed_width_too_small_errors() {
        let config = Config {
            bars: vec![BarConfig { name: "x".into(), output: "all".into(), collapsed_width: 1, ..BarConfig::default() }],
            modules: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("collapsed-width"), "got: {err}");
    }
    ```

  - [ ] 6.2 Tests for parsing in `src/config/parse.rs`:

    ```rust
    #[test]
    fn named_bar_parses() {
        let config = parse(r#"bar "main" { edge "left" collapsed-width 48 expanded-width 200 }"#).unwrap();
        assert_eq!(config.bars.len(), 1);
        assert_eq!(config.bars[0].name, "main");
        assert_eq!(config.bars[0].output, "all"); // default
    }

    #[test]
    fn bar_output_field_parses() {
        let config = parse(r#"bar "side" { output "DP-1" edge "top" collapsed-width 32 expanded-width 32 }"#).unwrap();
        assert_eq!(config.bars[0].output, "DP-1");
    }

    #[test]
    fn bar_output_defaults_to_all_when_omitted() {
        let config = parse(r#"bar "main" { edge "left" collapsed-width 48 expanded-width 200 }"#).unwrap();
        assert_eq!(config.bars[0].output, "all");
    }

    #[test]
    fn multiple_bar_sections_parse() {
        let kdl = r#"
            bar "main" { edge "left" collapsed-width 48 expanded-width 200 }
            bar "side" { output "DP-1" edge "top" collapsed-width 32 expanded-width 32 }
        "#;
        let config = parse(kdl).unwrap();
        assert_eq!(config.bars.len(), 2);
        assert_eq!(config.bars[1].name, "side");
        assert_eq!(config.bars[1].output, "DP-1");
    }

    #[test]
    fn bar_unknown_key_errors_and_lists_output_field() {
        let result = parse(r#"bar "main" { zoom 2 collapsed-width 48 expanded-width 200 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("output"), "error should mention output field: {msg}");
    }
    ```

## Dev Notes

### `BarConfig` field addition compile impact

Every struct literal that constructs `BarConfig` directly (in tests and in `parse.rs`) must add `name` and `output`. Use `..BarConfig::default()` in tests to reduce churn. In `parse_single_bar_node`, start with `BarConfig { name: name.to_string(), output: "all".to_string(), ..BarConfig::default() }` and overwrite fields as children are parsed.

### `Config::default()` no longer `#[derive(Default)]`

Once `BarConfig` has `name` and `output` fields with non-empty defaults, `Config` cannot use `#[derive(Default)]` (it would produce empty strings). Remove the `Default` derive and implement manually. Keep `#[derive(Debug, Clone)]`.

### AllScreens mode `bar_name` in `SurfaceState`

In AllScreens mode, `pending_surface_keys` is never populated (iced_layershell creates windows automatically). The `bar_name` on each surface remains `""`. In `view()` and `update()`, the fallback `config.bars.first()` handles this correctly — there can only be one `output "all"` bar in AllScreens mode (by the `use_allscreens` condition).

### `overlay_size_change_for()` in `app.rs`

This function (added in Story 7.4) uses `config.effective_bar_for()`. Replace it with the bar lookup by `bar_name` pattern from Task 5.5. If `overlay_size_change_for()` receives a window ID, use it to get the surface's `bar_name`.

### Module name validation across all bars

Story 7.4's `validate()` collected module names from `config.bar.modules`. Update it to iterate all `config.bars` and union their module name sets. A module referenced in any bar must be declared in the `modules` section.

### `open_new_output_windows()` dedup check

The current guard uses `self.surfaces.values().any(|s| s.output_name == name)`. Replace with a check on `(output_name, bar_name)` pairs to allow multiple bars on the same output without false-positive deduplication.

### `parse_bar_modules_from_doc` unchanged

This private helper (added in Story 7.4) is still used for parsing `modules { }` children inside a bar node. No changes needed.

### `SurfaceState` existing tests

Tests in `src/surface.rs` that construct `SurfaceState::new(...)` with one argument break at compile time. Update them to pass two arguments. Tests that use `SurfaceState { ... }` struct literals need `bar_name: "main".into()` or `bar_name: String::new()`.

### Test count baseline: 402

Task 6 adds ≥ 15 tests (6 in `config.rs` + 8 in `parse.rs` + surface.rs additions are updates not new). Aim for 417+.

### References

- Current `BarConfig` struct: `src/config.rs:222`
- Current `Config` struct: `src/config.rs:153`
- Current `OutputConfig` struct: `src/config.rs:124`
- Current `effective_bar_for()`: `src/config.rs:169`
- `parse_bar_section()` / `parse_output_sections()`: `src/config/parse.rs`
- `parse_bar_modules_from_doc()`: `src/config/parse.rs` (Story 7.4)
- `SurfaceState` struct: `src/surface.rs:19`
- `App::boot()` spawn logic: `src/app.rs`
- `App::view()` bar config lookup: `src/app.rs`

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

### File List
