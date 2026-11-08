//! Presentation and interaction for the shell's shared manual idle inhibitor.
use super::data_source::{
    ModuleDataSourceIdentity, ModuleDataSourceMapper, ModuleDataSourceState, ModuleInstanceOutcome,
};
use super::interaction::ModuleInteractionHandler;
use super::render::ModuleRender;
use super::view::{IconLabel, IconLabelLayout};
use super::{InteractionEvent, Module, ModuleId, ModuleKind, ModuleState};
use crate::app::{Message, messages::ModuleRuntimeEvent};
use crate::config::{IdleBackend, IdleInhibitorConfig, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleInhibitorState {
    Inactive,
    Active,
    Pending,
    Unavailable,
}

impl IdleInhibitorState {
    fn name(self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Active => "active",
            Self::Pending => "pending",
            Self::Unavailable => "unavailable",
        }
    }
    fn value(self) -> &'static str {
        match self {
            Self::Inactive => "Off",
            Self::Active => "On",
            Self::Pending => "…",
            Self::Unavailable => "Unavailable",
        }
    }
    fn help(self) -> &'static str {
        match self {
            Self::Inactive => "Idle inhibitor off · Click to keep awake",
            Self::Active => "Idle inhibitor on · Click to allow idle",
            Self::Pending => "Updating idle inhibitor…",
            Self::Unavailable => {
                "Idle inhibition unavailable · Requires a visible bar and compositor support"
            }
        }
    }
}

fn status(state: &ModuleState) -> IdleInhibitorState {
    match state {
        ModuleState::IdleInhibitor(state) => *state,
        ModuleState::Stasis(snapshot) => {
            if snapshot.manually_paused {
                IdleInhibitorState::Active
            } else {
                IdleInhibitorState::Inactive
            }
        }
        ModuleState::Unavailable => IdleInhibitorState::Unavailable,
        _ => IdleInhibitorState::Pending,
    }
}

pub(super) struct IdleInhibitorModule {
    pub(super) id: ModuleId,
    pub(super) config: IdleInhibitorConfig,
}

impl Module for IdleInhibitorModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn kind(&self) -> ModuleKind {
        ModuleKind::IdleInhibitor
    }
    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::none()
    }
    fn data_source(&self) -> Option<&dyn ModuleDataSourceMapper> {
        Some(self)
    }
    fn interaction_handler(&self) -> Option<&dyn ModuleInteractionHandler> {
        Some(self)
    }
}

impl ModuleInteractionHandler for IdleInhibitorModule {
    fn on_interaction(&self, event: InteractionEvent) -> Option<Message> {
        matches!(
            event,
            InteractionEvent::Clicked | InteractionEvent::Activated(_)
        )
        .then(|| {
            Message::Modules(if self.config.backend == IdleBackend::Stasis {
                ModuleRuntimeEvent::ToggleStasis
            } else {
                ModuleRuntimeEvent::ToggleIdleInhibitor
            })
        })
    }
}

impl ModuleDataSourceMapper for IdleInhibitorModule {
    fn data_source_identity(&self) -> ModuleDataSourceIdentity {
        if self.config.backend == IdleBackend::Stasis {
            ModuleDataSourceIdentity::Stasis
        } else {
            ModuleDataSourceIdentity::IdleInhibitor
        }
    }
    fn map_data_source_state(&self, state: &ModuleDataSourceState) -> ModuleInstanceOutcome {
        match state {
            ModuleDataSourceState::Stasis(snapshot)
                if self.config.backend == IdleBackend::Stasis =>
            {
                ModuleInstanceOutcome::Visible(ModuleState::Stasis(snapshot.clone()))
            }
            ModuleDataSourceState::IdleInhibitor(state)
                if self.config.backend == IdleBackend::Wayland =>
            {
                ModuleInstanceOutcome::Visible(ModuleState::IdleInhibitor(*state))
            }
            _ => ModuleInstanceOutcome::Unavailable,
        }
    }
}

impl IconLabel for IdleInhibitorModule {
    fn content_rotation(&self) -> crate::config::ContentRotation {
        self.config.content_rotation
    }

    fn icon(&self, state: &ModuleState) -> String {
        let status = status(state);
        self.config
            .icons
            .icon_for_state(status.name())
            .unwrap_or(match status {
                IdleInhibitorState::Active => "󰅶",
                IdleInhibitorState::Inactive => "󰾪",
                IdleInhibitorState::Pending => "󰔟",
                IdleInhibitorState::Unavailable => "󰅸",
            })
            .to_owned()
    }
    fn label(&self, state: &ModuleState) -> String {
        match state {
            ModuleState::Stasis(s) if s.paused && !s.manually_paused => "Inhibited".into(),
            _ => status(state).value().into(),
        }
    }
    fn color(&self, state: &ModuleState) -> iced::Color {
        match status(state) {
            IdleInhibitorState::Active => crate::theme::palette().accent,
            IdleInhibitorState::Unavailable => crate::theme::palette().warning,
            _ => crate::theme::palette().text,
        }
    }
    fn styled_segments_for_layout(
        &self,
        state: &ModuleState,
        layout: IconLabelLayout,
        style: &Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        let values = self
            .tooltip_values(state)
            .into_iter()
            .map(|(key, value)| (key, std::borrow::Cow::Owned(value)))
            .collect::<Vec<_>>();
        super::view::render_format_segments(
            &self.config.format,
            layout.into(),
            &values,
            Some(status(state).name()),
            style,
        )
    }
}

impl ModuleRender for IdleInhibitorModule {
    fn bar_panel_activation(
        &self,
        state: &ModuleState,
        _: iced::window::Id,
    ) -> Option<InteractionEvent> {
        (status(state) != IdleInhibitorState::Pending).then_some(InteractionEvent::Clicked)
    }
    fn render_bar_panel_with_style(
        &self,
        state: &ModuleState,
        _: &Style,
        context: super::render::PanelModuleContext,
    ) -> Option<iced::Element<'_, Message>> {
        let state = status(state);
        Some(super::view::panel_control(
            include_bytes!("../../assets/awake.svg"),
            "Keep awake",
            match state {
                IdleInhibitorState::Active => "Screen idle is paused",
                IdleInhibitorState::Inactive => "Screen can idle automatically",
                IdleInhibitorState::Pending => "Updating…",
                IdleInhibitorState::Unavailable if self.config.backend == IdleBackend::Stasis => {
                    "Stasis unavailable"
                }
                IdleInhibitorState::Unavailable => "Unavailable on this compositor",
            },
            match state {
                IdleInhibitorState::Active => super::view::PanelControlEnd::Toggle(true),
                IdleInhibitorState::Inactive => super::view::PanelControlEnd::Toggle(false),
                IdleInhibitorState::Pending => super::view::PanelControlEnd::Status("Wait"),
                IdleInhibitorState::Unavailable => super::view::PanelControlEnd::Status("Retry"),
            },
            context,
        ))
    }

    fn presentation(&self) -> Option<&dyn crate::modules::view::IconLabel> {
        Some(self)
    }

    fn has_content(&self, _: &ModuleState, _: crate::config::FormatMode) -> bool {
        true
    }
    fn tooltip_values(&self, state: &ModuleState) -> Vec<(&'static str, String)> {
        let status = status(state);
        let help = match state {
            ModuleState::Stasis(s) => format!(
                "Stasis: {} · Profile: {} · Manual hold: {} · Click to toggle manual hold",
                s.state,
                s.profile,
                if s.manually_paused { "on" } else { "off" }
            ),
            _ if self.config.backend == IdleBackend::Stasis => {
                "Stasis unavailable · Start canopa-idle and install its backend".into()
            }
            _ => status.help().into(),
        };
        vec![
            ("icon", self.icon(state)),
            ("value", self.label(state)),
            ("state", status.name().into()),
            ("label", help),
        ]
    }
}

pub fn stasis_subscription() -> iced::Subscription<Message> {
    iced::Subscription::run(stasis_stream)
}
fn stasis_stream() -> impl iced::futures::Stream<Item = Message> {
    use iced::futures::SinkExt;
    iced::stream::channel(
        8,
        |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
            let observer = canopa_widgets::idle::observe(sender);
            tokio::pin!(observer);
            loop {
                tokio::select! {
                    _=&mut observer=>break,
                    value=receiver.recv()=> {
                        let Some(value)=value else {break;};
                        if output.send(Message::Modules(ModuleRuntimeEvent::Stasis(value))).await.is_err() {break;}
                    }
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stasis_uses_its_own_source_and_distinguishes_automatic_from_manual_holds() {
        let module = IdleInhibitorModule {
            id: ModuleId::new("awake"),
            config: IdleInhibitorConfig {
                backend: IdleBackend::Stasis,
                ..Default::default()
            },
        };
        assert_eq!(
            module.data_source_identity(),
            ModuleDataSourceIdentity::Stasis
        );
        assert!(matches!(
            module.on_interaction(InteractionEvent::Clicked),
            Some(Message::Modules(ModuleRuntimeEvent::ToggleStasis))
        ));
        let snapshot = canopa_widgets::idle::Snapshot {
            state: "inhibited".into(),
            paused: true,
            manually_paused: false,
            profile: "work".into(),
        };
        let state = module
            .map_data_source_state(&ModuleDataSourceState::Stasis(snapshot.clone()))
            .into_module_state();
        assert_eq!(module.label(&state), "Inhibited");
        assert!(
            module
                .tooltip_values(&state)
                .iter()
                .any(|(key, value)| *key == "label" && value.contains("Manual hold: off"))
        );
        let wayland = IdleInhibitorModule {
            id: ModuleId::new("ordinary"),
            config: Default::default(),
        };
        assert_eq!(
            wayland.map_data_source_state(&ModuleDataSourceState::Stasis(snapshot)),
            ModuleInstanceOutcome::Unavailable
        );
    }
    #[test]
    fn mouse_and_keyboard_toggle_and_unavailable_stays_visible() {
        let module = IdleInhibitorModule {
            id: ModuleId::new("awake"),
            config: Default::default(),
        };
        for event in [
            InteractionEvent::Clicked,
            InteractionEvent::Activated(iced::window::Id::unique()),
        ] {
            assert!(matches!(
                module.on_interaction(event),
                Some(Message::Modules(ModuleRuntimeEvent::ToggleIdleInhibitor))
            ));
        }
        assert!(module.on_interaction(InteractionEvent::ScrollUp).is_none());
        assert!(
            module
                .on_interaction(InteractionEvent::RightClicked(iced::window::Id::unique()))
                .is_none()
        );
        assert!(module.has_content(
            &ModuleState::Unavailable,
            crate::config::FormatMode::Collapsed
        ));
        assert!(
            module
                .tooltip_values(&ModuleState::Unavailable)
                .contains(&("state", "unavailable".into()))
        );
        assert_ne!(
            module.icon(&ModuleState::IdleInhibitor(IdleInhibitorState::Active)),
            module.icon(&ModuleState::IdleInhibitor(IdleInhibitorState::Inactive))
        );
    }
}
