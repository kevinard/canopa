use super::surfaces::RegistryEffect;

impl super::App {
    #[cfg(test)]
    pub(super) fn register_panel_popup(
        &mut self,
        window: iced::window::Id,
        origin_surface: iced::window::Id,
        _attachment_id: super::panel_runtime::PanelAttachmentId,
    ) {
        self.surfaces.insert_panel_popup(
            window,
            super::surfaces::PanelMeta {
                origin_window: origin_surface,
            },
        );
        let tree =
            self.panel
                .active_panel_tree()
                .cloned()
                .unwrap_or(crate::surface::panel::PanelTree {
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::BarPanel,
                    origin_window: origin_surface,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    anchor: None,
                    screen_size: (1920, 1080),
                });
        self.foreground
            .handle(super::foreground::ForegroundIntent::PanelRuntimePrepared { tree });
        self.foreground
            .handle(super::foreground::ForegroundIntent::PanelWindowOpened { window });
    }

    #[cfg(test)]
    pub(super) fn register_menu_popup(
        &mut self,
        window: iced::window::Id,
        meta: super::surfaces::PopupMeta,
    ) {
        self.surfaces.insert_popup(window, meta.clone());
        let request = super::foreground::MenuForegroundRequest {
            meta,
            search_enabled: self.sni_menu_search_enabled(),
        };
        self.foreground
            .handle(super::foreground::ForegroundIntent::MenuWindowOpened { window, request });
    }

    #[cfg(test)]
    pub(super) fn register_tooltip_popup(
        &mut self,
        window: iced::window::Id,
        target_surface: iced::window::Id,
        target_module: crate::modules::ModuleId,
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
                RegistryEffect::PanelClosed { window } => {
                    let foreground_effects = self
                        .foreground
                        .handle(super::foreground::ForegroundIntent::PanelWindowClosed { window });
                    tasks.extend(self.apply_foreground_effects(foreground_effects));
                    self.surfaces.remove_panel_popup(window);
                    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
                }
                RegistryEffect::MenuClosed { window, meta } => {
                    self.foreground
                        .handle(super::foreground::ForegroundIntent::MenuWindowClosed { window });
                    self.surfaces.remove_popup(window);
                    send_menu_closed_event(self.shared.dbus.session.clone(), meta);
                    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
                }
                RegistryEffect::TooltipClosed { window } => {
                    self.surfaces.remove_tooltip_popup(window);
                    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
                }
                RegistryEffect::PendingMenuReady(meta) => {
                    let request = super::foreground::MenuForegroundRequest {
                        meta,
                        search_enabled: self.sni_menu_search_enabled(),
                    };
                    tasks.push(self.open_ready_menu_popup(request));
                }
                #[cfg(test)]
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

pub(super) fn send_menu_closed_event(
    conn: Option<zbus::Connection>,
    meta: super::surfaces::PopupMeta,
) {
    if let Some(conn) = conn {
        tokio::task::spawn(async move {
            let _ = crate::dbus::sni::host::send_menu_event(
                &conn,
                &meta.service,
                &meta.menu_path,
                0,
                "closed",
            )
            .await;
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::modules::ModuleId;

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn open_and_attach_panel(
        app: &mut super::super::App,
        origin: iced::window::Id,
        window: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) -> crate::app::panel_runtime::PanelAttachmentId {
        let request = app.panel_open_request(
            origin,
            crate::surface::panel::PanelOrigin::Bar,
            content,
            None,
        );
        let effects = app
            .panel
            .handle(crate::app::panel_runtime::PanelRuntimeIntent::Open {
                request,
                catalog: crate::app::panel_runtime::PanelMenuCatalog::new(
                    app.config.menus.as_slice(),
                ),
                module_detail_sessions: std::collections::HashMap::new(),
            });
        let attachment_id = effects
            .iter()
            .find_map(|effect| match effect {
                crate::app::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(open) => {
                    Some(open.attachment_id)
                }
                _ => None,
            })
            .expect("panel should request a window");
        app.panel.handle(
            crate::app::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
                attachment_id,
                window,
            },
        );
        attachment_id
    }

    #[test]
    fn register_panel_popup_tracks_lifecycle_and_metadata() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let origin = wid();
        let panel = wid();
        let attachment_id = crate::app::panel_runtime::PanelAttachmentId::for_test(1);

        app.register_panel_popup(panel, origin, attachment_id);

        let meta = app.surfaces.panel_popup(panel).expect("panel popup");
        assert_eq!(meta.origin_window, origin);
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
                origin: super::super::surfaces::BarSurfaceIdentity::all_screens(),
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

    #[test]
    fn stale_panel_closed_effect_does_not_clear_current_panel_runtime() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let origin = wid();
        let old_window = wid();
        let new_window = wid();

        let old_attachment = open_and_attach_panel(
            &mut app,
            origin,
            old_window,
            crate::surface::panel::PanelContent::BarPanel,
        );
        let new_attachment = open_and_attach_panel(
            &mut app,
            origin,
            new_window,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );
        app.register_panel_popup(old_window, origin, old_attachment);
        app.register_panel_popup(new_window, origin, new_attachment);

        let _ =
            app.apply_registry_effects(vec![super::super::surfaces::RegistryEffect::PanelClosed {
                window: old_window,
            }]);

        assert_eq!(app.panel.active_panel_window(), Some(new_window));
        assert!(app.surfaces.panel_popup(old_window).is_none());
        assert!(app.surfaces.panel_popup(new_window).is_some());
    }

    #[test]
    fn current_panel_closed_effect_clears_current_panel_runtime() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let origin = wid();
        let window = wid();
        let attachment_id = open_and_attach_panel(
            &mut app,
            origin,
            window,
            crate::surface::panel::PanelContent::BarPanel,
        );
        app.register_panel_popup(window, origin, attachment_id);

        let _ =
            app.apply_registry_effects(vec![super::super::surfaces::RegistryEffect::PanelClosed {
                window,
            }]);

        assert_eq!(app.panel.active_panel_window(), None);
        assert!(app.panel.active_panel_tree().is_none());
        assert!(app.surfaces.panel_popup(window).is_none());
    }
}
