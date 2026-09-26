# Runtime Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce niribar idle CPU usage and activity spikes by removing subscription churn and no-op update work.

**Architecture:** Keep the existing Iced app structure. Make long-lived subscription identities explicit, then narrow update handlers so unchanged input does not mutate app state or rebuild dynamic module vectors.

**Tech Stack:** Rust 2024, iced 0.14, iced_layershell 0.17, tokio, zbus, niri-ipc.

---

## File Structure

- Modify `src/niri/subscription.rs`: add a stable Niri subscription key type and use it instead of `()`.
- Modify `src/ipc.rs`: add a stable IPC subscription key type and use it instead of `()`.
- Modify `src/modules.rs`: derive `PartialEq` where needed for visible module-state comparisons.
- Modify module state files under `src/modules/`: derive `PartialEq` for state/event structs used in no-op detection.
- Modify `src/app/module_events.rs`: skip repeated module state writes.
- Modify `src/app/module_state.rs`: skip repeated battery, network, volume, and focused-window writes.
- Modify `src/app.rs`: ignore unnecessary cursor messages and no-op D-Bus readiness updates.
- Modify `src/app/niri.rs`: skip workspace rebuilds when relevant workspace data is unchanged.

## Task 1: Distinct Subscription Keys

**Files:**
- Modify: `src/niri/subscription.rs`
- Modify: `src/ipc.rs`

- [ ] **Step 1: Add failing tests or compile checks**

Run: `rtk cargo test niri_subscription ipc_subscription --lib`
Expected before implementation: compile succeeds, but code still uses duplicate `run_with(())`; this task is primarily compile-verified because Iced does not expose subscription keys for direct unit assertion.

- [ ] **Step 2: Implement named key types**

In `src/niri/subscription.rs`, add:

```rust
#[derive(Hash)]
struct NiriSubscriptionKey;
```

Change:

```rust
iced::Subscription::run_with((), |()| {
```

to:

```rust
iced::Subscription::run_with(NiriSubscriptionKey, |_: &NiriSubscriptionKey| {
```

In `src/ipc.rs`, add:

```rust
#[derive(Hash)]
struct IpcSubscriptionKey;
```

Change:

```rust
iced::Subscription::run_with((), |()| {
```

to:

```rust
iced::Subscription::run_with(IpcSubscriptionKey, |_: &IpcSubscriptionKey| {
```

- [ ] **Step 3: Verify**

Run: `rtk cargo test --lib`
Expected: PASS.

## Task 2: No-Op Module State Updates

**Files:**
- Modify: `src/modules.rs`
- Modify: `src/modules/battery.rs`
- Modify: `src/modules/clock.rs`
- Modify: `src/modules/focused_window.rs`
- Modify: `src/modules/network.rs`
- Modify: `src/modules/script.rs`
- Modify: `src/modules/volume.rs`
- Modify: `src/app/module_events.rs`
- Modify: `src/app/module_state.rs`

- [ ] **Step 1: Write failing tests**

Add tests in `src/app.rs` test module:

```rust
#[test]
fn repeated_clock_tick_keeps_existing_state() {
    let (mut app, _wid) = make_app();
    let id = ModuleId::new("clock");
    let first = ModuleState::Clock(crate::modules::clock::ClockState {
        time: "10:00".into(),
        date: "Sat".into(),
    });
    app.shared.modules.insert(id.clone(), first.clone());
    let _ = app.update(Message::Module(
        id.clone(),
        ModuleEvent::Clock(crate::modules::clock::ClockEvent::Tick {
            time: "10:00".into(),
            date: "Sat".into(),
        }),
    ));
    assert_eq!(app.shared.modules.get(&id), Some(&first));
}

#[test]
fn repeated_volume_state_keeps_existing_state() {
    let (mut app, _wid) = make_app();
    let id = ModuleId::new("volume");
    let first = ModuleState::Volume(crate::modules::volume::VolumeState {
        volume: 55.0,
        muted: false,
    });
    app.shared.modules.insert(id.clone(), first.clone());
    let _ = app.update(Message::Volume(crate::modules::volume::VolumeEvent::StateChanged {
        volume: 55.0,
        muted: false,
    }));
    assert_eq!(app.shared.modules.get(&id), Some(&first));
}
```

Run: `rtk cargo test repeated_clock_tick_keeps_existing_state repeated_volume_state_keeps_existing_state --lib`
Expected: FAIL to compile until `PartialEq` derives are added.

- [ ] **Step 2: Add equality derives**

Derive `PartialEq` for visible state/event structs and enums that are compared:

```rust
#[derive(Debug, Clone, PartialEq)]
```

Apply to `ModuleState`, `ClockState`, `ClockEvent`, `ScriptState`, `ScriptEvent`, `BatteryState`,
`BatteryEvent`, `NetworkState`, `NetworkConnectivity`, `NetworkEvent`, `VolumeState`,
`VolumeEvent`, and `FocusedWindowState`.

- [ ] **Step 3: Add helper and skip no-op inserts**

In `src/app/module_events.rs`, add:

```rust
fn insert_module_state_if_changed(
    modules: &mut std::collections::HashMap<ModuleId, ModuleState>,
    id: ModuleId,
    state: ModuleState,
) {
    if modules.get(&id) != Some(&state) {
        modules.insert(id, state);
    }
}
```

Use it for clock, script, and button state writes.

In `src/app/module_state.rs`, add:

```rust
fn push_state_if_changed(&mut self, id: crate::modules::ModuleId, state: ModuleState) {
    if self.shared.modules.get(&id) != Some(&state) {
        self.shared.modules.insert(id, state);
    }
}
```

Use it in battery, network, volume, and focused-window push methods.

- [ ] **Step 4: Verify**

Run: `rtk cargo test repeated_clock_tick_keeps_existing_state repeated_volume_state_keeps_existing_state --lib`
Expected: PASS.

## Task 3: Cursor Message Filtering

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write failing tests**

Add tests in `src/app.rs` test module:

```rust
#[test]
fn cursor_moved_is_ignored_without_tray_module() {
    let (mut app, wid) = make_app();
    let _ = app.update(Message::CursorMoved(wid, iced::Point::new(10.0, 20.0)));
    assert_eq!(app.last_cursor, None);
}

#[test]
fn cursor_moved_is_recorded_when_tray_module_exists() {
    let (mut app, wid) = make_app();
    app.modules.push((
        Zone::Top,
        Box::new(crate::modules::tray::TrayModule::new(ModuleId::new("tray"))),
    ));
    let point = iced::Point::new(10.0, 20.0);
    let _ = app.update(Message::CursorMoved(wid, point));
    assert_eq!(app.last_cursor, Some((wid, point)));
}
```

Run: `rtk cargo test cursor_moved_is_ignored_without_tray_module cursor_moved_is_recorded_when_tray_module_exists --lib`
Expected: first test FAILS because all cursor moves are currently stored.

- [ ] **Step 2: Implement tray-aware dedupe**

In `Message::CursorMoved`, replace the unconditional assignment with:

```rust
let has_tray = self
    .modules
    .iter()
    .any(|(_, module)| matches!(module.module_type(), "tray" | "tray_icon"));
if has_tray && self.last_cursor != Some((window, pos)) {
    self.last_cursor = Some((window, pos));
}
Task::none()
```

- [ ] **Step 3: Verify**

Run: `rtk cargo test cursor_moved_is_ignored_without_tray_module cursor_moved_is_recorded_when_tray_module_exists --lib`
Expected: PASS.

## Task 4: Workspace Rebuild Skip

**Files:**
- Modify: `src/app/niri.rs`

- [ ] **Step 1: Write failing test**

Add a test in `src/app.rs` test module:

```rust
#[test]
fn equivalent_workspace_event_keeps_module_count() {
    let (mut app, _wid) = make_app();
    let workspaces = vec![workspace(1, 1, Some("eDP-1"), true, true)];
    let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
        workspaces: workspaces.clone(),
    })));
    let count = app.modules.len();
    let _ = app.update(Message::Niri(NiriEvent::Event(Event::WorkspacesChanged {
        workspaces,
    })));
    assert_eq!(app.modules.len(), count);
}
```

Run: `rtk cargo test equivalent_workspace_event_keeps_module_count --lib`
Expected: PASS may already hold for count, but implementation still rebuilds. If count passes, add a lightweight helper test for a new `workspace_lists_equivalent` function.

- [ ] **Step 2: Implement equivalence helper**

In `src/app/niri.rs`, add a private helper comparing fields that affect workspace rendering:

```rust
fn workspace_render_state_equal(a: &[niri_ipc::Workspace], b: &[niri_ipc::Workspace]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(left, right)| {
            left.id == right.id
                && left.idx == right.idx
                && left.name == right.name
                && left.output == right.output
                && left.is_active == right.is_active
                && left.is_focused == right.is_focused
        })
}
```

In `Event::WorkspacesChanged`, only call `rebuild_workspace_item_modules` when this helper returns
false for current state vs incoming workspaces.

- [ ] **Step 3: Verify**

Run: `rtk cargo test equivalent_workspace_event_keeps_module_count --lib`
Expected: PASS.

## Task 5: Full Verification

**Files:**
- No new files.

- [ ] **Step 1: Run formatting**

Run: `rtk cargo fmt --check`
Expected: PASS.

- [ ] **Step 2: Run tests**

Run: `rtk cargo test --lib`
Expected: PASS.

- [ ] **Step 3: Review diff**

Run: `rtk git diff -- src/niri/subscription.rs src/ipc.rs src/modules.rs src/modules src/app.rs src/app/module_events.rs src/app/module_state.rs src/app/niri.rs`
Expected: Only targeted performance changes and tests.
