use iced::Task;

use super::app_effects::AppEffect;
use super::messages::ModuleRuntimeEvent;
use super::{App, Message};
use crate::modules::button::{ButtonEvent, ButtonState};
use crate::modules::clock::{ClockEvent, ClockState};
use crate::modules::script::{ScriptEvent, ScriptState};
use crate::modules::{InteractionEvent, ModuleEvent, ModuleId, ModuleState};

fn apply_module_state(
    modules: &mut super::module_runtime::ModuleRuntime,
    id: ModuleId,
    state: ModuleState,
) {
    let _ =
        modules.handle(super::module_runtime::ModuleRuntimeIntent::ApplyModuleState { id, state });
}

impl App {
    pub(super) fn handle_module_runtime_event(
        &mut self,
        event: ModuleRuntimeEvent,
    ) -> Task<Message> {
        tracing::trace!(
            target: "canopa_shell::modules",
            module_event = event.diagnostic_label(),
            "module runtime event received"
        );
        match event {
            ModuleRuntimeEvent::Stasis(value) => {
                let transition = self.data_sources.handle_stasis_event(value);
                self.apply_module_data_source_transition(transition)
            }
            ModuleRuntimeEvent::StasisToggleFinished(success) => {
                self.data_sources.stasis_toggle_pending = false;
                if !success {
                    tracing::warn!("Stasis manual hold request failed");
                }
                Task::none()
            }
            ModuleRuntimeEvent::ToggleStasis => {
                if self.data_sources.stasis_toggle_pending
                    || !self.modules.has_data_source(
                        crate::modules::data_source::ModuleDataSourceIdentity::Stasis,
                    )
                {
                    return Task::none();
                }
                self.data_sources.stasis_toggle_pending = true;
                Task::perform(
                    async {
                        tokio::time::timeout(
                            std::time::Duration::from_secs(3),
                            tokio::process::Command::new(canopa_widgets::idle::companion())
                                .arg("toggle")
                                .stdin(std::process::Stdio::null())
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .kill_on_drop(true)
                                .status(),
                        )
                        .await
                        .is_ok_and(|r| r.is_ok_and(|s| s.success()))
                    },
                    |success| Message::Modules(ModuleRuntimeEvent::StasisToggleFinished(success)),
                )
            }
            ModuleRuntimeEvent::ToggleIdleInhibitor => {
                if self
                    .modules
                    .has_kind(crate::modules::ModuleKind::IdleInhibitor)
                {
                    self.data_sources.idle_inhibitor.toggle();
                }
                Task::none()
            }
            ModuleRuntimeEvent::IdleInhibitorApplied {
                window,
                requested,
                available,
            } => {
                self.data_sources
                    .idle_inhibitor
                    .completed(window, requested, available);
                Task::none()
            }
            ModuleRuntimeEvent::Module(id, event) => self.handle_module_event(id, event),
            ModuleRuntimeEvent::Detail { target, message } => {
                self.handle_module_detail_message(target, message)
            }
            ModuleRuntimeEvent::ModuleClick(id, window) => {
                // A click answers the tooltip's question: it leaves with it.
                let mut tasks = self.close_tooltip_popup_if_open(window);
                self.surface_mut(window).tooltip_pending = None;
                tasks.push(self.dispatch_module_interaction(
                    &id,
                    crate::modules::InteractionEvent::Clicked,
                    window,
                ));
                Task::batch(tasks)
            }
            ModuleRuntimeEvent::ModuleRightClick(id, window) => self.dispatch_module_interaction(
                &id,
                crate::modules::InteractionEvent::RightClicked(window),
                window,
            ),
            ModuleRuntimeEvent::ModuleScroll(id, window, delta) => {
                match self.module_scroll.step(&id, delta) {
                    Some(true) => self.dispatch_module_interaction(
                        &id,
                        crate::modules::InteractionEvent::ScrollUp,
                        window,
                    ),
                    Some(false) => self.dispatch_module_interaction(
                        &id,
                        crate::modules::InteractionEvent::ScrollDown,
                        window,
                    ),
                    None => Task::none(),
                }
            }
            ModuleRuntimeEvent::Battery(event) => self.handle_battery_event(event),
            ModuleRuntimeEvent::Network(event) => self.handle_network_event(event),
            ModuleRuntimeEvent::Volume(event) => self.handle_volume_event(event),
            ModuleRuntimeEvent::Media(snapshot) => {
                let transition = self.data_sources.handle_media_event(snapshot);
                self.apply_module_data_source_transition(transition)
            }
            ModuleRuntimeEvent::Notifications(summary) => {
                tracing::debug!(target: "canopa_shell::notifications", available = summary.is_some(),
                    requires_attention = summary.map_or(0, |s| s.requires_attention),
                    dnd = summary.is_some_and(|s| s.do_not_disturb), "notification summary updated");
                let transition = self.data_sources.handle_notifications_event(summary);
                self.apply_module_data_source_transition(transition)
            }
        }
    }

    pub(super) fn handle_module_event(
        &mut self,
        id: ModuleId,
        event: ModuleEvent,
    ) -> Task<Message> {
        let module_kind = self
            .modules
            .kind_for(&id)
            .map(|kind| kind.runtime_name())
            .unwrap_or("unknown");
        let module_name = self.modules.configured_module_name_for_instance(&id);
        tracing::trace!(
            target: "canopa_shell::modules",
            module_name,
            module_kind,
            module_event = super::messages::module_event_diagnostic_label(&event),
            "module event received"
        );
        match event {
            ModuleEvent::Clock(ClockEvent::Tick { time, date }) => {
                apply_module_state(
                    &mut self.modules,
                    id,
                    ModuleState::Clock(ClockState { time, date }),
                );
            }
            ModuleEvent::Script(ScriptEvent::Output(text)) => {
                apply_module_state(
                    &mut self.modules,
                    id,
                    ModuleState::Script(ScriptState { output: text }),
                );
            }
            ModuleEvent::Script(ScriptEvent::Failed) => {
                apply_module_state(&mut self.modules, id, ModuleState::Unavailable);
            }
            ModuleEvent::Button(ButtonEvent::Ready) => {
                apply_module_state(&mut self.modules, id, ModuleState::Button(ButtonState));
            }
        }
        Task::none()
    }

    pub(super) fn handle_module_detail_message(
        &mut self,
        target: ModuleId,
        message: crate::modules::ModuleDetailMessage,
    ) -> Task<Message> {
        let Some(session) = self
            .foreground
            .active_panel_module_detail_session_mut(&target)
        else {
            return Task::none();
        };

        let effects = self.modules.handle(
            super::module_runtime::ModuleRuntimeIntent::ModuleDetailUpdate {
                target,
                session,
                message,
            },
        );
        self.apply_module_runtime_effects(effects)
    }

    pub(super) fn dispatch_module_interaction(
        &mut self,
        id: &ModuleId,
        event: InteractionEvent,
        origin_window: iced::window::Id,
    ) -> Task<Message> {
        // Re-activating the module that opened a panel dismisses it before
        // requesting bounds or starting another provider process.
        let trigger = match event {
            InteractionEvent::Clicked => Some(crate::config::ActionTrigger::Click),
            InteractionEvent::RightClicked(_) => Some(crate::config::ActionTrigger::RightClick),
            _ => None,
        };
        if let Some(action) = trigger.and_then(|trigger| self.modules.action_for(id, &trigger)) {
            let pending_provider = match action {
                crate::config::PanelAction::OpenProvider(name) => {
                    self.external_panel_session.pending_matches_panel(
                        &super::external_panel_session::ExternalPanelPanelIdentity {
                            origin_window,
                            origin: crate::surface::panel::PanelOrigin::Module(id.clone()),
                            provider_name: name.clone(),
                        },
                    )
                }
                _ => false,
            };
            if pending_provider
                || self
                    .foreground
                    .module_action_targets_active_panel(origin_window, id, action)
            {
                return Task::batch(self.close_active_panel());
            }
        }
        let effects = self.modules.handle(
            super::module_runtime::ModuleRuntimeIntent::ModuleInteraction {
                id: id.clone(),
                event,
                origin_window,
            },
        );
        self.apply_module_runtime_effects(effects)
    }

    pub(super) fn dispatch_keyboard_secondary_module_interaction(
        &mut self,
        id: &ModuleId,
        window: iced::window::Id,
        screen_size: (u32, u32),
    ) -> Task<Message> {
        let effects = self.modules.handle(
            super::module_runtime::ModuleRuntimeIntent::ModuleInteraction {
                id: id.clone(),
                event: InteractionEvent::Activated(window),
                origin_window: window,
            },
        );
        self.apply_keyboard_secondary_module_runtime_effects(effects, window, screen_size)
    }

    fn module_runtime_effect_to_task(
        &mut self,
        effect: super::module_runtime::ModuleRuntimeEffect,
    ) -> Task<Message> {
        match effect {
            super::module_runtime::ModuleRuntimeEffect::RunPanelAction {
                origin_window,
                origin,
                action,
            } => self.dispatch_panel_action(origin_window, origin, action),
            super::module_runtime::ModuleRuntimeEffect::RequestActionAnchor {
                origin_window,
                module,
                action,
            } => {
                let origin = crate::surface::panel::PanelOrigin::Module(module.clone());
                let target = super::widget_ops::module_widget_id(origin_window, &module);
                let op = super::widget_ops::FindBounds {
                    target,
                    result: None,
                };
                iced::advanced::widget::operate(op).then(move |bounds| {
                    let event = super::panel::panel_event_for_action(
                        origin_window,
                        origin.clone(),
                        action.clone(),
                        bounds,
                    );
                    match event {
                        Some(event) => Task::done(Message::Panel(event)),
                        None => Task::none(),
                    }
                })
            }
            super::module_runtime::ModuleRuntimeEffect::EmitMessage(message) => Task::done(message),
            super::module_runtime::ModuleRuntimeEffect::ClearSurfaceModuleRefs(ids) => {
                self.clear_surface_module_refs(&ids);
                Task::none()
            }
            super::module_runtime::ModuleRuntimeEffect::RequestForegroundPanelAction { action } => {
                let effects = self.foreground.handle(
                    super::foreground::ForegroundIntent::PanelActionRequested(action),
                );
                Task::batch(self.apply_foreground_effects(effects))
            }
            super::module_runtime::ModuleRuntimeEffect::CloseActivePanel => {
                Task::batch(self.close_active_panel())
            }
        }
    }

    fn keyboard_secondary_module_runtime_effect_to_task(
        &mut self,
        effect: super::module_runtime::ModuleRuntimeEffect,
        window: iced::window::Id,
        screen_size: (u32, u32),
    ) -> Task<Message> {
        match effect {
            super::module_runtime::ModuleRuntimeEffect::EmitMessage(Message::Tray(
                super::messages::TrayRuntimeEvent::MenuRequestKeyboard {
                    service,
                    menu_path,
                    source_window,
                },
            )) => {
                let target = crate::modules::tray::tray_icon_widget_id(window, &service);
                let op = super::widget_ops::FindBounds {
                    target,
                    result: None,
                };
                let svc = service.clone();
                let mp = menu_path.clone();
                iced::advanced::widget::operate(op).then(move |bounds| {
                    Task::done(Message::Tray(
                        super::messages::TrayRuntimeEvent::MenuRequestWithBounds {
                            service: svc.clone(),
                            menu_path: mp.clone(),
                            bounds,
                            screen_size,
                            source_window,
                        },
                    ))
                })
            }
            other => self.module_runtime_effect_to_task(other),
        }
    }

    fn apply_keyboard_secondary_module_runtime_effects(
        &mut self,
        effects: Vec<super::module_runtime::ModuleRuntimeEffect>,
        window: iced::window::Id,
        screen_size: (u32, u32),
    ) -> Task<Message> {
        let mut tasks = Vec::new();
        for effect in effects {
            tasks.push(self.keyboard_secondary_module_runtime_effect_to_task(
                effect,
                window,
                screen_size,
            ));
        }
        Task::batch(tasks)
    }

    fn apply_module_runtime_effects(
        &mut self,
        effects: Vec<super::module_runtime::ModuleRuntimeEffect>,
    ) -> Task<Message> {
        let mut tasks = Vec::new();
        for effect in effects {
            tasks.push(self.module_runtime_effect_to_task(effect));
        }
        Task::batch(tasks)
    }

    pub(super) fn apply_module_data_source_transition(
        &mut self,
        transition: super::module_data_sources::ModuleDataSourceTransition,
    ) -> Task<Message> {
        let intent = match transition {
            super::module_data_sources::ModuleDataSourceTransition::Available(state) => {
                super::module_runtime::ModuleRuntimeIntent::ApplyDataSourceState(state)
            }
            super::module_data_sources::ModuleDataSourceTransition::Unavailable(identity) => {
                super::module_runtime::ModuleRuntimeIntent::MarkDataSourceUnavailable(identity)
            }
        };
        let effects = self.modules.handle(intent);
        self.apply_module_runtime_effects(effects)
    }

    pub(super) fn handle_battery_event(
        &mut self,
        event: crate::modules::battery::BatteryEvent,
    ) -> Task<Message> {
        let transition = self.data_sources.handle_battery_event(event);
        self.apply_module_data_source_transition(transition)
    }

    pub(super) fn handle_network_event(
        &mut self,
        event: crate::modules::network::NetworkEvent,
    ) -> Task<Message> {
        let transition = self.data_sources.handle_network_event(event);
        self.apply_module_data_source_transition(transition)
    }

    pub(super) fn handle_volume_event(
        &mut self,
        event: crate::modules::volume::VolumeEvent,
    ) -> Task<Message> {
        let transition = self.data_sources.handle_volume_event(event);
        self.apply_module_data_source_transition(transition)
    }

    pub(super) fn handle_volume_adjust(&mut self, step: u8, up: bool) -> Task<Message> {
        tracing::debug!(
            target: "canopa_shell::modules",
            step,
            up,
            "volume adjustment requested"
        );
        Task::batch(self.apply_app_effects(vec![AppEffect::AdjustDefaultSinkVolume { step, up }]))
    }
}

/// Scrolled pixels that make one step, for touchpads and other smooth sources.
const SCROLL_PIXELS_PER_STEP: f32 = 20.0;

/// Turns scroll events over modules into Up/Down steps. A wheel notch is one
/// step. Smooth scrolling accumulates pixels, so a touchpad gesture is not a
/// step per event, and the zero-length event that ends it is not a step.
#[derive(Debug, Default)]
pub(super) struct ScrollSteps {
    pending: Option<(ModuleId, f32)>,
}

impl ScrollSteps {
    /// `Some(true)` for a step up, `Some(false)` for a step down.
    pub(super) fn step(&mut self, id: &ModuleId, delta: iced::mouse::ScrollDelta) -> Option<bool> {
        match delta {
            iced::mouse::ScrollDelta::Lines { y, .. } => {
                self.pending = None;
                (y != 0.0).then_some(y > 0.0)
            }
            iced::mouse::ScrollDelta::Pixels { y, .. } => {
                if y == 0.0 {
                    return None;
                }
                let pending = match self.pending.take() {
                    Some((module, pending)) if &module == id && (pending > 0.0) == (y > 0.0) => {
                        pending + y
                    }
                    _ => y,
                };
                if pending.abs() >= SCROLL_PIXELS_PER_STEP {
                    let remainder = pending - SCROLL_PIXELS_PER_STEP.copysign(pending);
                    self.pending = Some((id.clone(), remainder));
                    Some(pending > 0.0)
                } else {
                    self.pending = Some((id.clone(), pending));
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod scroll_step_tests {
    use super::ScrollSteps;
    use crate::modules::ModuleId;
    use iced::mouse::ScrollDelta;

    #[test]
    fn wheel_notches_step_and_zero_lines_do_not() {
        let id = ModuleId::new("volume");
        let mut steps = ScrollSteps::default();
        assert_eq!(
            steps.step(&id, ScrollDelta::Lines { x: 0.0, y: 1.0 }),
            Some(true)
        );
        assert_eq!(
            steps.step(&id, ScrollDelta::Lines { x: 0.0, y: -1.0 }),
            Some(false)
        );
        assert_eq!(steps.step(&id, ScrollDelta::Lines { x: 0.0, y: 0.0 }), None);
    }

    #[test]
    fn smooth_scrolling_accumulates_pixels_into_steps() {
        let id = ModuleId::new("minimap");
        let mut steps = ScrollSteps::default();
        let pixels = |y| ScrollDelta::Pixels { x: 0.0, y };
        assert_eq!(steps.step(&id, pixels(-8.0)), None);
        assert_eq!(steps.step(&id, pixels(-8.0)), None);
        assert_eq!(steps.step(&id, pixels(-8.0)), Some(false));
        // The gesture's end is not a step.
        assert_eq!(steps.step(&id, pixels(0.0)), None);
        // Reversing direction or moving to another module starts over.
        assert_eq!(steps.step(&id, pixels(12.0)), None);
        assert_eq!(steps.step(&ModuleId::new("volume"), pixels(12.0)), None);
        assert_eq!(
            steps.step(&ModuleId::new("volume"), pixels(12.0)),
            Some(true)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::messages::PanelEvent;
    use crate::config::{Config, ModuleActionsConfig, Style};
    use crate::modules::{
        Module, ModuleDetailAppEffect, ModuleDetailEffect, ModuleDetailMessage,
        ModuleDetailSession, ModuleDetailUpdate, ModuleId, ModuleKind, ModuleState, Zone,
    };

    #[derive(Debug)]
    enum DetailTestMessage {
        Add(u32),
        ClosePanel,
        RunCommand(String),
        ModuleOwned,
    }

    struct DetailTestModule {
        id: ModuleId,
    }

    impl DetailTestModule {
        fn new(id: &str) -> Self {
            Self {
                id: ModuleId::new(id),
            }
        }
    }

    impl Module for DetailTestModule {
        fn id(&self) -> ModuleId {
            self.id.clone()
        }

        fn kind(&self) -> ModuleKind {
            ModuleKind::Button
        }

        fn subscription(&self) -> iced::Subscription<Message> {
            iced::Subscription::none()
        }

        fn module_detail(&self) -> Option<&dyn crate::modules::detail::ModuleDetailLifecycle> {
            Some(self)
        }

        fn module_detail_mut(
            &mut self,
        ) -> Option<&mut (dyn crate::modules::detail::ModuleDetailLifecycle + '_)> {
            Some(self)
        }
    }

    impl crate::modules::detail::ModuleDetailLifecycle for DetailTestModule {
        fn create_module_detail_session(
            &self,
            _state: &ModuleState,
        ) -> Option<ModuleDetailSession> {
            Some(Box::new(0_u32))
        }

        fn update_module_detail_session(
            &mut self,
            session: &mut ModuleDetailSession,
            message: ModuleDetailMessage,
        ) -> ModuleDetailUpdate {
            let Some(message) = message.downcast_ref::<DetailTestMessage>() else {
                return ModuleDetailUpdate::default();
            };
            match message {
                DetailTestMessage::Add(amount) => {
                    *session.downcast_mut::<u32>().expect("u32 session") += amount;
                    ModuleDetailUpdate::default()
                }
                DetailTestMessage::ClosePanel => ModuleDetailUpdate {
                    effects: vec![ModuleDetailEffect::App(ModuleDetailAppEffect::ClosePanel)],
                },
                DetailTestMessage::RunCommand(command) => ModuleDetailUpdate {
                    effects: vec![ModuleDetailEffect::App(ModuleDetailAppEffect::PanelAction(
                        crate::config::PanelAction::Command(
                            crate::config::ConfiguredCommand::shell_for_test(command.clone()),
                        ),
                    ))],
                },
                DetailTestMessage::ModuleOwned => ModuleDetailUpdate {
                    effects: vec![ModuleDetailEffect::Module(ModuleDetailMessage::new(
                        "module-owned",
                    ))],
                },
            }
        }
    }

    impl crate::modules::render::ModuleRender for DetailTestModule {
        fn has_content(&self, _state: &ModuleState, _mode: crate::config::FormatMode) -> bool {
            // This fixture owns its content in the detail session.
            true
        }

        fn render_collapsed_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &Style,
        ) -> iced::Element<'_, Message> {
            iced::widget::text("").into()
        }

        fn render_panel_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &Style,
        ) -> iced::Element<'_, Message> {
            iced::widget::text("").into()
        }
    }

    fn app_with_detail_modules(ids: &[&str]) -> App {
        let (mut app, _) = App::boot(Config::default(), Vec::new());
        app.modules = super::super::module_runtime::ModuleRuntime::from_modules_with_actions(
            ids.iter()
                .map(|id| {
                    (
                        Zone::Center,
                        Box::new(DetailTestModule::new(id)) as Box<dyn Module>,
                        ModuleActionsConfig::default(),
                    )
                })
                .collect(),
        );
        app
    }

    fn open_module_detail(app: &mut App, origin_window: iced::window::Id, id: &str) {
        let id = ModuleId::new(id);
        let _ = app.update(Message::Panel(PanelEvent::Open {
            origin_window,
            origin: crate::surface::panel::PanelOrigin::Module(id.clone()),
            content: crate::surface::panel::PanelContent::ModulePanel(id),
            anchor: None,
        }));
    }

    fn open_bar_panel(app: &mut App, origin_window: iced::window::Id) {
        let _ = app.update(Message::Panel(PanelEvent::Open {
            origin_window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));
    }

    fn detail_session_value(app: &App, id: &str) -> Option<u32> {
        app.foreground
            .active_panel_module_detail_session(&ModuleId::new(id))
            .and_then(|session| session.downcast_ref::<u32>())
            .copied()
    }

    fn unique_tmp_path(name: &str) -> String {
        format!("/tmp/canopa-shell-{name}-{}", std::process::id())
    }

    fn wait_for_path(path: &str) -> bool {
        for _ in 0..50 {
            if std::path::Path::new(path).exists() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn opaque_detail_message_routes_to_active_originating_module() {
        let mut app = app_with_detail_modules(&["detail"]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        open_module_detail(&mut app, origin, "detail");

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("detail"),
            message: ModuleDetailMessage::new(DetailTestMessage::Add(7)),
        }));

        assert_eq!(detail_session_value(&app, "detail"), Some(7));
    }

    #[test]
    fn opaque_detail_message_routes_to_module_in_active_bar_panel() {
        let mut app = app_with_detail_modules(&["detail"]);
        app.config.bars[0].panel.modules.center = vec!["detail".into()];
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        open_bar_panel(&mut app, origin);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("detail"),
            message: ModuleDetailMessage::new(DetailTestMessage::Add(7)),
        }));

        assert_eq!(detail_session_value(&app, "detail"), Some(7));
    }

    #[test]
    fn bar_panel_routes_detail_messages_to_independent_module_sessions() {
        let mut app = app_with_detail_modules(&["first", "second"]);
        app.config.bars[0].panel.modules.start = vec!["first".into()];
        app.config.bars[0].panel.modules.end = vec!["second".into()];
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        open_bar_panel(&mut app, origin);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("first"),
            message: ModuleDetailMessage::new(DetailTestMessage::Add(7)),
        }));
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("second"),
            message: ModuleDetailMessage::new(DetailTestMessage::Add(11)),
        }));

        assert_eq!(detail_session_value(&app, "first"), Some(7));
        assert_eq!(detail_session_value(&app, "second"), Some(11));
    }

    #[test]
    fn opaque_detail_message_for_inactive_target_is_ignored() {
        let mut app = app_with_detail_modules(&["active", "inactive"]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        open_module_detail(&mut app, origin, "active");

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("inactive"),
            message: ModuleDetailMessage::new(DetailTestMessage::Add(7)),
        }));

        assert_eq!(detail_session_value(&app, "active"), Some(0));
    }

    #[test]
    fn stale_detail_message_after_replacement_does_not_mutate_active_session() {
        let mut app = app_with_detail_modules(&["first", "second"]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        open_module_detail(&mut app, origin, "first");
        open_module_detail(&mut app, origin, "second");

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("first"),
            message: ModuleDetailMessage::new(DetailTestMessage::Add(7)),
        }));

        assert_eq!(detail_session_value(&app, "second"), Some(0));
    }

    #[test]
    fn stale_detail_message_after_close_is_ignored() {
        let mut app = app_with_detail_modules(&["detail"]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        open_module_detail(&mut app, origin, "detail");
        let _ = app.update(Message::Panel(PanelEvent::Close));

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("detail"),
            message: ModuleDetailMessage::new(DetailTestMessage::Add(7)),
        }));

        assert_eq!(detail_session_value(&app, "detail"), None);
    }

    #[test]
    fn detail_update_effects_distinguish_app_owned_and_module_owned_work() {
        let mut app = app_with_detail_modules(&["detail"]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        open_module_detail(&mut app, origin, "detail");
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("detail"),
            message: ModuleDetailMessage::new(DetailTestMessage::ModuleOwned),
        }));
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("detail"),
            message: ModuleDetailMessage::new(DetailTestMessage::ClosePanel),
        }));
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            0
        );
    }

    #[test]
    fn app_owned_detail_panel_action_uses_central_command_runner() {
        let command_path = unique_tmp_path("detail-command");
        let _ = std::fs::remove_file(&command_path);
        let mut app = app_with_detail_modules(&["detail"]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        open_module_detail(&mut app, origin, "detail");

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("detail"),
            message: ModuleDetailMessage::new(DetailTestMessage::RunCommand(format!(
                "printf detail >{command_path}"
            ))),
        }));

        assert!(
            wait_for_path(&command_path),
            "app-owned detail command should create {command_path}"
        );
        let _ = std::fs::remove_file(&command_path);
    }
}
