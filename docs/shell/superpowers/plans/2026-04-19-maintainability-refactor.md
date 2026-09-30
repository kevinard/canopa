# Maintainability Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate remaining duplication and shrink the non-`app` god-modules (`config/parse.rs`, `dbus/sni_host.rs`) while preserving observable behavior.

**Architecture:** Four risk-ordered phases plus a hardening phase. Phases 1–2 extract pure helpers and collapse repeated module rendering. Phases 3–4 split `config/parse.rs` and `dbus/sni_host.rs` into focused modules. The `App` actor split is intentionally deferred to `docs/niribar/superpowers/plans/2026-04-19-app-actor-split.md`; do not start it from this plan.

**Tech Stack:** Rust 2024 edition, `iced` 0.14 with `iced_layershell`, `zbus` 5.x, `kdl` 6.x, `tokio`, `libpulse-binding`, `niri-ipc`.

**Verification gate (every phase and every task that compiles):**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

All three must pass. `cargo build --release` is NOT part of the gate.

---

## Phase 1 — Pure extractions

Eight self-contained extractions. Each is a separate task with its own tests and commit.

### Task 1.1: Generic D-Bus subscription stream builder

**Files:**
- Create: `crates/niribar/src/dbus/subscription.rs`
- Modify: `crates/niribar/src/dbus.rs` (add `pub mod subscription;`)

The three D-Bus modules (`upower.rs`, `networkmanager.rs`, `wireplumber.rs`) each duplicate a `SubState::{Init, Running(rx), Done}` state machine around `stream::unfold`. This task introduces a reusable builder; callers are migrated in Tasks 1.2–1.4.

- [ ] **Step 1: Create the module file**

Write `crates/niribar/src/dbus/subscription.rs`:

```rust
//! Generic builder for D-Bus subscription streams.
//!
//! Each D-Bus subscription in this crate follows the same three-state shape:
//! spawn a task that pushes updates onto an mpsc channel, emit a mapped
//! `Message` for each update, and terminate after the channel closes.
//!
//! This module encapsulates that shape so each D-Bus backend only supplies
//! (a) the task body and (b) the message-mapping closures.

use iced::futures::{stream, Stream};
use std::{pin::Pin, sync::Arc};
use tokio::sync::mpsc;

/// Internal state for the generated stream.
enum SubState<T, M> {
    Init {
        spawn: Option<Box<dyn FnOnce(mpsc::UnboundedSender<Result<T, ()>>) + Send>>,
        on_ok: Arc<dyn Fn(T) -> M + Send + Sync>,
        on_err: Arc<dyn Fn() -> M + Send + Sync>,
    },
    Running {
        rx: mpsc::UnboundedReceiver<Result<T, ()>>,
        on_ok: Arc<dyn Fn(T) -> M + Send + Sync>,
        on_err: Arc<dyn Fn() -> M + Send + Sync>,
    },
    Done,
}

/// Build a boxed iced subscription stream from:
/// - `spawn`: starts the backend task, taking the `tx` side of the channel.
/// - `on_ok`: maps a successful update into the outgoing `Message`.
/// - `on_err`: produces the single "unavailable" message emitted before termination.
///
/// The returned stream emits one `Message` per update, followed by a single
/// `on_err` message if the backend task exits or the channel closes.
pub fn build<T, M, Spawn, OnOk, OnErr>(
    spawn: Spawn,
    on_ok: OnOk,
    on_err: OnErr,
) -> Pin<Box<dyn Stream<Item = M> + Send + 'static>>
where
    T: Send + 'static,
    M: Send + 'static,
    Spawn: FnOnce(mpsc::UnboundedSender<Result<T, ()>>) + Send + 'static,
    OnOk: Fn(T) -> M + Send + Sync + 'static,
    OnErr: Fn() -> M + Send + Sync + 'static,
{
    let init = SubState::Init {
        spawn: Some(Box::new(spawn)),
        on_ok: Arc::new(on_ok),
        on_err: Arc::new(on_err),
    };

    Box::pin(stream::unfold(init, |state| async move {
        match state {
            SubState::Init {
                spawn,
                on_ok,
                on_err,
            } => {
                let (tx, mut rx) = mpsc::unbounded_channel();
                if let Some(f) = spawn {
                    f(tx);
                }
                match rx.recv().await {
                    Some(Ok(value)) => {
                        let msg = on_ok(value);
                        Some((msg, SubState::Running { rx, on_ok, on_err }))
                    }
                    _ => Some((on_err(), SubState::Done)),
                }
            }
            SubState::Running {
                mut rx,
                on_ok,
                on_err,
            } => match rx.recv().await {
                Some(Ok(value)) => {
                    let msg = on_ok(value);
                    Some((msg, SubState::Running { rx, on_ok, on_err }))
                }
                _ => Some((on_err(), SubState::Done)),
            },
            SubState::Done => None,
        }
    }))
}
```

Note: `on_ok` and `on_err` are stored in `Arc` inside the unfold state. Do not capture references to those closures inside the async block; that can produce futures borrowing from the unfold closure environment and fail to compile.

- [ ] **Step 2: Export from `crates/niribar/src/dbus.rs`**

Add at the top of `crates/niribar/src/dbus.rs`:

```rust
pub mod subscription;
```

- [ ] **Step 3: Write a unit test that proves the stream produces the mapped messages**

Append to `crates/niribar/src/dbus/subscription.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use iced::futures::StreamExt;

    #[tokio::test(flavor = "current_thread")]
    async fn emits_ok_messages_then_terminates_with_err() {
        let mut stream = build::<u32, &'static str, _, _, _>(
            |tx| {
                tokio::spawn(async move {
                    let _ = tx.send(Ok(1));
                    let _ = tx.send(Ok(2));
                    // dropping tx closes the channel and triggers on_err
                });
            },
            |n| match n {
                1 => "one",
                2 => "two",
                _ => "other",
            },
            || "unavailable",
        );
        assert_eq!(stream.next().await, Some("one"));
        assert_eq!(stream.next().await, Some("two"));
        assert_eq!(stream.next().await, Some("unavailable"));
        assert_eq!(stream.next().await, None);
    }
}
```

- [ ] **Step 4: Verify**

```bash
rtk cargo test dbus::subscription
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

Expected: all three pass; the new test is green.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/dbus.rs crates/niribar/src/dbus/subscription.rs
rtk git commit -m "refactor(dbus): add generic subscription stream builder"
```

---

### Task 1.2: Migrate `dbus/upower.rs` onto the generic builder

**Files:**
- Modify: `crates/niribar/src/dbus/upower.rs`

- [ ] **Step 1: Replace `SubState` and `build_battery_stream`**

Delete the existing three-variant `SubState` enum at lines ~43–47 and rewrite `build_battery_stream` to use the shared builder. Replace lines 49–94 with:

```rust
fn build_battery_stream(
    data: &BatteryConnData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>> {
    let conn = data.0.clone();
    crate::dbus::subscription::build::<(f64, bool), crate::app::Message, _, _, _>(
        move |tx| {
            tokio::spawn(async move {
                let _ = run_upower_task(conn, tx).await;
            });
        },
        |(pct, charging)| {
            crate::app::Message::Battery(BatteryEvent::StateChanged {
                percentage: pct,
                is_charging: charging,
            })
        },
        || crate::app::Message::Battery(BatteryEvent::Unavailable),
    )
}
```

Remove the `use iced::futures::stream;` line at the top since it's no longer used.

- [ ] **Step 2: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

Expected: all tests still pass (the existing `upower_state_is_charging` tests exercise the pure logic; the subscription wiring is covered by Task 1.1's test plus the unchanged end-to-end behavior).

- [ ] **Step 3: Commit**

```bash
rtk git add crates/niribar/src/dbus/upower.rs
rtk git commit -m "refactor(dbus): migrate upower onto generic subscription builder"
```

---

### Task 1.3: Migrate `dbus/networkmanager.rs` onto the generic builder

**Files:**
- Modify: `crates/niribar/src/dbus/networkmanager.rs`

- [ ] **Step 1: Replace `SubState` and `build_network_stream`**

Delete the existing three-variant `SubState` enum at lines ~78–82 and replace `build_network_stream` (lines 84–129) with:

```rust
fn build_network_stream(
    data: &NetworkConnData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>> {
    let conn = data.0.clone();
    crate::dbus::subscription::build::<(NetworkConnectivity, Option<String>), crate::app::Message, _, _, _>(
        move |tx| {
            tokio::spawn(async move {
                let _ = run_network_task(conn, tx).await;
            });
        },
        |(connectivity, label)| {
            crate::app::Message::Network(NetworkEvent::StateChanged { connectivity, label })
        },
        || crate::app::Message::Network(NetworkEvent::Unavailable),
    )
}
```

Remove `use iced::futures::stream;` at the top.

- [ ] **Step 2: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 3: Commit**

```bash
rtk git add crates/niribar/src/dbus/networkmanager.rs
rtk git commit -m "refactor(dbus): migrate networkmanager onto generic subscription builder"
```

---

### Task 1.4: Migrate `dbus/wireplumber.rs` onto the generic builder

**Files:**
- Modify: `crates/niribar/src/dbus/wireplumber.rs`

- [ ] **Step 1: Replace `SubState` and `build_volume_stream`**

Delete the existing three-variant `SubState` enum at lines ~13–17 and replace `build_volume_stream` (lines 19–58) with:

```rust
fn build_volume_stream(
    _data: &VolumeConnData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>> {
    crate::dbus::subscription::build::<(f64, bool), crate::app::Message, _, _, _>(
        |tx| {
            std::thread::spawn(move || run_pa_thread(tx));
        },
        |(volume, muted)| crate::app::Message::Volume(VolumeEvent::StateChanged { volume, muted }),
        || crate::app::Message::Volume(VolumeEvent::Unavailable),
    )
}
```

Remove `use iced::futures::stream;` at the top.

- [ ] **Step 2: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 3: Commit**

```bash
rtk git add crates/niribar/src/dbus/wireplumber.rs
rtk git commit -m "refactor(dbus): migrate wireplumber onto generic subscription builder"
```

---

### Task 1.5: `find_tray_slots()` helper

**Files:**
- Modify: `crates/niribar/src/app/tray.rs`

- [ ] **Step 1: Add the helper inside the `impl App` block**

Insert at the top of `impl App` in `crates/niribar/src/app/tray.rs`:

```rust
    fn find_tray_slots(&self) -> Vec<(usize, crate::modules::Zone, String)> {
        self.modules
            .iter()
            .enumerate()
            .filter(|(_, (_, m))| m.kind() == ModuleKind::Tray)
            .map(|(i, (z, m))| (i, z.clone(), m.id().0.clone()))
            .collect()
    }
```

- [ ] **Step 2: Replace both duplicated blocks**

In `handle_tray_event`, replace the full `let tray_slots: Vec<(usize, crate::modules::Zone, String)> = self.modules.iter().enumerate().filter(|(_, (_, m))| m.kind() == ModuleKind::Tray).map(|(i, (z, m))| (i, z.clone(), m.id().0.clone())).collect();` block in the `TrayEvent::Snapshot` arm (current lines 24–30) with:

```rust
                let tray_slots = self.find_tray_slots();
```

Do the same in the `TrayEvent::ItemAdded` arm (current lines 51–57).

- [ ] **Step 3: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 4: Commit**

```bash
rtk git add crates/niribar/src/app/tray.rs
rtk git commit -m "refactor(app): extract find_tray_slots helper"
```

---

### Task 1.6: Conditional D-Bus subscription helper

**Files:**
- Modify: `crates/niribar/src/app/subscriptions.rs`

The current `subscription()` repeats the same enabled-module-plus-optional-connection branch three times (battery/network/tray) and once for volume (no conn). This task collapses the D-Bus connection branches.

- [ ] **Step 1: Add the helper inside the `impl App` block**

At the top of `crates/niribar/src/app/subscriptions.rs`, above `impl App`:

```rust
fn dbus_subscription<F>(
    enabled: bool,
    conn: Option<zbus::Connection>,
    build: F,
) -> iced::Subscription<Message>
where
    F: FnOnce(zbus::Connection) -> iced::Subscription<Message>,
{
    match (enabled, conn) {
        (true, Some(c)) => build(c),
        _ => iced::Subscription::none(),
    }
}
```

- [ ] **Step 2: Replace the three D-Bus conditional blocks**

Replace the `battery_sub`, `network_sub`, and `sni_sub` blocks (current lines 29–47 and 55–63) with:

```rust
        let battery_sub = dbus_subscription(
            has_battery,
            self.shared.dbus.system.clone(),
            crate::dbus::upower::battery_subscription,
        );

        let network_sub = dbus_subscription(
            has_network,
            self.shared.dbus.system.clone(),
            crate::dbus::networkmanager::network_subscription,
        );

        let volume_sub = if has_volume {
            crate::dbus::wireplumber::volume_subscription()
        } else {
            iced::Subscription::none()
        };

        let sni_sub = dbus_subscription(
            has_tray,
            self.shared.dbus.session.clone(),
            crate::dbus::sni_host::sni_subscription,
        );
```

Leave `volume_sub` as a plain `if` since it takes no connection.

- [ ] **Step 3: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 4: Commit**

```bash
rtk git add crates/niribar/src/app/subscriptions.rs
rtk git commit -m "refactor(app): collapse conditional dbus subscription blocks"
```

---

### Task 1.7: `bar_for_window` helper and `animation.rs` cleanup

**Files:**
- Modify: `crates/niribar/src/app/animation.rs`

- [ ] **Step 1: Add the helper inside the `impl App` block**

At the top of the `impl App` block in `crates/niribar/src/app/animation.rs`:

```rust
    fn bar_for_window(&self, window: iced::window::Id) -> &crate::config::BarConfig {
        let bar_name = self
            .surface(window)
            .map(|s| s.bar_name.clone())
            .unwrap_or_default();
        Self::bar_for_surface_name(&self.config.bars, &bar_name)
    }
```

- [ ] **Step 2: Replace the repeated two-line lookups**

In `handle_bar_click`, replace lines 12–16 with:

```rust
        let bar = self.bar_for_window(window);
```

In `handle_bar_hover_enter`, replace lines 26–30 with:

```rust
        let bar = self.bar_for_window(window);
```

In `handle_bar_hover_exit`, replace lines 61–65 with:

```rust
        let bar = self.bar_for_window(window);
```

In `handle_tick`, inside the `for wid in window_ids` loop, replace the 6-line `bar_name` + `effective_bar` computation (lines 92–97) with:

```rust
            let effective_bar = self.bar_for_window(wid);
```

In `handle_toggle_expand`, replace the `bar_name` lookup (lines 145–150) with:

```rust
            let expand_enabled = self.bar_for_window(wid).expand;
```

- [ ] **Step 3: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 4: Commit**

```bash
rtk git add crates/niribar/src/app/animation.rs
rtk git commit -m "refactor(app): extract bar_for_window helper"
```

---

### Task 1.8: Error-path helper for config parsing

**Files:**
- Create: `src/config/parse_error.rs`
- Modify: `src/config/parse.rs`, `crates/niribar/src/config.rs`

Note: during Phase 3 this file will move into `crates/niribar/src/config/parse/error.rs`; for Phase 1 we keep `config/parse.rs` monolithic and add the helper beside it.

- [ ] **Step 1: Create `src/config/parse_error.rs`**

```rust
//! Shared error-path construction for config parsing.
//!
//! Most parsing errors in `config/parse.rs` start with
//! `modules[{idx}].{module}.{field}`. Funneling that prefix through one helper
//! lets us change the wording in a single place and keeps error paths uniform.

/// Build an error-path prefix for a module field.
#[inline]
pub(crate) fn field_path(idx: usize, module: &str, field: &str) -> String {
    format!("modules[{idx}].{module}.{field}")
}

/// Build an error-path prefix for a module itself (no field).
#[inline]
pub(crate) fn module_path(idx: usize, module: &str) -> String {
    format!("modules[{idx}].{module}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_path_formats_correctly() {
        assert_eq!(field_path(2, "battery", "warning"), "modules[2].battery.warning");
    }

    #[test]
    fn module_path_formats_correctly() {
        assert_eq!(module_path(0, "clock"), "modules[0].clock");
    }
}
```

- [ ] **Step 2: Wire the module into `crates/niribar/src/config.rs`**

Add near the top of `crates/niribar/src/config.rs` (next to `pub mod parse;`):

```rust
pub(crate) mod parse_error;
```

- [ ] **Step 3: Migrate `config/parse.rs` call sites**

Replace every occurrence of the literal `format!("modules[{idx}].{module}.{field}")` or equivalent inline string construction in `src/config/parse.rs` with `crate::config::parse_error::field_path(idx, module, field)`. Likewise for `module_path` where only the module prefix is used.

This is mechanical. Work through the file top-to-bottom. Do NOT alter the error message that follows the path — behavior parity is required (Q6 "behavior parity, implementation freedom" permits message changes, but this task preserves them for easier diff review; any intentional wording change lands in a later task with its own PR).

- [ ] **Step 4: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

All existing config-parser tests must still pass; no test assertions should need updating.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/config.rs src/config/parse.rs src/config/parse_error.rs
rtk git commit -m "refactor(config): extract field_path helper for parse errors"
```

---

## Phase 2 — Icon-label module view helpers

`network` and `volume` use the same collapsed icon plus expanded/horizontal icon-label pattern. `battery` shares the expanded/horizontal path but keeps its custom collapsed critical badge. `clock` and `button` look similar at first glance, but their collapsed/expanded layouts are intentionally different, so this phase leaves them custom to preserve behavior.

### Task 2.1: Add `IconLabel` helper trait and renderer

**Files:**
- Modify: `crates/niribar/src/modules/view.rs`

- [ ] **Step 1: Add the helper trait and layout enum**

Append to `crates/niribar/src/modules/view.rs` after the existing `use crate::app::Message;` line:

```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IconLabelLayout {
    Collapsed,
    Expanded,
    Horizontal,
}

/// State-derived data for modules that render as an icon plus a short label.
///
/// Modules that need extra rows, overlay badges, focus handling, or custom
/// click targets keep their bespoke `Module` view methods instead of using this.
pub trait IconLabel {
    fn icon(&self, state: &super::ModuleState) -> String;
    fn label(&self, state: &super::ModuleState) -> String;

    fn color(&self, _state: &super::ModuleState) -> iced::Color {
        crate::theme::TEXT
    }
}
```

- [ ] **Step 2: Add the renderer that matches the existing helper signatures**

Append this below the existing `expanded_icon_label` function in `crates/niribar/src/modules/view.rs`:

```rust
pub fn icon_label_view<'a, M>(
    module: &'a M,
    state: &'a super::ModuleState,
    layout: IconLabelLayout,
) -> Element<'a, Message>
where
    M: IconLabel,
{
    match layout {
        IconLabelLayout::Collapsed => collapsed_icon(module.icon(state), module.color(state)),
        IconLabelLayout::Expanded => {
            expanded_icon_label(module.icon(state), module.color(state), module.label(state))
        }
        IconLabelLayout::Horizontal => {
            horizontal_icon_label(module.icon(state), module.color(state), module.label(state))
        }
    }
}
```

This code deliberately calls the real helper signatures in `crates/niribar/src/modules/view.rs`: `collapsed_icon(icon, color)`, `expanded_icon_label(icon, icon_color, label)`, and `horizontal_icon_label(icon, icon_color, label)`.

- [ ] **Step 3: Verify compile**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 4: Commit**

```bash
rtk git add crates/niribar/src/modules/view.rs
rtk git commit -m "refactor(modules): add icon-label view helper"
```

---

### Task 2.2: Migrate `network` to `IconLabel`

**Files:**
- Modify: `crates/niribar/src/modules/network.rs`

- [ ] **Step 1: Add the trait implementation**

Add this above the existing `impl crate::modules::Module for NetworkModule` block:

```rust
impl crate::modules::view::IconLabel for NetworkModule {
    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Network(s) => icon_str(&s.connectivity).to_owned(),
            _ => String::new(),
        }
    }

    fn label(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Network(s) => s.label.clone().unwrap_or_else(|| {
                match &s.connectivity {
                    NetworkConnectivity::Full => "online",
                    NetworkConnectivity::Limited => "limited",
                    NetworkConnectivity::None => "offline",
                }
                .to_owned()
            }),
            _ => String::new(),
        }
    }

    fn color(&self, state: &crate::modules::ModuleState) -> iced::Color {
        match state {
            crate::modules::ModuleState::Network(s) => icon_color(&s.connectivity),
            _ => crate::theme::TEXT,
        }
    }
}
```

- [ ] **Step 2: Replace `view_*` bodies with one-liners**

Replace `view_collapsed`, `view_expanded`, and `view_horizontal` in `crates/niribar/src/modules/network.rs` with:

```rust
fn view_collapsed(
    &self,
    state: &crate::modules::ModuleState,
) -> iced::Element<'_, crate::app::Message> {
    match state {
        crate::modules::ModuleState::Network(_) => crate::modules::view::icon_label_view(
            self,
            state,
            crate::modules::view::IconLabelLayout::Collapsed,
        ),
        _ => iced::widget::Space::new().into(),
    }
}

fn view_expanded(
    &self,
    state: &crate::modules::ModuleState,
) -> iced::Element<'_, crate::app::Message> {
    match state {
        crate::modules::ModuleState::Network(_) => crate::modules::view::icon_label_view(
            self,
            state,
            crate::modules::view::IconLabelLayout::Expanded,
        ),
        _ => iced::widget::Space::new().into(),
    }
}

fn view_horizontal(
    &self,
    state: &crate::modules::ModuleState,
) -> iced::Element<'_, crate::app::Message> {
    match state {
        crate::modules::ModuleState::Network(_) => crate::modules::view::icon_label_view(
            self,
            state,
            crate::modules::view::IconLabelLayout::Horizontal,
        ),
        _ => iced::widget::Space::new().into(),
    }
}
```

Remove the now-unused `use crate::modules::view::{collapsed_icon, expanded_icon_label, horizontal_icon_label};` import and replace it with any narrower import still needed by the file.

- [ ] **Step 3: Verify**

```bash
rtk cargo test -p niribar --lib -- modules::network
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 4: Commit**

```bash
rtk git add crates/niribar/src/modules/network.rs
rtk git commit -m "refactor(network): render through IconLabel helper"
```

---

### Task 2.3: Migrate `volume` to `IconLabel`

**Files:**
- Modify: `crates/niribar/src/modules/volume.rs`

- [ ] **Step 1: Add the trait implementation**

Add this above the existing `impl crate::modules::Module for VolumeModule` block:

```rust
impl crate::modules::view::IconLabel for VolumeModule {
    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Volume(s) => {
                tier_icon(volume_tier(s.volume, s.muted)).to_owned()
            }
            _ => String::new(),
        }
    }

    fn label(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Volume(s) if s.muted => "muted".to_owned(),
            crate::modules::ModuleState::Volume(s) => format!("{}%", s.volume.round() as u64),
            _ => String::new(),
        }
    }

    fn color(&self, state: &crate::modules::ModuleState) -> iced::Color {
        match state {
            crate::modules::ModuleState::Volume(s) => tier_color(volume_tier(s.volume, s.muted)),
            _ => crate::theme::TEXT,
        }
    }
}
```

- [ ] **Step 2: Replace `view_*` bodies with one-liners**

Replace `view_collapsed`, `view_expanded`, and `view_horizontal` in `crates/niribar/src/modules/volume.rs` with:

```rust
fn view_collapsed(
    &self,
    state: &crate::modules::ModuleState,
) -> iced::Element<'_, crate::app::Message> {
    match state {
        crate::modules::ModuleState::Volume(_) => crate::modules::view::icon_label_view(
            self,
            state,
            crate::modules::view::IconLabelLayout::Collapsed,
        ),
        _ => iced::widget::Space::new().into(),
    }
}

fn view_expanded(
    &self,
    state: &crate::modules::ModuleState,
) -> iced::Element<'_, crate::app::Message> {
    match state {
        crate::modules::ModuleState::Volume(_) => crate::modules::view::icon_label_view(
            self,
            state,
            crate::modules::view::IconLabelLayout::Expanded,
        ),
        _ => iced::widget::Space::new().into(),
    }
}

fn view_horizontal(
    &self,
    state: &crate::modules::ModuleState,
) -> iced::Element<'_, crate::app::Message> {
    match state {
        crate::modules::ModuleState::Volume(_) => crate::modules::view::icon_label_view(
            self,
            state,
            crate::modules::view::IconLabelLayout::Horizontal,
        ),
        _ => iced::widget::Space::new().into(),
    }
}
```

- [ ] **Step 3: Verify and commit**

```bash
rtk cargo test -p niribar --lib -- modules::volume
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
rtk git add crates/niribar/src/modules/volume.rs
rtk git commit -m "refactor(volume): render through IconLabel helper"
```

---

### Task 2.4: Migrate battery expanded and horizontal views to `IconLabel`

**Files:**
- Modify: `crates/niribar/src/modules/battery.rs`

Battery keeps its custom collapsed view because the critical-percent badge is behavior, not duplication.

- [ ] **Step 1: Add the trait implementation**

Add this above the existing `impl crate::modules::Module for BatteryModule` block:

```rust
impl crate::modules::view::IconLabel for BatteryModule {
    fn icon(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Battery(s) if s.is_charging => "⚡".to_owned(),
            crate::modules::ModuleState::Battery(s) => {
                tier_icon(charge_tier(s.percentage, self.critical_threshold)).to_owned()
            }
            _ => String::new(),
        }
    }

    fn label(&self, state: &crate::modules::ModuleState) -> String {
        match state {
            crate::modules::ModuleState::Battery(s) => format!("{}%", s.percentage.round() as u8),
            _ => String::new(),
        }
    }

    fn color(&self, state: &crate::modules::ModuleState) -> iced::Color {
        match state {
            crate::modules::ModuleState::Battery(s) => {
                icon_color(s.percentage, s.is_charging, self.critical_threshold)
            }
            _ => crate::theme::TEXT,
        }
    }
}
```

- [ ] **Step 2: Replace only expanded and horizontal views**

Leave `view_collapsed` unchanged. Replace `view_expanded` with:

```rust
fn view_expanded(
    &self,
    state: &crate::modules::ModuleState,
) -> iced::Element<'_, crate::app::Message> {
    match state {
        crate::modules::ModuleState::Battery(_) => crate::modules::view::icon_label_view(
            self,
            state,
            crate::modules::view::IconLabelLayout::Expanded,
        ),
        _ => iced::widget::Space::new().into(),
    }
}
```

Replace `view_horizontal` with:

```rust
fn view_horizontal(
    &self,
    state: &crate::modules::ModuleState,
) -> iced::Element<'_, crate::app::Message> {
    match state {
        crate::modules::ModuleState::Battery(_) => crate::modules::view::icon_label_view(
            self,
            state,
            crate::modules::view::IconLabelLayout::Horizontal,
        ),
        _ => iced::widget::Space::new().into(),
    }
}
```

Remove `expanded_icon_label` and `horizontal_icon_label` from the top-level `use crate::modules::view::{collapsed_icon, expanded_icon_label, horizontal_icon_label};` import, keeping only imports still needed by `view_collapsed`.

- [ ] **Step 3: Verify and commit**

```bash
rtk cargo test -p niribar --lib -- modules::battery
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
rtk git add crates/niribar/src/modules/battery.rs
rtk git commit -m "refactor(battery): share icon-label expanded rendering"
```

- [ ] **Step 4: Record the intentional skips**

Add a short comment above `ClockModule::view_collapsed` in `crates/niribar/src/modules/clock.rs`:

```rust
// Clock keeps custom views: collapsed renders icon + time in a column and
// expanded renders time plus a second date row.
```

Add a short comment above `ButtonModule::view_collapsed` in `crates/niribar/src/modules/button.rs`:

```rust
// Button keeps custom collapsed/expanded views because its expanded layout
// right-aligns the label with a spacer instead of using the shared icon-label row.
```

Verify and commit:

```bash
rtk cargo test -p niribar --lib -- modules::clock
rtk cargo test -p niribar --lib -- modules::button
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
rtk git add crates/niribar/src/modules/clock.rs crates/niribar/src/modules/button.rs
rtk git commit -m "docs(modules): document custom clock and button view layouts"
```

---

## Phase 3 — `config/parse.rs` split

### Task 3.1: Create the `config/parse/` directory and move the existing file

**Files:**
- Rename: `src/config/parse.rs` → `crates/niribar/src/config/parse/mod.rs`
- Rename: `src/config/parse_error.rs` → `crates/niribar/src/config/parse/error.rs`

- [ ] **Step 1: Move the files with git mv**

```bash
rtk mkdir -p crates/niribar/src/config/parse
rtk git mv src/config/parse.rs crates/niribar/src/config/parse/mod.rs
rtk git mv src/config/parse_error.rs crates/niribar/src/config/parse/error.rs
```

- [ ] **Step 2: Update the module declaration in `crates/niribar/src/config/parse/mod.rs`**

The file's existing grouped imports from the parent module still refer to the parent module correctly because `mod.rs` in `config/parse/` has the same `super::` semantics as the old `config/parse.rs`. But the internal `pub(crate) mod parse_error;` declaration (added in Task 1.8) must change: in `crates/niribar/src/config.rs`, replace:

```rust
pub(crate) mod parse_error;
```

with nothing (the helper now lives under `parse::error`), and in `crates/niribar/src/config/parse/mod.rs` add at the top:

```rust
mod error;
```

Update all call sites in `mod.rs` that referenced `crate::config::parse_error::field_path` to `self::error::field_path` (or just `error::field_path` after the `mod error;`).

- [ ] **Step 3: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 4: Commit**

```bash
rtk git commit -m "refactor(config): move parse.rs into parse/ module"
```

---

### Task 3.2: Extract KDL helpers into `parse/kdl.rs`

**Files:**
- Create: `src/config/parse/kdl.rs`
- Modify: `crates/niribar/src/config/parse/mod.rs`

- [ ] **Step 1: Move the helpers**

Cut `first_child_value`, `required_string_child`, `optional_nonempty_string_child`, and `optional_u8_range_child` (current lines 9–73 of `mod.rs`) into a new `src/config/parse/kdl.rs`. Mark them `pub(super)` so the parent module and sibling module files can call them.

- [ ] **Step 2: Wire the module**

At the top of `crates/niribar/src/config/parse/mod.rs`:

```rust
mod kdl;
```

Update the call sites in `mod.rs` for `first_child_value`, `required_string_child`, `optional_nonempty_string_child`, and `optional_u8_range_child` to qualify through `kdl::`.

- [ ] **Step 3: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 4: Commit**

```bash
rtk git add crates/niribar/src/config/parse/
rtk git commit -m "refactor(config): extract KDL helpers into parse::kdl"
```

---

### Task 3.3: Split per-module parsers into `parse/modules/<name>.rs`

**Files:**
- Create: `crates/niribar/src/config/parse/modules/mod.rs`, plus one file per module parser: `battery.rs`, `clock.rs`, `network.rs`, `volume.rs`, `script.rs`, `button.rs`, `workspaces.rs`, `focused_window.rs`, `tray.rs`
- Modify: `crates/niribar/src/config/parse/mod.rs`

- [ ] **Step 1: Create `parse/modules/mod.rs` with the dispatch**

```rust
//! Per-module parameter parsers and dispatch.
//!
//! Each module kind has its own `parse_<kind>_params` function in a sibling
//! file. Config-level parsing in `super::mod` calls into `dispatch` to convert
//! a raw KDL node into the typed `ModuleParams` for a known `ModuleKind`.

use crate::config::ModuleParams;
use crate::modules::ModuleKind;

mod battery;
mod button;
mod clock;
mod focused_window;
mod network;
mod script;
mod tray;
mod volume;
mod workspaces;

pub(super) fn dispatch(
    kind: ModuleKind,
    node: &kdl::KdlNode,
    idx: usize,
) -> anyhow::Result<ModuleParams> {
    match kind {
        ModuleKind::Battery => battery::parse(node, idx).map(ModuleParams::Battery),
        ModuleKind::Clock => clock::parse(node, idx).map(ModuleParams::Clock),
        ModuleKind::Network => network::parse(node, idx).map(ModuleParams::Network),
        ModuleKind::Volume => volume::parse(node, idx).map(ModuleParams::Volume),
        ModuleKind::Script => script::parse(node, idx).map(ModuleParams::Script),
        ModuleKind::Button => button::parse(node, idx).map(ModuleParams::Button),
        ModuleKind::Workspaces => workspaces::parse(node, idx).map(ModuleParams::Workspaces),
        ModuleKind::FocusedWindow => focused_window::parse(node, idx).map(ModuleParams::FocusedWindow),
        ModuleKind::Tray => tray::parse(node, idx).map(ModuleParams::Tray),
        // Synthetic kinds (WorkspaceItem, TrayIcon) are never produced by the parser.
        ModuleKind::WorkspaceItem | ModuleKind::TrayIcon => {
            anyhow::bail!(
                "{}: internal error — {:?} is runtime-only and cannot appear in config",
                super::error::module_path(idx, kind.as_str()),
                kind
            )
        }
    }
}
```

- [ ] **Step 2: One commit per module parser extraction**

For each of `battery`, `clock`, `network`, `volume`, `script`, `button`, `workspaces`, `focused_window`, `tray`:

1. Cut the existing `parse_<kind>_params` (or whatever it's named) out of `parse/mod.rs`.
2. Paste into `crates/niribar/src/config/parse/modules/<kind>.rs` renamed to `pub(super) fn parse(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<<KindConfig>>`.
3. Update internal references to `kdl::required_string_child`, `kdl::optional_nonempty_string_child`, and `kdl::optional_u8_range_child` to use `super::super::kdl`, or add `use super::super::kdl;` at the top.
4. Update references to `error::field_path` and `error::module_path` to use `super::super::error`, or add `use super::super::error;` at the top.
5. If the parser has unit tests, move them into a `#[cfg(test)] mod tests` at the bottom of the new file.
6. Run the verification gate.
7. Commit with `refactor(config): split <kind> parser into parse/modules/<kind>.rs`.

Do NOT batch multiple module extractions into one commit; do them one at a time so a regression points at the exact extraction that broke.

- [ ] **Step 3: Replace the stringly dispatch in `parse/mod.rs`**

Once all nine parsers are moved, replace the current string-comparison dispatch chain for `battery`, `clock`, `network`, `volume`, `script`, `button`, `workspaces`, `focused-window`, and `tray` (around current line 279) with:

```rust
        let kind = ModuleKind::from_config_name(module_type).ok_or_else(|| {
            anyhow::anyhow!(
                "{}: unknown module type {module_type:?}",
                error::module_path(idx, module_type)
            )
        })?;
        let params = modules::dispatch(kind, node, idx)?;
```

Add `mod modules;` at the top of `parse/mod.rs` next to the other `mod` declarations.

- [ ] **Step 4: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

All existing parser tests must still pass; this includes the hundreds of unit tests around error messages.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/config/parse/
rtk git commit -m "refactor(config): typed ModuleKind dispatch replaces stringly if-chain"
```

---

### Task 3.4: Split bar and animation sections

**Files:**
- Create: `crates/niribar/src/config/parse/bars.rs`, `src/config/parse/animation.rs`
- Modify: `crates/niribar/src/config/parse/mod.rs`

- [ ] **Step 1: Move `parse_bar_sections` and helpers**

Move everything under the "bar section" in `parse/mod.rs` (including `parse_bar_sections`, `parse_single_bar`, and whatever supporting helpers exist) into `crates/niribar/src/config/parse/bars.rs`. Expose a single `pub(super) fn parse(doc: &KdlDocument) -> anyhow::Result<Vec<BarConfig>>`.

- [ ] **Step 2: Move animation parsing**

Likewise, move `parse_animation_section` and any supporting helpers into `src/config/parse/animation.rs`. Expose one `pub(super) fn parse(doc: &KdlDocument) -> anyhow::Result<AnimationConfig>` entry point, adjusting the return type to the exact animation config type currently used by `Config`.

- [ ] **Step 3: Update `parse/mod.rs` to delegate**

```rust
mod animation;
mod bars;

pub fn parse(input: &str) -> anyhow::Result<Config> {
    let doc: KdlDocument = input.parse().map_err(|e| anyhow::anyhow!("config: {e}"))?;
    let bars = bars::parse(&doc)?;
    let bars = if bars.is_empty() {
        vec![BarConfig::default()]
    } else {
        bars
    };
    let modules_cfg = parse_modules_section(&doc)?;
    Ok(Config { bars, modules: modules_cfg })
}
```

- [ ] **Step 4: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/config/parse/
rtk git commit -m "refactor(config): split bars and animation parsers into their own files"
```

---

### Task 3.5: Move parser tests to their homes

**Files:**
- Modify: every `src/config/parse/**/*.rs`
- Create: `crates/niribar/src/config/parse/tests.rs` (cross-cutting tests only)

- [ ] **Step 1: Classify each test in `parse/mod.rs`**

Walk through the `#[cfg(test)] mod tests` at the bottom of `parse/mod.rs`. For each test:
- Tests exercising exactly one module parser → move into that module file's `#[cfg(test)] mod tests`.
- Tests that exercise top-level behavior, multi-module configs, or error-surface shape → leave in one file, `crates/niribar/src/config/parse/tests.rs`.

- [ ] **Step 2: Do the moves**

For each moved test, adjust imports (`use super::*;` etc.) to match the new module context. If a test references helpers, use `use super::super::tests::helpers;` or similar.

- [ ] **Step 3: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

All tests — old and newly relocated — pass.

- [ ] **Step 4: Commit**

```bash
rtk git add crates/niribar/src/config/parse/
rtk git commit -m "test(config): relocate parser tests next to their parsers"
```

---

## Phase 4 — `dbus/sni_host.rs` split

### Task 4.1: Create `dbus/sni/` directory skeleton

**Files:**
- Create: `crates/niribar/src/dbus/sni/mod.rs`
- Rename: `src/dbus/sni_host.rs` → `crates/niribar/src/dbus/sni/host.rs`
- Modify: `crates/niribar/src/dbus.rs`

- [ ] **Step 1: Move the file and create the module root**

```bash
rtk mkdir -p crates/niribar/src/dbus/sni
rtk git mv src/dbus/sni_host.rs crates/niribar/src/dbus/sni/host.rs
```

Create `crates/niribar/src/dbus/sni/mod.rs`:

```rust
//! StatusNotifierItem (SNI) tray protocol implementation.
//!
//! Split into:
//! - `host`: SNI bus ownership and item lifecycle
//! - `subscription`: iced subscription wiring (built on `crate::dbus::subscription`)
//! - `pixmap`: pixmap validation, size/byte bounds, ARGB→RGBA conversion
//! - `menu`: DBusMenu fetch and bounds
//! - `proxy`: zbus proxy trait definitions

pub mod host;
// Created in later tasks:
// pub mod menu;
// pub mod pixmap;
// pub mod proxy;
// pub mod subscription;

// Re-export the public surface so existing imports keep working.
pub use host::{TrayEvent, TrayItem, ItemStatus, sni_subscription, menu_layout_subscription};
```

- [ ] **Step 2: Update `crates/niribar/src/dbus.rs`**

Replace:

```rust
pub mod sni_host;
```

with:

```rust
pub mod sni;
```

Then update every call site across the crate that used `crate::dbus::sni_host::X` to `crate::dbus::sni::X`. Run `rtk cargo build` after each small batch to catch missed ones.

- [ ] **Step 3: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "refactor(dbus): move sni_host into sni/ module"
```

---

### Task 4.2: Extract zbus proxy trait definitions into `sni/proxy.rs`

**Files:**
- Create: `crates/niribar/src/dbus/sni/proxy.rs`
- Modify: `crates/niribar/src/dbus/sni/host.rs`, `crates/niribar/src/dbus/sni/mod.rs`

- [ ] **Step 1: Identify and move**

In `sni/host.rs`, find every zbus proxy trait block (`SniItem`, `SniWatcher`, and `DbusMenu`). Cut the full trait blocks into `crates/niribar/src/dbus/sni/proxy.rs`. Re-export the generated proxy types as `pub(crate) use` so `host.rs` can still reach them.

- [ ] **Step 2: Register and import**

In `sni/mod.rs`, add:

```rust
pub(crate) mod proxy;
```

At the top of `sni/host.rs`, add `use super::proxy::*;` (or specific proxies by name).

- [ ] **Step 3: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 4: Commit**

```bash
rtk git add crates/niribar/src/dbus/sni/
rtk git commit -m "refactor(sni): move zbus proxy trait defs into sni::proxy"
```

---

### Task 4.3: Extract pixmap validation/conversion into `sni/pixmap.rs`

**Files:**
- Create: `crates/niribar/src/dbus/sni/pixmap.rs`
- Modify: `crates/niribar/src/dbus/sni/host.rs`, `crates/niribar/src/dbus/sni/mod.rs`

- [ ] **Step 1: Move all pixmap-touching code**

Cut everything in `host.rs` that accepts raw pixmap data from D-Bus, validates dimensions/byte length, caps sizes, and converts ARGB→RGBA, into `crates/niribar/src/dbus/sni/pixmap.rs`. Keep `valid_pixmap`, `retain_pixmaps_with_budget`, `argb_to_rgba`, and `pick_best_pixmap` in this module, with `pub(super)` visibility unless a caller outside `dbus::sni` needs them.

- [ ] **Step 2: Register**

In `sni/mod.rs`:

```rust
pub(super) mod pixmap;
```

- [ ] **Step 3: Move and keep the existing pixmap bounds tests**

Move these existing tests from the bottom of `sni/host.rs` into `sni/pixmap.rs` unchanged except for imports:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_to_rgba_converts_channels() {
        let argb = vec![0xAA, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
        assert_eq!(
            argb_to_rgba(&argb),
            vec![0x11, 0x22, 0x33, 0xAA, 0x55, 0x66, 0x77, 0x44]
        );
    }

    #[test]
    fn valid_pixmap_accepts_matching_byte_len() {
        assert!(valid_pixmap(16, 16, &[0u8; 16 * 16 * 4]));
    }

    #[test]
    fn valid_pixmap_rejects_negative_dimensions() {
        assert!(!valid_pixmap(-1, 16, &[0u8; 16 * 16 * 4]));
    }

    #[test]
    fn valid_pixmap_rejects_mismatched_byte_len() {
        assert!(!valid_pixmap(16, 16, &[0u8; 3]));
    }

    #[test]
    fn valid_pixmap_rejects_zero_dimensions() {
        assert!(!valid_pixmap(0, 16, &[]));
        assert!(!valid_pixmap(16, 0, &[]));
    }

    #[test]
    fn valid_pixmap_rejects_height_out_of_range() {
        let bytes = vec![0u8; (MAX_PIXMAP_DIMENSION as usize + 1) * 16 * 4];
        assert!(!valid_pixmap(16, MAX_PIXMAP_DIMENSION + 1, &bytes));
    }

    #[test]
    fn valid_pixmap_accepts_max_dimension() {
        let dim = MAX_PIXMAP_DIMENSION as usize;
        let bytes = vec![0u8; dim * dim * 4];
        assert!(valid_pixmap(
            MAX_PIXMAP_DIMENSION,
            MAX_PIXMAP_DIMENSION,
            &bytes
        ));
    }

    #[test]
    fn retain_pixmaps_with_budget_skips_invalid_and_limits_count() {
        let pixmaps = vec![
            (1, 1, vec![0u8; 3]),
            (16, 16, vec![0u8; 16 * 16 * 4]),
            (22, 22, vec![0u8; 22 * 22 * 4]),
            (32, 32, vec![0u8; 32 * 32 * 4]),
        ];
        let retained = retain_pixmaps_with_budget(pixmaps, 2, usize::MAX);
        assert_eq!(retained.len(), 2);
        assert_eq!(retained[0].0, 16);
        assert_eq!(retained[1].0, 22);
    }

    #[test]
    fn retain_pixmaps_with_budget_limits_total_bytes() {
        let pixmaps = vec![
            (16, 16, vec![0u8; 16 * 16 * 4]),
            (22, 22, vec![0u8; 22 * 22 * 4]),
            (32, 32, vec![0u8; 32 * 32 * 4]),
        ];
        let retained = retain_pixmaps_with_budget(pixmaps, 8, 16 * 16 * 4 + 22 * 22 * 4);
        assert_eq!(retained.len(), 2);
        assert_eq!(retained[0].0, 16);
        assert_eq!(retained[1].0, 22);
    }
}
```

The implementation entry points in this task should keep the existing names (`valid_pixmap`, `retain_pixmaps_with_budget`, `argb_to_rgba`, `pick_best_pixmap`) unless a rename is necessary for the module boundary. If they are renamed, rename the tests in the same commit.

- [ ] **Step 4: Verify**

```bash
rtk cargo test dbus::sni::pixmap
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 5: Commit**

```bash
rtk git add crates/niribar/src/dbus/sni/
rtk git commit -m "refactor(sni): extract pixmap validation into sni::pixmap"
```

---

### Task 4.4: Extract DBusMenu handling into `sni/menu.rs`

**Files:**
- Create: `crates/niribar/src/dbus/sni/menu.rs`
- Modify: `crates/niribar/src/dbus/sni/host.rs`, `crates/niribar/src/dbus/sni/mod.rs`

- [ ] **Step 1: Move menu parsing and bounds**

Cut DBusMenu fetch, node tree parsing, label length caps, and item-count caps out of `host.rs` into `crates/niribar/src/dbus/sni/menu.rs`. Expose a narrow validated type.

- [ ] **Step 2: Register**

```rust
pub(super) mod menu;
```

- [ ] **Step 3: Move and keep the existing DBusMenu bounds tests**

Move the existing `truncate_label_*` tests from `sni/host.rs` into `sni/menu.rs` and add the item-limit test below. The test uses the new narrow helper `limit_menu_items(items)` introduced in Step 1; have `fetch_menu` call that helper after `get_layout`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_label_keeps_short_label() {
        assert_eq!(truncate_label("Short".to_owned()), "Short");
    }

    #[test]
    fn truncate_label_caps_long_label() {
        let input = "x".repeat(MAX_MENU_LABEL_BYTES + 10);
        let out = truncate_label(input);
        assert_eq!(out.len(), MAX_MENU_LABEL_BYTES);
    }

    #[test]
    fn items_from_root_children_limits_item_count() {
        let children: Vec<crate::surface::menu_popup::MenuItem> = (0..MAX_MENU_ITEMS + 10)
            .map(|i| crate::surface::menu_popup::MenuItem {
                id: i.to_string(),
                label: format!("item {i}"),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            })
            .collect();

        let items = limit_menu_items(children);
        assert_eq!(items.len(), MAX_MENU_ITEMS);
        assert_eq!(items[0].id, "0");
        assert_eq!(items[MAX_MENU_ITEMS - 1].id, (MAX_MENU_ITEMS - 1).to_string());
    }
}
```

Step 1 must therefore expose this exact helper inside `menu.rs`:

```rust
fn limit_menu_items(
    items: Vec<crate::surface::menu_popup::MenuItem>,
) -> Vec<crate::surface::menu_popup::MenuItem> {
    items.into_iter().take(MAX_MENU_ITEMS).collect()
}
```

In `fetch_menu`, replace the inline `items.into_iter().take(MAX_MENU_ITEMS).collect()` shape with `limit_menu_items(items)` so the bound has a direct unit test.

- [ ] **Step 4: Verify and commit**

```bash
rtk cargo test dbus::sni::menu
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
rtk git add crates/niribar/src/dbus/sni/
rtk git commit -m "refactor(sni): extract DBusMenu handling into sni::menu"
```

---

### Task 4.5: Extract subscription wiring into `sni/subscription.rs`

**Files:**
- Create: `crates/niribar/src/dbus/sni/subscription.rs`
- Modify: `crates/niribar/src/dbus/sni/host.rs`, `crates/niribar/src/dbus/sni/mod.rs`

- [ ] **Step 1: Move the iced-subscription builders**

Cut `sni_subscription`, `menu_layout_subscription`, and their `build_*_stream` helpers out of `host.rs` into `subscription.rs`. Migrate them onto `crate::dbus::subscription::build` following the same pattern as Tasks 1.2–1.4.

- [ ] **Step 2: Register and re-export**

In `sni/mod.rs`:

```rust
pub mod subscription;
pub use subscription::{sni_subscription, menu_layout_subscription};
```

Remove `sni_subscription` and `menu_layout_subscription` from the corresponding `pub use host::{TrayEvent, TrayItem, ItemStatus, sni_subscription, menu_layout_subscription};` line.

- [ ] **Step 3: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [ ] **Step 4: Commit**

```bash
rtk git add crates/niribar/src/dbus/sni/
rtk git commit -m "refactor(sni): extract subscription wiring and reuse generic builder"
```

---

### Task 4.6: Enforce the bounds boundary

**Files:** (no code change — audit only)

- [ ] **Step 1: Grep verification**

Run:

```bash
rtk grep -Rn -E "(width|height)\s*\*\s*(height|width)\s*\*\s*4" src/ | rtk grep -v dbus/sni/pixmap.rs
rtk grep -Rn "max_menu_items\|max_label_len\|MAX_PIXMAP" src/ | rtk grep -v "dbus/sni/\(pixmap\|menu\).rs"
```

Expected: both greps produce no results outside `dbus/sni/pixmap.rs` and `dbus/sni/menu.rs`. If they do, the corresponding call site either belongs inside those files or needs to route its validation through their public API.

- [ ] **Step 2: Record the audit**

Append a one-paragraph note to `docs/niribar/superpowers/specs/2026-04-19-maintainability-refactor-design.md` under a new "Phase 4 audit" section, listing the `rtk grep` commands and the observed result.

- [ ] **Step 3: Commit**

```bash
rtk git add docs/niribar/superpowers/specs/2026-04-19-maintainability-refactor-design.md
rtk git commit -m "docs(refactor): record phase 4 security-boundary audit"
```

---

## Phase 5 — Cleanup and hardening for Phases 1–4

### Task 5.1: Visibility tightening

**Files:** every file touched by Phases 1–4

- [ ] **Step 1: Narrow `pub` to the minimum needed**

Walk every `pub fn`, `pub struct`, `pub enum`, `pub use`. For each, check with `rtk grep -Rn <name>` whether it's referenced from outside its defining module. If not, downgrade to `pub(super)` or `pub(crate)` as appropriate.

Do this by directory: `crates/niribar/src/config/`, `crates/niribar/src/dbus/`, `crates/niribar/src/modules/`, and the small `crates/niribar/src/app/` helpers touched in Phase 1. One commit per directory.

- [ ] **Step 2: Verify after each directory**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

---

### Task 5.2: Dead-code sweep

**Files:** every file touched by Phases 1–4

- [x] **Step 1: Enable warnings temporarily**

Run:

```bash
rtk cargo clippy --all-targets -- -W dead_code -W unused_imports -W unused_variables
```

For each dead-code warning, verify with `rtk grep` that the item really is unreferenced (some types appear in trait impls that clippy can miss), then delete.

- [x] **Step 2: Verify**

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

- [x] **Step 3: Commit**

```bash
rtk git commit -am "chore: remove dead code exposed by refactor"
```

---

### Task 5.3: Bounds-test completeness

**Files:**
- Modify: `crates/niribar/src/dbus/sni/pixmap.rs`, `crates/niribar/src/dbus/sni/menu.rs`

- [ ] **Step 1: Coverage check**

For each bounds check in `pixmap.rs` (max dimensions, byte-length match, retained count, retained bytes) and `menu.rs` (max items, max label length), confirm there is exactly one unit test proving the boundary is enforced. The expected test names after Phases 4.3 and 4.4 are:

```text
valid_pixmap_accepts_matching_byte_len
valid_pixmap_rejects_negative_dimensions
valid_pixmap_rejects_mismatched_byte_len
valid_pixmap_rejects_zero_dimensions
valid_pixmap_rejects_height_out_of_range
valid_pixmap_accepts_max_dimension
retain_pixmaps_with_budget_skips_invalid_and_limits_count
retain_pixmaps_with_budget_limits_total_bytes
truncate_label_keeps_short_label
truncate_label_caps_long_label
items_from_root_children_limits_item_count
```

If implementation renamed a helper during extraction, keep the same behavioral coverage with the renamed helper and adjust the test names to match the final API.

- [ ] **Step 2: Verify and commit**

```bash
rtk cargo test dbus::sni
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
rtk git commit -am "test(sni): complete bounds coverage for pixmap and menu"
```

---

### Task 5.4: Final smoke test and refactor summary

**Files:**
- Create: `docs/superpowers/specs/2026-05-XX-maintainability-refactor-summary.md` (name at completion time)

- [ ] **Step 1: Manual Wayland smoke test (full)**

Run through the smoke-test list that applies before the app actor split:

- Bar renders on every configured output; expand/collapse works.
- Tray icons add/remove; right-click menu opens and executes items; keyboard navigation works.
- Workspace switching updates focus highlight; focused-window title updates.
- Battery, network, volume update live.
- Config reload works without panic.
- No UI stutter during animations.

- [ ] **Step 2: Write the summary**

Produce a summary doc following the shape of `docs/niribar/superpowers/specs/2026-04-19-code-quality-refactor-summary.md`: Completed list, Verification block, Code Size Snapshot (`rtk wc -l` of key files), Residual Risk.

- [ ] **Step 3: Commit**

```bash
rtk git add docs/niribar/superpowers/specs/
rtk git commit -m "docs: summarize maintainability refactor"
```

---

## Deferred Work

The `App` actor split from the design spec is intentionally not implemented by this plan. It lives in `docs/niribar/superpowers/plans/2026-04-19-app-actor-split.md` and should be reviewed/refined after this plan lands green.

## Self-review notes

**Spec coverage:** This plan now intentionally covers the non-`app` half of the design: duplication eliminations → Phases 1–2; `config/parse.rs` split → Phase 3; `sni_host.rs` split and security boundary → Phase 4; hardening/testing → Phase 5. The `App` actor split is deferred to `docs/niribar/superpowers/plans/2026-04-19-app-actor-split.md`.

**Placeholders:** No placeholder markers or comment-only test bodies remain in executable tasks. Phase 4 test steps move existing concrete tests or add concrete helper tests.

**Type consistency:** The retained new types are `SubState`, `IconLabelLayout`, and the moved `dbus::sni::*` module boundaries. The plan avoids new app actor types in this branch.
