# Panel Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deepen the Panel Runtime module so Panel lifecycle, search/focus state, menu activation, and Dmenu Selection Request responder state live behind one focused interface.

**Architecture:** Add `src/app/panel_runtime.rs` as the state module for the active **Panel Runtime**. `App` remains the adapter for iced tasks, layer-shell windows, overlay effects, config/menu source lookup, process launch, and physical IPC response sending. `PanelMenuSession` remains the module for **Panel Menu Session** search, focus, submenu navigation, and activation calculation.

**Tech Stack:** Rust, iced `Task`, iced layer-shell, existing `cargo test` unit tests.

---

## File Structure

- Create: `src/app/panel_runtime.rs`
  - Owns active Panel Runtime state copied from `PanelSystem`.
  - Provides methods for search, menu scroll, focus movement, back, escape, activation, Dmenu responder storage, and closing-window bookkeeping.
  - Returns `PanelRuntimeEffect` values for outside-world work.
- Modify: `src/app.rs`
  - Add `mod panel_runtime;`.
  - Change `App.panel` from `panel::PanelSystem` to `panel_runtime::PanelRuntime`.
- Modify: `src/app/panel.rs`
  - Remove `PanelSystem`.
  - Convert panel event handling to call `PanelRuntime` methods.
  - Adapt `PanelRuntimeEffect` values into existing `Task<Message>` behavior.
  - Keep layer-shell settings, `PanelTree` creation, overlay effects, source resolution, process launch, and IPC response sending here.
- Modify: `src/app/surfaces.rs`
  - Update references to `self.panel.is_closing_window`, `mark_closing_window`, and `forget_closing_window` if method names change.
- Modify: `CONTEXT.md`
  - Already updated with **Panel Runtime**. Do not rename the term without updating the glossary.

---

## Task 1: Add Panel Runtime Skeleton

**Files:**

- Create: `src/app/panel_runtime.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Write the failing compile target**

Modify `src/app.rs` near the existing module declarations:

```rust
mod panel_runtime;
```

Modify the `App` struct field:

```rust
panel: panel_runtime::PanelRuntime,
```

- [ ] **Step 2: Run compile check to verify it fails**

Run:

```bash
cargo test --no-run
```

Expected: FAIL because `src/app/panel_runtime.rs` does not exist or `PanelRuntime` is not defined.

- [ ] **Step 3: Add minimal Panel Runtime module**

Create `src/app/panel_runtime.rs`:

```rust
use std::collections::HashSet;

#[derive(Debug, Default)]
pub(super) struct PanelRuntime {
    session: Option<super::panel_menu_session::PanelMenuSession>,
    menu_scroll_y: f32,
    menu_viewport_height: f32,
    search_focus_pending: bool,
    dmenu_responder: Option<super::messages::IpcResponder>,
    closing_windows: HashSet<iced::window::Id>,
}

impl PanelRuntime {
    pub(super) fn clear(&mut self) {
        self.session = None;
        self.menu_scroll_y = 0.0;
        self.menu_viewport_height = 0.0;
        self.search_focus_pending = false;
        self.dmenu_responder = None;
    }

    pub(super) fn mark_closing_window(&mut self, window: iced::window::Id) {
        self.closing_windows.insert(window);
    }

    pub(super) fn is_closing_window(&self, window: iced::window::Id) -> bool {
        self.closing_windows.contains(&window)
    }

    pub(super) fn forget_closing_window(&mut self, window: iced::window::Id) {
        self.closing_windows.remove(&window);
    }
}
```

- [ ] **Step 4: Update old direct field references temporarily**

In `src/app/panel.rs`, replace `PanelSystem` with imports from `panel_runtime` only after Task 2. For this task, keep the compile failure focused on `App.panel` type usage. If compile errors show private field access from `panel.rs`, stop and continue with Task 2.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/app/panel_runtime.rs
git commit -m "refactor: introduce panel runtime module"
```

---

## Task 2: Move PanelSystem State Behind Accessors

**Files:**

- Modify: `src/app/panel_runtime.rs`
- Modify: `src/app/panel.rs`

- [ ] **Step 1: Add failing tests for state access**

Add this test module to the bottom of `src/app/panel_runtime.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_resets_session_scroll_focus_and_dmenu_state() {
        let mut runtime = PanelRuntime::default();
        runtime.set_menu_scrolled(120.0, 300.0);
        runtime.set_search_focus_pending(true);
        runtime.clear();

        assert_eq!(runtime.menu_scroll_y(), 0.0);
        assert_eq!(runtime.menu_viewport_height(), 0.0);
        assert!(!runtime.search_focus_pending());
        assert!(!runtime.has_dmenu_responder());
    }

    #[test]
    fn closing_windows_are_tracked_by_window_id() {
        let window = iced::window::Id::unique();
        let mut runtime = PanelRuntime::default();

        runtime.mark_closing_window(window);
        assert!(runtime.is_closing_window(window));

        runtime.forget_closing_window(window);
        assert!(!runtime.is_closing_window(window));
    }
}
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test app::panel_runtime::tests -- --nocapture
```

Expected: FAIL because the accessor methods do not exist.

- [ ] **Step 3: Add accessors and scroll/focus setters**

Extend `impl PanelRuntime`:

```rust
impl PanelRuntime {
    pub(super) fn menu_scroll_y(&self) -> f32 {
        self.menu_scroll_y
    }

    pub(super) fn menu_viewport_height(&self) -> f32 {
        self.menu_viewport_height
    }

    pub(super) fn set_menu_scrolled(&mut self, y: f32, viewport_height: f32) {
        self.menu_scroll_y = y.max(0.0);
        self.menu_viewport_height = viewport_height.max(0.0);
    }

    pub(super) fn reset_menu_scroll(&mut self) {
        self.menu_scroll_y = 0.0;
    }

    pub(super) fn search_focus_pending(&self) -> bool {
        self.search_focus_pending
    }

    pub(super) fn set_search_focus_pending(&mut self, pending: bool) {
        self.search_focus_pending = pending;
    }

    pub(super) fn has_dmenu_responder(&self) -> bool {
        self.dmenu_responder.is_some()
    }
}
```

- [ ] **Step 4: Replace direct field access in `panel.rs`**

Replace these patterns in `src/app/panel.rs`:

```rust
self.panel.menu_scroll_y = y.max(0.0);
self.panel.menu_viewport_height = viewport_height.max(0.0);
```

with:

```rust
self.panel.set_menu_scrolled(y, viewport_height);
```

Replace:

```rust
self.panel.menu_scroll_y = 0.0;
```

with:

```rust
self.panel.reset_menu_scroll();
```

Replace reads of:

```rust
self.panel.menu_scroll_y
self.panel.menu_viewport_height
self.panel.search_focus_pending
```

with:

```rust
self.panel.menu_scroll_y()
self.panel.menu_viewport_height()
self.panel.search_focus_pending()
```

Replace writes of:

```rust
self.panel.search_focus_pending = true;
self.panel.search_focus_pending = false;
```

with:

```rust
self.panel.set_search_focus_pending(true);
self.panel.set_search_focus_pending(false);
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test app::panel_runtime::tests app::panel::tests::escape_clears_search_before_closing_panel -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/app/panel_runtime.rs src/app/panel.rs
git commit -m "refactor: encapsulate panel runtime state"
```

---

## Task 3: Move Panel Menu Session Operations Into PanelRuntime

**Files:**

- Modify: `src/app/panel_runtime.rs`
- Modify: `src/app/panel.rs`
- Modify: `src/app/render.rs`
- Modify: `src/app/view.rs` only if compiler reveals direct session access there

- [ ] **Step 1: Add failing tests for session behavior**

Add to `src/app/panel_runtime.rs` tests:

```rust
fn menu_with_two_entries() -> crate::config::MenuConfig {
    crate::config::MenuConfig {
        name: "apps".into(),
        prompt: Some("Run".into()),
        show_icons: false,
        source: None,
        items: vec![
            crate::config::MenuItemConfig {
                label: "Terminal".into(),
                icon: None,
                keywords: vec!["shell".into()],
                action: Some(crate::config::PanelAction::Command(
                    crate::config::CommandConfig::Shell("kitty".into()),
                )),
                children: vec![],
            },
            crate::config::MenuItemConfig {
                label: "Browser".into(),
                icon: None,
                keywords: vec!["web".into()],
                action: Some(crate::config::PanelAction::Command(
                    crate::config::CommandConfig::Shell("firefox".into()),
                )),
                children: vec![],
            },
        ],
    }
}

#[test]
fn search_changed_updates_session_and_resets_scroll() {
    let mut runtime = PanelRuntime::default();
    runtime.open_session(super::super::panel_menu_session::PanelMenuSource::Configured(
        menu_with_two_entries(),
    ));
    runtime.set_menu_scrolled(90.0, 200.0);

    runtime.search_changed("term".into());

    let snapshot = runtime.panel_menu_snapshot().expect("snapshot");
    assert_eq!(snapshot.search_query, "term");
    assert_eq!(runtime.menu_scroll_y(), 0.0);
}

#[test]
fn activate_focused_returns_panel_action_effect() {
    let mut runtime = PanelRuntime::default();
    runtime.open_session(super::super::panel_menu_session::PanelMenuSource::Configured(
        menu_with_two_entries(),
    ));

    let effects = runtime.activate(None);

    assert!(matches!(
        effects.as_slice(),
        [PanelRuntimeEffect::RunPanelAction(crate::config::PanelAction::Command(_))]
    ));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test app::panel_runtime::tests -- --nocapture
```

Expected: FAIL because `PanelRuntimeEffect`, `open_session`, `search_changed`, `panel_menu_snapshot`, and `activate` do not exist.

- [ ] **Step 3: Add session methods and effects**

Add near the top of `src/app/panel_runtime.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PanelRuntimeEffect {
    RequestSearchFocus,
    RunPanelAction(crate::config::PanelAction),
    LaunchInstalledApp(String),
    CompleteDmenu(crate::ipc::protocol::IpcResponse),
    None,
}
```

Add to `impl PanelRuntime`:

```rust
pub(super) fn open_session(&mut self, source: super::panel_menu_session::PanelMenuSource) {
    let session = super::panel_menu_session::PanelMenuSession::open(source);
    let _ = session.snapshot();
    self.session = Some(session);
}

pub(super) fn clear_session(&mut self) {
    self.session = None;
}

pub(super) fn search_changed(&mut self, query: String) {
    if let Some(session) = self.session.as_mut() {
        session.set_search_query(query);
    }
    self.search_focus_pending = false;
    self.menu_scroll_y = 0.0;
}

pub(super) fn focus_index(&mut self, index: usize) {
    if let Some(session) = self.session.as_mut() {
        session.focus_index(index);
    }
    self.search_focus_pending = false;
}

pub(super) fn move_focus(&mut self, delta: isize) -> Option<usize> {
    if self.active_panel_menu_entries().is_empty() {
        self.menu_scroll_y = 0.0;
        return None;
    }
    let session = self.session.as_mut()?;
    session.move_focus(delta);
    session.focused_index()
}

pub(super) fn back(&mut self) {
    if let Some(session) = self.session.as_mut() {
        session.back();
    }
}

pub(super) fn escape(&mut self) -> Vec<PanelRuntimeEffect> {
    let cleared_search = self
        .session
        .as_mut()
        .map(super::panel_menu_session::PanelMenuSession::clear_search)
        .unwrap_or(false);
    if cleared_search {
        self.search_focus_pending = true;
        vec![PanelRuntimeEffect::RequestSearchFocus]
    } else {
        Vec::new()
    }
}

pub(super) fn panel_menu_snapshot(
    &self,
) -> Option<super::panel_menu_session::PanelMenuSnapshot> {
    self.session.as_ref().map(|session| session.snapshot())
}

pub(super) fn active_panel_menu_entries(
    &self,
) -> Vec<super::panel_menu_session::PanelMenuEntry> {
    self.panel_menu_snapshot()
        .map(|snapshot| snapshot.entries.to_vec())
        .unwrap_or_default()
}

pub(super) fn activate(&mut self, index: Option<usize>) -> Vec<PanelRuntimeEffect> {
    let activation = match (self.session.as_mut(), index) {
        (Some(session), Some(index)) => session.activate_index(index),
        (Some(session), None) => session.activate_focused(),
        (None, _) => super::panel_menu_session::PanelMenuActivation::None,
    };
    match activation {
        super::panel_menu_session::PanelMenuActivation::RunPanelAction(action) => {
            vec![PanelRuntimeEffect::RunPanelAction(action)]
        }
        super::panel_menu_session::PanelMenuActivation::LaunchInstalledApp(id) => {
            vec![PanelRuntimeEffect::LaunchInstalledApp(id)]
        }
        super::panel_menu_session::PanelMenuActivation::CompleteDmenu(response) => {
            vec![PanelRuntimeEffect::CompleteDmenu(response)]
        }
        super::panel_menu_session::PanelMenuActivation::None => Vec::new(),
    }
}
```

- [ ] **Step 4: Adapt `panel.rs` to use session methods**

In `src/app/panel.rs`, replace direct session access:

```rust
self.panel.session = Some(session);
self.panel.session.as_mut()
self.panel.session.as_ref()
```

with `PanelRuntime` calls:

```rust
self.panel.open_session(source);
self.panel.search_changed(query);
self.panel.focus_index(index);
self.panel.back();
self.panel.panel_menu_snapshot();
self.panel.active_panel_menu_entries();
self.panel.activate(index);
```

Keep `request_panel_search_focus`, `scroll_panel_menu_to_focus`, and `apply_panel_menu_activation` in `panel.rs` for now. Change `apply_panel_menu_activation` to take `PanelRuntimeEffect` values instead of `PanelMenuActivation`.

- [ ] **Step 5: Run focused panel tests**

Run:

```bash
cargo test app::panel_runtime::tests app::panel::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/app/panel_runtime.rs src/app/panel.rs src/app/render.rs src/app/view.rs
git commit -m "refactor: move panel menu session control into runtime"
```

---

## Task 4: Move Dmenu Responder Lifecycle Into PanelRuntime

**Files:**

- Modify: `src/app/panel_runtime.rs`
- Modify: `src/app/panel.rs`

- [ ] **Step 1: Add failing tests for Dmenu lifecycle**

Add to `src/app/panel_runtime.rs` tests:

```rust
#[test]
fn dmenu_responder_rejects_second_active_request() {
    let (sender, _receiver) = tokio::sync::oneshot::channel();
    let responder = super::super::messages::ipc_responder(sender);
    let mut runtime = PanelRuntime::default();

    assert!(runtime.begin_dmenu_request(responder).is_ok());

    let (sender2, _receiver2) = tokio::sync::oneshot::channel();
    let responder2 = super::super::messages::ipc_responder(sender2);
    assert!(runtime.begin_dmenu_request(responder2).is_err());
}

#[test]
fn complete_dmenu_request_takes_responder_once() {
    let (sender, _receiver) = tokio::sync::oneshot::channel();
    let responder = super::super::messages::ipc_responder(sender);
    let mut runtime = PanelRuntime::default();

    runtime.begin_dmenu_request(responder).expect("stored");
    let first = runtime.take_dmenu_responder();
    let second = runtime.take_dmenu_responder();

    assert!(first.is_some());
    assert!(second.is_none());
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test app::panel_runtime::tests::dmenu -- --nocapture
```

Expected: FAIL because Dmenu methods do not exist.

- [ ] **Step 3: Add Dmenu responder methods**

Add to `impl PanelRuntime`:

```rust
pub(super) fn begin_dmenu_request(
    &mut self,
    responder: super::messages::IpcResponder,
) -> Result<(), super::messages::IpcResponder> {
    if self.dmenu_responder.is_some() {
        return Err(responder);
    }
    self.dmenu_responder = Some(responder);
    Ok(())
}

pub(super) fn take_dmenu_responder(&mut self) -> Option<super::messages::IpcResponder> {
    self.dmenu_responder.take()
}

pub(super) fn cancel_dmenu_if_active(&mut self) -> Option<super::messages::IpcResponder> {
    self.dmenu_responder.take()
}
```

- [ ] **Step 4: Adapt `panel.rs` Dmenu flow**

In `handle_dmenu_selection_request`, replace direct `self.panel.dmenu_responder` checks and assignment with:

```rust
if let Err(responder) = self.panel.begin_dmenu_request(responder) {
    send_ipc_response(
        responder,
        crate::ipc::protocol::IpcResponse::Rejected {
            reason: "another dmenu request is already active".into(),
        },
    );
    return Task::none();
}
```

After the default origin window check fails, take the responder back and reject:

```rust
if let Some(responder) = self.panel.take_dmenu_responder() {
    send_ipc_response(
        responder,
        crate::ipc::protocol::IpcResponse::Rejected {
            reason: "no bar surface is available".into(),
        },
    );
}
return Task::none();
```

In `complete_dmenu_request`, use:

```rust
if let Some(responder) = self.panel.take_dmenu_responder() {
    send_ipc_response(responder, response);
}
```

In `close_all_panels` and `close_all_untracked_panels_with_restore`, replace direct checks with:

```rust
if let Some(responder) = self.panel.cancel_dmenu_if_active() {
    send_ipc_response(responder, crate::ipc::protocol::IpcResponse::Cancelled);
}
```

- [ ] **Step 5: Run Dmenu and panel tests**

Run:

```bash
cargo test app::panel_runtime::tests::dmenu app::panel::tests::dmenu app::panel::tests::close_all_panels -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/app/panel_runtime.rs src/app/panel.rs
git commit -m "refactor: move dmenu responder lifecycle into panel runtime"
```

---

## Task 5: Adapt Runtime Effects In App

**Files:**

- Modify: `src/app/panel.rs`
- Modify: `src/app/panel_runtime.rs`

- [ ] **Step 1: Add focused test for Escape effect adaptation**

Use existing test `escape_that_clears_search_requests_search_refocus` in `src/app/panel.rs` as the behavior lock. If it does not fail during this task, add this assertion to that test:

```rust
assert!(app.panel.search_focus_pending());
```

- [ ] **Step 2: Add an effect adapter in `panel.rs`**

Add this helper in `impl App` near `apply_panel_menu_activation`:

```rust
fn apply_panel_runtime_effects(
    &mut self,
    effects: Vec<super::panel_runtime::PanelRuntimeEffect>,
) -> Task<Message> {
    let mut tasks = Vec::new();
    for effect in effects {
        match effect {
            super::panel_runtime::PanelRuntimeEffect::RequestSearchFocus => {
                tasks.push(request_panel_search_focus());
            }
            super::panel_runtime::PanelRuntimeEffect::RunPanelAction(action) => {
                let task = self.dispatch_panel_action_from_active_panel(action);
                tasks.push(task);
                tasks.extend(self.close_all_panels());
            }
            super::panel_runtime::PanelRuntimeEffect::LaunchInstalledApp(id) => {
                self.launch_installed_app_by_id(&id);
                tasks.extend(self.close_all_panels());
            }
            super::panel_runtime::PanelRuntimeEffect::CompleteDmenu(response) => {
                self.complete_dmenu_request(response);
                tasks.extend(self.close_all_panels());
            }
            super::panel_runtime::PanelRuntimeEffect::None => {}
        }
    }
    Task::batch(tasks)
}
```

- [ ] **Step 3: Simplify activation and escape call sites**

Change `activate_panel_entry` to:

```rust
fn activate_panel_entry(&mut self, index: Option<usize>) -> Task<Message> {
    let effects = self.panel.activate(index);
    self.apply_panel_runtime_effects(effects)
}
```

Change `handle_panel_escape_pressed` to:

```rust
pub(super) fn handle_panel_escape_pressed(&mut self) -> Task<Message> {
    let effects = self.panel.escape();
    if effects.is_empty() {
        self.handle_panel_event(super::messages::PanelEvent::Close)
    } else {
        self.apply_panel_runtime_effects(effects)
    }
}
```

- [ ] **Step 4: Remove obsolete activation helper**

Delete `apply_panel_menu_activation` once no call sites remain.

- [ ] **Step 5: Run panel tests**

Run:

```bash
cargo test app::panel::tests app::panel_runtime::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/app/panel.rs src/app/panel_runtime.rs
git commit -m "refactor: adapt panel runtime effects in app"
```

---

## Task 6: Final Cleanup And Verification

**Files:**

- Modify: `src/app/panel.rs`
- Modify: `src/app/panel_runtime.rs`
- Modify: any compiler-identified direct imports

- [ ] **Step 1: Search for leaked runtime fields**

Run:

```bash
rg "panel\\.(session|menu_scroll_y|menu_viewport_height|search_focus_pending|dmenu_responder)" src/app
```

Expected: no matches. If matches remain, replace them with `PanelRuntime` methods.

- [ ] **Step 2: Search for old type**

Run:

```bash
rg "PanelSystem" src
```

Expected: no matches.

- [ ] **Step 3: Run formatting**

Run:

```bash
cargo fmt
```

Expected: completes with no output.

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test app::panel::tests app::panel_runtime::tests app::render::tests app::surfaces::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Run full tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/app/panel.rs src/app/panel_runtime.rs src/app/render.rs src/app/view.rs src/app/surfaces.rs CONTEXT.md
git commit -m "refactor: deepen panel runtime lifecycle"
```

---

## Self-Review

Spec coverage:

- **Panel Runtime owns active state:** Tasks 1-3 move `PanelSystem` fields and session operations behind `PanelRuntime`.
- **Dmenu responder lifecycle lives in Panel Runtime:** Task 4 implements begin/take/cancel responder methods.
- **App remains adapter:** Tasks 3-5 keep layer-shell, overlays, IPC sending, process launch, and source resolution in `panel.rs`.
- **All Panel content is represented by the runtime:** Task 1 introduces a runtime used by `App.panel`; Task 5 adapts all activation and escape effects through runtime methods.
- **Panel Menu Source resolution stays in App:** Task 3 explicitly leaves `PanelMenuSource` construction in `panel.rs`.

Placeholder scan:

- No `TBD`, `TODO`, or “implement later” placeholders are used.
- Code snippets define concrete method names and exact files.

Type consistency:

- `PanelRuntime`, `PanelRuntimeEffect`, and method names are consistent across tasks.
- The plan intentionally keeps `PanelMenuSession`, `PanelMenuSource`, and `PanelMenuActivation` names from existing code.
