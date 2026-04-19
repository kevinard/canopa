use iced::Task;

use super::{App, Message};
use crate::modules::battery::BatteryEvent;
use crate::modules::button::{ButtonEvent, ButtonState};
use crate::modules::clock::{ClockEvent, ClockState};
use crate::modules::network::NetworkEvent;
use crate::modules::script::{ScriptEvent, ScriptState};
use crate::modules::volume::VolumeEvent;
use crate::modules::{InteractionEvent, ModuleEvent, ModuleId, ModuleKind, ModuleState};

fn insert_module_state_if_changed(
    modules: &mut std::collections::HashMap<ModuleId, ModuleState>,
    id: ModuleId,
    state: ModuleState,
) {
    if modules.get(&id) != Some(&state) {
        modules.insert(id, state);
    }
}

impl App {
    pub(super) fn handle_module_event(
        &mut self,
        id: ModuleId,
        event: ModuleEvent,
    ) -> Task<Message> {
        match event {
            ModuleEvent::Clock(ClockEvent::Tick { time, date }) => {
                insert_module_state_if_changed(
                    &mut self.shared.modules,
                    id,
                    ModuleState::Clock(ClockState { time, date }),
                );
            }
            ModuleEvent::Script(ScriptEvent::Output(text)) => {
                insert_module_state_if_changed(
                    &mut self.shared.modules,
                    id,
                    ModuleState::Script(ScriptState { output: text }),
                );
            }
            ModuleEvent::Script(ScriptEvent::Failed) => {}
            ModuleEvent::Button(ButtonEvent::Ready) => {
                insert_module_state_if_changed(
                    &mut self.shared.modules,
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
            let _ = tokio::process::Command::new("wpctl")
                .args(["set-volume", "-l", "1", "@DEFAULT_AUDIO_SINK@", &arg])
                .status()
                .await;
        });
        Task::none()
    }

    fn mark_module_type_unavailable(&mut self, module_type: ModuleKind) {
        let ids: Vec<_> = self.modules.ids_by_kind(module_type).to_vec();
        for id in ids {
            self.shared.modules.insert(id, ModuleState::Unavailable);
        }
    }
}
