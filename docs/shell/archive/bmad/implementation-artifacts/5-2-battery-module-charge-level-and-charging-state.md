# Story 5.2: Battery Module — Charge Level and Charging State

Status: review

## Story

As a **user**,
I want the bar to show my battery level and whether it's charging,
so that I can avoid running out of power unexpectedly.

## Acceptance Criteria

1. **Given** `battery` is in the module list and UPower is available on the session bus
   **When** the bar renders in collapsed state
   **Then** a battery icon reflects the current charge tier (full / high / medium / low / critical) with the appropriate color

2. **Given** the battery is charging
   **When** the bar renders (either state)
   **Then** the icon uses `color.success` (#a6e3a1) and shows "⚡" in collapsed state as the charging indicator

3. **Given** the bar is collapsed AND battery is at or below the configured threshold (default 15%)
   **When** the bar renders
   **Then** the `CriticalBadge` overlays the icon showing the integer percentage (e.g. "13%") — pill with `color.critical` bg, `color.base` text, 9px bold

4. **Given** the bar is expanded
   **When** it renders the battery module
   **Then** the expanded state shows icon + percentage label (e.g. "83%")

5. **Given** battery is at or below the critical threshold AND not charging
   **When** the bar renders in either state
   **Then** the icon color is `color.critical` (#f38ba8)

6. **Given** battery is above the critical threshold and at or below 30%, AND not charging
   **When** the bar renders
   **Then** the icon color is `color.warning` (#f9e2af)

7. **Given** battery state changes (charge level or charging status)
   **When** UPower emits a D-Bus signal
   **Then** the module updates within one rendered frame (no polling)

8. **Given** UPower is unavailable on the session bus
   **When** the bar starts
   **Then** the battery module is silently omitted — `ModuleState::Unavailable`, renders nothing (UX-DR16)

9. **Given** `battery` is NOT in the module list
   **When** the app runs
   **Then** no UPower D-Bus subscription is created

10. **Given** niribar is built
    **When** `cargo clippy -- -D warnings` is run
    **Then** zero warnings

11. **Given** niribar is built
    **When** `cargo test` is run
    **Then** all 183 pre-existing tests pass and at least 9 new tests pass

## Tasks / Subtasks

- [x] Task 1: Add `BatteryConfig` and `ModuleParams::Battery` in `src/config/mod.rs` (AC: #1, #3)
  - [x] 1.1: Add `BatteryConfig` struct after the existing `WorkspacesConfig`:
    ```rust
    /// Configuration for the battery module.
    #[derive(Debug, Clone)]
    pub struct BatteryConfig {
        /// Percentage at or below which the critical badge and red icon appear.
        /// Default: 15. Valid range: 0–100.
        pub critical_threshold: u8,
    }

    impl Default for BatteryConfig {
        fn default() -> Self {
            BatteryConfig { critical_threshold: 15 }
        }
    }
    ```

  - [x] 1.2: Add `Battery(BatteryConfig)` variant to `ModuleParams` enum:
    ```rust
    pub enum ModuleParams {
        None,
        Battery(BatteryConfig),  // ADD
        Clock(ClockConfig),
        Script(ScriptConfig),
        Button(ButtonConfig),
        Workspaces(WorkspacesConfig),
    }
    ```

  - [x] 1.3: Add battery parsing in `src/config/parse.rs` — update the `params =` block:
    ```rust
    let params = if name == "battery" {
        ModuleParams::Battery(parse_battery_params(node, i)?)
    } else if name == "clock" {
    ...
    ```
    And add the parse function:
    ```rust
    fn parse_battery_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<BatteryConfig> {
        // Optional `critical-threshold <int>` child node.
        let Some(children) = node.children() else {
            return Ok(BatteryConfig::default());
        };
        let critical_threshold = match children.get("critical-threshold") {
            None => BatteryConfig::default().critical_threshold,
            Some(n) => {
                let v = n.entries().first().map(|e| e.value());
                match v.and_then(|v| v.as_integer()) {
                    Some(n) if (0..=100).contains(&n) => n as u8,
                    Some(n) => anyhow::bail!(
                        "modules[{idx}].battery.critical-threshold: value {n} out of range. \
                         Expected: integer 0–100."
                    ),
                    None => anyhow::bail!(
                        "modules[{idx}].battery.critical-threshold: expected an integer value."
                    ),
                }
            }
        };
        Ok(BatteryConfig { critical_threshold })
    }
    ```
    Also add `BatteryConfig` to the use imports at the top of `parse.rs`:
    ```rust
    use super::{
        BarConfig, BatteryConfig, ButtonConfig, ClockConfig, Config, Edge, ExpandTrigger,
        ModuleConfig, ModuleParams, ScriptConfig, WorkspaceLabelConfig, WorkspacesConfig,
    };
    ```

- [x] Task 2: Create `crates/niribar/src/modules/battery.rs` (AC: #1–#8)
  - [x] 2.1: Define `BatteryState` and `BatteryEvent`:
    ```rust
    /// Runtime state for the battery module, fed by UPower D-Bus signals via app.rs.
    #[derive(Debug, Clone, Default)]
    pub struct BatteryState {
        pub percentage: f64,   // 0.0–100.0
        pub is_charging: bool,
    }

    /// Events emitted by the UPower D-Bus subscription.
    #[derive(Debug, Clone)]
    pub enum BatteryEvent {
        StateChanged { percentage: f64, is_charging: bool },
        Unavailable,
    }
    ```

  - [x] 2.2: Define charge tier helper and icon/color logic:
    ```rust
    /// Maps percentage + threshold to a charge tier.
    /// Tiers: Critical (≤threshold) / Low (threshold+1–30) / Medium (31–60) / High (61–95) / Full (≥96)
    fn charge_tier(percentage: f64, critical_threshold: u8) -> u8 {
        let pct = percentage.clamp(0.0, 100.0) as u8;
        if pct <= critical_threshold { 0 }       // Critical
        else if pct <= 30 { 1 }                  // Low
        else if pct <= 60 { 2 }                  // Medium
        else if pct <= 95 { 3 }                  // High
        else { 4 }                               // Full
    }

    /// Icon character for each charge tier (not-charging state).
    fn tier_icon(tier: u8) -> &'static str {
        match tier {
            0 => "▏",  // Critical
            1 => "░",  // Low
            2 => "▒",  // Medium
            3 => "▓",  // High
            _ => "█",  // Full
        }
    }

    /// Icon color based on state (AC: #2, #5, #6).
    ///
    /// Priority: charging > critical > warning > normal
    fn icon_color(percentage: f64, is_charging: bool, critical_threshold: u8) -> iced::Color {
        if is_charging {
            return iced::Color::from_rgb8(0xa6, 0xe3, 0xa1); // color.success (green)
        }
        let tier = charge_tier(percentage, critical_threshold);
        match tier {
            0 => iced::Color::from_rgb8(0xf3, 0x8b, 0xa8), // color.critical (red)
            1 => iced::Color::from_rgb8(0xf9, 0xe2, 0xaf), // color.warning (yellow)
            _ => iced::Color::from_rgb8(0x89, 0xb4, 0xfa), // color.accent (blue)
        }
    }
    ```

  - [x] 2.3: Define `BatteryModule` and implement the `Module` trait:
    ```rust
    pub struct BatteryModule {
        id: crate::modules::ModuleId,
        /// Percentage threshold for CriticalBadge and red icon. Set from config at construction.
        critical_threshold: u8,
    }

    impl BatteryModule {
        pub fn new(critical_threshold: u8) -> Self {
            BatteryModule {
                id: crate::modules::ModuleId::new("battery"),
                critical_threshold,
            }
        }
    }

    impl crate::modules::Module for BatteryModule {
        fn id(&self) -> crate::modules::ModuleId {
            self.id.clone()
        }

        /// No independent subscription — fed via Message::Battery in app.rs (AR5, Story 5.1).
        fn subscription(&self) -> iced::Subscription<crate::app::Message> {
            iced::Subscription::none()
        }

        fn view_collapsed(
            &self,
            state: &crate::modules::ModuleState,
        ) -> iced::Element<'_, crate::app::Message> {
            use crate::modules::ModuleState;
            use iced::widget::{container, stack, text};
            use iced::{Alignment, Length};

            let battery = match state {
                ModuleState::Battery(s) => s,
                _ => return iced::widget::Space::new().into(), // Loading / Unavailable → nothing
            };

            let icon_str = if battery.is_charging { "⚡" } else { tier_icon(charge_tier(battery.percentage, self.critical_threshold)) };
            let color = icon_color(battery.percentage, battery.is_charging, self.critical_threshold);

            let icon = container(
                text(icon_str).size(16).color(color),
            )
            .width(Length::Fill)
            .height(20)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

            // CriticalBadge: shown when collapsed AND at/below threshold (AC: #3, UX-DR17).
            if battery.percentage <= self.critical_threshold as f64 {
                let badge_label = format!("{}%", battery.percentage.round() as u8);
                let badge = container(
                    text(badge_label)
                        .size(9)
                        .color(iced::Color::from_rgb8(0x1e, 0x1e, 0x2e)), // color.base
                )
                .padding([1, 3])
                .style(|_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(
                        iced::Color::from_rgb8(0xf3, 0x8b, 0xa8), // color.critical
                    )),
                    border: iced::Border {
                        radius: iced::border::Radius::from(4.0),
                        ..Default::default()
                    },
                    ..Default::default()
                });
                stack![icon, badge].into()
            } else {
                icon.into()
            }
        }

        fn view_expanded(
            &self,
            state: &crate::modules::ModuleState,
        ) -> iced::Element<'_, crate::app::Message> {
            use crate::modules::ModuleState;
            use iced::widget::{container, row, text};
            use iced::{Alignment, Length};

            let battery = match state {
                ModuleState::Battery(s) => s,
                _ => return iced::widget::Space::new().into(),
            };

            let icon_str = if battery.is_charging { "⚡" } else { tier_icon(charge_tier(battery.percentage, self.critical_threshold)) };
            let color = icon_color(battery.percentage, battery.is_charging, self.critical_threshold);
            let pct_label = format!("{}%", battery.percentage.round() as u8);

            container(
                row![
                    text(icon_str).size(16).color(color),
                    text(pct_label)
                        .size(12)
                        .color(iced::Color::from_rgb8(0xa6, 0xad, 0xc8)) // color.subtext
                        .width(Length::Fill),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .height(20)
            .align_y(Alignment::Center)
            .padding([0, 6])
            .into()
        }
    }
    ```

  - [x] 2.4: Add `#[cfg(test)]` block at the bottom of `crates/niribar/src/modules/battery.rs`:
    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn battery_module_id_is_battery() {
            assert_eq!(BatteryModule::new(15).id().0, "battery");
        }

        #[test]
        fn battery_state_default_percentage_zero() {
            assert_eq!(BatteryState::default().percentage, 0.0);
        }

        #[test]
        fn battery_state_default_not_charging() {
            assert!(!BatteryState::default().is_charging);
        }

        #[test]
        fn charge_tier_critical_at_threshold() {
            assert_eq!(charge_tier(15.0, 15), 0); // exactly at threshold → Critical
        }

        #[test]
        fn charge_tier_low_above_threshold() {
            assert_eq!(charge_tier(16.0, 15), 1); // just above → Low
        }

        #[test]
        fn charge_tier_full() {
            assert_eq!(charge_tier(100.0, 15), 4);
        }

        #[test]
        fn icon_color_charging_returns_success() {
            let color = icon_color(10.0, true, 15);
            assert_eq!(color, iced::Color::from_rgb8(0xa6, 0xe3, 0xa1));
        }

        #[test]
        fn icon_color_critical_not_charging_returns_critical() {
            let color = icon_color(10.0, false, 15);
            assert_eq!(color, iced::Color::from_rgb8(0xf3, 0x8b, 0xa8));
        }

        #[test]
        fn icon_color_warning_not_charging_returns_warning() {
            let color = icon_color(25.0, false, 15);
            assert_eq!(color, iced::Color::from_rgb8(0xf9, 0xe2, 0xaf));
        }

        #[test]
        fn icon_color_normal_returns_accent() {
            let color = icon_color(80.0, false, 15);
            assert_eq!(color, iced::Color::from_rgb8(0x89, 0xb4, 0xfa));
        }
    }
    ```

- [x] Task 3: Create `crates/niribar/src/dbus/upower.rs` — UPower D-Bus subscription (AC: #7, #8, #9)
  - [x] 3.1: Define the `#[proxy]` and subscription state machine:
    ```rust
    use iced::futures::stream;
    use tokio::sync::mpsc;

    use crate::modules::battery::BatteryEvent;

    /// UPower DisplayDevice D-Bus proxy (zbus 5.x).
    ///
    /// - Service: org.freedesktop.UPower
    /// - Path:    /org/freedesktop/UPower/devices/DisplayDevice
    /// - Interface: org.freedesktop.UPower.Device
    ///
    /// UPower State values: 1=Charging, 2=Discharging, 3=Empty, 4=FullyCharged,
    /// 5=PendingCharge, 6=PendingDischarge
    #[zbus::proxy(
        interface = "org.freedesktop.UPower.Device",
        default_service = "org.freedesktop.UPower",
        default_path = "/org/freedesktop/UPower/devices/DisplayDevice"
    )]
    trait UPowerDevice {
        #[zbus(property)]
        fn percentage(&self) -> zbus::Result<f64>;

        #[zbus(property)]
        fn state(&self) -> zbus::Result<u32>;
    }

    /// Returns `true` for states that indicate "not discharging":
    /// Charging (1), FullyCharged (4), PendingCharge (5).
    fn upower_state_is_charging(state: u32) -> bool {
        matches!(state, 1 | 4 | 5)
    }
    ```

  - [x] 3.2: Add `battery_subscription` function:
    ```rust
    /// Returns an Iced subscription that connects to the UPower DisplayDevice on `conn`
    /// and emits `Message::Battery(BatteryEvent)` on each state change.
    ///
    /// Emits `BatteryEvent::Unavailable` (once) if UPower is unreachable, then terminates.
    pub fn battery_subscription(conn: zbus::Connection) -> iced::Subscription<crate::app::Message> {
        #[derive(Hash)]
        struct BatterySubId;

        enum SubState {
            Init,
            Running(mpsc::UnboundedReceiver<Result<(f64, bool), ()>>),
            Done,
        }

        iced::Subscription::run_with(BatterySubId, move |_: &BatterySubId| {
            let conn = conn.clone();
            stream::unfold(SubState::Init, move |state| {
                let conn = conn.clone();
                async move {
                    match state {
                        SubState::Init => {
                            let (tx, rx) = mpsc::unbounded_channel();
                            // Spawn a tokio task to own the proxy and drive property change events.
                            // The spawned task sends (percentage, is_charging) or Err(()) via channel.
                            tokio::spawn(async move {
                                let _ = run_upower_task(conn, tx).await;
                            });
                            SubState::Running(rx)
                        }
                        SubState::Running(mut rx) => {
                            match rx.recv().await {
                                Some(Ok((pct, charging))) => {
                                    return Some((
                                        crate::app::Message::Battery(BatteryEvent::StateChanged {
                                            percentage: pct,
                                            is_charging: charging,
                                        }),
                                        SubState::Running(rx),
                                    ));
                                }
                                _ => {
                                    // Channel closed or error → unavailable
                                    return Some((
                                        crate::app::Message::Battery(BatteryEvent::Unavailable),
                                        SubState::Done,
                                    ));
                                }
                            }
                        }
                        SubState::Done => return None,
                    }
                }
            })
        })
    }
    ```

    **Note on SubState::Init transition:** The `Init` arm transitions to `Running(rx)` WITHOUT yielding a message — but `unfold` requires `Option<(Item, State)>` or `None`. Use a trick: from `Init`, recursively step to `Running(rx)` by re-entering the unfold body. The cleanest approach is to NOT return from `Init` but instead await the first message there:

    ```rust
    SubState::Init => {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let _ = run_upower_task(conn, tx).await;
        });
        // Await first message before transitioning to Running
        let mut rx = rx;
        match rx.recv().await {
            Some(Ok((pct, charging))) => Some((
                crate::app::Message::Battery(BatteryEvent::StateChanged {
                    percentage: pct,
                    is_charging: charging,
                }),
                SubState::Running(rx),
            )),
            _ => Some((
                crate::app::Message::Battery(BatteryEvent::Unavailable),
                SubState::Done,
            )),
        }
    }
    SubState::Running(mut rx) => match rx.recv().await {
        Some(Ok((pct, charging))) => Some((
            crate::app::Message::Battery(BatteryEvent::StateChanged {
                percentage: pct,
                is_charging: charging,
            }),
            SubState::Running(rx),
        )),
        _ => Some((
            crate::app::Message::Battery(BatteryEvent::Unavailable),
            SubState::Done,
        )),
    },
    SubState::Done => None,
    ```

  - [x] 3.3: Add `run_upower_task` function:
    ```rust
    /// Drives the UPower property subscription.
    ///
    /// Sends (percentage, is_charging) on initial read and on each property change.
    /// Sends Err(()) on any fatal error (proxy creation, property read failure).
    /// The task owns the proxy for its entire lifetime, avoiding lifetime issues with the generated proxy.
    async fn run_upower_task(
        conn: zbus::Connection,
        tx: mpsc::UnboundedSender<Result<(f64, bool), ()>>,
    ) -> zbus::Error {
        // Build proxy — error if UPower not on bus.
        let proxy = match UPowerDeviceProxy::new(&conn).await {
            Ok(p) => p,
            Err(e) => {
                let _ = tx.send(Err(()));
                return e;
            }
        };

        // Send initial state.
        match read_state(&proxy).await {
            Ok(data) => {
                if tx.send(Ok(data)).is_err() { return zbus::Error::Failure("channel closed".into()); }
            }
            Err(e) => {
                let _ = tx.send(Err(()));
                return e;
            }
        }

        // Subscribe to property changes.
        // zbus 5.x: `receive_percentage_changed()` and `receive_state_changed()` return
        // `impl Stream` (awaitable). Use tokio::select! to merge both streams.
        let mut pct_stream = proxy.receive_percentage_changed().await;
        let mut state_stream = proxy.receive_state_changed().await;

        use iced::futures::StreamExt;
        loop {
            tokio::select! {
                _ = pct_stream.next() => {
                    match read_state(&proxy).await {
                        Ok(data) => { if tx.send(Ok(data)).is_err() { break; } }
                        Err(_) => { let _ = tx.send(Err(())); break; }
                    }
                }
                _ = state_stream.next() => {
                    match read_state(&proxy).await {
                        Ok(data) => { if tx.send(Ok(data)).is_err() { break; } }
                        Err(_) => { let _ = tx.send(Err(())); break; }
                    }
                }
            }
        }
        zbus::Error::Failure("stream ended".into())
    }

    async fn read_state(proxy: &UPowerDeviceProxy<'_>) -> zbus::Result<(f64, bool)> {
        let pct = proxy.percentage().await?;
        let state = proxy.state().await?;
        Ok((pct, upower_state_is_charging(state)))
    }
    ```

  - [x] 3.4: Add tests at bottom of `crates/niribar/src/dbus/upower.rs`:
    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn charging_state_1_is_charging() {
            assert!(upower_state_is_charging(1));
        }

        #[test]
        fn fully_charged_state_4_is_charging() {
            assert!(upower_state_is_charging(4));
        }

        #[test]
        fn discharging_state_2_is_not_charging() {
            assert!(!upower_state_is_charging(2));
        }
    }
    ```

- [x] Task 4: Update `src/modules/mod.rs` — register battery, add state variant (AC: #1, #8, #9)
  - [x] 4.1: Add `pub mod battery;` at the top with other module declarations:
    ```rust
    pub mod battery;
    ```

  - [x] 4.2: Add `Battery(battery::BatteryState)` variant to `ModuleState` after `FocusedWindow`:
    ```rust
    Battery(battery::BatteryState),
    ```

  - [x] 4.3: Register in `build_registry()` — add after the `"focused-window"` entry:
    ```rust
    map.insert("battery", battery_ctor as ModuleConstructor);
    ```
    And add the constructor function:
    ```rust
    fn battery_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
        use crate::config::{BatteryConfig, ModuleParams};
        let threshold = match &mc.params {
            ModuleParams::Battery(cfg) => cfg.critical_threshold,
            _ => BatteryConfig::default().critical_threshold,
        };
        Box::new(battery::BatteryModule::new(threshold))
    }
    ```

- [x] Task 5: Update `crates/niribar/src/app.rs` — add `Message::Battery`, handler, and subscription (AC: #7, #8, #9)
  - [x] 5.1: Add import for battery module:
    ```rust
    use crate::modules::battery::{self, BatteryEvent, BatteryState};
    ```

  - [x] 5.2: Add `Message::Battery(BatteryEvent)` variant to the `Message` enum after `DbusReady`:
    ```rust
    /// Battery charge state from UPower D-Bus subscription (Story 5.2).
    Battery(BatteryEvent),
    ```

  - [x] 5.3: Add `push_battery_state` helper method to `App` (in a separate `impl App` block or alongside `push_focused_window_state`):
    ```rust
    /// Push new battery state to all battery module instances in shared.modules.
    fn push_battery_state(&mut self, percentage: f64, is_charging: bool) {
        for (_, module) in &self.modules {
            if module.id().0 == "battery" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::Battery(BatteryState { percentage, is_charging }),
                );
            }
        }
    }
    ```

  - [x] 5.4: Handle `Message::Battery(...)` in `App::update()` — add **before** `Message::DbusReady`:
    ```rust
    Message::Battery(BatteryEvent::StateChanged { percentage, is_charging }) => {
        self.push_battery_state(percentage, is_charging);
        Task::none()
    }
    Message::Battery(BatteryEvent::Unavailable) => {
        // Mark all battery instances unavailable — renders nothing (UX-DR16).
        for (_, module) in &self.modules {
            if module.id().0 == "battery" {
                self.shared.modules.insert(module.id(), ModuleState::Unavailable);
            }
        }
        Task::none()
    }
    ```

  - [x] 5.5: Add battery subscription in `App::subscription()` — add after `dbus_sub` declaration:
    ```rust
    // AC #9: Only subscribe if battery module is present AND session bus is connected.
    let battery_sub = if let Some(conn) = self.shared.dbus.session.clone() {
        if self.modules.iter().any(|(_, m)| m.id().0 == "battery") {
            crate::dbus::upower::battery_subscription(conn)
        } else {
            iced::Subscription::none()
        }
    } else {
        iced::Subscription::none()
    };
    ```
    Include `battery_sub` in all `iced::Subscription::batch(...)` calls in `subscription()`.

  - [x] 5.6: Add tests to `crates/niribar/src/app.rs` `#[cfg(test)]` block:
    ```rust
    // Story 5.2 tests

    fn make_battery_app() -> App {
        use crate::config::{BatteryConfig, ModuleConfig, ModuleParams};
        use crate::modules::Zone;
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "battery".into(),
            zone: Zone::Bottom,
            params: ModuleParams::Battery(BatteryConfig { critical_threshold: 15 }),
        }];
        App::new(config)
    }

    #[test]
    fn battery_state_changed_updates_module() {
        let mut app = make_battery_app();
        let _ = app.update(Message::Battery(BatteryEvent::StateChanged {
            percentage: 72.0,
            is_charging: false,
        }));
        let id = crate::modules::ModuleId::new("battery");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Battery(s)) => {
                assert!((s.percentage - 72.0).abs() < 0.001);
                assert!(!s.is_charging);
            }
            other => panic!("expected Battery state, got {:?}", other),
        }
    }

    #[test]
    fn battery_unavailable_marks_module_unavailable() {
        let mut app = make_battery_app();
        let _ = app.update(Message::Battery(BatteryEvent::Unavailable));
        let id = crate::modules::ModuleId::new("battery");
        assert!(matches!(
            app.shared.modules.get(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn battery_state_not_updated_without_battery_module() {
        let mut app = App::new(Config::default()); // no battery module
        let _ = app.update(Message::Battery(BatteryEvent::StateChanged {
            percentage: 50.0,
            is_charging: true,
        }));
        let id = crate::modules::ModuleId::new("battery");
        assert!(!app.shared.modules.contains_key(&id));
    }
    ```

- [x] Task 6: Quality gates (AC: #10, #11)
  - [x] 6.1: `cargo build` — succeeds
  - [x] 6.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 6.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 6.4: `cargo test` — at least 9 new tests + all 183 pre-existing pass (≥192 total)

## Dev Notes

### What Exists After Story 5.1

- `src/dbus/mod.rs`: `DbusConnections { session: Option<Connection>, system: Option<Connection> }` + `dbus_init_subscription()`
- `crates/niribar/src/app.rs`:
  - `SharedState.dbus: DbusConnections` field — populated by `Message::DbusReady`
  - `Message::DbusReady(DbusConnections)` handled in `update()`
  - `subscription()` includes config-gated `dbus_sub` (starts when battery/network/volume in config)
  - Pattern `push_focused_window_state()` — parallel pattern for `push_battery_state()`
- `src/modules/mod.rs`: `ModuleState` has `Battery` NOT in it yet; `KNOWN_MODULE_NAMES` already includes "battery"; `build_registry()` has NO "battery" entry yet
- No `crates/niribar/src/modules/battery.rs` — create new file
- No `crates/niribar/src/dbus/upower.rs` — create new file
- `BatteryConfig` and `ModuleParams::Battery` NOT in config — add them
- 183 tests passing

### D-Bus Subscription Architecture (AR5)

**Why `BatteryModule::subscription()` returns `Subscription::none()`:**
The session bus connection (`shared.dbus.session`) arrives AFTER modules are constructed (via `Message::DbusReady`). Modules don't have access to `SharedState`, so the connection cannot be passed to `module.subscription()`. Instead:
- `App::subscription()` owns the D-Bus subscriptions — same pattern as `niri_subscription()`
- `battery_subscription(conn)` lives in `crates/niribar/src/dbus/upower.rs`
- `BatteryModule::subscription()` → `Subscription::none()` (like `FocusedWindowModule`)

**Connection availability guard:** The battery subscription starts only when BOTH:
1. `self.shared.dbus.session.is_some()` — D-Bus session bus connected
2. `self.modules.iter().any(|(_, m)| m.id().0 == "battery")` — battery module in config

This ensures AC #9: no subscription created if battery not in module list.

### UPower D-Bus API (zbus 5.12.0)

**Service:** `org.freedesktop.UPower`
**Path:** `/org/freedesktop/UPower/devices/DisplayDevice`
**Interface:** `org.freedesktop.UPower.Device`

**Properties:**
- `Percentage: f64` — 0.0 to 100.0
- `State: u32` — 1=Charging, 2=Discharging, 3=Empty, 4=FullyCharged, 5=PendingCharge, 6=PendingDischarge

**`#[zbus::proxy]` attribute placement:** The `#[proxy]` macro in zbus 5.x uses `#[zbus::proxy(...)]` attribute on a `trait` block. The macro generates a `UPowerDeviceProxy<'_>` struct and impl.

**`receive_{property}_changed()` API:** In zbus 5.x, `receive_percentage_changed()` returns `impl Stream<Item = PropertyChanged<f64>>`. These streams borrow `proxy`, so both must live inside the same `tokio::spawn` async block — this avoids all lifetime conflicts.

**`tokio::select!` import:** `tokio::select!` is a macro from the `tokio` crate (already in Cargo.toml with feature `full`). Use `use iced::futures::StreamExt;` for `.next()` on streams.

### Subscription Pattern — Channel Bridge (same as niri for blocking I/O)

```
tokio::spawn(async { UPower proxy + signal loop → send via tx })
        ↓ channel
stream::unfold(Running(rx)) { rx.recv().await → emit Message::Battery(...) }
```

This pattern prevents the proxy's borrow-tied lifetime from crossing async boundaries in the unfold closure. The spawned task owns the proxy for its lifetime.

### `BatteryEvent` vs `ModuleEvent::Battery`

Battery uses `Message::Battery(BatteryEvent)` (top-level variant, same pattern as `Message::Niri(NiriEvent)`), NOT `Message::Module(id, ModuleEvent::Battery(...))`. Rationale:
- The D-Bus subscription is at the app level, not module level
- Multiple battery module instances receive the same update (broadcast)
- Consistent with how Niri IPC events are handled (app.rs routes to individual modules)

`ModuleEvent` is NOT extended. Do NOT add `Battery` to `ModuleEvent`.

### `iced::widget::stack!` for CriticalBadge

iced 0.14 provides `iced::widget::stack![]` macro for overlaying widgets. The badge sits on top of the icon when `percentage ≤ critical_threshold`. If `stack!` is not in scope, try `iced::widget::Stack::with_children(vec![icon.into(), badge.into()])`.

### Color Constants

| Color | Hex | Usage |
|-------|-----|-------|
| `color.accent` | `#89b4fa` | Normal battery icon (Blue) |
| `color.warning` | `#f9e2af` | Low battery, ≤30% (Yellow) |
| `color.critical` | `#f38ba8` | Critical battery, ≤threshold (Red) |
| `color.success` | `#a6e3a1` | Charging indicator (Green) |
| `color.subtext` | `#a6adc8` | Percentage label in expanded view |
| `color.base` | `#1e1e2e` | CriticalBadge text |

### Charge Tier Thresholds

| Tier | Percentage Range |
|------|----------------|
| Critical | ≤ `critical_threshold` (default 15%) |
| Low | `threshold+1` – 30% |
| Medium | 31–60% |
| High | 61–95% |
| Full | ≥96% |

### Icon Characters

| State | Collapsed Icon |
|-------|----------------|
| Charging (any level) | "⚡" |
| Full (≥96%) | "█" |
| High (61-95%) | "▓" |
| Medium (31-60%) | "▒" |
| Low (≤30%) | "░" |
| Critical (≤threshold) | "▏" |

### `push_battery_state` Pattern — Parallels `push_focused_window_state`

```rust
// Exact same pattern as push_focused_window_state in Story 4.4:
fn push_battery_state(&mut self, percentage: f64, is_charging: bool) {
    for (_, module) in &self.modules {
        if module.id().0 == "battery" {
            self.shared.modules.insert(
                module.id(),
                ModuleState::Battery(BatteryState { percentage, is_charging }),
            );
        }
    }
}
```

### zbus `#[proxy]` Macro Location

The `#[zbus::proxy]` attribute goes on a Rust `trait` definition. Place at the top of `crates/niribar/src/dbus/upower.rs`. The generated `UPowerDeviceProxy<'_>` struct is used inside `run_upower_task`.

### Config KDL syntax (for reference)

```kdl
module "battery" zone="bottom" {
    critical-threshold 20  // optional; integer 0–100; default 15
}
```

### File Locations

| What | Where |
|------|--------|
| `BatteryConfig`, `ModuleParams::Battery` | `src/config/mod.rs` |
| `parse_battery_params()` | `src/config/parse.rs` |
| `BatteryState`, `BatteryEvent`, `BatteryModule`, view logic | `crates/niribar/src/modules/battery.rs` (new) |
| `ModuleState::Battery(...)`, `battery_ctor`, `pub mod battery` | `src/modules/mod.rs` |
| `UPowerDevice` proxy, `battery_subscription()`, `run_upower_task()` | `crates/niribar/src/dbus/upower.rs` (new) |
| `use crate::modules::battery::{...}`, `Message::Battery`, `push_battery_state`, handler, sub | `crates/niribar/src/app.rs` |

### References

- Epic 5 §"Story 5.2": user story + 6 ACs
- Architecture §"D-Bus Naming": `crates/niribar/src/dbus/upower.rs`, `UPowerProxy`
- Architecture §"Module System": `BatteryState`, `BatteryEvent` naming convention
- Architecture §"Communication Patterns": `BatteryEvent::Unavailable` on service absence (not panic)
- Architecture §"D-Bus Boundary": session bus for UPower — `shared.dbus.session`
- Story 5.1: `DbusConnections`, `Message::DbusReady`, session bus availability check
- Story 4.4 dev notes: `push_focused_window_state` pattern — identical structure for `push_battery_state`
- UX-DR5 (ModuleIcon): stateful color — Normal/Warning/Critical/Dim
- UX-DR7/UX-DR17 (CriticalBadge): threshold overlay, collapsed only, `color.critical` bg, `color.base` text, 9px
- Color system: accent=#89b4fa, warning=#f9e2af, critical=#f38ba8, success=#a6e3a1, subtext=#a6adc8, base=#1e1e2e

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

- `iced::Subscription::run_with` requires `fn` pointer (not closure) and `D: Hash`. Used `BatteryConnData(zbus::Connection)` wrapper with constant hash, and `build_battery_stream` as a named fn returning a pinned boxed stream.
- `iced::widget::Space::new()` takes no arguments in iced 0.14 (not `Space::new(w, h)`).
- `BatteryModule::id()` test required `use crate::modules::Module;` in scope.

### Completion Notes List

- Implemented `BatteryConfig` + `ModuleParams::Battery` with optional `critical-threshold` KDL parsing.
- Created `crates/niribar/src/modules/battery.rs`: `BatteryState`, `BatteryEvent`, charge tier logic, icon/color helpers, `BatteryModule` (collapsed + expanded views with CriticalBadge overlay).
- Created `crates/niribar/src/dbus/upower.rs`: `#[zbus::proxy]` for UPower DisplayDevice, channel-bridge subscription pattern, `run_upower_task` (initial read + `tokio::select!` on property change streams).
- Updated `src/modules/mod.rs`: `pub mod battery`, `ModuleState::Battery`, `battery_ctor` in registry.
- Updated `crates/niribar/src/app.rs`: `Message::Battery`, `push_battery_state`, battery message handler, battery subscription (session-bus-gated, module-presence-gated).
- All 199 tests pass (183 pre-existing + 16 new). `cargo clippy -- -D warnings` clean. `cargo fmt --check` clean.

### File List

- src/config/mod.rs
- src/config/parse.rs
- crates/niribar/src/modules/battery.rs (new)
- src/modules/mod.rs
- src/dbus/mod.rs
- crates/niribar/src/dbus/upower.rs (new)
- crates/niribar/src/app.rs

## Change Log

- 2026-04-05: Implemented Story 5.2 — battery module with UPower D-Bus subscription, charge tier icons/colors, CriticalBadge overlay, and session-bus-gated subscription. 16 new tests added (199 total).
