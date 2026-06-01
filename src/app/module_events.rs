use iced::Task;

use super::messages::ModuleRuntimeEvent;
use super::{App, Message};
use crate::modules::battery::BatteryEvent;
use crate::modules::button::{ButtonEvent, ButtonState};
use crate::modules::clock::{ClockEvent, ClockState};
use crate::modules::network::NetworkEvent;
use crate::modules::script::{ScriptEvent, ScriptState};
use crate::modules::volume::VolumeEvent;
use crate::modules::{InteractionEvent, ModuleEvent, ModuleId, ModuleState};

fn insert_module_state_if_changed(
    modules: &mut super::module_runtime::ModuleRuntime,
    id: ModuleId,
    state: ModuleState,
) {
    if modules.state(&id) != Some(&state) {
        modules.set_state(id, state);
    }
}

impl App {
    pub(super) fn handle_module_runtime_event(
        &mut self,
        event: ModuleRuntimeEvent,
    ) -> Task<Message> {
        tracing::trace!(
            target: "niribar::modules",
            module_event = event.diagnostic_label(),
            "module runtime event received"
        );
        match event {
            ModuleRuntimeEvent::Module(id, event) => self.handle_module_event(id, event),
            ModuleRuntimeEvent::Detail { target, message } => {
                self.handle_module_detail_message(target, message)
            }
            ModuleRuntimeEvent::ModuleClick(id, window) => self.dispatch_module_interaction(
                &id,
                crate::modules::InteractionEvent::Clicked,
                window,
            ),
            ModuleRuntimeEvent::ModuleRightClick(id, window) => self.dispatch_module_interaction(
                &id,
                crate::modules::InteractionEvent::RightClicked(window),
                window,
            ),
            ModuleRuntimeEvent::ModuleScrollUp(id, window) => self.dispatch_module_interaction(
                &id,
                crate::modules::InteractionEvent::ScrollUp,
                window,
            ),
            ModuleRuntimeEvent::ModuleScrollDown(id, window) => self.dispatch_module_interaction(
                &id,
                crate::modules::InteractionEvent::ScrollDown,
                window,
            ),
            ModuleRuntimeEvent::Battery(event) => self.handle_battery_event(event),
            ModuleRuntimeEvent::Network(event) => self.handle_network_event(event),
            ModuleRuntimeEvent::Volume(event) => self.handle_volume_event(event),
        }
    }

    pub(super) fn handle_module_event(
        &mut self,
        id: ModuleId,
        event: ModuleEvent,
    ) -> Task<Message> {
        let module_kind = self
            .modules
            .get(&id)
            .map(|module| module.kind().runtime_name())
            .unwrap_or("unknown");
        tracing::trace!(
            target: "niribar::modules",
            module_name = self.modules.diagnostic_name_for(&id),
            module_kind,
            module_event = super::messages::module_event_diagnostic_label(&event),
            "module event received"
        );
        match event {
            ModuleEvent::Clock(ClockEvent::Tick { time, date }) => {
                insert_module_state_if_changed(
                    &mut self.modules,
                    id,
                    ModuleState::Clock(ClockState { time, date }),
                );
            }
            ModuleEvent::Script(ScriptEvent::Output(text)) => {
                insert_module_state_if_changed(
                    &mut self.modules,
                    id,
                    ModuleState::Script(ScriptState { output: text }),
                );
            }
            ModuleEvent::Script(ScriptEvent::Failed) => {
                self.modules.set_state(id, ModuleState::Unavailable);
            }
            ModuleEvent::Button(ButtonEvent::Ready) => {
                insert_module_state_if_changed(
                    &mut self.modules,
                    id,
                    ModuleState::Button(ButtonState),
                );
            }
        }
        Task::none()
    }

    pub(super) fn handle_module_detail_message(
        &mut self,
        target: ModuleId,
        message: crate::modules::ModuleDetailMessage,
    ) -> Task<Message> {
        if !self.panel.active_module_panel_contains(&target) {
            return Task::none();
        }

        let Some(module) = self.modules.get_mut(&target) else {
            return Task::none();
        };
        let Some(session) = self.panel.module_detail_session_for_mut(&target) else {
            return Task::none();
        };

        let update = module.update_module_detail_session(session, message);
        self.apply_module_detail_update(update)
    }

    fn apply_module_detail_update(
        &mut self,
        update: crate::modules::ModuleDetailUpdate,
    ) -> Task<Message> {
        let mut tasks = Vec::new();
        for effect in update.effects {
            match effect {
                crate::modules::ModuleDetailEffect::App(app_effect) => {
                    tasks.push(self.apply_module_detail_app_effect(app_effect));
                }
                crate::modules::ModuleDetailEffect::Module(_module_effect) => {}
            }
        }
        Task::batch(tasks)
    }

    fn apply_module_detail_app_effect(
        &mut self,
        effect: crate::modules::ModuleDetailAppEffect,
    ) -> Task<Message> {
        match effect {
            crate::modules::ModuleDetailAppEffect::PanelAction(action) => {
                self.dispatch_panel_action_from_active_panel(action)
            }
            crate::modules::ModuleDetailAppEffect::ClosePanel => {
                Task::batch(self.close_all_panels())
            }
        }
    }

    pub(super) fn dispatch_module_interaction(
        &mut self,
        id: &ModuleId,
        event: InteractionEvent,
        origin_window: iced::window::Id,
    ) -> Task<Message> {
        let trigger = match &event {
            InteractionEvent::Clicked => Some(crate::config::ActionTrigger::Click),
            InteractionEvent::RightClicked(_) => Some(crate::config::ActionTrigger::RightClick),
            _ => None,
        };
        if let Some(action) =
            trigger.and_then(|trigger| self.modules.action_for(id, &trigger).cloned())
        {
            tracing::debug!(
                target: "niribar::modules",
                module_name = self.modules.diagnostic_name_for(id),
                has_anchor = super::panel::panel_action_needs_anchor(&action),
                "dispatching configured module action"
            );
            let origin = crate::surface::panel::PanelOrigin::Module(id.clone());
            if super::panel::panel_action_needs_anchor(&action) {
                let target = super::widget_ops::module_widget_id(origin_window, id);
                let op = super::widget_ops::FindBounds {
                    target,
                    result: None,
                };
                return iced::advanced::widget::operate(op).then(move |bounds| {
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
                });
            }

            return self.dispatch_panel_action(origin_window, origin, action);
        }

        if let Some(module) = self.modules.get(id)
            && let Some(msg) = module.on_interaction(event)
        {
            tracing::trace!(
                target: "niribar::modules",
                module_name = self.modules.diagnostic_name_for(id),
                module_kind = module.kind().runtime_name(),
                "module interaction produced app message"
            );
            return Task::done(msg);
        }
        Task::none()
    }

    fn apply_module_data_source_state(
        &mut self,
        state: crate::modules::data_source::ModuleDataSourceState,
    ) {
        let outcomes = self.data_sources.apply_source_state(state, &self.modules);
        self.modules.apply_outcomes(outcomes);
    }

    fn apply_module_data_source_unavailable(
        &mut self,
        identity: crate::modules::data_source::ModuleDataSourceIdentity,
    ) {
        let outcomes = self.data_sources.mark_unavailable(identity, &self.modules);
        self.modules.apply_outcomes(outcomes);
    }

    pub(super) fn handle_battery_event(&mut self, event: BatteryEvent) -> Task<Message> {
        match event {
            BatteryEvent::StateChanged {
                percentage,
                is_charging,
            } => self.apply_module_data_source_state(
                crate::modules::data_source::ModuleDataSourceState::Battery(
                    crate::modules::data_source::BatteryReading {
                        percentage,
                        is_charging,
                    },
                ),
            ),
            BatteryEvent::Unavailable => self.apply_module_data_source_unavailable(
                crate::modules::data_source::ModuleDataSourceIdentity::Battery,
            ),
        }
        Task::none()
    }

    pub(super) fn handle_network_event(&mut self, event: NetworkEvent) -> Task<Message> {
        match event {
            NetworkEvent::StateChanged {
                connectivity,
                label,
            } => self.apply_module_data_source_state(
                crate::modules::data_source::ModuleDataSourceState::Network(
                    crate::modules::data_source::NetworkReading {
                        connectivity,
                        label,
                    },
                ),
            ),
            NetworkEvent::Unavailable => self.apply_module_data_source_unavailable(
                crate::modules::data_source::ModuleDataSourceIdentity::Network,
            ),
        }
        Task::none()
    }

    pub(super) fn handle_volume_event(&mut self, event: VolumeEvent) -> Task<Message> {
        match event {
            VolumeEvent::StateChanged { volume, muted } => self.apply_module_data_source_state(
                crate::modules::data_source::ModuleDataSourceState::Volume(
                    crate::modules::data_source::VolumeReading { volume, muted },
                ),
            ),
            VolumeEvent::Unavailable => self.apply_module_data_source_unavailable(
                crate::modules::data_source::ModuleDataSourceIdentity::Volume,
            ),
        }
        Task::none()
    }

    pub(super) fn handle_volume_adjust(&self, step: u8, up: bool) -> Task<Message> {
        tracing::debug!(
            target: "niribar::modules",
            step,
            up,
            "volume adjustment requested"
        );
        let arg = if up {
            format!("{step}%+")
        } else {
            format!("{step}%-")
        };
        tokio::task::spawn(async move {
            match crate::command::set_default_sink_volume(&arg).await {
                Ok(status) if status.success() => {}
                Ok(status) => tracing::warn!(
                    target: "niribar::modules",
                    ?status,
                    "wpctl set-volume exited unsuccessfully"
                ),
                Err(err) => tracing::warn!(
                    target: "niribar::modules",
                    error = %err,
                    "failed to run wpctl set-volume"
                ),
            }
        });
        Task::none()
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

        fn view_collapsed_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &Style,
        ) -> iced::Element<'_, Message> {
            iced::widget::text("").into()
        }

        fn view_panel_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &Style,
        ) -> iced::Element<'_, Message> {
            iced::widget::text("").into()
        }

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
                        crate::config::PanelAction::Command(crate::config::CommandConfig::Shell(
                            command.clone(),
                        )),
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
        app.panel
            .module_detail_session_for(&ModuleId::new(id))
            .and_then(|session| session.downcast_ref::<u32>())
            .copied()
    }

    fn unique_tmp_path(name: &str) -> String {
        format!("/tmp/niribar-{name}-{}", std::process::id())
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
        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("detail"),
            message: ModuleDetailMessage::new(DetailTestMessage::ModuleOwned),
        }));
        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Detail {
            target: ModuleId::new("detail"),
            message: ModuleDetailMessage::new(DetailTestMessage::ClosePanel),
        }));
        assert_eq!(app.surfaces.panel_popup_ids().count(), 0);
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
