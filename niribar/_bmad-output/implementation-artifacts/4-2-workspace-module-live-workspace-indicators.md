# Story 4.2: Workspace Module — Live Workspace Indicators

Status: review

## Story

As a **user on Niri**,
I want the bar to show all my workspaces as visual indicators,
so that I can see which workspaces exist and which one is active at a glance.

## Acceptance Criteria

1. **Given** `workspaces` is in the module list and niribar is running on Niri
   **When** the bar renders
   **Then** each workspace is shown as a `WorkspacePill` widget (a styled container per workspace)

2. **Given** the bar is rendering workspace pills
   **When** a workspace is the focused/active workspace
   **Then** that pill has `color.accent` (`#89b4fa`) background with `color.background` (`#1e1e2e`) text

3. **Given** the bar is rendering workspace pills
   **When** a workspace is NOT the focused workspace
   **Then** that pill has transparent background with `color.text` (`#cdd6f4`) text

4. **Given** the bar is in collapsed state
   **When** workspace pills render
   **Then** each pill is exactly 32×20px showing the workspace number only (`ws.idx + 1`)

5. **Given** the bar is in expanded state
   **When** workspace pills render
   **Then** each pill fills the full module width (32px→full) showing the workspace number
   (labels are Story 4.3 scope; no label config exists yet)

6. **Given** niri emits workspace data
   **When** pills are rendered
   **Then** workspace order matches niri's scrollable-tiling order (sorted by `ws.idx` ascending, displayed top-to-bottom)

7. **Given** a workspace becomes focused
   **When** niri emits `WorkspacesChanged`
   **Then** the active pill updates to `color.accent` background within one rendered frame (NFR4)

8. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

9. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 148 pre-existing tests pass and at least 8 new tests pass

## Tasks / Subtasks

- [x] Task 1: Extend `src/modules/mod.rs` — register module and add state/event variants (AC: #1)
  - [x] 1.1: Add `pub mod workspaces;` after `pub mod script;`:
    ```rust
    pub mod workspaces;
    ```

  - [x] 1.2: Add `Workspace(workspaces::WorkspaceState)` variant to `ModuleState`, after `Button`:
    ```rust
    Workspace(workspaces::WorkspaceState),
    ```

  - [x] 1.3: Register in `build_registry()` — add after the `"script"` entry:
    ```rust
    map.insert("workspaces", workspaces_ctor as ModuleConstructor);
    ```
    And add the constructor function after `script_ctor`:
    ```rust
    fn workspaces_ctor(_mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
        Box::new(workspaces::WorkspacesModule::new())
    }
    ```

- [x] Task 2: Create `src/modules/workspaces.rs` (AC: #1–#7)
  - [x] 2.1: Define `WorkspaceState`:
    ```rust
    /// Runtime state for the workspaces module — current workspace list from niri.
    ///
    /// Populated by the WorkspacesChanged niri IPC event via app.rs update().
    /// Stories 4.2–4.3 read this directly; Story 4.5 may set Unavailable.
    #[derive(Debug, Clone, Default)]
    pub struct WorkspaceState {
        pub workspaces: Vec<niri_ipc::Workspace>,
    }
    ```

  - [x] 2.2: Define `WorkspacesModule`:
    ```rust
    /// Workspace module: shows all niri workspaces as pills, fed by niri IPC via SharedState.
    pub struct WorkspacesModule {
        id: crate::modules::ModuleId,
    }

    impl WorkspacesModule {
        pub fn new() -> Self {
            WorkspacesModule {
                id: crate::modules::ModuleId::new("workspaces"),
            }
        }
    }
    ```

  - [x] 2.3: Implement `Module` trait for `WorkspacesModule`:
    ```rust
    impl crate::modules::Module for WorkspacesModule {
        fn id(&self) -> crate::modules::ModuleId {
            self.id.clone()
        }

        /// Workspace module has no independent data source — fed via Message::Niri in app.rs.
        fn subscription(&self) -> iced::Subscription<crate::app::Message> {
            iced::Subscription::none()
        }

        fn view_collapsed(
            &self,
            state: &crate::modules::ModuleState,
        ) -> iced::Element<'_, crate::app::Message> {
            use crate::modules::ModuleState;
            use iced::widget::{column, container, text};
            use iced::{Alignment, Background, Color, Length};

            let workspaces = match state {
                ModuleState::Workspace(s) => &s.workspaces,
                _ => return iced::widget::Space::new().into(), // Loading/Unavailable → empty (UX-DR16)
            };

            // Sort by idx to match Niri's scrollable-tiling order (AC #6).
            let mut sorted: Vec<_> = workspaces.iter().collect();
            sorted.sort_by_key(|ws| ws.idx);

            let pills: Vec<iced::Element<'_, crate::app::Message>> = sorted
                .iter()
                .map(|ws| {
                    let is_focused = ws.is_focused;
                    let bg = if is_focused {
                        Color::from_rgb8(0x89, 0xb4, 0xfa) // color.accent
                    } else {
                        Color::from_rgba(0.0, 0.0, 0.0, 0.0) // transparent
                    };
                    let fg = if is_focused {
                        Color::from_rgb8(0x1e, 0x1e, 0x2e) // color.background (dark on accent)
                    } else {
                        Color::from_rgb8(0xcd, 0xd6, 0xf4) // color.text
                    };
                    container(
                        text(format!("{}", ws.idx + 1))
                            .size(10)
                            .color(fg),
                    )
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

            column(pills)
                .spacing(2)
                .width(Length::Fill)
                .into()
        }

        fn view_expanded(
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
                        Color::from_rgb8(0x89, 0xb4, 0xfa)
                    } else {
                        Color::from_rgba(0.0, 0.0, 0.0, 0.0)
                    };
                    let fg = if is_focused {
                        Color::from_rgb8(0x1e, 0x1e, 0x2e)
                    } else {
                        Color::from_rgb8(0xcd, 0xd6, 0xf4)
                    };
                    // Story 4.3 will add label alongside number.
                    // For 4.2, full-width pill shows number only.
                    container(
                        text(format!("{}", ws.idx + 1))
                            .size(12)
                            .color(fg),
                    )
                    .width(Length::Fill)
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

            column(pills)
                .spacing(2)
                .width(Length::Fill)
                .into()
        }
    }
    ```

- [x] Task 3: Update `src/app.rs` — propagate WorkspacesChanged to module state (AC: #7)
  - [x] 3.1: Add `use crate::modules::workspaces;` import (after existing `crate::modules::` imports):
    ```rust
    use crate::modules::workspaces;
    ```

  - [x] 3.2: Extend the `WorkspacesChanged` arm in `update()`. Currently:
    ```rust
    Event::WorkspacesChanged { workspaces } => {
        self.shared.niri.workspaces = workspaces;
    }
    ```
    Replace with:
    ```rust
    Event::WorkspacesChanged { workspaces } => {
        // Update each workspace module's ModuleState so view_*() gets fresh data (AC #7).
        for (_, module) in &self.modules {
            if module.id() == ModuleId::new("workspaces") {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::Workspace(workspaces::WorkspaceState {
                        workspaces: workspaces.clone(),
                    }),
                );
            }
        }
        self.shared.niri.workspaces = workspaces;
    }
    ```

- [x] Task 4: Tests (AC: #8, #9)
  - [x] 4.1: In `src/modules/workspaces.rs` (add `#[cfg(test)]` block):
    - `workspaces_module_id_is_workspaces` — `WorkspacesModule::new().id().0 == "workspaces"`
    - `workspace_state_default_is_empty` — `WorkspaceState::default().workspaces.is_empty()`
    - `workspace_state_stores_workspaces` — create a `WorkspaceState` with one workspace, verify len == 1

  - [x] 4.2: In `src/modules/mod.rs` tests (add to existing `#[cfg(test)]` block):
    - `build_registry_contains_workspaces` — `build_registry().contains_key("workspaces")`

  - [x] 4.3: In `src/app.rs` tests (add to existing `#[cfg(test)]` block — import `niri_ipc::Workspace`):
    - `workspaces_module_state_updated_on_workspace_change` — configure app with `workspaces` module, send `Message::Niri(NiriEvent::Event(Event::WorkspacesChanged { workspaces: vec![...] }))`, verify `shared.modules[ModuleId("workspaces")] == ModuleState::Workspace(s)` with `s.workspaces.len() == 1`
    - `workspace_module_focused_workspace_reflected_in_state` — send two workspaces (one focused), verify the focused flag on the stored state
    - `workspaces_module_state_not_updated_without_workspaces_module_in_config` — send WorkspacesChanged on an app with no workspaces module, verify `shared.modules` does NOT gain a Workspace entry
    - `workspaces_changed_order_preserved_in_state` — send 3 workspaces with idx 2,0,1, verify stored state has them in arrival order (view sorts — state doesn't need to)

- [x] Task 5: Quality gates (AC: #8, #9)
  - [x] 5.1: `cargo build` — succeeds
  - [x] 5.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 5.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 5.4: `cargo test` — 8 new tests + all 148 pre-existing pass (156 total)

## Dev Notes

### What Exists After Story 4.1

- `shared.niri.workspaces: Vec<niri_ipc::Workspace>` — populated by `WorkspacesChanged` handler in `app.rs`
- `shared.niri.connected: bool` — true when niri IPC is live
- `Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {...}))` handler already stores workspaces in `shared.niri.workspaces`
- `_ => {}` catch-all in Event match arm already exists — Story 4.2 replaces the `WorkspacesChanged` arm body only
- 148 tests pass. `build_registry()` currently maps `"button"`, `"clock"`, `"script"`
- `ModuleState` has: Loading, Unavailable, Clock, Script, Button — add Workspace variant

### niri_ipc::Workspace Fields (v25.11.0)

```rust
pub struct Workspace {
    pub id: u64,          // unique ID (stable across sessions)
    pub idx: u8,          // 0-based position in scrollable stack; display as (idx + 1)
    pub name: Option<String>, // niri-assigned name (NOT niribar config label — that's Story 4.3)
    pub output: Option<String>,
    pub is_urgent: bool,
    pub is_active: bool,  // visible on its output (can be multiple in multi-monitor)
    pub is_focused: bool, // has keyboard focus — exactly ONE per compositor; use for accent pill
    pub active_window_id: Option<u64>,
}
```

**Use `is_focused` for accent color** — not `is_active`. In multi-monitor setups, multiple workspaces can be `is_active` (one per output), but only one is `is_focused`.

**Sort by `ws.idx` ascending** when rendering — this preserves Niri's scrollable-tiling order (top workspace = idx 0, rendered at top of column).

**Display `ws.idx + 1`** as the workspace number (niri is 1-indexed to users; `idx` is 0-based internally).

### Data Flow: WorkspacesChanged → WorkspacePill

```
niri IPC event
    → NiriSubState::Running reads event from OS thread channel
    → Message::Niri(NiriEvent::Event(Event::WorkspacesChanged { workspaces }))
    → app.update() handler:
        → loop self.modules, find "workspaces" module
        → shared.modules.insert(id, ModuleState::Workspace(WorkspaceState { workspaces.clone() }))
        → shared.niri.workspaces = workspaces   (for future stories 4.4/4.5)
    → view() calls module.view_collapsed/view_expanded(shared.modules.get(&id))
    → WorkspacesModule reads ModuleState::Workspace(s) → renders pills
```

This is the same data flow as clock/script/button — the module's state lives in `shared.modules`, not in `shared.niri`. The workspace module does NOT subscribe to `Message::Niri` directly; `app.rs` bridges it.

### Workspace Module: No Independent Subscription

`WorkspacesModule.subscription()` returns `iced::Subscription::none()`. The module has no data source of its own — it is passive. The data arrives via `Message::Niri` in `app.rs.update()` which pushes updates into `shared.modules`.

Contrast with `ClockModule.subscription()` which actively fires `Message::Module(id, ClockEvent::Tick)` every second.

### Graceful Degradation for Story 4.2

Story 4.2 scope: on non-Niri compositors, `WorkspacesChanged` never fires → `ModuleState` stays `Loading` → `view_*()` returns `Space::new()` (empty). This is acceptable for 4.2; Story 4.5 will explicitly set `ModuleState::Unavailable` for niri modules when `NiriEvent::Unavailable` fires.

Do NOT add `Unavailable` handling in Story 4.2.

### Color Tokens (Hardcoded — No Theme System Yet)

| Token | Hex | Use |
|---|---|---|
| `color.accent` | `#89b4fa` — `Color::from_rgb8(0x89, 0xb4, 0xfa)` | Active/focused pill background |
| `color.background` | `#1e1e2e` — `Color::from_rgb8(0x1e, 0x1e, 0x2e)` | Text on accent pill |
| `color.text` | `#cdd6f4` — `Color::from_rgb8(0xcd, 0xd6, 0xf4)` | Text on inactive pill |
| transparent | `Color::from_rgba(0.0, 0.0, 0.0, 0.0)` | Inactive pill background |

These are hardcoded per the existing pattern in `app.rs` (no theme token system yet).

### Iced Container Style with Move Closure

The pill background color varies per workspace (focused vs inactive). Use `move |_theme|` to capture the `Color` value per pill:

```rust
let bg = if ws.is_focused { accent } else { transparent };
// Color is Copy — move closure captures by copy (no lifetime issues)
container(...).style(move |_theme| container::Style {
    background: Some(Background::Color(bg)),
    border: iced::Border { radius: 4.0.into(), ..Default::default() },
    ..Default::default()
})
```

`iced::Color` is `Copy`. The move closure captures `bg` by copy — this is idiomatic for per-item styling in iced.

### app.rs: WorkspacesChanged Handler Modification

Current (after Story 4.1):
```rust
Event::WorkspacesChanged { workspaces } => {
    self.shared.niri.workspaces = workspaces;
}
```

Replace with (Story 4.2 — extends the existing arm):
```rust
Event::WorkspacesChanged { workspaces } => {
    for (_, module) in &self.modules {
        if module.id() == ModuleId::new("workspaces") {
            self.shared.modules.insert(
                module.id(),
                ModuleState::Workspace(workspaces::WorkspaceState {
                    workspaces: workspaces.clone(),
                }),
            );
        }
    }
    self.shared.niri.workspaces = workspaces;
}
```

Requires adding `use crate::modules::workspaces;` to the imports in `app.rs`.

### config.rs: No Changes Required

`"workspaces"` is already in `KNOWN_MODULE_NAMES` (`src/config/mod.rs:15`). Config validates it. `ModuleParams::None` is correct for Story 4.2 (no workspace config params yet). Story 4.3 will add `ModuleParams::Workspaces(WorkspacesConfig)`.

### Test Helpers for app.rs Tests

To test workspace module state updates, configure an app with a workspaces module:
```rust
fn make_workspaces_app() -> App {
    use crate::config::{ModuleConfig, ModuleParams};
    use crate::modules::Zone;
    let mut config = Config::default();
    config.modules = vec![ModuleConfig {
        name: "workspaces".into(),
        zone: Zone::Top,
        params: ModuleParams::None,
    }];
    App::new(config)
}

fn make_workspace(idx: u8, is_focused: bool) -> niri_ipc::Workspace {
    niri_ipc::Workspace {
        id: idx as u64 + 1,
        idx,
        name: None,
        output: Some("eDP-1".to_string()),
        is_urgent: false,
        is_active: is_focused,
        is_focused,
        active_window_id: None,
    }
}
```

### File Locations

| What | Where |
|---|---|
| `pub mod workspaces;` | `src/modules/mod.rs` — after `pub mod script;` |
| `ModuleState::Workspace(...)` | `src/modules/mod.rs` — ModuleState enum |
| `WorkspacesModule`, `WorkspaceState` | `src/modules/workspaces.rs` (new file) |
| `workspaces_ctor`, registry entry | `src/modules/mod.rs` — build_registry() |
| WorkspacesChanged extension | `src/app.rs` — update() niri event handler |
| `use crate::modules::workspaces;` | `src/app.rs` — imports |
| Tests | `src/modules/workspaces.rs`, `src/modules/mod.rs`, `src/app.rs` |

### References

- Epic 4 §"Story 4.2": user story + 7 ACs
- Architecture §"Module System": Module trait, naming, ModuleState/ModuleEvent patterns
- Architecture §"Naming Conventions": `WorkspaceState`, `workspaces.rs` path
- Architecture §"SharedState": `shared.modules` is the store; module data lives there
- Architecture §"Communication Patterns": `Message::Module(ModuleId, ModuleEvent)` pattern (workspaces bypasses ModuleEvent, fed via app.rs directly)
- UX §"WorkspacePill": 32×20px collapsed / full-width expanded / `color.accent` active
- UX §Color Tokens: accent #89b4fa, background #1e1e2e, text #cdd6f4
- UX §Journey 3 "Workspace Interaction": click/scroll deferred to post-4.2 stories
- Story 4.1 dev notes: `shared.niri.workspaces`, WorkspacesChanged handler, `_ => {}` catch-all
- Story 2.3 (clock): `ModuleState::Clock`, subscription pattern, `view_*()` with Space fallback
- Story 2.5 (button): `ModuleState::Button`, `subscription()` returning unfold-based stream
- NFR4: IPC state changes reflected within one rendered frame — satisfied by `app.update()` driving ModuleState sync

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

- Created `src/modules/workspaces.rs` with `WorkspaceState`, `WorkspacesModule`, full `Module` trait impl (collapsed 32×20px pills, expanded full-width pills, `is_focused`→accent color)
- Extended `src/modules/mod.rs`: `pub mod workspaces`, `ModuleState::Workspace`, `workspaces_ctor`, registry entry
- Extended `WorkspacesChanged` handler in `src/app.rs` to propagate workspace data to `shared.modules`
- Added `use crate::modules::workspaces` import to `src/app.rs`
- All 156 tests pass (148 pre-existing + 8 new); `cargo clippy -- -D warnings` zero warnings

### File List

- src/modules/workspaces.rs
- src/modules/mod.rs
- src/app.rs

## Change Log

| Date | Summary |
|---|---|
| 2026-04-04 | Story context created |
| 2026-04-04 | Implemented all tasks: WorkspacesModule with pill rendering, WorkspaceState, app.rs propagation, 8 new tests; 156 total tests pass |
