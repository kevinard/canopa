use iced::Task;

use super::{App, Message};

#[derive(Debug, Clone)]
pub(super) enum AppEffect {
    OpenBarSurface {
        desired: super::bar_surface_plan::PlannedBarSurface,
    },
    OpenPanelWindow {
        tree: crate::surface::panel::PanelTree,
    },
    OpenMenuWindow {
        request: super::foreground::MenuForegroundRequest,
    },
    OpenTooltipWindow {
        target_window: iced::window::Id,
        target_module: crate::modules::ModuleId,
        tree: crate::surface::tooltip_popup::TooltipTree,
    },
    RemoveWindow(iced::window::Id),
    SuspendBarKeyboardInteractivity,
    RestoreBarKeyboardInteractivity,
    RunConfiguredCommand(crate::config::ConfiguredCommand),
    AdjustDefaultSinkVolume {
        step: u8,
        up: bool,
    },
    LaunchInstalledApp(String),
    SendDmenuResponse {
        responder: super::messages::IpcResponder,
        response: crate::ipc::protocol::IpcResponse,
    },
    SendSniMenuClosed {
        meta: super::foreground::MenuPopupMeta,
    },
    SendSniMenuItemClicked {
        activation: super::menu::MenuActivation,
    },
    FetchSniMenuUpdate {
        service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
        anchor: (i32, i32),
        screen_size: (u32, u32),
    },
    FetchSniMenuForOpen {
        dbus_service: String,
        event_service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
        anchor: (i32, i32),
        screen_size: (u32, u32),
        origin_window: iced::window::Id,
    },
    CallSniActivate {
        item: crate::dbus::sni::host::TrayItem,
    },
    CallSniSecondaryActivate {
        item: crate::dbus::sni::host::TrayItem,
    },
    CallSniScroll {
        item: crate::dbus::sni::host::TrayItem,
        delta: i32,
        orientation: String,
    },
    ScrollWidgetTo {
        target: iced::widget::Id,
        y: f32,
    },
}

pub(super) fn app_effects_for_bar_surface_lifecycle(
    effects: Vec<super::surfaces::BarSurfaceLifecycleEffect>,
) -> Vec<AppEffect> {
    effects
        .into_iter()
        .map(|effect| match effect {
            super::surfaces::BarSurfaceLifecycleEffect::Open(desired) => {
                AppEffect::OpenBarSurface { desired }
            }
            super::surfaces::BarSurfaceLifecycleEffect::RemoveWindow(window) => {
                AppEffect::RemoveWindow(window)
            }
        })
        .collect()
}

pub(super) fn app_effects_for_foreground(
    effect: &super::foreground::ForegroundEffect,
) -> Option<Vec<AppEffect>> {
    use super::foreground::ForegroundEffect;

    match effect {
        ForegroundEffect::OpenPanelWindow { tree } => {
            Some(vec![AppEffect::OpenPanelWindow { tree: tree.clone() }])
        }
        ForegroundEffect::RemoveWindow(window) => Some(vec![AppEffect::RemoveWindow(*window)]),
        ForegroundEffect::SuspendBarKeyboardInteractivity => {
            Some(vec![AppEffect::SuspendBarKeyboardInteractivity])
        }
        ForegroundEffect::RestoreBarKeyboardInteractivity => {
            Some(vec![AppEffect::RestoreBarKeyboardInteractivity])
        }
        ForegroundEffect::OpenMenuWindow { request } => Some(vec![AppEffect::OpenMenuWindow {
            request: request.clone(),
        }]),
        ForegroundEffect::CloseMenu { window, meta } => Some(vec![
            AppEffect::SendSniMenuClosed { meta: meta.clone() },
            AppEffect::RemoveWindow(*window),
        ]),
        ForegroundEffect::LaunchInstalledApp(id) => {
            Some(vec![AppEffect::LaunchInstalledApp(id.clone())])
        }
        ForegroundEffect::SendDmenuResponse {
            responder,
            response,
        } => Some(vec![AppEffect::SendDmenuResponse {
            responder: responder.clone(),
            response: response.clone(),
        }]),
        ForegroundEffect::ScrollPanelMenuToFocused { window, y } => {
            Some(vec![AppEffect::ScrollWidgetTo {
                target: super::panel::panel_menu_scroll_id(*window),
                y: *y,
            }])
        }
        ForegroundEffect::CloseTooltipsForSurface(_)
        | ForegroundEffect::CloseAllTooltips
        | ForegroundEffect::RequestPanelSearchFocus
        | ForegroundEffect::LoadInstalledAppsThenPreparePanel(_)
        | ForegroundEffect::RunPanelAction { .. } => None,
    }
}

impl App {
    pub(super) fn apply_bar_surface_lifecycle_effects(
        &mut self,
        effects: Vec<super::surfaces::BarSurfaceLifecycleEffect>,
    ) -> Vec<Task<Message>> {
        self.apply_app_effects(app_effects_for_bar_surface_lifecycle(effects))
    }

    pub(super) fn apply_app_effects(&mut self, effects: Vec<AppEffect>) -> Vec<Task<Message>> {
        let mut tasks = Vec::new();
        for effect in effects {
            tasks.extend(self.apply_app_effect(effect));
        }
        tasks
    }

    fn apply_app_effect(&mut self, effect: AppEffect) -> Vec<Task<Message>> {
        match effect {
            AppEffect::OpenBarSurface { desired } => vec![self.open_bar_surface_window(desired)],
            AppEffect::OpenPanelWindow { tree } => vec![self.open_panel_window(tree)],
            AppEffect::OpenMenuWindow { request } => vec![self.open_menu_window(request)],
            AppEffect::OpenTooltipWindow {
                target_window,
                target_module,
                tree,
            } => self.open_tooltip_window(target_window, target_module, tree),
            AppEffect::RemoveWindow(window) => vec![Task::done(Message::RemoveWindow(window))],
            AppEffect::SuspendBarKeyboardInteractivity => self.suspend_bar_keyboard_interactivity(),
            AppEffect::RestoreBarKeyboardInteractivity => self.restore_bar_keyboard_interactivity(),
            AppEffect::RunConfiguredCommand(command) => {
                let _ = crate::command::spawn_configured_command_fire_and_forget(&command);
                Vec::new()
            }
            AppEffect::AdjustDefaultSinkVolume { step, up } => {
                adjust_default_sink_volume(step, up);
                Vec::new()
            }
            AppEffect::LaunchInstalledApp(id) => {
                self.launch_installed_app_by_id(&id);
                Vec::new()
            }
            AppEffect::SendDmenuResponse {
                responder,
                response,
            } => {
                send_ipc_response(responder, response);
                Vec::new()
            }
            AppEffect::SendSniMenuClosed { meta } => {
                send_menu_closed_event(self.shared.dbus.session.clone(), meta);
                Vec::new()
            }
            AppEffect::SendSniMenuItemClicked { activation } => {
                send_menu_item_clicked_event(self.shared.dbus.session.clone(), activation);
                Vec::new()
            }
            AppEffect::FetchSniMenuUpdate {
                service,
                menu_path,
                anchor,
                screen_size,
            } => vec![fetch_sni_menu_update(
                self.shared.dbus.session.clone(),
                service,
                menu_path,
                anchor,
                screen_size,
            )],
            AppEffect::FetchSniMenuForOpen {
                dbus_service,
                event_service,
                menu_path,
                anchor,
                screen_size,
                origin_window,
            } => vec![fetch_sni_menu_for_open(
                self.shared.dbus.session.clone(),
                dbus_service,
                event_service,
                menu_path,
                anchor,
                screen_size,
                origin_window,
            )],
            AppEffect::CallSniActivate { item } => {
                call_sni_activate(self.shared.dbus.session.clone(), item);
                Vec::new()
            }
            AppEffect::CallSniSecondaryActivate { item } => {
                call_sni_secondary_activate(self.shared.dbus.session.clone(), item);
                Vec::new()
            }
            AppEffect::CallSniScroll {
                item,
                delta,
                orientation,
            } => {
                call_sni_scroll(self.shared.dbus.session.clone(), item, delta, orientation);
                Vec::new()
            }
            AppEffect::ScrollWidgetTo { target, y } => vec![iced::widget::operation::scroll_to(
                target,
                iced::widget::operation::AbsoluteOffset {
                    x: None,
                    y: Some(y),
                },
            )],
        }
    }

    fn open_bar_surface_window(
        &mut self,
        desired: super::bar_surface_plan::PlannedBarSurface,
    ) -> Task<Message> {
        let output_name = desired.identity.output_name_for_surface_state();
        let bar_name = desired.identity.bar_name.clone();
        let settings = desired
            .physical
            .layer_shell_settings(&output_name, Self::namespace());
        let (id, task) = Message::layershell_open(settings);
        tracing::debug!(
            target: "niribar::surface",
            bar = bar_name.as_str(),
            output = output_name.as_str(),
            "opening bar surface"
        );
        self.surfaces
            .pending_surface_keys
            .insert(id, (output_name.clone(), bar_name.clone()));
        self.surfaces
            .surfaces
            .insert(id, crate::surface::SurfaceState::new(output_name, bar_name));
        task
    }

    fn open_tooltip_window(
        &mut self,
        target_window: iced::window::Id,
        target_module: crate::modules::ModuleId,
        tree: crate::surface::tooltip_popup::TooltipTree,
    ) -> Vec<Task<Message>> {
        let settings = super::surfaces::tooltip_popup_window_settings(&tree);
        let (popup_wid, open_task) = Message::layershell_open(settings);
        let effects = self
            .surfaces
            .open_tooltip_overlay(super::surfaces::TooltipPopup {
                window: popup_wid,
                target_window,
                target_module,
                tree,
            });
        let mut tasks = self.apply_registry_effects(effects);
        tasks.push(open_task);
        tasks
    }

    fn launch_installed_app_by_id(&mut self, id: &str) {
        #[cfg(test)]
        {
            self.foreground.record_launched_installed_app_for_test(id);
        }

        #[cfg(not(test))]
        {
            let Some(app) = self
                .load_installed_apps_for_panel()
                .into_iter()
                .find(|app| app.id == id)
            else {
                return;
            };
            let _ = crate::command::spawn_desktop_application_fire_and_forget(&app);
        }
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

fn adjust_default_sink_volume(step: u8, up: bool) {
    let arg = if up {
        format!("{step}%+")
    } else {
        format!("{step}%-")
    };
    tokio::task::spawn(async move {
        match crate::command::set_default_sink_volume(&arg).await {
            Ok(status) if status.success() => {}
            Ok(status) => tracing::warn!(
                target: "niribar::modules",
                ?status,
                "wpctl set-volume exited unsuccessfully"
            ),
            Err(err) => tracing::warn!(
                target: "niribar::modules",
                error = %err,
                "failed to run wpctl set-volume"
            ),
        }
    });
}

fn send_menu_closed_event(conn: Option<zbus::Connection>, meta: super::foreground::MenuPopupMeta) {
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

fn send_menu_item_clicked_event(
    conn: Option<zbus::Connection>,
    activation: super::menu::MenuActivation,
) {
    if let Some(conn) = conn {
        tokio::task::spawn(async move {
            let _ = crate::dbus::sni::host::send_menu_event(
                &conn,
                &activation.service,
                &activation.menu_path,
                activation.item_id,
                "clicked",
            )
            .await;
        });
    }
}

fn fetch_sni_menu_update(
    conn: Option<zbus::Connection>,
    service: String,
    menu_path: zbus::zvariant::OwnedObjectPath,
    anchor: (i32, i32),
    screen_size: (u32, u32),
) -> Task<Message> {
    let Some(conn) = conn else {
        return Task::none();
    };
    let svc = service.clone();
    let mp = menu_path.clone();
    Task::perform(
        async move { crate::dbus::sni::host::fetch_menu(&conn, &svc, &mp).await },
        move |tree_opt| {
            if let Some(mut tree) = tree_opt {
                tree.anchor = anchor;
                tree.screen_size = screen_size;
                Message::Menu(super::messages::MenuEvent::Updated {
                    service,
                    menu_path,
                    tree,
                })
            } else {
                Message::Menu(super::messages::MenuEvent::Closed)
            }
        },
    )
}

fn fetch_sni_menu_for_open(
    conn: Option<zbus::Connection>,
    dbus_service: String,
    event_service: String,
    menu_path: zbus::zvariant::OwnedObjectPath,
    anchor: (i32, i32),
    screen_size: (u32, u32),
    origin_window: iced::window::Id,
) -> Task<Message> {
    let Some(conn) = conn else {
        return Task::none();
    };
    let svc = dbus_service.clone();
    let mp = menu_path.clone();
    Task::perform(
        async move { crate::dbus::sni::host::fetch_menu(&conn, &svc, &mp).await },
        move |tree_opt| {
            if let Some(mut tree) = tree_opt {
                tree.anchor = anchor;
                tree.screen_size = screen_size;
                Message::Menu(super::messages::MenuEvent::Open {
                    service: event_service.clone(),
                    menu_path: menu_path.clone(),
                    tree,
                    origin_window,
                })
            } else {
                Message::Tray(super::messages::TrayRuntimeEvent::Activate {
                    service: event_service.clone(),
                })
            }
        },
    )
}

fn call_sni_activate(conn: Option<zbus::Connection>, item: crate::dbus::sni::host::TrayItem) {
    if let Some(conn) = conn {
        tokio::task::spawn(async move {
            let _ = crate::dbus::sni::host::call_activate(&conn, &item.service, &item.object_path)
                .await;
        });
    }
}

fn call_sni_secondary_activate(
    conn: Option<zbus::Connection>,
    item: crate::dbus::sni::host::TrayItem,
) {
    if let Some(conn) = conn {
        tokio::task::spawn(async move {
            let _ = crate::dbus::sni::host::call_secondary_activate(
                &conn,
                &item.service,
                &item.object_path,
            )
            .await;
        });
    }
}

fn call_sni_scroll(
    conn: Option<zbus::Connection>,
    item: crate::dbus::sni::host::TrayItem,
    delta: i32,
    orientation: String,
) {
    if let Some(conn) = conn {
        tokio::task::spawn(async move {
            let _ = crate::dbus::sni::host::call_scroll(
                &conn,
                &item.service,
                &item.object_path,
                delta,
                &orientation,
            )
            .await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn panel_origin() -> crate::surface::panel::PanelOrigin {
        crate::surface::panel::PanelOrigin::Bar
    }

    #[test]
    fn bar_surface_lifecycle_effects_map_to_external_app_effects() {
        let window = wid();
        let planned = super::super::bar_surface_plan::PlannedBarSurface::from_bar(
            &crate::config::BarConfig::default(),
            "DP-1",
        );

        let effects = app_effects_for_bar_surface_lifecycle(vec![
            super::super::surfaces::BarSurfaceLifecycleEffect::Open(planned.clone()),
            super::super::surfaces::BarSurfaceLifecycleEffect::RemoveWindow(window),
        ]);

        assert_eq!(effects.len(), 2);
        assert!(matches!(
            &effects[0],
            AppEffect::OpenBarSurface { desired } if desired == &planned
        ));
        assert!(matches!(effects[1], AppEffect::RemoveWindow(id) if id == window));
    }

    #[test]
    fn foreground_coordination_effects_do_not_become_app_effects() {
        let window = wid();
        let action = crate::config::PanelAction::ClosePanel;

        let coordination = vec![
            super::super::foreground::ForegroundEffect::CloseTooltipsForSurface(window),
            super::super::foreground::ForegroundEffect::CloseAllTooltips,
            super::super::foreground::ForegroundEffect::RequestPanelSearchFocus,
            super::super::foreground::ForegroundEffect::RunPanelAction {
                origin_window: window,
                origin: panel_origin(),
                action,
            },
        ];

        for effect in coordination {
            assert!(
                app_effects_for_foreground(&effect).is_none(),
                "coordination effect crossed the AppEffect seam: {effect:?}"
            );
        }
    }

    #[test]
    fn foreground_close_menu_maps_to_external_effects() {
        let window = wid();
        let meta = super::super::foreground::MenuPopupMeta {
            service: "org.test".into(),
            menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
            tree: crate::surface::menu_popup::MenuTree {
                items: Vec::new(),
                anchor: (0, 0),
                screen_size: (1920, 1080),
            },
            origin: super::super::surfaces::BarSurfaceIdentity::all_screens(),
        };

        let effects =
            app_effects_for_foreground(&super::super::foreground::ForegroundEffect::CloseMenu {
                window,
                meta,
            })
            .expect("close menu should map to external app effects");

        assert_eq!(effects.len(), 2);
        assert!(matches!(effects[0], AppEffect::SendSniMenuClosed { .. }));
        assert!(matches!(effects[1], AppEffect::RemoveWindow(id) if id == window));
    }
}
