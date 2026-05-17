use crate::modules::ModuleId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OverlayEffect {
    ClosePanel(iced::window::Id),
    CloseMenu(iced::window::Id),
    CloseTooltip(iced::window::Id),
    SuspendBarKeyboardInteractivity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActivePanel {
    pub(super) window: iced::window::Id,
    pub(super) origin_surface: iced::window::Id,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActiveMenu {
    pub(super) window: iced::window::Id,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActiveTooltip {
    pub(super) window: iced::window::Id,
    pub(super) target_surface: iced::window::Id,
    pub(super) target_module: ModuleId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClosedOverlay {
    Panel,
    Menu,
    Tooltip,
}

#[derive(Debug, Default)]
pub(super) struct OverlayLifecycle {
    panels: Vec<ActivePanel>,
    menus: Vec<ActiveMenu>,
    tooltips: Vec<ActiveTooltip>,
}

impl OverlayLifecycle {
    pub(super) fn panel(&self, window: iced::window::Id) -> Option<&ActivePanel> {
        self.panels.iter().find(|panel| panel.window == window)
    }

    pub(super) fn menu(&self, window: iced::window::Id) -> Option<&ActiveMenu> {
        self.menus.iter().find(|menu| menu.window == window)
    }

    pub(super) fn tooltip(&self, window: iced::window::Id) -> Option<&ActiveTooltip> {
        self.tooltips
            .iter()
            .find(|tooltip| tooltip.window == window)
    }

    pub(super) fn panel_opened(
        &mut self,
        window: iced::window::Id,
        origin_surface: iced::window::Id,
    ) {
        self.forget_panel(window);
        self.panels.push(ActivePanel {
            window,
            origin_surface,
        });
    }

    pub(super) fn menu_opened(&mut self, window: iced::window::Id) {
        self.forget_menu(window);
        self.menus.push(ActiveMenu { window });
    }

    pub(super) fn tooltip_opened(
        &mut self,
        window: iced::window::Id,
        target_surface: iced::window::Id,
        target_module: ModuleId,
    ) {
        self.forget_tooltip(window);
        self.tooltips.push(ActiveTooltip {
            window,
            target_surface,
            target_module,
        });
    }

    pub(super) fn open_panel(
        &mut self,
        origin_surface: iced::window::Id,
        window: iced::window::Id,
    ) -> Vec<OverlayEffect> {
        let mut effects = self.close_tooltips_for_surface(origin_surface);
        effects.extend(self.close_menus());
        effects.extend(self.close_panels());
        effects.push(OverlayEffect::SuspendBarKeyboardInteractivity);
        self.panel_opened(window, origin_surface);
        effects
    }

    pub(super) fn close_all_panels(&mut self) -> Vec<OverlayEffect> {
        self.close_panels()
    }

    pub(super) fn open_menu(&mut self, window: iced::window::Id) -> Vec<OverlayEffect> {
        let mut effects = self.close_tooltips();
        effects.extend(self.close_panels());
        effects.extend(self.close_menus());
        self.menu_opened(window);
        effects
    }

    pub(super) fn close_all_menus(&mut self) -> Vec<OverlayEffect> {
        self.close_menus()
    }

    pub(super) fn open_tooltip(
        &mut self,
        window: iced::window::Id,
        target_surface: iced::window::Id,
        target_module: ModuleId,
    ) -> Vec<OverlayEffect> {
        let effects = self.close_tooltips_for_surface(target_surface);
        self.tooltip_opened(window, target_surface, target_module);
        effects
    }

    pub(super) fn close_tooltip_for_surface(
        &mut self,
        target_surface: iced::window::Id,
    ) -> Vec<OverlayEffect> {
        self.close_tooltips_for_surface(target_surface)
    }

    pub(super) fn close_tooltip_for_target(
        &mut self,
        target_surface: iced::window::Id,
        target_module: &ModuleId,
    ) -> Vec<OverlayEffect> {
        let mut effects = Vec::new();
        self.tooltips.retain(|tooltip| {
            if tooltip.target_surface == target_surface && &tooltip.target_module == target_module {
                effects.push(OverlayEffect::CloseTooltip(tooltip.window));
                false
            } else {
                true
            }
        });
        effects
    }

    pub(super) fn forget_panel(&mut self, window: iced::window::Id) {
        self.panels.retain(|panel| panel.window != window);
    }

    pub(super) fn forget_menu(&mut self, window: iced::window::Id) {
        self.menus.retain(|menu| menu.window != window);
    }

    pub(super) fn forget_tooltip(&mut self, window: iced::window::Id) {
        self.tooltips.retain(|tooltip| tooltip.window != window);
    }

    pub(super) fn window_closed(&mut self, window: iced::window::Id) -> Option<ClosedOverlay> {
        if self.panel(window).is_some() {
            self.forget_panel(window);
            return Some(ClosedOverlay::Panel);
        }
        if self.menu(window).is_some() {
            self.forget_menu(window);
            return Some(ClosedOverlay::Menu);
        }
        if self.tooltip(window).is_some() {
            self.forget_tooltip(window);
            return Some(ClosedOverlay::Tooltip);
        }
        None
    }

    fn close_panels(&mut self) -> Vec<OverlayEffect> {
        self.panels
            .drain(..)
            .map(|panel| OverlayEffect::ClosePanel(panel.window))
            .collect()
    }

    fn close_menus(&mut self) -> Vec<OverlayEffect> {
        self.menus
            .drain(..)
            .map(|menu| OverlayEffect::CloseMenu(menu.window))
            .collect()
    }

    fn close_tooltips(&mut self) -> Vec<OverlayEffect> {
        self.tooltips
            .drain(..)
            .map(|tooltip| OverlayEffect::CloseTooltip(tooltip.window))
            .collect()
    }

    fn close_tooltips_for_surface(
        &mut self,
        target_surface: iced::window::Id,
    ) -> Vec<OverlayEffect> {
        let mut effects = Vec::new();
        self.tooltips.retain(|tooltip| {
            if tooltip.target_surface == target_surface {
                effects.push(OverlayEffect::CloseTooltip(tooltip.window));
                false
            } else {
                true
            }
        });
        effects
    }
}

#[cfg(test)]
mod tests {
    use crate::modules::ModuleId;

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    #[test]
    fn opening_panel_replaces_existing_panel_menu_and_origin_tooltip() {
        let origin = wid();
        let old_panel = wid();
        let old_menu = wid();
        let old_tooltip = wid();
        let new_panel = wid();

        let mut lifecycle = super::OverlayLifecycle::default();
        lifecycle.panel_opened(old_panel, origin);
        lifecycle.menu_opened(old_menu);
        lifecycle.tooltip_opened(old_tooltip, origin, ModuleId::new("clock"));

        let effects = lifecycle.open_panel(origin, new_panel);

        assert_eq!(
            effects,
            vec![
                super::OverlayEffect::CloseTooltip(old_tooltip),
                super::OverlayEffect::CloseMenu(old_menu),
                super::OverlayEffect::ClosePanel(old_panel),
                super::OverlayEffect::SuspendBarKeyboardInteractivity,
            ]
        );
        assert!(lifecycle.panel(new_panel).is_some());
        assert!(lifecycle.menu(old_menu).is_none());
        assert!(lifecycle.tooltip(old_tooltip).is_none());
    }

    #[test]
    fn opening_menu_replaces_existing_menu_and_closes_all_panels_and_tooltips() {
        let origin = wid();
        let old_panel = wid();
        let old_menu = wid();
        let tooltip = wid();
        let new_menu = wid();

        let mut lifecycle = super::OverlayLifecycle::default();
        lifecycle.panel_opened(old_panel, origin);
        lifecycle.menu_opened(old_menu);
        lifecycle.tooltip_opened(tooltip, origin, ModuleId::new("clock"));

        let effects = lifecycle.open_menu(new_menu);

        assert_eq!(
            effects,
            vec![
                super::OverlayEffect::CloseTooltip(tooltip),
                super::OverlayEffect::ClosePanel(old_panel),
                super::OverlayEffect::CloseMenu(old_menu),
            ]
        );
        assert!(lifecycle.menu(new_menu).is_some());
        assert!(lifecycle.panel(old_panel).is_none());
        assert!(lifecycle.tooltip(tooltip).is_none());
    }

    #[test]
    fn opening_tooltip_replaces_tooltip_for_same_bar_surface_only() {
        let surface_a = wid();
        let surface_b = wid();
        let old_tooltip_a = wid();
        let old_tooltip_b = wid();
        let new_tooltip_a = wid();

        let mut lifecycle = super::OverlayLifecycle::default();
        lifecycle.tooltip_opened(old_tooltip_a, surface_a, ModuleId::new("clock"));
        lifecycle.tooltip_opened(old_tooltip_b, surface_b, ModuleId::new("volume"));

        let effects = lifecycle.open_tooltip(new_tooltip_a, surface_a, ModuleId::new("network"));

        assert_eq!(
            effects,
            vec![super::OverlayEffect::CloseTooltip(old_tooltip_a)]
        );
        assert!(lifecycle.tooltip(new_tooltip_a).is_some());
        assert!(lifecycle.tooltip(old_tooltip_a).is_none());
        assert!(lifecycle.tooltip(old_tooltip_b).is_some());
    }

    #[test]
    fn close_all_panels_forgets_tracked_panels() {
        let origin = wid();
        let panel = wid();
        let mut lifecycle = super::OverlayLifecycle::default();
        lifecycle.panel_opened(panel, origin);

        let effects = lifecycle.close_all_panels();

        assert_eq!(effects, vec![super::OverlayEffect::ClosePanel(panel)]);
        assert!(lifecycle.panel(panel).is_none());
    }

    #[test]
    fn close_tooltip_for_target_ignores_other_modules_on_same_bar_surface() {
        let surface = wid();
        let clock_tooltip = wid();
        let volume_tooltip = wid();
        let mut lifecycle = super::OverlayLifecycle::default();
        lifecycle.tooltip_opened(clock_tooltip, surface, ModuleId::new("clock"));
        lifecycle.tooltip_opened(volume_tooltip, surface, ModuleId::new("volume"));

        let effects = lifecycle.close_tooltip_for_target(surface, &ModuleId::new("clock"));

        assert_eq!(
            effects,
            vec![super::OverlayEffect::CloseTooltip(clock_tooltip)]
        );
        assert!(lifecycle.tooltip(clock_tooltip).is_none());
        assert!(lifecycle.tooltip(volume_tooltip).is_some());
    }

    #[test]
    fn window_closed_classifies_and_forgets_tracked_overlay() {
        let mut lifecycle = super::OverlayLifecycle::default();
        let menu = wid();
        lifecycle.menu_opened(menu);

        assert_eq!(
            lifecycle.window_closed(menu),
            Some(super::ClosedOverlay::Menu)
        );
        assert!(lifecycle.menu(menu).is_none());
        assert_eq!(lifecycle.window_closed(menu), None);
    }
}
