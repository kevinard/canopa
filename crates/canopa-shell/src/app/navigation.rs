use super::App;
use crate::modules::{ModuleId, Zone};

impl App {
    /// Visible content is shared by rendering, hover and tooltip availability.
    pub(super) fn visible_module_layout(&self, window: iced::window::Id) -> Vec<(Zone, ModuleId)> {
        let surface = self.surface(window).cloned().unwrap_or_default();
        let bar = Self::bar_for_surface_name(&self.config.bars, &surface.bar_name);
        let mode = if bar.edge.is_horizontal() {
            crate::config::FormatMode::Horizontal
        } else {
            crate::config::FormatMode::Collapsed
        };
        self.effective_module_layout_for_bar(bar)
            .into_iter()
            .filter(|(_, id)| self.module_has_content_on_surface(id, &surface, mode))
            .collect()
    }

    /// Focus includes visible, actionable or inspectable targets. Configured
    /// instance actions matter even when the module kind has no intrinsic action.
    pub(super) fn effective_nav_list(&self, window: iced::window::Id) -> Vec<(Zone, ModuleId)> {
        self.visible_module_layout(window)
            .into_iter()
            .filter(|(_, id)| self.module_is_keyboard_target(id))
            .collect()
    }

    pub(super) fn module_is_keyboard_target(&self, id: &ModuleId) -> bool {
        self.modules.supports_keyboard_interaction(id) || self.module_tooltip_template(id).is_some()
    }

    /// Returns the ID of the next module in the surface's effective layout (wraps last→first).
    pub(super) fn nav_next(
        &self,
        current: &ModuleId,
        window: iced::window::Id,
    ) -> Option<ModuleId> {
        let list = self.effective_nav_list(window);
        let idx = list.iter().position(|(_, id)| id == current)?;
        let next_idx = (idx + 1) % list.len();
        Some(list[next_idx].1.clone())
    }

    /// Returns the ID of the previous module in the surface's effective layout (wraps first→last).
    pub(super) fn nav_prev(
        &self,
        current: &ModuleId,
        window: iced::window::Id,
    ) -> Option<ModuleId> {
        let list = self.effective_nav_list(window);
        let idx = list.iter().position(|(_, id)| id == current)?;
        let prev_idx = (idx + list.len() - 1) % list.len();
        Some(list[prev_idx].1.clone())
    }

    /// Returns the ID of the first module in the next non-empty zone for this surface.
    pub(super) fn nav_next_zone(
        &self,
        current: &ModuleId,
        window: iced::window::Id,
    ) -> Option<ModuleId> {
        let list = self.effective_nav_list(window);
        let idx = list.iter().position(|(_, id)| id == current)?;
        let current_zone = &list[idx].0;
        let cycle = [Zone::Start, Zone::Center, Zone::End];
        let start = cycle.iter().position(|z| z == current_zone).unwrap_or(0);
        for step in 1..=3 {
            let next_zone = &cycle[(start + step) % 3];
            if let Some((_, id)) = list.iter().find(|(z, _)| z == next_zone) {
                return Some(id.clone());
            }
        }
        None
    }

    /// Returns the ID of the first module in the previous non-empty zone for this surface.
    pub(super) fn nav_prev_zone(
        &self,
        current: &ModuleId,
        window: iced::window::Id,
    ) -> Option<ModuleId> {
        let list = self.effective_nav_list(window);
        let idx = list.iter().position(|(_, id)| id == current)?;
        let current_zone = &list[idx].0;
        let cycle = [Zone::Start, Zone::End, Zone::Center];
        let start = cycle.iter().position(|z| z == current_zone).unwrap_or(0);
        for step in 1..=3 {
            let prev_zone = &cycle[(start + step) % 3];
            if let Some((_, id)) = list.iter().find(|(z, _)| z == prev_zone) {
                return Some(id.clone());
            }
        }
        None
    }

    pub(super) fn popup_nav_step(&mut self, window: iced::window::Id, forward: bool) {
        self.foreground
            .handle(super::foreground::ForegroundIntent::MenuMoveFocus { window, forward });
    }
}
