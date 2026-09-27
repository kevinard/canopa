# Foreground Interactive Overlay Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deepen **Foreground** into the app runtime seam for interactive overlays so **Panel** and **Menu** lifecycle, physical overlay identity, replacement, stale close handling, active **Menu** interaction, and role classification live behind one pure intent/effect interface.

**Architecture:** **Foreground** becomes the first module crossed by App-level **Panel** and **Menu** open/toggle/close intents. It owns physical **Panel** and **Menu** window identity and emits effects for app-owned side work. **Panel Runtime** owns **Panel** content and interaction state only. **Bar Surface Registry** narrows to live **Bar Surface** records and passive **Tooltip** metadata. **Panel Attachment** collapses into internal **Foreground** state.

**Tech Stack:** Rust 2024, iced, iced_layershell, zbus, existing `rtk cargo test` suite.

---

## Scope

This plan migrates interactive overlay ownership in tracer-bullet slices:

1. **Foreground** owns physical **Panel** overlay identity and **Panel Lifecycle** open/toggle/close decisions.
2. **Foreground** owns physical **Menu** overlay identity and **Menu** replacement/stale-close behavior.
3. **Foreground** owns the active **Menu** **Selectable Menu Session** and `MenuSystem` disappears.
4. **Foreground** owns interactive overlay role snapshots for input routing.
5. **Bar Surface Registry** narrows to live **Bar Surface** records and passive **Tooltip** metadata.

The first implementation slice is intentionally Panel-first. It proves the new seam with the stale-close bug class while leaving **Menu** interaction internals mostly in place.

## Out of Scope

- Moving **Panel Menu** sessions out of **Panel Runtime** in the first slice.
- Moving **Dmenu Selection Request** selection behavior out of **Panel Runtime** in the first slice.
- Changing **Tooltip** behavior except for calling existing Tooltip close work through **Foreground** effects.
- Changing layer-shell placement math or visual layout.
- Changing `Command Policy`, **Configured Command**, or **Installed Desktop Application** behavior.
- Rewriting ADR-0007; it remains historical and superseded by ADR-0008.

## Current Decisions

- **Foreground** is a pure state machine module with an intent/effect interface.
- App remains the adapter for iced tasks, layer-shell windows, DBus calls, process launch, IPC responses, **Panel Runtime** content updates, and passive **Tooltip** close work.
- App-level **Panel** open/toggle/close intents cross **Foreground** before **Panel Runtime**.
- **Panel Runtime** does not emit physical **Panel** window open or close effects.
- **Panel Runtime** does not store physical **Panel** window IDs.
- **Panel Attachment** is internal **Foreground** state, not a separate app-level seam.
- **Foreground** is the primary test surface for replacement and stale close behavior.
- App tests verify adapter effect order and should not duplicate **Foreground** state-machine coverage.

## File Structure

- Modify: `crates/niribar/src/app/foreground.rs`
  - Add user-facing **Panel** intents.
  - Add pure effects for preparing/clearing **Panel Runtime**, opening/removing physical **Panel** windows, closing existing **Menu** windows, closing **Tooltips**, suspending/restoring bar keyboard interactivity, and role snapshots.
  - Move current **Panel Attachment** identity into private **Foreground** state if an identity token is still needed.
- Modify: `crates/niribar/src/app/panel_runtime.rs`
  - Remove physical **Panel** window ownership from active **Panel** state.
  - Remove physical open/close effects from `PanelRuntimeEffect`.
  - Replace open/toggle window effects with content preparation results/effects.
  - Keep **Selectable Menu Session**, **Module Detail Session**, search focus, menu scroll, activation, installed-app load, and **Dmenu Selection Request** responder behavior.
- Modify: `crates/niribar/src/app/panel.rs`
  - Route `PanelEvent::Open`, `PanelEvent::Toggle`, and `PanelEvent::Close` through **Foreground** first.
  - Adapt **Foreground** effects into layer-shell tasks and **Panel Runtime** content calls.
  - Keep physical window creation/removal in App as adapter work.
- Modify: `crates/niribar/src/app/overlay_adapter.rs`
  - Route physical **Panel** close events to **Foreground**.
  - Stop coordinating `PanelAttachment` as a separate module.
- Delete or empty after migration: `src/app/panel_attachment.rs`
  - Its behavior should be absorbed by **Foreground** tests and implementation.
- Modify: `crates/niribar/src/app/surfaces.rs`
  - Remove **Panel** overlay maps and closing sets in the first slice if practical.
  - If a short transitional map remains, mark it temporary and keep **Foreground** as the source of truth for physical **Panel** identity.
- Modify: `crates/niribar/src/app/surface_input_router.rs`
  - Keep pure routing, but have role classification read **Foreground** for **Panel** and **Menu** roles.
- Modify: `crates/niribar/src/app/render.rs`
  - Read active **Panel Origin** and active **Panel** render identity through **Foreground** / **Panel Runtime** seam rather than **Bar Surface Registry** metadata.
- Modify: `crates/niribar/src/app/view.rs`
  - Verify **Panel** view still consumes **Panel Runtime** render state without knowing physical window identity.
- Modify: `CONTEXT.md`
  - Already updated with the new **Foreground**, **Panel Runtime**, **Panel Attachment**, and **Bar Surface Registry** ownership language.
- Read: `docs/niribar/adr/0008-model-interactive-overlays-as-foreground.md`
  - Use this ADR as the guardrail.

---

## Slice 1: Foreground Owns Panel Physical Identity

### Task 1: Add Red Foreground Panel Lifecycle Tests

**Files:**
- Modify: `crates/niribar/src/app/foreground.rs`

- [ ] **Step 1: Add test helpers for Panel requests**

Add helpers that build a **Panel** request with origin **Bar Surface** render context and simple **Panel** content:

```rust
fn panel_request(
    origin_window: iced::window::Id,
    content: crate::surface::panel::PanelContent,
) -> super::PanelForegroundRequest {
    super::PanelForegroundRequest {
        origin_window,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content,
        anchor: None,
        origin_bar_name: "main".into(),
        origin_output_name: "DP-1".into(),
        screen_size: (1920, 1080),
    }
}
```

Expected initial result: compile fails because `PanelForegroundRequest` does not exist.

- [ ] **Step 2: Add a failing test for opening through Foreground first**

Add a test that sends `ForegroundIntent::OpenPanel { request }` and expects one effect to prepare **Panel Runtime** content, with no physical window opened until content preparation returns:

```rust
#[test]
fn opening_panel_prepares_panel_runtime_before_opening_window() {
    let origin = wid();
    let mut foreground = Foreground::default();
    let request = panel_request(origin, crate::surface::panel::PanelContent::BarPanel);

    let effects = foreground.handle(ForegroundIntent::OpenPanel {
        request: request.clone(),
    });

    assert_eq!(
        effects,
        vec![ForegroundEffect::PreparePanelRuntime {
            request,
            mode: PanelOpenMode::Open,
        }]
    );
    assert!(foreground.active_overlay().is_none());
}
```

Expected initial result: compile fails on missing intent, effect, request, and mode types.

- [ ] **Step 3: Add a failing test for prepared content becoming active before window attachment**

Add a test that sends `PanelRuntimePrepared` after the prepare effect:

```rust
#[test]
fn prepared_panel_is_foreground_before_window_opens() {
    let origin = wid();
    let mut foreground = Foreground::default();
    let tree = panel_tree(origin, crate::surface::panel::PanelContent::BarPanel);

    let effects = foreground.handle(ForegroundIntent::PanelRuntimePrepared {
        tree: tree.clone(),
    });

    assert_eq!(effects, vec![ForegroundEffect::OpenPanelWindow { tree: tree.clone() }]);
    assert_eq!(foreground.active_panel_tree(), Some(&tree));
    assert_eq!(foreground.active_panel_window(), None);
}
```

Expected initial result: compile fails because the prepared-content intent/effect does not exist.

- [ ] **Step 4: Add a failing test for replacing a Panel**

Add a test that attaches the first **Panel** window, then prepares a second **Panel**:

```rust
#[test]
fn replacing_panel_closes_old_window_and_opens_new_panel() {
    let origin = wid();
    let old_window = wid();
    let old_tree = panel_tree(origin, crate::surface::panel::PanelContent::BarPanel);
    let new_tree = panel_tree(
        origin,
        crate::surface::panel::PanelContent::Menu("main".into()),
    );
    let mut foreground = Foreground::default();

    foreground.handle(ForegroundIntent::PanelRuntimePrepared {
        tree: old_tree.clone(),
    });
    foreground.handle(ForegroundIntent::PanelWindowOpened { window: old_window });

    let effects = foreground.handle(ForegroundIntent::PanelRuntimePrepared {
        tree: new_tree.clone(),
    });

    assert_eq!(
        effects,
        vec![
            ForegroundEffect::RemoveWindow(old_window),
            ForegroundEffect::OpenPanelWindow { tree: new_tree.clone() },
        ]
    );
    assert_eq!(foreground.active_panel_tree(), Some(&new_tree));
    assert_eq!(foreground.active_panel_window(), None);
}
```

Expected initial result: compile fails on missing physical-window-free Panel lifecycle intents/effects.

- [ ] **Step 5: Add failing tests for stale and current Panel window close**

Add one test that closes the old window after a replacement and expects no state change. Add one test that closes the current window and expects Foreground to clear active **Panel** and emit a clear **Panel Runtime** effect.

Expected current-close effect shape:

```rust
vec![
    ForegroundEffect::ClearPanelRuntime,
    ForegroundEffect::RestoreBarKeyboardInteractivity,
]
```

Expected initial result: compile fails until **Foreground** owns close matching.

- [ ] **Step 6: Add a failing test for toggle same Panel**

Send `ForegroundIntent::TogglePanel { request }` for the active **Panel** and assert it removes the current physical window, clears **Panel Runtime**, and clears **Foreground**.

Expected initial result: compile fails until toggle decisions live in **Foreground**.

- [ ] **Step 7: Add a failing test for Panel opening while Menu is active**

Keep the existing test coverage that opening a **Panel** replaces a **Menu**, but assert the replacement is emitted through **Foreground** effects and that **Panel Runtime** preparation remains first where the **Panel** content is not yet ready.

Expected initial result: compile fails until **Foreground** owns the full replacement path.

- [ ] **Step 8: Run the red Foreground tests**

Run:

```bash
rtk cargo test app::foreground::tests::opening_panel_prepares_panel_runtime_before_opening_window \
  app::foreground::tests::prepared_panel_is_foreground_before_window_opens \
  app::foreground::tests::replacing_panel_closes_old_window_and_opens_new_panel \
  app::foreground::tests::stale_panel_window_close_does_not_clear_current_panel \
  app::foreground::tests::current_panel_window_close_clears_foreground_and_panel_runtime \
  app::foreground::tests::toggle_same_panel_closes_through_foreground -- --nocapture
```

Expected: FAIL to compile on missing **Foreground** intent/effect types.

### Task 2: Implement Foreground Panel State And Effects

**Files:**
- Modify: `crates/niribar/src/app/foreground.rs`

- [ ] **Step 1: Add Foreground Panel request and mode types**

Add a request type equivalent to the current `PanelOpenRequest`, but owned by the **Foreground** interface.

```rust
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PanelForegroundRequest {
    pub(super) origin_window: iced::window::Id,
    pub(super) origin: crate::surface::panel::PanelOrigin,
    pub(super) content: crate::surface::panel::PanelContent,
    pub(super) anchor: Option<iced::Rectangle>,
    pub(super) origin_bar_name: String,
    pub(super) origin_output_name: String,
    pub(super) screen_size: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelOpenMode {
    Open,
    Toggle,
}
```

- [ ] **Step 2: Add Foreground Panel intents**

Add:

- `OpenPanel { request: PanelForegroundRequest }`
- `TogglePanel { request: PanelForegroundRequest }`
- `ClosePanel`
- `PanelRuntimePrepared { tree: crate::surface::panel::PanelTree }`
- `PanelWindowOpened { window: iced::window::Id }`
- `PanelWindowClosed { window: iced::window::Id }`

- [ ] **Step 3: Add Foreground effects**

Add effects:

- `PreparePanelRuntime { request: PanelForegroundRequest, mode: PanelOpenMode }`
- `ClearPanelRuntime`
- `OpenPanelWindow { tree: crate::surface::panel::PanelTree }`
- `RemoveWindow(iced::window::Id)`
- `SuspendBarKeyboardInteractivity`
- `RestoreBarKeyboardInteractivity`
- `CloseTooltipsForSurface(iced::window::Id)`
- Existing **Menu** close effects preserved for transitional behavior.

- [ ] **Step 4: Make Panel attachment private to Foreground**

Represent active **Panel** as:

```rust
Panel {
    attachment_id: PanelAttachmentId,
    window: Option<iced::window::Id>,
    tree: crate::surface::panel::PanelTree,
}
```

`PanelAttachmentId` should be private to `foreground.rs` unless tests need a `cfg(test)` accessor.

- [ ] **Step 5: Implement open/toggle/close behavior**

Rules:

- `OpenPanel` emits `PreparePanelRuntime` and does not mutate active **Foreground** yet.
- `PanelRuntimePrepared` replaces active **Foreground** immediately, emits close effects for the previous active **Panel** or **Menu**, then emits `OpenPanelWindow`.
- `TogglePanel` closes the active **Panel** when request origin/content match the active **Panel**; otherwise it prepares a replacement.
- `ClosePanel` closes only if active **Foreground** is a **Panel**.
- `PanelWindowOpened` attaches only to the current active **Panel** without changing content state.
- `PanelWindowClosed` clears only when the physical window matches the current active **Panel**.

- [ ] **Step 6: Verify Foreground tests pass**

Run the focused `app::foreground::tests::*panel*` tests.

Expected: PASS.

### Task 3: Convert Panel Runtime To Content State Only

**Files:**
- Modify: `crates/niribar/src/app/panel_runtime.rs`

- [ ] **Step 1: Remove physical window fields from active Panel state**

Change `ActivePanel` so it no longer stores `window` or `attachment_id`.

Keep the current **Panel** `PanelTree` or equivalent render identity because **Panel Runtime** still owns content state used for rendering and interaction.

- [ ] **Step 2: Replace physical window effects**

Remove from `PanelRuntimeEffect`:

- `OpenPanelWindow(PanelWindowOpenRequest)`
- `ClosePanelWindow(iced::window::Id)`

Add content preparation effects/results such as:

- `PanelPrepared(crate::surface::panel::PanelTree)`
- `LoadInstalledAppsThenPreparePanel(PendingPanelOpen)`
- `RequestSearchFocus`
- existing activation/launch/Dmenu response effects.

The exact shape can be a result type instead of an enum variant if it keeps the interface smaller.

- [ ] **Step 3: Replace open/toggle intents with prepare/clear intents**

Replace `PanelRuntimeIntent::Open` and `PanelRuntimeIntent::Toggle` with content-specific intents:

- `PrepareOpen { request, catalog, module_detail_sessions }`
- `PrepareInstalledAppsLoaded { pending, apps, catalog }`
- `Clear`

Remove:

- `PanelWindowOpened`
- `PanelWindowClosed`

- [ ] **Step 4: Keep Panel content/session behavior unchanged**

Preserve:

- **Panel Menu** `SelectableMenuSession`
- **Module Detail Session**
- search focus pending state
- menu scroll state
- **Dmenu Selection Request** responder
- installed-applications **Panel Menu Source** loading
- activation outcomes

- [ ] **Step 5: Update Panel Runtime tests**

Replace stale-window and physical-window assertions with content-state assertions:

- preparing a Bar Panel stores the Panel tree/content state.
- preparing a Panel Menu opens a **Selectable Menu Session** and requests search focus.
- clearing removes Panel content state, session state, menu scroll, pending focus, and Dmenu responder.
- installed-app load remains deferred until app adapter supplies apps.

- [ ] **Step 6: Run Panel Runtime tests**

Run:

```bash
rtk cargo test app::panel_runtime -- --nocapture
```

Expected: PASS.

### Task 4: Route App Panel Events Through Foreground

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`
- Modify: `crates/niribar/src/app/messages.rs` only if event names need adjustment.

- [ ] **Step 1: Convert `panel_open_request` to Foreground request**

Make App build `PanelForegroundRequest` from origin **Bar Surface** state.

- [ ] **Step 2: Route PanelEvent through Foreground first**

Change `handle_panel_event`:

- `PanelEvent::Open` -> `ForegroundIntent::OpenPanel`
- `PanelEvent::Toggle` -> `ForegroundIntent::TogglePanel`
- `PanelEvent::Close` -> `ForegroundIntent::ClosePanel`

Panel search, scroll, focus, activate, and back events still go directly to **Panel Runtime** because they are content interactions for an already-active **Panel**.

- [ ] **Step 3: Implement App effect adapter for Foreground effects**

App applies effects in order:

- `PreparePanelRuntime` -> call **Panel Runtime** prepare intent; feed `PanelPrepared` back to **Foreground**; apply returned effects.
- `ClearPanelRuntime` -> call **Panel Runtime** clear intent.
- `OpenPanelWindow` -> create layer-shell **Panel** window; then send `ForegroundIntent::PanelWindowOpened`.
- `RemoveWindow` -> send `Message::RemoveWindow`.
- `CloseTooltipsForSurface` -> call existing passive **Tooltip** close work.
- `SuspendBarKeyboardInteractivity` / `RestoreBarKeyboardInteractivity` -> call existing adapter helpers.
- Transitional **Menu** close effects -> call existing menu/registry adapter helpers until later slices move **Menu** identity into **Foreground**.

- [ ] **Step 4: Preserve installed-app deferred open**

When **Panel Runtime** requests installed-app loading during prepare, App loads applications and feeds the loaded result back into **Panel Runtime**. Only after **Panel Runtime** emits prepared content should App feed `PanelRuntimePrepared` into **Foreground**.

- [ ] **Step 5: Preserve search focus**

When **Panel Runtime** emits `RequestSearchFocus`, App still schedules the search input focus task after the physical **Panel** window exists.

If needed, App can ask **Foreground** for the current active **Panel** window.

- [ ] **Step 6: Add App adapter tests**

Add focused tests that verify:

- `PanelEvent::Open` causes App to call **Foreground** first and then **Panel Runtime** through a Foreground effect.
- effect order closes old physical windows before opening replacement windows.
- installed-app menus do not close the current **Foreground** until content preparation completes.
- stale physical close events are routed to **Foreground**, not **Panel Runtime**.

Keep these tests thin; **Foreground** state-machine decisions should remain covered in `foreground.rs`.

### Task 5: Move Panel Physical Identity Out Of SurfaceManager

**Files:**
- Modify: `crates/niribar/src/app/surfaces.rs`
- Modify: `crates/niribar/src/app/overlay_adapter.rs`
- Delete or empty: `src/app/panel_attachment.rs`

- [ ] **Step 1: Remove Panel metadata maps**

Remove or stop using:

- `PanelMeta`
- `panel_popups`
- `closing_panel_popups`
- `RegistryEffect::PanelClosed`
- `open_panel_overlay`
- `close_panel_overlays`
- `close_panel_popup_window`
- `panel_popup_ids`
- `panel_popup`
- `remove_panel_popup`

If full removal is too large in one task, keep transitional helpers private and mark them with a short `TODO` tied to this plan. Do not let them remain the source of truth.

- [ ] **Step 2: Route physical Panel close through Foreground**

Window close handling should classify active **Panel** windows through **Foreground** and call `ForegroundIntent::PanelWindowClosed`.

- [ ] **Step 3: Handle originating Bar Surface removal**

When a **Bar Surface** disappears, App should ask **Foreground** to close any active **Panel** or **Menu** originating from that **Bar Surface**. **Bar Surface Registry** should not inspect Panel metadata to do this.

- [ ] **Step 4: Remove panel_attachment module**

Delete `src/app/panel_attachment.rs` when no longer referenced.

Update `crates/niribar/src/app.rs` module declarations accordingly.

- [ ] **Step 5: Run focused tests**

Run:

```bash
rtk cargo test app::foreground app::panel_runtime app::surfaces app::overlay_adapter -- --nocapture
```

Expected: PASS.

### Task 6: Render And Input Read Panel State Through The New Seam

**Files:**
- Modify: `crates/niribar/src/app/render.rs`
- Modify: `crates/niribar/src/app/view.rs`
- Modify: `crates/niribar/src/app/surface_input_router.rs`
- Modify: `crates/niribar/src/app/surfaces.rs`

- [ ] **Step 1: Render active Panel origin from Foreground/Panel Runtime**

Bar rendering should mark active **Panel Origin** by asking the **Foreground** / **Panel Runtime** seam, not by reading **Bar Surface Registry** Panel metadata.

- [ ] **Step 2: Classify Panel windows through Foreground**

Input role classification should ask **Foreground** for active **Panel** and **Menu** windows first, then ask **Bar Surface Registry** for **Bar Surface** and **Tooltip** windows.

- [ ] **Step 3: Keep surface input routing pure**

Do not move stateful classification into `surface_input_router.rs`. It should remain a pure router from role + input to effect.

- [ ] **Step 4: Run rendering/input tests**

Run:

```bash
rtk cargo test app::render app::surface_input_router app::view -- --nocapture
```

Expected: PASS.

### Task 7: Verification Gate For Slice 1

- [ ] **Step 1: Run the app test suite**

```bash
rtk cargo test
```

Expected: PASS.

- [ ] **Step 2: Run clippy**

```bash
rtk cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 3: Run build**

```bash
rtk cargo build
```

Expected: PASS.

- [ ] **Step 4: Inspect remaining Panel ownership references**

Run:

```bash
rtk rg -n "PanelAttachment|PanelMeta|panel_popups|closing_panel_popups|PanelWindowOpened|PanelWindowClosed|OpenPanelWindow|ClosePanelWindow" crates/niribar/src/app crates/niribar/src/modules CONTEXT.md docs/niribar/adr
```

Expected:

- `PanelAttachment` appears only as current domain language or private **Foreground** detail.
- `PanelMeta`, `panel_popups`, and `closing_panel_popups` are gone or explicitly transitional.
- **Panel Runtime** has no physical window ID fields or physical window effects.

---

## Slice 2: Foreground Owns Menu Physical Identity

**Goal:** Move physical **Menu** window identity, pending replacement, and stale close handling out of **Bar Surface Registry** and into **Foreground**.

**Files:**
- Modify: `crates/niribar/src/app/foreground.rs`
- Modify: `crates/niribar/src/app/menu.rs`
- Modify: `crates/niribar/src/app/surfaces.rs`
- Modify: `crates/niribar/src/app/overlay_adapter.rs`
- Modify: `crates/niribar/src/app/subscriptions.rs`

Tasks:

- [ ] Add **Foreground** tests for **Menu** open, replacement, stale close, current close, layout update targeting, and opening **Menu** while **Panel** is active.
- [ ] Add **Foreground** effects for opening/removing physical **Menu** windows and sending DBusMenu closed events through App.
- [ ] Move `PopupMeta`, pending **Menu** replacement, and closing **Menu** identity out of **Bar Surface Registry** or make **Foreground** the source of truth during a short transition.
- [ ] Route `MenuEvent::Open`, `MenuEvent::Closed`, and physical **Menu** window close through **Foreground**.
- [ ] Keep DBus menu fetch and activation in App/DBus adapters.
- [ ] Verify **Tooltip** close-on-Menu-open still happens through **Foreground** effects.

Verification:

```bash
rtk cargo test app::foreground app::menu app::surfaces app::overlay_adapter -- --nocapture
rtk cargo test
```

---

## Slice 3: Foreground Owns Active Menu Session

**Goal:** Collapse window-keyed `MenuSystem` into **Foreground** because there is at most one active **Menu**.

**Files:**
- Modify: `crates/niribar/src/app/foreground.rs`
- Modify: `crates/niribar/src/app/menu.rs`
- Modify: `crates/niribar/src/app/selectable_menu_session.rs`
- Modify: `crates/niribar/src/app/panel_menu_session.rs` only if naming cleanup is in scope.

Tasks:

- [ ] Add **Foreground** tests for **SNI Menu** hover, focus move, search query, activation, searchable nested entries, and layout update reset.
- [ ] Move the active **Menu** `SelectableMenuSession` into `ForegroundOverlay::Menu`.
- [ ] Remove `MenuSystem` and replace calls with **Foreground** intents/effects.
- [ ] Keep **Panel Menu** sessions in **Panel Runtime** for this slice.
- [ ] Make **SNI Menu** activation produce a generic selected entry effect that App interprets as DBusMenu item activation.
- [ ] Keep `SelectableMenuSession` itself generic and unaware of DBus, process launch, IPC, or physical windows.

Verification:

```bash
rtk cargo test app::foreground app::selectable_menu_session app::menu -- --nocapture
rtk cargo test
```

---

## Slice 4: Foreground Owns Interactive Overlay Role Snapshots

**Goal:** Make role classification read **Foreground** for **Panel** and **Menu** and **Bar Surface Registry** for **Bar Surface** and **Tooltip** only.

**Files:**
- Modify: `crates/niribar/src/app/foreground.rs`
- Modify: `crates/niribar/src/app/surface_input_router.rs`
- Modify: `crates/niribar/src/app/surfaces.rs`
- Modify: `src/app/input.rs` if needed.

Tasks:

- [ ] Add a **Foreground** snapshot that reports active physical **Panel** and **Menu** window roles.
- [ ] Update App classification to ask **Foreground** first for interactive overlay roles.
- [ ] Keep `surface_input_router.rs` pure.
- [ ] Remove **Panel** and **Menu** role classification from **Bar Surface Registry**.
- [ ] Preserve passive **Tooltip** role classification in **Bar Surface Registry** or a later Tooltip module.

Verification:

```bash
rtk cargo test app::surface_input_router app::foreground app::surfaces -- --nocapture
rtk cargo test
```

---

## Slice 5: Narrow Bar Surface Registry

**Goal:** Leave **Bar Surface Registry** with live **Bar Surface** records and passive **Tooltip** metadata only.

**Files:**
- Modify: `crates/niribar/src/app/surfaces.rs`
- Modify: `crates/niribar/src/app/render.rs`
- Modify: `crates/niribar/src/app/view.rs`
- Modify: `crates/niribar/src/app/subscriptions.rs`

Tasks:

- [ ] Remove remaining **Panel** and **Menu** overlay maps, pending popup state, closing sets, and related registry effects.
- [ ] Rename internal types if needed so implementation language matches **Bar Surface Registry** and does not imply overlay lifecycle ownership.
- [ ] Update tests that still assert **Panel** or **Menu** ownership through **Bar Surface Registry**.
- [ ] Keep **Tooltip** tests proving passive hover behavior and close rules.
- [ ] Run `rtk rg` for old overlay lifecycle terms and remove stale code/comments.

Verification:

```bash
rtk cargo test app::surfaces app::foreground app::menu app::panel_runtime -- --nocapture
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

---

## Final Verification

- [ ] Full suite:

```bash
rtk cargo test
```

- [ ] Lints:

```bash
rtk cargo clippy --all-targets -- -D warnings
```

- [ ] Build:

```bash
rtk cargo build
```

- [ ] Architecture search:

```bash
rtk rg -n "panel_popups|closing_panel_popups|closing_menu_popups|pending_menu_popup|PanelMeta|PopupMeta|MenuSystem|PanelAttachment|PanelWindowOpened|PanelWindowClosed|OpenPanelWindow|ClosePanelWindow" crates/niribar/src/app
```

Expected final state:

- **Foreground** is the only owner of physical **Panel** and **Menu** overlay identity.
- **Foreground** owns the active **Menu** **Selectable Menu Session**.
- **Panel Runtime** owns **Panel** content/session state and no physical window identity.
- **Bar Surface Registry** owns live **Bar Surface** records and passive **Tooltip** metadata only.
- App translates **Foreground** effects into side-effect work.
- Tests prove replacement and stale close behavior through the **Foreground** interface.
