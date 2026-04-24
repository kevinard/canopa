# Highest ROI Roadmap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce maintenance drag and extension cost first, while improving the command-security boundary and shrinking the app render hotspot without changing user-visible behavior.

**Architecture:** Execute three independent workstreams in ROI order. Workstream 1 removes duplicated module metadata and registration logic. Workstream 2 strengthens the centralized process-execution boundary while preserving compatibility for existing config. Workstream 3 moves bar-state derivation out of `App::view()` into typed render models so future UI work lands in small focused units instead of one large mixed-responsibility function.

**Tech Stack:** Rust 2024, `iced` 0.14, `iced_layershell` 0.17, `tokio`, `zbus`, `kdl`, `niri-ipc`.

---

## Scope

This roadmap intentionally covers three separate subsystems:

1. Module metadata unification
2. Command boundary hardening
3. Render-model cleanup

Each workstream must remain shippable on its own. Do not batch all three into one large PR.

## Verification Gate

Run after every task that compiles:

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

All three must pass before committing.

## File Structure

### Workstream 1: Module Metadata Unification

- Modify `src/modules.rs`: introduce a typed module descriptor table and remove scattered per-kind duplication.
- Modify `src/config.rs`: stop maintaining a second hardcoded module-type list.
- Modify `src/config/parse/mod.rs`: derive known-module behavior from the shared descriptor source.
- Modify `src/app.rs`: replace `default_params_for_type()` and constructor lookup with descriptor-driven boot logic.
- Modify module constructor callsites in `src/modules.rs`: move constructor/default-param wiring behind descriptors.
- Test in `src/modules.rs`, `src/config.rs`, and `src/app.rs`.

### Workstream 2: Command Boundary Hardening

- Modify `src/command.rs`: replace `Option<String>`/fire-and-forget shell helpers with richer typed execution results and explicit execution modes.
- Modify `src/modules/script.rs`: consume the richer result and preserve current user-visible failure behavior.
- Modify `src/modules/button.rs`: route clicks through the new execution API.
- Modify `src/app/module_events.rs`: keep volume adjustment on the non-shell `wpctl` path and align error handling with the new command result model.
- Modify parser/config files only if needed to support a future-safe internal representation without changing config syntax yet.
- Test primarily in `src/command.rs`, plus targeted integration tests in `src/modules/script.rs` and `src/modules/button.rs`.

### Workstream 3: Render-Model Cleanup

- Create `src/app/render.rs`: typed bar render model, popup placement model, and module render entries.
- Modify `src/app.rs`: register the new module.
- Modify `src/app/view.rs`: reduce it to rendering precomputed models and wiring widget events.
- Modify `src/app/niri.rs` or `src/app/module_runtime.rs` only if state derivation helpers need relocation.
- Test in `src/app/render.rs` and existing `src/app.rs`/`src/app/view.rs` tests.

---

## Workstream 1: Module Metadata Unification

### Objective

Remove the current five-way duplication around:

- config-known module names
- `ModuleKind`
- module constructor registry
- default synthesized params
- parser dispatch assumptions

Today that logic is split across `src/modules.rs`, `src/config.rs`, `src/config/parse/mod.rs`, and `src/app.rs`. The goal is that adding a new built-in module requires one descriptor entry plus the module implementation, not a sweep across unrelated files.

### Target Design

Add a descriptor type in `src/modules.rs`:

```rust
pub(crate) struct ModuleDescriptor {
    pub(crate) kind: ModuleKind,
    pub(crate) config_name: &'static str,
    pub(crate) constructor: ModuleConstructor,
    pub(crate) synthesized_params: Option<fn() -> crate::config::ModuleParams>,
}
```

Add one static list:

```rust
pub(crate) static MODULE_DESCRIPTORS: &[ModuleDescriptor] = &[/* one per built-in config module */];
```

Expose small query helpers:

```rust
pub(crate) fn descriptor_by_config_name(name: &str) -> Option<&'static ModuleDescriptor>;
pub(crate) fn config_module_kinds() -> &'static [ModuleKind];
pub(crate) fn known_config_module_names() -> &'static [&'static str];
```

Do not put dynamic-only kinds such as `WorkspaceItem` and `TrayIcon` into the config-facing descriptor list.

### Tasks

#### Task 1.1: Introduce typed descriptors in `src/modules.rs`

**Files:**
- Modify: `src/modules.rs`
- Test: `src/modules.rs`

- [ ] Add `ModuleDescriptor` and `MODULE_DESCRIPTORS`.
- [ ] Move constructor wiring behind descriptor entries.
- [ ] Replace ad hoc `build_registry()` with a descriptor-backed map or remove the map entirely if direct descriptor lookup is simpler.
- [ ] Add tests that prove:
  - every config-facing `ModuleKind` has exactly one descriptor
  - descriptor `config_name` round-trips through `ModuleKind`
  - dynamic-only kinds are excluded from config descriptors
- [ ] Verify with the full verification gate.
- [ ] Commit with:

```bash
rtk git add src/modules.rs
rtk git commit -m "refactor(modules): centralize built-in module descriptors"
```

#### Task 1.2: Replace duplicated config-known module lists

**Files:**
- Modify: `src/config.rs`
- Modify: `src/config/parse/mod.rs`
- Test: `src/config.rs`

- [ ] Remove `KNOWN_MODULE_TYPES` from `src/config.rs`.
- [ ] Replace `is_known_config_module_type()` with descriptor-backed lookup.
- [ ] Replace parser logic that infers `module_type` from a known module name with descriptor-backed lookup.
- [ ] Preserve current error messages and accepted config behavior.
- [ ] Add/adjust tests to prove:
  - default config still parses
  - known module names still infer type
  - unknown names still require explicit `type`
- [ ] Verify with the full verification gate.
- [ ] Commit with:

```bash
rtk git add src/config.rs src/config/parse/mod.rs
rtk git commit -m "refactor(config): derive known modules from shared descriptors"
```

#### Task 1.3: Replace synthesized-default boot logic

**Files:**
- Modify: `src/app.rs`
- Test: `src/app.rs`

- [ ] Remove `default_params_for_type()` from `src/app.rs`.
- [ ] During boot, resolve undeclared built-in modules through the shared descriptor and its optional synthesized params factory.
- [ ] Preserve current behavior:
  - known module names in bar sections can still be auto-instantiated
  - modules requiring mandatory params (`script`, `button`) are still skipped if undeclared
- [ ] Add tests for:
  - undeclared known built-ins still instantiate
  - undeclared mandatory-param modules still skip
  - constructor lookup still matches the expected module kind
- [ ] Verify with the full verification gate.
- [ ] Commit with:

```bash
rtk git add src/app.rs
rtk git commit -m "refactor(app): use module descriptors during boot"
```

### Risks

- The highest risk is accidental behavior drift in config parsing or boot-time synthesis.
- Keep descriptor helpers config-facing only. Do not leak tray/workspace runtime-only kinds into parser logic.

### PR Boundary

- PR 1 should contain only Workstream 1.
- No render refactors.
- No command-execution changes.

---

## Workstream 2: Command Boundary Hardening

### Objective

Keep `src/command.rs` as the single shell/process audit point, but make it more explicit and harder to misuse. The current interface collapses timeout and non-zero exit into the same `None`, and shell execution remains the default path for script/button actions.

The immediate goal is not to change config syntax. It is to make the internal API safer, better typed, and ready for a future argv-based config format.

### Target Design

Add typed execution results:

```rust
pub enum CaptureResult {
    Success(String),
    NonZero(std::process::ExitStatus),
    TimedOut,
}

pub enum SpawnResult {
    Spawned,
}
```

Add explicit execution modes:

```rust
pub enum CommandSpec<'a> {
    Shell(&'a str),
    Argv { program: &'a str, args: &'a [&'a str] },
}
```

Keep current user-facing config shell-based for `script.command` and `button.on_click`, but route them through `CommandSpec::Shell(...)` so the security boundary is explicit in code. Preserve `wpctl` volume changes on the argv path.

### Tasks

#### Task 2.1: Introduce typed results in `src/command.rs`

**Files:**
- Modify: `src/command.rs`
- Test: `src/command.rs`

- [ ] Replace `run_shell_capture_stdout(...)-> std::io::Result<Option<String>>` with a typed result.
- [ ] Preserve timeout kill-and-wait behavior.
- [ ] Preserve output byte caps and trimming behavior.
- [ ] Add tests that distinguish:
  - zero exit
  - non-zero exit
  - timeout
  - large-output success
- [ ] Verify with the full verification gate.
- [ ] Commit with:

```bash
rtk git add src/command.rs
rtk git commit -m "refactor(command): use typed capture results"
```

#### Task 2.2: Add explicit command specs and preserve existing behavior

**Files:**
- Modify: `src/command.rs`
- Modify: `src/modules/script.rs`
- Modify: `src/modules/button.rs`
- Test: `src/command.rs`
- Test: `src/modules/script.rs`
- Test: `src/modules/button.rs`

- [ ] Add `CommandSpec` with `Shell` and `Argv` variants.
- [ ] Add one internal runner for captured commands and one for fire-and-forget commands.
- [ ] Convert:
  - script polling to `CommandSpec::Shell`
  - button click actions to `CommandSpec::Shell`
  - keep volume on direct argv execution
- [ ] Update script-module handling so timeout and non-zero exit both remain user-visible as “retain previous output / emit Failed”, but are now distinguishable internally.
- [ ] Add tests proving shell mode still works and non-shell `wpctl` path remains direct argv.
- [ ] Verify with the full verification gate.
- [ ] Commit with:

```bash
rtk git add src/command.rs src/modules/script.rs src/modules/button.rs
rtk git commit -m "refactor(command): add explicit shell and argv execution modes"
```

#### Task 2.3: Audit callsites and document the boundary

**Files:**
- Modify: `src/app/module_events.rs`
- Modify: `src/command.rs`

- [ ] Verify every external process launch now goes through `src/command.rs`.
- [ ] Add a short module-level comment documenting which paths are shell-based by design and which are argv-only.
- [ ] Keep `wpctl` volume adjustment on the argv-only path.
- [ ] Verify with the full verification gate.
- [ ] Commit with:

```bash
rtk git add src/app/module_events.rs src/command.rs
rtk git commit -m "docs(command): document shell and argv execution boundaries"
```

### Risks

- Accidentally changing script/button semantics would be user-visible.
- Do not change config syntax in this workstream.
- Do not “improve” shell compatibility by broadening privileges or removing timeouts.

### PR Boundary

- PR 2 should contain only Workstream 2.
- It may depend on PR 1, but should not include render cleanup.

---

## Workstream 3: Render-Model Cleanup

### Objective

Reduce [src/app/view.rs](/home/kevinard/projects/niribar/src/app/view.rs:12) from a mixed state-derivation/render function into a thin renderer over typed render models. That gives you a clearer place to evolve visual behavior, and a smaller surface for UI regressions.

### Target Design

Create `src/app/render.rs` with focused data types:

```rust
pub(super) struct PopupRenderModel {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) tree: crate::surface::menu_popup::MenuTree,
    pub(super) bar_edge: crate::config::Edge,
    pub(super) bar_size: f32,
}

pub(super) struct ModuleRenderEntry {
    pub(super) id: crate::modules::ModuleId,
    pub(super) zone: crate::modules::Zone,
    pub(super) hovered: bool,
    pub(super) focused: bool,
    pub(super) element: iced::Element<'static, crate::app::Message>,
}

pub(super) struct BarRenderModel {
    pub(super) edge: crate::config::Edge,
    pub(super) content_size: u32,
    pub(super) top: Vec<iced::Element<'static, crate::app::Message>>,
    pub(super) center: Vec<iced::Element<'static, crate::app::Message>>,
    pub(super) bottom: Vec<iced::Element<'static, crate::app::Message>>,
}
```

Also add helpers:

```rust
pub(super) fn popup_render_model(&self, window: iced::window::Id) -> Option<PopupRenderModel>;
pub(super) fn bar_render_model(&self, window: iced::window::Id) -> BarRenderModel;
```

`App::view()` should only:

- record perf
- branch popup vs bar
- render the returned model

### Tasks

#### Task 3.1: Extract popup placement model

**Files:**
- Create: `src/app/render.rs`
- Modify: `src/app.rs`
- Modify: `src/app/view.rs`
- Test: `src/app/view.rs` or `src/app/render.rs`

- [x] Move popup anchor clamping and edge-relative placement calculations out of `App::view()`.
- [x] Build a `PopupRenderModel` from surface/menu/config state.
- [x] Keep current popup behavior unchanged.
- [x] Add tests for:
  - left/right/top/bottom edge placement data
  - anchor clamping when popup would overflow screen bounds
- [x] Verify with the full verification gate.
- [x] Commit with:

```bash
rtk git add src/app.rs src/app/view.rs src/app/render.rs
rtk git commit -m "refactor(app): extract popup render model"
```

#### Task 3.2: Extract bar module-entry derivation

**Files:**
- Modify: `src/app/render.rs`
- Modify: `src/app/view.rs`
- Test: `src/app/render.rs`

- [x] Move these responsibilities into `bar_render_model()`:
  - effective zone lookup
  - focused-window derived title selection per output
  - workspace-item derived state
  - hovered/focused decoration state
  - interaction wrapping per module
- [x] Leave final widget assembly in `view.rs` for this step.
- [x] Add tests for:
  - focused-window title selection on focused vs non-focused outputs
  - workspace-item state derivation per surface output
  - modules omitted when not present in the effective zone map
- [x] Verify with the full verification gate.
- [x] Commit with:

```bash
rtk git add src/app/render.rs src/app/view.rs
rtk git commit -m "refactor(app): extract bar render model derivation"
```

#### Task 3.3: Reduce `App::view()` to a thin renderer

**Files:**
- Modify: `src/app/view.rs`
- Test: `src/app.rs`
- Test: `src/app/view.rs`

- [x] Make `App::view()` only:
  - record perf
  - render popup from `PopupRenderModel`
  - render bar from `BarRenderModel`
- [x] Keep border-strip and top/center/bottom layout rendering in `view.rs`.
- [x] Do not move unrelated navigation, menu, or surface logic into this workstream.
- [x] Add or adjust smoke tests so the app still renders for a default surface and popup surface.
- [x] Verify with the full verification gate.
- [x] Commit with:

```bash
rtk git add src/app/view.rs src/app/render.rs
rtk git commit -m "refactor(app): make view a thin renderer"
```

### Risks

- Render-model extraction can accidentally change subtle behavior around focused-window titles or popup placement.
- Keep the model types internal to `app`; do not turn this into a cross-crate abstraction exercise.

### PR Boundary

- PR 3 should contain only Workstream 3.
- It can land after PR 1 and PR 2.
- Do not combine with dynamic-module redesign; that is separate future work.

---

## Suggested Execution Order

1. Workstream 1: Module metadata unification
2. Workstream 2: Command boundary hardening
3. Workstream 3: Render-model cleanup

That order front-loads the lowest-risk, highest-maintenance-return work; then the highest security leverage; then the largest local architecture cleanup.

## Suggested PR Sequence

1. `refactor(modules): centralize built-in module descriptors`
2. `refactor(config): derive known modules from shared descriptors`
3. `refactor(app): use module descriptors during boot`
4. `refactor(command): use typed capture results`
5. `refactor(command): add explicit shell and argv execution modes`
6. `docs(command): document shell and argv execution boundaries`
7. `refactor(app): extract popup render model`
8. `refactor(app): extract bar render model derivation`
9. `refactor(app): make view a thin renderer`

## Out of Scope

- Dynamic tray/workspace module redesign
- Config syntax changes for argv commands
- New built-in modules
- Menu-system redesign
- Any broader runtime-performance pass beyond the local effects of these changes

## Self-Review

- Spec coverage: the plan covers the three ROI items discussed with the user and keeps each independently shippable.
- Placeholder scan: no `TODO`/`TBD` placeholders remain.
- Type consistency: descriptor, command, and render-model concepts are named consistently across tasks.
