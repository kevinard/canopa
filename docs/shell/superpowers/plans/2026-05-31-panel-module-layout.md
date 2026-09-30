# Panel Module Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Treat **Bar Panel** and **Module Panel** as one module-backed **Panel** path with independent **Module Detail Sessions** per contained **Module**.

**Architecture:** Add a resolved `PanelModuleEntry` layout used by rendering, session creation, and detail-message routing. Change `PanelRuntime` from one optional `ModuleDetailSession` to a `HashMap<ModuleId, ModuleDetailSession>`, while keeping menu and dmenu behavior unchanged.

**Tech Stack:** Rust, iced, existing `cargo test` suite.

---

## File Structure

- Modify: `crates/niribar/src/surface/panel.rs`
  - Add `PanelModuleEntry` as the resolved module-backed **Panel** layout entry.
- Modify: `crates/niribar/src/app/panel.rs`
  - Resolve `PanelContent` into `Vec<PanelModuleEntry>`.
  - Create one detail session per resolved module.
- Modify: `crates/niribar/src/app/panel_runtime.rs`
  - Store detail sessions by `ModuleId`.
  - Expose immutable and mutable session lookup by `ModuleId`.
  - Expose active module-backed target checks through the active content.
- Modify: `crates/niribar/src/app/render.rs`
  - Render both `BarPanel` and `ModulePanel` from the shared resolved layout.
  - Pass each module its own detail session.
- Modify: `crates/niribar/src/app/module_events.rs`
  - Route detail messages through the active module-backed target/session lookup.
- Modify: `docs/niribar/superpowers/specs/2026-05-31-panel-module-layout-design.md`
  - Already added as the approved design.

## Task 1: Red Tests For Per-Module Detail Sessions

**Files:**
- Modify: `crates/niribar/src/app/module_events.rs`
- Modify: `crates/niribar/src/app/render.rs`

- [ ] **Step 1: Add failing detail-routing test**

In `crates/niribar/src/app/module_events.rs`, add this test inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn bar_panel_routes_detail_messages_to_independent_module_sessions() {
    let mut app = app_with_detail_modules(&["first", "second"]);
    app.config.bars[0].panel.modules.start = vec!["first".into()];
    app.config.bars[0].panel.modules.end = vec!["second".into()];
    let origin = iced::window::Id::unique();
    app.surface_mut(origin);
    open_bar_panel(&mut app, origin);

    let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
        target: ModuleId::new("first"),
        message: ModuleDetailMessage::new(DetailTestMessage::Add(7)),
    }));
    let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
        target: ModuleId::new("second"),
        message: ModuleDetailMessage::new(DetailTestMessage::Add(11)),
    }));

    assert_eq!(detail_session_value(&app, "first"), Some(7));
    assert_eq!(detail_session_value(&app, "second"), Some(11));
}
```

Replace the existing `active_u32_session` helper with:

```rust
fn detail_session_value(app: &App, id: &str) -> Option<u32> {
    app.panel
        .module_detail_session_for(&ModuleId::new(id))
        .and_then(|session| session.downcast_ref::<u32>())
        .copied()
}
```

Update existing assertions that call `active_u32_session(&app)` to call `detail_session_value(&app, "<expected id>")`.

- [ ] **Step 2: Add failing render-model test**

In `crates/niribar/src/app/render.rs`, add this test inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn panel_render_model_passes_each_module_its_own_detail_session() {
    let mut app = crate::app::module_events::tests::app_with_detail_modules(&["first", "second"]);
    app.config.bars[0].panel.modules.start = vec!["first".into()];
    app.config.bars[0].panel.modules.end = vec!["second".into()];
    let origin = test_wid();
    app.surface_mut(origin);
    let _ = app.update(crate::app::Message::Panel(crate::app::messages::PanelEvent::Open {
        origin_window: origin,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content: crate::surface::panel::PanelContent::BarPanel,
        anchor: None,
    }));

    let panel = test_wid();
    app.register_panel_popup(
        panel,
        origin,
        crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            origin_window: origin,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 1080),
        },
    );

    let model = app.panel_render_model(panel).expect("panel model");

    assert_eq!(
        model.entries.iter().map(|entry| entry.id.clone()).collect::<Vec<_>>(),
        vec![ModuleId::new("first"), ModuleId::new("second")]
    );
}
```

- [ ] **Step 3: Run focused tests and verify red**

Run:

```bash
cargo test app::module_events::tests::bar_panel_routes_detail_messages_to_independent_module_sessions -- --nocapture
cargo test app::render::tests::panel_render_model_passes_each_module_its_own_detail_session -- --nocapture
```

Expected: fail because `module_detail_session_for` does not exist and runtime still stores one session.

## Task 2: Add Shared Panel Module Layout Resolution

**Files:**
- Modify: `crates/niribar/src/surface/panel.rs`
- Modify: `crates/niribar/src/app/panel.rs`
- Modify: `crates/niribar/src/app/render.rs`

- [ ] **Step 1: Add resolved layout entry**

Add to `crates/niribar/src/surface/panel.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelModuleEntry {
    pub id: ModuleId,
    pub zone: crate::modules::Zone,
    pub focused: bool,
}
```

- [ ] **Step 2: Add layout resolver on `App`**

In `crates/niribar/src/app/panel.rs`, replace `panel_module_ids_for_origin_window` with:

```rust
pub(super) fn panel_module_layout_for_request(
    &self,
    request: &super::panel_runtime::PanelOpenRequest,
) -> Vec<crate::surface::panel::PanelModuleEntry> {
    self.panel_module_layout_for_content(request.origin_window, &request.origin, &request.content)
}

pub(super) fn panel_module_layout_for_content(
    &self,
    origin_window: iced::window::Id,
    origin: &crate::surface::panel::PanelOrigin,
    content: &crate::surface::panel::PanelContent,
) -> Vec<crate::surface::panel::PanelModuleEntry> {
    match content {
        crate::surface::panel::PanelContent::BarPanel => {
            let origin_surface = self.surface(origin_window).cloned().unwrap_or_default();
            let bar = Self::bar_for_surface_name(&self.config.bars, &origin_surface.bar_name);
            self.effective_panel_module_layout_for_bar(bar)
                .into_iter()
                .map(|(zone, id)| {
                    let focused = origin == &crate::surface::panel::PanelOrigin::Module(id.clone());
                    crate::surface::panel::PanelModuleEntry { id, zone, focused }
                })
                .collect()
        }
        crate::surface::panel::PanelContent::ModulePanel(id) => {
            vec![crate::surface::panel::PanelModuleEntry {
                id: id.clone(),
                zone: crate::modules::Zone::Center,
                focused: origin == &crate::surface::panel::PanelOrigin::Module(id.clone()),
            }]
        }
        crate::surface::panel::PanelContent::Menu(_)
        | crate::surface::panel::PanelContent::DmenuRequest => Vec::new(),
    }
}
```

- [ ] **Step 3: Update render model to use the resolver**

In `crates/niribar/src/app/render.rs`, replace the local `layout` match in `panel_render_model` with:

```rust
let layout =
    self.panel_module_layout_for_content(tree.origin_window, &tree.origin, &tree.content);
```

Update the `filter_map` destructuring from `(zone, id)` to `entry`, then use `entry.id`, `entry.zone`, and `entry.focused`.

- [ ] **Step 4: Run render tests**

Run:

```bash
cargo test app::render::tests::panel_render_model_uses_bar_panel_modules app::render::tests::panel_render_model_uses_module_panel_content -- --nocapture
```

Expected: pass.

## Task 3: Store Detail Sessions Per Module

**Files:**
- Modify: `crates/niribar/src/app/panel_runtime.rs`
- Modify: `crates/niribar/src/app/panel.rs`
- Modify: `crates/niribar/src/app/module_events.rs`
- Modify: `crates/niribar/src/app/render.rs`

- [ ] **Step 1: Change runtime storage**

In `crates/niribar/src/app/panel_runtime.rs`, add `HashMap` to the imports:

```rust
use std::{collections::{HashMap, HashSet}, sync::Arc};
```

Change the field:

```rust
module_detail_sessions: HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession>,
```

Replace assignments to `self.module_detail_session = None` with `self.module_detail_sessions.clear()`.

- [ ] **Step 2: Change intents to carry a session map**

Change `PanelRuntimeIntent::Open` and `Toggle` fields from:

```rust
module_detail_session: Option<crate::modules::ModuleDetailSession>,
```

to:

```rust
module_detail_sessions: HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession>,
```

Apply the same signature change to `open_panel_with_catalog`, `toggle_panel_with_catalog`, and `open_panel_after_source_ready`.

- [ ] **Step 3: Add runtime lookup methods**

Replace `module_detail_session` and `module_detail_session_mut` with:

```rust
pub(super) fn module_detail_session_for(
    &self,
    id: &crate::modules::ModuleId,
) -> Option<&crate::modules::ModuleDetailSession> {
    self.module_detail_sessions.get(id)
}

pub(super) fn module_detail_session_for_mut(
    &mut self,
    id: &crate::modules::ModuleId,
) -> Option<&mut crate::modules::ModuleDetailSession> {
    self.module_detail_sessions.get_mut(id)
}
```

Add:

```rust
pub(super) fn active_module_panel_contains(&self, id: &crate::modules::ModuleId) -> bool {
    match self.active_panel.as_ref() {
        Some(ActivePanel {
            content: crate::surface::panel::PanelContent::ModulePanel(content_id),
            ..
        }) => content_id == id,
        Some(ActivePanel {
            content: crate::surface::panel::PanelContent::BarPanel,
            ..
        }) => self.module_detail_sessions.contains_key(id),
        _ => false,
    }
}
```

- [ ] **Step 4: Create session maps in `App`**

In `crates/niribar/src/app/panel.rs`, replace `module_detail_session_for_request` with:

```rust
fn module_detail_sessions_for_request(
    &self,
    request: &super::panel_runtime::PanelOpenRequest,
) -> std::collections::HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession> {
    self.panel_module_layout_for_request(request)
        .into_iter()
        .filter_map(|entry| {
            self.module_detail_session_for(&entry.id)
                .map(|session| (entry.id, session))
        })
        .collect()
}
```

Update `Open` and `Toggle` calls to pass `module_detail_sessions`.

- [ ] **Step 5: Route detail messages by target session**

In `crates/niribar/src/app/module_events.rs`, replace the current target checks and session lookup in `handle_module_detail_message` with:

```rust
if !self.panel.active_module_panel_contains(&target) {
    return Task::none();
}

let Some(module) = self.modules.get_mut(&target) else {
    return Task::none();
};
let Some(session) = self.panel.module_detail_session_for_mut(&target) else {
    return Task::none();
};
```

- [ ] **Step 6: Pass per-module sessions in render**

In `crates/niribar/src/app/render.rs`, change the module view call to:

```rust
let module_view = module.view_module_detail_session_with_style(
    &state,
    &bar.style,
    self.panel.module_detail_session_for(&id),
);
```

- [ ] **Step 7: Run focused tests**

Run:

```bash
cargo test app::module_events::tests::bar_panel_routes_detail_messages_to_independent_module_sessions -- --nocapture
cargo test app::module_events::tests::opaque_detail_message_routes_to_active_originating_module -- --nocapture
cargo test app::module_events::tests::opaque_detail_message_routes_to_module_in_active_bar_panel -- --nocapture
cargo test app::render::tests::panel_render_model_passes_each_module_its_own_detail_session -- --nocapture
```

Expected: pass.

## Task 4: Update Runtime Tests And Compatibility

**Files:**
- Modify: `crates/niribar/src/app/panel_runtime.rs`
- Modify: `crates/niribar/src/app/module_events.rs`
- Modify: `crates/niribar/src/app/render.rs`

- [ ] **Step 1: Update panel runtime unit tests**

Replace test helper payloads like:

```rust
module_detail_session: Some(Box::new(11_u32)),
```

with:

```rust
module_detail_sessions: [(crate::modules::ModuleId::new("custom"), Box::new(11_u32) as crate::modules::ModuleDetailSession)]
    .into_iter()
    .collect(),
```

Replace `runtime.module_detail_session_value::<u32>()` assertions with:

```rust
runtime
    .module_detail_session_for(&crate::modules::ModuleId::new("custom"))
    .and_then(|session| session.downcast_ref::<u32>())
```

- [ ] **Step 2: Update old module-event helper assertions**

Use `detail_session_value(&app, "detail")`, `detail_session_value(&app, "active")`, or `detail_session_value(&app, "second")` according to each test’s active target.

- [ ] **Step 3: Run all panel/module focused tests**

Run:

```bash
cargo test app::panel_runtime::tests app::module_events::tests app::render::tests -- --nocapture
```

Expected: pass.

## Task 5: Final Verification

**Files:**
- No code changes expected.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt
```

Expected: no errors.

- [ ] **Step 2: Full tests**

Run:

```bash
cargo test
```

Expected: pass.

- [ ] **Step 3: Review diff**

Run:

```bash
git diff -- crates/niribar/src/surface/panel.rs crates/niribar/src/app/panel.rs crates/niribar/src/app/panel_runtime.rs crates/niribar/src/app/render.rs crates/niribar/src/app/module_events.rs docs/niribar/superpowers/specs/2026-05-31-panel-module-layout-design.md docs/niribar/superpowers/plans/2026-05-31-panel-module-layout.md
```

Expected: changes are scoped to shared module-backed **Panel** layout and per-module **Module Detail Sessions**.
