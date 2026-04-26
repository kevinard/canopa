# Module Actions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add per-module `actions { click "..."; right-click "..." }` configuration that overrides built-in click behavior when present.

**Architecture:** Parse shared actions once in `src/config/parse/mod.rs` into `ModuleConfig`, store action metadata in `ModuleRuntime` next to module trait objects, and dispatch configured shell commands before calling `Module::on_interaction`. Right-click gets a runtime event parallel to existing left-click events.

**Tech Stack:** Rust, KDL parser, Iced `mouse_area`, existing `crate::command::spawn_command_fire_and_forget`, `cargo test`.

---

### Task 1: Parse Shared Actions

**Files:**
- Modify: `src/config.rs`
- Modify: `src/config/parse/mod.rs`
- Create: `src/config/parse/actions.rs`
- Modify: `src/config/parse/modules/button.rs`

- [ ] **Step 1: Write failing parser tests**

Add tests in `src/config/parse/tests.rs` that parse `actions.click`, parse `actions.right-click`, allow an empty block, reject unknown action nodes, reject duplicate nodes, and allow a button with `actions.click` but no legacy `on-click`.

- [ ] **Step 2: Verify parser tests fail**

Run: `rtk cargo test config::parse::tests::parse_module_actions`

Expected: FAIL because `ModuleConfig.actions` and action parsing do not exist.

- [ ] **Step 3: Implement shared action config**

Add `ModuleActionsConfig { click: Option<String>, right_click: Option<String> }`, add `actions` to `ModuleConfig`, parse optional `actions` blocks in `parse_modules_section`, and loosen button parsing so either legacy `on-click` or shared `actions.click` is present.

- [ ] **Step 4: Verify parser tests pass**

Run: `rtk cargo test config::parse::tests::parse_module_actions`

Expected: PASS.

### Task 2: Store and Dispatch Configured Actions

**Files:**
- Modify: `src/app/module_runtime.rs`
- Modify: `src/app/module_store.rs`
- Modify: `src/app/module_events.rs`
- Modify: `src/app/messages.rs`
- Modify: `src/modules.rs`

- [ ] **Step 1: Write failing runtime tests**

Add tests showing configured left-click runs instead of a module built-in, configured right-click runs before built-ins, and missing actions fall back to existing `Module::on_interaction`.

- [ ] **Step 2: Verify runtime tests fail**

Run: `rtk cargo test app::module_runtime::tests::module_actions`

Expected: FAIL because runtime action lookup and right-click event support do not exist.

- [ ] **Step 3: Implement runtime action lookup**

Store `ModuleActionsConfig` by module ID in `ModuleRuntime`, add `InteractionEvent::RightClicked`, add `ModuleRuntimeEvent::ModuleRightClick`, and update `dispatch_module_interaction` to run configured shell commands before module built-ins.

- [ ] **Step 4: Verify runtime tests pass**

Run: `rtk cargo test app::module_runtime::tests::module_actions`

Expected: PASS.

### Task 3: Wire Right-Click Rendering and Dynamic Inheritance

**Files:**
- Modify: `src/app/render.rs`
- Modify: `src/app/tray.rs`
- Modify: `src/app/niri.rs`
- Modify: `src/modules/tray.rs`
- Modify: `src/modules/workspaces.rs`

- [ ] **Step 1: Write failing behavior tests**

Add tests showing the shared module wrapper can emit right-click messages, tray icons inherit `tray.actions.right-click`, and workspace items inherit `workspaces.actions.click`.

- [ ] **Step 2: Verify tests fail**

Run: `rtk cargo test app::render::tests app::module_runtime::tests::dynamic_module_actions`

Expected: FAIL because right-click is not wired in rendering and generated modules do not inherit actions.

- [ ] **Step 3: Implement render and dynamic wiring**

Use `mouse_area.on_right_press` in the shared wrapper. When creating tray icons or workspace items, copy actions from their config placeholder into the generated runtime module entry. For tray icons, configured right-click action overrides the tray menu request.

- [ ] **Step 4: Verify tests pass**

Run: `rtk cargo test app::render::tests app::module_runtime::tests::dynamic_module_actions`

Expected: PASS.

### Task 4: Update Defaults and Full Verification

**Files:**
- Modify: `config/default.kdl`

- [ ] **Step 1: Document actions in the default config**

Add commented `actions` examples under representative modules without changing default behavior.

- [ ] **Step 2: Run focused verification**

Run: `rtk cargo test config::parse app::module_runtime app::render modules::button`

Expected: PASS.

- [ ] **Step 3: Run full verification**

Run: `rtk cargo test`

Expected: PASS.
