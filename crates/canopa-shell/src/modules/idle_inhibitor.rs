//! Presentation and interaction for the shell's shared manual idle inhibitor.
use super::data_source::{
    ModuleDataSourceIdentity, ModuleDataSourceMapper, ModuleDataSourceState, ModuleInstanceOutcome,
};
use super::interaction::ModuleInteractionHandler;
use super::render::{BarRenderContext, ModuleRender};
use super::view::{IconLabel, IconLabelLayout};
use super::{InteractionEvent, Module, ModuleId, ModuleKind, ModuleState};
use crate::app::{Message, messages::ModuleRuntimeEvent};
use crate::config::{IdleInhibitorConfig, Style};

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
        .then(|| Message::Modules(ModuleRuntimeEvent::ToggleIdleInhibitor))
    }
}

impl ModuleDataSourceMapper for IdleInhibitorModule {
    fn data_source_identity(&self) -> ModuleDataSourceIdentity {
        ModuleDataSourceIdentity::IdleInhibitor
    }
    fn map_data_source_state(&self, state: &ModuleDataSourceState) -> ModuleInstanceOutcome {
        match state {
            ModuleDataSourceState::IdleInhibitor(state) => {
                ModuleInstanceOutcome::Visible(ModuleState::IdleInhibitor(*state))
            }
            _ => ModuleInstanceOutcome::Unavailable,
        }
    }
}

impl IconLabel for IdleInhibitorModule {
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
        status(state).value().into()
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
    fn has_content(&self, _: &ModuleState, _: crate::config::FormatMode) -> bool {
        true
    }
    fn tooltip_values(&self, state: &ModuleState) -> Vec<(&'static str, String)> {
        let status = status(state);
        vec![
            ("icon", self.icon(state)),
            ("value", status.value().into()),
            ("state", status.name().into()),
            ("label", status.help().into()),
        ]
    }
    fn render_collapsed_with_style(
        &self,
        state: &ModuleState,
        style: &Style,
    ) -> iced::Element<'_, Message> {
        super::view::icon_label_view_with_default(self, state, IconLabelLayout::Collapsed, style)
    }
    fn render_panel_with_style(
        &self,
        state: &ModuleState,
        style: &Style,
    ) -> iced::Element<'_, Message> {
        super::view::icon_label_view_with_default(self, state, IconLabelLayout::Expanded, style)
    }
    fn render_horizontal_with_style(
        &self,
        state: &ModuleState,
        style: &Style,
    ) -> Option<iced::Element<'_, Message>> {
        Some(super::view::icon_label_view_with_default(
            self,
            state,
            IconLabelLayout::Horizontal,
            style,
        ))
    }
    fn render_bar_collapsed_with_style(
        &self,
        state: &ModuleState,
        context: &BarRenderContext<'_>,
        style: &Style,
    ) -> Option<iced::Element<'_, Message>> {
        let rotation = self.config.content_rotation.resolve_for_edge(context.edge);
        (rotation != crate::config::ResolvedContentRotation::None
            && self
                .config
                .format
                .template_for(crate::config::FormatMode::Collapsed)
                .is_some())
        .then(|| {
            super::view::icon_label_view_with_rotation(
                self,
                state,
                IconLabelLayout::Collapsed,
                style,
                rotation,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
