# Panel Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deepen the **Panel Runtime** module so it owns Panel logical lifecycle, search/focus state, Panel Menu Session activation, and Dmenu Selection Request responder state behind one focused interface.

**Architecture:** Delete the shallow `PanelCoordinator` module and fold its active Panel tracking, `PanelOpenRequest`, pending Installed Desktop Application menu opens, and lifecycle effects into `PanelRuntime`. `App` remains the adapter for iced `Task`s, layer-shell windows, overlay effects, config/menu lookup inputs, Installed Desktop Application discovery, process launch, and physical IPC response sending. `PanelMenuSession` remains the module for menu search, focus, submenu navigation, and activation calculation.

**Tech Stack:** Rust, iced, iced layer-shell, existing unit tests through `cargo test`.

---

## File Structure

- Modify: `crates/niribar/src/app/panel_runtime.rs`
  - Owns `PanelRuntime`, `PanelRuntimeEffect`, `PanelOpenRequest`, `PendingPanelOpen`, and active Panel tracking.
  - Provides logical lifecycle methods: `open_panel_with_menus`, `toggle_panel_with_menus`, `open_panel_after_installed_apps_loaded`, `panel_window_opened`, `begin_dmenu_selection`, and existing interaction methods.
  - Returns effects for `App` to adapt into iced/window/IPC/process work.
- Delete: `src/app/panel_coordinator.rs`
  - Its current interface is mostly forwarding and active Panel bookkeeping. The deletion test says its complexity should move into `PanelRuntime`.
- Modify: `crates/niribar/src/app.rs`
  - Remove `mod panel_coordinator;`.
  - Store `panel: panel_runtime::PanelRuntime`.
- Modify: `crates/niribar/src/app/panel.rs`
  - Replace all `panel_coordinator` references with `panel_runtime`.
  - Adapt the expanded `PanelRuntimeEffect` enum into existing iced tasks and IPC responses.
  - Keep layer-shell settings, `PanelTree` construction, overlay cleanup, menu source lookup inputs, Installed Desktop Application discovery, launch, and physical IPC response sending here.
- Modify: `crates/niribar/src/app/surfaces.rs`
  - Keep calls to closing-window bookkeeping on `self.panel`; the implementation now lives directly in `PanelRuntime`.
- Modify: `crates/niribar/src/app/render.rs`
  - Keep read-only Panel Menu Snapshot access through `PanelRuntime`.
- Modify: `CONTEXT.md`
  - No glossary change is expected because **Panel Runtime** already exists. Only edit if implementation introduces a new domain term.

---

## Task 1: Move Panel Lifecycle Types Into Panel Runtime

**Files:**
- Modify: `crates/niribar/src/app/panel_runtime.rs`
- Modify: `src/app/panel_coordinator.rs`

- [ ] **Step 1: Add failing lifecycle tests to `crates/niribar/src/app/panel_runtime.rs`**

Append these tests inside the existing `#[cfg(test)] mod tests` in `crates/niribar/src/app/panel_runtime.rs`:

```rust
fn wid() -> iced::window::Id {
    iced::window::Id::unique()
}

fn open_request(window: iced::window::Id, content: crate::surface::panel::PanelContent) -> super::PanelOpenRequest {
    super::PanelOpenRequest {
        origin_window: window,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content,
        anchor: None,
    }
}

#[test]
fn opening_a_panel_replaces_the_active_panel() {
    let origin = wid();
    let first_panel = wid();
    let second_panel = wid();
    let mut runtime = PanelRuntime::default();

    let first = runtime.open_panel_with_menus(
        open_request(origin, crate::surface::panel::PanelContent::BarPanel),
        &[],
    );
    assert_eq!(
        first,
        vec![PanelRuntimeEffect::OpenPanelWindow(open_request(
            origin,
            crate::surface::panel::PanelContent::BarPanel
        ))]
    );
    runtime.panel_window_opened(
        first_panel,
        origin,
        crate::surface::panel::PanelContent::BarPanel,
    );

    let second = runtime.open_panel_with_menus(
        open_request(origin, crate::surface::panel::PanelContent::Menu("main".into())),
        &[],
    );

    assert_eq!(
        second,
        vec![
            PanelRuntimeEffect::ClosePanelWindow(first_panel),
            PanelRuntimeEffect::OpenPanelWindow(open_request(
                origin,
                crate::surface::panel::PanelContent::Menu("main".into())
            )),
            PanelRuntimeEffect::RequestSearchFocus,
        ]
    );
    runtime.panel_window_opened(
        second_panel,
        origin,
        crate::surface::panel::PanelContent::Menu("main".into()),
    );
    assert_eq!(runtime.active_panel_window(), Some(second_panel));
}

#[test]
fn toggling_the_same_panel_closes_it() {
    let origin = wid();
    let panel = wid();
    let mut runtime = PanelRuntime::default();
    runtime.panel_window_opened(
        panel,
        origin,
        crate::surface::panel::PanelContent::BarPanel,
    );

    let effects = runtime.toggle_panel_with_menus(
        open_request(origin, crate::surface::panel::PanelContent::BarPanel),
        &[],
    );

    assert_eq!(effects, vec![PanelRuntimeEffect::ClosePanelWindow(panel)]);
    assert_eq!(runtime.active_panel_window(), None);
}

#[test]
fn installed_apps_menu_returns_load_effect_before_opening_panel() {
    let origin = wid();
    let mut runtime = PanelRuntime::default();
    let menus = vec![crate::config::MenuConfig {
        name: "apps".into(),
        prompt: Some("Run".into()),
        show_icons: true,
        source: Some(crate::config::MenuSourceConfig {
            kind: crate::config::MenuSourceKind::InstalledApps,
        }),
        items: Vec::new(),
    }];

    let effects = runtime.open_panel_with_menus(
        open_request(origin, crate::surface::panel::PanelContent::Menu("apps".into())),
        &menus,
    );

    assert_eq!(
        effects,
        vec![PanelRuntimeEffect::LoadInstalledAppsThenOpenPanel(
            super::PendingPanelOpen {
                request: open_request(origin, crate::surface::panel::PanelContent::Menu("apps".into())),
                menu_name: "apps".into(),
            }
        )]
    );
    assert_eq!(runtime.active_panel_window(), None);
}
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run:

```bash
cargo test app::panel_runtime::tests::opening_a_panel_replaces_the_active_panel app::panel_runtime::tests::toggling_the_same_panel_closes_it app::panel_runtime::tests::installed_apps_menu_returns_load_effect_before_opening_panel -- --nocapture
```

Expected: FAIL with missing `PanelOpenRequest`, `PendingPanelOpen`, lifecycle `PanelRuntimeEffect` variants, and lifecycle methods in `PanelRuntime`.

- [ ] **Step 3: Copy lifecycle types into `crates/niribar/src/app/panel_runtime.rs`**

Add this above `PanelRuntimeEffect`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PanelOpenRequest {
    pub(super) origin_window: iced::window::Id,
    pub(super) origin: crate::surface::panel::PanelOrigin,
    pub(super) content: crate::surface::panel::PanelContent,
    pub(super) anchor: Option<iced::Rectangle>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PendingPanelOpen {
    pub(super) request: PanelOpenRequest,
    pub(super) menu_name: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ActivePanel {
    window: iced::window::Id,
    origin_window: iced::window::Id,
    content: crate::surface::panel::PanelContent,
}
```

Change `PanelRuntimeEffect` to:

```rust
#[derive(Debug, Clone, PartialEq)]
pub(super) enum PanelRuntimeEffect {
    OpenPanelWindow(PanelOpenRequest),
    ClosePanelWindow(iced::window::Id),
    RequestSearchFocus,
    LoadInstalledAppsThenOpenPanel(PendingPanelOpen),
    RunPanelAction(crate::config::PanelAction),
    LaunchInstalledApp(String),
    CompleteDmenu(crate::ipc::protocol::IpcResponse),
}
```

Add this field to `PanelRuntime`:

```rust
active_panel: Option<ActivePanel>,
```

Update `clear` so it clears active Panel state too:

```rust
pub(super) fn clear(&mut self) {
    self.active_panel = None;
    self.session = None;
    self.menu_scroll_y = 0.0;
    self.menu_viewport_height = 0.0;
    self.search_focus_pending = false;
    self.dmenu_responder = None;
}
```

- [ ] **Step 4: Add lifecycle methods to `PanelRuntime`**

Add these methods to `impl PanelRuntime`:

```rust
pub(super) fn active_panel_window(&self) -> Option<iced::window::Id> {
    self.active_panel.as_ref().map(|panel| panel.window)
}

pub(super) fn panel_window_opened(
    &mut self,
    window: iced::window::Id,
    origin_window: iced::window::Id,
    content: crate::surface::panel::PanelContent,
) {
    self.active_panel = Some(ActivePanel {
        window,
        origin_window,
        content,
    });
}

pub(super) fn open_panel_with_menus(
    &mut self,
    request: PanelOpenRequest,
    menus: &[crate::config::MenuConfig],
) -> Vec<PanelRuntimeEffect> {
    if let crate::surface::panel::PanelContent::Menu(name) = &request.content
        && menus
            .iter()
            .find(|menu| menu.name == *name)
            .and_then(|menu| menu.source.as_ref())
            .is_some_and(|source| {
                matches!(source.kind, crate::config::MenuSourceKind::InstalledApps)
            })
    {
        return vec![PanelRuntimeEffect::LoadInstalledAppsThenOpenPanel(
            PendingPanelOpen {
                request,
                menu_name: name.clone(),
            },
        )];
    }
    self.open_panel_after_source_ready(request)
}

pub(super) fn toggle_panel_with_menus(
    &mut self,
    request: PanelOpenRequest,
    menus: &[crate::config::MenuConfig],
) -> Vec<PanelRuntimeEffect> {
    if self.active_panel.as_ref().is_some_and(|panel| {
        panel.origin_window == request.origin_window && panel.content == request.content
    }) {
        if let Some(window) = self.active_panel_window() {
            self.active_panel = None;
            self.session = None;
            self.menu_scroll_y = 0.0;
            self.menu_viewport_height = 0.0;
            self.search_focus_pending = false;
            return vec![PanelRuntimeEffect::ClosePanelWindow(window)];
        }
    }
    self.open_panel_with_menus(request, menus)
}

pub(super) fn open_panel_after_installed_apps_loaded(
    &mut self,
    pending: PendingPanelOpen,
    apps: Vec<crate::desktop_apps::DesktopApplication>,
    menus: &[crate::config::MenuConfig],
) -> Vec<PanelRuntimeEffect> {
    let effects = self.open_panel_after_source_ready(pending.request);
    if let Some(menu) = menus.iter().find(|menu| menu.name == pending.menu_name) {
        self.open_session(super::panel_menu_session::PanelMenuSource::InstalledApps {
            prompt: menu.prompt.clone(),
            show_icons: menu.show_icons,
            apps,
        });
    }
    effects
}

fn open_panel_after_source_ready(
    &mut self,
    request: PanelOpenRequest,
) -> Vec<PanelRuntimeEffect> {
    let mut effects = Vec::new();
    if let Some(window) = self.active_panel_window() {
        effects.push(PanelRuntimeEffect::ClosePanelWindow(window));
    }
    self.active_panel = None;
    self.session = None;
    self.menu_scroll_y = 0.0;
    self.menu_viewport_height = 0.0;
    self.search_focus_pending = false;
    effects.push(PanelRuntimeEffect::OpenPanelWindow(request.clone()));
    if matches!(
        request.content,
        crate::surface::panel::PanelContent::Menu(_)
            | crate::surface::panel::PanelContent::DmenuRequest
    ) {
        effects.push(PanelRuntimeEffect::RequestSearchFocus);
    }
    effects
}
```

- [ ] **Step 5: Run focused tests and verify they pass**

Run:

```bash
cargo test app::panel_runtime::tests::opening_a_panel_replaces_the_active_panel app::panel_runtime::tests::toggling_the_same_panel_closes_it app::panel_runtime::tests::installed_apps_menu_returns_load_effect_before_opening_panel -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Remove duplicated lifecycle tests from `src/app/panel_coordinator.rs`**

Delete the `#[cfg(test)] mod tests` block from `src/app/panel_coordinator.rs`. The lifecycle behavior is now covered by `PanelRuntime` tests.

- [ ] **Step 7: Commit**

```bash
git add crates/niribar/src/app/panel_runtime.rs src/app/panel_coordinator.rs
git commit -m "refactor: move panel lifecycle into panel runtime"
```

---

## Task 2: Wire App To PanelRuntime Lifecycle Effects

**Files:**
- Modify: `crates/niribar/src/app.rs`
- Modify: `crates/niribar/src/app/panel.rs`

- [ ] **Step 1: Change `App` to store `PanelRuntime`**

In `crates/niribar/src/app.rs`, remove:

```rust
mod panel_coordinator;
```

Ensure this module is present:

```rust
mod panel_runtime;
```

Change the `App` field:

```rust
panel: panel_runtime::PanelRuntime,
```

Change `App::boot` initialization:

```rust
panel: panel_runtime::PanelRuntime::default(),
```

- [ ] **Step 2: Run compile check and verify it fails at old `panel_coordinator` references**

Run:

```bash
cargo test --no-run
```

Expected: FAIL with references to `super::panel_coordinator` in `crates/niribar/src/app/panel.rs`.

- [ ] **Step 3: Replace `PanelOpenRequest` construction in `crates/niribar/src/app/panel.rs`**

Replace both old constructions:

```rust
let request = super::panel_coordinator::PanelOpenRequest {
    origin_window,
    origin,
    content,
    anchor,
};
```

with:

```rust
let request = super::panel_runtime::PanelOpenRequest {
    origin_window,
    origin,
    content,
    anchor,
};
```

Change the `open_panel_window` argument type from:

```rust
request: super::panel_coordinator::PanelOpenRequest,
```

to:

```rust
request: super::panel_runtime::PanelOpenRequest,
```

- [ ] **Step 4: Rename the coordinator effect adapter**

Rename:

```rust
fn apply_panel_coordinator_effects(
    &mut self,
    effects: Vec<super::panel_coordinator::PanelEffect>,
) -> Task<Message>
```

to:

```rust
fn apply_panel_runtime_effects(
    &mut self,
    effects: Vec<super::panel_runtime::PanelRuntimeEffect>,
) -> Task<Message>
```

In the match, replace old variants:

```rust
super::panel_coordinator::PanelEffect::OpenPanelWindow(request)
super::panel_coordinator::PanelEffect::ClosePanelWindow(window)
super::panel_coordinator::PanelEffect::RequestSearchFocus
super::panel_coordinator::PanelEffect::LoadInstalledAppsThenOpenPanel(pending)
```

with:

```rust
super::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(request)
super::panel_runtime::PanelRuntimeEffect::ClosePanelWindow(window)
super::panel_runtime::PanelRuntimeEffect::RequestSearchFocus
super::panel_runtime::PanelRuntimeEffect::LoadInstalledAppsThenOpenPanel(pending)
```

In the recursive branch, replace:

```rust
tasks.push(self.apply_panel_coordinator_effects(effects));
```

with:

```rust
tasks.push(self.apply_panel_runtime_effects(effects));
```

- [ ] **Step 5: Merge activation effect handling into the same adapter**

In `apply_panel_runtime_effects`, include these existing activation arms:

```rust
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
```

Delete the second older `fn apply_panel_runtime_effects` if it remains below `activate_panel_entry`.

- [ ] **Step 6: Update call sites in `crates/niribar/src/app/panel.rs`**

Replace:

```rust
self.apply_panel_coordinator_effects(effects)
```

with:

```rust
self.apply_panel_runtime_effects(effects)
```

Do this for `PanelEvent::Toggle`, `open_panel`, and the Installed Desktop Application continuation branch.

- [ ] **Step 7: Run compile check**

Run:

```bash
cargo test --no-run
```

Expected: PASS compile, or FAIL only because `src/app/panel_coordinator.rs` is now unused but still present. No `super::panel_coordinator` references should remain.

- [ ] **Step 8: Commit**

```bash
git add crates/niribar/src/app.rs crates/niribar/src/app/panel.rs
git commit -m "refactor: adapt app to panel runtime lifecycle effects"
```

---

## Task 3: Move Dmenu Selection Request Lifecycle Behind PanelRuntime

**Files:**
- Modify: `crates/niribar/src/app/panel_runtime.rs`
- Modify: `crates/niribar/src/app/panel.rs`

- [ ] **Step 1: Add failing Dmenu lifecycle tests to `crates/niribar/src/app/panel_runtime.rs`**

Append these tests inside the existing `#[cfg(test)] mod tests`:

```rust
fn dmenu_request() -> crate::ipc::protocol::DmenuRequest {
    crate::ipc::protocol::DmenuRequest {
        prompt: Some("Run".into()),
        entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
        allow_custom: true,
    }
}

fn ipc_responder_for_test() -> super::super::messages::IpcResponder {
    let (sender, _receiver) = tokio::sync::oneshot::channel();
    super::super::messages::ipc_responder(sender)
}

#[test]
fn dmenu_selection_rejects_when_another_request_is_active() {
    let mut runtime = PanelRuntime::default();
    let first = ipc_responder_for_test();
    let second = ipc_responder_for_test();
    assert!(runtime.begin_dmenu_request(first).is_ok());

    let effects = runtime.begin_dmenu_selection(dmenu_request(), second, Some(wid()));

    assert_eq!(effects.len(), 1);
    match &effects[0] {
        PanelRuntimeEffect::SendDmenuResponse {
            response: crate::ipc::protocol::IpcResponse::Rejected { reason },
            ..
        } => assert_eq!(reason, "another dmenu request is already active"),
        other => panic!("unexpected effect: {other:?}"),
    }
}

#[test]
fn dmenu_selection_rejects_when_no_bar_surface_exists() {
    let mut runtime = PanelRuntime::default();

    let effects = runtime.begin_dmenu_selection(dmenu_request(), ipc_responder_for_test(), None);

    assert_eq!(effects.len(), 1);
    match &effects[0] {
        PanelRuntimeEffect::SendDmenuResponse {
            response: crate::ipc::protocol::IpcResponse::Rejected { reason },
            ..
        } => assert_eq!(reason, "no bar surface is available"),
        other => panic!("unexpected effect: {other:?}"),
    }
    assert!(!runtime.has_dmenu_responder());
}

#[test]
fn dmenu_selection_opens_dmenu_panel_and_requests_search_focus() {
    let origin = wid();
    let mut runtime = PanelRuntime::default();

    let effects =
        runtime.begin_dmenu_selection(dmenu_request(), ipc_responder_for_test(), Some(origin));

    assert!(runtime.has_dmenu_responder());
    assert_eq!(
        effects,
        vec![
            PanelRuntimeEffect::OpenPanelWindow(super::PanelOpenRequest {
                origin_window: origin,
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::DmenuRequest,
                anchor: None,
            }),
            PanelRuntimeEffect::RequestSearchFocus,
        ]
    );
    assert!(runtime.panel_menu_snapshot().is_some());
}
```

- [ ] **Step 2: Run focused tests and verify they fail**

Run:

```bash
cargo test app::panel_runtime::tests::dmenu_selection_rejects_when_another_request_is_active app::panel_runtime::tests::dmenu_selection_rejects_when_no_bar_surface_exists app::panel_runtime::tests::dmenu_selection_opens_dmenu_panel_and_requests_search_focus -- --nocapture
```

Expected: FAIL because `SendDmenuResponse` and `begin_dmenu_selection` are not defined.

- [ ] **Step 3: Extend `PanelRuntimeEffect`**

Add these variants to `PanelRuntimeEffect`:

```rust
SendDmenuResponse {
    responder: super::messages::IpcResponder,
    response: crate::ipc::protocol::IpcResponse,
},
```

Because `IpcResponder` does not implement `PartialEq`, replace the derived `PartialEq` for `PanelRuntimeEffect` with this manual implementation:

```rust
#[derive(Debug, Clone)]
pub(super) enum PanelRuntimeEffect {
    OpenPanelWindow(PanelOpenRequest),
    ClosePanelWindow(iced::window::Id),
    RequestSearchFocus,
    LoadInstalledAppsThenOpenPanel(PendingPanelOpen),
    RunPanelAction(crate::config::PanelAction),
    LaunchInstalledApp(String),
    CompleteDmenu(crate::ipc::protocol::IpcResponse),
    SendDmenuResponse {
        responder: super::messages::IpcResponder,
        response: crate::ipc::protocol::IpcResponse,
    },
}

impl PartialEq for PanelRuntimeEffect {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::OpenPanelWindow(left),
                Self::OpenPanelWindow(right),
            ) => left == right,
            (
                Self::ClosePanelWindow(left),
                Self::ClosePanelWindow(right),
            ) => left == right,
            (Self::RequestSearchFocus, Self::RequestSearchFocus) => true,
            (
                Self::LoadInstalledAppsThenOpenPanel(left),
                Self::LoadInstalledAppsThenOpenPanel(right),
            ) => left == right,
            (
                Self::RunPanelAction(left),
                Self::RunPanelAction(right),
            ) => left == right,
            (
                Self::LaunchInstalledApp(left),
                Self::LaunchInstalledApp(right),
            ) => left == right,
            (
                Self::CompleteDmenu(left),
                Self::CompleteDmenu(right),
            ) => left == right,
            (
                Self::SendDmenuResponse { response: left, .. },
                Self::SendDmenuResponse { response: right, .. },
            ) => left == right,
            _ => false,
        }
    }
}
```

- [ ] **Step 4: Add `begin_dmenu_selection`**

Add this method to `impl PanelRuntime`:

```rust
pub(super) fn begin_dmenu_selection(
    &mut self,
    request: crate::ipc::protocol::DmenuRequest,
    responder: super::messages::IpcResponder,
    origin_window: Option<iced::window::Id>,
) -> Vec<PanelRuntimeEffect> {
    if self.dmenu_responder.is_some() {
        return vec![PanelRuntimeEffect::SendDmenuResponse {
            responder,
            response: crate::ipc::protocol::IpcResponse::Rejected {
                reason: "another dmenu request is already active".into(),
            },
        }];
    }

    let Some(origin_window) = origin_window else {
        return vec![PanelRuntimeEffect::SendDmenuResponse {
            responder,
            response: crate::ipc::protocol::IpcResponse::Rejected {
                reason: "no bar surface is available".into(),
            },
        }];
    };

    let mut effects = Vec::new();
    if let Some(window) = self.active_panel_window() {
        effects.push(PanelRuntimeEffect::ClosePanelWindow(window));
    }
    self.active_panel = None;
    self.session = None;
    self.menu_scroll_y = 0.0;
    self.menu_viewport_height = 0.0;
    self.search_focus_pending = false;
    self.open_session(super::panel_menu_session::PanelMenuSource::Dmenu(request));
    self.dmenu_responder = Some(responder);

    let open_request = PanelOpenRequest {
        origin_window,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content: crate::surface::panel::PanelContent::DmenuRequest,
        anchor: None,
    };

    effects.push(PanelRuntimeEffect::OpenPanelWindow(open_request));
    effects.push(PanelRuntimeEffect::RequestSearchFocus);
    effects
}
```

- [ ] **Step 5: Run focused tests and verify they pass**

Run:

```bash
cargo test app::panel_runtime::tests::dmenu_selection_rejects_when_another_request_is_active app::panel_runtime::tests::dmenu_selection_rejects_when_no_bar_surface_exists app::panel_runtime::tests::dmenu_selection_opens_dmenu_panel_and_requests_search_focus -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Replace `App::handle_dmenu_selection_request`**

Replace the body of `handle_dmenu_selection_request` in `crates/niribar/src/app/panel.rs` with:

```rust
let effects =
    self.panel
        .begin_dmenu_selection(request, responder, self.default_panel_origin_window());
self.apply_panel_runtime_effects(effects)
```

- [ ] **Step 7: Adapt new effects in `crates/niribar/src/app/panel.rs`**

Add these match arms to `apply_panel_runtime_effects`:

```rust
super::panel_runtime::PanelRuntimeEffect::SendDmenuResponse { responder, response } => {
    send_ipc_response(responder, response);
}
```

- [ ] **Step 8: Run Dmenu-related tests**

Run:

```bash
cargo test app::panel::tests::dmenu app::panel_runtime::tests::dmenu -- --nocapture
```

Expected: PASS. If the exact filter finds zero tests, run:

```bash
cargo test dmenu -- --nocapture
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/niribar/src/app/panel_runtime.rs crates/niribar/src/app/panel.rs
git commit -m "refactor: move dmenu panel lifecycle into panel runtime"
```

---

## Task 4: Delete PanelCoordinator

**Files:**
- Delete: `src/app/panel_coordinator.rs`
- Modify: `crates/niribar/src/app.rs`
- Modify: `crates/niribar/src/app/panel.rs`

- [ ] **Step 1: Search for remaining coordinator references**

Run:

```bash
rg "panel_coordinator|PanelCoordinator|PanelEffect" src
```

Expected before deletion: matches only in `src/app/panel_coordinator.rs` or stale references that must be replaced.

- [ ] **Step 2: Replace any stale references**

If `rg` reports `super::panel_coordinator::PanelOpenRequest`, replace it with:

```rust
super::panel_runtime::PanelOpenRequest
```

If `rg` reports `super::panel_coordinator::PanelEffect`, replace it with:

```rust
super::panel_runtime::PanelRuntimeEffect
```

If `crates/niribar/src/app.rs` still contains:

```rust
mod panel_coordinator;
```

delete that line.

- [ ] **Step 3: Delete the coordinator file**

Use:

```bash
git rm src/app/panel_coordinator.rs
```

Expected: `rm 'src/app/panel_coordinator.rs'`.

- [ ] **Step 4: Verify the coordinator is gone**

Run:

```bash
rg "panel_coordinator|PanelCoordinator|PanelEffect" src
```

Expected: no matches.

- [ ] **Step 5: Run compile check**

Run:

```bash
cargo test --no-run
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/niribar/src/app.rs crates/niribar/src/app/panel.rs crates/niribar/src/app/panel_runtime.rs
git commit -m "refactor: delete panel coordinator"
```

---

## Task 5: Tighten Panel Runtime Interface And Tests

**Files:**
- Modify: `crates/niribar/src/app/panel_runtime.rs`
- Modify: `crates/niribar/src/app/panel.rs`
- Modify: `crates/niribar/src/app/render.rs`

- [ ] **Step 1: Search for leaked runtime internals**

Run:

```bash
rg "panel\\.(session|active_panel|menu_scroll_y|menu_viewport_height|search_focus_pending|dmenu_responder)" crates/niribar/src/app
```

Expected: no matches. If matches appear in non-test code, use existing `PanelRuntime` methods instead.

- [ ] **Step 2: Search for direct lifecycle orchestration outside `PanelRuntime`**

Run:

```bash
rg "panel_window_opened|open_panel_with_menus|toggle_panel_with_menus|open_panel_after_installed_apps_loaded|begin_dmenu_selection|OpenPanelWindow|ClosePanelWindow" crates/niribar/src/app
```

Expected:

```text
crates/niribar/src/app/panel_runtime.rs
crates/niribar/src/app/panel.rs
```

Only `panel_runtime.rs` should decide lifecycle effects. `panel.rs` may adapt those effects.

- [ ] **Step 3: Run focused Panel tests**

Run:

```bash
cargo test app::panel_runtime::tests app::panel::tests app::render::tests app::surfaces::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Run formatting**

Run:

```bash
cargo fmt
```

Expected: completes with no output.

- [ ] **Step 5: Run full test suite**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/niribar/src/app/panel_runtime.rs crates/niribar/src/app/panel.rs crates/niribar/src/app/render.rs crates/niribar/src/app/surfaces.rs
git commit -m "refactor: tighten panel runtime interface"
```

---

## Self-Review

Spec coverage:

- **Panel Runtime owns full logical lifecycle:** Tasks 1-3 move active Panel tracking, open/toggle/close, Installed Desktop Application menu continuation, search focus, Panel Menu Session activation, and Dmenu Selection Request handling into `PanelRuntime`.
- **PanelCoordinator disappears:** Task 4 deletes `src/app/panel_coordinator.rs` and verifies no `PanelCoordinator` references remain.
- **`PanelOpenRequest` name stays:** Task 1 moves the type into `panel_runtime.rs` without renaming it.
- **App remains adapter:** Tasks 2-3 keep iced `Task`, layer-shell windows, overlay effects, Installed Desktop Application discovery, process launch, and physical IPC response sending in `panel.rs`.
- **Tests improve at the interface:** Tasks 1 and 3 add lifecycle tests directly against `PanelRuntime`; Task 5 runs focused and full suites.

Placeholder scan:

- The plan contains no `TBD`, `TODO`, or “implement later” placeholders.
- Every code-changing step includes exact code or exact replacement text.
- Every verification step includes exact commands and expected outcomes.

Type consistency:

- `PanelOpenRequest`, `PendingPanelOpen`, `ActivePanel`, `PanelRuntime`, and `PanelRuntimeEffect` are defined in `crates/niribar/src/app/panel_runtime.rs`.
- Later tasks use `PanelRuntimeEffect` consistently for lifecycle, menu activation, and Dmenu effects.
- `App` stores `panel_runtime::PanelRuntime` throughout the final state.
