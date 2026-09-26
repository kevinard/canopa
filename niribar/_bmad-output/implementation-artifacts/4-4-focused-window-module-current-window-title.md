# Story 4.4: Focused Window Module — Current Window Title

Status: review

## Story

As a **user**,
I want the bar to show the title of the window I'm currently focused on,
so that I can identify the active application without looking at the window itself.

## Acceptance Criteria

1. **Given** `focused-window` is in the module list
   **When** the user focuses a window
   **Then** the window title updates in the module within one rendered frame (NFR4)

2. **Given** a window is focused
   **When** the bar renders in collapsed state
   **Then** the module shows a window icon only (no title text)

3. **Given** a window is focused
   **When** the bar renders in expanded state
   **Then** the module shows the window title, clipped to fit the module width

4. **Given** no window is focused (focus on a layer-shell surface, desktop, etc.)
   **When** the bar renders in either state
   **Then** the focused-window module renders nothing — no gap, no placeholder (UX-DR16)

5. **Given** `focused-window` is NOT in the module list
   **When** window focus events fire
   **Then** `shared.modules` does NOT gain a FocusedWindow entry

6. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

7. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 168 pre-existing tests pass and at least 8 new tests pass

## Tasks / Subtasks

- [x] Task 1: Update `src/niri/mod.rs` — add `windows` cache to `NiriState` (AC: #1, #3, #4)
  - [x] 1.1: Extend the import to include `Window`:
    ```rust
    use niri_ipc::{Event, Window, Workspace};
    ```

  - [x] 1.2: Add `windows` field to `NiriState` after `focused_window_id`:
    ```rust
    /// Cache of all open windows, keyed by window ID.
    /// Populated by WindowsChanged and WindowOpenedOrChanged events.
    /// Used to look up the title of the currently focused window.
    pub windows: std::collections::HashMap<u64, Window>,
    ```

- [x] Task 2: Create `src/modules/focused_window.rs` (AC: #1–#4)
  - [x] 2.1: Define `FocusedWindowState`:
    ```rust
    /// Runtime state for the focused-window module.
    ///
    /// `title = None` means no window is focused → module renders nothing (UX-DR16).
    /// Populated by WindowFocusChanged / WindowsChanged / WindowOpenedOrChanged / WindowClosed
    /// events via app.rs update(). Story 4.5 may set Unavailable instead.
    #[derive(Debug, Clone, Default)]
    pub struct FocusedWindowState {
        pub title: Option<String>,
    }
    ```

  - [x] 2.2: Define `FocusedWindowModule`:
    ```rust
    /// Focused-window module: shows the focused window title, fed by niri IPC via SharedState.
    pub struct FocusedWindowModule {
        id: crate::modules::ModuleId,
    }

    impl FocusedWindowModule {
        pub fn new() -> Self {
            FocusedWindowModule {
                id: crate::modules::ModuleId::new("focused-window"),
            }
        }
    }
    ```

  - [x] 2.3: Implement `Module` trait for `FocusedWindowModule`:
    ```rust
    impl crate::modules::Module for FocusedWindowModule {
        fn id(&self) -> crate::modules::ModuleId {
            self.id.clone()
        }

        /// No independent data source — fed via Message::Niri in app.rs (same pattern as workspaces).
        fn subscription(&self) -> iced::Subscription<crate::app::Message> {
            iced::Subscription::none()
        }

        fn view_collapsed(
            &self,
            state: &crate::modules::ModuleState,
        ) -> iced::Element<'_, crate::app::Message> {
            use crate::modules::ModuleState;
            use iced::widget::{container, text};
            use iced::{Alignment, Color, Length};

            let title = match state {
                ModuleState::FocusedWindow(s) => &s.title,
                _ => return iced::widget::Space::new().into(), // Loading → empty
            };

            // No focused window → render nothing (AC #4, UX-DR16).
            if title.is_none() {
                return iced::widget::Space::new().into();
            }

            // Window is focused → show icon placeholder (AC #2).
            // "□" (U+25A1) is a window-shape placeholder; replaced when icon system is added.
            container(
                text("□")
                    .size(16)
                    .color(Color::from_rgb8(0xcd, 0xd6, 0xf4)), // color.text
            )
            .width(Length::Fill)
            .height(20)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
        }

        fn view_expanded(
            &self,
            state: &crate::modules::ModuleState,
        ) -> iced::Element<'_, crate::app::Message> {
            use crate::modules::ModuleState;
            use iced::widget::{container, text};
            use iced::{Alignment, Color, Length};

            let title = match state {
                ModuleState::FocusedWindow(s) => &s.title,
                _ => return iced::widget::Space::new().into(),
            };

            // No focused window → render nothing (AC #4, UX-DR16).
            let Some(title_str) = title else {
                return iced::widget::Space::new().into();
            };

            // Show title, clipped to module width (AC #3).
            // Iced clips text naturally when container width limits it.
            container(
                text(title_str)
                    .size(12)
                    .color(Color::from_rgb8(0xcd, 0xd6, 0xf4)) // color.text
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(Length::Fill)
            .height(20)
            .align_x(Alignment::Start)
            .align_y(Alignment::Center)
            .padding([0, 6])
            .into()
        }
    }
    ```

- [x] Task 3: Update `src/modules/mod.rs` — register module and add state variant (AC: #1–#5)
  - [x] 3.1: Add `pub mod focused_window;` after `pub mod workspaces;`:
    ```rust
    pub mod focused_window;
    ```

  - [x] 3.2: Add `FocusedWindow(focused_window::FocusedWindowState)` variant to `ModuleState` after `Workspace`:
    ```rust
    FocusedWindow(focused_window::FocusedWindowState),
    ```

  - [x] 3.3: Register in `build_registry()` — add after the `"workspaces"` entry:
    ```rust
    map.insert("focused-window", focused_window_ctor as ModuleConstructor);
    ```
    And add the constructor function after `workspaces_ctor`:
    ```rust
    fn focused_window_ctor(_mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
        Box::new(focused_window::FocusedWindowModule::new())
    }
    ```

- [x] Task 4: Update `src/app.rs` — handle window events and push `FocusedWindowState` (AC: #1, #4, #5)
  - [x] 4.1: Add import for `focused_window` module (after existing `use crate::modules::workspaces;`):
    ```rust
    use crate::modules::focused_window;
    ```

  - [x] 4.2: Add private helper method to `App` to push current focused-window title to module state.
    Add inside the `impl Application for App` block (or a separate `impl App` block) — before or after `update()`:
    ```rust
    /// Derive the current focused window title from NiriState and push it to all
    /// focused-window module instances in shared.modules.
    fn push_focused_window_state(&mut self) {
        let title = self
            .shared
            .niri
            .focused_window_id
            .and_then(|id| self.shared.niri.windows.get(&id))
            .and_then(|w| w.title.clone());
        for (_, module) in &self.modules {
            if module.id() == ModuleId::new("focused-window") {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::FocusedWindow(focused_window::FocusedWindowState {
                        title: title.clone(),
                    }),
                );
            }
        }
    }
    ```

  - [x] 4.3: Extend the existing `WindowFocusChanged` handler (currently only sets `focused_window_id`) to also call the helper:
    ```rust
    Event::WindowFocusChanged { id } => {
        self.shared.niri.focused_window_id = id;
        self.push_focused_window_state(); // AC #1 — update module state within one frame
    }
    ```

  - [x] 4.4: Replace the `_ => {}` catch-all with explicit handlers for the three remaining window events, then a new catch-all. Current code:
    ```rust
    _ => {} // Other events ignored; Stories 4.3/4.4 extend this
    ```
    Replace with:
    ```rust
    Event::WindowsChanged { windows } => {
        // Full snapshot — rebuild window cache entirely.
        self.shared.niri.windows =
            windows.into_iter().map(|w| (w.id, w)).collect();
        self.push_focused_window_state();
    }
    Event::WindowOpenedOrChanged { window } => {
        // If this window is now focused, update focused_window_id.
        if window.is_focused {
            self.shared.niri.focused_window_id = Some(window.id);
        }
        self.shared.niri.windows.insert(window.id, window);
        self.push_focused_window_state();
    }
    Event::WindowClosed { id } => {
        self.shared.niri.windows.remove(&id);
        if self.shared.niri.focused_window_id == Some(id) {
            self.shared.niri.focused_window_id = None;
        }
        self.push_focused_window_state();
    }
    _ => {} // Other events (keyboard layouts, urgency, etc.) not used by any module yet
    ```

- [x] Task 5: Tests (AC: #6, #7)
  - [x] 5.1: In `src/niri/mod.rs` tests (add to existing `#[cfg(test)]` block):
    - `niri_state_windows_default_empty` — `NiriState::default().windows.is_empty()`

  - [x] 5.2: In `src/modules/focused_window.rs` (add `#[cfg(test)]` block):
    - `focused_window_module_id_is_focused_window` — `FocusedWindowModule::new().id().0 == "focused-window"`
    - `focused_window_state_default_has_no_title` — `FocusedWindowState::default().title.is_none()`
    - `focused_window_state_stores_title` — create `FocusedWindowState { title: Some("foo".into()) }`, verify `.title == Some("foo")`

  - [x] 5.3: In `src/app.rs` tests (add after existing Story 4.2 test block):
    Add a helper:
    ```rust
    fn make_focused_window_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "focused-window".into(),
            zone: Zone::Center,
            params: ModuleParams::None,
        }];
        App::new(config)
    }

    fn make_window(id: u64, title: Option<&str>, is_focused: bool) -> niri_ipc::Window {
        niri_ipc::Window {
            id,
            title: title.map(|t| t.to_string()),
            app_id: None,
            pid: None,
            workspace_id: None,
            is_focused,
            is_floating: false,
            is_urgent: false,
            layout: niri_ipc::WindowLayout {
                pos_in_scrolling_layout: None,
                tile_size: (0.0, 0.0),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
            },
            focus_timestamp: None,
        }
    }
    ```

    Tests:
    - `focused_window_state_updated_on_window_focus_changed` — configure app with focused-window module; pre-populate `shared.niri.windows` with a window (id=1, title="Firefox"); send `WindowFocusChanged { id: Some(1) }`; verify `shared.modules[ModuleId("focused-window")] == FocusedWindowState { title: Some("Firefox") }`
    - `focused_window_state_cleared_when_focus_is_none` — configure app with focused-window; pre-populate state with focused id=1; send `WindowFocusChanged { id: None }`; verify title is None
    - `focused_window_state_updated_on_windows_changed` — send `WindowsChanged` with one focused window; verify FocusedWindowState stores its title
    - `focused_window_state_updated_on_window_opened_or_changed` — send `WindowOpenedOrChanged` with `is_focused: true`; verify FocusedWindowState title is updated
    - `focused_window_state_cleared_on_window_closed` — configure with focused window in state; send `WindowClosed { id }`; verify FocusedWindowState title is None
    - `focused_window_state_not_updated_without_focused_window_module` — app without focused-window module; send focus events; verify `shared.modules` does NOT gain FocusedWindow entry

- [x] Task 6: Quality gates (AC: #6, #7)
  - [x] 6.1: `cargo build` — succeeds
  - [x] 6.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 6.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 6.4: `cargo test` — 11 new tests + all 168 pre-existing pass (179 total)

## Dev Notes

### What Exists After Story 4.3

- `src/niri/mod.rs`: `NiriState` has `workspaces: Vec<Workspace>`, `focused_window_id: Option<u64>`, `connected: bool` — **no `windows` cache yet**
- `src/app.rs`: `WindowFocusChanged { id }` sets `shared.niri.focused_window_id = id` only; does NOT push to any module state
- `_ => {}` catch-all at end of niri event match arm — Story 4.4 replaces this with window event handlers
- `KNOWN_MODULE_NAMES` in `src/config/mod.rs` already contains `"focused-window"` — no config changes needed
- 168 tests passing; `build_registry()` maps `"button"`, `"clock"`, `"script"`, `"workspaces"` — add `"focused-window"`
- `ModuleState` variants: Loading, Unavailable, Clock, Script, Button, Workspace — add FocusedWindow

### niri_ipc Window Events: Data Flow Design

Three events feed the window cache; one event uses it:

```
WindowsChanged { windows }         → rebuild windows HashMap entirely → derive title → push FocusedWindowState
WindowOpenedOrChanged { window }   → insert/update in HashMap        → derive title → push FocusedWindowState
WindowClosed { id }                → remove from HashMap             → derive title → push FocusedWindowState
WindowFocusChanged { id }          → set focused_window_id           → derive title → push FocusedWindowState
```

**Why a window cache is needed:** `WindowFocusChanged` gives only the ID, not the title. The title lives on `niri_ipc::Window` which arrives via `WindowsChanged` and `WindowOpenedOrChanged`. Storing `HashMap<u64, Window>` in `NiriState` allows title lookup by ID.

### niri_ipc::Window Key Fields

```rust
pub struct Window {
    pub id: u64,
    pub title: Option<String>,   // None if app hasn't set a title yet
    pub app_id: Option<String>,  // wayland app ID (e.g. "org.gnome.Nautilus")
    pub pid: Option<i32>,
    pub workspace_id: Option<u64>,
    pub is_focused: bool,        // true on exactly one window globally
    pub is_floating: bool,
    pub is_urgent: bool,
    pub layout: WindowLayout,
    pub focus_timestamp: Option<Timestamp>,
}
```

`niri_ipc::Window` derives `Clone`. `niri_ipc::WindowLayout` fields:
```rust
pub struct WindowLayout {
    pub pos_in_scrolling_layout: Option<(usize, usize)>,
    pub tile_size: (f64, f64),
    pub window_size: (i32, i32),
    pub tile_pos_in_workspace_view: Option<(f64, f64)>,
    pub window_offset_in_tile: (f64, f64),
}
```

`WindowLayout` does NOT implement `Default` — use literal init in `make_window` test helper with `tile_size: (0.0, 0.0)`, `window_size: (0, 0)`, all `Option` fields as `None`.

### Placement of `push_focused_window_state` Method

Add `push_focused_window_state` as a method on the `App` struct. In this codebase, `App` is defined as the struct implementing `iced_layershell::MultiApplication`. Add an `impl App` block (separate from the `Application` impl) or inline inside the existing impl — either works. The method borrows `self` mutably; call sites finish their own writes before calling it, so no borrow conflicts.

### Collapsed Icon: "□" Placeholder

`view_collapsed` shows `"□"` (U+25A1 WHITE SQUARE) when a window is focused. This is a text-character placeholder — the same approach as `ButtonModule` using `">"` as its icon. The real icon system (SVG/icon font per UX-DR5) is deferred beyond Story 4.4. The `□` character visually suggests a window/app.

Color: `color.text` (`#cdd6f4`) — same as inactive workspace pills, no accent because focus state is already conveyed by the presence of content.

### Expanded State: Title Clipping

Iced clips text that overflows a container's bounds. Use:
```rust
text(title_str)
    .wrapping(iced::widget::text::Wrapping::None) // no line breaks
```
inside a `container(...).width(Length::Fill)` — Iced's default rendering clips text to the container boundary. **No manual truncation needed.**

### `push_focused_window_state`: Why Not Inline?

Four call sites vs two for the workspace equivalent. A method is cleaner and avoids repeating the HashMap lookup + clone pattern 4 times. The method borrows `self.shared.niri` (read) and `self.shared.modules` (write) and `self.modules` (read) — no conflicts since they are distinct fields.

### WindowOpenedOrChanged and focused_window_id

When `WindowOpenedOrChanged { window }` fires with `window.is_focused == true`, niri guarantees "all other windows are no longer focused" (per niri-ipc docs). Update `focused_window_id` to `Some(window.id)` in this handler. Otherwise leave `focused_window_id` unchanged (niri will send a separate `WindowFocusChanged` if focus moved).

### WindowsChanged: Complete Snapshot

`WindowsChanged` replaces the entire window list — any window absent from the snapshot has been closed. Rebuild the HashMap entirely (use `into_iter().map(|w| (w.id, w)).collect()`). Then re-derive the focused title from the new cache.

### No-Focus Renders Nothing (UX-DR16)

When `FocusedWindowState.title == None`, both `view_collapsed` and `view_expanded` return `Space::new()`. This means the module takes zero space — no gap, no placeholder. This satisfies AC #4 and UX-DR16. The `Space::new()` pattern is identical to how `WorkspacesModule` handles `ModuleState::Loading` / `Unavailable`.

### niri_ipc Import Change in niri/mod.rs

Current: `use niri_ipc::{Event, Workspace};`
Change to: `use niri_ipc::{Event, Window, Workspace};`

The `windows` HashMap type in `NiriState` will be:
`pub windows: std::collections::HashMap<u64, Window>,`

Alternatively use the full path `std::collections::HashMap<u64, niri_ipc::Window>` without the import — either works. Prefer adding `Window` to the import for consistency with `Workspace`.

### app.rs: Import Addition

Add alongside existing niri module imports:
```rust
use crate::modules::focused_window;
```
Place after `use crate::modules::workspaces;` at the top of app.rs.

### File Locations

| What | Where |
|---|---|
| `NiriState.windows` field | `src/niri/mod.rs` |
| `FocusedWindowState`, `FocusedWindowModule` | `src/modules/focused_window.rs` (new file) |
| `pub mod focused_window;` | `src/modules/mod.rs` |
| `ModuleState::FocusedWindow(...)` | `src/modules/mod.rs` — ModuleState enum |
| `focused_window_ctor`, registry entry | `src/modules/mod.rs` — build_registry() |
| `push_focused_window_state()`, window event handlers | `src/app.rs` |
| `use crate::modules::focused_window;` | `src/app.rs` — imports |

### References

- Epic 4 §"Story 4.4": user story + 4 ACs
- Architecture §"Module System": Module trait, naming, ModuleState patterns
- Architecture §"Niri IPC": `focused_window.rs` listed as expected location
- Architecture §"SharedState": `shared.modules` is the data store; module state lives there (AR6)
- Story 4.2 dev notes: WorkspacesModule — identical data flow pattern (`subscription()` returns none; app.rs bridges IPC → ModuleState)
- Story 4.2 dev notes: `ModuleState::Workspace`, `WorkspaceState`, `WorkspacesModule::new()` — parallel structure for FocusedWindow
- UX-DR16: Unavailable modules render as nothing (zero size, no gap)
- UX-DR4: `Space::new()` pattern for empty/unavailable state — same approach
- NFR4: IPC state changes reflected within one rendered frame — satisfied by `app.update()` directly driving ModuleState sync
- niri-ipc 25.11.0: `Window` has `title: Option<String>`, derives `Clone`; `WindowLayout` has no `Default`

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

None — implementation proceeded without unexpected errors after resolving a lifetime issue in `view_expanded` (fixed by cloning `title` before the `let Some` destructure rather than borrowing from `state`).

### Completion Notes List

- `niri_ipc::Window` derives `Clone`; `WindowLayout` has no `Default` — the `make_window` test helper uses struct literal init with zero/None values for all `WindowLayout` fields.
- `view_expanded` required `title.clone()` to avoid a borrow-checker lifetime error: the returned `Element` is tied to `&self`'s lifetime, but `title` was borrowed from `state` (a shorter lifetime). Cloning avoids the mismatch.
- `push_focused_window_state` is a standalone `impl App` method (not inside the `impl Application` block). It reads `self.shared.niri` and `self.modules` (read) and writes `self.shared.modules` — no borrow conflicts since they are distinct fields.
- `WindowOpenedOrChanged` handler updates `focused_window_id` only when `window.is_focused == true`; otherwise leaves it unchanged (niri sends a separate `WindowFocusChanged` if focus moved).
- `WindowsChanged` rebuilds the entire `windows` HashMap from scratch using `.into_iter().map(|w| (w.id, w)).collect()`.

### File List

- `src/niri/mod.rs` — added `Window` to import; added `pub windows: HashMap<u64, Window>` field to `NiriState`; added `niri_state_windows_default_empty` test
- `src/modules/focused_window.rs` — new file: `FocusedWindowState`, `FocusedWindowModule`, `Module` impl, 3 unit tests
- `src/modules/mod.rs` — added `pub mod focused_window;`; added `ModuleState::FocusedWindow(...)` variant; registered `"focused-window"` in `build_registry()`; added `focused_window_ctor`; added `build_registry_contains_focused_window` test
- `src/app.rs` — added `use crate::modules::focused_window;`; added `push_focused_window_state()` helper; extended `WindowFocusChanged` handler; added `WindowsChanged`, `WindowOpenedOrChanged`, `WindowClosed` handlers; added 6 new tests + 2 helper functions

### Change Log

- Added `windows: HashMap<u64, niri_ipc::Window>` cache to `NiriState` for title lookup by window ID
- Created `src/modules/focused_window.rs` with `FocusedWindowState` and `FocusedWindowModule` implementing the `Module` trait; collapsed view shows "□" icon, expanded shows window title with no-wrap clipping
- Registered `"focused-window"` module type in `build_registry()` and `ModuleState` enum
- Added `push_focused_window_state()` to `App` to drive module state from window cache; wired to all four window IPC events (`WindowFocusChanged`, `WindowsChanged`, `WindowOpenedOrChanged`, `WindowClosed`)
- 179 tests total (168 pre-existing + 11 new)
