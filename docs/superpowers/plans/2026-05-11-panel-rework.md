# Panel Rework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace bar expansion with a generic anchored Panel surface that can show module panel views, bar panel module layouts, and named launcher/menu content.

**Architecture:** The bar no longer resizes or switches all modules into expanded rendering. Opening a panel creates one overlay layer-shell surface anchored to the originating Bar Surface, with origin metadata used for highlighting and output-scoped behavior. A typed action system drives pointer and focused-key triggers for bars, modules, and menu entries.

**Tech Stack:** Rust, iced + iced_layershell, KDL config, zbus for existing tray menus, local IPC.

---

## Resolved Design

- Replace the old expansion feature entirely; no compatibility or migration path is needed.
- Rename the domain concept from Expansion to Panel. Remove `expanded-width`, `expand`, `expand-trigger`, `expand-keybind`, and bar `hover-delay`.
- Keep the bar in its normal collapsed/horizontal form. A panel opens beside it.
- Only one panel may be open at a time.
- Panel origin belongs to the originating Bar Surface. Origin can be `Bar` or `Module(ModuleId)`.
- Source module remains visually active while its panel is open, even when keyboard focus is in the panel.
- Opening a panel closes SNI menus and tooltips. Opening an SNI menu closes panels. Tooltips do not open while a panel is open.
- Use one generic Panel primitive with content modes:
  - `BarPanel`: the originating bar's configured `panel { start/center/end }` module layout.
  - `ModulePanel`: the source module's panel view.
  - `Menu`: a named launcher/menu list.
- Bar panel layout uses the same `start/center/end` config shape as bar modules, but renders as a single vertical stack of sections in that order.
- Panel layouts may reference modules not present in the collapsed bar layout.
- Dynamic placeholder modules such as `tray` and `workspaces` expand in panel layouts the same way they do in bar layouts.
- Duplicate module references are rejected within one panel layout.
- Module panel rendering uses the renamed `view_panel_with_style`, not `view_expanded_with_style`.
- Panel opening is a configurable typed action, not default module click behavior.
- Actions live in `actions { ... }` blocks. Trigger node names are direct names:

```kdl
actions {
  click { open-panel }
  right-click { command "notify-send clock" }
  Ctrl+E { toggle-panel }
  Super+Space { toggle-menu "main" }
}
```

- Known pointer triggers are `click` and `right-click`. Any other action trigger node name must parse as a valid single key combination.
- Module key actions fire only when that module is focused. Bar key actions fire while the Bar Surface or its panel has focus. Module actions take precedence over bar actions.
- Navigation and Escape remain hardcoded. `Enter`/activation can be overridden by actions; otherwise existing built-in keyboard behavior remains.
- Action operations for v1:
  - `command "shell command"` or `command { argv "program" "arg" }`
  - `open-panel`
  - `toggle-panel`
  - `close-panel`
  - `open-menu "name"`
  - `toggle-menu "name"`
- A trigger or menu item has exactly one action operation.
- `open-panel` is idempotent for the same target. `toggle-panel` closes only when the same target is already open; otherwise it replaces the current panel content.
- `toggle-panel` with no explicit target opens/toggles the bar default panel, not the last-opened mode.
- Bar `click` actions fire on empty bar background only. Module clicks do not bubble to bar actions.
- Panel uses a normal overlay layer-shell window with no exclusive zone.
- Panel opens beside the bar edge: left -> right, right -> left, top -> below, bottom -> above. Module-sourced panels align along the bar axis to the source module bounds and clamp to screen bounds.
- Panel takes keyboard focus for v1. Escape closes panel and restores origin bar interactivity. Panel stays open when the origin bar loses focus to the panel, and closes when the panel loses focus to another application/window.
- Outside click closes every panel mode in v1.
- Panel has per-bar sizing config and scrolls when content exceeds max bounds.
- Panel style inherits from the originating bar style in v1.
- Menu mode is app-owned launcher/menu content, separate from SNI DBus menus.
- Named menus are global and static config in v1. Menu entries use the same typed action enum as bar/module actions.
- Menu mode includes search in the v1 contract. Search focuses automatically when menu mode opens.
- Search filters recursively through nested items and shows flattened breadcrumb labels such as `Apps / Terminal`.
- Escape clears non-empty menu search first, then closes the panel. Backspace at empty search navigates one submenu level up.
- Menu item activation closes the panel after launching a command. Module panel actions do not auto-close by default.

## File Map

- `src/config/module.rs`: Replace `CommandConfig`-only module actions with typed action config shared by modules, bars, and menus.
- `src/config/bar.rs`: Remove expand fields; add `BarActionsConfig`, `PanelConfig`, and panel layout sizing.
- `src/config.rs`: Export new panel/action/menu types; add global named menus to `Config`.
- `src/config/parse/actions.rs`: Parse typed action operations and direct trigger node names.
- `src/config/parse/bars.rs` and `src/config/parse/bars_tests.rs`: Parse bar actions and panel config; remove expand parsing tests.
- `src/config/parse/mod.rs`: Parse global `menus { menu "name" { ... } }`.
- `src/config/validate.rs`: Validate action targets, panel layouts, duplicate modules, and named menus.
- `src/modules.rs` and `src/modules/*`: Rename expanded view methods to panel view methods.
- `src/surface.rs`: Remove `ExpandState`; add active panel origin metadata if the renderer needs direct lookup.
- `src/surface/panel.rs`: New panel data model and view helpers.
- `src/app/panel.rs`: New PanelSystem, lifecycle, actions, focus/search/navigation state.
- `src/app/surfaces.rs`: Store panel popup metadata alongside menu/tooltip windows.
- `src/app/render.rs`: Render active origin state in bar modules and render panel windows.
- `src/app/view.rs`: Route panel windows to `surface::panel` rendering.
- `src/app/module_events.rs`: Dispatch typed module actions instead of command-only actions.
- `src/app/focus.rs`: Route focused key triggers through typed actions; handle panel focus and Escape.
- `src/app/subscriptions.rs`: Remove expand animation/tick/IP C needs; subscribe to IPC when panel commands exist.
- `src/surface/input.rs`: Replace global expand keybind handling with generic focused key event messages.
- `src/cli.rs` and `src/ipc.rs`: Rename `--toggle-expand` / `toggle-expand` to `--toggle-panel` / `toggle-panel`.
- `CONTEXT.md`: Rename Expansion terms to Panel and update domain Q&A.
- `config/default.kdl` and `config/new_default.kdl`: Update config examples to typed actions, `panel`, and named menus.

---

## Phase 1 - Typed Actions

### Task 1: Replace command-only module actions with typed actions

**Files:**
- Modify: `src/config/module.rs`
- Modify: `src/config.rs`
- Test: `src/config/parse/tests.rs`

- [ ] **Step 1: Add typed action structs**

In `src/config/module.rs`, replace `ModuleActionsConfig` with:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionTrigger {
    Click,
    RightClick,
    Key(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelAction {
    Command(CommandConfig),
    OpenPanel,
    TogglePanel,
    ClosePanel,
    OpenMenu(String),
    ToggleMenu(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionsConfig {
    pub bindings: Vec<ActionBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBinding {
    pub trigger: ActionTrigger,
    pub action: PanelAction,
}

impl ActionsConfig {
    pub fn action_for(&self, trigger: &ActionTrigger) -> Option<&PanelAction> {
        self.bindings
            .iter()
            .find(|binding| &binding.trigger == trigger)
            .map(|binding| &binding.action)
    }
}

pub type ModuleActionsConfig = ActionsConfig;
```

Keep the `ModuleActionsConfig` alias for a small first slice so existing field names compile while call sites are migrated.

- [ ] **Step 2: Export the new types**

In `src/config.rs`, add `ActionBinding`, `ActionTrigger`, `ActionsConfig`, and `PanelAction` to the `pub use module::{ ... }` list.

- [ ] **Step 3: Update parse tests to describe the new syntax**

In `src/config/parse/tests.rs`, replace command-only action assertions with:

```rust
#[test]
fn parse_typed_module_actions_click_command_and_key_panel() {
    let kdl = r#"
        modules {
            module "clock" {
                actions {
                    click { command "gnome-clocks" }
                    Ctrl+E { open-panel }
                }
            }
        }
    "#;
    let config = parse(kdl).unwrap();
    let actions = &config.modules[0].actions;
    assert_eq!(
        actions.action_for(&crate::config::ActionTrigger::Click),
        Some(&crate::config::PanelAction::Command(
            crate::config::CommandConfig::Shell("gnome-clocks".into())
        ))
    );
    assert_eq!(
        actions.action_for(&crate::config::ActionTrigger::Key("Ctrl+E".into())),
        Some(&crate::config::PanelAction::OpenPanel)
    );
}
```

- [ ] **Step 4: Run the focused test**

Run: `cargo test --lib config::parse::tests::parse_typed_module_actions_click_command_and_key_panel`

Expected: FAIL until the parser is updated.

- [ ] **Step 5: Commit**

```bash
git add src/config/module.rs src/config.rs src/config/parse/tests.rs
git commit -m "refactor(config): introduce typed action model"
```

### Task 2: Parse typed action operations

**Files:**
- Modify: `src/config/parse/actions.rs`
- Test: `src/config/parse/tests.rs`

- [ ] **Step 1: Implement trigger parsing**

In `src/config/parse/actions.rs`, add:

```rust
fn parse_trigger(name: &str, ctx: &ParseCtx, node: &KdlNode) -> ConfigResult<crate::config::ActionTrigger> {
    match name {
        "click" => Ok(crate::config::ActionTrigger::Click),
        "right-click" => Ok(crate::config::ActionTrigger::RightClick),
        other if crate::surface::input::parse_keybind(other).is_some() => {
            Ok(crate::config::ActionTrigger::Key(other.to_string()))
        }
        other => Err(ctx.node_err(
            node,
            format!("unknown action trigger {other:?}"),
            Some("expected `click`, `right-click`, or a keybind such as `Ctrl+E`".into()),
        )),
    }
}
```

- [ ] **Step 2: Implement operation parsing**

In `src/config/parse/actions.rs`, add:

```rust
fn parse_panel_action(
    action: &KdlNode,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<crate::config::PanelAction> {
    let Some(children) = action.children() else {
        return Err(ctx.node_err(
            action,
            "expected exactly one action child",
            Some("use `click { command \"kitty\" }` or `click { open-panel }`".into()),
        ));
    };
    let nodes = children.nodes();
    if nodes.len() != 1 {
        return Err(ctx.node_err(action, "expected exactly one action child", None));
    }

    let op = &nodes[0];
    let op_ctx = ctx.child(op.name().value());
    match op.name().value() {
        "command" => parse_command_node_ctx(op, &op_ctx, command_policy)
            .map(crate::config::PanelAction::Command),
        "open-panel" => Ok(crate::config::PanelAction::OpenPanel),
        "toggle-panel" => Ok(crate::config::PanelAction::TogglePanel),
        "close-panel" => Ok(crate::config::PanelAction::ClosePanel),
        "open-menu" => parse_named_menu_action(op, &op_ctx, crate::config::PanelAction::OpenMenu),
        "toggle-menu" => parse_named_menu_action(op, &op_ctx, crate::config::PanelAction::ToggleMenu),
        other => Err(op_ctx.node_err(
            op,
            format!("unknown action operation {other:?}"),
            Some("expected command, open-panel, toggle-panel, close-panel, open-menu, or toggle-menu".into()),
        )),
    }
}

fn parse_named_menu_action(
    node: &KdlNode,
    ctx: &ParseCtx,
    build: impl FnOnce(String) -> crate::config::PanelAction,
) -> ConfigResult<crate::config::PanelAction> {
    if node.entries().len() != 1 || node.children().is_some() {
        return Err(ctx.node_err(node, "expected one menu name string", None));
    }
    let Some(name) = node.entries()[0].value().as_string() else {
        return Err(ctx.entry_err(&node.entries()[0], "expected menu name string", None));
    };
    if name.is_empty() {
        return Err(ctx.entry_err(&node.entries()[0], "menu name must not be empty", None));
    }
    Ok(build(name.to_string()))
}
```

- [ ] **Step 3: Update `parse` to return `ActionsConfig` bindings**

Replace the old `match action_name { "click" => config.click = ... }` block with construction of `ActionBinding` values. Reject duplicate triggers by comparing `ActionTrigger`.

- [ ] **Step 4: Add parser regression tests**

Add tests in `src/config/parse/tests.rs`:

```rust
#[test]
fn parse_action_rejects_compact_command_string() {
    let kdl = r#"modules { module "clock" { actions { click "date" } } }"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(err.contains("expected exactly one action child"), "got: {err}");
}

#[test]
fn parse_action_rejects_multiple_operations() {
    let kdl = r#"modules { module "clock" { actions { click { open-panel command "date" } } } }"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(err.contains("expected exactly one action child"), "got: {err}");
}

#[test]
fn parse_action_rejects_invalid_trigger() {
    let kdl = r#"modules { module "clock" { actions { clik { open-panel } } } }"#;
    let err = parse(kdl).unwrap_err().to_string();
    assert!(err.contains("unknown action trigger"), "got: {err}");
}
```

- [ ] **Step 5: Run parser tests**

Run: `cargo test --lib config::parse::tests::parse_action`

Expected: PASS.

- [x] **Step 6: Commit**

```bash
git add src/config/parse/actions.rs src/config/parse/tests.rs
git commit -m "feat(config): parse typed actions"
```

---

## Phase 2 - Panel Config and Domain Rename

### Task 3: Replace bar expansion config with panel config

**Files:**
- Modify: `src/config/bar.rs`
- Modify: `src/config/parse/bars.rs`
- Modify: `src/config/parse/bars_tests.rs`
- Modify: `src/config/tests.rs`

- [ ] **Step 1: Define panel config**

In `src/config/bar.rs`, add:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PanelModules {
    pub start: Vec<String>,
    pub center: Vec<String>,
    pub end: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelAnimationConfig {
    pub open_ms: f32,
    pub close_ms: f32,
}

impl Default for PanelAnimationConfig {
    fn default() -> Self {
        Self {
            open_ms: 180.0,
            close_ms: 120.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelConfig {
    pub width: u32,
    pub max_width: Option<u32>,
    pub height: Option<u32>,
    pub max_height: Option<u32>,
    pub modules: PanelModules,
    pub animation: PanelAnimationConfig,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            width: 360,
            max_width: None,
            height: None,
            max_height: Some(600),
            modules: PanelModules::default(),
            animation: PanelAnimationConfig::default(),
        }
    }
}
```

Then remove `expanded_width`, `expand`, `expand_trigger`, `hover_delay`, `expand_keybind`, and `animation` from `BarConfig`. Add:

```rust
pub actions: crate::config::ActionsConfig,
pub panel: PanelConfig,
```

- [ ] **Step 2: Parse `panel` and bar `actions`**

In `src/config/parse/bars.rs`, remove accepted fields for old expansion and add accepted fields `actions` and `panel`. Parse:

```kdl
bar "main" {
  actions {
    click { open-panel }
    Ctrl+E { toggle-panel }
  }
  panel {
    width 360
    max-height 600
    start "workspaces"
    center "focused-window"
    end "network" "volume" "battery" "clock"
    animation { open-ms 180 close-ms 120 }
  }
}
```

Use the existing bar modules parser pattern for `start`, `center`, and `end`.

- [ ] **Step 3: Add parse tests**

In `src/config/parse/bars_tests.rs`, add:

```rust
#[test]
fn parse_bar_actions_and_panel_layout() {
    let kdl = r#"
        bar "main" {
            actions {
                click { open-panel }
                Ctrl+E { toggle-panel }
            }
            panel {
                width 420
                max-height 720
                start "workspaces"
                center "focused-window"
                end "network" "clock"
                animation { open-ms 200 close-ms 100 }
            }
        }
    "#;
    let config = parse(kdl).unwrap();
    let bar = &config.bars[0];
    assert_eq!(bar.panel.width, 420);
    assert_eq!(bar.panel.max_height, Some(720));
    assert_eq!(bar.panel.modules.start, vec!["workspaces"]);
    assert_eq!(bar.panel.modules.center, vec!["focused-window"]);
    assert_eq!(bar.panel.modules.end, vec!["network", "clock"]);
    assert_eq!(bar.panel.animation.open_ms, 200.0);
    assert_eq!(bar.panel.animation.close_ms, 100.0);
    assert_eq!(
        bar.actions.action_for(&crate::config::ActionTrigger::Click),
        Some(&crate::config::PanelAction::OpenPanel)
    );
}
```

- [ ] **Step 4: Remove obsolete tests**

Delete tests that assert:

- `expanded-width` parses or validates.
- `expand-trigger` parses.
- `expand-keybind` parses.
- hover expansion delay behavior exists.

- [ ] **Step 5: Run config tests**

Run: `cargo test --lib config`

Expected: PASS after call sites compile.

- [ ] **Step 6: Commit**

```bash
git add src/config/bar.rs src/config/parse/bars.rs src/config/parse/bars_tests.rs src/config/tests.rs
git commit -m "refactor(config): replace expansion fields with panel config"
```

### Task 4: Rename module expanded rendering to panel rendering

**Files:**
- Modify: `src/modules.rs`
- Modify: `src/modules/*.rs`
- Modify: `src/app/render.rs`

- [ ] **Step 1: Rename trait methods**

In `src/modules.rs`, rename:

```rust
fn view_expanded_with_style(...)
```

to:

```rust
fn view_panel_with_style(
    &self,
    state: &ModuleState,
    bar_style: &crate::config::Style,
) -> iced::Element<'_, crate::app::Message>;
```

Update default horizontal fallback to call `view_panel_with_style`.

- [ ] **Step 2: Change bar rendering to never request panel views**

In `Module::view_for_bar`, remove the `expanded: bool` argument and make the dispatch:

```rust
if horizontal {
    self.view_horizontal_with_style(state, bar_style)
} else {
    self.view_collapsed_with_style(state, bar_style)
}
```

Update all call sites.

- [ ] **Step 3: Run a mechanical search**

Run: `rg "expanded|view_expanded|ExpandState|expanded_width|expand_trigger|expand_keybind|hover_pending_ms" src`

Expected: remaining matches are only in historical docs or tests that have not been updated in later tasks. No production code should rely on old expansion semantics.

- [ ] **Step 4: Run module render tests**

Run: `cargo test --lib modules app::render`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/modules.rs src/modules src/app/render.rs
git commit -m "refactor(modules): rename expanded rendering to panel rendering"
```

---

## Phase 3 - Generic Panel Surface

### Task 5: Add panel surface model

**Files:**
- Create: `src/surface/panel.rs`
- Modify: `src/surface.rs`
- Modify: `src/app/messages.rs`
- Modify: `src/app/surfaces.rs`

- [ ] **Step 1: Create `surface::panel`**

Create `src/surface/panel.rs`:

```rust
use crate::modules::ModuleId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelOrigin {
    Bar,
    Module(ModuleId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelContent {
    BarPanel,
    ModulePanel(ModuleId),
    Menu(String),
}

#[derive(Debug, Clone)]
pub struct PanelTree {
    pub origin: PanelOrigin,
    pub content: PanelContent,
    pub origin_window: iced::window::Id,
    pub origin_bar_name: String,
    pub origin_output_name: String,
    pub anchor: Option<iced::Rectangle>,
    pub screen_size: (u32, u32),
}
```

Export it from `src/surface.rs` with `pub mod panel;`.

- [ ] **Step 2: Add panel events**

In `src/app/messages.rs`, add:

```rust
#[derive(Debug, Clone)]
pub enum PanelEvent {
    Open {
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
    },
    Close,
    Toggle {
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
    },
    SearchChanged(String),
    FocusMoveNext,
    FocusMovePrev,
    Activate,
    Back,
}
```

Add `Message::Panel(PanelEvent)` in the app message enum.

- [ ] **Step 3: Store panel metadata**

In `src/app/surfaces.rs`, add:

```rust
#[derive(Debug, Clone)]
pub(super) struct PanelMeta {
    pub(super) tree: crate::surface::panel::PanelTree,
}
```

Add `panel_popups: HashMap<iced::window::Id, PanelMeta>` to `SurfaceManager`, plus methods `panel_popup`, `insert_panel_popup`, `remove_panel_popup`, `panel_popup_ids`, `panel_popup_values`, and `has_panel_popup`.

- [ ] **Step 4: Add active origin lookup**

Either add to `SurfaceState`:

```rust
pub active_panel_origin: Option<crate::surface::panel::PanelOrigin>,
```

or implement a `SurfaceManager::active_panel_origin_for(origin_window)` lookup. Prefer lookup if it keeps `SurfaceState` smaller.

- [ ] **Step 5: Run surface tests**

Run: `cargo test --lib surface app::surfaces`

Expected: PASS after compile fixes.

- [ ] **Step 6: Commit**

```bash
git add src/surface.rs src/surface/panel.rs src/app/messages.rs src/app/surfaces.rs
git commit -m "feat(panel): add generic panel surface metadata"
```

### Task 6: Open and close one overlay panel window

**Files:**
- Create: `src/app/panel.rs`
- Modify: `src/app.rs`
- Modify: `src/app/view.rs`
- Modify: `src/app/surfaces.rs`

- [ ] **Step 1: Add `PanelSystem`**

Create `src/app/panel.rs`:

```rust
#[derive(Debug, Default)]
pub(super) struct PanelSystem {
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) menu_path: Vec<usize>,
}

impl PanelSystem {
    pub(super) fn clear(&mut self) {
        self.search_query.clear();
        self.focused_index = None;
        self.menu_path.clear();
    }
}
```

Wire it into `App` as `panel: PanelSystem`.

- [ ] **Step 2: Implement panel window settings**

In `src/app/panel.rs`, add a `panel_window_settings` helper using `NewLayerShellSettings`:

```rust
fn panel_window_settings(
    bar: &crate::config::BarConfig,
    tree: &crate::surface::panel::PanelTree,
) -> iced_layershell::reexport::NewLayerShellSettings {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings};

    let width = bar.panel.width;
    let height = bar.panel.height.or(bar.panel.max_height).unwrap_or(600);
    let anchor = match bar.edge {
        crate::config::Edge::Top => Anchor::Top | Anchor::Left,
        crate::config::Edge::Bottom => Anchor::Bottom | Anchor::Left,
        crate::config::Edge::Left => Anchor::Left | Anchor::Top,
        crate::config::Edge::Right => Anchor::Right | Anchor::Top,
    };

    NewLayerShellSettings {
        anchor,
        exclusive_zone: Some(-1),
        size: Some((width, height)),
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        ..Default::default()
    }
}
```

The first implementation may use coarse edge anchoring. Add source-module alignment in Task 8.

- [ ] **Step 3: Implement close-all and open**

In `src/app/panel.rs`, implement:

```rust
impl App {
    pub(super) fn close_all_panels(&mut self) -> Vec<iced::Task<Message>> {
        let ids: Vec<_> = self.surfaces.panel_popup_ids().collect();
        self.panel.clear();
        ids.into_iter()
            .map(|id| {
                self.surfaces.remove_panel_popup(id);
                iced::Task::done(Message::RemoveWindow(id))
            })
            .collect()
    }
}
```

Handle `PanelEvent::Open`, `PanelEvent::Toggle`, and `PanelEvent::Close` by:

- Closing tooltips.
- Closing SNI menus.
- Closing/replacing existing panels.
- Opening a new layer-shell window.
- Storing `PanelMeta`.

- [ ] **Step 4: Route panel windows in `view`**

In `src/app/view.rs`, before tooltip rendering, add:

```rust
if let Some(panel) = self.surfaces.panel_popup(window) {
    return render_panel_model(self.panel_render_model(panel), window);
}
```

For now, render a simple container with text `"panel"` so lifecycle tests can pass. Task 9 replaces it with real content.

- [ ] **Step 5: Add lifecycle tests**

In `src/app/panel.rs`, add tests:

```rust
#[test]
fn opening_panel_closes_existing_panel() {
    let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
    let origin = iced::window::Id::unique();
    app.surface_mut(origin);

    let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
        origin_window: origin,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content: crate::surface::panel::PanelContent::BarPanel,
        anchor: None,
    }));
    assert_eq!(app.surfaces.panel_popup_ids().count(), 1);

    let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
        origin_window: origin,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content: crate::surface::panel::PanelContent::Menu("main".into()),
        anchor: None,
    }));
    assert_eq!(app.surfaces.panel_popup_ids().count(), 1);
}
```

- [ ] **Step 6: Run panel tests**

Run: `cargo test --lib app::panel`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/app.rs src/app/panel.rs src/app/view.rs src/app/surfaces.rs
git commit -m "feat(panel): open a single overlay panel"
```

### Task 7: Dispatch typed actions

**Files:**
- Modify: `src/app/module_runtime.rs`
- Modify: `src/app/module_events.rs`
- Modify: `src/app/focus.rs`
- Modify: `src/app/panel.rs`

- [ ] **Step 1: Update runtime action lookup**

Change `ModuleRuntime::action_for` to accept `ActionTrigger` and return `PanelAction`.

- [ ] **Step 2: Add app action dispatcher**

In `src/app/panel.rs`, add:

```rust
impl App {
    pub(super) fn dispatch_panel_action(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        action: crate::config::PanelAction,
    ) -> iced::Task<Message> {
        match action {
            crate::config::PanelAction::Command(command) => {
                let _ = crate::command::spawn_config_command_fire_and_forget(&command);
                iced::Task::none()
            }
            crate::config::PanelAction::OpenPanel => {
                let content = match &origin {
                    crate::surface::panel::PanelOrigin::Bar => crate::surface::panel::PanelContent::BarPanel,
                    crate::surface::panel::PanelOrigin::Module(id) => crate::surface::panel::PanelContent::ModulePanel(id.clone()),
                };
                iced::Task::done(Message::Panel(crate::app::messages::PanelEvent::Open {
                    origin_window,
                    origin,
                    content,
                    anchor: None,
                }))
            }
            crate::config::PanelAction::TogglePanel => {
                let content = match &origin {
                    crate::surface::panel::PanelOrigin::Bar => crate::surface::panel::PanelContent::BarPanel,
                    crate::surface::panel::PanelOrigin::Module(id) => crate::surface::panel::PanelContent::ModulePanel(id.clone()),
                };
                iced::Task::done(Message::Panel(crate::app::messages::PanelEvent::Toggle {
                    origin_window,
                    origin,
                    content,
                    anchor: None,
                }))
            }
            crate::config::PanelAction::ClosePanel => {
                iced::Task::done(Message::Panel(crate::app::messages::PanelEvent::Close))
            }
            crate::config::PanelAction::OpenMenu(name) => {
                iced::Task::done(Message::Panel(crate::app::messages::PanelEvent::Open {
                    origin_window,
                    origin,
                    content: crate::surface::panel::PanelContent::Menu(name),
                    anchor: None,
                }))
            }
            crate::config::PanelAction::ToggleMenu(name) => {
                iced::Task::done(Message::Panel(crate::app::messages::PanelEvent::Toggle {
                    origin_window,
                    origin,
                    content: crate::surface::panel::PanelContent::Menu(name),
                    anchor: None,
                }))
            }
        }
    }
}
```

- [ ] **Step 3: Update pointer dispatch**

In `src/app/module_events.rs`, map `ModuleClick(id, window)` to `ActionTrigger::Click` and `ModuleRightClick(id, window)` to `ActionTrigger::RightClick`. When a typed action exists, call `dispatch_panel_action(window, PanelOrigin::Module(id.clone()), action.clone())`. If no action exists, keep existing built-in module behavior.

- [ ] **Step 4: Add tests for action precedence**

Add tests proving:

- Configured module `click { open-panel }` opens a module panel and does not call built-in command.
- Configured module `right-click { command "..." }` still runs command.
- Missing action falls back to built-in module behavior.

- [ ] **Step 5: Run tests**

Run: `cargo test --lib app::module_runtime app::module_events app::panel`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/app/module_runtime.rs src/app/module_events.rs src/app/focus.rs src/app/panel.rs
git commit -m "feat(panel): dispatch typed actions"
```

---

## Phase 4 - Rendering Panel Content

### Task 8: Resolve panel module layouts

**Files:**
- Modify: `src/app/layout.rs`
- Modify: `src/app/render.rs`
- Test: `src/app/render.rs`

- [x] **Step 1: Add panel layout resolver**

In `src/app/layout.rs`, add `effective_panel_module_layout_for_bar(&self, bar: &BarConfig) -> Vec<(Zone, ModuleId)>` using `bar.panel.modules` with the same dynamic placeholder expansion logic as `effective_module_layout_for_bar`.

- [x] **Step 2: Add duplicate validation**

In `src/config/validate.rs`, validate that a bar's `panel` layout does not reference the same configured module name twice after static names are considered. Error message must include `bar.<name>.panel`.

- [x] **Step 3: Add tests**

Add tests proving:

- A panel can reference a module not present in `bar.modules`.
- A panel reference to `workspaces` expands to workspace item instances.
- Duplicate panel references are rejected.

- [x] **Step 4: Run tests**

Run: `cargo test --lib app::layout config::tests::validate`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/app/layout.rs src/config/validate.rs src/app/render.rs
git commit -m "feat(panel): resolve panel module layouts"
```

### Task 9: Render bar and module panel content

**Files:**
- Modify: `src/surface/panel.rs`
- Modify: `src/app/render.rs`
- Modify: `src/app/view.rs`

- [x] **Step 1: Add panel render model**

In `src/app/render.rs`, add:

```rust
pub(super) struct PanelRenderModel<'a> {
    pub(super) tree: &'a crate::surface::panel::PanelTree,
    pub(super) entries: Vec<ModuleRenderEntry<'a>>,
    pub(super) bar_style: &'a crate::config::Style,
}
```

For `PanelContent::BarPanel`, build entries from `effective_panel_module_layout_for_bar`.

For `PanelContent::ModulePanel(id)`, build one entry for that module.

- [x] **Step 2: Add panel frame helper**

In `src/surface/panel.rs`, add:

```rust
pub fn panel_module_frame<'a>(
    content: iced::Element<'a, crate::app::Message>,
    focused: bool,
) -> iced::Element<'a, crate::app::Message> {
    use iced::widget::container;
    container(content)
        .padding([8, 10])
        .width(iced::Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: if focused {
                Some(iced::Background::Color(crate::theme::SURFACE0))
            } else {
                Some(iced::Background::Color(crate::theme::SURFACE1))
            },
            border: iced::Border {
                color: crate::theme::BORDER,
                width: 1.0,
                radius: iced::border::Radius::from(4.0),
            },
            ..Default::default()
        })
        .into()
}
```

- [x] **Step 3: Render single-column panel**

In `src/app/view.rs`, implement `render_panel_model` as a `scrollable(column(entries).spacing(8)).padding(8)` inside a container using the originating bar style colors.

- [x] **Step 4: Highlight active origin in the bar**

In `src/app/render.rs`, set `active = panel origin lookup matches this module`. Pass `hovered || focused || active` to the bar module frame.

- [x] **Step 5: Run render tests**

Run: `cargo test --lib app::render app::view`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/surface/panel.rs src/app/render.rs src/app/view.rs
git commit -m "feat(panel): render module and bar panel content"
```

### Task 10: Anchor panel to module bounds

**Files:**
- Modify: `src/app/render.rs`
- Modify: `src/app/module_events.rs`
- Modify: `src/app/focus.rs`
- Modify: `src/app/panel.rs`
- Modify: `src/app/widget_ops.rs`

- [x] **Step 1: Assign stable widget IDs to bar module frames**

Add a helper similar to tray icon widget IDs:

```rust
pub(crate) fn module_widget_id(window: iced::window::Id, id: &crate::modules::ModuleId) -> iced::widget::Id {
    iced::widget::Id::new(format!("module:{}:{:?}", id.0, window))
}
```

Attach it to the module frame container in `src/app/render.rs`.

- [x] **Step 2: Find bounds before opening module panels**

When a module action opens/toggles a panel, run `iced::advanced::widget::operate(FindBounds { target, result: None })` and then emit `PanelEvent::Open` / `Toggle` with `anchor: bounds`.

- [x] **Step 3: Use anchor in panel positioning**

Update `panel_window_settings` to align along the bar axis using `tree.anchor` when present, clamping to `tree.screen_size`.

- [x] **Step 4: Add tests for settings math**

Add pure helper tests for left/right/top/bottom:

```rust
#[test]
fn left_edge_panel_opens_right_of_bar_and_aligns_to_anchor_y() {
    let position = panel_position_for_test(crate::config::Edge::Left, 48, (360, 600), Some(iced::Rectangle {
        x: 0.0, y: 100.0, width: 48.0, height: 32.0,
    }), (1920, 1080));
    assert_eq!(position.0, 48);
    assert!(position.1 >= 0);
}
```

- [x] **Step 5: Run tests**

Run: `cargo test --lib app::panel app::render`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/app/render.rs src/app/module_events.rs src/app/focus.rs src/app/panel.rs src/app/widget_ops.rs
git commit -m "feat(panel): anchor panels to source modules"
```

---

## Phase 5 - Named Menus and Search

### Task 11: Add global named menu config

**Files:**
- Modify: `src/config/module.rs`
- Modify: `src/config.rs`
- Modify: `src/config/parse/mod.rs`
- Test: `src/config/parse/tests.rs`

- [x] **Step 1: Define menu config types**

In `src/config/module.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuConfig {
    pub name: String,
    pub items: Vec<MenuItemConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItemConfig {
    pub label: String,
    pub keywords: Vec<String>,
    pub action: Option<PanelAction>,
    pub children: Vec<MenuItemConfig>,
}
```

In `Config`, add:

```rust
pub menus: Vec<MenuConfig>,
```

- [x] **Step 2: Parse `menus`**

In `src/config/parse/mod.rs`, parse:

```kdl
menus {
  menu "main" {
    item "Terminal" { command "kitty" }
    item "Apps" {
      item "Browser" { command "firefox" }
    }
  }
}
```

Rules:

- `menu` requires one non-empty name string.
- `item` requires one non-empty label string.
- An item may have exactly one action operation or child `item` nodes.
- An item cannot mix action operation and child item nodes in v1.
- `keywords` is optional and accepts string arguments.

- [x] **Step 3: Validate menu references**

In `src/config/validate.rs`, validate every `open-menu` and `toggle-menu` action points to a declared menu name.

- [x] **Step 4: Add tests**

Add tests for:

- Parsing a menu with one command item.
- Parsing nested menu items.
- Rejecting duplicate menu names.
- Rejecting `open-menu "missing"`.

- [x] **Step 5: Run tests**

Run: `cargo test --lib config::parse::tests config::tests::validate`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/config/module.rs src/config.rs src/config/parse/mod.rs src/config/validate.rs src/config/parse/tests.rs
git commit -m "feat(config): add named panel menus"
```

### Task 12: Render menu mode with search

**Files:**
- Modify: `src/surface/panel.rs`
- Modify: `src/app/panel.rs`
- Modify: `src/app/view.rs`
- Modify: `src/app/focus.rs`

- [x] **Step 1: Add menu flattening helpers**

In `src/app/panel.rs`, add pure helpers:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MenuEntryView {
    pub(super) label: String,
    pub(super) path: Vec<usize>,
    pub(super) is_submenu: bool,
}
```

Implement current-level entries and recursive search entries. Search matches labels and keywords case-insensitively. Search labels include breadcrumbs joined by `" / "`.

- [x] **Step 2: Render search input and entries**

In `src/app/view.rs`, when `PanelContent::Menu(name)`, render:

- A text input bound to `PanelEvent::SearchChanged`.
- A vertical list of visible entries.
- Hover highlight and focused highlight.
- Mouse click activation.

- [x] **Step 3: Keyboard behavior**

In `src/app/focus.rs`, route key events for panel windows:

- Character input updates search through the text input.
- Arrow down/up changes focused item.
- Enter activates focused entry.
- Escape clears search if non-empty; otherwise closes panel.
- Backspace at empty search goes up one submenu level.

- [x] **Step 4: Activation behavior**

In `PanelEvent::Activate`, if focused entry is a submenu, push the submenu path and clear search. If it is actionable, dispatch its `PanelAction` using the panel origin context. If the action is `Command`, close the panel after spawning the command.

- [x] **Step 5: Add tests**

Add pure helper tests:

```rust
#[test]
fn menu_search_flattens_nested_matches_with_breadcrumb() {
    let menu = menu_with_apps_browser();
    let results = search_menu_entries(&menu, "browser");
    assert_eq!(results[0].label, "Apps / Browser");
}

#[test]
fn escape_clears_search_before_closing_panel() {
    let mut panel = PanelSystem::default();
    panel.search_query = "term".into();
    assert!(panel.escape_clears_search());
    assert_eq!(panel.search_query, "");
}
```

- [x] **Step 6: Run tests**

Run: `cargo test --lib app::panel app::focus app::view`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/surface/panel.rs src/app/panel.rs src/app/view.rs src/app/focus.rs
git commit -m "feat(panel): add named menu mode with search"
```

---

## Phase 6 - IPC, Cleanup, Defaults, Docs

### Task 13: Replace expand IPC with panel IPC

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/ipc.rs`
- Modify: `src/app/surfaces.rs`
- Modify: `src/app/messages.rs`

- [x] **Step 1: Rename CLI command**

In `src/cli.rs`, replace:

```rust
"--toggle-expand" => return Command::IpcCommand("toggle-expand".into()),
```

with:

```rust
"--toggle-panel" => return Command::IpcCommand("toggle-panel".into()),
```

- [x] **Step 2: Handle IPC toggle-panel**

In the app IPC handler, map `toggle-panel` to the focused output's Bar Surface when known; if any panel is open, close it. If no focused output is known, open the first deterministic Bar Surface's default panel. Do not toggle every surface.

- [x] **Step 3: Add tests**

Add tests proving:

- `--toggle-panel` emits IPC command `toggle-panel`.
- `toggle-panel` closes an open panel.
- `toggle-panel` opens one panel, not one per surface.

- [x] **Step 4: Run tests**

Run: `cargo test --lib cli ipc app::surfaces`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/ipc.rs src/app/surfaces.rs src/app/messages.rs
git commit -m "refactor(ipc): replace expand command with panel command"
```

### Task 14: Remove old expansion code

**Files:**
- Delete: `src/surface/expand.rs`
- Modify: `src/surface.rs`
- Modify: `src/app/animation.rs`
- Modify: `src/app/subscriptions.rs`
- Modify: `src/app/surfaces.rs`

- [x] **Step 1: Delete `ExpandState`**

Remove `src/surface/expand.rs` and all imports of `crate::surface::expand::ExpandState`.

- [x] **Step 2: Remove expand ticking**

Remove animation tick logic used only by width expansion. Keep tooltip dwell ticking if tooltip code still needs it.

- [x] **Step 3: Remove hover expansion state**

Remove `bar_hovered` and `hover_pending_ms` unless still needed by another feature. Tooltip dwell remains `tooltip_pending`.

- [x] **Step 4: Search for obsolete names**

Run:

```bash
rg "ExpandState|expanded-width|expanded_width|expand-trigger|expand_keybind|hover-delay|toggle-expand|view_expanded|Expansion" src config docs/agents CONTEXT.md
```

Expected: no production config/code matches. Historical planning docs may still match under `docs/superpowers`.

- [x] **Step 5: Run full test suite**

Run: `cargo test --lib`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/surface.rs src/app/animation.rs src/app/subscriptions.rs src/app/surfaces.rs
git rm src/surface/expand.rs
git commit -m "refactor(panel): remove legacy expansion state"
```

### Task 15: Update default configs and domain docs

**Files:**
- Modify: `config/default.kdl`
- Modify: `config/new_default.kdl`
- Modify: `CONTEXT.md`
- Modify: `docs/agents/domain.md`

- [x] **Step 1: Update bundled config**

Replace old expansion fields with:

```kdl
menus {
  menu "main" {
    item "Terminal" { command "kitty" }
    item "Browser" { command "firefox" }
  }
}

bar "main" {
  actions {
    click { open-panel }
    Super+Space { toggle-menu "main" }
  }

  panel {
    width 360
    max-height 600
    start "workspaces"
    center "focused-window"
    end "network" "volume" "battery" "clock"
  }
}
```

Use actual module names present in each default config.

- [x] **Step 2: Update `CONTEXT.md` domain terms**

Replace the old **Expansion** section with:

```markdown
**Panel**:
A transient overlay surface opened from a **Bar Surface**. A Panel is anchored to its originating **Bar Surface** or **Module** and can render module detail content, the Bar's configured panel layout, or named menu content.
_Avoid_: Expansion, popup when referring to the generic Niribar panel

**Panel Origin**:
The **Bar Surface** or **Module** that triggered the current **Panel**. The origin remains visually marked while the Panel is open.
_Avoid_: Active module when referring specifically to panel source

**Panel Action**:
A typed configured operation triggered by pointer input, focused keyboard input, menu item activation, or IPC. Examples: `open-panel`, `toggle-panel`, `open-menu`, `command`.
_Avoid_: Shell action when the action is not a command
```

Update Q&A for per-surface state and `niribar --toggle-panel`.

- [x] **Step 3: Run config load tests**

Run: `cargo test --lib config`

Expected: PASS.

- [x] **Step 4: Commit**

```bash
git add config/default.kdl config/new_default.kdl CONTEXT.md docs/agents/domain.md
git commit -m "docs(panel): update defaults and domain language"
```

### Task 16: Final verification

**Files:**
- No planned edits.

- [x] **Step 1: Run full tests**

Run: `cargo test`

Expected: PASS.

- [x] **Step 2: Run lint if available**

Run: `cargo clippy --all-targets -- -D warnings`

Expected: PASS or existing unrelated warnings documented in the final handoff.

- [x] **Step 3: Run obsolete vocabulary check**

Run:

```bash
rg "toggle-expand|expand-trigger|expanded-width|view_expanded|ExpandState" src config CONTEXT.md
```

Expected: no matches.

- [ ] **Step 4: Manual smoke test**

Run Niribar with the updated default config and verify:

- Bar remains collapsed when opening panel.
- Bar click on empty background opens the bar panel.
- Module configured with `click { open-panel }` opens only that module panel.
- `Super+Space` while focused opens menu mode.
- Menu search focuses immediately.
- Escape clears search first, then closes panel.
- Opening tray SNI menu closes any panel.
- Opening panel closes any tooltip/menu.

- [ ] **Step 5: Commit final fixes**

```bash
git add .
git commit -m "test(panel): verify panel rework"
```

---

## Self-Review

- Spec coverage: The plan covers replacement semantics, generic panel lifecycle, typed actions, bar/module/menu content, keyboard behavior, menu search, IPC rename, dynamic module layout, mutual exclusion with menus/tooltips, and domain docs.
- Placeholder scan: No task uses TBD/TODO language. Every validation and parse behavior has concrete expected outcomes.
- Type consistency: `ActionTrigger`, `PanelAction`, `ActionsConfig`, `PanelOrigin`, `PanelContent`, `PanelTree`, and `PanelSystem` are introduced before use in later tasks.
