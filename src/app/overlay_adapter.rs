use super::surfaces::RegistryEffect;

impl super::App {
    #[cfg(test)]
    pub(super) fn register_panel_popup(
        &mut self,
        window: iced::window::Id,
        origin_surface: iced::window::Id,
    ) {
        if self.foreground.active_panel_tree().is_none() {
            let request = super::foreground::PanelForegroundRequest {
                origin_window: origin_surface,
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::BarPanel,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                anchor: None,
                screen_size: (1920, 1080),
            };
            let context = super::foreground::PanelOpenContext::new(
                self.config.menus.as_slice(),
                std::collections::HashMap::new(),
            );
            let _ = self
                .foreground
                .handle(super::foreground::ForegroundIntent::OpenPanel { request, context });
        }
        self.foreground
            .handle(super::foreground::ForegroundIntent::PanelWindowOpened { window });
    }

    #[cfg(test)]
    pub(super) fn register_menu_popup(
        &mut self,
        window: iced::window::Id,
        meta: super::foreground::MenuPopupMeta,
    ) {
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
                RegistryEffect::TooltipClosed { window } => {
                    self.surfaces.remove_tooltip_popup(window);
                    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
                }
            }
        }
        tasks
    }

    pub(super) fn apply_foreground_effects(
        &mut self,
        effects: Vec<super::foreground::ForegroundEffect>,
    ) -> Vec<iced::Task<super::Message>> {
        let mut tasks = Vec::new();
        for effect in effects {
            match effect {
                super::foreground::ForegroundEffect::OpenPanelWindow { tree } => {
                    tasks.push(self.open_panel_window(tree));
                }
                super::foreground::ForegroundEffect::RemoveWindow(window) => {
                    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
                }
                super::foreground::ForegroundEffect::SuspendBarKeyboardInteractivity => {
                    tasks.extend(self.suspend_bar_keyboard_interactivity());
                }
                super::foreground::ForegroundEffect::RestoreBarKeyboardInteractivity => {
                    tasks.extend(self.restore_bar_keyboard_interactivity());
                }
                super::foreground::ForegroundEffect::CloseTooltipsForSurface(window) => {
                    let effects = self.surfaces.close_tooltip_for_surface(window);
                    tasks.extend(self.apply_registry_effects(effects));
                }
                super::foreground::ForegroundEffect::CloseAllTooltips => {
                    let effects = self.surfaces.close_tooltip_overlays();
                    tasks.extend(self.apply_registry_effects(effects));
                }
                super::foreground::ForegroundEffect::OpenMenuWindow { request } => {
                    tasks.push(self.open_menu_window(request));
                }
                super::foreground::ForegroundEffect::CloseMenu { window, meta } => {
                    send_menu_closed_event(self.shared.dbus.session.clone(), meta);
                    tasks.push(iced::Task::done(super::Message::RemoveWindow(window)));
                }
                super::foreground::ForegroundEffect::RequestPanelSearchFocus => {
                    tasks.push(super::panel::request_panel_search_focus());
                }
                super::foreground::ForegroundEffect::LoadInstalledAppsThenPreparePanel(pending) => {
                    let apps = self.load_installed_apps_for_panel();
                    let effects = self.foreground.handle(
                        super::foreground::ForegroundIntent::PanelInstalledAppsLoaded {
                            pending,
                            apps,
                            menus: self.config.menus.as_slice(),
                        },
                    );
                    tasks.extend(self.apply_foreground_effects(effects));
                }
                super::foreground::ForegroundEffect::RunPanelAction {
                    origin_window,
                    origin,
                    action,
                } => {
                    tasks.push(self.dispatch_panel_action(origin_window, origin, action));
                }
                super::foreground::ForegroundEffect::LaunchInstalledApp(id) => {
                    self.launch_installed_app_by_id(&id);
                }
                super::foreground::ForegroundEffect::SendDmenuResponse {
                    responder,
                    response,
                } => {
                    send_ipc_response(responder, response);
                }
                super::foreground::ForegroundEffect::ScrollPanelMenuToFocused { window, y } => {
                    tasks.push(iced::widget::operation::scroll_to(
                        super::panel::panel_menu_scroll_id(window),
                        iced::widget::operation::AbsoluteOffset {
                            x: None,
                            y: Some(y),
                        },
                    ));
                }
            }
        }
        tasks
    }
}

fn send_ipc_response(
    responder: super::messages::IpcResponder,
    response: crate::ipc::protocol::IpcResponse,
) {
    if let Some(sender) = responder
        .lock()
        .expect("IPC responder mutex poisoned")
        .take()
    {
        let _ = sender.send(response);
    }
}

pub(super) fn send_menu_closed_event(
    conn: Option<zbus::Connection>,
    meta: super::foreground::MenuPopupMeta,
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
    ) {
        let request = app.panel_open_request(
            origin,
            crate::surface::panel::PanelOrigin::Bar,
            content,
            None,
        );
        let effects = app.prepare_panel_for_test(request);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            crate::app::foreground::ForegroundEffect::OpenPanelWindow { .. }
        )));
        app.register_panel_popup(window, origin);
    }

    fn prepare_panel(
        app: &mut super::super::App,
        origin: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) {
        let request = app.panel_open_request(
            origin,
            crate::surface::panel::PanelOrigin::Bar,
            content,
            None,
        );
        let effects = app.prepare_panel_for_test(request);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            crate::app::foreground::ForegroundEffect::OpenPanelWindow { .. }
        )));
    }

    #[test]
    fn register_panel_popup_tracks_foreground_lifecycle() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let origin = wid();
        let panel = wid();

        app.register_panel_popup(panel, origin);

        assert!(app.foreground.is_panel_window(panel));
        assert_eq!(
            app.foreground
                .active_panel_tree()
                .map(|tree| tree.origin_window),
            Some(origin)
        );
    }

    #[test]
    fn register_menu_popup_tracks_foreground_lifecycle() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let menu = wid();
        let tree = crate::surface::menu_popup::MenuTree {
            items: Vec::new(),
            anchor: (0, 0),
            screen_size: (1920, 1080),
        };

        app.register_menu_popup(
            menu,
            super::super::foreground::MenuPopupMeta {
                service: "org.test".into(),
                menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
                tree,
                origin: super::super::surfaces::BarSurfaceIdentity::all_screens(),
            },
        );

        assert!(app.foreground.is_menu_window(menu));
        assert_eq!(
            app.foreground
                .active_menu_meta_for_window(menu)
                .map(|meta| meta.service.as_str()),
            Some("org.test")
        );
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

        open_and_attach_panel(
            &mut app,
            origin,
            old_window,
            crate::surface::panel::PanelContent::BarPanel,
        );
        open_and_attach_panel(
            &mut app,
            origin,
            new_window,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowClosed(
            old_window,
        ));

        assert_eq!(app.foreground.active_panel_window(), Some(new_window));
        assert!(!app.foreground.is_panel_window(old_window));
        assert!(app.foreground.is_panel_window(new_window));
    }

    #[test]
    fn current_panel_closed_effect_clears_current_panel_runtime() {
        let (mut app, _) = super::super::App::boot(crate::config::Config::default(), vec![]);
        let origin = wid();
        let window = wid();
        prepare_panel(
            &mut app,
            origin,
            crate::surface::panel::PanelContent::BarPanel,
        );
        app.register_panel_popup(window, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::WindowClosed(window));

        assert!(app.foreground.active_panel_tree().is_none());
        assert!(!app.foreground.is_panel_window(window));
    }
}
