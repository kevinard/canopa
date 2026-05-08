use iced::Task;

use super::messages::ModuleRuntimeEvent;
use super::{App, Message};
use crate::modules::battery::BatteryEvent;
use crate::modules::button::{ButtonEvent, ButtonState};
use crate::modules::clock::{ClockEvent, ClockState};
use crate::modules::network::NetworkEvent;
use crate::modules::script::{ScriptEvent, ScriptState};
use crate::modules::volume::VolumeEvent;
use crate::modules::{InteractionEvent, ModuleEvent, ModuleId, ModuleKind, ModuleState};

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
        match event {
            ModuleRuntimeEvent::Module(id, event) => self.handle_module_event(id, event),
            ModuleRuntimeEvent::ModuleClick(id, _) => {
                self.dispatch_module_interaction(&id, crate::modules::InteractionEvent::Clicked)
            }
            ModuleRuntimeEvent::ModuleRightClick(id, window) => self.dispatch_module_interaction(
                &id,
                crate::modules::InteractionEvent::RightClicked(window),
            ),
            ModuleRuntimeEvent::ModuleScrollUp(id, _) => {
                self.dispatch_module_interaction(&id, crate::modules::InteractionEvent::ScrollUp)
            }
            ModuleRuntimeEvent::ModuleScrollDown(id, _) => {
                self.dispatch_module_interaction(&id, crate::modules::InteractionEvent::ScrollDown)
            }
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

    pub(super) fn dispatch_module_interaction(
        &self,
        id: &ModuleId,
        event: InteractionEvent,
    ) -> Task<Message> {
        if let Some(command) = self.modules.action_for(id, &event) {
            let _ = crate::command::spawn_config_command_fire_and_forget(command);
            return Task::none();
        }

        if let Some(module) = self.modules.get(id)
            && let Some(msg) = module.on_interaction(event)
        {
            return Task::done(msg);
        }
        Task::none()
    }

    pub(super) fn handle_battery_event(&mut self, event: BatteryEvent) -> Task<Message> {
        match event {
            BatteryEvent::StateChanged {
                percentage,
                is_charging,
            } => self.push_battery_state(percentage, is_charging),
            BatteryEvent::Unavailable => self.mark_module_type_unavailable(ModuleKind::Battery),
        }
        Task::none()
    }

    pub(super) fn handle_network_event(&mut self, event: NetworkEvent) -> Task<Message> {
        match event {
            NetworkEvent::StateChanged {
                connectivity,
                label,
            } => self.push_network_state(connectivity, label),
            NetworkEvent::Unavailable => self.mark_module_type_unavailable(ModuleKind::Network),
        }
        Task::none()
    }

    pub(super) fn handle_volume_event(&mut self, event: VolumeEvent) -> Task<Message> {
        match event {
            VolumeEvent::StateChanged { volume, muted } => self.push_volume_state(volume, muted),
            VolumeEvent::Unavailable => self.mark_module_type_unavailable(ModuleKind::Volume),
        }
        Task::none()
    }

    pub(super) fn handle_volume_adjust(&self, step: u8, up: bool) -> Task<Message> {
        let arg = if up {
            format!("{step}%+")
        } else {
            format!("{step}%-")
        };
        tokio::task::spawn(async move {
            match crate::command::set_default_sink_volume(&arg).await {
                Ok(status) if status.success() => {}
                Ok(status) => eprintln!("niribar: wpctl set-volume exited with {status}"),
                Err(err) => eprintln!("niribar: failed to run wpctl set-volume: {err}"),
            }
        });
        Task::none()
    }

    fn mark_module_type_unavailable(&mut self, module_type: ModuleKind) {
        let ids: Vec<_> = self.modules.ids_by_kind(module_type).to_vec();
        for id in ids {
            self.modules.set_state(id, ModuleState::Unavailable);
        }
    }
}
