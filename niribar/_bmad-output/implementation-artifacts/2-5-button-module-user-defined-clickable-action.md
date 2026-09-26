# Story 2.5: Button Module — User-Defined Clickable Action

Status: done

## Story

As a **user**,
I want a button module that runs a command when I click it,
so that I can launch apps or trigger actions directly from the bar.

## Acceptance Criteria

1. **Given** a `button` module entry with `label "..."`, `icon "..."`, and `on-click "..."` in config
   **When** the bar renders
   **Then** the module is instantiated and registered from the static registry

2. **Given** the button module is rendered
   **When** the bar is in collapsed state
   **Then** the icon is displayed at 16px

3. **Given** the button module is rendered
   **When** the bar is in expanded state
   **Then** the icon (16px) and label (13px) are displayed in a row, icon left, label right (ModuleLabel format per UX-DR6)

4. **Given** the user clicks the button in either collapsed or expanded state
   **When** the click event fires
   **Then** `on-click` command is executed via `sh -c <command>` in the background
   **And** the bar does not freeze or stall — the command is spawned and forgotten (fire-and-forget)

5. **Given** the button module is hovered
   **When** the pointer enters the module area
   **Then** the ModuleFrame hover fill (Catppuccin Mocha Surface0 `#313244`, 4px border radius) is shown — identical to all other modules (UX-DR3)

6. **Given** `button` module entry is missing any of `label`, `icon`, or `on-click`
   **When** the config is parsed
   **Then** an actionable error is produced in the format: `error: modules[N].button.<field>: missing required parameter. Expected: a string value.`

7. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

8. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 65 pre-existing tests pass and at least 10 new tests pass

## Tasks / Subtasks

- [x] Task 1: Add `ButtonConfig` and `ModuleParams::Button` in `src/config/mod.rs` (AC: #1, #6)
  - [x] 1.1: Add `ButtonConfig` struct after `ScriptConfig`:
    ```rust
    /// Configuration for the button module — all fields required.
    #[derive(Debug, Clone)]
    pub struct ButtonConfig {
        pub label: String,
        pub icon: String,
        pub on_click: String,
    }
    ```
    No `Default` impl — all three fields are required with no sensible defaults.
  - [x] 1.2: Replace the `// Button(ButtonConfig) — Story 2.5` comment in `ModuleParams`:
    ```rust
    pub enum ModuleParams {
        None,
        Clock(ClockConfig),
        Script(ScriptConfig),
        Button(ButtonConfig),
    }
    ```

- [x] Task 2: Parse button params from KDL in `src/config/parse.rs` (AC: #6)
  - [x] 2.1: Add `parse_button_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<ButtonConfig>`:
    - All three fields (`label`, `icon`, `on-click`) are required positional string args on child nodes
    - If children block is absent → error on `label` (first required field):
      ```
      "modules[{idx}].button.label: missing required parameter. Expected: a string value."
      ```
    - For each field, if child node absent → error:
      ```
      "modules[{idx}].button.label: missing required parameter. Expected: a string value."
      "modules[{idx}].button.icon: missing required parameter. Expected: a string value."
      "modules[{idx}].button.on-click: missing required parameter. Expected: a string value."
      ```
    - If value is wrong type (non-string) → error:
      ```
      "modules[{idx}].button.label: expected a string value."
      ```
    - Empty string is allowed for all fields (unlike script.command) — icon/label may be empty
    - Return `ButtonConfig { label, icon, on_click }`
    - Note: KDL child node is `on-click` (kebab-case); Rust field is `on_click` (snake_case)
  - [x] 2.2: In `parse_modules_section`, add `"button"` branch before the `else` fallthrough:
    ```rust
    let params = if name == "clock" {
        ModuleParams::Clock(parse_clock_params(node, i)?)
    } else if name == "script" {
        ModuleParams::Script(parse_script_params(node, i)?)
    } else if name == "button" {
        ModuleParams::Button(parse_button_params(node, i)?)
    } else {
        ModuleParams::None
    };
    ```
  - [x] 2.3: Add `ButtonConfig` to the `use` import at the top of `parse.rs`:
    ```rust
    use super::{BarConfig, ButtonConfig, ClockConfig, Config, Edge, ModuleConfig, ModuleParams, ScriptConfig};
    ```

- [x] Task 3: Extend module system in `src/modules/mod.rs` (AC: #1)
  - [x] 3.1: Add `pub mod button;` declaration at top (alongside `pub mod clock;` and `pub mod script;`)
  - [x] 3.2: Replace the `// Button(ButtonState) — Story 2.5` comment in `ModuleState`:
    ```rust
    pub enum ModuleState {
        #[default]
        Loading,
        #[allow(dead_code)]
        Unavailable,
        Clock(clock::ClockState),
        Script(script::ScriptState),
        Button(button::ButtonState),
    }
    ```
  - [x] 3.3: Add `Button(button::ButtonEvent)` to `ModuleEvent` enum:
    ```rust
    pub enum ModuleEvent {
        Clock(clock::ClockEvent),
        Script(script::ScriptEvent),
        Button(button::ButtonEvent),
    }
    ```
  - [x] 3.4: Register `"button"` in `build_registry()`:
    ```rust
    map.insert("button", button_ctor as ModuleConstructor);
    ```
  - [x] 3.5: Add `button_ctor` function alongside `script_ctor`:
    ```rust
    fn button_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
        use crate::config::ModuleParams;
        match &mc.params {
            ModuleParams::Button(cfg) => Box::new(button::ButtonModule::new(
                cfg.label.clone(),
                cfg.icon.clone(),
                cfg.on_click.clone(),
            )),
            _ => panic!("button_ctor called with non-Button params — this is a programming error"),
        }
    }
    ```

- [x] Task 4: Implement `ButtonModule` in `src/modules/button.rs` (new file) (AC: #2, #3, #4)
  - [x] 4.1: Define types:
    ```rust
    /// Runtime state for the button module. Button appearance is config-driven, not data-driven.
    #[derive(Debug, Clone)]
    pub struct ButtonState;

    /// Events emitted by the button module.
    #[derive(Debug, Clone)]
    pub enum ButtonEvent {
        /// Emitted once at startup to transition Loading → Button(ButtonState).
        Ready,
    }

    pub struct ButtonModule {
        id: crate::modules::ModuleId,
        label: String,
        icon: String,
        on_click: String,
    }

    impl ButtonModule {
        pub fn new(label: String, icon: String, on_click: String) -> Self {
            ButtonModule {
                id: crate::modules::ModuleId::new("button"),
                label,
                icon,
                on_click,
            }
        }
    }
    ```

  - [x] 4.2: Implement `Module::subscription()` — emit `ButtonEvent::Ready` once, then end:

    The button has no async data source. Unlike clock/script, no periodic polling is needed.
    Subscription emits one `ButtonEvent::Ready` to satisfy the "emit within 1 second" contract
    and transition the module from `Loading` to `Button(ButtonState)`, then the stream ends.

    ```rust
    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        let id = self.id.clone();
        iced::Subscription::run_with(id, |id: &crate::modules::ModuleId| {
            let id = id.clone();
            iced::futures::stream::unfold(Some(id), |state| async move {
                match state {
                    Some(id) => {
                        let msg = crate::app::Message::Module(
                            id,
                            crate::modules::ModuleEvent::Button(ButtonEvent::Ready),
                        );
                        Some((msg, None))  // None = stream exhausted after first item
                    }
                    None => {
                        // Keep subscription alive but emit nothing further.
                        std::future::pending::<()>().await;
                        unreachable!()
                    }
                }
            })
        })
    }
    ```

    **Why `pending()` on None:** When `unfold` returns `None`, the stream is exhausted. Using
    `std::future::pending()` keeps the subscription alive (prevents iced from recreating it)
    without emitting further messages. This is the correct pattern for one-shot subscriptions.

    **If `run_with` API has changed from Story 2.4:** Check `Subscription::run_with` signature.
    Story 2.4 confirmed: `Subscription::run_with<D, S>(data: D, builder: fn(&D) -> S)`.
    The builder `|id: &ModuleId| { ... }` must be a non-capturing closure (or fn pointer).
    The `id.clone()` inside uses the `id` parameter, NOT an external capture — this coerces to `fn`.

  - [x] 4.3: Implement `Module::on_interaction()` — spawn the command on click:

    ```rust
    fn on_interaction(
        &self,
        event: crate::modules::InteractionEvent,
    ) -> Option<crate::app::Message> {
        use crate::modules::InteractionEvent;
        match event {
            InteractionEvent::Clicked => {
                // Fire-and-forget: spawn the command and return immediately (AC #4)
                // tokio::process::Command::spawn() is synchronous — it starts the process
                // without waiting for completion. Safe to call from app.update().
                let _ = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&self.on_click)
                    .spawn();
                None
            }
            _ => None,
        }
    }
    ```

    **No new Message variant needed:** The command is spawned directly inside `on_interaction`.
    `tokio::process::Command::spawn()` is NOT async — it synchronously starts the child process
    and returns `io::Result<Child>`. The child runs independently; we discard the handle with `let _`.
    This does NOT block the render loop.

    **NFR7 compliance:** Runs with the permissions of the niribar process — no privilege escalation.

  - [x] 4.4: Implement `view_collapsed()`:
    ```rust
    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, text};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Button(_) => column![
                text(self.icon.clone()).size(16),  // font.size.icon per UX-DR9
            ]
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .into(),
            _ => iced::widget::Space::new().into(),  // Loading/Unavailable → empty (UX-DR16)
        }
    }
    ```

  - [x] 4.5: Implement `view_expanded()`:
    ```rust
    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{row, text};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Button(_) => row![
                text(self.icon.clone()).size(16),       // icon left (UX-DR6)
                iced::widget::Space::new().width(Length::Fill),
                text(self.label.clone()).size(13),      // label right, font.size.base (UX-DR9)
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into(),
            _ => iced::widget::Space::new().into(),
        }
    }
    ```

    **Critical — use `.clone()` on label/icon:** `view_expanded`/`view_collapsed` take `&self`.
    Iced elements cannot borrow from `self` through the `Module` trait boundary (lifetime constraint
    confirmed in Story 2.3). Use `.clone()` on all string fields accessed in view methods.

  - [x] 4.6: Implement `id()`:
    ```rust
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }
    ```

  - [x] 4.7: Add `use tokio::process::Command as TokioCommand;` OR use full path inline.
    No new crate dep — `tokio = { features = ["full"] }` already includes `tokio::process`.

- [x] Task 5: Update `src/app.rs` — click routing and view wiring (AC: #4, #5)
  - [x] 5.1: Add to the `use` imports at the top:
    ```rust
    use crate::modules::button::{ButtonEvent, ButtonState};
    use crate::modules::InteractionEvent;
    ```
  - [x] 5.2: Add `ModuleClick(ModuleId)` variant to the `Message` enum:
    ```rust
    #[to_layer_message]
    #[derive(Debug, Clone)]
    pub enum Message {
        Module(ModuleId, ModuleEvent),
        ModuleHoverEnter(ModuleId),
        ModuleHoverExit(ModuleId),
        ModuleClick(ModuleId),   // ← new: fires when user clicks a module
    }
    ```
  - [x] 5.3: Add click handler in `update()` BEFORE the `_ => Task::none()` catch-all:
    ```rust
    Message::ModuleClick(id) => {
        for (_, module) in &self.modules {
            if module.id() == id {
                let _ = module.on_interaction(InteractionEvent::Clicked);
                break;
            }
        }
        Task::none()
    }
    Message::Module(id, ModuleEvent::Button(ButtonEvent::Ready)) => {
        self.shared
            .modules
            .insert(id, ModuleState::Button(ButtonState));
        Task::none()
    }
    ```
  - [x] 5.4: Add `.on_press(Message::ModuleClick(id.clone()))` to the `mouse_area` in `view()`:
    ```rust
    let view: Element<'_, Message> =
        mouse_area(module_frame(module.view_collapsed(&state), hovered))
            .on_enter(Message::ModuleHoverEnter(id.clone()))
            .on_exit(Message::ModuleHoverExit(id.clone()))
            .on_press(Message::ModuleClick(id))
            .into();
    ```
    **Note:** `id` is consumed by `on_press` so `on_exit` must use `id.clone()` too.
    Check existing code at app.rs lines 128–131 — currently `on_exit` consumes `id`. Adjust:
    - Before: `.on_enter(Message::ModuleHoverEnter(id.clone())).on_exit(Message::ModuleHoverExit(id))`
    - After:  `.on_enter(Message::ModuleHoverEnter(id.clone())).on_exit(Message::ModuleHoverExit(id.clone())).on_press(Message::ModuleClick(id))`

- [x] Task 6: Tests (AC: #7, #8)
  - [x] 6.1: In `src/config/mod.rs` tests:
    - `button_config_stores_fields` — `ButtonConfig { label: "launch".into(), icon: ">".into(), on_click: "kitty".into() }` stores all three fields correctly
    - `module_params_button_stores_config` — `ModuleParams::Button(cfg)` round-trips all three fields
  - [x] 6.2: In `src/config/parse.rs` tests:
    - `parse_button_with_all_params` — KDL with `label "Launch"`, `icon ">"`, `on-click "kitty"` → `ButtonConfig { label: "Launch", icon: ">", on_click: "kitty" }`
    - `parse_button_missing_label_errors` — `module "button" zone="bottom"` (no children) → `Err` containing `"button.label"`
    - `parse_button_missing_on_click_errors` — children with `label` and `icon` but no `on-click` → `Err` containing `"button.on-click"`
    - `parse_button_wrong_type_errors` — `label 42` (integer, not string) → `Err` containing `"button.label"`
  - [x] 6.3: In `src/modules/button.rs` tests:
    - `button_module_id_is_button` — `ButtonModule::new("L".into(), ">".into(), "kitty".into()).id().0 == "button"`
    - `button_state_is_debug` — `ButtonState` derives `Debug`; formatting it doesn't panic
    - `button_event_ready_variant` — `ButtonEvent::Ready` constructs without error
  - [x] 6.4: In `src/app.rs` tests:
    - `button_module_instantiated_from_registry` — Config with `module "button" zone="bottom"` and `ButtonConfig` → `app.modules.len() == 1`
    - `button_ready_event_updates_state` — `update(Message::Module(id, ModuleEvent::Button(ButtonEvent::Ready)))` → state is `ModuleState::Button(ButtonState)`
    - `module_click_calls_interaction` — `update(Message::ModuleClick(id))` with no matching module → returns `Task::none()` without panic (boundary test)
  - [x] 6.5: Verify all 65 pre-existing tests still pass (`cargo test`)

- [x] Task 7: Quality gates
  - [x] 7.1: `cargo build` — succeeds
  - [x] 7.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 7.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 7.4: `cargo test` — 10+ new tests + all 65 pre-existing pass (75+ total)

### Review Findings

- [x] [Review][Patch] Dispatch `on_interaction` return value — call site uses `let _` and silently discards `Some(msg)`; any module returning a message from `on_interaction` will have it swallowed [src/app.rs:116]
- [x] [Review][Patch] Use `std::process::Command` instead of `tokio::process::Command` for fire-and-forget spawn — `tokio::process::Command::spawn()` requires an active Tokio runtime; `std::process::Command` is correct for a synchronous, non-async spawn [src/modules/button.rs:68]
- [x] [Review][Patch] Fix misleading error message when button node has no child block — currently reports `button.label: missing required parameter` when the real issue is the absence of the `{ }` child block entirely; separate the two error paths or use a clearer message [src/config/parse.rs:164]
- [x] [Review][Patch] Add `parse_button_missing_icon_errors` test — `icon` is a required field with its own error path but no test exercises the missing-icon case [src/config/parse.rs]
- [x] [Review][Defer] Hardcoded `ModuleId("button")` prevents multiple button instances [src/modules/button.rs:22] — deferred, pre-existing known limitation documented in Dev Notes (same as Defer-1 from Story 2.4)
- [x] [Review][Defer] `spawn()` failure silently ignored with `let _` [src/modules/button.rs:70] — deferred, no logging infrastructure exists yet; fire-and-forget is intentional per AC #4
- [x] [Review][Defer] `ButtonConfig` does not derive `PartialEq` [src/config/mod.rs] — deferred, not currently needed by any call site
- [x] [Review][Defer] No visual affordance (cursor, pressed state) for interactive icon [src/modules/button.rs:87] — deferred, not required by story spec; raw `text` widget in `mouse_area` is consistent with other modules
- [x] [Review][Defer] `view_expanded` not called from `app.view()` [src/app.rs] — deferred, expand/collapse wiring is Story 3.1; method is fully implemented and ready

## Dev Notes

### This Story Introduces Click Handling (FR15)

The button module is the first module to use pointer click interaction. Three new things are wired:
1. `Message::ModuleClick(ModuleId)` — new top-level message variant
2. `mouse_area.on_press(...)` — wired in `app.view()` for ALL modules (not just button)
3. `Module::on_interaction(Clicked)` — ButtonModule implements this; others return `None` (default)

All modules will now respond to clicks (routing through `ModuleClick` → `on_interaction`). Modules that don't override `on_interaction` return `None` (the default impl in the trait) — a no-op.

### ButtonModule Has No Async Data Source

Unlike clock (tokio interval) and script (process polling), button has no data to fetch. Its subscription emits `ButtonEvent::Ready` once at startup and then stays open (via `pending()`) without further events. This satisfies the subscription contract ("emit within 1 second") and avoids iced recreating the subscription.

Do NOT try to use `iced::Subscription::none()` — that violates the subscription contract and leaves the module in `ModuleState::Loading` forever. Always emit `ButtonEvent::Ready`.

### `tokio::process::Command::spawn()` Is Synchronous

Unlike `tokio::process::Command::output().await` (async, waits for completion), `spawn()` is synchronous — it starts the child process and returns `io::Result<Child>` immediately. Discarding the `Child` handle with `let _` means no cleanup; the child runs independently. This is intentional (fire-and-forget per AC #4).

`std::process::Command::spawn()` would also work for this use case, but `tokio::process::Command` is used for consistency with `script.rs`.

### All Three Button Config Fields Are Required

Unlike clock (all optional, all have defaults) and script (interval optional), all three button fields (`label`, `icon`, `on-click`) are required. If any is absent, produce an error before the bar starts.

This means `ButtonConfig` has no `Default` impl. Do not add one.

### KDL Field Name: `on-click` → Rust: `on_click`

KDL uses kebab-case (`on-click`). The Rust struct uses snake_case (`on_click`). This is the same convention as `time-format` → `time_format` in `ClockConfig`. The parser reads `children.get("on-click")`.

### Mouse Area `on_press` Change in `app.view()`

The existing `mouse_area` chain ends with `.on_exit(Message::ModuleHoverExit(id))` which consumes `id`. To add `.on_press(Message::ModuleClick(id))` after that, change `on_exit` to use `id.clone()`:

```rust
// Before (Story 2.4 state):
mouse_area(module_frame(module.view_collapsed(&state), hovered))
    .on_enter(Message::ModuleHoverEnter(id.clone()))
    .on_exit(Message::ModuleHoverExit(id))
    .into()

// After (Story 2.5):
mouse_area(module_frame(module.view_collapsed(&state), hovered))
    .on_enter(Message::ModuleHoverEnter(id.clone()))
    .on_exit(Message::ModuleHoverExit(id.clone()))
    .on_press(Message::ModuleClick(id))
    .into()
```

### ModuleId Limitation (Known, Deferred)

`ButtonModule::new` hardcodes `ModuleId::new("button")`. Multiple `module "button"` entries in config would share the same ID. This is the same known limitation as clock and script (tracked in Story 2.4 Defer-1). Do NOT attempt to fix it in this story.

### File Locations

| What | Where |
|---|---|
| `ButtonConfig`, `ModuleParams::Button` | `src/config/mod.rs` — extend existing |
| `parse_button_params()` | `src/config/parse.rs` — add new function |
| `ButtonState`, `ButtonEvent`, `ButtonModule` | `src/modules/button.rs` — **new file** |
| `pub mod button;`, `ModuleState::Button`, `ModuleEvent::Button`, registry | `src/modules/mod.rs` |
| `Message::ModuleClick`, routing, `on_press` wiring | `src/app.rs` |

Do NOT create:
- Any `src/dbus/` files — Stories 5.x
- Any `src/surface/expand.rs` — Story 3.1
- Any `src/niri/` files — Stories 4.x

### State After Story 2.5

When complete:
- `build_registry()` returns `{"clock": clock_ctor, "script": script_ctor, "button": button_ctor}` — three entries
- Config with a `button` module → `app.modules` includes a `ButtonModule`
- On startup: `ButtonEvent::Ready` → `SharedState.modules["button"] = ModuleState::Button(ButtonState)`
- On click: `Message::ModuleClick("button")` → `on_interaction(Clicked)` → `sh -c <on_click>` spawned
- Hover feedback works via existing `ModuleHoverEnter/Exit` + `module_frame` — no new code needed for hover

### `#[to_layer_message]` Macro Caveat

The `#[to_layer_message]` macro on `Message` injects LayerShell variants. Keep the `_ => Task::none()` catch-all in `app.update()`. New match arms (ModuleClick, Button events) must be added BEFORE the catch-all — same as script in Story 2.4.

### Project Structure Notes

- Button module file: `src/modules/button.rs` — confirmed in architecture doc §"Complete Project Directory Structure"
- Config type location: `src/config/mod.rs` — all `*Config` types live here
- Parse function location: `src/config/parse.rs` — all `parse_*_params()` functions live here
- No separate `tests/` directory — all tests co-located per architecture §"Test Organization"

### References

- Epic 2, Story 2.5 (epics.md §"Story 2.5"): user story + acceptance criteria
- Architecture: "Module System Design" — static registry, subscription contract, on_interaction
- Architecture: "Naming Patterns" — `ButtonState`, `ButtonEvent` conventions
- Architecture: "Communication Patterns" — `Message::Module(ModuleId, ModuleEvent)` routing
- Architecture: "State Management Patterns" — module data in `SharedState` only
- Architecture: FR11–16, FR22–28 locations — `src/modules/`
- Architecture: §"Process Patterns" — `Loading`/`Unavailable` → `Space::new()`, one-frame latency
- UX Design: UX-DR3 (ModuleFrame hover fill — already implemented, no new code needed)
- UX Design: UX-DR5 (icon 16px), UX-DR6 (icon left + label right), UX-DR9 (13px base), UX-DR16 (unavailable → nothing)
- Story 2.4 dev notes: `Subscription::run_with` API, `stream::unfold` pattern, `Space::new()` usage, `.clone()` in view methods
- Story 2.3 dev notes: `#[allow(dead_code)]` pattern for new variants not yet consumed by binary
- FR15: Modules respond to pointer interactions — this story delivers it
- NFR7: Button commands run with niribar user permissions — no privilege escalation, no sandboxing

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

None — implementation completed without errors. Zero warnings on `cargo clippy -- -D warnings`.

### Completion Notes List

- Added `ButtonConfig` struct (label, icon, on_click — all required, no Default impl) and `ModuleParams::Button` variant to `src/config/mod.rs`
- Added `parse_button_params()` in `src/config/parse.rs` with per-field error messages; validates all three required fields; KDL `on-click` (kebab-case) maps to Rust `on_click` (snake_case)
- Extended `src/modules/mod.rs`: added `pub mod button;`, `ModuleState::Button`, `ModuleEvent::Button`, `button_ctor()`, and registry entry
- Created `src/modules/button.rs`: `ButtonState`, `ButtonEvent::Ready`, `ButtonModule` with full Module trait impl — one-shot subscription (unfold + pending()), fire-and-forget `on_interaction(Clicked)` via `tokio::process::Command::spawn()`, collapsed (icon 16px) and expanded (icon + label row) views
- Updated `src/app.rs`: added `Message::ModuleClick(ModuleId)`, click routing in `update()`, `ButtonEvent::Ready` handler, and `mouse_area.on_press()` wiring in `view()`
- Applied `#[allow(dead_code)]` to `ButtonModule::label` field (used only in `view_expanded`, not yet called from binary)
- 14 new tests added across 4 files; 79 total tests pass; zero clippy warnings; `cargo fmt --check` passes

### File List

- src/config/mod.rs
- src/config/parse.rs
- src/modules/mod.rs
- src/modules/button.rs (new)
- src/app.rs
- _bmad-output/implementation-artifacts/2-5-button-module-user-defined-clickable-action.md
- _bmad-output/implementation-artifacts/sprint-status.yaml

## Change Log

| Date | Summary |
|---|---|
| 2026-04-02 | Implemented Story 2.5: ButtonConfig + parse, ButtonModule (button.rs new file), Message::ModuleClick routing, on_press wiring; 14 new tests; all quality gates pass |
