use crate::modules::ModuleId;

use super::overlay_lifecycle::OverlayEffect;

impl super::App {
    pub(super) fn register_panel_popup(
        &mut self,
        window: iced::window::Id,
        origin_surface: iced::window::Id,
        tree: crate::surface::panel::PanelTree,
    ) {
        self.overlays.panel_opened(window, origin_surface);
        self.surfaces.insert_panel_popup(
            window,
            super::surfaces::PanelMeta {
                tree,
                search_input_id: super::panel::panel_search_input_id(window),
            },
        );
    }

    pub(super) fn register_menu_popup(
        &mut self,
        window: iced::window::Id,
        meta: super::surfaces::PopupMeta,
    ) {
        self.overlays.menu_opened(window);
        self.surfaces.insert_popup(window, meta);
    }

    pub(super) fn register_tooltip_popup(
        &mut self,
        window: iced::window::Id,
        target_surface: iced::window::Id,
        target_module: ModuleId,
        tree: crate::surface::tooltip_popup::TooltipTree,
    ) {
        self.overlays
            .tooltip_opened(window, target_surface, target_module.clone());
        self.surfaces
            .insert_tooltip_popup(super::surfaces::TooltipPopup {
                window,
                target_window: target_surface,
                target_module,
                tree,
            });
    }

    pub(super) fn apply_overlay_effects(
        &mut self,
        effects: Vec<OverlayEffect>,
        restore_bar_interactivity_on_panel_close: bool,
    ) -> Vec<iced::Task<super::Message>> {
        let mut tasks = Vec::new();
        for effect in effects {
            match effect {
                OverlayEffect::ClosePanel(window) => {
                    self.panel.mark_closing_window(window);
                    self.surfaces.remove_panel_popup(window);
                    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
                    if restore_bar_interactivity_on_panel_close {
                        tasks.extend(self.restore_bar_keyboard_interactivity());
                    }
                }
                OverlayEffect::CloseMenu(window) => {
                    self.surfaces.remove_popup(window);
                    self.menu.clear_window(window);
                    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
                }
                OverlayEffect::CloseTooltip(window) => {
                    self.surfaces.remove_tooltip_popup(window);
                    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
                }
                OverlayEffect::SuspendBarKeyboardInteractivity => {
                    tasks.extend(self.suspend_bar_keyboard_interactivity());
                }
            }
        }
        tasks
    }
}

#[cfg(test)]
mod tests {
    use crate::modules::ModuleId;

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    #[test]
    fn register_panel_popup_tracks_lifecycle_and_metadata() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let origin = wid();
        let panel = wid();
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            origin_window: origin,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 1080),
        };

        app.register_panel_popup(panel, origin, tree);

        assert!(app.overlays.panel(panel).is_some());
        assert!(app.surfaces.panel_popup(panel).is_some());
    }

    #[test]
    fn register_menu_popup_tracks_lifecycle_and_metadata() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let menu = wid();
        let tree = crate::surface::menu_popup::MenuTree {
            items: Vec::new(),
            anchor: (0, 0),
            screen_size: (1920, 1080),
        };

        app.register_menu_popup(
            menu,
            super::super::surfaces::PopupMeta {
                service: "org.test".into(),
                menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
                tree,
                bar_name: "main".into(),
            },
        );

        assert!(app.overlays.menu(menu).is_some());
        assert!(app.surfaces.popup(menu).is_some());
    }

    #[test]
    fn register_tooltip_popup_tracks_lifecycle_and_metadata() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let surface = wid();
        let tooltip = wid();
        let module = ModuleId::new("clock");
        let tree = crate::surface::tooltip_popup::TooltipTree {
            segments: Vec::new(),
            anchor: (0, 0),
            bar_size: (48, 1080),
            screen_size: (48, 1080),
            bar_edge: crate::config::Edge::Left,
        };

        app.register_tooltip_popup(tooltip, surface, module.clone(), tree);

        assert!(app.overlays.tooltip(tooltip).is_some());
        assert!(app.surfaces.tooltip_popup(tooltip).is_some());
    }
}
