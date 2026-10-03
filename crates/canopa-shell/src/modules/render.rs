use crate::modules::{ModuleDetailSession, ModuleState};

pub struct BarRenderContext<'a> {
    pub window: iced::window::Id,
    pub edge: &'a crate::config::Edge,
}

/// App-resolved interactions for a Module displayed inside a Bar Panel.
pub struct PanelModuleContext {
    pub primary: Option<crate::app::Message>,
    pub scroll_up: Option<crate::app::Message>,
    pub scroll_down: Option<crate::app::Message>,
    pub focused: bool,
}

pub trait ModuleRender: Send + Sync {
    fn bar_panel_activation(
        &self,
        _state: &ModuleState,
        _window: iced::window::Id,
    ) -> Option<super::InteractionEvent> {
        Some(super::InteractionEvent::Clicked)
    }

    fn has_bar_panel_content(&self, state: &ModuleState) -> bool {
        self.has_content(state, crate::config::FormatMode::Expanded)
    }

    /// Modules may offer compact controls while keeping their full detail view.
    fn render_bar_panel_with_style(
        &self,
        _state: &ModuleState,
        _style: &crate::config::Style,
        _context: PanelModuleContext,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        None
    }
    /// Ordinary modules supply values and optional fallback/decorations. This
    /// boundary owns format layout, style defaults and edge-specific rotation.
    fn presentation(&self) -> Option<&dyn super::view::IconLabel> {
        None
    }

    fn render_bar(
        &self,
        state: &ModuleState,
        context: &BarRenderContext<'_>,
        style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        let horizontal = context.edge.is_horizontal();
        if let Some(presentation) = self.presentation() {
            let layout = if horizontal {
                super::view::IconLabelLayout::Horizontal
            } else {
                super::view::IconLabelLayout::Collapsed
            };
            return super::view::icon_label_view_with_rotation(
                presentation,
                state,
                layout,
                style,
                presentation
                    .content_rotation()
                    .resolve_for_edge(context.edge),
            );
        }
        // Rich surfaces retain their image, anchor or workspace frame semantics.
        if horizontal {
            self.render_bar_horizontal_with_style(state, context, style)
                .or_else(|| self.render_horizontal_with_style(state, style))
                .unwrap_or_else(|| self.render_panel_with_style(state, style))
        } else {
            self.render_bar_collapsed_with_style(state, context, style)
                .unwrap_or_else(|| self.render_collapsed_with_style(state, style))
        }
    }

    /// Whether this presentation has content, before adding frames or input targets.
    fn has_content(&self, state: &ModuleState, _mode: crate::config::FormatMode) -> bool {
        match state {
            ModuleState::Empty | ModuleState::Unavailable => false,
            ModuleState::FocusedWindow(state) => state.window.is_some(),
            ModuleState::Script(state) => !state.output.trim().is_empty(),
            _ => true,
        }
    }

    fn render_collapsed_with_style(
        &self,
        state: &ModuleState,
        style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        self.presentation().map_or_else(
            || iced::widget::Space::new().into(),
            |presentation| {
                super::view::icon_label_view_with_default(
                    presentation,
                    state,
                    super::view::IconLabelLayout::Collapsed,
                    style,
                )
            },
        )
    }

    fn render_panel_with_style(
        &self,
        state: &ModuleState,
        style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        self.presentation().map_or_else(
            || iced::widget::Space::new().into(),
            |presentation| {
                super::view::icon_label_view_with_default(
                    presentation,
                    state,
                    super::view::IconLabelLayout::Expanded,
                    style,
                )
            },
        )
    }

    fn render_bar_collapsed_with_style(
        &self,
        _state: &ModuleState,
        _context: &BarRenderContext<'_>,
        _style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        None
    }

    fn render_horizontal_with_style(
        &self,
        state: &ModuleState,
        style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        self.presentation().map(|presentation| {
            super::view::icon_label_view_with_default(
                presentation,
                state,
                super::view::IconLabelLayout::Horizontal,
                style,
            )
        })
    }

    fn render_bar_horizontal_with_style(
        &self,
        _state: &ModuleState,
        _context: &BarRenderContext<'_>,
        _style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        None
    }

    fn render_detail_with_style(
        &self,
        _state: &ModuleState,
        _style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        None
    }

    fn render_detail_session_with_style(
        &self,
        _state: &ModuleState,
        _style: &crate::config::Style,
        _session: &ModuleDetailSession,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        None
    }

    fn detail_preferred_size(
        &self,
        _state: &ModuleState,
        _style: &crate::config::Style,
        _session: Option<&ModuleDetailSession>,
    ) -> Option<crate::surface::panel::PanelPreferredSize> {
        None
    }

    fn tooltip_values(&self, _state: &ModuleState) -> Vec<(&'static str, String)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests;
