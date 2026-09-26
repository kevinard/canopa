# Story 4.5: Graceful Degradation on Non-Niri Compositors

Status: review

## Story

As a **user running niribar on a non-Niri wlr-layer-shell compositor**,
I want Niri-specific modules to disappear quietly,
so that the bar is still usable with all other modules functioning normally.

## Acceptance Criteria

1. **Given** niribar is running on a compositor other than Niri (no Niri IPC socket available)
   **When** the bar starts with `workspaces` or `focused-window` in the module list
   **Then** those modules are silently omitted from the rendered bar — no gap, no placeholder

2. **Given** niribar is running without a Niri socket
   **When** non-Niri modules (clock, battery, etc.) are configured
   **Then** all non-Niri modules render and function normally

3. **Given** niribar starts on a non-Niri compositor
   **When** the niri subscription fires `NiriEvent::Unavailable`
   **Then** no error is printed to stderr for the missing Niri modules

4. **Given** niribar is running on Niri normally
   **When** all niri-dependent modules are rendering correctly
   **Then** the behavior is unchanged — regression guard against breaking the Niri path

5. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

6. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 399 pre-existing tests pass and at least 3 new tests pass

## Tasks / Subtasks

- [x] Task 1: Update `src/app.rs` — extend `NiriEvent::Unavailable` handler to push `ModuleState::Unavailable` to niri-dependent modules (AC: #1, #2, #3)
  - [x] 1.1: In the existing `Message::Niri(NiriEvent::Unavailable)` arm (currently at ~line 466), after setting `self.shared.niri.connected = false`, add a loop over `self.modules` that pushes `ModuleState::Unavailable` to all `"workspaces"` and `"focused-window"` modules:
    ```rust
    Message::Niri(NiriEvent::Unavailable) => {
        // Running on a non-Niri compositor or connection dropped.
        // Set connected = false; push Unavailable to all niri-dependent modules
        // so they render as nothing (zero size, no gap) — FR21, AC #1, UX-DR16.
        self.shared.niri.connected = false;
        for (_, module) in &self.modules {
            if matches!(module.module_type(), "workspaces" | "focused-window") {
                self.shared
                    .modules
                    .insert(module.id(), ModuleState::Unavailable);
            }
        }
        Task::none()
    }
    ```
    **Note:** Use `matches!(module.module_type(), "workspaces" | "focused-window")` — identical pattern to how `"network"`, `"volume"`, and `"battery"` Unavailable handlers work (see lines ~635–657 in app.rs).

- [x] Task 2: Tests in `src/app.rs` (AC: #1, #4, #6)
  - [x] 2.1: Update existing `niri_unavailable_event_does_not_crash` test to also assert that `workspaces` and `focused-window` module states become `Unavailable`. This test currently only checks `!app.shared.niri.connected` — expand it to verify the new state transitions, OR rename it and add a new focused test:
    ```rust
    #[test]
    fn niri_unavailable_sets_workspaces_module_unavailable() {
        use crate::config::{ModuleConfig, ModuleParams, Zone};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "workspaces".into(),
            zone: Zone::Center,
            params: ModuleParams::Workspaces(Default::default()),
        }];
        let (mut app, _) = App::boot(config, vec![]);
        let _ = app.update(Message::Niri(NiriEvent::Unavailable));
        let mid = crate::modules::ModuleId::new("workspaces");
        assert!(matches!(
            app.shared.modules.get(&mid),
            Some(ModuleState::Unavailable)
        ));
    }
    ```

  - [x] 2.2: Add test for `focused-window` module:
    ```rust
    #[test]
    fn niri_unavailable_sets_focused_window_module_unavailable() {
        use crate::config::{ModuleConfig, ModuleParams, Zone};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "focused-window".into(),
            zone: Zone::Center,
            params: ModuleParams::None,
        }];
        let (mut app, _) = App::boot(config, vec![]);
        let _ = app.update(Message::Niri(NiriEvent::Unavailable));
        let mid = crate::modules::ModuleId::new("focused-window");
        assert!(matches!(
            app.shared.modules.get(&mid),
            Some(ModuleState::Unavailable)
        ));
    }
    ```

  - [x] 2.3: Add regression test verifying non-Niri modules are NOT marked Unavailable by `NiriEvent::Unavailable`:
    ```rust
    #[test]
    fn niri_unavailable_does_not_affect_non_niri_modules() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams, Zone};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            zone: Zone::Center,
            params: ModuleParams::Clock(ClockConfig::default()),
        }];
        let (mut app, _) = App::boot(config, vec![]);
        let _ = app.update(Message::Niri(NiriEvent::Unavailable));
        let mid = crate::modules::ModuleId::new("clock");
        // Clock state should be Loading (not Unavailable) — niri events don't touch it
        assert!(!matches!(
            app.shared.modules.get(&mid),
            Some(ModuleState::Unavailable)
        ));
    }
    ```

- [x] Task 3: Quality gates (AC: #5, #6)
  - [x] 3.1: `cargo build` — succeeds
  - [x] 3.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 3.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 3.4: `cargo test` — all 399 pre-existing tests pass + 3 new tests (402 total)

## Dev Notes

### What Exists After Story 4.4

The IPC pipeline is fully wired:
- `src/niri/subscription.rs`: `niri_subscription()` emits `NiriEvent::Unavailable` (once, terminates stream) when socket is unreachable. Does NOT log to stderr — silent by design.
- `src/niri.rs`: `NiriEvent` enum has `Connected`, `Unavailable`, `Event(Event)` variants. `NiriState` has `connected: bool`.
- `src/app.rs` `NiriEvent::Unavailable` handler (~line 466): currently only sets `self.shared.niri.connected = false`. **This is the ONLY file to change.**
- `src/modules/workspaces.rs` view functions: `_ =>` arm returns `Space::new()` — already handles `Unavailable` correctly.
- `src/modules/focused_window.rs` view functions: `_ =>` arm returns `Space::new()` — already handles `Unavailable` correctly.
- `ModuleState::Unavailable` is already defined in `src/modules.rs` and rendering infrastructure handles it as zero-size.

### The One Change Required

The fix is **three lines** added to an existing match arm in `src/app.rs`. No new files, no new traits, no new module code. The view layer already handles `Unavailable` via the `_ => Space::new()` catch-all in both `workspaces.rs` and `focused_window.rs`.

**Before:**
```rust
Message::Niri(NiriEvent::Unavailable) => {
    self.shared.niri.connected = false;
    Task::none()
}
```

**After:**
```rust
Message::Niri(NiriEvent::Unavailable) => {
    self.shared.niri.connected = false;
    for (_, module) in &self.modules {
        if matches!(module.module_type(), "workspaces" | "focused-window") {
            self.shared
                .modules
                .insert(module.id(), ModuleState::Unavailable);
        }
    }
    Task::none()
}
```

### Pattern Precedent: D-Bus Unavailable Handlers

See the exact same pattern used 3 times already in `app.rs`:
- `Message::Network(NetworkEvent::Unavailable)` — ~line 635
- `Message::Volume(VolumeEvent::Unavailable)` — ~line 649
- `Message::Battery(BatteryEvent::Unavailable)` — ~line 704

Each iterates `self.modules`, matches on `module.module_type()`, and inserts `ModuleState::Unavailable`. **Copy that pattern exactly.**

### Why `module_type()` Not `id()`

Module IDs are user-defined names from config (e.g., `"workspaces"`, `"my-workspace-bar"`). `module_type()` returns the canonical type string (always `"workspaces"` or `"focused-window"`). Match on `module_type()` — same as the D-Bus handlers.

### Why View Code Needs No Changes

Both modules use this pattern:
```rust
match state {
    ModuleState::FocusedWindow(s) => &s.title,  // or Workspace
    _ => return iced::widget::Space::new().into(),  // catches Loading AND Unavailable
}
```
`ModuleState::Unavailable` falls into `_` and renders as zero-size space. UX-DR16 satisfied.

### No stderr Logging

The `niri_subscription()` already silently terminates on socket failure — no `eprintln!` calls. The `NiriEvent::Unavailable` handler must NOT add logging. Non-Niri users should see zero indication in stderr.

### Test Configuration Helpers

In app.rs tests, look at how existing tests for battery/network/volume Unavailable are structured (`niri_unavailable_sets_workspaces_module_unavailable` etc.). They use `App::boot(config, vec![])` with a config containing just the module under test. Follow this exact pattern.

For `ModuleParams::Workspaces`, use `ModuleParams::Workspaces(Default::default())` — `WorkspacesConfig` implements `Default`.

For `ModuleParams::None` with `focused-window` — check config.rs; `ModuleParams::None` is the fallback for modules with no params.

### Test Config Struct

Tests use `Config::default()` then set `config.modules`. Look at `make_config()` helper and `make_focused_window_app()` helper already in the test section of app.rs (~lines 1750–1850) to understand the pattern. Do NOT duplicate `make_config()` — use it where appropriate, or create a minimal config inline.

### File Locations

| What | Where |
|---|---|
| `NiriEvent::Unavailable` handler | `src/app.rs` — match arm in `update()` |
| New tests | `src/app.rs` — `#[cfg(test)]` section, after Story 4.1 tests |

### References

- Epic 4 §"Story 4.5": user story + 4 ACs
- Architecture §"Niri IPC": FR21 — "niri modules fail gracefully when running on any other compositor"
- `src/niri/subscription.rs`: `NiriSubState::Done` path — `Unavailable` emitted once, stream terminates
- `src/app.rs` ~line 635: `NetworkEvent::Unavailable` — canonical pattern to follow
- `src/modules/workspaces.rs`: `_ => Space::new()` already handles `Unavailable`
- `src/modules/focused_window.rs`: same `_ => Space::new()` pattern
- UX-DR16: unavailable modules render as nothing (zero size, no gap)

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

None — implementation proceeded without errors.

### Completion Notes List

- Extended `NiriEvent::Unavailable` handler in `app.rs` with a 6-line loop (matching the D-Bus Unavailable pattern) that pushes `ModuleState::Unavailable` to all `"workspaces"` and `"focused-window"` modules. No view-layer changes required — both modules already handled `Unavailable` via their `_ => Space::new()` catch-all.
- Added 3 new tests using existing `make_workspaces_app()` and `make_focused_window_app()` helpers: workspaces gets `Unavailable`, focused-window gets `Unavailable`, clock is unaffected.
- 399 pre-existing tests + 3 new = 402 total. All pass. Clippy clean. Fmt clean.

### File List

- `src/app.rs` — extended `NiriEvent::Unavailable` handler; added 3 tests

### Change Log

- Extended `NiriEvent::Unavailable` to push `ModuleState::Unavailable` to `workspaces` and `focused-window` modules, so they render as zero-size on non-Niri compositors (FR21, UX-DR16)
