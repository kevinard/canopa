# Module Runtime Interface Deepening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deepen **Module Runtime** into the app-level interface for **Module Instance** behavior so App callers stop calling optional hooks on concrete **Module** implementations directly.

**Architecture:** **Module Runtime** becomes the external seam for **Module Instance** rendering, **Tooltip** values, **Module Data Source** application, **Module Interaction**, **Module Detail Session** routing, layout expansion, configured **Module Name** lookup, and dynamic child storage behavior. Concrete **Module** implementation hooks remain internal during migration. Query behavior stays method-shaped and may return Iced elements. Mutating behavior uses intent/effect handling.

**Tech Stack:** Rust 2024, iced, current Niribar `Module` implementation code, existing `rtk cargo test` / `rtk cargo clippy --all-targets -- -D warnings` / `rtk cargo build` verification gate.

---

## Scope

This plan changes the app-facing seam first and keeps behavior stable. It does not rewrite rendering into a view model and does not move DBus/source lifecycle into **Module Runtime**.

In scope:

- Add **Module Runtime** query methods for bar rendering, **Module Detail Content** rendering, **Tooltip** values, layout expansion, and configured **Module Name** lookup.
- Add **Module Runtime** intent/effect handling for **Module Interaction**, **Module Detail Session** updates, **Module Data Source** application, and dynamic child reconciliation.
- Migrate App callers away from direct concrete **Module** hooks.
- Move **Dynamic Module Reconciliation Plan** application from App into **Module Runtime**.
- Keep **Module Data Source Runtime** separate, while making **Module Runtime** map **Module Data Source State** into **Module Instance** render state.
- Shrink or split the internal concrete **Module** implementation interface only after the external seam is clean.

Out of scope:

- Replacing Iced elements with render models.
- Moving **Module Data Source Runtime** lifecycle, subscription ownership, or recovery policy into **Module Runtime**.
- Changing **Foreground** or **Panel Runtime** ownership of active **Panel** interaction state.
- Changing **Dynamic Module Lifecycle** planning semantics.
- Changing visual layout, style, or module behavior.
- Migrating **Configured Command** lifecycle; that remains a separate architecture candidate.

## Current Decisions

- **Module Runtime** is the external seam for **Module Instance** behavior.
- Query behavior stays method-shaped.
- Mutating behavior uses intent/effect handling.
- **Module Runtime** methods may return `iced::Element`.
- **Module Data Source Runtime** remains separate and owns source lifecycle, subscriptions, first emission, recovery, and unavailable transitions.
- **Module Runtime** owns applying source-shaped state to dependent **Module Instances**.
- **Dynamic Module Lifecycle** remains the pure planner.
- **Module Runtime** applies **Dynamic Module Reconciliation Plans** to durable child **Module Instance** storage.
- App applies only effects returned by **Module Runtime**, such as stale **Keyboard Focus** or hover cleanup.
- Do not split the concrete `Module` trait first. Add **Module Runtime** methods that delegate to current hooks, migrate callers, then shrink internal implementation hooks.

## File Structure

- Modify: `src/app/module_runtime.rs`
  - Add query methods and intent/effect types.
  - Move dynamic plan application into **Module Runtime**.
  - Keep concrete **Module** implementation hooks private to the runtime implementation.
- Modify: `src/app/render.rs`
  - Render bar and **Panel** module content through **Module Runtime** query methods.
- Modify: `src/app/surfaces.rs`
  - Resolve **Tooltip** values and configured **Module Name** through **Module Runtime**.
- Modify: `src/app/module_events.rs`
  - Route **Module Interaction**, **Module Detail Session** updates, and **Module Data Source** application through **Module Runtime** intent/effect handling.
- Modify: `src/app/module_data_sources.rs`
  - Return source lifecycle transitions instead of directly mapping dependent **Module Instances**.
- Modify: `src/app/dynamic_modules.rs`
  - Keep planning pure; remove App-owned storage application.
- Modify: `src/app/navigation.rs`, `src/app/panel.rs`, `src/app/subscriptions.rs`, `src/app/niri.rs`, `src/app/tray.rs` as needed where direct **Module Instance** storage/hook access remains.
- Modify later: `src/modules.rs`
  - Shrink or split the internal concrete **Module** implementation interface after App callers cross **Module Runtime**.
- Already updated: `CONTEXT.md`
  - Contains the **Module Runtime** and **Module Data Source Runtime** seam decisions.

---

## Slice 1: Add Module Runtime Render And Tooltip Queries

**Files:**
- Modify: `src/app/module_runtime.rs`
- Modify: `src/app/render.rs`
- Modify: `src/app/surfaces.rs`

- [ ] **Step 1: Add red tests for Module Runtime rendering queries**

Add tests in `src/app/module_runtime.rs` that create a small runtime with a test **Module Instance** and assert:

- `render_bar_module` uses the current state or `ModuleState::Empty` fallback.
- `render_panel_detail` uses a provided **Module Detail Session** when present.
- `tooltip_values_for` returns the same values currently produced by the concrete module hook.
- missing ids return an explicit empty/missing outcome instead of panicking.

Expected initial result: tests fail to compile because the query methods and request/result types do not exist.

- [ ] **Step 2: Add query request/result types**

Add focused request types near `ModuleRuntime`:

```rust
pub(super) struct BarModuleRenderRequest<'a> {
    pub id: &'a ModuleId,
    pub window: iced::window::Id,
    pub horizontal: bool,
    pub expanded: bool,
    pub style: &'a crate::config::Style,
}

pub(super) struct ModuleDetailRenderRequest<'a> {
    pub id: &'a ModuleId,
    pub style: &'a crate::config::Style,
    pub session: Option<&'a crate::modules::ModuleDetailSession>,
}
```

Use result shapes that let callers distinguish "not found" from "rendered empty" without reaching into storage.

- [ ] **Step 3: Implement query methods by delegation**

Add methods that delegate to the current concrete `Module` hooks:

```rust
impl ModuleRuntime {
    pub(super) fn render_bar_module<'a>(
        &'a self,
        request: BarModuleRenderRequest<'a>,
    ) -> iced::Element<'a, super::Message> { ... }

    pub(super) fn render_module_detail<'a>(
        &'a self,
        request: ModuleDetailRenderRequest<'a>,
    ) -> iced::Element<'a, super::Message> { ... }

    pub(super) fn tooltip_values_for(
        &self,
        id: &ModuleId,
    ) -> Vec<(&'static str, String)> { ... }
}
```

Keep the default state lookup local to **Module Runtime**.

- [ ] **Step 4: Migrate render callers**

In `src/app/render.rs`, replace direct calls to:

- `self.modules.get(...)`
- `self.modules.state(...)`
- `module.view_for_bar(...)`
- `module.view_module_detail_session_with_style(...)`

with **Module Runtime** query methods.

Keep **Foreground** / **Panel Runtime** responsible for identifying the active **Panel** and session; pass session references to **Module Runtime** for rendering only.

- [ ] **Step 5: Migrate Tooltip value callers**

In `src/app/surfaces.rs`, replace direct concrete hook calls in `tooltip_values_for` with `self.modules.tooltip_values_for(module)`.

Do not move passive **Tooltip** window ownership out of **Bar Surface Registry** in this slice.

- [ ] **Step 6: Run focused tests**

Run:

```bash
rtk cargo test app::module_runtime::tests app::render::tests app::surfaces::tests -- --nocapture
```

Expected: pass after caller migration.

---

## Slice 2: Move Configured Module Name And Layout Queries Behind Module Runtime

**Files:**
- Modify: `src/app/module_runtime.rs`
- Modify: `src/app/dynamic_modules.rs`
- Modify: `src/app/navigation.rs`
- Modify: `src/app/render.rs`
- Modify: `src/app/surfaces.rs`
- Modify: `src/app/panel.rs`

- [ ] **Step 1: Add tests for configured Module Name lookup**

Add **Module Runtime** tests that cover:

- configured **Module Instance** returns its own **Module Name**.
- tray child **Module Instance** returns the parent tray **Module Name**.
- workspace child **Module Instance** returns the parent workspaces **Module Name**.

Expected initial result: fail until lookup is exposed through **Module Runtime**.

- [ ] **Step 2: Add Module Runtime lookup and layout query methods**

Add:

```rust
impl ModuleRuntime {
    pub(super) fn configured_module_name_for_instance(&self, id: &ModuleId) -> String { ... }

    pub(super) fn expand_dynamic_module_layout(
        &self,
        placements: [(Zone, &[String]); 3],
    ) -> Vec<(Zone, ModuleId)> { ... }
}
```

These may delegate to the existing **Dynamic Module Runtime Snapshot** implementation.

- [ ] **Step 3: Migrate callers**

Move App callers from `App::configured_module_name_for_instance` and `App::expand_dynamic_module_layout` to `self.modules...`.

Likely callers:

- `src/app/navigation.rs`
- `src/app/render.rs`
- `src/app/surfaces.rs`
- `src/app/panel.rs`
- `src/app/module_events.rs`
- `src/app/tray.rs`

- [ ] **Step 4: Remove shallow App pass-through helpers**

Delete or narrow App helper methods that only delegate to **Dynamic Module Runtime Snapshot** through `self.modules`.

- [ ] **Step 5: Run layout and dynamic module tests**

Run:

```bash
rtk cargo test app::dynamic_modules::tests app::module_runtime::tests app::render::tests app::surfaces::tests app::navigation::tests -- --nocapture
```

Expected: pass.

---

## Slice 3: Route Module Data Source Application Through Module Runtime

**Files:**
- Modify: `src/app/module_data_sources.rs`
- Modify: `src/app/module_runtime.rs`
- Modify: `src/app/module_events.rs`
- Modify: `src/modules/data_source.rs`

- [ ] **Step 1: Add source transition type**

In `src/modules/data_source.rs` or `src/app/module_data_sources.rs`, add a type that represents lifecycle output before **Module Runtime** maps it:

```rust
pub(super) enum ModuleDataSourceTransition {
    Available(crate::modules::data_source::ModuleDataSourceState),
    Unavailable(crate::modules::data_source::ModuleDataSourceIdentity),
}
```

Keep source lifecycle state in **Module Data Source Runtime**.

- [ ] **Step 2: Add red tests for Module Runtime source application**

Add tests in `src/app/module_runtime.rs` for:

- applying battery source state updates all dependent battery **Module Instances**.
- marking a source unavailable marks only dependent **Module Instances** unavailable.
- recovered available state remaps dependents.
- non-dependent **Module Instances** are untouched.

Expected initial result: fail until **Module Runtime** owns source-state mapping.

- [ ] **Step 3: Add Module Runtime intent/effect handling**

Add intent/effect variants:

```rust
pub(super) enum ModuleRuntimeIntent {
    ApplyDataSourceState(crate::modules::data_source::ModuleDataSourceState),
    MarkDataSourceUnavailable(crate::modules::data_source::ModuleDataSourceIdentity),
    // later variants in later slices
}

pub(super) enum ModuleRuntimeEffect {
    // initially empty or diagnostic-only if no app side effect is required
}
```

Implement mapping by delegating to the existing concrete `Module::map_data_source_state` hook internally.

- [ ] **Step 4: Change Module Data Source Runtime handlers**

Change `handle_battery_event`, `handle_network_event`, and `handle_volume_event` to update source lifecycle and return `ModuleDataSourceTransition`, not `(ModuleId, ModuleInstanceOutcome)` pairs.

- [ ] **Step 5: Migrate module event handlers**

In `src/app/module_events.rs`, route returned transitions into `self.modules.handle(...)`.

App should no longer call `self.modules.apply_outcomes(...)` from source lifecycle code.

- [ ] **Step 6: Run data source tests**

Run:

```bash
rtk cargo test app::module_data_sources::tests app::module_runtime::tests modules::data_source -- --nocapture
```

Expected: pass.

---

## Slice 4: Route Module Interaction Through Module Runtime

**Files:**
- Modify: `src/app/module_runtime.rs`
- Modify: `src/app/module_events.rs`
- Modify: `src/app/widget_ops.rs` only if target construction needs a helper

- [ ] **Step 1: Add red interaction tests at the Module Runtime seam**

Add **Module Runtime** tests for:

- configured click **Module Action** produces a **Panel Action** effect.
- configured action needing an anchor produces an anchor-request effect instead of resolving bounds directly.
- module-owned interaction produces an app message effect.
- unsupported interaction produces no effect.

Expected initial result: fail until interaction intent/effect variants exist.

- [ ] **Step 2: Add interaction intent/effect variants**

Add variants similar to:

```rust
pub(super) enum ModuleRuntimeIntent {
    ModuleInteraction {
        id: ModuleId,
        event: crate::modules::InteractionEvent,
        origin_window: iced::window::Id,
    },
    // existing source variants
}

pub(super) enum ModuleRuntimeEffect {
    RunPanelAction {
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        action: crate::config::PanelAction,
    },
    RequestActionAnchor {
        origin_window: iced::window::Id,
        module: ModuleId,
        action: crate::config::PanelAction,
    },
    EmitMessage(super::Message),
    ClearSurfaceModuleRefs(Vec<ModuleId>),
}
```

Keep Iced widget operations in App. **Module Runtime** should request anchor resolution; App remains the adapter that performs `iced::advanced::widget::operate`.

- [ ] **Step 3: Move configured action lookup into Module Runtime**

Move trigger lookup, configured **Module Action** precedence, and module-owned interaction fallback out of `src/app/module_events.rs` into **Module Runtime**.

Keep `panel_action_needs_anchor` as-is unless the call naturally moves with the routing logic.

- [ ] **Step 4: Add App effect adapter**

Add an App helper such as:

```rust
fn apply_module_runtime_effects(
    &mut self,
    effects: Vec<super::module_runtime::ModuleRuntimeEffect>,
) -> Task<Message>
```

Handle `RequestActionAnchor` by performing the existing `FindBounds` operation and converting the result into the current `PanelEvent`.

- [ ] **Step 5: Migrate `dispatch_module_interaction`**

`dispatch_module_interaction` should build a `ModuleRuntimeIntent::ModuleInteraction`, then apply returned **Module Runtime** effects.

- [ ] **Step 6: Run interaction tests**

Run:

```bash
rtk cargo test app::module_runtime::tests app::module_events::tests -- --nocapture
```

Expected: pass.

---

## Slice 5: Route Module Detail Session Updates Through Module Runtime

**Files:**
- Modify: `src/app/module_runtime.rs`
- Modify: `src/app/module_events.rs`
- Modify: `src/app/panel.rs`
- Modify: `src/app/render.rs`

- [ ] **Step 1: Add red Module Detail Session tests**

Add tests proving:

- **Module Runtime** creates a **Module Detail Session** for a module-originated **Panel** when the concrete module supports it.
- **Module Runtime** renders **Module Detail Content** with the provided session.
- **Module Runtime** handles a **Module Detail Session** update and returns app-owned effects.
- missing module or wrong session type is ignored safely.

Expected initial result: fail until create/update routing methods exist.

- [ ] **Step 2: Add Module Detail query and intent shapes**

Add query methods for session creation and rendering:

```rust
pub(super) fn create_module_detail_session(
    &self,
    id: &ModuleId,
) -> Option<crate::modules::ModuleDetailSession>;
```

Add a mutating intent for session update. Use lifetimes if the intent borrows the session:

```rust
pub(super) enum ModuleRuntimeIntent<'a> {
    ModuleDetailUpdate {
        target: ModuleId,
        session: &'a mut crate::modules::ModuleDetailSession,
        message: crate::modules::ModuleDetailMessage,
    },
    // existing variants
}
```

Map `ModuleDetailEffect::App` into **Module Runtime** effects and keep module-owned effects internal until a real adapter needs them.

- [ ] **Step 3: Migrate session creation**

In `src/app/panel.rs`, replace direct module lookup and `create_module_detail_session` calls with **Module Runtime** methods.

Keep session storage in **Panel Runtime** / **Foreground** for this plan.

- [ ] **Step 4: Migrate session rendering**

In `src/app/render.rs`, use **Module Runtime** to render **Module Detail Content** with the session provided by **Foreground**.

- [ ] **Step 5: Migrate session update**

In `src/app/module_events.rs`, replace direct concrete module mutation and `update_module_detail_session` calls with the **Module Runtime** intent.

- [ ] **Step 6: Run Module Detail tests**

Run:

```bash
rtk cargo test app::module_runtime::tests app::module_events::tests app::panel::tests app::render::tests modules::clock::tests -- --nocapture
```

Expected: pass.

---

## Slice 6: Move Dynamic Module Plan Application Into Module Runtime

**Files:**
- Modify: `src/app/module_runtime.rs`
- Modify: `src/app/dynamic_modules.rs`
- Modify: `src/app/niri.rs`
- Modify: `src/app/tray.rs`

- [ ] **Step 1: Add red dynamic reconciliation application tests**

Move or duplicate enough existing dynamic tests to **Module Runtime** to assert:

- child removal prunes states/actions/origins/interactions.
- workspace child update happens in place.
- tray child rebuild preserves inherited **Module Actions** and interaction profile.
- stale child ids are returned as an effect for **Keyboard Focus** and hover cleanup.

Expected initial result: fail until **Module Runtime** owns plan application.

- [ ] **Step 2: Add dynamic reconciliation intent/effect**

Add:

```rust
pub(super) enum ModuleRuntimeIntent {
    ApplyDynamicModuleReconciliation(
        super::dynamic_modules::DynamicModuleReconciliationPlan,
    ),
    // existing variants
}
```

Return:

```rust
ModuleRuntimeEffect::ClearSurfaceModuleRefs(Vec<ModuleId>)
```

when stale child ids need cleanup outside **Module Runtime**.

- [ ] **Step 3: Move storage mutation from App to Module Runtime**

Move the implementation currently in `App::apply_dynamic_module_reconciliation_plan` into **Module Runtime**.

This includes:

- taking existing dynamic children.
- updating workspace children in place.
- inserting children with inherited actions, origin, and interaction profile.
- pruning metadata.

- [ ] **Step 4: Keep Dynamic Module Lifecycle pure**

`src/app/dynamic_modules.rs` should keep plan construction and layout expansion logic pure. It should not mutate **Module Runtime** directly.

- [ ] **Step 5: Migrate App dynamic callers**

In `reconcile_workspace_dynamic_modules` and `reconcile_tray_dynamic_modules`, App should:

1. build a **Dynamic Module Runtime Snapshot** from `self.modules`.
2. ask **Dynamic Module Lifecycle** for a plan.
3. pass the plan to `self.modules.handle(...)`.
4. apply returned **Module Runtime** effects.

- [ ] **Step 6: Run dynamic tests**

Run:

```bash
rtk cargo test app::dynamic_modules::tests app::module_runtime::tests app::niri::tests app::tray::tests -- --nocapture
```

Expected: pass.

---

## Slice 7: Shrink The Internal Concrete Module Interface

**Files:**
- Modify: `src/modules.rs`
- Modify: `src/modules/*.rs`
- Modify tests in concrete module files as needed

- [ ] **Step 1: Scan for direct concrete Module hook calls outside Module Runtime**

Run:

```bash
rtk rg -n "\\.view_|view_for_bar|tooltip_values|data_source_identity|map_data_source_state|on_interaction|create_module_detail_session|update_module_detail_session|update_workspace|config_name\\(" src/app src/modules.rs src/modules
```

Expected: direct calls from `src/app` should be gone or confined to **Module Runtime** implementation/tests.

- [ ] **Step 2: Decide the smallest internal split**

Only split the concrete implementation interface where it reduces real optional-hook width. Candidate internal traits:

- identity and kind.
- rendering.
- **Tooltip** values.
- **Module Data Source** mapping.
- **Module Interaction**.
- **Module Detail Session**.
- dynamic child update.

Do not expose these as App-facing seams.

- [ ] **Step 3: Split one internal behavior at a time**

Start with the least risky split:

1. **Tooltip** values.
2. **Module Data Source** mapping.
3. **Module Detail Session**.
4. dynamic child update.
5. rendering last, because it touches the most files.

Each split should keep **Module Runtime** tests green and should delete obsolete tests that now test past the **Module Runtime** interface.

- [ ] **Step 4: Update docs if terms sharpen**

If implementation introduces a stable term not already in `CONTEXT.md`, update `CONTEXT.md` in the same change.

- [ ] **Step 5: Run full verification**

Run:

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

Expected: all pass.

---

## Regression Checklist

- [ ] Bar rendering still handles collapsed, horizontal, expanded, and output-sensitive workspace item rendering.
- [ ] **Tooltip** rendering still uses dynamic child parent **Module Name** settings.
- [ ] **Module Data Source** recovery remaps all dependent **Module Instances**.
- [ ] Configured **Module Actions** still take precedence over module-owned interactions.
- [ ] Anchor-requiring **Panel Actions** still use the originating **Module** widget bounds.
- [ ] **Module Detail Session** state remains reset on each **Panel** open.
- [ ] Clock **Module Detail Content** month browsing still works.
- [ ] Dynamic workspace child update remains in-place.
- [ ] Dynamic tray child rebuild remains deterministic.
- [ ] Stale **Keyboard Focus** and hover cleanup still happen when dynamic children disappear.

## Final Verification

Run:

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
rtk git status --short
```

Expected:

- tests pass.
- clippy passes with warnings denied.
- build passes.
- git status shows only intentional implementation and documentation changes.
