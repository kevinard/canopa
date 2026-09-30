# Panel Attachment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an explicit **Panel Attachment** Seam so logical **Panel Lifecycle** state cannot be cleared by stale overlay window events from an older **Panel**.

**Architecture:** Keep **Panel Runtime** as the owner of logical **Panel Lifecycle** and interaction state. Keep **Bar Surface Registry** as the owner of overlay window classification, cleanup, and stale-window tracking. Connect them with `PanelAttachmentId`, carried from **Panel Runtime** into `PanelMeta` and `RegistryEffect::PanelClosed`, so close events clear runtime state only when they match the current logical **Panel**.

**Tech Stack:** Rust 2024, iced, iced_layershell, zbus, existing `rtk cargo test` suite.

---

## File Structure

- Modify: `crates/niribar/src/app/panel_runtime.rs`
  - Add `PanelAttachmentId`.
  - Make a logical **Panel** active when the open request is accepted, before the overlay window exists.
  - Track the optional attached overlay window on the active logical **Panel**.
  - Add attachment-gated handling for overlay window close notifications.
  - Own the current **Panel** render identity through a stored `PanelTree`.
- Modify: `crates/niribar/src/app/surfaces.rs`
  - Store `PanelAttachmentId` in `PanelMeta`.
  - Keep only overlay cleanup metadata in `PanelMeta`: attachment id and originating **Bar Surface** window.
  - Carry `PanelAttachmentId` in `RegistryEffect::PanelClosed`.
- Modify: `crates/niribar/src/app/overlay_adapter.rs`
  - Replace direct `self.panel.clear()` on `PanelClosed` with an attachment-gated `PanelRuntimeIntent::PanelWindowClosed`.
  - Update test helpers to register a panel popup with an attachment id.
- Modify: `crates/niribar/src/app/panel.rs`
  - Build full `PanelOpenRequest` values with origin **Bar Surface** render context before sending them to **Panel Runtime**.
  - Adapt `PanelRuntimeEffect::OpenPanelWindow` into layer-shell window creation, then attach the created window back to the same `PanelAttachmentId`.
  - Route Dmenu Selection Requests through the same request-building path.
- Modify: `crates/niribar/src/app/render.rs`
  - Read logical **Panel** identity from **Panel Runtime** via `PanelAttachmentId`.
  - Use **Panel Runtime**, not **Bar Surface Registry**, to mark active **Panel Origin** in **Bar Surface** rendering.
- Modify: `crates/niribar/src/app/view.rs`
  - No expected behavioral change; verify it still consumes `PanelRenderModel` unchanged.
- Modify: `CONTEXT.md`
  - Already updated with **Panel Lifecycle** and **Panel Attachment** language.
- Read: `docs/niribar/adr/0007-separate-panel-lifecycle-from-overlay-window-lifecycle.md`
  - Use this ADR as the guardrail. Do not move all overlay cleanup into **Panel Runtime** and do not move **Panel** interaction state into **Bar Surface Registry**.

## Task 1: Add Red Panel Runtime Attachment Tests

**Files:**
- Modify: `crates/niribar/src/app/panel_runtime.rs`

- [ ] **Step 1: Update the test `open_request` helper with render context**

Inside `#[cfg(test)] mod tests` in `crates/niribar/src/app/panel_runtime.rs`, replace the existing `open_request` helper with:

```rust
fn open_request(
    window: iced::window::Id,
    content: crate::surface::panel::PanelContent,
) -> super::PanelOpenRequest {
    super::PanelOpenRequest {
        origin_window: window,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content,
        anchor: None,
        origin_bar_name: "main".into(),
        origin_output_name: "DP-1".into(),
        screen_size: (1920, 1080),
    }
}
```

Expected initial result: compile fails because `PanelOpenRequest` does not yet have `origin_bar_name`, `origin_output_name`, or `screen_size`.

- [ ] **Step 2: Add a failing test for logical activation before window attachment**

Add this test in `crates/niribar/src/app/panel_runtime.rs` tests:

```rust
#[test]
fn opening_panel_is_logically_active_before_window_attaches() {
    let origin = wid();
    let mut runtime = PanelRuntime::default();

    let effects = open_panel(
        &mut runtime,
        open_request(origin, crate::surface::panel::PanelContent::BarPanel),
    );

    let [PanelRuntimeEffect::OpenPanelWindow(open)] = effects.as_slice() else {
        panic!("expected only OpenPanelWindow, got {effects:?}");
    };

    assert_eq!(runtime.active_panel_window(), None);
    assert_eq!(runtime.current_attachment_for_test(), Some(open.attachment_id));
    assert_eq!(runtime.active_panel_tree_for_test(), Some(&open.tree));
    assert_eq!(open.tree.origin_window, origin);
    assert_eq!(open.tree.origin_bar_name, "main");
    assert_eq!(open.tree.origin_output_name, "DP-1");
}
```

Expected initial result: compile fails because `PanelRuntimeEffect::OpenPanelWindow` still contains `PanelOpenRequest`, and `current_attachment_for_test` / `active_panel_tree_for_test` do not exist.

- [ ] **Step 3: Add a failing test for stale close protection**

Add:

```rust
#[test]
fn stale_panel_window_close_does_not_clear_replacement_panel() {
    let origin = wid();
    let first_window = wid();
    let second_window = wid();
    let mut runtime = PanelRuntime::default();

    let first_effects = open_panel(
        &mut runtime,
        open_request(origin, crate::surface::panel::PanelContent::BarPanel),
    );
    let [PanelRuntimeEffect::OpenPanelWindow(first_open)] = first_effects.as_slice() else {
        panic!("expected first OpenPanelWindow, got {first_effects:?}");
    };
    let first_attachment = first_open.attachment_id;
    runtime.handle(PanelRuntimeIntent::PanelWindowOpened {
        attachment_id: first_attachment,
        window: first_window,
    });

    let second_effects = open_panel(
        &mut runtime,
        open_request(
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        ),
    );
    let second_open = second_effects
        .iter()
        .find_map(|effect| match effect {
            PanelRuntimeEffect::OpenPanelWindow(open) => Some(open),
            _ => None,
        })
        .expect("replacement should open a second panel");
    let second_attachment = second_open.attachment_id;
    runtime.handle(PanelRuntimeIntent::PanelWindowOpened {
        attachment_id: second_attachment,
        window: second_window,
    });

    let effects = runtime.handle(PanelRuntimeIntent::PanelWindowClosed {
        attachment_id: first_attachment,
        window: first_window,
    });

    assert!(effects.is_empty());
    assert_eq!(runtime.current_attachment_for_test(), Some(second_attachment));
    assert_eq!(runtime.active_panel_window(), Some(second_window));
}
```

Expected initial result: compile fails because `PanelRuntimeIntent::PanelWindowClosed` does not exist.

- [ ] **Step 4: Add a failing test for current close clearing**

Add:

```rust
#[test]
fn current_panel_window_close_clears_runtime_state() {
    let origin = wid();
    let panel_window = wid();
    let mut runtime = PanelRuntime::default();

    let effects = open_panel(
        &mut runtime,
        open_request(origin, crate::surface::panel::PanelContent::BarPanel),
    );
    let [PanelRuntimeEffect::OpenPanelWindow(open)] = effects.as_slice() else {
        panic!("expected OpenPanelWindow, got {effects:?}");
    };
    runtime.handle(PanelRuntimeIntent::PanelWindowOpened {
        attachment_id: open.attachment_id,
        window: panel_window,
    });

    let close_effects = runtime.handle(PanelRuntimeIntent::PanelWindowClosed {
        attachment_id: open.attachment_id,
        window: panel_window,
    });

    assert!(close_effects.is_empty());
    assert_eq!(runtime.current_attachment_for_test(), None);
    assert_eq!(runtime.active_panel_window(), None);
    assert!(runtime.active_panel_tree_for_test().is_none());
}
```

Expected initial result: compile fails on the same missing attachment types and intent.

- [ ] **Step 5: Run the red tests**

Run:

```bash
rtk cargo test app::panel_runtime::tests::opening_panel_is_logically_active_before_window_attaches app::panel_runtime::tests::stale_panel_window_close_does_not_clear_replacement_panel app::panel_runtime::tests::current_panel_window_close_clears_runtime_state -- --nocapture
```

Expected: FAIL to compile on missing `PanelAttachmentId`, `PanelWindowOpenRequest`, attachment accessors, and `PanelRuntimeIntent::PanelWindowClosed`.

## Task 2: Implement Panel Runtime Attachment State

**Files:**
- Modify: `crates/niribar/src/app/panel_runtime.rs`

- [ ] **Step 1: Add attachment and open-window request types**

Near `PanelOpenRequest`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct PanelAttachmentId(u64);

impl PanelAttachmentId {
    #[cfg(test)]
    pub(super) fn for_test(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PanelWindowOpenRequest {
    pub(super) attachment_id: PanelAttachmentId,
    pub(super) tree: crate::surface::panel::PanelTree,
}
```

Expected: compile still fails until `PanelOpenRequest`, `PanelRuntimeEffect`, and `PanelRuntimeIntent` are updated.

- [ ] **Step 2: Expand `PanelOpenRequest` and add a `tree` method**

Replace `PanelOpenRequest` with:

```rust
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PanelOpenRequest {
    pub(super) origin_window: iced::window::Id,
    pub(super) origin: crate::surface::panel::PanelOrigin,
    pub(super) content: crate::surface::panel::PanelContent,
    pub(super) anchor: Option<iced::Rectangle>,
    pub(super) origin_bar_name: String,
    pub(super) origin_output_name: String,
    pub(super) screen_size: (u32, u32),
}

impl PanelOpenRequest {
    fn tree(&self) -> crate::surface::panel::PanelTree {
        crate::surface::panel::PanelTree {
            origin: self.origin.clone(),
            content: self.content.clone(),
            origin_window: self.origin_window,
            origin_bar_name: self.origin_bar_name.clone(),
            origin_output_name: self.origin_output_name.clone(),
            anchor: self.anchor,
            screen_size: self.screen_size,
        }
    }
}
```

Expected: compile fails at call sites that construct `PanelOpenRequest` without the new fields.

- [ ] **Step 3: Update runtime effect and intent shapes**

Change the relevant enum variants to:

```rust
pub(super) enum PanelRuntimeEffect {
    OpenPanelWindow(PanelWindowOpenRequest),
    ClosePanelWindow(iced::window::Id),
    RequestSearchFocus,
    LoadInstalledAppsThenOpenPanel(PendingPanelOpen),
    RunPanelAction(crate::config::PanelAction),
    LaunchInstalledApp(String),
    SendDmenuResponse {
        responder: super::messages::IpcResponder,
        response: crate::ipc::protocol::IpcResponse,
    },
}
```

Change the window lifecycle intents to:

```rust
PanelWindowOpened {
    attachment_id: PanelAttachmentId,
    window: iced::window::Id,
},
PanelWindowClosed {
    attachment_id: PanelAttachmentId,
    window: iced::window::Id,
},
```

Change `BeginDmenuSelection` to carry an already resolved panel request:

```rust
BeginDmenuSelection {
    request: crate::ipc::protocol::DmenuRequest,
    responder: super::messages::IpcResponder,
    panel_request: Option<PanelOpenRequest>,
},
```

Expected: compile fails in the match arms and tests until the implementations are updated.

- [ ] **Step 4: Replace `ActivePanel` with optional attachment**

Replace `ActivePanel` with:

```rust
#[derive(Debug, Clone, PartialEq)]
struct ActivePanel {
    attachment_id: PanelAttachmentId,
    window: Option<iced::window::Id>,
    tree: crate::surface::panel::PanelTree,
}
```

Add `next_attachment_id` to `PanelRuntime`:

```rust
#[derive(Default)]
pub(super) struct PanelRuntime {
    active_panel: Option<ActivePanel>,
    session: Option<super::panel_menu_session::PanelMenuSession>,
    module_detail_sessions: HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession>,
    menu_scroll_y: f32,
    menu_viewport_height: f32,
    search_focus_pending: bool,
    dmenu_responder: Option<super::messages::IpcResponder>,
    next_attachment_id: u64,
    #[cfg(test)]
    pub(super) installed_apps_for_test: Option<Vec<crate::desktop_apps::DesktopApplication>>,
    #[cfg(test)]
    pub(super) launched_installed_app_ids_for_test: Vec<String>,
}
```

Expected: compile fails until constructors and methods use the new fields.

- [ ] **Step 5: Add attachment helpers and read methods**

Inside `impl PanelRuntime`, add:

```rust
fn allocate_attachment_id(&mut self) -> PanelAttachmentId {
    self.next_attachment_id = self.next_attachment_id.saturating_add(1);
    PanelAttachmentId(self.next_attachment_id)
}

pub(super) fn active_panel_window(&self) -> Option<iced::window::Id> {
    self.active_panel.as_ref().and_then(|panel| panel.window)
}

pub(super) fn panel_tree_for_attachment(
    &self,
    attachment_id: PanelAttachmentId,
) -> Option<&crate::surface::panel::PanelTree> {
    self.active_panel
        .as_ref()
        .filter(|panel| panel.attachment_id == attachment_id)
        .map(|panel| &panel.tree)
}

pub(super) fn panel_tree_for_window(
    &self,
    window: iced::window::Id,
) -> Option<&crate::surface::panel::PanelTree> {
    self.active_panel
        .as_ref()
        .filter(|panel| panel.window == Some(window))
        .map(|panel| &panel.tree)
}

pub(super) fn active_panel_origin_for(
    &self,
    origin_window: iced::window::Id,
) -> Option<&crate::surface::panel::PanelOrigin> {
    self.active_panel
        .as_ref()
        .filter(|panel| panel.tree.origin_window == origin_window)
        .map(|panel| &panel.tree.origin)
}

pub(super) fn is_menu_panel_window(&self, window: iced::window::Id) -> bool {
    self.panel_tree_for_window(window).is_some_and(|tree| {
        matches!(
            tree.content,
            crate::surface::panel::PanelContent::Menu(_)
                | crate::surface::panel::PanelContent::DmenuRequest
        )
    })
}

pub(super) fn active_panel_tree(&self) -> Option<&crate::surface::panel::PanelTree> {
    self.active_panel.as_ref().map(|panel| &panel.tree)
}

#[cfg(test)]
pub(super) fn current_attachment_for_test(&self) -> Option<PanelAttachmentId> {
    self.active_panel.as_ref().map(|panel| panel.attachment_id)
}

#[cfg(test)]
pub(super) fn active_panel_tree_for_test(&self) -> Option<&crate::surface::panel::PanelTree> {
    self.active_panel_tree()
}
```

Expected: compile fails if a duplicate `active_panel_window` remains. Remove the older implementation when this helper is added.

- [ ] **Step 6: Update open, attach, and close logic**

Replace `panel_window_opened` with:

```rust
fn panel_window_opened(
    &mut self,
    attachment_id: PanelAttachmentId,
    window: iced::window::Id,
) {
    if let Some(panel) = self
        .active_panel
        .as_mut()
        .filter(|panel| panel.attachment_id == attachment_id)
    {
        panel.window = Some(window);
    }
}
```

Add:

```rust
fn panel_window_closed(
    &mut self,
    attachment_id: PanelAttachmentId,
    window: iced::window::Id,
) -> Vec<PanelRuntimeEffect> {
    let should_clear = self.active_panel.as_ref().is_some_and(|panel| {
        panel.attachment_id == attachment_id && panel.window == Some(window)
    });
    if should_clear {
        self.clear();
    }
    Vec::new()
}
```

Replace `open_panel_after_source_ready` with:

```rust
fn open_panel_after_source_ready(
    &mut self,
    request: PanelOpenRequest,
    module_detail_sessions: HashMap<
        crate::modules::ModuleId,
        crate::modules::ModuleDetailSession,
    >,
    menu_source: Option<super::panel_menu_session::PanelMenuSource>,
) -> Vec<PanelRuntimeEffect> {
    let mut effects = Vec::new();
    if let Some(window) = self.active_panel_window() {
        effects.push(PanelRuntimeEffect::ClosePanelWindow(window));
    }

    self.session = None;
    if let Some(source) = menu_source {
        self.open_session(source);
    }

    self.module_detail_sessions = module_detail_sessions;
    self.menu_scroll_y = 0.0;
    self.menu_viewport_height = 0.0;
    self.search_focus_pending = false;

    let tree = request.tree();
    let attachment_id = self.allocate_attachment_id();
    self.active_panel = Some(ActivePanel {
        attachment_id,
        window: None,
        tree: tree.clone(),
    });

    effects.push(PanelRuntimeEffect::OpenPanelWindow(PanelWindowOpenRequest {
        attachment_id,
        tree,
    }));
    if matches!(
        request.content,
        crate::surface::panel::PanelContent::Menu(_)
            | crate::surface::panel::PanelContent::DmenuRequest
    ) {
        effects.push(self.request_search_focus());
    }
    effects
}
```

Update `open_panel_with_catalog` to call the new signature:

```rust
let menu_source = if let crate::surface::panel::PanelContent::Menu(name) = &request.content {
    catalog.configured_menu_source(name)
} else {
    None
};
self.open_panel_after_source_ready(request, module_detail_sessions, menu_source)
```

Update `open_panel_after_installed_apps_loaded` to call:

```rust
let source = catalog.installed_apps_source(&pending.menu_name, apps);
self.open_panel_after_source_ready(pending.request, HashMap::new(), source)
```

Expected: compile fails at old callers that still pass `PanelWindowOpened` origin/content/catalog fields.

- [ ] **Step 7: Update the `handle` match arms**

Replace the `PanelWindowOpened` match arm with:

```rust
PanelRuntimeIntent::PanelWindowOpened {
    attachment_id,
    window,
} => {
    self.panel_window_opened(attachment_id, window);
    Vec::new()
}
```

Add the close arm:

```rust
PanelRuntimeIntent::PanelWindowClosed {
    attachment_id,
    window,
} => self.panel_window_closed(attachment_id, window),
```

Update `BeginDmenuSelection` to pass `panel_request` into `begin_dmenu_selection`.

Expected: compile fails until `begin_dmenu_selection` accepts the new argument.

- [ ] **Step 8: Update Dmenu selection opening**

Replace `begin_dmenu_selection` with:

```rust
fn begin_dmenu_selection(
    &mut self,
    request: crate::ipc::protocol::DmenuRequest,
    responder: super::messages::IpcResponder,
    panel_request: Option<PanelOpenRequest>,
) -> Vec<PanelRuntimeEffect> {
    if self.dmenu_responder.is_some() {
        return vec![PanelRuntimeEffect::SendDmenuResponse {
            responder,
            response: crate::ipc::protocol::IpcResponse::Rejected {
                reason: "another dmenu request is already active".into(),
            },
        }];
    }

    let Some(panel_request) = panel_request else {
        return vec![PanelRuntimeEffect::SendDmenuResponse {
            responder,
            response: crate::ipc::protocol::IpcResponse::Rejected {
                reason: "no bar surface is available".into(),
            },
        }];
    };

    self.dmenu_responder = Some(responder);
    self.open_panel_after_source_ready(
        panel_request,
        HashMap::new(),
        Some(super::panel_menu_session::PanelMenuSource::Dmenu(request)),
    )
}
```

Expected: Dmenu tests may fail if they still expect `BeginDmenuSelection { origin_window }`.

- [ ] **Step 9: Run Panel Runtime tests**

Run:

```bash
rtk cargo test app::panel_runtime::tests -- --nocapture
```

Expected: Panel Runtime tests compile and pass after updating old expected `OpenPanelWindow` values to `PanelWindowOpenRequest { attachment_id, tree }`. For tests that compare attachment ids, capture the id from the emitted effect instead of hard-coding it.

- [ ] **Step 10: Commit**

```bash
rtk git add crates/niribar/src/app/panel_runtime.rs
rtk git commit -m "refactor(panel): add panel attachment runtime state"
```

## Task 3: Build Full Panel Open Requests In App

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`

- [ ] **Step 1: Add an App helper that resolves Panel render context**

In `impl App` in `crates/niribar/src/app/panel.rs`, add:

```rust
pub(super) fn panel_open_request(
    &self,
    origin_window: iced::window::Id,
    origin: crate::surface::panel::PanelOrigin,
    content: crate::surface::panel::PanelContent,
    anchor: Option<iced::Rectangle>,
) -> super::panel_runtime::PanelOpenRequest {
    let origin_surface = self
        .surface(origin_window)
        .cloned()
        .unwrap_or_default();
    super::panel_runtime::PanelOpenRequest {
        origin_window,
        origin,
        content,
        anchor,
        origin_bar_name: origin_surface.bar_name,
        origin_output_name: origin_surface.output_name,
        screen_size: origin_surface.screen_size,
    }
}
```

Expected: compile passes for this helper but old request construction sites still fail.

- [ ] **Step 2: Update `open_panel` to use the helper**

Replace the request construction in `open_panel` with:

```rust
let request = self.panel_open_request(origin_window, origin, content, anchor);
```

Expected: compile may fail in `module_detail_sessions_for_request` if it reads fields that moved or changed. Keep field reads through `request.origin_window`, `request.origin`, and `request.content`.

- [ ] **Step 3: Update Panel toggle event request construction**

In the `PanelEvent::Toggle` arm, replace the request literal with:

```rust
let request = self.panel_open_request(origin_window, origin, content, anchor);
```

Expected: compile should move to tests and Dmenu call sites.

- [ ] **Step 4: Update Dmenu Selection Request routing**

Replace `handle_dmenu_selection_request` with:

```rust
pub(super) fn handle_dmenu_selection_request(
    &mut self,
    request: crate::ipc::protocol::DmenuRequest,
    responder: super::messages::IpcResponder,
) -> Task<Message> {
    let panel_request = self.default_panel_origin_window().map(|origin_window| {
        self.panel_open_request(
            origin_window,
            crate::surface::panel::PanelOrigin::Bar,
            crate::surface::panel::PanelContent::DmenuRequest,
            None,
        )
    });
    let effects = self.panel.handle(
        super::panel_runtime::PanelRuntimeIntent::BeginDmenuSelection {
            request,
            responder,
            panel_request,
        },
    );
    self.apply_panel_runtime_effects(effects)
}
```

Expected: compile fails at Dmenu tests that still construct `BeginDmenuSelection` with `origin_window`.

- [ ] **Step 5: Update `open_panel_window` to consume `PanelWindowOpenRequest`**

Replace the `open_panel_window` signature and body start with:

```rust
fn open_panel_window(
    &mut self,
    request: super::panel_runtime::PanelWindowOpenRequest,
) -> Task<Message> {
    let tree = request.tree.clone();
    let origin_surface = self
        .surface(tree.origin_window)
        .cloned()
        .unwrap_or_default();
    let bar = Self::bar_for_surface_name(&self.config.bars, &tree.origin_bar_name);
    let surface_origin = self.inferred_panel_surface_origin(&origin_surface, bar);
    let settings = panel_window_settings(bar, &tree, surface_origin);
    let (panel_window, open_task) = Message::layershell_open(settings);
    self.surface_mut(tree.origin_window).tooltip_pending = None;
```

Replace the `PanelMeta` construction in the same function with:

```rust
let panel_meta = super::surfaces::PanelMeta {
    attachment_id: request.attachment_id,
    origin_window: tree.origin_window,
};
```

Replace the `PanelWindowOpened` feedback with:

```rust
self.panel.handle(
    super::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
        attachment_id: request.attachment_id,
        window: panel_window,
    },
);
```

Expected: compile fails until `PanelMeta` is updated in `crates/niribar/src/app/surfaces.rs`.

- [ ] **Step 6: Run focused App panel tests**

Run:

```bash
rtk cargo test app::panel::tests::global_panel_toggle_opens_default_panel app::panel::tests::dmenu_selection_request_opens_panel app::panel::tests::escape_that_clears_search_requests_search_refocus -- --nocapture
```

Expected: fail to compile until registry metadata and render paths are updated in later tasks. The failures should point at `PanelMeta`, `PanelRuntimeIntent::PanelWindowOpened`, and `PanelRuntimeIntent::BeginDmenuSelection` call sites.

## Task 4: Carry Panel Attachment Through Bar Surface Registry

**Files:**
- Modify: `crates/niribar/src/app/surfaces.rs`
- Modify: `crates/niribar/src/app/overlay_adapter.rs`

- [ ] **Step 1: Import `PanelAttachmentId` into `surfaces.rs`**

Near the top of `crates/niribar/src/app/surfaces.rs`, add:

```rust
use super::panel_runtime::PanelAttachmentId;
```

Expected: compile fails until `PanelAttachmentId` is public within `app`.

- [ ] **Step 2: Replace `PanelMeta`**

Replace `PanelMeta` with:

```rust
#[derive(Debug, Clone)]
pub(super) struct PanelMeta {
    pub(super) attachment_id: PanelAttachmentId,
    pub(super) origin_window: iced::window::Id,
}
```

Expected: compile fails at every `PanelMeta { tree, search_input_id }` construction.

- [ ] **Step 3: Carry attachment id in `RegistryEffect::PanelClosed`**

Change the variant to:

```rust
PanelClosed {
    window: iced::window::Id,
    attachment_id: PanelAttachmentId,
},
```

Expected: compile fails at every `RegistryEffect::PanelClosed { window }` pattern and construction.

- [ ] **Step 4: Update registry close helpers**

In `close_overlays_for_bar_surface`, change the panel filtering to use `origin_window` and emit attachment ids:

```rust
let closing_panels: Vec<_> = self
    .panel_popups
    .iter()
    .filter_map(|(window, meta)| (meta.origin_window == target_surface).then_some(*window))
    .collect();
for window in &closing_panels {
    if let Some(meta) = self.panel_popups.remove(window) {
        self.closing_panel_popups.insert(*window);
        effects.push(RegistryEffect::PanelClosed {
            window: *window,
            attachment_id: meta.attachment_id,
        });
    }
}
if !closing_panels.is_empty() {
    effects.push(RegistryEffect::RestoreBarKeyboardInteractivity);
}
```

Replace `close_panel_overlays_without_restore` with:

```rust
fn close_panel_overlays_without_restore(&mut self) -> Vec<RegistryEffect> {
    self.panel_popups
        .drain()
        .map(|(window, meta)| {
            self.closing_panel_popups.insert(window);
            RegistryEffect::PanelClosed {
                window,
                attachment_id: meta.attachment_id,
            }
        })
        .collect()
}
```

Replace the panel branch in `window_closed` with:

```rust
if let Some(meta) = self.panel_popups.remove(&window) {
    return vec![
        RegistryEffect::PanelClosed {
            window,
            attachment_id: meta.attachment_id,
        },
        RegistryEffect::RestoreBarKeyboardInteractivity,
    ];
}
```

Expected: compile fails in tests that still assert old `PanelClosed` values.

- [ ] **Step 5: Update test helpers in `surfaces.rs`**

Add this helper inside `crates/niribar/src/app/surfaces.rs` tests:

```rust
fn panel_meta_for_test(
    attachment_id: super::panel_runtime::PanelAttachmentId,
    origin_window: iced::window::Id,
) -> PanelMeta {
    PanelMeta {
        attachment_id,
        origin_window,
    }
}
```

When constructing panel metadata in tests, use:

```rust
panel_meta_for_test(super::panel_runtime::PanelAttachmentId::for_test(1), origin)
```

Expected: compile fails at any remaining direct `PanelMeta { tree, search_input_id }` literals.

- [ ] **Step 6: Add registry test for attachment-carrying close effects**

Add this test in `crates/niribar/src/app/surfaces.rs` tests:

```rust
#[test]
fn panel_closed_effect_carries_attachment_id() {
    let origin = test_wid();
    let panel = test_wid();
    let attachment_id = super::panel_runtime::PanelAttachmentId::for_test(7);
    let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new(), true);
    registry.insert_panel_popup(panel, panel_meta_for_test(attachment_id, origin));

    let effects = registry.window_closed(panel);

    assert_eq!(
        effects,
        vec![
            RegistryEffect::PanelClosed {
                window: panel,
                attachment_id,
            },
            RegistryEffect::RestoreBarKeyboardInteractivity,
        ]
    );
}
```

Expected: fail until all `PanelClosed` constructors include attachment ids.

- [ ] **Step 7: Update overlay adapter test registration helper**

Replace `register_panel_popup` in `crates/niribar/src/app/overlay_adapter.rs` with:

```rust
#[cfg(test)]
pub(super) fn register_panel_popup(
    &mut self,
    window: iced::window::Id,
    origin_surface: iced::window::Id,
    attachment_id: super::panel_runtime::PanelAttachmentId,
) {
    self.surfaces.insert_panel_popup(
        window,
        super::surfaces::PanelMeta {
            attachment_id,
            origin_window: origin_surface,
        },
    );
}
```

Expected: compile fails at tests that still pass a `PanelTree` into `register_panel_popup`.

- [ ] **Step 8: Run registry and overlay adapter tests**

Run:

```bash
rtk cargo test app::surfaces::tests::panel_closed_effect_carries_attachment_id app::overlay_adapter::tests -- --nocapture
```

Expected: fail in `overlay_adapter` until `RegistryEffect::PanelClosed` is handled with attachment-gated runtime close.

## Task 5: Gate Overlay Adapter Panel Close By Attachment

**Files:**
- Modify: `crates/niribar/src/app/overlay_adapter.rs`

- [ ] **Step 1: Replace direct Panel Runtime clearing**

In `apply_registry_effects`, replace the `PanelClosed` arm with:

```rust
RegistryEffect::PanelClosed {
    window,
    attachment_id,
} => {
    self.panel.handle(
        super::panel_runtime::PanelRuntimeIntent::PanelWindowClosed {
            attachment_id,
            window,
        },
    );
    self.surfaces.remove_panel_popup(window);
    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
}
```

Expected: compile passes for this arm after `PanelRuntimeIntent::PanelWindowClosed` exists.

- [ ] **Step 2: Add stale-close regression test in `overlay_adapter.rs`**

Add this test inside `crates/niribar/src/app/overlay_adapter.rs` tests:

```rust
#[test]
fn stale_panel_closed_effect_does_not_clear_current_panel_runtime() {
    let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
    let origin = wid();
    let old_window = wid();
    let new_window = wid();

    let old_attachment = crate::app::panel_runtime::PanelAttachmentId::for_test(1);
    let new_attachment = crate::app::panel_runtime::PanelAttachmentId::for_test(2);

    app.panel.handle(
        crate::app::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
            attachment_id: new_attachment,
            window: new_window,
        },
    );
    app.register_panel_popup(old_window, origin, old_attachment);
    app.register_panel_popup(new_window, origin, new_attachment);

    let _ = app.apply_registry_effects(vec![super::super::surfaces::RegistryEffect::PanelClosed {
        window: old_window,
        attachment_id: old_attachment,
    }]);

    assert_eq!(app.panel.active_panel_window(), Some(new_window));
    assert!(app.surfaces.panel_popup(old_window).is_none());
    assert!(app.surfaces.panel_popup(new_window).is_some());
}
```

Expected initial result: fail because the test attaches a window without first creating an active logical panel. Fix the setup in Step 3.

- [ ] **Step 3: Replace the test setup with a real runtime open**

Use this setup in the test before registering popups:

```rust
let old_request = app.panel_open_request(
    origin,
    crate::surface::panel::PanelOrigin::Bar,
    crate::surface::panel::PanelContent::BarPanel,
    None,
);
let old_effects = app.panel.handle(
    crate::app::panel_runtime::PanelRuntimeIntent::Open {
        request: old_request,
        catalog: crate::app::panel_runtime::PanelMenuCatalog::new(app.config.menus.as_slice()),
        module_detail_sessions: std::collections::HashMap::new(),
    },
);
let old_attachment = old_effects
    .iter()
    .find_map(|effect| match effect {
        crate::app::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(open) => {
            Some(open.attachment_id)
        }
        _ => None,
    })
    .expect("old panel should request a window");
app.panel.handle(
    crate::app::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
        attachment_id: old_attachment,
        window: old_window,
    },
);

let new_request = app.panel_open_request(
    origin,
    crate::surface::panel::PanelOrigin::Bar,
    crate::surface::panel::PanelContent::Menu("main".into()),
    None,
);
let new_effects = app.panel.handle(
    crate::app::panel_runtime::PanelRuntimeIntent::Open {
        request: new_request,
        catalog: crate::app::panel_runtime::PanelMenuCatalog::new(app.config.menus.as_slice()),
        module_detail_sessions: std::collections::HashMap::new(),
    },
);
let new_attachment = new_effects
    .iter()
    .find_map(|effect| match effect {
        crate::app::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(open) => {
            Some(open.attachment_id)
        }
        _ => None,
    })
    .expect("new panel should request a window");
app.panel.handle(
    crate::app::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
        attachment_id: new_attachment,
        window: new_window,
    },
);
```

Expected: the stale close test passes after direct `panel.clear()` is removed.

- [ ] **Step 4: Add current-close regression test**

Add:

```rust
#[test]
fn current_panel_closed_effect_clears_current_panel_runtime() {
    let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
    let origin = wid();
    let window = wid();
    let request = app.panel_open_request(
        origin,
        crate::surface::panel::PanelOrigin::Bar,
        crate::surface::panel::PanelContent::BarPanel,
        None,
    );
    let effects = app.panel.handle(
        crate::app::panel_runtime::PanelRuntimeIntent::Open {
            request,
            catalog: crate::app::panel_runtime::PanelMenuCatalog::new(app.config.menus.as_slice()),
            module_detail_sessions: std::collections::HashMap::new(),
        },
    );
    let attachment_id = effects
        .iter()
        .find_map(|effect| match effect {
            crate::app::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(open) => {
                Some(open.attachment_id)
            }
            _ => None,
        })
        .expect("panel should request a window");
    app.panel.handle(
        crate::app::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
            attachment_id,
            window,
        },
    );
    app.register_panel_popup(window, origin, attachment_id);

    let _ = app.apply_registry_effects(vec![super::super::surfaces::RegistryEffect::PanelClosed {
        window,
        attachment_id,
    }]);

    assert_eq!(app.panel.active_panel_window(), None);
    assert!(app.panel.active_panel_tree().is_none());
    assert!(app.surfaces.panel_popup(window).is_none());
}
```

Expected: pass after attachment-gated close is implemented.

- [ ] **Step 5: Run adapter tests**

Run:

```bash
rtk cargo test app::overlay_adapter::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/niribar/src/app/surfaces.rs crates/niribar/src/app/overlay_adapter.rs
rtk git commit -m "refactor(panel): gate overlay closes by panel attachment"
```

## Task 6: Move Panel Read Paths To Panel Runtime

**Files:**
- Modify: `crates/niribar/src/app/render.rs`
- Modify: `crates/niribar/src/app/panel.rs`
- Modify: `crates/niribar/src/app/surfaces.rs`

- [ ] **Step 1: Update `panel_render_model` to join through `PanelAttachmentId`**

In `crates/niribar/src/app/render.rs`, replace the start of `panel_render_model` with:

```rust
let meta = self.surfaces.panel_popup(window)?;
let tree = self.panel.panel_tree_for_attachment(meta.attachment_id)?;
let bar = Self::bar_for_surface_name(&self.config.bars, &tree.origin_bar_name);
let origin_surface = self
    .surfaces
    .surface(tree.origin_window)
    .cloned()
    .unwrap_or_else(|| {
        crate::surface::SurfaceState::new(
            tree.origin_output_name.clone(),
            tree.origin_bar_name.clone(),
        )
    });
```

Replace the `search_input_id` field assignment with:

```rust
search_input_id: super::panel::panel_search_input_id(window),
```

Expected: compile fails if `PanelRenderModel` still tries to read `meta.search_input_id`.

- [ ] **Step 2: Update active Panel Origin highlighting**

In `bar_render_model`, replace:

```rust
let active = self.surfaces.active_panel_origin_for(window)
    == Some(&crate::surface::panel::PanelOrigin::Module(id.clone()));
```

with:

```rust
let active = self.panel.active_panel_origin_for(window)
    == Some(&crate::surface::panel::PanelOrigin::Module(id.clone()));
```

Expected: compile passes after `PanelRuntime::active_panel_origin_for` exists.

- [ ] **Step 3: Remove logical Panel identity lookup from `SurfaceManager`**

Delete `SurfaceManager::active_panel_origin_for`.

Expected: compile fails at any remaining caller. Replace remaining callers with `self.panel.active_panel_origin_for(...)`.

- [ ] **Step 4: Update panel focus gained handling**

In `crates/niribar/src/app/panel.rs`, replace `handle_panel_focus_gained` with:

```rust
pub(super) fn handle_panel_focus_gained(&mut self, window: iced::window::Id) -> Task<Message> {
    if self.panel.is_menu_panel_window(window) {
        let effects = self
            .panel
            .handle(super::panel_runtime::PanelRuntimeIntent::RequestSearchFocus);
        return self.apply_panel_runtime_effects(effects);
    }
    Task::none()
}
```

Expected: compile passes after `PanelRuntime::is_menu_panel_window` exists.

- [ ] **Step 5: Update search focus task lookup**

In `focus_panel_search_input`, replace the `self.surfaces.panel_popup_values().find_map(...)` block with:

```rust
let Some(panel_window) = self.panel.active_panel_window() else {
    self.panel
        .handle(super::panel_runtime::PanelRuntimeIntent::SearchFocusSettled);
    return Task::none();
};
let search_input_id = panel_search_input_id(panel_window);
```

Expected: menu search focus tests continue to pass because search input ids are deterministic from the panel window.

- [ ] **Step 6: Update Panel Action dispatch from active Panel**

Replace `dispatch_panel_action_from_active_panel` with:

```rust
pub(super) fn dispatch_panel_action_from_active_panel(
    &mut self,
    action: crate::config::PanelAction,
) -> Task<Message> {
    let Some(tree) = self.panel.active_panel_tree().cloned() else {
        return Task::none();
    };
    self.dispatch_panel_action(tree.origin_window, tree.origin, action)
}
```

Expected: compile passes after `active_panel_tree` is `pub(super)`.

- [ ] **Step 7: Update tests that register panel popups for rendering**

Where tests previously call:

```rust
app.register_panel_popup(panel, origin, tree);
```

replace the setup with:

```rust
let request = app.panel_open_request(
    origin,
    tree.origin.clone(),
    tree.content.clone(),
    tree.anchor,
);
let effects = app.panel.handle(
    crate::app::panel_runtime::PanelRuntimeIntent::Open {
        request,
        catalog: crate::app::panel_runtime::PanelMenuCatalog::new(app.config.menus.as_slice()),
        module_detail_sessions: std::collections::HashMap::new(),
    },
);
let attachment_id = effects
    .iter()
    .find_map(|effect| match effect {
        crate::app::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(open) => {
            Some(open.attachment_id)
        }
        _ => None,
    })
    .expect("panel should request a window");
app.panel.handle(
    crate::app::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
        attachment_id,
        window: panel,
    },
);
app.register_panel_popup(panel, origin, attachment_id);
```

Expected: render tests compile after helper signatures are updated.

- [ ] **Step 8: Add stale render test**

Add this test in `crates/niribar/src/app/render.rs` tests:

```rust
#[test]
fn panel_render_model_ignores_stale_panel_window_attachment() {
    let (mut app, _) = App::boot(Config::default(), vec![]);
    let origin = test_wid();
    let stale_panel = test_wid();
    let current_panel = test_wid();

    let stale_request = app.panel_open_request(
        origin,
        crate::surface::panel::PanelOrigin::Bar,
        crate::surface::panel::PanelContent::BarPanel,
        None,
    );
    let stale_effects = app.panel.handle(
        crate::app::panel_runtime::PanelRuntimeIntent::Open {
            request: stale_request,
            catalog: crate::app::panel_runtime::PanelMenuCatalog::new(app.config.menus.as_slice()),
            module_detail_sessions: std::collections::HashMap::new(),
        },
    );
    let stale_attachment = stale_effects
        .iter()
        .find_map(|effect| match effect {
            crate::app::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(open) => {
                Some(open.attachment_id)
            }
            _ => None,
        })
        .expect("stale panel should request a window");
    app.panel.handle(
        crate::app::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
            attachment_id: stale_attachment,
            window: stale_panel,
        },
    );
    app.register_panel_popup(stale_panel, origin, stale_attachment);

    let current_request = app.panel_open_request(
        origin,
        crate::surface::panel::PanelOrigin::Bar,
        crate::surface::panel::PanelContent::Menu("main".into()),
        None,
    );
    let current_effects = app.panel.handle(
        crate::app::panel_runtime::PanelRuntimeIntent::Open {
            request: current_request,
            catalog: crate::app::panel_runtime::PanelMenuCatalog::new(app.config.menus.as_slice()),
            module_detail_sessions: std::collections::HashMap::new(),
        },
    );
    let current_attachment = current_effects
        .iter()
        .find_map(|effect| match effect {
            crate::app::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(open) => {
                Some(open.attachment_id)
            }
            _ => None,
        })
        .expect("current panel should request a window");
    app.panel.handle(
        crate::app::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
            attachment_id: current_attachment,
            window: current_panel,
        },
    );
    app.register_panel_popup(current_panel, origin, current_attachment);

    assert!(app.panel_render_model(stale_panel).is_none());
    assert!(app.panel_render_model(current_panel).is_some());
}
```

Expected: PASS after render joins through `PanelAttachmentId`.

- [ ] **Step 9: Run render and panel tests**

Run:

```bash
rtk cargo test app::render::tests app::panel::tests -- --nocapture
```

Expected: PASS after all test helpers and request construction sites are updated.

- [ ] **Step 10: Commit**

```bash
rtk git add crates/niribar/src/app/render.rs crates/niribar/src/app/panel.rs crates/niribar/src/app/surfaces.rs crates/niribar/src/app/overlay_adapter.rs
rtk git commit -m "refactor(panel): read panel identity from runtime attachment"
```

## Task 7: Add End-To-End Replacement Regression

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`

- [ ] **Step 1: Add App-level replacement regression test**

Add this test in `crates/niribar/src/app/panel.rs` tests:

```rust
#[test]
fn replacing_panel_ignores_stale_old_window_close() {
    let config = config_with_menu(menu_with_two_entries());
    let (mut app, _) = App::boot(config, vec![]);
    let origin = iced::window::Id::unique();
    app.surface_mut(origin).bar_name = "main".into();
    app.surface_mut(origin).output_name = "DP-1".into();
    app.surface_mut(origin).screen_size = (1920, 1080);

    let first_window = iced::window::Id::unique();
    let first_request = app.panel_open_request(
        origin,
        crate::surface::panel::PanelOrigin::Bar,
        crate::surface::panel::PanelContent::BarPanel,
        None,
    );
    let first_effects = app.panel.handle(super::super::panel_runtime::PanelRuntimeIntent::Open {
        request: first_request,
        catalog: super::super::panel_runtime::PanelMenuCatalog::new(app.config.menus.as_slice()),
        module_detail_sessions: std::collections::HashMap::new(),
    });
    let first_attachment = first_effects
        .iter()
        .find_map(|effect| match effect {
            super::super::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(open) => {
                Some(open.attachment_id)
            }
            _ => None,
        })
        .expect("first panel should request a window");
    app.panel.handle(super::super::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
        attachment_id: first_attachment,
        window: first_window,
    });
    app.register_panel_popup(first_window, origin, first_attachment);

    let second_window = iced::window::Id::unique();
    let second_request = app.panel_open_request(
        origin,
        crate::surface::panel::PanelOrigin::Bar,
        crate::surface::panel::PanelContent::Menu("main".into()),
        None,
    );
    let second_effects = app.panel.handle(super::super::panel_runtime::PanelRuntimeIntent::Open {
        request: second_request,
        catalog: super::super::panel_runtime::PanelMenuCatalog::new(app.config.menus.as_slice()),
        module_detail_sessions: std::collections::HashMap::new(),
    });
    let second_attachment = second_effects
        .iter()
        .find_map(|effect| match effect {
            super::super::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(open) => {
                Some(open.attachment_id)
            }
            _ => None,
        })
        .expect("second panel should request a window");
    app.panel.handle(super::super::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
        attachment_id: second_attachment,
        window: second_window,
    });
    app.register_panel_popup(second_window, origin, second_attachment);

    let _ = app.apply_registry_effects(vec![super::super::surfaces::RegistryEffect::PanelClosed {
        window: first_window,
        attachment_id: first_attachment,
    }]);

    assert_eq!(app.panel.active_panel_window(), Some(second_window));
    assert!(app.panel.panel_tree_for_attachment(second_attachment).is_some());
    assert!(app.panel.panel_tree_for_attachment(first_attachment).is_none());
    assert!(app.surfaces.panel_popup(first_window).is_none());
    assert!(app.surfaces.panel_popup(second_window).is_some());
}
```

Expected: PASS after Tasks 2-6.

- [ ] **Step 2: Run focused regression**

Run:

```bash
rtk cargo test app::panel::tests::replacing_panel_ignores_stale_old_window_close -- --nocapture
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
rtk git add crates/niribar/src/app/panel.rs
rtk git commit -m "test(panel): cover stale replacement window close"
```

## Task 8: Cleanup And Verification

**Files:**
- Modify: `crates/niribar/src/app/panel_runtime.rs`
- Modify: `crates/niribar/src/app/surfaces.rs`
- Modify: `crates/niribar/src/app/overlay_adapter.rs`
- Modify: `crates/niribar/src/app/panel.rs`
- Modify: `crates/niribar/src/app/render.rs`

- [ ] **Step 1: Remove stale direct clear and old identity lookups**

Run:

```bash
rtk rg -n "RegistryEffect::PanelClosed \\{ window \\}|self\\.panel\\.clear\\(\\)|active_panel_origin_for\\(|PanelMeta \\{[^\\n]*(tree|search_input_id)|PanelWindowOpened \\{[^\\n]*(origin_window|origin:|content:|catalog:)" crates/niribar/src/app
```

Expected: no matches for:

```text
RegistryEffect::PanelClosed { window }
self.panel.clear()
SurfaceManager::active_panel_origin_for(...)
PanelMeta { tree, ... }
PanelMeta { search_input_id, ... }
PanelWindowOpened { origin_window, origin, content, catalog }
```

If the `rg` command reports a legitimate `PanelRuntime::clear` method definition or internal call, keep it. The rejected pattern is external Adapter code clearing runtime state without an attachment check.

- [ ] **Step 2: Run focused test suites**

Run:

```bash
rtk cargo test app::panel_runtime::tests -- --nocapture
rtk cargo test app::surfaces::tests -- --nocapture
rtk cargo test app::overlay_adapter::tests -- --nocapture
rtk cargo test app::panel::tests -- --nocapture
rtk cargo test app::render::tests -- --nocapture
```

Expected: PASS for each focused suite.

- [ ] **Step 3: Run full test suite**

Run:

```bash
rtk cargo test
```

Expected: PASS.

- [ ] **Step 4: Review docs against implementation**

Run:

```bash
rtk sed -n '120,150p' CONTEXT.md
rtk sed -n '336,348p' CONTEXT.md
rtk sed -n '1,220p' docs/niribar/adr/0007-separate-panel-lifecycle-from-overlay-window-lifecycle.md
```

Expected: docs still match the implementation:

```text
Panel Runtime owns logical Panel Lifecycle.
Bar Surface Registry owns overlay window metadata, classification, cleanup, and stale-window tracking.
Panel Attachment connects the current logical Panel to its overlay window.
Stale old-window close events do not clear newer logical Panel state.
```

- [ ] **Step 5: Commit final cleanup**

```bash
rtk git add crates/niribar/src/app/panel_runtime.rs crates/niribar/src/app/surfaces.rs crates/niribar/src/app/overlay_adapter.rs crates/niribar/src/app/panel.rs crates/niribar/src/app/render.rs CONTEXT.md docs/niribar/adr/0007-separate-panel-lifecycle-from-overlay-window-lifecycle.md docs/niribar/superpowers/plans/2026-06-03-panel-attachment.md
rtk git commit -m "refactor(panel): separate lifecycle from overlay attachment"
```

## Self-Review

**Spec coverage:** The plan implements ADR-0007 by keeping **Panel Runtime** and **Bar Surface Registry** responsibilities separate, adding an explicit **Panel Attachment**, and testing stale replacement windows. The plan also uses the newly added **Panel Lifecycle** and **Panel Attachment** domain terms from `CONTEXT.md`.

**Placeholder scan:** The plan contains concrete file paths, concrete test names, concrete code snippets, concrete commands, and expected results. It does not use placeholder implementation steps.

**Type consistency:** The plan consistently uses `PanelAttachmentId`, `PanelWindowOpenRequest`, `PanelOpenRequest::tree`, `PanelRuntimeIntent::PanelWindowOpened { attachment_id, window }`, `PanelRuntimeIntent::PanelWindowClosed { attachment_id, window }`, `PanelMeta { attachment_id, origin_window }`, and `RegistryEffect::PanelClosed { window, attachment_id }`.
