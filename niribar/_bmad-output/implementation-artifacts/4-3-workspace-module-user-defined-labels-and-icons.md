# Story 4.3: Workspace Module — User-Defined Labels and Icons

Status: review

## Story

As a **user**,
I want to assign custom labels and icons to my workspaces in config,
so that my workspace indicators are meaningful rather than just numbers.

## Acceptance Criteria

1. **Given** the config contains workspace label/icon assignments (e.g., `workspace 1 { label "web" icon ">" }`)
   **When** the workspace module renders in expanded state
   **Then** the configured label is shown alongside the workspace number in the pill

2. **Given** the config contains workspace icon assignments
   **When** the workspace module renders in collapsed state
   **Then** the configured icon is shown in the 32×20px pill (in place of the number)

3. **Given** the config contains workspace icon assignments
   **When** the workspace module renders in expanded state
   **Then** the configured icon is shown alongside the number (and label, if configured)

4. **Given** a workspace has no configured label or icon
   **When** the workspace module renders in either state
   **Then** collapsed shows the workspace number only; expanded shows the workspace number only

5. **Given** workspace label/icon config exists for workspace number N
   **When** a workspace with `ws.idx + 1 == N` renders
   **Then** the config is applied to that workspace regardless of its niri stack position

6. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

7. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 158 pre-existing tests pass and at least 8 new tests pass

## Tasks / Subtasks

- [x] Task 1: Update `src/config/mod.rs` — add `WorkspacesConfig` and `ModuleParams::Workspaces` (AC: #1–#5)
  - [x] 1.1: Add `WorkspaceLabelConfig` struct after `ButtonConfig`:
    ```rust
    /// Per-workspace label and icon for the workspaces module.
    #[derive(Debug, Clone)]
    pub struct WorkspaceLabelConfig {
        pub label: Option<String>,
        pub icon: Option<String>,
    }
    ```

  - [x] 1.2: Add `WorkspacesConfig` struct with `Default` impl after `WorkspaceLabelConfig`:
    ```rust
    /// Configuration for the workspaces module — optional label/icon per workspace number.
    #[derive(Debug, Clone)]
    pub struct WorkspacesConfig {
        /// Maps workspace number (1-based: ws.idx + 1) to label/icon config.
        /// Absent key → workspace shows number only (AC #4).
        pub workspaces: std::collections::HashMap<u32, WorkspaceLabelConfig>,
    }

    impl Default for WorkspacesConfig {
        fn default() -> Self {
            WorkspacesConfig {
                workspaces: std::collections::HashMap::new(),
            }
        }
    }
    ```

  - [x] 1.3: Add `Workspaces(WorkspacesConfig)` variant to `ModuleParams` enum after `Button`:
    ```rust
    Workspaces(WorkspacesConfig),
    ```

- [x] Task 2: Update `src/config/parse.rs` — parse workspaces config from KDL (AC: #1–#5)
  - [x] 2.1: Add import for `WorkspacesConfig` and `WorkspaceLabelConfig` to the `use super::{}` block:
    ```rust
    use super::{
        BarConfig, ButtonConfig, ClockConfig, Config, Edge, ExpandTrigger,
        ModuleConfig, ModuleParams, ScriptConfig, WorkspaceLabelConfig, WorkspacesConfig,
    };
    ```

  - [x] 2.2: Add `parse_workspaces_params()` function after `parse_button_params`:
    ```rust
    fn parse_workspaces_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<WorkspacesConfig> {
        let mut config = WorkspacesConfig::default();

        let Some(children) = node.children() else {
            return Ok(config); // No workspace config — all workspaces show number only.
        };

        for ws_node in children.nodes() {
            if ws_node.name().value() != "workspace" {
                anyhow::bail!(
                    "modules[{idx}].workspaces: unknown child node {:?}. Expected: \"workspace\".",
                    ws_node.name().value()
                );
            }

            let ws_num = ws_node
                .entries()
                .iter()
                .find(|e| e.name().is_none())
                .and_then(|e| e.value().as_integer())
                .and_then(|n| if n >= 1 { Some(n as u32) } else { None })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "modules[{idx}].workspaces.workspace: missing or invalid workspace number. \
                         Expected: a positive integer (1-based workspace number, e.g. 1)."
                    )
                })?;

            let (label, icon) = if let Some(ws_children) = ws_node.children() {
                let label = ws_children
                    .get("label")
                    .and_then(|n| n.entries().first())
                    .and_then(|e| e.value().as_string())
                    .map(|s| s.to_string());
                let icon = ws_children
                    .get("icon")
                    .and_then(|n| n.entries().first())
                    .and_then(|e| e.value().as_string())
                    .map(|s| s.to_string());
                (label, icon)
            } else {
                (None, None)
            };

            config
                .workspaces
                .insert(ws_num, WorkspaceLabelConfig { label, icon });
        }

        Ok(config)
    }
    ```

  - [x] 2.3: In `parse_modules_section()`, add the `"workspaces"` branch in the params `if`/`else if` chain (before the `else` fallback):
    ```rust
    } else if name == "workspaces" {
        ModuleParams::Workspaces(parse_workspaces_params(node, i)?)
    } else {
        ModuleParams::None
    };
    ```

- [x] Task 3: Update `src/modules/workspaces.rs` — store config, update `view_*` (AC: #1–#5)
  - [x] 3.1: Update `WorkspacesModule` to store config:
    ```rust
    pub struct WorkspacesModule {
        id: crate::modules::ModuleId,
        config: crate::config::WorkspacesConfig,
    }
    ```

  - [x] 3.2: Update `new()` to accept config:
    ```rust
    impl WorkspacesModule {
        pub fn new(config: crate::config::WorkspacesConfig) -> Self {
            WorkspacesModule {
                id: crate::modules::ModuleId::new("workspaces"),
                config,
            }
        }
    }
    ```

  - [x] 3.3: Replace `view_collapsed()` body to show icon when configured (AC: #2, #4):
    ```rust
    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, container, text};
        use iced::{Alignment, Background, Color, Length};

        let workspaces = match state {
            ModuleState::Workspace(s) => &s.workspaces,
            _ => return iced::widget::Space::new().into(),
        };

        let mut sorted: Vec<_> = workspaces.iter().collect();
        sorted.sort_by_key(|ws| ws.idx);

        let pills: Vec<iced::Element<'_, crate::app::Message>> = sorted
            .iter()
            .map(|ws| {
                let is_focused = ws.is_focused;
                let bg = if is_focused {
                    Color::from_rgb8(0x89, 0xb4, 0xfa) // color.accent
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.0)
                };
                let fg = if is_focused {
                    Color::from_rgb8(0x1e, 0x1e, 0x2e) // color.background
                } else {
                    Color::from_rgb8(0xcd, 0xd6, 0xf4) // color.text
                };
                let ws_num = (ws.idx as u32) + 1;
                let label_cfg = self.config.workspaces.get(&ws_num);
                // Collapsed: show icon if configured, else number (AC #2, #4).
                let content = label_cfg
                    .and_then(|c| c.icon.as_deref())
                    .map(|icon| icon.to_string())
                    .unwrap_or_else(|| format!("{}", ws_num));
                container(text(content).size(10).color(fg))
                    .width(32)
                    .height(20)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(bg)),
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .into()
            })
            .collect();

        column(pills).spacing(2).width(Length::Fill).into()
    }
    ```

  - [x] 3.4: Replace `view_expanded()` body to show icon + number + label when configured (AC: #1, #3, #4):
    ```rust
    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, container, row, text};
        use iced::{Alignment, Background, Color, Length};

        let workspaces = match state {
            ModuleState::Workspace(s) => &s.workspaces,
            _ => return iced::widget::Space::new().into(),
        };

        let mut sorted: Vec<_> = workspaces.iter().collect();
        sorted.sort_by_key(|ws| ws.idx);

        let pills: Vec<iced::Element<'_, crate::app::Message>> = sorted
            .iter()
            .map(|ws| {
                let is_focused = ws.is_focused;
                let bg = if is_focused {
                    Color::from_rgb8(0x89, 0xb4, 0xfa)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.0)
                };
                let fg = if is_focused {
                    Color::from_rgb8(0x1e, 0x1e, 0x2e)
                } else {
                    Color::from_rgb8(0xcd, 0xd6, 0xf4)
                };
                let ws_num = (ws.idx as u32) + 1;
                let label_cfg = self.config.workspaces.get(&ws_num);
                let icon_str = label_cfg.and_then(|c| c.icon.as_deref());
                let label_str = label_cfg.and_then(|c| c.label.as_deref());

                // Expanded: icon (if any) + number + label (if any), space-separated (AC #1, #3).
                let mut parts: Vec<iced::Element<'_, crate::app::Message>> = Vec::new();
                if let Some(icon) = icon_str {
                    parts.push(text(icon).size(12).color(fg).into());
                }
                parts.push(text(format!("{}", ws_num)).size(12).color(fg).into());
                if let Some(lbl) = label_str {
                    parts.push(text(lbl).size(12).color(fg).into());
                }

                container(row(parts).spacing(4).align_y(Alignment::Center))
                    .width(Length::Fill)
                    .height(20)
                    .align_x(Alignment::Start)
                    .align_y(Alignment::Center)
                    .padding([0, 6])
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(bg)),
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .into()
            })
            .collect();

        column(pills).spacing(2).width(Length::Fill).into()
    }
    ```

- [x] Task 4: Update `src/modules/mod.rs` — update `workspaces_ctor` to pass config (AC: #1–#5)
  - [x] 4.1: Replace the current `workspaces_ctor` (which calls `WorkspacesModule::new()` with no args) with:
    ```rust
    fn workspaces_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
        let config = match &mc.params {
            crate::config::ModuleParams::Workspaces(c) => c.clone(),
            _ => crate::config::WorkspacesConfig::default(),
        };
        Box::new(workspaces::WorkspacesModule::new(config))
    }
    ```

- [x] Task 5: Tests (AC: #6, #7)
  - [x] 5.1: In `src/config/parse.rs` tests (add to existing `#[cfg(test)]` block):
    - `parse_workspaces_with_label_and_icon` — KDL with `workspace 1 { label "web" icon ">" }`, verify `HashMap[1].label == Some("web")` and `.icon == Some(">")`
    - `parse_workspaces_no_children_is_empty` — `module "workspaces" zone="top"` with no block → `WorkspacesConfig` with empty map
    - `parse_workspaces_label_only` — `workspace 1 { label "web" }` → `icon == None`
    - `parse_workspaces_icon_only` — `workspace 1 { icon ">" }` → `label == None`
    - `parse_workspaces_multiple_workspaces` — workspace 1 and 3 configured, workspace 2 absent → map has 2 entries, `get(&2) == None`
    - `parse_workspaces_invalid_workspace_number_errors` — `workspace 0 { label "bad" }` → error message contains "workspace number"

  - [x] 5.2: In `src/modules/workspaces.rs` tests (add to existing `#[cfg(test)]` block):
    - `workspaces_config_default_is_empty` — `WorkspacesConfig::default().workspaces.is_empty()`
    - `workspaces_config_lookup_returns_none_for_unconfigured` — `WorkspacesConfig::default().workspaces.get(&1) == None`

- [x] Task 6: Quality gates (AC: #6, #7)
  - [x] 6.1: `cargo build` — succeeds
  - [x] 6.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 6.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 6.4: `cargo test` — 10 new tests + all 158 pre-existing pass (168 total)

## Dev Notes

### What Exists After Story 4.2

- `src/modules/workspaces.rs`: `WorkspacesModule`, `WorkspaceState`, full `Module` trait impl (collapsed 32×20px pills, expanded full-width pills, `is_focused`→accent color). `WorkspacesModule::new()` currently takes **no arguments**.
- `src/modules/mod.rs`: `pub mod workspaces`, `ModuleState::Workspace(workspaces::WorkspaceState)`, `workspaces_ctor`, `build_registry()` entry.
- `src/config/mod.rs`: `ModuleParams` has `None`, `Clock(ClockConfig)`, `Script(ScriptConfig)`, `Button(ButtonConfig)` — no `Workspaces` variant yet.
- `src/config/parse.rs`: `parse_modules_section()` dispatches by name — `"workspaces"` falls through to `ModuleParams::None`.
- 158 tests currently passing.

### Key Design: Config is per-workspace-number (1-based)

AC #5: config is keyed by workspace **number** (1-based), not by niri stack position or `ws.idx`. Mapping: `workspace_number = ws.idx + 1`.

```
User config: workspace 1 { label "web" }
                        ↑ workspace number (1-based)
Runtime:     ws.idx == 0  → ws_num = 0 + 1 = 1 → matches config
```

`HashMap<u32, WorkspaceLabelConfig>` in `WorkspacesConfig` — O(1) lookup per pill.

### KDL Config Format (New Syntax for Story 4.3)

```kdl
module "workspaces" zone="top" {
    workspace 1 {
        label "web"
        icon ">"
    }
    workspace 2 {
        label "code"
    }
    workspace 3 {
        icon ">"
    }
}
```

- `workspace N` — node name `"workspace"`, first positional argument is the workspace number
- `label` and `icon` are optional child nodes; both being absent is valid (no-op entry)
- Workspaces not listed in config show number only (AC #4)
- This mirrors the existing `button` param pattern: optional children block, gracefully absent → defaults

### Module Constructor Change

Story 4.2 `workspaces_ctor` ignores `mc` (the `_mc: &ModuleConfig` placeholder). Story 4.3 must extract `WorkspacesConfig` from `mc.params`:

```rust
// Before (4.2):
fn workspaces_ctor(_mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    Box::new(workspaces::WorkspacesModule::new())
}

// After (4.3):
fn workspaces_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
    let config = match &mc.params {
        crate::config::ModuleParams::Workspaces(c) => c.clone(),
        _ => crate::config::WorkspacesConfig::default(),
    };
    Box::new(workspaces::WorkspacesModule::new(config))
}
```

The `_ =>` arm handles the case where someone passes a workspaces module with `ModuleParams::None` (e.g., from tests using old-style `params: ModuleParams::None`) — falls back to empty config, safe.

### view_collapsed: Icon Replaces Number

Collapsed state is 32×20px — space is minimal. When icon is configured, it replaces the workspace number (not added alongside). This matches UX-DR4: "32×20px pill, number only" — the icon serves the same function as the number (identity signal). Without icon, falls back to number exactly as before (zero regression risk).

```rust
let content = label_cfg
    .and_then(|c| c.icon.as_deref())
    .map(|icon| icon.to_string())
    .unwrap_or_else(|| format!("{}", ws_num));
```

### view_expanded: row() with Parts

Import `row` in addition to the existing imports. The expanded pill uses a `row(parts)` to lay out icon + number + label. Use `padding([0, 6])` (6px horizontal) instead of `align_x(Alignment::Center)` for the expanded pill — this gives left-aligned content with breathing room, matching UX "icon + label left-aligned" intent from UX-DR6.

**Import addition required:**
```rust
use iced::widget::{column, container, row, text};
```
Add `row` to the existing import in `view_expanded`. `view_collapsed` does not need `row`.

### No Changes to: app.rs, default.kdl, WorkspaceState, niri IPC flow

- `WorkspaceState` remains unchanged — it only holds `Vec<niri_ipc::Workspace>` (live niri data). Config is stored in the module struct, not state.
- The `WorkspacesChanged` handler in `app.rs` remains unchanged — it still pushes `ModuleState::Workspace(WorkspaceState { workspaces: ... })`.
- `config/default.kdl` does not include workspaces module — no change needed.
- No changes to `KNOWN_MODULE_NAMES` — "workspaces" is already registered.

### Test Helpers

The existing `make_workspace(idx, is_focused)` helper in `workspaces.rs` tests is reusable. For `WorkspacesConfig` tests:

```rust
fn make_workspaces_config_with_label(ws_num: u32, label: &str) -> WorkspacesConfig {
    use crate::config::{WorkspaceLabelConfig, WorkspacesConfig};
    let mut config = WorkspacesConfig::default();
    config.workspaces.insert(ws_num, WorkspaceLabelConfig {
        label: Some(label.to_string()),
        icon: None,
    });
    config
}
```

### Parse Test Structure for Workspaces

Follow the established pattern from button tests — assert on `ModuleParams::Workspaces(cfg)`:

```rust
fn parse_workspaces_with_label_and_icon() {
    let kdl = r#"
modules {
    module "workspaces" zone="top" {
        workspace 1 {
            label "web"
            icon ">"
        }
    }
}
"#;
    let config = parse(kdl).unwrap();
    match &config.modules[0].params {
        ModuleParams::Workspaces(cfg) => {
            let ws1 = cfg.workspaces.get(&1).expect("workspace 1 should be configured");
            assert_eq!(ws1.label.as_deref(), Some("web"));
            assert_eq!(ws1.icon.as_deref(), Some(">"));
        }
        _ => panic!("expected ModuleParams::Workspaces"),
    }
}
```

### File Locations

| What | Where |
|---|---|
| `WorkspaceLabelConfig`, `WorkspacesConfig` | `src/config/mod.rs` — after `ButtonConfig` |
| `ModuleParams::Workspaces(WorkspacesConfig)` | `src/config/mod.rs` — in `ModuleParams` enum |
| `parse_workspaces_params()` | `src/config/parse.rs` — after `parse_button_params()` |
| Workspaces branch in `parse_modules_section()` | `src/config/parse.rs` — `if/else if` chain |
| `WorkspacesModule` struct + `new(config)` update | `src/modules/workspaces.rs` |
| `workspaces_ctor` update | `src/modules/mod.rs` |

### References

- Epic 4 §"Story 4.3": user story + ACs (per-workspace-number keying, icon in collapsed+expanded, label in expanded, fallback to number)
- Story 4.2 dev notes: existing `WorkspacesModule::new()` signature, `WorkspaceState` structure, data flow, color tokens, `make_workspace` helper
- Architecture §"Config Boundary": `Config` fully parsed before surfaces — `WorkspacesConfig` is part of static config, not runtime state
- Architecture §"Module System": module constructor pattern, `ModuleParams` extensibility
- UX-DR4: WorkspacePill — collapsed 32×20px / expanded full-width; active `color.accent`; "collapsed (32×20px, number only) and expanded (full-width, number + label)"
- UX-DR6: ModuleLabel — icon + label left-aligned in expanded state
- Story 2.5 (button): `ButtonConfig`, `ModuleParams::Button`, `parse_button_params()` — direct parallel for `WorkspacesConfig`/`parse_workspaces_params()`
- `src/config/parse.rs:68-76`: current `if/else if` chain where workspaces branch is added

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

- Added `WorkspaceLabelConfig` and `WorkspacesConfig` (with `#[derive(Default)]`) to `src/config/mod.rs`; added `ModuleParams::Workspaces(WorkspacesConfig)` variant
- Added `parse_workspaces_params()` to `src/config/parse.rs`; parses `workspace N { label "..." icon "..." }` KDL child nodes into `HashMap<u32, WorkspaceLabelConfig>`; added "workspaces" branch in `parse_modules_section()` dispatch chain
- Updated `WorkspacesModule` to store `config: WorkspacesConfig`; `new()` now accepts config; `view_collapsed` shows icon (if configured) or number fallback; `view_expanded` builds a `row()` of icon (optional) + number + label (optional)
- Updated `workspaces_ctor` in `src/modules/mod.rs` to extract `WorkspacesConfig` from `ModuleParams::Workspaces`; falls back to `WorkspacesConfig::default()` for other params (safe for existing app.rs tests using `ModuleParams::None`)
- 10 new tests added (6 in parse.rs, 4 in workspaces.rs); 168 total tests pass; `cargo clippy -- -D warnings` zero warnings; `cargo fmt --check` passes

### File List

- src/config/mod.rs
- src/config/parse.rs
- src/modules/workspaces.rs
- src/modules/mod.rs

## Change Log

| Date | Summary |
|---|---|
| 2026-04-04 | Story context created |
| 2026-04-04 | Implemented: WorkspacesConfig/WorkspaceLabelConfig in config, KDL parsing for workspace labels/icons, WorkspacesModule updated to render icon+label; 168 tests pass |
