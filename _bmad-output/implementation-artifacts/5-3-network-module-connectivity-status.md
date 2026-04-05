# Story 5.3: Network Module — Connectivity Status

Status: review

## Story

As a **user**,
I want the bar to show my current network connectivity state,
So that I can see at a glance whether I'm online or not.

## Acceptance Criteria

1. **Given** `network` is in the module list and NetworkManager **or** systemd-networkd is available on the system bus
   **When** the bar renders in collapsed state
   **Then** a network icon reflects connectivity: full ("◆" accent), limited ("◈" warning), none ("✕" critical)

2. **Given** the bar is expanded
   **When** it renders the network module
   **Then** the expanded state shows icon + connection label (NM: profile name, networkd: first online non-loopback interface name)

3. **Given** connectivity is full (NM `Connectivity == 4`, networkd `OnlineState == "online"`)
   **When** the bar renders
   **Then** icon color is `color.accent` (#89b4fa)

4. **Given** connectivity is limited (NM `Connectivity == 2 or 3`, networkd `OnlineState == "partial"`)
   **When** the bar renders
   **Then** icon color is `color.warning` (#f9e2af)

5. **Given** connectivity is none (NM `Connectivity == 0 or 1`, networkd `OnlineState == "offline"`)
   **When** the bar renders
   **Then** icon color is `color.critical` (#f38ba8)

6. **Given** network state changes
   **When** the backend emits a D-Bus property change signal
   **Then** the module updates within one rendered frame (no polling)

7. **Given** NetworkManager is unavailable but systemd-networkd is available
   **When** the bar starts
   **Then** the module connects via systemd-networkd instead

8. **Given** neither NetworkManager nor systemd-networkd is available on the system bus
   **When** the bar starts
   **Then** the network module is silently omitted — `ModuleState::Unavailable`, renders nothing (UX-DR16)

9. **Given** `network` is NOT in the module list
   **When** the app runs
   **Then** no network D-Bus subscription is created

10. **Given** niribar is built
    **When** `cargo clippy -- -D warnings` is run
    **Then** zero warnings

11. **Given** niribar is built
    **When** `cargo test` is run
    **Then** all 199 pre-existing tests pass and at least 13 new tests pass (≥212 total)

## Tasks / Subtasks

- [x] Task 1: Add `NetworkConfig` and `ModuleParams::Network` in `src/config/mod.rs` and `src/config/parse.rs` (AC: #1)
  - [x] 1.1: Add `NetworkConfig` struct after `BatteryConfig` in `src/config/mod.rs`:
    ```rust
    /// Configuration for the network module (no user-configurable params for MVP).
    #[derive(Debug, Clone, Default)]
    pub struct NetworkConfig;
    ```

  - [x] 1.2: Add `Network(NetworkConfig)` variant to `ModuleParams` enum (after `Battery`):
    ```rust
    pub enum ModuleParams {
        None,
        Battery(BatteryConfig),
        Network(NetworkConfig),  // ADD
        Clock(ClockConfig),
        Script(ScriptConfig),
        Button(ButtonConfig),
        Workspaces(WorkspacesConfig),
    }
    ```

  - [x] 1.3: Add battery parsing in `src/config/parse.rs` — update `params =` block and imports:
    ```rust
    // In the params chain — add BEFORE the "clock" branch:
    let params = if name == "battery" {
        ModuleParams::Battery(parse_battery_params(node, i)?)
    } else if name == "network" {
        ModuleParams::Network(NetworkConfig)  // no configurable params
    } else if name == "clock" {
    // ...

    // In the use import at the top — add NetworkConfig:
    use super::{
        BarConfig, BatteryConfig, ButtonConfig, ClockConfig, Config, Edge, ExpandTrigger,
        ModuleConfig, ModuleParams, NetworkConfig, ScriptConfig, WorkspaceLabelConfig, WorkspacesConfig,
    };
    ```

- [x] Task 2: Create `src/modules/network.rs` (AC: #1–#5, #8)
  - [x] 2.1: Define `NetworkConnectivity`, `NetworkState`, `NetworkEvent`:
    ```rust
    /// Connectivity level reported by the active backend.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub enum NetworkConnectivity {
        Full,    // full internet access
        Limited, // local/site connectivity only or captive portal
        #[default]
        None,    // no connectivity
    }

    /// Runtime state for the network module.
    #[derive(Debug, Clone, Default)]
    pub struct NetworkState {
        pub connectivity: NetworkConnectivity,
        /// Connection label: NM profile name or networkd interface name.
        pub label: Option<String>,
    }

    /// Events emitted by the network D-Bus subscription.
    #[derive(Debug, Clone)]
    pub enum NetworkEvent {
        StateChanged { connectivity: NetworkConnectivity, label: Option<String> },
        Unavailable,
    }
    ```

  - [x] 2.2: Define icon and color helpers:
    ```rust
    fn icon_str(connectivity: &NetworkConnectivity) -> &'static str {
        match connectivity {
            NetworkConnectivity::Full    => "◆",
            NetworkConnectivity::Limited => "◈",
            NetworkConnectivity::None    => "✕",
        }
    }

    fn icon_color(connectivity: &NetworkConnectivity) -> iced::Color {
        match connectivity {
            NetworkConnectivity::Full    => iced::Color::from_rgb8(0x89, 0xb4, 0xfa), // accent
            NetworkConnectivity::Limited => iced::Color::from_rgb8(0xf9, 0xe2, 0xaf), // warning
            NetworkConnectivity::None    => iced::Color::from_rgb8(0xf3, 0x8b, 0xa8), // critical
        }
    }
    ```

  - [x] 2.3: Define `NetworkModule` and implement the `Module` trait:
    ```rust
    pub struct NetworkModule {
        id: crate::modules::ModuleId,
    }

    impl NetworkModule {
        pub fn new() -> Self {
            NetworkModule { id: crate::modules::ModuleId::new("network") }
        }
    }

    impl crate::modules::Module for NetworkModule {
        fn id(&self) -> crate::modules::ModuleId { self.id.clone() }

        fn subscription(&self) -> iced::Subscription<crate::app::Message> {
            iced::Subscription::none() // fed via Message::Network from app.rs (AR5)
        }

        fn view_collapsed(&self, state: &crate::modules::ModuleState)
            -> iced::Element<'_, crate::app::Message>
        {
            use crate::modules::ModuleState;
            use iced::widget::{container, text};
            use iced::{Alignment, Length};

            let net = match state {
                ModuleState::Network(s) => s,
                _ => return iced::widget::Space::new().into(),
            };

            container(
                text(icon_str(&net.connectivity))
                    .size(16)
                    .color(icon_color(&net.connectivity)),
            )
            .width(Length::Fill)
            .height(20)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
        }

        fn view_expanded(&self, state: &crate::modules::ModuleState)
            -> iced::Element<'_, crate::app::Message>
        {
            use crate::modules::ModuleState;
            use iced::widget::{container, row, text};
            use iced::{Alignment, Length};

            let net = match state {
                ModuleState::Network(s) => s,
                _ => return iced::widget::Space::new().into(),
            };

            let label_text = net.label.as_deref().unwrap_or(match &net.connectivity {
                NetworkConnectivity::Full    => "online",
                NetworkConnectivity::Limited => "limited",
                NetworkConnectivity::None    => "offline",
            });

            container(
                row![
                    text(icon_str(&net.connectivity))
                        .size(16)
                        .color(icon_color(&net.connectivity)),
                    text(label_text)
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

  - [x] 2.4: Add `#[cfg(test)]` block at the bottom of `src/modules/network.rs`:
    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::modules::Module;

        #[test]
        fn network_module_id_is_network() {
            assert_eq!(NetworkModule::new().id().0, "network");
        }

        #[test]
        fn network_state_default_connectivity_is_none() {
            assert_eq!(NetworkState::default().connectivity, NetworkConnectivity::None);
        }

        #[test]
        fn network_state_default_label_is_none() {
            assert!(NetworkState::default().label.is_none());
        }

        #[test]
        fn network_state_stores_label() {
            let s = NetworkState {
                connectivity: NetworkConnectivity::Full,
                label: Some("Home WiFi".to_string()),
            };
            assert_eq!(s.label.as_deref(), Some("Home WiFi"));
        }
    }
    ```

- [x] Task 3: Create `src/dbus/networkmanager.rs` — dual-backend (NM + networkd) subscription (AC: #6–#9)
  - [x] 3.1: Define D-Bus proxies for both backends:
    ```rust
    use crate::modules::network::{NetworkConnectivity, NetworkEvent};
    use iced::futures::stream;
    use tokio::sync::mpsc;

    // ── NetworkManager proxy ─────────────────────────────────────────────────

    /// Main NetworkManager proxy.
    #[zbus::proxy(
        interface = "org.freedesktop.NetworkManager",
        default_service = "org.freedesktop.NetworkManager",
        default_path = "/org/freedesktop/NetworkManager"
    )]
    trait NetworkManager {
        /// Connectivity: 0=Unknown, 1=None, 2=Limited, 3=Portal, 4=Full
        #[zbus(property)]
        fn connectivity(&self) -> zbus::Result<u32>;

        /// Path to primary active connection, or "/" when no connection.
        #[zbus(property)]
        fn primary_connection(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
    }

    /// Active connection proxy for reading the profile name.
    #[zbus::proxy(
        interface = "org.freedesktop.NetworkManager.Connection.Active",
        default_service = "org.freedesktop.NetworkManager",
        default_path = "/"   // placeholder; always use builder to set real path
    )]
    trait NMActiveConnection {
        /// Human-readable connection profile name (e.g. "Home WiFi").
        #[zbus(property)]
        fn id(&self) -> zbus::Result<String>;
    }

    // ── systemd-networkd proxy ───────────────────────────────────────────────

    /// systemd-networkd Manager proxy.
    #[zbus::proxy(
        interface = "org.freedesktop.network1.Manager",
        default_service = "org.freedesktop.network1",
        default_path = "/org/freedesktop/network1"
    )]
    trait NetworkdManager {
        /// "online" | "partial" | "offline"
        #[zbus(property)]
        fn online_state(&self) -> zbus::Result<String>;

        /// Returns (index: i32, name: String, path: OwnedObjectPath) per link.
        fn list_links(
            &self,
        ) -> zbus::Result<Vec<(i32, String, zbus::zvariant::OwnedObjectPath)>>;
    }
    ```

  - [x] 3.2: Define connectivity conversion helpers:
    ```rust
    /// Convert NM Connectivity u32 to NetworkConnectivity.
    fn nm_connectivity(value: u32) -> NetworkConnectivity {
        match value {
            4 => NetworkConnectivity::Full,
            2 | 3 => NetworkConnectivity::Limited,
            _ => NetworkConnectivity::None, // 0=Unknown, 1=None
        }
    }

    /// Convert networkd OnlineState string to NetworkConnectivity.
    fn networkd_connectivity(state: &str) -> NetworkConnectivity {
        match state {
            "online"  => NetworkConnectivity::Full,
            "partial" => NetworkConnectivity::Limited,
            _         => NetworkConnectivity::None, // "offline" or unknown
        }
    }
    ```

  - [x] 3.3: Define the subscription (same structural pattern as `battery_subscription`):
    ```rust
    struct NetworkConnData(zbus::Connection);

    impl std::hash::Hash for NetworkConnData {
        fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
            "network-subscription".hash(h);
        }
    }

    enum SubState {
        Init,
        Running(mpsc::UnboundedReceiver<Result<(NetworkConnectivity, Option<String>), ()>>),
        Done,
    }

    fn build_network_stream(
        data: &NetworkConnData,
    ) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>>
    {
        let conn = data.0.clone();
        Box::pin(stream::unfold(SubState::Init, move |state| {
            let conn = conn.clone();
            async move {
                match state {
                    SubState::Init => {
                        let (tx, rx) = mpsc::unbounded_channel();
                        tokio::spawn(async move {
                            let _ = run_network_task(conn, tx).await;
                        });
                        let mut rx = rx;
                        match rx.recv().await {
                            Some(Ok((c, l))) => Some((
                                crate::app::Message::Network(NetworkEvent::StateChanged {
                                    connectivity: c,
                                    label: l,
                                }),
                                SubState::Running(rx),
                            )),
                            _ => Some((
                                crate::app::Message::Network(NetworkEvent::Unavailable),
                                SubState::Done,
                            )),
                        }
                    }
                    SubState::Running(mut rx) => match rx.recv().await {
                        Some(Ok((c, l))) => Some((
                            crate::app::Message::Network(NetworkEvent::StateChanged {
                                connectivity: c,
                                label: l,
                            }),
                            SubState::Running(rx),
                        )),
                        _ => Some((
                            crate::app::Message::Network(NetworkEvent::Unavailable),
                            SubState::Done,
                        )),
                    },
                    SubState::Done => None,
                }
            }
        }))
    }

    /// Returns an Iced subscription for network state.
    /// Tries NetworkManager first; falls back to systemd-networkd; emits Unavailable if both absent.
    pub fn network_subscription(conn: zbus::Connection) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::run_with(NetworkConnData(conn), build_network_stream)
    }
    ```

  - [x] 3.4: Add `run_network_task` — auto-detects backend, runs selected one:
    ```rust
    async fn run_network_task(
        conn: zbus::Connection,
        tx: mpsc::UnboundedSender<Result<(NetworkConnectivity, Option<String>), ()>>,
    ) -> zbus::Error {
        // Auto-detect: try NetworkManager first, fall back to systemd-networkd.
        let dbus = match zbus::fdo::DBusProxy::new(&conn).await {
            Ok(p) => p,
            Err(e) => { let _ = tx.send(Err(())); return e; }
        };

        let has_nm = dbus
            .name_has_owner("org.freedesktop.NetworkManager".try_into().unwrap())
            .await
            .unwrap_or(false);
        let has_nd = dbus
            .name_has_owner("org.freedesktop.network1".try_into().unwrap())
            .await
            .unwrap_or(false);

        if has_nm {
            run_nm_task(conn, tx).await
        } else if has_nd {
            run_networkd_task(conn, tx).await
        } else {
            let _ = tx.send(Err(()));
            zbus::Error::Failure("no network backend available".into())
        }
    }
    ```

  - [x] 3.5: Add `run_nm_task` — NetworkManager backend:
    ```rust
    async fn run_nm_task(
        conn: zbus::Connection,
        tx: mpsc::UnboundedSender<Result<(NetworkConnectivity, Option<String>), ()>>,
    ) -> zbus::Error {
        let proxy = match NetworkManagerProxy::new(&conn).await {
            Ok(p) => p,
            Err(e) => { let _ = tx.send(Err(())); return e; }
        };

        // Send initial state.
        match read_nm_state(&proxy, &conn).await {
            Ok(data) => { if tx.send(Ok(data)).is_err() { return zbus::Error::Failure("channel closed".into()); } }
            Err(e) => { let _ = tx.send(Err(())); return e; }
        }

        // Watch Connectivity and PrimaryConnection property changes.
        let mut conn_stream = proxy.receive_connectivity_changed().await;
        let mut pc_stream   = proxy.receive_primary_connection_changed().await;

        use iced::futures::StreamExt;
        loop {
            tokio::select! {
                _ = conn_stream.next() => {}
                _ = pc_stream.next()   => {}
            }
            match read_nm_state(&proxy, &conn).await {
                Ok(data) => { if tx.send(Ok(data)).is_err() { break; } }
                Err(_)   => { let _ = tx.send(Err(())); break; }
            }
        }
        zbus::Error::Failure("NM stream ended".into())
    }

    async fn read_nm_state(
        proxy: &NetworkManagerProxy<'_>,
        conn: &zbus::Connection,
    ) -> zbus::Result<(NetworkConnectivity, Option<String>)> {
        let connectivity = nm_connectivity(proxy.connectivity().await?);
        let label = read_nm_label(proxy, conn).await;
        Ok((connectivity, label))
    }

    /// Get NM profile name from PrimaryConnection → Id.
    /// Returns None gracefully if no primary connection or query fails.
    async fn read_nm_label(
        proxy: &NetworkManagerProxy<'_>,
        conn: &zbus::Connection,
    ) -> Option<String> {
        let path = proxy.primary_connection().await.ok()?;
        // "/" means no active primary connection.
        if path.as_str() == "/" { return None; }
        let active = NMActiveConnectionProxy::builder(conn)
            .path(path)
            .ok()?
            .build()
            .await
            .ok()?;
        active.id().await.ok()
    }
    ```

  - [x] 3.6: Add `run_networkd_task` — systemd-networkd backend:
    ```rust
    async fn run_networkd_task(
        conn: zbus::Connection,
        tx: mpsc::UnboundedSender<Result<(NetworkConnectivity, Option<String>), ()>>,
    ) -> zbus::Error {
        let proxy = match NetworkdManagerProxy::new(&conn).await {
            Ok(p) => p,
            Err(e) => { let _ = tx.send(Err(())); return e; }
        };

        // Send initial state.
        match read_networkd_state(&proxy).await {
            Ok(data) => { if tx.send(Ok(data)).is_err() { return zbus::Error::Failure("channel closed".into()); } }
            Err(e) => { let _ = tx.send(Err(())); return e; }
        }

        // Watch OnlineState property changes on the Manager.
        let mut online_stream = proxy.receive_online_state_changed().await;

        use iced::futures::StreamExt;
        loop {
            if online_stream.next().await.is_none() { break; }
            match read_networkd_state(&proxy).await {
                Ok(data) => { if tx.send(Ok(data)).is_err() { break; } }
                Err(_)   => { let _ = tx.send(Err(())); break; }
            }
        }
        zbus::Error::Failure("networkd stream ended".into())
    }

    async fn read_networkd_state(
        proxy: &NetworkdManagerProxy<'_>,
    ) -> zbus::Result<(NetworkConnectivity, Option<String>)> {
        let online_state = proxy.online_state().await?;
        let connectivity = networkd_connectivity(&online_state);
        // Get label: first non-loopback link name from ListLinks.
        let label = proxy.list_links().await.ok().and_then(|links| {
            links
                .into_iter()
                .filter(|(_, name, _)| name != "lo")
                .map(|(_, name, _)| name)
                .next()
        });
        Ok((connectivity, label))
    }
    ```

  - [x] 3.7: Add tests at the bottom of `src/dbus/networkmanager.rs`:
    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn nm_connectivity_4_is_full() {
            assert_eq!(nm_connectivity(4), NetworkConnectivity::Full);
        }

        #[test]
        fn nm_connectivity_3_is_limited() {
            assert_eq!(nm_connectivity(3), NetworkConnectivity::Limited);
        }

        #[test]
        fn nm_connectivity_2_is_limited() {
            assert_eq!(nm_connectivity(2), NetworkConnectivity::Limited);
        }

        #[test]
        fn nm_connectivity_1_is_none() {
            assert_eq!(nm_connectivity(1), NetworkConnectivity::None);
        }

        #[test]
        fn nm_connectivity_0_is_none() {
            assert_eq!(nm_connectivity(0), NetworkConnectivity::None);
        }

        #[test]
        fn networkd_online_state_online_is_full() {
            assert_eq!(networkd_connectivity("online"), NetworkConnectivity::Full);
        }

        #[test]
        fn networkd_online_state_partial_is_limited() {
            assert_eq!(networkd_connectivity("partial"), NetworkConnectivity::Limited);
        }

        #[test]
        fn networkd_online_state_offline_is_none() {
            assert_eq!(networkd_connectivity("offline"), NetworkConnectivity::None);
        }

        #[test]
        fn networkd_unknown_online_state_is_none() {
            assert_eq!(networkd_connectivity("unknown"), NetworkConnectivity::None);
        }
    }
    ```

- [x] Task 4: Update `src/modules/mod.rs` — register network, add state variant (AC: #1, #8, #9)
  - [x] 4.1: Add `pub mod network;` after `pub mod battery;`:
    ```rust
    pub mod network;
    ```

  - [x] 4.2: Add `Network(network::NetworkState)` to `ModuleState` after `Battery`:
    ```rust
    Network(network::NetworkState),
    ```

  - [x] 4.3: Register in `build_registry()` and add constructor:
    ```rust
    map.insert("network", network_ctor as ModuleConstructor);

    // constructor function:
    fn network_ctor(_mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
        Box::new(network::NetworkModule::new())
    }
    ```

- [x] Task 5: Update `src/app.rs` — add `Message::Network`, handler, and subscription (AC: #6, #8, #9)
  - [x] 5.1: Add import:
    ```rust
    use crate::modules::network::{NetworkEvent, NetworkState};
    ```

  - [x] 5.2: Add `Message::Network(NetworkEvent)` after `Message::Battery`:
    ```rust
    /// Network connectivity state from D-Bus subscription (Story 5.3).
    Network(NetworkEvent),
    ```

  - [x] 5.3: Add `push_network_state` helper (same pattern as `push_battery_state`):
    ```rust
    fn push_network_state(
        &mut self,
        connectivity: crate::modules::network::NetworkConnectivity,
        label: Option<String>,
    ) {
        use crate::modules::network::NetworkState;
        for (_, module) in &self.modules {
            if module.id().0 == "network" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::Network(NetworkState { connectivity, label: label.clone() }),
                );
            }
        }
    }
    ```

  - [x] 5.4: Handle `Message::Network(...)` in `App::update()` — add before `Message::Battery`:
    ```rust
    Message::Network(NetworkEvent::StateChanged { connectivity, label }) => {
        self.push_network_state(connectivity, label);
        Task::none()
    }
    Message::Network(NetworkEvent::Unavailable) => {
        for (_, module) in &self.modules {
            if module.id().0 == "network" {
                self.shared.modules.insert(module.id(), ModuleState::Unavailable);
            }
        }
        Task::none()
    }
    ```

  - [x] 5.5: Add network subscription in `App::subscription()` — after `battery_sub`:
    ```rust
    // AC #9: Only subscribe if network module is present AND system bus is connected.
    let network_sub = if let Some(conn) = self.shared.dbus.system.clone() {
        if self.modules.iter().any(|(_, m)| m.id().0 == "network") {
            crate::dbus::networkmanager::network_subscription(conn)
        } else {
            iced::Subscription::none()
        }
    } else {
        iced::Subscription::none()
    };
    ```
    Include `network_sub` in all `iced::Subscription::batch(...)` calls.

  - [x] 5.6: Add Story 5.3 tests to `src/app.rs` `#[cfg(test)]` block:
    ```rust
    // Story 5.3 tests

    fn make_network_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams, NetworkConfig};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "network".into(),
            zone: Zone::Bottom,
            params: ModuleParams::Network(NetworkConfig),
        }];
        App::new(config)
    }

    #[test]
    fn network_state_changed_updates_module() {
        use crate::modules::network::NetworkConnectivity;
        let mut app = make_network_app();
        let _ = app.update(Message::Network(NetworkEvent::StateChanged {
            connectivity: NetworkConnectivity::Full,
            label: Some("Home".into()),
        }));
        let id = ModuleId::new("network");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Network(s)) => {
                assert_eq!(s.connectivity, NetworkConnectivity::Full);
                assert_eq!(s.label.as_deref(), Some("Home"));
            }
            other => panic!("expected Network state, got {:?}", other),
        }
    }

    #[test]
    fn network_unavailable_marks_module_unavailable() {
        let mut app = make_network_app();
        let _ = app.update(Message::Network(NetworkEvent::Unavailable));
        let id = ModuleId::new("network");
        assert!(matches!(app.shared.modules.get(&id), Some(ModuleState::Unavailable)));
    }

    #[test]
    fn network_state_not_updated_without_network_module() {
        use crate::modules::network::NetworkConnectivity;
        let mut app = App::new(Config::default()); // no network module
        let _ = app.update(Message::Network(NetworkEvent::StateChanged {
            connectivity: NetworkConnectivity::Full,
            label: None,
        }));
        let id = ModuleId::new("network");
        assert!(!app.shared.modules.contains_key(&id));
    }
    ```

- [x] Task 6: Register `pub mod networkmanager` in `src/dbus/mod.rs` (AC: #6)
  - [x] 6.1: Add after `pub mod upower;`:
    ```rust
    pub mod networkmanager;
    ```

- [x] Task 7: Quality gates (AC: #10, #11)
  - [x] 7.1: `cargo build` — succeeds
  - [x] 7.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 7.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 7.4: `cargo test` — all 199 pre-existing tests pass + 16 new tests (215 total)

## Dev Notes

### What Exists After Story 5.2

- `src/dbus/mod.rs`: `pub mod upower;` declared; `DbusConnections { session, system }` + `dbus_init_subscription()`
- `src/app.rs`:
  - `Message::Battery(BatteryEvent)` handled
  - `push_battery_state()` pattern — copy exactly for `push_network_state()`
  - `subscription()` uses `self.shared.dbus.system.clone()` for battery subscription — do the same for network
  - Battery subscription guard: `if let Some(conn) = self.shared.dbus.system.clone()`
- `src/modules/mod.rs`: `pub mod battery;` declared; `ModuleState::Battery`; `battery_ctor`; registry has `"battery"`
- `src/config/mod.rs`: `BatteryConfig`; `ModuleParams::Battery` — add `NetworkConfig` and `ModuleParams::Network` in the same pattern
- 199 tests passing

### ⚠️ Critical Lesson from Story 5.2: Use the SYSTEM Bus

**Both NetworkManager and systemd-networkd are on the SYSTEM bus, NOT the session bus.**

- `session` bus: UPower, WirePlumber — per-user services
- `system` bus: NetworkManager, systemd-networkd — system-wide services

The battery subscription was incorrectly implemented to use `session` bus (per the architecture doc which said "session bus for UPower") and was later fixed. The architecture doc's "AR5: session bus for UPower/WirePlumber, system bus for NetworkManager" is correct.

**Always use `self.shared.dbus.system` for `network_sub`.**

### Dual-Backend Architecture (User Requirement)

This story extends the original spec to support both NetworkManager and systemd-networkd. The detection strategy:

1. Use `zbus::fdo::DBusProxy` to call `NameHasOwner` on the system bus
2. Try NM first (`org.freedesktop.NetworkManager`) — more feature-rich (provides connection name)
3. Fall back to networkd (`org.freedesktop.network1`) — present on this development machine
4. Emit `Unavailable` if neither found

**On this development machine:** Only `org.freedesktop.network1` is present (no NM). Test accordingly.

### D-Bus Subscription Pattern (identical to battery)

The `run_with` constraint:
- `iced::Subscription::run_with(data, builder)` requires `data: D` where `D: Hash + 'static` AND `builder: fn(&D) -> S` (fn pointer, not closure)
- `zbus::Connection` doesn't implement `Hash` → wrap it: `struct NetworkConnData(zbus::Connection)` with constant hash
- Return type must be concrete: `Pin<Box<dyn Stream<Item = Message> + Send + 'static>>`
- Use a named top-level function `build_network_stream(data: &NetworkConnData) -> Pin<Box<...>>` — this satisfies the fn pointer requirement

### NetworkManager D-Bus API

**Service:** `org.freedesktop.NetworkManager` | **Path:** `/org/freedesktop/NetworkManager` | **Bus:** system

**Key Properties:**
| Property | Type | Values |
|----------|------|--------|
| `Connectivity` | u32 | 0=Unknown, 1=None, 2=Limited, 3=Portal, 4=Full |
| `PrimaryConnection` | ObjectPath | path to active connection, or "/" if none |

**Active Connection Profile Name:**
- Path: `{PrimaryConnection path}` with interface `org.freedesktop.NetworkManager.Connection.Active`
- Property: `Id: s` — human-readable profile name (e.g. "Home WiFi", "Wired connection 1")
- Create with builder: `NMActiveConnectionProxy::builder(&conn).path(path)?.build().await?`
- "/" path = no connection → return `None`

**Property change streams:**
- `proxy.receive_connectivity_changed().await` → `impl Stream`
- `proxy.receive_primary_connection_changed().await` → `impl Stream`
- Use `tokio::select!` to watch both simultaneously (identical pattern to battery's `pct_stream + state_stream`)

### systemd-networkd D-Bus API

**Service:** `org.freedesktop.network1` | **Path:** `/org/freedesktop/network1` | **Bus:** system

**Confirmed live on this dev machine** via `busctl --system`:
```
OnlineState:     "partial"
OperationalState: "routable"
ListLinks():     [(4, "wlan0", "…/link/_34"), (1, "lo", "…/link/_31"), (5, "docker0", "…/link/_35"), (2, "enp0s31f6", "…/link/_32")]
```

**Key Properties on `org.freedesktop.network1.Manager`:**
| Property | Type | Values |
|----------|------|--------|
| `OnlineState` | s | "online", "partial", "offline" |
| `OperationalState` | s | "routable", "degraded", "carrier", "no-carrier", "off" |

**`ListLinks()` return type:** `a(iso)` → `Vec<(i32, String, OwnedObjectPath)>` — index, interface name, object path

**Label derivation for networkd:** Call `list_links()`, filter out "lo" (loopback), return the first entry's name. No need to query per-link state objects — the Manager's `OnlineState` already describes overall state.

**Property change stream:** `proxy.receive_online_state_changed().await` — single stream sufficient (OperationalState not needed for our 3-state model)

### `zbus::fdo::DBusProxy` for `NameHasOwner`

```rust
let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
let has_nm = dbus
    .name_has_owner("org.freedesktop.NetworkManager".try_into().unwrap())
    .await
    .unwrap_or(false);
```

`"org.freedesktop.NetworkManager".try_into()` converts `&str` to `BusName<'_>`. This is the correct zbus 5.x pattern. `zbus::fdo::DBusProxy` lives in `zbus::fdo` — it's already a dependency of `zbus`.

### `NMActiveConnectionProxy` — No Default Path

The active connection proxy has no fixed path (each connection has a unique D-Bus object path). Define it with `default_path = "/"` as a placeholder and always create it with the builder:

```rust
#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/"  // placeholder — always override with builder
)]
trait NMActiveConnection { ... }

// Usage:
NMActiveConnectionProxy::builder(conn)
    .path(path)?
    .build()
    .await?
```

### Colors (Catppuccin Mocha)

| Color | Hex | Usage |
|-------|-----|-------|
| `color.accent` | `#89b4fa` | Full connectivity icon (Blue) |
| `color.warning` | `#f9e2af` | Limited/portal connectivity icon (Yellow) |
| `color.critical` | `#f38ba8` | No connectivity icon (Red) |
| `color.subtext` | `#a6adc8` | Connection label in expanded view |

### Icons

| State | Collapsed Icon |
|-------|----------------|
| Full | "◆" (accent) |
| Limited | "◈" (warning) |
| None | "✕" (critical) |

### File Locations

| What | Where |
|------|--------|
| `NetworkConfig`, `ModuleParams::Network` | `src/config/mod.rs` |
| `parse` chain update | `src/config/parse.rs` |
| `NetworkState`, `NetworkEvent`, `NetworkConnectivity`, `NetworkModule` | `src/modules/network.rs` (new) |
| `ModuleState::Network(...)`, `network_ctor`, `pub mod network` | `src/modules/mod.rs` |
| `pub mod networkmanager` | `src/dbus/mod.rs` |
| All proxies, `network_subscription()`, `run_network_task`, `run_nm_task`, `run_networkd_task` | `src/dbus/networkmanager.rs` (new) |
| `Message::Network`, `push_network_state`, handler, sub | `src/app.rs` |

### Test Count

Pre-existing: 199 | New minimum: 13 | Target total: ≥212

New tests location and count:
- `src/dbus/networkmanager.rs`: 9 (5 nm_connectivity + 4 networkd_connectivity)
- `src/modules/network.rs`: 4 (id, default state, label, connectivity default)
- `src/app.rs`: 3 (state_changed, unavailable, no_module)

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

- Implemented `NetworkConfig` struct (zero-config MVP) and `ModuleParams::Network` variant in config layer.
- Created `src/modules/network.rs`: `NetworkConnectivity` enum (Full/Limited/None), `NetworkState`, `NetworkEvent`, `NetworkModule` with collapsed/expanded views using Catppuccin Mocha accent/warning/critical colors.
- Created `src/dbus/networkmanager.rs`: dual-backend D-Bus subscription — auto-detects NM first, falls back to systemd-networkd, emits `Unavailable` if both absent. Pattern mirrors `battery_subscription` exactly.
- Fixed lifetime issue in `view_expanded` by cloning the label `Option<String>` instead of borrowing `&str` from state.
- Added `Message::Network` variant, `push_network_state()` helper, handler in `update()`, and `network_sub` in `subscription()` to `src/app.rs`.
- All 215 tests pass: 199 pre-existing + 16 new (4 in network.rs, 9 in networkmanager.rs, 3 in app.rs).
- `cargo build`, `cargo clippy -- -D warnings`, and `cargo fmt --check` all pass with zero issues.

### File List

- src/config/mod.rs (modified)
- src/config/parse.rs (modified)
- src/modules/network.rs (new)
- src/modules/mod.rs (modified)
- src/dbus/networkmanager.rs (new)
- src/dbus/mod.rs (modified)
- src/app.rs (modified)

## Change Log

- 2026-04-05: Implemented Story 5.3 — network module with dual-backend D-Bus subscription (NM + networkd), icon/color rendering, and 16 new tests (215 total). Status → review.
