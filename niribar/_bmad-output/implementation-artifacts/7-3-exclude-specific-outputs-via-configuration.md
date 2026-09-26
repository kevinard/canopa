# Story 7.3: Exclude Specific Outputs via Configuration

Status: review

## Story

As a **user who does not want a bar on every screen**,
I want to exclude specific outputs from having a bar,
So that my secondary monitor or projector is unobstructed.

## Acceptance Criteria

1. **Given** the config specifies `output "DP-1" { enabled = false }` (or equivalent exclusion syntax)
   **When** niribar starts
   **Then** no surface is created for the excluded output

2. **And** all other outputs continue to receive a bar surface

3. **And** a connected output not mentioned in config receives a bar with default settings

4. **And** if the excluded output is disconnected and reconnected, it remains excluded

5. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

6. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 322 pre-existing tests pass and at least 11 new tests pass (≥ 333 total)

## The Output Exclusion Model

### Why This Requires Changing `StartMode::AllScreens`

Story 7.1 used `StartMode::AllScreens` which tells `iced_layershell` to automatically create one window per connected Wayland output. This is transparent — the iced app receives no notification of which output a window belongs to.

`iced_layershell` 0.17.1 fires `LayerShellEvent::XdgInfoChanged` (which carries the output name) per window unit, but its `multi_window` handler drops this event in the `_ => {}` branch. The iced app cannot receive output names via `iced_layershell`'s normal event path.

**Solution**: Switch from `StartMode::AllScreens` to **manual per-output window creation** using `Message::layershell_open(NewLayerShellSettings { output_option: OutputOption::OutputName(name) })`. When we create each window, we pre-assign `id → output_name` in a `pending_output_names` map so `SurfaceState` can be initialized with the correct name.

**Source for output names**: Use Niri IPC `Request::Outputs → Response::Outputs(HashMap<String, Output>)` (keys are output names) — same IPC connection already used for workspace events.

**Hotplug (AC #4)**: With manual creation and `StartMode::Background` (no auto windows), new outputs connected while the app is running will NOT automatically get a bar. The `NewDisplay` event in `layershellev` only auto-creates windows when `is_allscreens()` is true. For excluded outputs, this is the *correct* behavior — they remain excluded because they're never in the creation list.

**Fallback**: If Niri IPC is unavailable (non-Niri compositor or Niri not running), AND exclusions are configured, warn to stderr and fall back to `StartMode::AllScreens` (bars on all outputs, exclusion config ignored). If no exclusions are configured, `AllScreens` is used as before — zero behavior change for users without output exclusion config.

## File Map

| File | Action | What changes |
|------|--------|-------------|
| `src/config/mod.rs` | **MODIFY** | Add `OutputConfig`, `Vec<OutputConfig>` field to `Config` |
| `src/config/parse.rs` | **MODIFY** | Parse `output "name" { enabled = ... }` KDL blocks |
| `src/main.rs` | **MODIFY** | Add `fetch_niri_outputs()`, conditional `StartMode` in `try_main()`, pass `allowed_outputs` to `App::boot()` |
| `src/app.rs` | **MODIFY** | Add `pending_output_names` to `App`, update `boot()` signature + impl, update `surface_mut()` |

**No changes to:** `src/surface.rs`, `src/config/defaults.rs`, `config/default.kdl`, or any module file.

## Tasks / Subtasks

- [x] Task 1: Add `OutputConfig` and extend `Config` in `src/config/mod.rs` (AC: #1–4)
  - [x] 1.1: Add struct before `Config`:

    ```rust
    /// Per-output configuration, parsed from `output "name" { ... }` KDL blocks.
    /// A connected output not in this list receives the global defaults.
    #[derive(Debug, Clone)]
    pub struct OutputConfig {
        /// The Wayland output name (e.g. "eDP-1", "DP-1", "HDMI-A-1").
        pub name: String,
        /// Whether to create a bar surface on this output. Default: true.
        pub enabled: bool,
    }
    ```

  - [x] 1.2: Add `outputs` field to `Config`:

    ```rust
    /// Parsed configuration for niribar.
    /// Fully populated before any surface is created (AR7).
    #[derive(Debug, Clone, Default)]
    pub struct Config {
        pub bar: BarConfig,
        pub modules: Vec<ModuleConfig>,
        /// Per-output overrides. Outputs not listed here use global defaults.
        pub outputs: Vec<OutputConfig>,
    }
    ```

  - [x] 1.3: Add validation for duplicate output names in `validate()`:

    ```rust
    // Check for duplicate output names
    let mut seen_output_names = std::collections::HashSet::new();
    for oc in &config.outputs {
        if !seen_output_names.insert(&oc.name) {
            anyhow::bail!(
                "output {:?}: duplicate output name. \
                 Expected: each output name to appear at most once.",
                oc.name
            );
        }
    }
    ```

- [ ] Task 2: Parse `output` blocks in `src/config/parse.rs` (AC: #1–4)
  - [ ] 2.1: In `parse()`, call a new `parse_output_sections()` and store result in `Config.outputs`:

    ```rust
    pub fn parse(input: &str) -> anyhow::Result<Config> {
        let doc: KdlDocument = input.parse().map_err(|e| anyhow::anyhow!("config: {e}"))?;
        let bar = parse_bar_section(&doc)?;
        let modules = parse_modules_section(&doc)?;
        let outputs = parse_output_sections(&doc)?;
        Ok(Config { bar, modules, outputs })
    }
    ```

  - [ ] 2.2: Implement `parse_output_sections()`:

    ```rust
    fn parse_output_sections(doc: &KdlDocument) -> anyhow::Result<Vec<OutputConfig>> {
        let mut result = Vec::new();
        for node in doc.nodes() {
            if node.name().value() != "output" {
                continue;
            }
            // output name is the first positional argument
            let name = node
                .entries()
                .iter()
                .find(|e| e.name().is_none())
                .and_then(|e| e.value().as_string())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "output: missing name argument. \
                         Expected: output \"<name>\" {{ ... }}, e.g. output \"DP-1\" {{ enabled = false }}"
                    )
                })?
                .to_string();
            let mut enabled = true;
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
                                        "output {:?}: enabled: expected a boolean value (true or false).",
                                        name
                                    )
                                })?;
                        }
                        other => {
                            anyhow::bail!(
                                "output {:?}: unknown key {:?}. Expected: one of: enabled.",
                                name,
                                other
                            );
                        }
                    }
                }
            }
            result.push(OutputConfig { name, enabled });
        }
        Ok(result)
    }
    ```

- [ ] Task 3: Add `fetch_niri_outputs()` and update startup in `src/main.rs` (AC: #1–4)
  - [ ] 3.1: Add imports at the top of `main.rs`:

    ```rust
    use std::collections::HashSet;
    ```

  - [ ] 3.2: Add `fetch_niri_outputs()` after `send_ipc_command()`:

    ```rust
    /// Synchronously query Niri IPC for connected output names.
    /// Returns `None` if Niri is not running or the IPC call fails.
    fn fetch_niri_outputs() -> Option<Vec<String>> {
        let mut socket = niri_ipc::socket::Socket::connect().ok()?;
        match socket.send(niri_ipc::Request::Outputs).ok()? {
            Ok(niri_ipc::Response::Outputs(map)) => Some(map.into_keys().collect()),
            _ => None,
        }
    }
    ```

  - [ ] 3.3: Update `try_main()` to resolve `start_mode` and `allowed_outputs`:

    Replace this block in `try_main()`:
    ```rust
    // (OLD)
    daemon(
        move || App::boot(config.clone()),
        ...
    )
    .settings(Settings {
        layer_settings: LayerShellSettings {
            anchor,
            exclusive_zone,
            size,
            start_mode: StartMode::AllScreens,
            ..Default::default()
        },
        ...
    })
    ```

    With:
    ```rust
    // Compute which outputs to show the bar on.
    // If no outputs have enabled=false, skip Niri query and keep AllScreens (zero behavior change).
    let excluded: HashSet<String> = config
        .outputs
        .iter()
        .filter(|o| !o.enabled)
        .map(|o| o.name.clone())
        .collect();

    let (start_mode, allowed_outputs) = if excluded.is_empty() {
        // No exclusions configured — use AllScreens as before (Story 7.1 behavior).
        (StartMode::AllScreens, Vec::new())
    } else {
        match fetch_niri_outputs() {
            Some(all) => {
                let allowed: Vec<String> =
                    all.into_iter().filter(|n| !excluded.contains(n)).collect();
                (StartMode::Background, allowed)
            }
            None => {
                // Niri unavailable — warn and fall back to AllScreens (exclusions ignored).
                eprintln!(
                    "warning: niribar: output exclusion configured but Niri IPC unavailable; \
                     bar will appear on all outputs."
                );
                (StartMode::AllScreens, Vec::new())
            }
        }
    };

    daemon(
        move || App::boot(config.clone(), allowed_outputs.clone()),
        App::namespace,
        App::update,
        App::view,
    )
    .settings(Settings {
        layer_settings: LayerShellSettings {
            anchor,
            exclusive_zone,
            size,
            start_mode,
            ..Default::default()
        },
        ..Default::default()
    })
    .style(App::style)
    .subscription(App::subscription)
    .run()
    .map_err(anyhow::Error::from)
    ```

- [ ] Task 4: Update `App` in `src/app.rs` (AC: #1–4)
  - [ ] 4.1: Add `pending_output_names` field to `App`:

    ```rust
    pub struct App {
        pub config: Config,
        pub shared: SharedState,
        pub modules: Vec<(Zone, Box<dyn Module>)>,
        /// Per-output render state, keyed by iced::window::Id (Story 7.1).
        /// One entry per Wayland output; lazily initialised on first event or view call.
        pub surfaces: HashMap<iced::window::Id, SurfaceState>,
        /// Window IDs pre-assigned to output names during manual window creation (Story 7.3).
        /// Entry is consumed on first `surface_mut()` call for that id.
        /// Empty when using StartMode::AllScreens (no output exclusions configured).
        pending_output_names: HashMap<iced::window::Id, String>,
    }
    ```

  - [ ] 4.2: Add required imports near the top of `src/app.rs`:

    ```rust
    use iced_layershell::reexport::{NewLayerShellSettings, OutputOption};
    ```

  - [ ] 4.3: Change `App::boot()` signature and body:

    Change from:
    ```rust
    pub fn boot(config: Config) -> (Self, Task<Message>) {
    ```
    To:
    ```rust
    /// `allowed_outputs`: non-empty list of output names to create windows on (Story 7.3).
    /// Empty means `StartMode::AllScreens` handles window creation (no exclusions configured).
    pub fn boot(config: Config, allowed_outputs: Vec<String>) -> (Self, Task<Message>) {
    ```

    At the end of `boot()`, replace:
    ```rust
    let app = App {
        config,
        shared: SharedState::new(),
        modules,
        surfaces: HashMap::new(), // lazily populated on first view/event per window
    };
    (app, Task::none())
    ```
    With:
    ```rust
    // Create windows for each allowed output when using manual per-output creation.
    // When allowed_outputs is empty, StartMode::AllScreens handles window creation automatically.
    let mut pending_output_names: HashMap<iced::window::Id, String> = HashMap::new();
    let window_tasks: Vec<Task<Message>> = if !allowed_outputs.is_empty() {
        let anchor = config.bar.edge.anchor();
        let collapsed_width = config.bar.collapsed_width;
        let exclusive_zone = i32::try_from(collapsed_width).unwrap_or(48);
        let size = if config.bar.edge.is_horizontal() {
            Some((0, collapsed_width))
        } else {
            Some((collapsed_width, 0))
        };
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
    } else {
        vec![]
    };

    let app = App {
        config,
        shared: SharedState::new(),
        modules,
        surfaces: HashMap::new(),
        pending_output_names,
    };
    (app, Task::batch(window_tasks))
    ```

  - [ ] 4.4: Update `surface_mut()` to initialize `SurfaceState` with the pending output name:

    Replace:
    ```rust
    pub fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
        self.surfaces.entry(id).or_default()
    }
    ```
    With:
    ```rust
    pub fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
        if !self.surfaces.contains_key(&id) {
            // Pop the pending output name assigned during window creation (Story 7.3).
            // Falls back to "" when using StartMode::AllScreens (no exclusions).
            let name = self.pending_output_names.remove(&id).unwrap_or_default();
            self.surfaces.insert(id, SurfaceState::new(name));
        }
        self.surfaces.get_mut(&id).unwrap()
    }
    ```

- [ ] Task 5: Write tests in `src/config/mod.rs` and `src/app.rs` (AC: #6)
  - [ ] 5.1: Add config tests in `src/config/mod.rs` tests block:

    ```rust
    #[test]
    fn output_config_enabled_true_parses() {
        let config = parse::parse(r#"output "eDP-1" { enabled = true }"#).unwrap();
        assert_eq!(config.outputs.len(), 1);
        assert_eq!(config.outputs[0].name, "eDP-1");
        assert!(config.outputs[0].enabled);
    }

    #[test]
    fn output_config_enabled_false_parses() {
        let config = parse::parse(r#"output "DP-1" { enabled = false }"#).unwrap();
        assert_eq!(config.outputs.len(), 1);
        assert_eq!(config.outputs[0].name, "DP-1");
        assert!(!config.outputs[0].enabled);
    }

    #[test]
    fn output_config_empty_block_defaults_enabled_true() {
        // An output block with no children defaults enabled = true
        let config = parse::parse(r#"output "HDMI-1" {}"#).unwrap();
        assert_eq!(config.outputs.len(), 1);
        assert!(config.outputs[0].enabled);
    }

    #[test]
    fn config_with_no_output_blocks_has_empty_vec() {
        let config = parse::parse("").unwrap();
        assert!(config.outputs.is_empty());
    }

    #[test]
    fn config_with_multiple_output_blocks_parses() {
        let kdl = r#"
            output "eDP-1" { enabled = true }
            output "DP-1" { enabled = false }
        "#;
        let config = parse::parse(kdl).unwrap();
        assert_eq!(config.outputs.len(), 2);
        let dp1 = config.outputs.iter().find(|o| o.name == "DP-1").unwrap();
        assert!(!dp1.enabled);
    }

    #[test]
    fn output_unknown_key_errors() {
        let result = parse::parse(r#"output "DP-1" { zoom = 2 }"#);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown key"), "got: {msg}");
    }

    #[test]
    fn output_missing_name_errors() {
        // output block with no name argument
        let result = parse::parse("output {}");
        assert!(result.is_err());
    }

    #[test]
    fn validate_duplicate_output_name_errors() {
        let kdl = r#"
            output "DP-1" { enabled = false }
            output "DP-1" { enabled = true }
        "#;
        let result = parse::parse(kdl).unwrap();
        let err = validate(&result).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("DP-1"), "got: {msg}");
        assert!(msg.contains("duplicate"), "got: {msg}");
    }

    #[test]
    fn excluded_outputs_computed_from_config() {
        let kdl = r#"
            output "eDP-1" { enabled = true }
            output "DP-1" { enabled = false }
        "#;
        let config = parse::parse(kdl).unwrap();
        let excluded: std::collections::HashSet<String> = config
            .outputs
            .iter()
            .filter(|o| !o.enabled)
            .map(|o| o.name.clone())
            .collect();
        assert!(excluded.contains("DP-1"));
        assert!(!excluded.contains("eDP-1"));
    }

    #[test]
    fn output_not_in_config_is_implicitly_allowed() {
        // An output not mentioned in config is NOT excluded
        let kdl = r#"output "DP-1" { enabled = false }"#;
        let config = parse::parse(kdl).unwrap();
        let excluded: std::collections::HashSet<String> = config
            .outputs
            .iter()
            .filter(|o| !o.enabled)
            .map(|o| o.name.clone())
            .collect();
        // HDMI-1 is not mentioned → not excluded
        assert!(!excluded.contains("HDMI-1"));
    }
    ```

  - [ ] 5.2: Add `App::boot()` test for `pending_output_names` in `src/app.rs` tests:

    ```rust
    #[test]
    fn boot_with_empty_allowed_outputs_uses_allscreens_mode() {
        // Empty allowed_outputs → pending_output_names is empty (AllScreens handles creation)
        let config = make_config();
        let (app, _) = App::boot(config, vec![]);
        assert!(app.pending_output_names.is_empty());
    }
    ```

    Note: testing the `allowed_outputs` non-empty path requires an iced runtime context
    (the `layershell_open` call generates a Task that needs the iced runtime). The config
    parsing + exclusion-set logic is fully testable in unit tests without iced.

## Dev Notes

### iced_layershell API for manual window creation

`Message::layershell_open(settings: NewLayerShellSettings) -> (IcedId, Task<Self>)` is
generated by the `#[to_layer_message(multi)]` macro on the `Message` enum. It returns:
- `id: IcedId` — the iced window ID that will be assigned to this window
- A `Task` that dispatches `Message::NewLayerShell { settings, id }` to iced_layershell

Both `NewLayerShellSettings` and `OutputOption` are in `iced_layershell::reexport`.

`NewLayerShellSettings::default()` sets:
```
anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
layer: Layer::Top,
exclusive_zone: None,
size: None,
margin: Some((0, 0, 0, 0)),
keyboard_interactivity: KeyboardInteractivity::OnDemand,
```

Override `anchor`, `exclusive_zone`, `size`, `output_option`, and `namespace` from the config.

### `StartMode::Background` vs `StartMode::AllScreens`

`StartMode::Background` creates a background surface (visible but non-interactive background plane). For our use case with `allowed_outputs` non-empty, we override it with manual `NewLayerShell` tasks. The background surface from `Background` mode may or may not interfere — if it does, consider using `StartMode::Active` instead (creates one window on the active/focused screen, then we close it or it gets replaced by our manual windows). The safest option is to test `StartMode::Background` and see if it compiles and runs correctly with the manual windows.

**Alternative**: If `StartMode::Background` causes issues, the dev can instead use `StartMode::Active` (same default behavior as when `AllScreens` is not set), and accept that one extra window might be created on the currently active output. The `pending_output_names` map would have no entry for that ID and `SurfaceState::new("")` would be used — which is the same as the pre-Story-7.3 behavior for that window.

### niri-ipc synchronous query in `try_main()`

`try_main()` is a synchronous function called before the Tokio runtime is active (the runtime starts inside the iced `daemon().run()` call). The niri-ipc `Socket::connect()` and `socket.send()` are blocking operations — this is safe to call synchronously from `try_main()`.

Required imports in `main.rs`:
```rust
use std::collections::HashSet;
```
The `niri_ipc` crate is already a dependency; `niri_ipc::socket::Socket`, `niri_ipc::Request`, and `niri_ipc::Response` are the types needed.

### `pending_output_names` field access in tests

The field is private (`pending_output_names: HashMap<iced::window::Id, String>`). The test in Task 5.2 accesses it via `app.pending_output_names.is_empty()`. This requires either making the field `pub` or marking the test with `#[allow(dead_code)]` — use `pub` to keep it accessible for future stories (7.4 will need this map too).

### Impact on existing tests

Existing tests that call `App::boot(config)` will need to be updated to `App::boot(config, vec![])`. There are several in the test module of `src/app.rs`. Check all call sites:

```bash
grep -n "App::boot" src/app.rs
```

All calls use `make_config()` helper — update each to `App::boot(make_config(), vec![])`.

### `#[allow(dead_code)]` on `output_name`

In Story 7.2, `SurfaceState.output_name` was annotated with `#[allow(dead_code)]`. After Story 7.3, `output_name` WILL be set (from `pending_output_names`), but it may still not be READ in production code (reading is for Story 7.4). Keep the `#[allow(dead_code)]` annotation — it's intentional.

### Config syntax (for default config comment)

The default config (`config/default.kdl`) does NOT need to change — no `output` blocks means all outputs get a bar (existing behavior). The parse function already handles the empty case: no `output` nodes → `Vec::new()`.

### Clippy: No Issues Expected

Adding `OutputConfig`, the `outputs` field, and the `pending_output_names` field with `pub` visibility should not trigger clippy warnings. The `fetch_niri_outputs()` function uses `?`-chaining cleanly.

### Test Count Baseline

322 tests pass as of Story 7.2 completion. Task 5 adds 11 new tests → target ≥ 333.

## Previous Story Intelligence (Story 7.2)

- `SurfaceState.output_name` field is in `src/surface.rs`, annotated `#[allow(dead_code)]`
- `SurfaceState::new(output_name: String)` is the constructor — Story 7.3 will populate it
- `App::surface_mut(id)` uses `or_default()` (inserts empty SurfaceState) — Story 7.3 changes this
- Fixed 3 preexisting clippy errors in Story 7.2; watch for any that may have been introduced since
- `App::boot(config)` returns `(App, Task<Message>)` — signature changes to `boot(config, allowed_outputs)`
- The `make_config()` test helper is the canonical way to build test configs in `src/app.rs`
- All test calls to `App::boot(...)` use `make_config()` — ALL must be updated to `App::boot(make_config(), vec![])`

## Definition of Done

- [ ] `OutputConfig { name: String, enabled: bool }` struct added to `src/config/mod.rs`
- [ ] `Config.outputs: Vec<OutputConfig>` field added
- [ ] `output "name" { enabled = false }` KDL blocks parsed correctly
- [ ] Duplicate output name produces a `validate()` error
- [ ] `fetch_niri_outputs()` function in `src/main.rs` returns `Option<Vec<String>>`
- [ ] `try_main()` uses `StartMode::Background` + manual creation when exclusions configured and Niri available
- [ ] `try_main()` falls back to `StartMode::AllScreens` (with warning) when Niri unavailable
- [ ] `App::boot()` accepts `allowed_outputs: Vec<String>` second parameter
- [ ] All existing `App::boot(config)` call sites updated to `App::boot(config, vec![])`
- [ ] When `allowed_outputs` non-empty, `NewLayerShell` tasks created per output with `OutputOption::OutputName`
- [ ] `App.pending_output_names: HashMap<iced::window::Id, String>` tracked
- [ ] `App::surface_mut()` pops from `pending_output_names` when initializing a new `SurfaceState`
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo test` shows ≥ 333 tests passing

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-04-06 | 1.0 | Created story: output exclusion via config, manual per-output window creation | claude-sonnet-4-6 |
