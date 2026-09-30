# Story 5.4: Volume Module — Audio Output Level with Scroll Interaction

Status: review

## Story

As a **user**,
I want the bar to show my current audio output volume and let me adjust it by scrolling,
So that I can control volume without opening a separate app.

## Acceptance Criteria

1. **Given** `volume` is in the module list and PipeWire's PulseAudio compat layer is active
   **When** the bar renders in collapsed state
   **Then** a speaker icon reflects the volume tier: high ("◈" accent), medium ("▓" accent), low ("░" accent), muted/zero ("✕" overlay0)

2. **Given** the bar is expanded
   **When** it renders the volume module
   **Then** the expanded state shows icon + percentage label (e.g. "◈  60%") or "✕  muted" when muted

3. **Given** the audio output is muted (or volume is 0)
   **When** the bar renders
   **Then** the icon color is `color.overlay0` (#6c7086)

4. **Given** the audio output is NOT muted
   **When** the bar renders at any tier
   **Then** the icon color is `color.accent` (#89b4fa)

5. **Given** the user scrolls up on the volume module (collapsed or expanded)
   **When** the scroll event fires
   **Then** volume increases by `step` percent (default 5), capped at 150% via `wpctl -l 1.5`

6. **Given** the user scrolls down on the volume module
   **When** the scroll event fires
   **Then** volume decreases by `step` percent

7. **Given** volume state changes (via scroll or any external app)
   **When** the PA compat subscription fires a Sink change event
   **Then** the module updates within one rendered frame (no polling)

8. **Given** the PulseAudio compat socket is unavailable
   **When** the bar starts
   **Then** the volume module is silently omitted — `ModuleState::Unavailable`, renders nothing (UX-DR16)

9. **Given** `volume` is NOT in the module list
   **When** the app runs
   **Then** no PA subscription is created and no scroll handler is wired

10. **Given** niribar is built
    **When** `cargo clippy -- -D warnings` is run
    **Then** zero warnings

11. **Given** niribar is built
    **When** `cargo test` is run
    **Then** all 215 pre-existing tests pass and at least 11 new tests pass (≥226 total)

## Tasks / Subtasks

- [x] Task 1: Add `VolumeConfig` and `ModuleParams::Volume` in `src/config/mod.rs` and `src/config/parse.rs` (AC: #1)
  - [x] 1.1: Add `VolumeConfig` struct after `NetworkConfig` in `src/config/mod.rs`:
    ```rust
    /// Configuration for the volume module — scroll step in percent.
    #[derive(Debug, Clone)]
    pub struct VolumeConfig {
        /// Volume adjustment step per scroll event (percent, 1–100). Default: 5.
        pub step: u8,
    }

    impl Default for VolumeConfig {
        fn default() -> Self {
            VolumeConfig { step: 5 }
        }
    }
    ```

  - [x] 1.2: Add `Volume(VolumeConfig)` variant to `ModuleParams` enum after `Network(NetworkConfig)`:
    ```rust
    Volume(VolumeConfig),
    ```

  - [x] 1.3: Add `VolumeConfig` to the `use super` import in `src/config/parse.rs`:
    ```rust
    use super::{
        BarConfig, BatteryConfig, ButtonConfig, ClockConfig, Config, Edge, ExpandTrigger,
        ModuleConfig, ModuleParams, NetworkConfig, ScriptConfig, VolumeConfig, WorkspaceLabelConfig,
        WorkspacesConfig,
    };
    ```

  - [x] 1.4: Add volume parsing in `src/config/parse.rs` — insert AFTER the "network" branch, BEFORE "clock":
    ```rust
    } else if name == "volume" {
        ModuleParams::Volume(parse_volume_params(node, i)?)
    } else if name == "clock" {
    ```
    And add the parsing function:
    ```rust
    fn parse_volume_params(node: &kdl::KdlNode, idx: usize) -> anyhow::Result<VolumeConfig> {
        let Some(children) = node.children() else {
            return Ok(VolumeConfig::default());
        };
        let step = match children.get("step") {
            None => VolumeConfig::default().step,
            Some(n) => {
                let v = n.entries().first().map(|e| e.value());
                match v.and_then(|v| v.as_integer()) {
                    Some(n) if (1..=100).contains(&n) => n as u8,
                    Some(n) => anyhow::bail!(
                        "modules[{idx}].volume.step: value {n} out of range. \
                         Expected: integer 1–100."
                    ),
                    None => anyhow::bail!(
                        "modules[{idx}].volume.step: expected an integer value."
                    ),
                }
            }
        };
        Ok(VolumeConfig { step })
    }
    ```

- [x] Task 2: Create `crates/niribar/src/modules/volume.rs` (AC: #1–#6, #8)
  - [x] 2.1: Define `VolumeState` and `VolumeEvent`:
    ```rust
    /// Runtime state for the volume module.
    #[derive(Debug, Clone, Default)]
    pub struct VolumeState {
        /// Current volume percentage 0.0–150.0 (PA can exceed 100%).
        pub volume: f64,
        /// Whether the default sink is muted.
        pub muted: bool,
    }

    /// Events emitted by the PA subscription.
    #[derive(Debug, Clone)]
    pub enum VolumeEvent {
        StateChanged { volume: f64, muted: bool },
        Unavailable,
    }
    ```

  - [x] 2.2: Define volume tier and icon/color helpers:
    ```rust
    /// Map volume + mute to a display tier.
    /// Tier 0=Muted/Zero, 1=Low (1–32%), 2=Medium (33–65%), 3=High (>65%)
    fn volume_tier(volume: f64, muted: bool) -> u8 {
        if muted || volume < 0.5 {
            0
        } else if volume <= 32.0 {
            1
        } else if volume <= 65.0 {
            2
        } else {
            3
        }
    }

    fn tier_icon(tier: u8) -> &'static str {
        match tier {
            0 => "✕", // Muted/Zero
            1 => "░", // Low
            2 => "▓", // Medium
            _ => "◈", // High
        }
    }

    fn tier_color(tier: u8) -> iced::Color {
        if tier == 0 {
            iced::Color::from_rgb8(0x6c, 0x70, 0x86) // color.overlay0 — muted/dim
        } else {
            iced::Color::from_rgb8(0x89, 0xb4, 0xfa) // color.accent — normal
        }
    }
    ```

  - [x] 2.3: Define `VolumeModule` and implement the `Module` trait (including `on_interaction` for scroll):
    ```rust
    pub struct VolumeModule {
        id: crate::modules::ModuleId,
        step: u8,
    }

    impl VolumeModule {
        pub fn new(step: u8) -> Self {
            VolumeModule {
                id: crate::modules::ModuleId::new("volume"),
                step,
            }
        }
    }

    impl crate::modules::Module for VolumeModule {
        fn id(&self) -> crate::modules::ModuleId {
            self.id.clone()
        }

        fn subscription(&self) -> iced::Subscription<crate::app::Message> {
            iced::Subscription::none() // fed via Message::Volume from app.rs
        }

        fn view_collapsed(
            &self,
            state: &crate::modules::ModuleState,
        ) -> iced::Element<'_, crate::app::Message> {
            use crate::modules::ModuleState;
            use iced::widget::{container, text};
            use iced::{Alignment, Length};

            let vol = match state {
                ModuleState::Volume(s) => s,
                _ => return iced::widget::Space::new().into(),
            };
            let tier = volume_tier(vol.volume, vol.muted);

            container(
                text(tier_icon(tier))
                    .size(16)
                    .color(tier_color(tier)),
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
            use iced::widget::{container, row, text};
            use iced::{Alignment, Length};

            let vol = match state {
                ModuleState::Volume(s) => s,
                _ => return iced::widget::Space::new().into(),
            };
            let tier = volume_tier(vol.volume, vol.muted);
            let pct_label = if vol.muted {
                "muted".to_owned()
            } else {
                format!("{}%", vol.volume.round() as u64)
            };

            container(
                row![
                    text(tier_icon(tier))
                        .size(16)
                        .color(tier_color(tier)),
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

        fn on_interaction(
            &self,
            event: crate::modules::InteractionEvent,
        ) -> Option<crate::app::Message> {
            use crate::modules::InteractionEvent;
            match event {
                InteractionEvent::ScrollUp => Some(crate::app::Message::VolumeAdjust {
                    step: self.step,
                    up: true,
                }),
                InteractionEvent::ScrollDown => Some(crate::app::Message::VolumeAdjust {
                    step: self.step,
                    up: false,
                }),
                _ => None,
            }
        }
    }
    ```

  - [x] 2.4: Add `#[cfg(test)]` block:
    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::modules::Module;

        #[test]
        fn volume_module_id_is_volume() {
            assert_eq!(VolumeModule::new(5).id().0, "volume");
        }

        #[test]
        fn volume_state_default_volume_is_zero() {
            assert_eq!(VolumeState::default().volume, 0.0);
        }

        #[test]
        fn volume_state_default_not_muted() {
            assert!(!VolumeState::default().muted);
        }

        #[test]
        fn volume_tier_muted_returns_zero() {
            assert_eq!(volume_tier(80.0, true), 0);
        }

        #[test]
        fn volume_tier_zero_returns_zero() {
            assert_eq!(volume_tier(0.0, false), 0);
        }

        #[test]
        fn volume_tier_low() {
            assert_eq!(volume_tier(20.0, false), 1);
        }

        #[test]
        fn volume_tier_medium() {
            assert_eq!(volume_tier(50.0, false), 2);
        }

        #[test]
        fn volume_tier_high() {
            assert_eq!(volume_tier(80.0, false), 3);
        }
    }
    ```

- [x] Task 3: Create `crates/niribar/src/dbus/wireplumber.rs` — PA compat subscription (AC: #7–#9)
  - [x] 3.1: Add `libpulse-binding = "2.30.1"` to `Cargo.toml` `[dependencies]`.
    **Do NOT add** `libpulse-tokio` — bridging is via `std::thread::spawn` + `tokio::sync::mpsc`.

  - [x] 3.2: Define subscription data, state machine, `build_volume_stream`, and `volume_subscription`:
    ```rust
    use crate::modules::volume::VolumeEvent;
    use iced::futures::stream;
    use tokio::sync::mpsc;

    struct VolumeConnData;

    impl std::hash::Hash for VolumeConnData {
        fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
            "volume-subscription".hash(h);
        }
    }

    enum SubState {
        Init,
        Running(mpsc::UnboundedReceiver<Result<(f64, bool), ()>>),
        Done,
    }

    fn build_volume_stream(
        _data: &VolumeConnData,
    ) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>>
    {
        Box::pin(stream::unfold(SubState::Init, |state| async move {
            match state {
                SubState::Init => {
                    let (tx, rx) = mpsc::unbounded_channel();
                    std::thread::spawn(move || run_pa_thread(tx));
                    let mut rx = rx;
                    match rx.recv().await {
                        Some(Ok((v, m))) => Some((
                            crate::app::Message::Volume(VolumeEvent::StateChanged {
                                volume: v,
                                muted: m,
                            }),
                            SubState::Running(rx),
                        )),
                        _ => Some((
                            crate::app::Message::Volume(VolumeEvent::Unavailable),
                            SubState::Done,
                        )),
                    }
                }
                SubState::Running(mut rx) => match rx.recv().await {
                    Some(Ok((v, m))) => Some((
                        crate::app::Message::Volume(VolumeEvent::StateChanged {
                            volume: v,
                            muted: m,
                        }),
                        SubState::Running(rx),
                    )),
                    _ => Some((
                        crate::app::Message::Volume(VolumeEvent::Unavailable),
                        SubState::Done,
                    )),
                },
                SubState::Done => None,
            }
        }))
    }

    /// Returns an Iced subscription for volume state via PulseAudio compat layer.
    /// Emits Unavailable if PA socket is not reachable. No zbus connection needed.
    pub fn volume_subscription() -> iced::Subscription<crate::app::Message> {
        iced::Subscription::run_with(VolumeConnData, build_volume_stream)
    }
    ```

  - [x] 3.3: Implement `run_pa_thread` — PA threaded mainloop with subscription callbacks:
    ```rust
    /// Runs in a dedicated std::thread. Sets up the PA threaded mainloop,
    /// sends initial state, subscribes to sink changes, and blocks until PA terminates.
    fn run_pa_thread(tx: mpsc::UnboundedSender<Result<(f64, bool), ()>>) {
        use libpulse_binding as pulse;
        use pulse::context::subscribe::{Facility, InterestMaskSet, Operation};
        use pulse::mainloop::threaded::Mainloop;

        let mainloop = match Mainloop::new() {
            Some(m) => m,
            None => {
                let _ = tx.send(Err(()));
                return;
            }
        };
        let context = match pulse::context::Context::new(&mainloop, "niribar") {
            Some(c) => c,
            None => {
                let _ = tx.send(Err(()));
                return;
            }
        };

        mainloop.lock();
        if context
            .connect(None, pulse::context::FlagSet::NOAUTOSPAWN, None)
            .is_err()
        {
            mainloop.unlock();
            let _ = tx.send(Err(()));
            return;
        }
        mainloop.unlock();

        if mainloop.start().is_err() {
            let _ = tx.send(Err(()));
            return;
        }

        // Wait for context Ready.
        mainloop.lock();
        loop {
            match context.get_state() {
                pulse::context::State::Ready => break,
                pulse::context::State::Failed | pulse::context::State::Terminated => {
                    mainloop.unlock();
                    let _ = tx.send(Err(()));
                    return;
                }
                _ => mainloop.wait(),
            }
        }

        // Send initial volume state.
        fetch_volume(&context, &tx);

        // Subscribe to sink + server changes so we get notified when volume changes.
        context.subscribe(InterestMaskSet::SINKS | InterestMaskSet::SERVER, |_| {});
        let tx_cb = tx.clone();
        let ctx_cb = context.clone();
        context.set_subscribe_callback(Some(Box::new(move |facility, op, _idx| {
            match (facility, op) {
                (
                    Some(Facility::Sink) | Some(Facility::Server),
                    Some(Operation::Changed) | Some(Operation::New),
                ) => fetch_volume(&ctx_cb, &tx_cb),
                _ => {}
            }
        })));

        mainloop.unlock();

        // Block this thread until PA terminates (PA mainloop runs in its own thread).
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            mainloop.lock();
            let state = context.get_state();
            mainloop.unlock();
            if matches!(
                state,
                pulse::context::State::Failed | pulse::context::State::Terminated
            ) {
                break;
            }
        }
        let _ = tx.send(Err(())); // Signal stream end
    }

    /// Fetch the default sink volume and mute state via PA introspect API.
    /// Called both for initial state and in subscribe callback.
    fn fetch_volume(
        ctx: &libpulse_binding::context::Context,
        tx: &mpsc::UnboundedSender<Result<(f64, bool), ()>>,
    ) {
        let tx2 = tx.clone();
        let introspect = ctx.introspect();
        introspect.get_server_info(move |info| {
            let sink_name = info
                .default_sink_name
                .as_deref()
                .unwrap_or("@DEFAULT_SINK@")
                .to_owned();
            let tx3 = tx2.clone();
            let introspect2 = ctx_from_introspect_workaround();  // see note below
            // Actually get a fresh introspect from ctx — can't easily clone Context here.
            // If Context is Clone in 2.30.1, use ctx.clone().introspect()
            // If not, restructure fetch_volume to take ctx and call ctx.introspect() each time.
            // Safest: call get_sink_info directly via the same introspect.
            introspect.get_sink_info_by_name(&sink_name, move |res| {
                use libpulse_binding::callbacks::ListResult;
                use libpulse_binding::volume::VolumeLinear;
                if let ListResult::Item(sink) = res {
                    let vol: VolumeLinear = sink.volume.avg().into();
                    let pct = (vol.0 * 100.0).clamp(0.0, 150.0);
                    let _ = tx3.send(Ok((pct, sink.mute)));
                }
            });
        });
    }
    ```

    ⚠️ **Context/Introspect Clone issue:** In `libpulse_binding 2.30`, `Context` is NOT `Clone`.
    The `fetch_volume` function above won't compile as written because `get_server_info` consumes
    `introspect` before `get_sink_info_by_name` can use it. The correct pattern uses nested callbacks:

    ```rust
    fn fetch_volume(
        ctx: &libpulse_binding::context::Context,
        tx: &mpsc::UnboundedSender<Result<(f64, bool), ()>>,
    ) {
        // Pattern: nested callbacks, both using the same introspect value.
        // introspect() returns a new Introspect by value each call.
        // We need two calls: one for server info, one for sink info.
        // The nested approach: get server info, then in that callback get sink info.
        // PROBLEM: ctx borrow issues across closure boundary.
        // 
        // Simplest working pattern: skip get_server_info entirely and use "@DEFAULT_SINK@":
        let tx2 = tx.clone();
        ctx.introspect().get_sink_info_by_name("@DEFAULT_SINK@", move |res| {
            use libpulse_binding::callbacks::ListResult;
            use libpulse_binding::volume::VolumeLinear;
            if let ListResult::Item(sink) = res {
                let vol: VolumeLinear = sink.volume.avg().into();
                let pct = (vol.0 * 100.0).clamp(0.0, 150.0);
                let _ = tx2.send(Ok((pct, sink.mute)));
            }
        });
    }
    ```
    **Use `"@DEFAULT_SINK@"` directly** — this is a PA alias that always resolves to the default
    output sink. No need to query `get_server_info` first.

  - [x] 3.4: Add tests at the bottom of `crates/niribar/src/dbus/wireplumber.rs`:
    ```rust
    #[cfg(test)]
    mod tests {
        #[test]
        fn pa_volume_linear_zero_maps_to_0_pct() {
            let linear = 0.0_f64;
            let pct = (linear * 100.0).clamp(0.0, 150.0);
            assert_eq!(pct, 0.0);
        }

        #[test]
        fn pa_volume_linear_one_maps_to_100_pct() {
            let linear = 1.0_f64;
            let pct = (linear * 100.0).clamp(0.0, 150.0);
            assert_eq!(pct, 100.0);
        }

        #[test]
        fn pa_volume_linear_over_one_clamped_to_150() {
            let linear = 2.0_f64;
            let pct = (linear * 100.0).clamp(0.0, 150.0);
            assert_eq!(pct, 150.0);
        }
    }
    ```

- [x] Task 4: Update `src/modules/mod.rs` — register volume, add state variant (AC: #1, #8, #9)
  - [x] 4.1: Add `pub mod volume;` after `pub mod network;`

  - [x] 4.2: Add `Volume(volume::VolumeState)` to `ModuleState` after `Network`:
    ```rust
    Volume(volume::VolumeState),
    ```

  - [x] 4.3: Register in `build_registry()` and add constructor:
    ```rust
    map.insert("volume", volume_ctor as ModuleConstructor);

    fn volume_ctor(mc: &crate::config::ModuleConfig) -> Box<dyn Module> {
        use crate::config::{ModuleParams, VolumeConfig};
        let step = match &mc.params {
            ModuleParams::Volume(cfg) => cfg.step,
            _ => VolumeConfig::default().step,
        };
        Box::new(volume::VolumeModule::new(step))
    }
    ```

- [x] Task 5: Update `crates/niribar/src/app.rs` — add messages, handlers, scroll detection, subscription (AC: #5–#9)
  - [x] 5.1: Add import:
    ```rust
    use crate::modules::volume::{VolumeEvent, VolumeState};
    ```

  - [x] 5.2: Add `Message::Volume`, `Message::VolumeAdjust`, `Message::ModuleScrollUp/Down` after `Message::Network`:
    ```rust
    /// Volume state from PA compat subscription (Story 5.4).
    Volume(VolumeEvent),
    /// Scroll-triggered volume adjustment (Story 5.4).
    VolumeAdjust { step: u8, up: bool },
    /// Scroll-up event on a module (Story 5.4).
    ModuleScrollUp(ModuleId),
    /// Scroll-down event on a module (Story 5.4).
    ModuleScrollDown(ModuleId),
    ```

  - [x] 5.3: Handle `Message::Volume(...)` in `App::update()` — add after `Message::Network` handlers:
    ```rust
    Message::Volume(VolumeEvent::StateChanged { volume, muted }) => {
        self.push_volume_state(volume, muted);
        Task::none()
    }
    Message::Volume(VolumeEvent::Unavailable) => {
        for (_, module) in &self.modules {
            if module.id().0 == "volume" {
                self.shared
                    .modules
                    .insert(module.id(), ModuleState::Unavailable);
            }
        }
        Task::none()
    }
    ```

  - [x] 5.4: Handle `Message::VolumeAdjust` in `App::update()`:
    ```rust
    Message::VolumeAdjust { step, up } => {
        let arg = if up {
            format!("{}%+", step)
        } else {
            format!("{}%-", step)
        };
        // Fire-and-forget: wpctl adjusts PA volume; PA subscription emits StateChanged.
        // "-l 1.5" caps at 150%.
        tokio::task::spawn(async move {
            let _ = tokio::process::Command::new("wpctl")
                .args(["set-volume", "-l", "1.5", "@DEFAULT_AUDIO_SINK@", &arg])
                .status()
                .await;
        });
        Task::none()
    }
    ```

  - [x] 5.5: Handle `Message::ModuleScrollUp/Down` in `App::update()` — same pattern as `ModuleClick`:
    ```rust
    Message::ModuleScrollUp(id) => {
        for (_, module) in &self.modules {
            if module.id() == id {
                if let Some(msg) = module.on_interaction(InteractionEvent::ScrollUp) {
                    return Task::done(msg);
                }
                break;
            }
        }
        Task::none()
    }
    Message::ModuleScrollDown(id) => {
        for (_, module) in &self.modules {
            if module.id() == id {
                if let Some(msg) = module.on_interaction(InteractionEvent::ScrollDown) {
                    return Task::done(msg);
                }
                break;
            }
        }
        Task::none()
    }
    ```

  - [x] 5.6: Add scroll detection to the `mouse_area` in `App::view()`:
    The current view construction is:
    ```rust
    let id = module.id();
    // ...
    let view: Element<'_, Message> = mouse_area(module_frame(module_view, hovered))
        .on_enter(Message::ModuleHoverEnter(id.clone()))
        .on_exit(Message::ModuleHoverExit(id.clone()))
        .on_press(Message::ModuleClick(id))  // ← `id` is moved here
        .into();
    ```
    Change to (note: `id` must be cloned for `on_press` since scroll also needs it):
    ```rust
    let id = module.id();
    // ...
    let scroll_id = id.clone();
    let view: Element<'_, Message> = mouse_area(module_frame(module_view, hovered))
        .on_enter(Message::ModuleHoverEnter(id.clone()))
        .on_exit(Message::ModuleHoverExit(id.clone()))
        .on_press(Message::ModuleClick(id))
        .on_scroll(move |delta| {
            let up = match delta {
                iced::mouse::ScrollDelta::Lines { y, .. }
                | iced::mouse::ScrollDelta::Pixels { y, .. } => y > 0.0,
            };
            if up {
                Message::ModuleScrollUp(scroll_id.clone())
            } else {
                Message::ModuleScrollDown(scroll_id.clone())
            }
        })
        .into();
    ```
    Note: If `on_press(Message::ModuleClick(id))` moves `id`, clone it before:
    `.on_press(Message::ModuleClick(id.clone()))` so `id` is still available. Adjust as needed.

  - [x] 5.7: Add `push_volume_state` helper after `push_network_state`:
    ```rust
    fn push_volume_state(&mut self, volume: f64, muted: bool) {
        for (_, module) in &self.modules {
            if module.id().0 == "volume" {
                self.shared.modules.insert(
                    module.id(),
                    ModuleState::Volume(VolumeState { volume, muted }),
                );
            }
        }
    }
    ```

  - [x] 5.8: Add volume subscription in `App::subscription()` — after `network_sub`:
    ```rust
    // AC #9: Only subscribe if volume module is present.
    // No zbus connection needed — PA auto-discovers its socket.
    let volume_sub = if self.modules.iter().any(|(_, m)| m.id().0 == "volume") {
        crate::dbus::wireplumber::volume_subscription()
    } else {
        iced::Subscription::none()
    };
    ```
    Include `volume_sub` in **all** `iced::Subscription::batch(...)` calls (both the animating and
    non-animating branches).

  - [x] 5.9: Add Story 5.4 tests to `#[cfg(test)]` block:
    ```rust
    // Story 5.4 tests

    fn make_volume_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams, VolumeConfig};
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "volume".into(),
            zone: Zone::Bottom,
            params: ModuleParams::Volume(VolumeConfig { step: 5 }),
        }];
        App::new(config)
    }

    #[test]
    fn volume_state_changed_updates_module() {
        let mut app = make_volume_app();
        let _ = app.update(Message::Volume(VolumeEvent::StateChanged {
            volume: 60.0,
            muted: false,
        }));
        let id = ModuleId::new("volume");
        match app.shared.modules.get(&id) {
            Some(ModuleState::Volume(s)) => {
                assert!((s.volume - 60.0).abs() < 0.001);
                assert!(!s.muted);
            }
            other => panic!("expected Volume state, got {:?}", other),
        }
    }

    #[test]
    fn volume_unavailable_marks_module_unavailable() {
        let mut app = make_volume_app();
        let _ = app.update(Message::Volume(VolumeEvent::Unavailable));
        let id = ModuleId::new("volume");
        assert!(matches!(
            app.shared.modules.get(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn volume_state_not_updated_without_volume_module() {
        let mut app = App::new(Config::default()); // no volume module
        let _ = app.update(Message::Volume(VolumeEvent::StateChanged {
            volume: 50.0,
            muted: false,
        }));
        let id = ModuleId::new("volume");
        assert!(!app.shared.modules.contains_key(&id));
    }
    ```

- [x] Task 6: Register `pub mod wireplumber` in `src/dbus/mod.rs` (AC: #7)
  - [x] 6.1: Add after `pub mod networkmanager;`:
    ```rust
    pub mod wireplumber;
    ```

- [x] Task 7: Quality gates (AC: #10, #11)
  - [x] 7.1: `cargo build` — succeeds
  - [x] 7.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 7.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 7.4: `cargo test` — all 215 pre-existing tests pass + 14 new tests (229 total)

## Dev Notes

### What Exists After Story 5.3

- `src/dbus/mod.rs`: `pub mod upower;` + `pub mod networkmanager;` declared; `DbusConnections { session, system }` + `dbus_init_subscription()`
- `crates/niribar/src/app.rs`:
  - `Message::Battery(BatteryEvent)` + `Message::Network(NetworkEvent)` handled
  - `push_battery_state()` + `push_network_state()` helpers — copy exactly for `push_volume_state()`
  - `subscription()` has `battery_sub` + `network_sub` guards — add `volume_sub`
  - 215 tests passing
- `src/modules/mod.rs`: `pub mod battery;` + `pub mod network;` declared; `ModuleState::Battery` + `ModuleState::Network`; registry has `"battery"` and `"network"`
- `src/config/mod.rs`: `BatteryConfig`, `NetworkConfig`; `ModuleParams::Battery`, `ModuleParams::Network` — add `VolumeConfig` and `ModuleParams::Volume` in the same pattern

### ⚠️ Critical: WirePlumber Does NOT Have a D-Bus Volume API

**WirePlumber 0.5.13 does NOT register a D-Bus service for volume control.** Despite the architecture doc naming the file `wireplumber.rs`, the implementation must use the **PulseAudio compatibility layer**, NOT zbus.

- WirePlumber only registers `org.freedesktop.ReserveDevice1.AudioN` on D-Bus (device reservation only — no volume)
- `wpctl` communicates with WirePlumber via the WirePlumber C API through a PipeWire socket, not D-Bus
- `libpulse-binding 2.30.1` wraps `libpulse.so` which connects to the PA compat socket at `/run/user/1000/pulse/native`
- PipeWire emulates PulseAudio via `pipewire-pulse` (confirmed active on this dev machine)

**Add to Cargo.toml:**
```toml
libpulse-binding = "2.30.1"
```
**Do NOT add** `libpulse-tokio` — we bridge PA's threaded mainloop to tokio via `std::thread::spawn` + `tokio::sync::mpsc` (same pattern as battery/network but with a std thread instead of tokio::spawn).

### ⚠️ Critical: Volume Subscription Needs No zbus Connection

Unlike battery (UPower on session bus) and network (NetworkManager/networkd on system bus), the volume subscription uses NO zbus connection at all.

- `VolumeConnData` is a unit struct (hashed to constant string)
- `volume_subscription()` takes **no argument** (PA auto-discovers its socket via `PULSE_SERVER` or `/run/user/UID/pulse/native`)
- In `subscription()`: only check `if self.modules.iter().any(...)` — no connection guard needed
- `needs_dbus` check in subscription already includes `"volume"` (see existing code) — no change needed there

### Scroll Interaction — First Module with Write Operations

Volume is the **first module with scroll interaction** in this codebase. This requires:

1. **Two new `Message` variants:** `ModuleScrollUp(ModuleId)` and `ModuleScrollDown(ModuleId)`
2. **Two new `App::update` handlers** (same pattern as `ModuleClick`)
3. **`on_scroll()` added to `mouse_area`** in `App::view()` for ALL modules (non-volume modules return `None` from `on_interaction` → `Task::none()`)

**Scroll delta convention in iced:** Positive `y` = scroll up (wheel forward).

### Volume Adjustment via wpctl Subprocess

Volume **setting** uses `wpctl set-volume` via `tokio::task::spawn` (fire-and-forget):
```
wpctl set-volume -l 1.5 @DEFAULT_AUDIO_SINK@ 5%+   ← increase, cap at 150%
wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-            ← decrease
```
No `-l` flag needed for decreasing (can't go below 0). The PA subscription detects the change and emits `VolumeEvent::StateChanged` automatically — no explicit update needed.

`wpctl` is part of the `wireplumber` package and always available when WirePlumber is installed.

### PulseAudio compat API Details (libpulse-binding 2.30.1)

| Operation | API |
|---|---|
| Create mainloop | `Mainloop::new()` → `Option<Mainloop>` |
| Create context | `Context::new(&mainloop, "niribar")` → `Option<Context>` |
| Connect | `context.connect(None, FlagSet::NOAUTOSPAWN, None)` |
| Start mainloop thread | `mainloop.start()` → `Result<(), ()>` |
| Wait in lock loop | `mainloop.wait()` (only valid while mainloop is locked) |
| Get state | `context.get_state()` → `pulse::context::State` |
| Get introspect | `ctx.introspect()` → `Introspect` (by value, use fresh each call) |
| Query default sink | `introspect.get_sink_info_by_name("@DEFAULT_SINK@", cb)` |
| Read volume | `SinkInfo::volume.avg()` → `VolumeLinear`; `.0 * 100.0` = percentage |
| Read mute | `SinkInfo::mute: bool` |
| Subscribe | `ctx.subscribe(InterestMaskSet::SINKS \| InterestMaskSet::SERVER, \|_\| {})` |
| Change callback | `ctx.set_subscribe_callback(Some(Box::new(\|facility, op, _idx\| {...})))` |

**Volume scale:** `VolumeLinear::0` is `f64` 0.0–1.0 (1.0 = 100%). Multiply by 100 for percentage display. Same result as `wpctl get-volume` output.

**`"@DEFAULT_SINK@"` alias:** Use this directly in `get_sink_info_by_name` — no need to call `get_server_info` first. PA resolves this alias to the current default output.

### Threaded Mainloop Lifecycle

`Mainloop::start()` spawns PA's own thread. `run_pa_thread` must:
1. Set up everything while mainloop is locked
2. Unlock and call `start()`
3. Block in a polling loop until `context.get_state()` is Failed/Terminated
4. Send `Err(())` to signal stream end

The 5-second polling loop for context state is acceptable — it's only checking if PA has died (rare), not polling for volume changes (those come via callbacks).

### Context Clone

In `libpulse_binding 2.30`, `Context` IS `Clone` (Arc-backed internally). You can call `context.clone()` in the subscribe callback setup. The `set_subscribe_callback` closure receives a clone of `ctx_cb`.

If compilation fails with "Context is not Clone", use a different pattern:
- Store callback state in a `Arc<Mutex<...>>` shared between the setup code and the callback

### Icons and Colors (Catppuccin Mocha)

| State | Icon | Color | Hex |
|---|---|---|---|
| Muted / 0% | "✕" | `color.overlay0` | `#6c7086` |
| Low (1–32%) | "░" | `color.accent` | `#89b4fa` |
| Medium (33–65%) | "▓" | `color.accent` | `#89b4fa` |
| High (>65%) | "◈" | `color.accent` | `#89b4fa` |

UX-DR5 "dim" state = muted audio (only case where non-critical state uses overlay0).

### File Locations

| What | Where |
|---|---|
| `VolumeConfig`, `ModuleParams::Volume` | `src/config/mod.rs` |
| `parse_volume_params`, parse chain | `src/config/parse.rs` |
| `VolumeState`, `VolumeEvent`, `VolumeModule` | `crates/niribar/src/modules/volume.rs` (new) |
| `ModuleState::Volume(...)`, `volume_ctor`, `pub mod volume` | `src/modules/mod.rs` |
| `pub mod wireplumber` | `src/dbus/mod.rs` |
| PA subscription, `volume_subscription()`, `run_pa_thread`, `fetch_volume` | `crates/niribar/src/dbus/wireplumber.rs` (new) |
| `Message::Volume`, `Message::VolumeAdjust`, `Message::ModuleScrollUp/Down`, handlers, sub, `push_volume_state` | `crates/niribar/src/app.rs` |

### Test Count

Pre-existing: 215 | New minimum: 11 | Target total: ≥226

| File | Count | Tests |
|---|---|---|
| `crates/niribar/src/modules/volume.rs` | 8 | id, state defaults, tier helpers (muted, zero, low, medium, high) |
| `crates/niribar/src/dbus/wireplumber.rs` | 3 | PA linear volume scale conversions (pure math, no live PA needed) |
| `crates/niribar/src/app.rs` | 3 | state_changed, unavailable, no_module |

Note: Tests for the PA mainloop itself are integration-level and require a live PA session — not required for unit test suite.

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

All 7 tasks and subtasks implemented and verified:
- `VolumeConfig` (step: u8, default 5) added to config system with `parse_volume_params()`
- `crates/niribar/src/modules/volume.rs`: VolumeState, VolumeEvent, VolumeModule with `on_interaction` for ScrollUp/Down → VolumeAdjust
- `crates/niribar/src/dbus/wireplumber.rs`: PA compat subscription via `std::thread::spawn` + `tokio::sync::mpsc` bridge; used `Rc<RefCell<Context>>` since Context is not Clone; `InterestMaskSet::SINK` (not SINKS)
- `src/modules/mod.rs`: `pub mod volume`, `ModuleState::Volume`, `volume_ctor` registered
- `crates/niribar/src/app.rs`: `Message::Volume/VolumeAdjust/ModuleScrollUp/Down`, handlers, `push_volume_state`, `volume_sub`, scroll `on_scroll()` on all mouse_areas
- `src/dbus/mod.rs`: `pub mod wireplumber` registered
- Quality gates: `cargo build` ✓, `cargo clippy -- -D warnings` ✓ (0 warnings), `cargo fmt --check` ✓, `cargo test` ✓ (229 tests: 215 pre-existing + 14 new)

Key deviation from story template: `format!("{step}%+")` used instead of `format!("{}%+", step)` per clippy `uninlined_format_args`; `match` converted to `if let` per `single_match`.

### File List

- Cargo.toml
- Cargo.lock
- src/config/mod.rs
- src/config/parse.rs
- src/modules/mod.rs
- crates/niribar/src/modules/volume.rs (new)
- src/dbus/mod.rs
- crates/niribar/src/dbus/wireplumber.rs (new)
- crates/niribar/src/app.rs

## Change Log

- 2026-04-05: Story 5.4 implemented — volume module with PA compat subscription, scroll interaction, wpctl volume adjustment. 14 new tests added (229 total). (claude-sonnet-4-6)
