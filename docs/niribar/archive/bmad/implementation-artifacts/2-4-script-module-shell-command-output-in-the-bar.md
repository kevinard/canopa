# Story 2.4: Script Module — Shell Command Output in the Bar

Status: done

## Story

As a **user**,
I want to run a shell command and display its stdout in the bar,
So that I can show arbitrary system state the built-in modules don't cover.

## Acceptance Criteria

1. **Given** a `script` module entry with `command "..."` and `interval N` in config
   **When** the bar renders
   **Then** the command is executed via `sh -c <command>` in a Tokio task at the configured interval

2. **Given** the script command produces stdout output
   **When** the command exits with code 0
   **Then** the stdout (trimmed) is displayed as the module value in expanded state
   **And** the first N characters (≤5) are shown in collapsed state (icon + truncated text)

3. **Given** the script command exits with a non-zero exit code OR fails to spawn
   **When** the failure occurs
   **Then** the previous displayed output is retained unchanged — no crash, no error shown in the bar

4. **Given** the script module is running
   **When** the command is executing
   **Then** the render loop is never blocked — command runs asynchronously off the Iced update thread

5. **Given** the command runs with `sh -c`
   **When** the process is spawned
   **Then** it runs with the permissions of the niribar process — no privilege escalation, no sandboxing imposed (NFR7)

6. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

7. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 50 pre-existing tests pass and at least 6 new tests pass

## Tasks / Subtasks

- [x] Task 1: Add `ScriptConfig` and `ModuleParams::Script` in `src/config/mod.rs` (AC: #1, #2)
  - [x] 1.1: Add `ScriptConfig` struct after `ClockConfig`:
    ```rust
    #[derive(Debug, Clone)]
    pub struct ScriptConfig {
        pub command: String,
        pub interval: u64, // seconds between executions
    }
    ```
    No `Default` impl — `command` is required with no sensible default.
  - [x] 1.2: Add `Script(ScriptConfig)` to `ModuleParams` enum (replace the `// Script(ScriptConfig) — Story 2.4` comment):
    ```rust
    pub enum ModuleParams {
        None,
        Clock(ClockConfig),
        Script(ScriptConfig),
        // Button(ButtonConfig) — Story 2.5
    }
    ```

- [x] Task 2: Parse script params from KDL in `src/config/parse.rs` (AC: #1)
  - [x] 2.1: Add `parse_script_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<ScriptConfig>`:
    - Look for children block; if absent → error (`command` is required)
    - Read `command` child node as positional string arg (required):
      ```
      error: "modules[{idx}].script.command: missing required parameter. Expected: a string value."
      error: "modules[{idx}].script.command: expected a string value."  // wrong type
      ```
    - Read `interval` child node as positional integer arg (optional, default: `5`):
      - If present but not an integer → `"modules[{idx}].script.interval: expected a positive integer."`
      - If present but zero → `"modules[{idx}].script.interval: value must be at least 1. Expected: a positive integer."`
    - Return `ScriptConfig { command, interval }`
  - [x] 2.2: In `parse_modules_section`, add `"script"` branch before the `else` fallthrough:
    ```rust
    let params = if name == "clock" {
        ModuleParams::Clock(parse_clock_params(node, i)?)
    } else if name == "script" {
        ModuleParams::Script(parse_script_params(node, i)?)
    } else {
        ModuleParams::None
    };
    ```
  - [x] 2.3: Add `ScriptConfig` to the `use` import at the top of `parse.rs`:
    ```rust
    use super::{BarConfig, ClockConfig, Config, Edge, ModuleConfig, ModuleParams, ScriptConfig};
    ```

- [x] Task 3: Extend module system in `src/modules/mod.rs` (AC: #3, #4)
  - [x] 3.1: Add `pub mod script;` declaration at top (alongside `pub mod clock;`)
  - [x] 3.2: Add `Script(script::ScriptState)` to `ModuleState` enum (replace comment placeholder):
    ```rust
    pub enum ModuleState {
        #[default]
        Loading,
        #[allow(dead_code)]
        Unavailable,
        Clock(clock::ClockState),
        Script(script::ScriptState),
        // Button(ButtonState) — Story 2.5
    }
    ```
  - [x] 3.3: Add `Script(script::ScriptEvent)` to `ModuleEvent` enum:
    ```rust
    pub enum ModuleEvent {
        Clock(clock::ClockEvent),
        Script(script::ScriptEvent),
    }
    ```
  - [x] 3.4: Register `"script"` in `build_registry()`:
    ```rust
    map.insert("script", script_ctor as ModuleConstructor);
    ```
  - [x] 3.5: Add `script_ctor` function alongside `clock_ctor`:
    ```rust
    fn script_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
        use crate::config::ModuleParams;
        match &mc.params {
            ModuleParams::Script(cfg) => {
                Box::new(script::ScriptModule::new(cfg.command.clone(), cfg.interval))
            }
            _ => panic!("script_ctor called with non-Script params — this is a programming error"),
        }
    }
    ```
    Note: `panic!` here is acceptable — this represents a bug in the registry construction code, not a runtime user error. It will only fire if the registry is wired incorrectly.

- [x] Task 4: Implement `ScriptModule` in `crates/niribar/src/modules/script.rs` (new file) (AC: #1, #2, #3, #4, #5)
  - [x] 4.1: Define types:
    ```rust
    /// Runtime state for the script module.
    #[derive(Debug, Clone)]
    pub struct ScriptState {
        pub output: String,
    }

    /// Events emitted by the script subscription.
    #[derive(Debug, Clone)]
    pub enum ScriptEvent {
        /// Command exited 0; stdout (trimmed) is the new output.
        Output(String),
        /// Command exited non-zero or failed to spawn; previous output retained.
        Failed,
    }

    pub struct ScriptModule {
        id: crate::modules::ModuleId,
        command: String,
        interval: u64,
    }

    impl ScriptModule {
        pub fn new(command: String, interval: u64) -> Self {
            ScriptModule {
                id: crate::modules::ModuleId::new("script"),
                command,
                interval,
            }
        }
    }
    ```
  - [x] 4.2: Implement `Module::subscription()` using an async loop with `tokio::process::Command`:

    **CRITICAL — Subscription Pattern for iced 0.14:**

    The clock uses `iced::time::every(...).with(data).map(fn)` with a synchronous map. This CANNOT be used for the script module because running a process requires `await`. The map closure in `iced::time::every().map()` is synchronous — calling `.await` is not possible inside it.

    **Recommended approach — `iced::futures::stream::unfold`:**
    ```rust
    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        use std::any::TypeId;

        struct ScriptSubscription;

        let cmd = self.command.clone();
        let interval = self.interval;
        let id = self.id.clone();

        iced::Subscription::run_with_id(
            TypeId::of::<ScriptSubscription>(),
            iced::futures::stream::unfold((cmd, interval, id), |(cmd, interval, id)| async move {
                // Wait the configured interval before each execution
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

                let event = match tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .output()
                    .await
                {
                    Ok(out) if out.status.success() => {
                        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        ScriptEvent::Output(text)
                    }
                    _ => ScriptEvent::Failed,
                };

                let msg = crate::app::Message::Module(
                    id.clone(),
                    crate::modules::ModuleEvent::Script(event),
                );
                Some((msg, (cmd, interval, id)))
            }),
        )
    }
    ```

    **If `iced::Subscription::run_with_id` is not available in iced 0.14:**

    Look for one of these alternatives in the iced 0.14 API:
    - `iced::Subscription::run(stream)` — may not accept an ID
    - `iced::advanced::subscription::channel(id, buffer, |sender| async { ... })` — if still present under a different path
    - `iced::subscription::channel(...)` — check if it was moved, not removed

    The subscription must use `tokio::process::Command` (async), NOT `std::process::Command` (blocking). Using `std::process::Command` inside an async context would block a Tokio worker thread, violating AC #4.

    **Contract:** First execution fires after `interval` seconds (not immediately) — this is acceptable for a polled data source. If early first-tick is needed, restructure to execute first, then loop.

  - [x] 4.3: Implement `view_collapsed()`:
    ```rust
    fn view_collapsed(&self, state: &crate::modules::ModuleState) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, text};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Script(s) => {
                let truncated: String = s.output.chars().take(5).collect();
                column![
                    text(">_").size(16),                   // terminal icon placeholder
                    text(truncated).size(11),              // font.size.small per UX-DR9
                ]
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            _ => iced::widget::Space::new().into(),
        }
    }
    ```
  - [x] 4.4: Implement `view_expanded()`:
    ```rust
    fn view_expanded(&self, state: &crate::modules::ModuleState) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{row, text};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Script(s) => row![
                text(">_").size(16),
                iced::widget::Space::new().width(Length::Fill),
                text(s.output.clone()).size(13), // font.size.base per UX-DR9
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into(),
            _ => iced::widget::Space::new().into(),
        }
    }
    ```
  - [x] 4.5: Implement `id()`:
    ```rust
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }
    ```
  - [x] 4.6: Add `use tokio::process::Command as TokioCommand;` or use the full path `tokio::process::Command` inline. No new crate dep needed — `tokio = { features = ["full"] }` already includes `tokio::process`.

- [x] Task 5: Update `crates/niribar/src/app.rs` — event routing (AC: #3)
  - [x] 5.1: Add to the `use` imports:
    ```rust
    use crate::modules::script::{ScriptEvent, ScriptState};
    ```
  - [x] 5.2: Add match arms in `update()` BEFORE the `_ => Task::none()` catch-all:
    ```rust
    Message::Module(id, ModuleEvent::Script(ScriptEvent::Output(text))) => {
        self.shared
            .modules
            .insert(id, ModuleState::Script(ScriptState { output: text }));
        Task::none()
    }
    Message::Module(_, ModuleEvent::Script(ScriptEvent::Failed)) => {
        // Retain previous state — do nothing (AC #3)
        Task::none()
    }
    ```
    Keep existing `_ => Task::none()` catch-all — `#[to_layer_message]` injects LayerShell variants.

- [x] Task 6: Tests (AC: #6, #7)
  - [x] 6.1: In `src/config/mod.rs` tests:
    - `script_config_stores_command_and_interval` — `ScriptConfig { command: "echo hi".into(), interval: 10 }` stores both fields
    - `module_params_script_stores_config` — `ModuleParams::Script(cfg)` round-trips the command and interval
  - [x] 6.2: In `src/config/parse.rs` tests:
    - `parse_script_with_command_and_interval` — KDL with `command "date"` and `interval 10` → `ScriptConfig { command: "date", interval: 10 }`
    - `parse_script_defaults_interval_to_5` — KDL with `command "date"` only → `ScriptConfig { interval: 5 }`
    - `parse_script_missing_command_errors` — `module "script" zone="bottom"` (no children) → `Err` containing `"script.command"`
  - [x] 6.3: In `crates/niribar/src/modules/script.rs` tests:
    - `script_module_id_is_script` — `ScriptModule::new("echo hi".into(), 5).id().0 == "script"`
    - `script_state_stores_output` — `ScriptState { output: "hello".into() }.output == "hello"`
    - `script_event_failed_variant` — `ScriptEvent::Failed` constructs without error
  - [x] 6.4: In `crates/niribar/src/app.rs` tests:
    - `script_module_instantiated_from_registry` — Config with `module "script" zone="bottom"` and `ScriptConfig` → `app.modules.len() == 1`
    - `script_output_event_updates_state` — `update(Message::Module(id, ModuleEvent::Script(ScriptEvent::Output("hi".into()))))` → state is `ModuleState::Script(ScriptState { output: "hi" })`
    - `script_failed_event_retains_previous_state` — set state to Script("prev"), then `update(ScriptEvent::Failed)` → state is still `ModuleState::Script(ScriptState { output: "prev" })`
  - [x] 6.5: Verify all 50 pre-existing tests still pass (`cargo test`)

- [x] Task 7: Quality gates
  - [x] 7.1: `cargo build` — succeeds
  - [x] 7.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 7.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 7.4: `cargo test` — 6+ new tests + all 50 pre-existing pass (56+ total)

## Dev Notes

### No New Dependencies

`tokio = { version = "1", features = ["full"] }` in `Cargo.toml` already includes `tokio::process`. **Do NOT add any new crate dependency** for script execution — `tokio::process::Command` is all that is needed.

### Critical: Subscription Must Be Async

Unlike the clock, the script subscription MUST use an async loop. `std::process::Command::output()` blocks a thread — never call it from an async Tokio context. Use `tokio::process::Command::output().await`.

The clock uses `iced::time::every(...).with(data).map(fn)` with a synchronous closure. This pattern CANNOT be used here. Investigate these iced 0.14 APIs in order:
1. `iced::Subscription::run_with_id(id, stream)` where stream is from `iced::futures::stream::unfold`
2. `iced::subscription::channel(...)` — may have been moved, not removed; check `iced::advanced::subscription`
3. If neither works, use `iced::futures::stream::unfold` directly with `iced::Subscription::run`

The `unfold` approach (recommended, Task 4.2 above) creates a `Stream<Item = Message>` from an async state machine — each iteration sleeps `interval` seconds, runs the command, yields a message, and carries state forward. This satisfies both non-blocking (AC #4) and previous-output-retention (AC #3) requirements.

### Retention of Previous Output

AC #3 requires retaining previous output on command failure. This is handled entirely in `app.update()`:
- `ScriptEvent::Output(text)` → insert new `ModuleState::Script(ScriptState { output: text })`
- `ScriptEvent::Failed` → do nothing (previous state in `SharedState.modules` unchanged)

The `ScriptModule` itself has no internal output storage — all state lives in `SharedState.modules` (architecture rule: module data in SharedState, never in module struct).

### KDL Config Format

```kdl
modules {
    module "script" zone="bottom" {
        command "date +%H:%M:%S"  // required; no default
        interval 5                 // optional, default: 5 (seconds)
    }
}
```

- `command` is a positional string arg on the `command` child node (same as `time-format` in clock)
- `interval` is a positional integer arg on the `interval` child node (optional, default 5)
- `command` is **required** — error if `module "script"` has no children block or no `command` child

### File Locations

| What | Where |
|---|---|
| `ScriptConfig`, `ModuleParams::Script` | `src/config/mod.rs` — extend existing file |
| `parse_script_params()` | `src/config/parse.rs` — add new function |
| `ScriptState`, `ScriptEvent`, `ScriptModule` | `crates/niribar/src/modules/script.rs` — **new file** |
| `pub mod script;`, `ModuleState::Script`, `ModuleEvent::Script`, registry | `src/modules/mod.rs` |
| `ScriptEvent` routing | `crates/niribar/src/app.rs` |

Do NOT create:
- `crates/niribar/src/modules/button.rs` — Story 2.5
- `src/surface/expand.rs` — Story 3.1
- Any `crates/niribar/src/dbus/` files — Stories 5.x
- Do NOT modify `crates/niribar/config/default.kdl` — the default config has no script module (command is user-specific)

### ModuleId for Script

`ScriptModule` uses `ModuleId::new("script")`. Multiple `module "script"` blocks in config would share the same ID and overwrite each other's state in `SharedState.modules`. This is a known architectural limitation (same as multiple clock modules — see Story 2.3 deferred item). Do NOT attempt to fix it in this story.

### `#[allow(dead_code)]` Pattern

Follow the Story 2.3 pattern: apply `#[allow(dead_code)]` to `ScriptState` fields or enum variants that are defined but not yet consumed by the binary. Remove allows as items get used.

### View Implementation Notes

**Collapsed (48px bar, 47px content):**
- Icon: `">_"` (text, 16px) — common terminal/script glyph; no SVG yet (Epic 8)
- Truncated output: `s.output.chars().take(5).collect::<String>()` — shows up to 5 chars at 11px
- Column layout: icon above truncated text, center-aligned (matches clock pattern)

**Expanded (200px bar):**
- `ModuleLabel` format per UX-DR6: icon left, value right
- `row![text(">_").size(16), Space::fill(), text(output).size(13)]`
- Use `s.output.clone()` — do NOT borrow from `state` (Module trait lifetime constraint, per Story 2.3 debug log)

**Loading/Unavailable:** Both views return `Space::new()` — no spinners, no placeholders (UX-DR16).

**Space::new() usage in iced 0.14:**
- Empty spacer: `iced::widget::Space::new()` (no args)
- Horizontal fill: `iced::widget::Space::new().width(Length::Fill)`

### `#[to_layer_message]` Macro Caveat

Keep the `_ => Task::none()` catch-all in `app.update()`. The macro injects LayerShell variants into `Message` that must not be exhaustively matched. Adding new arms must be BEFORE the catch-all.

### State After Story 2.4

When Story 2.4 is complete:
- `build_registry()` returns `{"clock": clock_ctor, "script": script_ctor}` — two entries
- Config with a `script` module → `app.modules` includes a `ScriptModule`
- Script executes every `interval` seconds → `SharedState.modules["script"] = ModuleState::Script(ScriptState { output: "..." })`
- Collapsed view: `">_"` icon + first 5 chars; expanded: icon row + full output

### Test: `script_failed_event_retains_previous_state`

This test requires setting up initial state before calling update:
```rust
#[test]
fn script_failed_event_retains_previous_state() {
    use crate::config::{ModuleConfig, ModuleParams, ScriptConfig};
    use crate::modules::{ModuleId, ModuleState, Zone};
    use crate::modules::script::{ScriptEvent, ScriptState};

    let mut config = Config::default();
    config.modules = vec![ModuleConfig {
        name: "script".into(),
        zone: Zone::Bottom,
        params: ModuleParams::Script(ScriptConfig { command: "echo hi".into(), interval: 5 }),
    }];
    let mut app = App::new(config);

    // Pre-populate state with a known output
    let id = ModuleId::new("script");
    app.shared.modules.insert(id.clone(), ModuleState::Script(ScriptState { output: "prev".into() }));

    // Fire a Failed event
    let _ = app.update(Message::Module(id.clone(), ModuleEvent::Script(ScriptEvent::Failed)));

    // State must be unchanged
    match app.shared.modules.get(&id) {
        Some(ModuleState::Script(s)) => assert_eq!(s.output, "prev"),
        other => panic!("expected Script state, got {:?}", other),
    }
}
```

### References

- Epic 2, Story 2.4 (epics.md §"Story 2.4"): user story + acceptance criteria
- Architecture: "Module System Design" — static registry, subscription contract
- Architecture: "Naming Patterns" — `ScriptState`, `ScriptEvent` conventions
- Architecture: "Module Boundary" — exact `Module` trait signature
- Architecture: "Communication Patterns" — `Message::Module(ModuleId, ModuleEvent)` routing
- Architecture: "State Management Patterns" — module data in `SharedState` only

### Review Findings

**Reviewed:** 2026-04-01 | **Result:** 1 decision-needed, 1 patch, 4 deferred, 5 dismissed

#### Decision Needed

- **[D1] First command execution delayed by full interval** (`crates/niribar/src/modules/script.rs::subscription`): `stream::unfold` sleeps `interval` seconds before the first `sh -c` call. The architecture contract (`src/modules/mod.rs` line 64) requires "emit within 1 second or emit `Unavailable`." The story spec notes ("Contract: First execution fires after `interval` seconds — acceptable for a polled data source") acknowledges this but creates a spec/arch conflict. Options: **(A)** execute command immediately then enter the sleep→exec loop (satisfies subscription contract); **(B)** emit `ScriptEvent::Failed` as an `Unavailable` signal before the first sleep (no initial state, then populates after first interval).

#### Patch Required

- **[P1] Empty command string not rejected** (`src/config/parse.rs::parse_script_params`): `command ""` is accepted; `sh -c ""` runs silently and produces empty output with no error. Add a guard analogous to clock's empty-format rejection: `Some("") => bail!("modules[{}].script.command: must not be empty", idx)`.

#### Deferred

- **[Defer-1] `ModuleId` hardcoded as `"script"` in subscription** (`crates/niribar/src/modules/script.rs:66`): subscription always emits `ModuleId::new("script")` regardless of `self.id`; affects multi-instance correctness. Explicitly deferred per story notes (same pattern as clock). Revisit with unique-ID scheme.
- **[Defer-2] Non-UTF-8 output silently replaced** (`crates/niribar/src/modules/script.rs:59`): `String::from_utf8_lossy` replaces invalid bytes with U+FFFD rather than treating it as a failure. Acceptable for a bar display; revisit if scripts producing binary output need explicit error handling.
- **[Defer-3] Very large interval effectively infinite sleep** (`crates/niribar/src/modules/script.rs:50`): `interval: u64` with no upper bound; `u64::MAX` seconds is functionally never. Story spec only rejects zero. Add an upper-bound validation (e.g., ≤ 3600) in a future config hardening pass.
- **[Defer-4] All command errors collapse to `Failed` with no logging** (`crates/niribar/src/modules/script.rs:62`): spawn failures (e.g., `sh` not found) and non-zero exit codes are indistinguishable; no stderr captured. By-design per spec; revisit when a logging/diagnostic layer is introduced.

#### Dismissed

- **Shell injection via `command` string** — by-design (NFR7/AC #5); user config, no privilege escalation imposed.
- **Unbounded process spawning** — false positive; `stream::unfold` is sequential; next iteration cannot start until `output().await` returns.
- **Race condition on concurrent `ScriptEvent` delivery** — only relevant with multiple instances sharing `ModuleId("script")`; tracked in Defer-1.
- **`script_ctor` panic message says "programming error"** — accurate; unreachable in correct operation; not misleading.
- **`PartialEq` not derived on `ScriptState`/`ScriptEvent`** — consistent with `ClockState`/`ClockEvent`; not needed for current test coverage.
- Architecture: "Process Patterns" — `Loading`/`Unavailable` → `Space::new()` (no spinners)
- Architecture: "Project Structure" — `crates/niribar/src/modules/script.rs` location
- Architecture: FR11–FR16 locations — `crates/niribar/src/modules/`
- UX Design: UX-DR5 (icon sizing), UX-DR6 (ModuleLabel format), UX-DR9 (11px small, 13px base), UX-DR16 (unavailable → nothing)
- PRD: FR16 (custom script module), NFR7 (no privilege escalation)
- Story 2.3 dev notes: iced 0.14 subscription pattern changes, `Space::new()` API, `clone()` required in view methods, `ModuleConstructor` signature

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

- `Subscription::run_with<D, S>(data: D, builder: fn(&D) -> S)` is the correct iced 0.14 API for async subscriptions. The builder is a function pointer (non-capturing closure coerces to `fn`). The old `subscription::channel` is gone; replaced by `Subscription::run_with` + `iced::futures::stream::unfold`.
- `iced::stream::channel(size, async |sender| { ... })` also exists in iced 0.14 (in `iced_futures::stream`) and uses stable async closures (Rust 2024 edition). Either approach works; `stream::unfold` was chosen to match the pattern used by `iced::time::every`.
- `(String, u64): Hash` — both `String` and `u64` implement `Hash`; tuple of Hash types is Hash. The `D: Hash` constraint in `run_with` is satisfied.
- `tokio::process::Command` is available via `tokio = { features = ["full"] }` — no new dependency needed.
- Unused `ScriptState` import in test — removed local import; top-level `use` in `app.rs` covers it.
- `Module` trait must be in scope to call `.id()` on `ScriptModule` in tests — added `use crate::modules::Module;`.

### Completion Notes List

- Implemented `ScriptConfig { command, interval }` in `src/config/mod.rs`; added `ModuleParams::Script(ScriptConfig)` variant
- `parse_script_params()` in `src/config/parse.rs`: `command` required, `interval` optional (default 5), zero/negative interval rejected with actionable error
- `ScriptModule`, `ScriptState`, `ScriptEvent::Output/Failed` in `crates/niribar/src/modules/script.rs` (new file)
- Subscription uses `Subscription::run_with((cmd, interval), fn_ptr)` + `stream::unfold` — fully async via `tokio::process::Command`, never blocks the render loop
- Output retention on failure: `ScriptEvent::Failed` → no-op in `app.update()`; previous `SharedState.modules` entry unchanged
- `build_registry()` now returns `{"clock": clock_ctor, "script": script_ctor}` — both modules registered
- `app.update()` routes `ScriptEvent::Output` (inserts new state) and `ScriptEvent::Failed` (no-op)
- 65 tests total: 50 pre-existing + 15 new (2 config/mod, 4 parse, 3 script module, 3 app, 1 module registry, 2 config types)
- 0 clippy warnings; `cargo fmt --check` passes; `cargo build` clean

### File List

- Cargo.toml (unchanged — no new deps needed)
- src/config/mod.rs
- src/config/parse.rs
- src/modules/mod.rs
- crates/niribar/src/modules/script.rs (new)
- crates/niribar/src/app.rs

### Change Log

- 2026-04-01: Implemented Story 2.4 — script module with shell command output (Date: 2026-04-01)
