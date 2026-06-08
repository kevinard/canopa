use crate::modules::{ModuleDetailSession, ModuleState};

pub struct BarRenderContext<'a> {
    pub window: iced::window::Id,
    pub edge: &'a crate::config::Edge,
    pub horizontal: bool,
}

pub trait ModuleRender: Send + Sync {
    fn render_collapsed_with_style(
        &self,
        state: &ModuleState,
        style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message>;

    fn render_panel_with_style(
        &self,
        state: &ModuleState,
        style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message>;

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
        _state: &ModuleState,
        _style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        None
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
