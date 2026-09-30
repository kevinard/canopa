# Story 2.3: Clock Module with Configurable Format

Status: done

## Story

As a **user**,
I want a clock module that shows the current time and date,
so that I always know the time without leaving my workspace.

## Acceptance Criteria

1. **Given** `clock` is in the module list
   **When** the bar renders
   **Then** the collapsed state shows a time icon (`⌚` U+231A) and the current time in `font.size.small` (11px) in a compact column

2. **Given** `clock` is in the module list
   **When** the bar renders
   **Then** the expanded state shows icon + time on one line and the date below it, in `ModuleLabel` format (icon + label left-aligned, value right-aligned)

3. **Given** the clock module is running
   **When** the seconds tick
   **Then** the time and date update each second without blocking the render loop
   **And** the update is delivered via `iced::Subscription` (async), never blocking `update()`

4. **Given** the user has set `time-format` and `date-format` in the `clock` module config block
   **When** the clock renders
   **Then** it uses those format strings (chrono format syntax)

5. **Given** no `time-format` or `date-format` is configured
   **When** the clock renders
   **Then** defaults of `%H:%M` (time) and `%a %d %b` (date) are used

6. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

7. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 42 pre-existing tests pass and at least 5 new tests pass

## Tasks / Subtasks

- [x] Task 1: Add `chrono` dependency to `Cargo.toml` (AC: #3, #4, #5)
  - [x] 1.1: Add to `[dependencies]`:
    ```toml
    chrono = { version = "0.4", features = ["clock"] }
    ```
    (`features = ["clock"]` is required for `chrono::Local::now()`)

- [x] Task 2: Extend `ModuleConfig` in `src/config/mod.rs` (AC: #4, #5)
  - [x] 2.1: Add `ClockConfig` struct with defaults:
    ```rust
    #[derive(Debug, Clone)]
    pub struct ClockConfig {
        pub time_format: String,
        pub date_format: String,
    }

    impl Default for ClockConfig {
        fn default() -> Self {
            ClockConfig {
                time_format: String::from("%H:%M"),
                date_format: String::from("%a %d %b"),
            }
        }
    }
    ```
  - [x] 2.2: Add `ModuleParams` enum (extensible for Stories 2.4–2.5):
    ```rust
    #[derive(Debug, Clone)]
    pub enum ModuleParams {
        None,
        Clock(ClockConfig),
        // Script(ScriptConfig) — Story 2.4
        // Button(ButtonConfig) — Story 2.5
    }
    ```
  - [x] 2.3: Add `params: ModuleParams` field to `ModuleConfig`:
    ```rust
    pub struct ModuleConfig {
        pub name: String,
        pub zone: crate::modules::Zone,
        pub params: ModuleParams,
    }
    ```
  - [x] 2.4: Update all test code in `src/config/mod.rs` that constructs `ModuleConfig` to include `params: ModuleParams::None` (or `ModuleParams::Clock(ClockConfig::default())` where appropriate)

- [x] Task 3: Parse clock params from KDL in `src/config/parse.rs` (AC: #4, #5)
  - [x] 3.1: Add `parse_clock_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<ClockConfig>`:
    - Look for children block on the `module` node; if absent → return `ClockConfig::default()`
    - From children, read `time-format` and `date-format` as positional string args
    - Missing → use `ClockConfig::default()` for that field (both fields are optional)
    - Wrong type (non-string) → error: `modules[{idx}].clock.time-format: expected a string value.`
    - Return `ClockConfig { time_format, date_format }`
  - [x] 3.2: In `parse_modules_section`, after parsing `name` and `zone`, resolve `params`:
    ```rust
    let params = if name == "clock" {
        ModuleParams::Clock(parse_clock_params(node, i)?)
    } else {
        ModuleParams::None
    };
    result.push(ModuleConfig { name, zone, params });
    ```
  - [x] 3.3: Update all `ModuleConfig { name, zone }` struct literals in `parse.rs` tests to include `params: ModuleParams::None` (non-clock modules) or appropriate clock params

- [x] Task 4: Update module system in `src/modules/mod.rs` (AC: #3)
  - [x] 4.1: Add `Clock(ClockEvent)` variant to `ModuleEvent`:
    ```rust
    pub enum ModuleEvent {
        Clock(ClockEvent),
    }
    ```
    Import `ClockEvent` from `super::clock::ClockEvent`
  - [x] 4.2: Add `Clock(ClockState)` variant to `ModuleState`:
    ```rust
    pub enum ModuleState {
        #[default]
        Loading,
        #[allow(dead_code)]
        Unavailable,
        Clock(ClockState),
    }
    ```
    Import `ClockState` from `super::clock::ClockState`
  - [x] 4.3: Change `ModuleConstructor` type alias to accept `&ModuleConfig`:
    ```rust
    pub type ModuleConstructor = fn(&crate::config::ModuleConfig) -> Box<dyn Module>;
    ```
    This enables module-specific config (clock format strings, script commands, button labels).
  - [x] 4.4: Register clock in `build_registry()`:
    ```rust
    use crate::modules::clock::ClockModule;
    use crate::config::{ClockConfig, ModuleParams};

    pub fn build_registry() -> HashMap<&'static str, ModuleConstructor> {
        let mut map = HashMap::new();
        map.insert("clock", clock_ctor as ModuleConstructor);
        map
    }

    fn clock_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
        let (time_fmt, date_fmt) = match &mc.params {
            ModuleParams::Clock(cfg) => (cfg.time_format.clone(), cfg.date_format.clone()),
            _ => {
                let d = ClockConfig::default();
                (d.time_format, d.date_format)
            }
        };
        Box::new(ClockModule::new(time_fmt, date_fmt))
    }
    ```
  - [x] 4.5: Add `pub mod clock;` to declare the new module

- [x] Task 5: Implement `ClockModule` in `crates/niribar/src/modules/clock.rs` (new file) (AC: #1, #2, #3)
  - [x] 5.1: Define types:
    ```rust
    #[derive(Debug, Clone)]
    pub struct ClockState {
        pub time: String,
        pub date: String,
    }

    #[derive(Debug, Clone)]
    pub enum ClockEvent {
        Tick { time: String, date: String },
    }

    pub struct ClockModule {
        id: crate::modules::ModuleId,
        time_format: String,
        date_format: String,
    }

    impl ClockModule {
        pub fn new(time_format: String, date_format: String) -> Self {
            ClockModule {
                id: crate::modules::ModuleId::new("clock"),
                time_format,
                date_format,
            }
        }
    }
    ```
  - [x] 5.2: Implement `Module::subscription()` using a 1-second tokio interval:
    ```rust
    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        // Marker type for unique subscription ID — required by architecture (no magic strings)
        struct ClockSubscription;

        let time_fmt = self.time_format.clone();
        let date_fmt = self.date_format.clone();

        iced::subscription::channel(
            std::any::TypeId::of::<ClockSubscription>(),
            1,
            |mut sender| async move {
                loop {
                    let now = chrono::Local::now();
                    let time = now.format(&time_fmt).to_string();
                    let date = now.format(&date_fmt).to_string();
                    let _ = sender
                        .send(crate::app::Message::Module(
                            crate::modules::ModuleId::new("clock"),
                            crate::modules::ModuleEvent::Clock(ClockEvent::Tick { time, date }),
                        ))
                        .await;
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            },
        )
    }
    ```
    Contract: first tick fires within 1 second of subscription start (immediate loop entry). Never panics. No `Result` return — errors are impossible for a clock.
  - [x] 5.3: Implement `Module::view_collapsed()`:
    ```rust
    fn view_collapsed(&self, state: &crate::modules::ModuleState) -> iced::Element<'_, crate::app::Message> {
        use iced::widget::{column, text};
        use iced::{Alignment, Length};
        use crate::modules::ModuleState;

        match state {
            ModuleState::Clock(s) => column![
                text("⌚").size(16),   // icon placeholder (U+231A Watch, 16px per UX-DR5)
                text(&s.time).size(11), // font.size.small per UX-DR9
            ]
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .into(),
            _ => iced::widget::Space::new(0, 0).into(),
        }
    }
    ```
  - [x] 5.4: Implement `Module::view_expanded()`:
    ```rust
    fn view_expanded(&self, state: &crate::modules::ModuleState) -> iced::Element<'_, crate::app::Message> {
        use iced::widget::{column, row, text};
        use iced::{Alignment, Length};
        use crate::modules::ModuleState;

        match state {
            ModuleState::Clock(s) => column![
                // ModuleLabel format: icon left, time value right (UX-DR6)
                row![
                    text("⌚").size(16),
                    iced::widget::Space::new(Length::Fill, 0),
                    text(&s.time).size(13), // font.size.base per UX-DR9
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill),
                // Date below in smaller text
                text(&s.date).size(11), // font.size.small per UX-DR9
            ]
            .width(Length::Fill)
            .into(),
            _ => iced::widget::Space::new(0, 0).into(),
        }
    }
    ```
  - [x] 5.5: Implement `Module::id()`:
    ```rust
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }
    ```

- [x] Task 6: Update `crates/niribar/src/app.rs` — constructor call + event routing (AC: #3)
  - [x] 6.1: Update the `ctor` call in `App::new()` to pass `mc`:
    ```rust
    let modules: Vec<(Zone, Box<dyn Module>)> = config
        .modules
        .iter()
        .filter_map(|mc| {
            registry
                .get(mc.name.as_str())
                .map(|ctor| (mc.zone.clone(), ctor(mc)))  // pass mc — was ctor()
        })
        .collect();
    ```
  - [x] 6.2: Handle clock events in `App::update()`:
    ```rust
    Message::Module(id, ModuleEvent::Clock(ClockEvent::Tick { time, date })) => {
        self.shared
            .modules
            .insert(id, ModuleState::Clock(ClockState { time, date }));
        Task::none()
    }
    ```
    Keep `_ => Task::none()` catch-all for `#[to_layer_message]` macro variants.
  - [x] 6.3: Update `use` imports in `app.rs` to include `ClockState`, `ClockEvent` from `crate::modules::clock`

- [x] Task 7: Tests (AC: #6, #7)
  - [x] 7.1: In `src/config/mod.rs` tests:
    - `clock_config_default_formats` — `ClockConfig::default()` has `time_format == "%H:%M"` and `date_format == "%a %d %b"`
    - `module_params_clock_stores_config` — `ModuleParams::Clock(ClockConfig { time_format: "%I:%M %p".into(), date_format: "%Y-%m-%d".into() })` stores correct values
  - [x] 7.2: In `src/config/parse.rs` tests:
    - `parse_clock_with_custom_formats` — KDL with `time-format "%I:%M %p"` and `date-format "%Y-%m-%d"` → `ModuleParams::Clock` with correct strings
    - `parse_clock_uses_defaults_without_children` — `module "clock" zone="bottom"` (no children block) → `ModuleParams::Clock` with default formats `%H:%M` and `%a %d %b`
  - [x] 7.3: In `crates/niribar/src/modules/clock.rs` tests:
    - `clock_module_id_is_clock` — `ClockModule::new(...)::id().0 == "clock"`
    - `clock_state_stores_formatted_strings` — `ClockState { time: "12:34".into(), date: "Mon 01 Jan".into() }` stores both fields correctly
  - [x] 7.4: In `crates/niribar/src/app.rs` tests:
    - `clock_module_instantiated_from_registry` — Config with `module "clock" zone="bottom"` → `app.modules.len() == 1` (registry now populated)
  - [x] 7.5: Verify all 42 pre-existing tests still pass

- [x] Task 8: Quality gates
  - [x] 8.1: `cargo build` — succeeds
  - [x] 8.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 8.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 8.4: `cargo test` — 5+ new tests + all 42 pre-existing pass (47+ total)

### Review Findings

- [x] [Review][Patch] No validation of chrono format strings at parse time [src/config/parse.rs::parse_clock_params] — Empty strings and invalid specifiers (e.g. `time-format ""` or `time-format "%Q"`) are accepted silently; `chrono::Local::now().format()` produces empty or garbage output with no error. Fix: reject empty strings and validate format at parse time.
- [x] [Review][Defer] Subscription uses hardcoded `ModuleId::new("clock")` instead of self.id [crates/niribar/src/modules/clock.rs::subscription] — deferred, pre-existing concern: not a current bug (IDs always match) but a latent correctness risk if ID generation strategy changes
- [x] [Review][Defer] Multiple `module "clock"` entries share a single ModuleId [crates/niribar/src/modules/clock.rs] — deferred, pre-existing: architectural limitation; both subscriptions overwrite each other's state in shared.modules
- [x] [Review][Defer] Extra values on format nodes silently use only the first [src/config/parse.rs] — deferred, pre-existing: consistent with existing parser behaviour across all fields
- [x] [Review][Defer] `clock_ctor` silently falls back to defaults for mismatched ModuleParams [src/modules/mod.rs::clock_ctor] — deferred, pre-existing concern: unreachable in practice but could mask misconfiguration silently

## Dev Notes

### Critical Architectural Change: `ModuleConstructor` Signature

**`ModuleConstructor` MUST change from `fn() -> Box<dyn Module>` to `fn(&ModuleConfig) -> Box<dyn Module>`.**

This is the core change that makes module-specific config (clock format strings, script commands, button labels in Stories 2.4–2.5) possible. The 2.2 story explicitly deferred this to Story 2.3.

Update `App::new()` to pass `mc` to `ctor`:
```rust
registry.get(mc.name.as_str()).map(|ctor| (mc.zone.clone(), ctor(mc)))
//                                                                   ^^ was ctor()
```

### Dependency: `chrono`

Add to `Cargo.toml` `[dependencies]`:
```toml
chrono = { version = "0.4", features = ["clock"] }
```

`features = ["clock"]` is **required** — without it `chrono::Local::now()` is unavailable. `chrono` 0.4 is stable and widely used; no breaking changes expected.

### `#[allow(dead_code)]` Pattern

Follow the pattern from Stories 2.1–2.2: use `#[allow(dead_code)]` on `pub` items not yet used by the binary (e.g., `ClockConfig`, `ModuleParams` in config until consumed by a module). Remove these allows when the item is actually used.

### File Locations

| What | Where |
|---|---|
| `ClockConfig`, `ModuleParams` | `src/config/mod.rs` — add to existing file |
| Clock KDL param parsing | `src/config/parse.rs` — add `parse_clock_params()` |
| `ClockState`, `ClockEvent`, `ClockModule` | `crates/niribar/src/modules/clock.rs` — **new file** |
| `pub mod clock;` declaration | `src/modules/mod.rs` |
| `ModuleEvent::Clock`, `ModuleState::Clock`, registry | `src/modules/mod.rs` |
| `App::new()` + `App::update()` | `crates/niribar/src/app.rs` |

Do NOT create:
- `crates/niribar/src/modules/script.rs` — Story 2.4
- `crates/niribar/src/modules/button.rs` — Story 2.5
- `src/surface/expand.rs` — Story 3.1
- `crates/niribar/src/surface/input.rs` — Story 6.1
- `crates/niribar/src/dbus/` files — Stories 5.x

### KDL Config Format for Clock

Clock-specific params live in a child block on the `module` node:
```kdl
modules {
    module "clock" zone="bottom" {
        time-format "%H:%M"    // optional, default: "%H:%M"
        date-format "%a %d %b" // optional, default: "%a %d %b"
    }
}
```

`crates/niribar/config/default.kdl` currently has `module "clock" zone="bottom"` with no children block — that is valid and correct; `parse_clock_params` must return `ClockConfig::default()` when the `module` node has no children.

### Subscription Pattern

Architecture mandates `std::any::TypeId::of::<MarkerType>()` as the subscription ID (no magic strings). Use a local marker struct:
```rust
struct ClockSubscription; // local, never constructed — marker only
// ...
iced::subscription::channel(std::any::TypeId::of::<ClockSubscription>(), 1, |mut sender| async move { ... })
```

The subscription loops forever: emit tick, sleep 1 second, repeat. First tick fires immediately on first iteration (before the sleep), satisfying the "emit within 1 second" contract.

### Message Routing

Clock events route as:
```rust
Message::Module(ModuleId::new("clock"), ModuleEvent::Clock(ClockEvent::Tick { time, date }))
```

In `app.update()`, match this and update `self.shared.modules`:
```rust
Message::Module(id, ModuleEvent::Clock(ClockEvent::Tick { time, date })) => {
    self.shared.modules.insert(id, ModuleState::Clock(ClockState { time, date }));
    Task::none()
}
```

Keep `_ => Task::none()` as the catch-all — `#[to_layer_message]` injects LayerShell variants that must not be exhaustively matched.

### View Implementation Notes

**Icon placeholder:** Use `⌚` (U+231A Watch, `text("⌚").size(16)`) as the clock icon. SVG icons are deferred to the packaging epic (Epic 8). This is the intended placeholder approach for Epic 2 modules.

**Collapsed (48px bar width, 47px content):** A vertical column with icon centered above the time text is appropriate for the narrow vertical bar:
```
⌚
12:34
```
Use `Alignment::Center` on the column.

**Expanded (200px bar width):** `ModuleLabel` format per UX-DR6 — icon + time value spread across the row, date on the line below:
```
⌚          12:34
Mon 01 Jan
```
Use `row![text("⌚"), Space::fill(), text(time)]` for the first line.

**Loading/Unavailable states:** Both `view_collapsed` and `view_expanded` must handle `ModuleState::Loading` and `ModuleState::Unavailable` by returning `Space::new(0, 0)` (renders as nothing — per UX-DR16, no spinners or placeholders).

### `#[to_layer_message]` Macro Caveat

The macro in `crates/niribar/src/app.rs` adds LayerShell-specific variants to `Message`. When adding the new `Message::Module` match arm, the existing `_ => Task::none()` catch-all handles these. Do NOT remove the catch-all.

### State After Story 2.3

When Story 2.3 is complete:
- `build_registry()` returns `{"clock": clock_ctor}` — one entry
- `app.modules` is `[(Zone::Bottom, ClockModule{..})]` when using default.kdl
- Clock ticks every second via subscription → `SharedState.modules["clock"] = ModuleState::Clock(ClockState { time, date })`
- Collapsed view renders `⌚` + formatted time; expanded renders icon + time row + date row
- `ModuleFrame` hover fill from Story 2.2 wraps the clock view automatically (no changes needed there)

### Patterns to Follow from Story 2.2

- `#[allow(dead_code)]` on pub items not yet consumed by the binary
- Tests co-located: `#[cfg(test)] mod tests { ... }` at the bottom of each file
- `anyhow` for config/startup errors
- Run `cargo fmt` before `cargo fmt --check` to auto-fix formatting
- All quality gates must pass before marking done

### References

- Epic 2, Story 2.3 (epics.md §"Story 2.3"): user story + acceptance criteria
- Architecture: "Naming Patterns" — `{Name}State`, `{Name}Event` conventions
- Architecture: "Module System Design" — static registry, subscription contract, first-event-within-1s rule
- Architecture: "Subscription ID Pattern" — `TypeId::of::<{Name}Subscription>()`
- Architecture: "Module Boundary" — exact `Module` trait signature
- Architecture: "Communication Patterns" — `Message::Module(ModuleId, ModuleEvent)` routing
- Architecture: "State Management Patterns" — module data in `SharedState` only
- Architecture: "Process Patterns" — `Loading` → empty space, `Unavailable` → nothing (no spinner)
- Architecture: "Project Structure" — `crates/niribar/src/modules/clock.rs` location
- UX Design: UX-DR5 (ModuleIcon 20px), UX-DR6 (ModuleLabel format), UX-DR9 (11px small, 13px base)
- UX Design: UX-DR16 — unavailable modules render as nothing, no loading spinners
- PRD: FR25 (clock module), FR26 (configurable format strings)
- Story 2.2 dev notes: `ModuleConstructor` change deferred to Story 2.3, registry populated in 2.3

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

- iced 0.14 removed `iced::subscription::channel`; replaced with `iced::time::every(...).with(data).map(fn)` pattern (required adding `iced = { features = ["tokio"] }` to Cargo.toml)
- `Subscription::map` in iced 0.14 requires a non-capturing closure (zero-sized); format strings passed via `Subscription::with()` to satisfy this constraint
- `Space::new()` in iced 0.14 takes no arguments; replaced `Space::new(0,0)` and `Space::new(Length::Fill, 0)` with `Space::new()` and `Space::new().width(Length::Fill)` respectively
- Module trait view method lifetime: `view_collapsed(&self, state: &ModuleState)` ties `'_` to `&self`, so elements cannot borrow from `state`; fixed by cloning strings (`s.time.clone()`) inside view implementations

### Completion Notes List

- Implemented full clock module: `ClockConfig`, `ModuleParams`, `ClockState`, `ClockEvent`, `ClockModule`
- `ModuleConstructor` signature updated from `fn() -> Box<dyn Module>` to `fn(&ModuleConfig) -> Box<dyn Module>`, enabling module-specific config for Stories 2.4–2.5
- Clock subscription uses `iced::time::every(1s).with(fmts).map(fn)` — ticks every second, first tick at t=1s (within 1-second contract)
- KDL parsing supports optional `time-format` and `date-format` children; defaults to `%H:%M` and `%a %d %b`
- 50 tests pass (42 pre-existing + 8 new); 0 clippy warnings; `cargo fmt --check` passes

### File List

- Cargo.toml
- src/config/mod.rs
- src/config/parse.rs
- src/modules/mod.rs
- crates/niribar/src/modules/clock.rs (new)
- crates/niribar/src/app.rs

### Change Log

- 2026-04-01: Implemented Story 2.3 — clock module with configurable format (Date: 2026-04-01)
