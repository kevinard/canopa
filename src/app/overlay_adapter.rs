#[cfg(test)]
use crate::modules::ModuleId;

use super::surfaces::RegistryEffect;

impl super::App {
    #[cfg(test)]
    pub(super) fn register_panel_popup(
        &mut self,
        window: iced::window::Id,
        _origin_surface: iced::window::Id,
        tree: crate::surface::panel::PanelTree,
    ) {
        self.surfaces.insert_panel_popup(
            window,
            super::surfaces::PanelMeta {
                tree,
                search_input_id: super::panel::panel_search_input_id(window),
            },
        );
    }

    #[cfg(test)]
    pub(super) fn register_menu_popup(
        &mut self,
        window: iced::window::Id,
        meta: super::surfaces::PopupMeta,
    ) {
        self.surfaces.insert_popup(window, meta);
    }

    #[cfg(test)]
    pub(super) fn register_tooltip_popup(
        &mut self,
        window: iced::window::Id,
        target_surface: iced::window::Id,
        target_module: ModuleId,
        tree: crate::surface::tooltip_popup::TooltipTree,
    ) {
        self.surfaces
            .insert_tooltip_popup(super::surfaces::TooltipPopup {
                window,
                target_window: target_surface,
                target_module,
                tree,
            });
    }

    pub(super) fn apply_registry_effects(
        &mut self,
        effects: Vec<RegistryEffect>,
    ) -> Vec<iced::Task<super::Message>> {
        let mut tasks = Vec::new();
        for effect in effects {
            match effect {
                RegistryEffect::RemoveWindow(window) => {
                    self.panel.mark_closing_window(window);
                    self.surfaces.remove_panel_popup(window);
                    self.surfaces.remove_popup(window);
                    self.surfaces.remove_tooltip_popup(window);
                    self.menu.clear_window(window);
                    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
                }
                RegistryEffect::SuspendBarKeyboardInteractivity => {
                    tasks.extend(self.suspend_bar_keyboard_interactivity());
                }
                RegistryEffect::RestoreBarKeyboardInteractivity => {
                    tasks.extend(self.restore_bar_keyboard_interactivity());
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

        assert!(app.surfaces.tooltip_popup(tooltip).is_some());
    }
}
