# Internal Module Render Interface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the internal concrete **Module** implementation interface after **Module Runtime** is the only app-facing seam, starting with Iced-shaped **Module Instance** rendering and **Tooltip** value projection.

**Architecture:** **Module Runtime** remains the external seam for App callers. The internal concrete **Module** implementation interface is split only after App no longer reads concrete **Module** hooks, raw **Module Instance** storage, or per-instance state directly. The first split isolates rendering behind an internal render adapter. Collapsed and panel rendering are required render forms. Horizontal, **Module Detail Content**, **Module Detail Session** rendering, and **Tooltip** values are handled through explicit render behavior, with fallback selection centralized in the adapter or **Module Runtime**.

**Tech Stack:** Rust 2024, iced, current Niribar `Module` implementations, existing `rtk cargo test` / `rtk cargo clippy --all-targets -- -D warnings` / `rtk cargo build` verification gate.

---

## Scope

This plan is intentionally gated. Do not start the rendering split until the **Module Runtime** app-facing seam is clean.

In scope:

- Verify no App caller bypasses **Module Runtime** through concrete **Module** hooks, raw storage iteration, indexing, or direct per-**Module Instance** state reads/mutations.
- Add an internal rendering interface for concrete **Module** implementations.
- Keep the first rendering split Iced-element-shaped.
- Include bar rendering, panel rendering, **Module Detail Content** rendering, existing **Module Detail Session** rendering, and **Tooltip** value projection.
- Centralize rendering fallback selection in an internal render adapter or **Module Runtime**.
- Remove rendering methods from the broad concrete `Module` trait after concrete implementations have moved.

Out of scope:

- Replacing Iced elements with render models.
- Splitting source mapping, **Module Interaction**, **Module Detail Session** creation/update, subscriptions, or dynamic-child behavior in this plan.
- Changing public **Module Runtime** query or intent/effect behavior.
- Changing visual layout, style, **Tooltip** ownership, or **Foreground** ownership.
- Moving **Module Data Source Runtime** lifecycle into **Module Runtime**.

## Current Decisions

- **Module Runtime** is the app-facing seam for **Module Instance** rendering and **Tooltip** values.
- App callers must not call concrete **Module** hooks directly.
- App callers must not iterate or index raw **Module Instance** storage.
- App callers must not directly read or mutate per-**Module Instance** state.
- The internal rendering split happens only after those bypasses are gone.
- The first internal split is rendering plus **Tooltip** value projection.
- The first internal split remains Iced-element-shaped.
- **Module Detail Content** rendering is part of the first rendering split.
- Creating and updating a **Module Detail Session** stay in later detail-session behavior.
- Fallback selection is centralized in the adapter or **Module Runtime**, not recreated as broad default methods on another concrete **Module** implementation trait.
- Separate render forms are preferred over one mode-enum render method.
- Collapsed and panel rendering are required forms.

## File Structure

- Modify: `crates/niribar/src/modules.rs`
  - Narrow the broad concrete `Module` trait after rendering behavior moves out.
  - Make the broad trait depend on the new internal render interface while migration is in progress.
- Add or modify: `crates/niribar/src/modules/render.rs`
  - Define the internal concrete render implementation interface.
  - Keep collapsed and panel render forms required.
  - Keep optional render forms explicit; do not encode fallback behavior here.
- Modify: `crates/niribar/src/app/module_runtime.rs`
  - Add or use an internal render adapter for fallback selection.
  - Keep App-facing render query methods unchanged.
  - Route **Tooltip** value projection through the render adapter.
- Modify: `src/modules/*.rs`
  - Move concrete rendering methods to the new internal render interface.
- Modify tests in: `crates/niribar/src/app/module_runtime.rs`, `crates/niribar/src/modules.rs`, and concrete `src/modules/*` test modules as needed.
- Already updated: `CONTEXT.md`
  - Contains the gate and first-split decisions.

---

## Slice 0: Enforce The Gate

**Files:**
- Read: `src/app/*.rs`
- Modify only if needed: files still bypassing **Module Runtime**

- [ ] **Step 1: Audit App callers for concrete Module bypasses**

Run:

```bash
rtk rg -n "modules\\.(get|get_mut|iter|state|set_state|ids_by_kind)|for .*modules|&self\\.modules|self\\.modules\\[" crates/niribar/src/app
```

Expected result before starting this plan: every match is either inside `crates/niribar/src/app/module_runtime.rs`, inside tests that should be migrated, or is an allowed **Module Runtime** query/intent call. If production App code still uses raw storage or direct state access, stop this plan and finish the **Module Runtime** interface deepening first.

- [ ] **Step 2: Remove remaining production bypasses**

If the audit finds production bypasses outside `crates/niribar/src/app/module_runtime.rs`, add or use focused **Module Runtime** methods instead.

Likely replacements:

- direct state reads -> **Module Runtime** render/query method
- direct state writes -> **Module Runtime** intent
- concrete hook calls -> **Module Runtime** query or intent
- storage iteration -> narrow **Module Runtime** snapshot or query

- [ ] **Step 3: Narrow test bypasses**

Migrate tests that construct large fake `Module` implementations only to inspect storage or state. Prefer testing through **Module Runtime** queries and intents.

- [ ] **Step 4: Re-run the audit**

Run the same `rtk rg` command from Step 1. The split may continue only when remaining matches are intentional and local to **Module Runtime** implementation or tests.

- [ ] **Step 5: Run the focused gate tests**

Run:

```bash
rtk cargo test app::module_runtime::tests app::render::tests app::module_events::tests app::dynamic_modules::tests -- --nocapture
```

Expected: pass before changing the internal render interface.

---

## Slice 1: Add The Internal Render Interface

**Files:**
- Add or modify: `crates/niribar/src/modules/render.rs`
- Modify: `crates/niribar/src/modules.rs`
- Modify: `crates/niribar/src/app/module_runtime.rs`

- [ ] **Step 1: Add red Module Runtime render-adapter tests**

Add tests in `crates/niribar/src/app/module_runtime.rs` using a small test **Module Instance** with distinguishable render outputs or markers. Assert:

- bar rendering uses collapsed render for collapsed vertical bars.
- bar rendering uses horizontal render when provided.
- bar rendering falls back to panel render when horizontal render is absent.
- panel rendering uses panel render.
- **Module Detail Content** rendering uses detail render when provided.
- **Module Detail Content** rendering falls back to panel render when detail render is absent.
- existing **Module Detail Session** rendering uses session render when provided.
- existing **Module Detail Session** rendering falls back to detail render, then panel render.
- **Tooltip** values are returned through the internal render path.

Expected initial result: fail to compile because the render interface and adapter do not exist.

- [ ] **Step 2: Define the internal render interface**

Create or update `crates/niribar/src/modules/render.rs` with an internal trait shaped around separate render forms.

Required forms:

```rust
pub trait ModuleRender: Send + Sync {
    fn render_collapsed_with_style(
        &self,
        state: &crate::modules::ModuleState,
        style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message>;

    fn render_panel_with_style(
        &self,
        state: &crate::modules::ModuleState,
        style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message>;
}
```

Add explicit optional forms without fallback behavior:

```rust
fn render_horizontal_with_style(...) -> Option<iced::Element<'_, crate::app::Message>>;
fn render_detail_with_style(...) -> Option<iced::Element<'_, crate::app::Message>>;
fn render_detail_session_with_style(...) -> Option<iced::Element<'_, crate::app::Message>>;
fn tooltip_values(...) -> Vec<(&'static str, String)>;
```

Do not put horizontal/detail/detail-session fallback selection in this trait. If temporary default methods are needed for a tracer-bullet compile, remove them by Slice 4.

- [ ] **Step 3: Make the broad Module trait depend on the render interface**

In `crates/niribar/src/modules.rs`, make the current broad concrete `Module` trait extend the new render interface:

```rust
pub trait Module: render::ModuleRender + Send + Sync {
    ...
}
```

Keep non-render behavior on `Module` for now. This preserves **Module Runtime** storage while allowing render behavior to move first.

- [ ] **Step 4: Add the internal render adapter**

In `crates/niribar/src/app/module_runtime.rs`, add a private render adapter or helper functions that centralize fallback selection:

- horizontal -> horizontal if present, else panel
- detail -> detail if present, else panel
- detail session -> session if present, else detail, else panel
- **Tooltip** values -> concrete render implementation values

The App-facing `render_bar_module`, `render_module_detail`, and `tooltip_values_for` methods should keep their existing interface.

- [ ] **Step 5: Verify Slice 1**

Run:

```bash
rtk cargo test app::module_runtime::tests -- --nocapture
```

Expected: tests compile and pass with current concrete modules still rendering the same UI.

---

## Slice 2: Move Concrete Module Rendering

**Files:**
- Modify: `crates/niribar/src/modules/battery.rs`
- Modify: `crates/niribar/src/modules/button.rs`
- Modify: `crates/niribar/src/modules/clock.rs`
- Modify: `crates/niribar/src/modules/focused_window.rs`
- Modify: `crates/niribar/src/modules/network.rs`
- Modify: `crates/niribar/src/modules/script.rs`
- Modify: `crates/niribar/src/modules/tray.rs`
- Modify: `crates/niribar/src/modules/volume.rs`
- Modify: `crates/niribar/src/modules/workspaces.rs`
- Modify: `crates/niribar/src/modules.rs`

- [ ] **Step 1: Move one simple Module first**

Start with the simplest concrete **Module Kind**, such as button or clock.

Move rendering methods from its `impl Module` block into `impl ModuleRender`.

Keep behavior stable:

- collapsed render output unchanged.
- panel render output unchanged.
- horizontal override unchanged if it had one.
- detail render unchanged if it had one.
- **Tooltip** values unchanged if it had any.

- [ ] **Step 2: Run focused tests**

Run tests for the migrated module and **Module Runtime** rendering:

```bash
rtk cargo test modules::button app::module_runtime::tests -- --nocapture
```

Adjust the module name in the command to match the first migrated module.

- [ ] **Step 3: Migrate remaining concrete modules**

Move render behavior module by module. Keep each move mechanical and avoid changing output.

Suggested order:

1. button
2. clock
3. battery
4. network
5. volume
6. script
7. focused-window
8. workspaces and workspace item
9. tray and tray icon

- [ ] **Step 4: Verify all module render migrations**

Run:

```bash
rtk cargo test modules:: app::module_runtime::tests app::render::tests -- --nocapture
```

Expected: pass.

---

## Slice 3: Remove Rendering From The Broad Module Trait

**Files:**
- Modify: `crates/niribar/src/modules.rs`
- Modify: `crates/niribar/src/app/module_runtime.rs`
- Modify tests with fake modules

- [ ] **Step 1: Remove render methods from `Module`**

Remove these from the broad concrete `Module` trait:

- `view_collapsed_with_style`
- `view_panel_with_style`
- `view_module_detail_with_style`
- `view_module_detail_session_with_style`
- `view_horizontal_with_style`
- `view_for_bar`
- `tooltip_values`

They should now live behind `ModuleRender` plus the internal render adapter.

- [ ] **Step 2: Fix fake modules in tests**

Update test modules to implement `ModuleRender` separately from `Module`.

The expected payoff is visible here: tests that only need source, interaction, detail-session, or dynamic-child behavior should no longer need to provide render methods unless they cross a rendering test surface.

- [ ] **Step 3: Keep fallback selection out of the trait**

Search for broad default render fallback methods:

```bash
rtk rg -n "fn view_for_bar|fn render_.*\\{\\s*self\\.render_|unwrap_or_else\\(\\|\\| self\\.render" crates/niribar/src/modules.rs crates/niribar/src/modules
```

Expected: no broad render fallback selection remains on the concrete implementation trait. Fallback selection should be local to the render adapter or **Module Runtime**.

- [ ] **Step 4: Verify Slice 3**

Run:

```bash
rtk cargo test app::module_runtime::tests modules:: -- --nocapture
```

Expected: pass.

---

## Slice 4: Tighten The Module Runtime Storage Interface

**Files:**
- Modify: `crates/niribar/src/app/module_runtime.rs`
- Modify: tests as needed

- [ ] **Step 1: Remove or restrict raw storage escape hatches**

Remove or make private any methods that are no longer needed outside **Module Runtime**:

- `get`
- `get_mut`
- `iter`
- `IntoIterator for &ModuleRuntime`
- `IntoIterator for &mut ModuleRuntime`
- `Index<usize> for ModuleRuntime`
- direct state methods that are not part of a deliberate **Module Runtime** query or intent

If tests still need these, prefer a narrow test-only query over raw storage exposure.

- [ ] **Step 2: Re-run the bypass audit**

Run:

```bash
rtk rg -n "modules\\.(get|get_mut|iter|state|set_state|ids_by_kind)|for .*modules|&self\\.modules|self\\.modules\\[" crates/niribar/src/app
```

Expected: no production bypasses outside `crates/niribar/src/app/module_runtime.rs`.

- [ ] **Step 3: Verify Slice 4**

Run:

```bash
rtk cargo test app::module_runtime::tests app::render::tests app::surfaces::tests app::module_events::tests -- --nocapture
```

Expected: pass.

---

## Slice 5: Full Verification

**Files:**
- No additional file changes expected.

- [ ] **Step 1: Run full tests**

Run:

```bash
rtk cargo test
```

- [ ] **Step 2: Run clippy**

Run:

```bash
rtk cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 3: Run build**

Run:

```bash
rtk cargo build
```

- [ ] **Step 4: Update documentation if the implementation shape changed**

If the final split differs from this plan, update `CONTEXT.md` with the new durable decision. Do not update `CONTEXT.md` for temporary implementation details.

---

## Stop Conditions

Stop and return to **Module Runtime** interface deepening if:

- App production code still bypasses **Module Runtime** after Slice 0.
- The rendering split requires changing the App-facing **Module Runtime** interface.
- The split starts to require a render model.
- Source mapping, interaction, detail-session creation/update, or dynamic-child behavior must be split first to make rendering compile.

Those are signs the gate was not actually satisfied or the planned first split is no longer the right first move.
