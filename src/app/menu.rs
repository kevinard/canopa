use std::collections::HashMap;

use iced::Task;
use iced_layershell::actions::IcedNewPopupSettings;
use iced_layershell::reexport::KeyboardInteractivity;

use super::{App, Message};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MenuActivation {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) item_id: i32,
}

#[derive(Debug, Clone)]
pub(super) struct MenuModel {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
}

impl MenuModel {
    pub(super) fn activation_for(&self, item_id: i32) -> MenuActivation {
        MenuActivation {
            service: self.service.clone(),
            menu_path: self.menu_path.clone(),
            item_id,
        }
    }
}

#[derive(Default)]
pub(super) struct MenuSystem {
    sessions: HashMap<iced::window::Id, super::selectable_menu_session::SelectableMenuSession>,
}

impl MenuSystem {
    pub(super) fn open_window(
        &mut self,
        window: iced::window::Id,
        tree: crate::surface::menu_popup::MenuTree,
        search_enabled: bool,
    ) {
        self.sessions.insert(
            window,
            super::selectable_menu_session::SelectableMenuSession::open(
                super::selectable_menu_session::SelectableMenuSource::SniMenu {
                    tree,
                    search_enabled,
                },
            ),
        );
    }

    pub(super) fn hovered_item(&self, window: iced::window::Id) -> Option<String> {
        self.sessions
            .get(&window)
            .and_then(|session| session.hovered_sni_item())
    }

    pub(super) fn focused_item(&self, window: iced::window::Id) -> Option<String> {
        self.sessions
            .get(&window)
            .and_then(|session| session.focused_sni_item())
    }

    pub(super) fn hover_enter(&mut self, window: iced::window::Id, item_id: String) {
        if let Some(session) = self.sessions.get_mut(&window) {
            session.hover_sni_item(&item_id);
        }
    }

    pub(super) fn hover_exit(&mut self, window: iced::window::Id) {
        if let Some(session) = self.sessions.get_mut(&window) {
            session.clear_hover();
        }
    }

    #[cfg(test)]
    pub(super) fn focus(&mut self, window: iced::window::Id, item_id: String) {
        if let Some(session) = self.sessions.get_mut(&window) {
            session.focus_sni_item(&item_id);
        }
    }

    pub(super) fn move_focus(&mut self, window: iced::window::Id, forward: bool) {
        if let Some(session) = self.sessions.get_mut(&window) {
            session.move_focus(if forward { 1 } else { -1 });
        }
    }

    pub(super) fn set_search_query(&mut self, window: iced::window::Id, query: String) {
        if let Some(session) = self.sessions.get_mut(&window) {
            session.set_search_query(query);
        }
    }

    #[cfg(test)]
    pub(super) fn snapshot(
        &self,
        window: iced::window::Id,
    ) -> Option<super::selectable_menu_session::SelectableMenuSnapshot> {
        self.sessions.get(&window).map(|session| session.snapshot())
    }

    pub(super) fn session(
        &self,
        window: iced::window::Id,
    ) -> Option<&super::selectable_menu_session::SelectableMenuSession> {
        self.sessions.get(&window)
    }

    pub(super) fn clear_window(&mut self, window: iced::window::Id) {
        self.sessions.remove(&window);
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

fn menu_popup_window_settings(
    edge: &crate::config::Edge,
    bar_size: u32,
    tree: &crate::surface::menu_popup::MenuTree,
) -> IcedNewPopupSettings {
    let size = crate::surface::menu_popup::popup_size(tree);
    let placement = crate::surface::placement::edge_adjacent_position(
        edge,
        crate::surface::placement::Point {
            x: tree.anchor.0,
            y: tree.anchor.1,
        },
        crate::surface::placement::Size {
            width: size.0,
            height: size.1,
        },
        crate::surface::placement::Size {
            width: tree.screen_size.0,
            height: tree.screen_size.1,
        },
        crate::surface::placement::Point::default(),
        bar_size,
        0,
    );

    IcedNewPopupSettings {
        size,
        position: (placement.position.x, placement.position.y),
        grab: true,
    }
}

impl App {
    fn sni_menu_search_enabled(&self) -> bool {
        self.config.modules.iter().any(|module| {
            matches!(
                &module.params,
                crate::config::ModuleParams::Tray(tray) if tray.menu_search
            )
        })
    }

    pub(super) fn menu_model(&self, window: iced::window::Id) -> Option<MenuModel> {
        let popup = self.surfaces.popup(window)?;
        Some(MenuModel {
            service: popup.service.clone(),
            menu_path: popup.menu_path.clone(),
        })
    }

    pub(super) fn close_all_menu_popups(&mut self) -> Vec<Task<Message>> {
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::CloseMenu);
        if effects.is_empty() {
            self.surfaces.clear_pending_menu_popup();
            let effects = self.surfaces.close_menu_overlays();
            self.apply_registry_effects(effects)
        } else {
            self.apply_foreground_effects(effects)
        }
    }

    pub(super) fn restore_bar_keyboard_interactivity(&mut self) -> Vec<Task<Message>> {
        let ids: Vec<iced::window::Id> = self.surfaces.keys().copied().collect();
        for surface in self.surfaces.values_mut() {
            surface.focused_module = None;
            surface.module_activated = false;
        }
        ids.into_iter()
            .map(|id| {
                Task::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::OnDemand,
                })
            })
            .collect()
    }

    pub(super) fn handle_menu_event(&mut self, event: super::messages::MenuEvent) -> Task<Message> {
        use super::messages::MenuEvent;

        match event {
            MenuEvent::ItemSelected {
                service,
                menu_item_id,
                menu_path,
            } => self.handle_menu_item_selected(service, menu_item_id, menu_path),
            MenuEvent::Closed => self.handle_menu_closed(),
            MenuEvent::ItemHoverEnter { window, item_id } => {
                self.menu.hover_enter(window, item_id);
                Task::none()
            }
            MenuEvent::ItemHoverExit(window) => {
                self.menu.hover_exit(window);
                Task::none()
            }
            MenuEvent::SearchChanged { window, query } => {
                self.menu.set_search_query(window, query);
                Task::none()
            }
            MenuEvent::LayoutChanged { service, menu_path } => {
                self.handle_menu_layout_changed(service, menu_path)
            }
            MenuEvent::Open {
                service,
                menu_path,
                tree,
                origin_window,
            } => self.open_menu_popup(service, menu_path, tree, origin_window),
            MenuEvent::Updated {
                service,
                menu_path,
                tree,
            } => self.update_menu_popup(service, menu_path, tree),
        }
    }

    pub(super) fn handle_menu_item_selected(
        &mut self,
        service: String,
        menu_item_id: i32,
        menu_path: zbus::zvariant::OwnedObjectPath,
    ) -> Task<Message> {
        let conn = self.shared.dbus.session.clone();
        tokio::task::spawn(async move {
            if let Some(conn) = conn {
                let _ = crate::dbus::sni::host::send_menu_event(
                    &conn,
                    &service,
                    &menu_path,
                    menu_item_id,
                    "clicked",
                )
                .await;
            }
        });

        let mut tasks = self.close_all_menu_popups();
        tasks.extend(self.restore_bar_keyboard_interactivity());
        Task::batch(tasks)
    }

    pub(super) fn open_menu_popup(
        &mut self,
        service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
        tree: crate::surface::menu_popup::MenuTree,
        origin_window: iced::window::Id,
    ) -> Task<Message> {
        let meta = super::surfaces::PopupMeta {
            service,
            menu_path,
            tree,
            origin: self
                .surfaces
                .bar_surface_identity_for_window(origin_window)
                .unwrap_or_else(super::surfaces::BarSurfaceIdentity::all_screens),
        };
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::OpenMenu { meta });
        Task::batch(self.apply_foreground_effects(effects))
    }

    pub(super) fn open_menu_window(&mut self, meta: super::surfaces::PopupMeta) -> Task<Message> {
        let bar = Self::bar_for_surface_identity(&self.config.bars, &meta.origin);
        let settings = menu_popup_window_settings(&bar.edge, bar.collapsed_width, &meta.tree);
        let (popup_id, open_task) = Message::popup_open(settings);
        self.surfaces
            .register_ready_menu_overlay(popup_id, meta.clone());
        self.menu
            .open_window(popup_id, meta.tree.clone(), self.sni_menu_search_enabled());
        self.foreground
            .handle(super::foreground::ForegroundIntent::MenuWindowOpened {
                window: popup_id,
                meta,
            });
        open_task
    }

    pub(super) fn open_ready_menu_popup(
        &mut self,
        meta: super::surfaces::PopupMeta,
    ) -> Task<Message> {
        let bar = Self::bar_for_surface_identity(&self.config.bars, &meta.origin);
        let settings = menu_popup_window_settings(&bar.edge, bar.collapsed_width, &meta.tree);
        let (popup_id, open_task) = Message::popup_open(settings);
        self.surfaces
            .register_ready_menu_overlay(popup_id, meta.clone());
        self.menu
            .open_window(popup_id, meta.tree.clone(), self.sni_menu_search_enabled());
        self.foreground
            .handle(super::foreground::ForegroundIntent::MenuWindowOpened {
                window: popup_id,
                meta,
            });
        open_task
    }

    pub(super) fn update_menu_popup(
        &mut self,
        service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
        tree: crate::surface::menu_popup::MenuTree,
    ) -> Task<Message> {
        let search_enabled = self.sni_menu_search_enabled();
        let Some((window, popup)) = self
            .surfaces
            .popups
            .iter_mut()
            .find(|(_, popup)| popup.service == service && popup.menu_path == menu_path)
        else {
            return Task::none();
        };

        popup.tree = tree.clone();
        self.menu.open_window(*window, tree, search_enabled);
        Task::none()
    }

    pub(super) fn handle_menu_layout_changed(
        &mut self,
        service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
    ) -> Task<Message> {
        let conn = self.shared.dbus.session.clone();
        let existing = self
            .surfaces
            .popup_values()
            .find(|popup| popup.service == service && popup.menu_path == menu_path);
        let (anchor, screen_size) = match existing {
            Some(popup) => (popup.tree.anchor, popup.tree.screen_size),
            None => return Task::none(),
        };
        if let Some(conn) = conn {
            let svc = service.clone();
            let mp = menu_path.clone();
            return Task::perform(
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
            );
        }
        Task::none()
    }

    pub(super) fn handle_menu_closed(&mut self) -> Task<Message> {
        let mut tasks = self.close_all_menu_popups();
        tasks.extend(self.restore_bar_keyboard_interactivity());
        Task::batch(tasks)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Message, messages, surfaces};
    use crate::config::Config;

    fn make_config() -> Config {
        Config::default()
    }

    fn make_searchable_tray_config() -> Config {
        use crate::config::{ModuleConfig, ModuleParams, TrayConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "tray".into(),
                module_type: "tray".into(),
                params: ModuleParams::Tray(TrayConfig {
                    menu_search: true,
                    ..TrayConfig::default()
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Config::default()
        };
        config.bars[0].modules.end = vec!["tray".into()];
        config
    }

    fn test_wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn make_interact_app() -> (App, iced::window::Id) {
        use crate::config::{ButtonConfig, ClockConfig, ModuleConfig, ModuleParams, VolumeConfig};
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "clock".into(),
                    module_type: "clock".into(),
                    params: ModuleParams::Clock(ClockConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "volume".into(),
                    module_type: "volume".into(),
                    params: ModuleParams::Volume(VolumeConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "button".into(),
                    module_type: "button".into(),
                    params: ModuleParams::Button(ButtonConfig {
                        label: "test".into(),
                        icon: ">".into(),
                        format: crate::config::FormatConfig::default(),
                        tooltip: crate::config::TooltipSetting::Default,
                    }),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["clock".into(), "volume".into()];
        config.bars[0].modules.center = vec!["button".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surface_mut(wid);
        (app, wid)
    }

    fn make_popup_wid(app: &mut App) -> iced::window::Id {
        use crate::surface::menu_popup::{MenuItem, MenuTree};
        let wid = test_wid();
        let tree = MenuTree {
            items: vec![
                MenuItem {
                    id: "1".into(),
                    label: "Open".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                MenuItem {
                    id: "2".into(),
                    label: "Close".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
            ],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let path = zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap();
        app.register_menu_popup(
            wid,
            surfaces::PopupMeta {
                service: "org.test".into(),
                menu_path: path,
                tree,
                origin: surfaces::BarSurfaceIdentity::all_screens(),
            },
        );
        wid
    }

    fn menu_tree(anchor: (i32, i32)) -> crate::surface::menu_popup::MenuTree {
        crate::surface::menu_popup::MenuTree {
            items: vec![crate::surface::menu_popup::MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor,
            screen_size: (500, 48),
        }
    }

    #[test]
    fn tray_menu_search_config_controls_sni_session_search() {
        let (mut app, _) = App::boot(make_searchable_tray_config(), vec![]);
        let wid = test_wid();
        app.menu
            .open_window(wid, menu_tree((0, 0)), app.sni_menu_search_enabled());

        assert!(
            app.menu
                .session(wid)
                .is_some_and(|session| session.search_enabled())
        );

        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.menu
            .open_window(wid, menu_tree((0, 0)), app.sni_menu_search_enabled());

        assert!(
            app.menu
                .session(wid)
                .is_some_and(|session| !session.search_enabled())
        );
    }

    #[test]
    fn sni_menu_update_resets_search_and_focus() {
        let (mut app, _) = App::boot(make_searchable_tray_config(), vec![]);
        let wid = test_wid();
        let menu_path = zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap();
        app.register_menu_popup(
            wid,
            surfaces::PopupMeta {
                service: "org.test".into(),
                menu_path: menu_path.clone(),
                tree: menu_tree((0, 0)),
                origin: surfaces::BarSurfaceIdentity::all_screens(),
            },
        );
        app.menu
            .open_window(wid, menu_tree((0, 0)), app.sni_menu_search_enabled());
        app.menu.set_search_query(wid, "open".into());
        app.menu.move_focus(wid, true);

        let _ = app.handle_menu_event(messages::MenuEvent::Updated {
            service: "org.test".into(),
            menu_path,
            tree: menu_tree((12, 34)),
        });

        let snapshot = app.menu.snapshot(wid).expect("sni session");
        assert!(snapshot.search_enabled);
        assert_eq!(snapshot.search_query, "");
        assert_eq!(snapshot.focused_index, None);
        assert_eq!(
            app.surfaces.popup(wid).expect("popup").tree.anchor,
            (12, 34)
        );
    }

    #[test]
    fn menu_item_hover_enter_sets_hovered_item() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let _ = app.update(Message::Menu(messages::MenuEvent::ItemHoverEnter {
            window: wid,
            item_id: "1".into(),
        }));
        assert_eq!(app.menu.hovered_item(wid).as_deref(), Some("1"));
    }

    #[test]
    fn menu_item_hover_exit_clears_hovered_item() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let _ = app.update(Message::Menu(messages::MenuEvent::ItemHoverEnter {
            window: wid,
            item_id: "1".into(),
        }));
        let _ = app.update(Message::Menu(messages::MenuEvent::ItemHoverExit(wid)));
        assert_eq!(app.menu.hovered_item(wid), None);
    }

    #[test]
    fn menu_item_hover_enter_different_popups_are_independent() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid_a = make_popup_wid(&mut app);
        let wid_b = make_popup_wid(&mut app);
        let _ = app.update(Message::Menu(messages::MenuEvent::ItemHoverEnter {
            window: wid_a,
            item_id: "1".into(),
        }));
        assert_eq!(app.menu.hovered_item(wid_b), None);
    }

    #[test]
    fn menu_item_hover_enter_on_second_item_replaces_first_without_exit() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let _ = app.update(Message::Menu(messages::MenuEvent::ItemHoverEnter {
            window: wid,
            item_id: "1".into(),
        }));
        let _ = app.update(Message::Menu(messages::MenuEvent::ItemHoverEnter {
            window: wid,
            item_id: "2".into(),
        }));
        assert_eq!(app.menu.hovered_item(wid).as_deref(), Some("2"));
    }

    #[test]
    fn escape_in_popup_fires_menu_closed() {
        let (mut app, _wid) = make_interact_app();
        let menu_path =
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![crate::surface::menu_popup::MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let popup_wid = iced::window::Id::unique();
        app.register_menu_popup(
            popup_wid,
            surfaces::PopupMeta {
                service: "svc".into(),
                menu_path,
                tree,
                origin: surfaces::BarSurfaceIdentity::all_screens(),
            },
        );
        let _ = app.update(Message::Surface(messages::SurfaceEvent::EscapePressed(
            popup_wid,
        )));
        let _ = app.update(Message::Menu(messages::MenuEvent::Closed));
        assert!(app.surfaces.popup(popup_wid).is_none());
    }

    #[test]
    fn popup_focus_loss_fires_menu_closed() {
        let (mut app, _wid) = make_interact_app();
        let popup_wid = make_popup_wid(&mut app);

        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusLost(
            popup_wid,
        )));
        let _ = app.update(Message::Menu(messages::MenuEvent::Closed));

        assert!(app.surfaces.popup(popup_wid).is_none());
    }

    #[test]
    fn arrow_down_in_popup_focuses_first_item() {
        let (mut app, _wid) = make_interact_app();
        let menu_path =
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![
                crate::surface::menu_popup::MenuItem {
                    id: "1".into(),
                    label: "Open".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                crate::surface::menu_popup::MenuItem {
                    id: "2".into(),
                    label: "Close".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
            ],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let popup_wid = iced::window::Id::unique();
        app.register_menu_popup(
            popup_wid,
            surfaces::PopupMeta {
                service: "svc".into(),
                menu_path,
                tree,
                origin: surfaces::BarSurfaceIdentity::all_screens(),
            },
        );
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid).as_deref(), Some("1"));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid).as_deref(), Some("2"));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid).as_deref(), Some("1"));
    }

    #[test]
    fn arrow_up_in_popup_wraps_to_last() {
        let (mut app, _wid) = make_interact_app();
        let menu_path =
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![
                crate::surface::menu_popup::MenuItem {
                    id: "1".into(),
                    label: "Open".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                crate::surface::menu_popup::MenuItem {
                    id: "2".into(),
                    label: "Close".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
            ],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let popup_wid = iced::window::Id::unique();
        app.register_menu_popup(
            popup_wid,
            surfaces::PopupMeta {
                service: "svc".into(),
                menu_path,
                tree,
                origin: surfaces::BarSurfaceIdentity::all_screens(),
            },
        );
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMovePrev(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid).as_deref(), Some("2"));
    }

    #[test]
    fn popup_skips_separators_and_disabled_items() {
        let (mut app, _wid) = make_interact_app();
        let menu_path =
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![
                crate::surface::menu_popup::MenuItem {
                    id: "1".into(),
                    label: "Open".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                crate::surface::menu_popup::MenuItem {
                    id: "0".into(),
                    label: String::new(),
                    enabled: false,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: true,
                },
                crate::surface::menu_popup::MenuItem {
                    id: "3".into(),
                    label: "Disabled".into(),
                    enabled: false,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                crate::surface::menu_popup::MenuItem {
                    id: "4".into(),
                    label: "Close".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
            ],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let popup_wid = iced::window::Id::unique();
        app.register_menu_popup(
            popup_wid,
            surfaces::PopupMeta {
                service: "svc".into(),
                menu_path,
                tree,
                origin: surfaces::BarSurfaceIdentity::all_screens(),
            },
        );
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid).as_deref(), Some("1"));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid).as_deref(), Some("4"));
    }

    #[test]
    fn menu_closed_clears_bar_focus_on_all_surfaces() {
        let (mut app, wid) = make_interact_app();
        let _ = app.update(Message::Surface(messages::SurfaceEvent::BarFocusGained(
            wid,
        )));
        assert!(app.surfaces[&wid].focused_module.is_some());
        let menu_path =
            zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let popup_wid = iced::window::Id::unique();
        app.register_menu_popup(
            popup_wid,
            surfaces::PopupMeta {
                service: "svc".into(),
                menu_path,
                tree,
                origin: surfaces::BarSurfaceIdentity::all_screens(),
            },
        );
        app.menu.focus(popup_wid, "1".into());
        let _ = app.update(Message::Menu(messages::MenuEvent::Closed));
        assert!(app.surfaces[&wid].focused_module.is_none());
        assert!(!app.surfaces[&wid].module_activated);
        assert!(app.menu.is_empty());
    }

    #[test]
    fn close_all_menu_popups_removes_popup_and_focus_maps() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        app.menu.hover_enter(wid, "1".into());
        app.menu.focus(wid, "1".into());

        let tasks = app.close_all_menu_popups();

        assert!(app.surfaces.popup(wid).is_none());
        assert!(app.menu.is_empty());
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn opening_menu_while_menu_is_open_waits_for_old_window_closed() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let old_wid = make_popup_wid(&mut app);
        let new_path = zbus::zvariant::OwnedObjectPath::try_from("/new_menu").unwrap();

        let _ = app.open_menu_popup("org.next".into(), new_path, menu_tree((24, 12)), test_wid());

        assert!(app.surfaces.popup(old_wid).is_none());
        assert!(app.foreground.is_menu_window(old_wid));
        assert_eq!(app.foreground.active_menu_window(), None);
        assert!(app.surfaces.popups.is_empty());
    }

    #[test]
    fn pending_menu_opens_after_old_menu_window_closed() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let old_wid = make_popup_wid(&mut app);
        let new_path = zbus::zvariant::OwnedObjectPath::try_from("/new_menu").unwrap();

        let _ = app.open_menu_popup(
            "org.next".into(),
            new_path.clone(),
            menu_tree((24, 12)),
            test_wid(),
        );
        let _ = app.update(Message::Surface(messages::SurfaceEvent::WindowClosed(
            old_wid,
        )));

        assert!(app.surfaces.closing_menu_popups.is_empty());
        assert!(!app.surfaces.has_pending_menu_for_test());
        let popup = app
            .surfaces
            .popup_values()
            .next()
            .expect("pending popup should open");
        assert_eq!(popup.service, "org.next");
        assert_eq!(popup.menu_path, new_path);
    }

    #[test]
    fn menu_tree_update_replaces_existing_popup_without_reopening() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let menu_path = zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap();
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![crate::surface::menu_popup::MenuItem {
                id: "2".into(),
                label: "Updated".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor: (12, 34),
            screen_size: (500, 48),
        };

        let task = app.handle_menu_event(messages::MenuEvent::Updated {
            service: "org.test".into(),
            menu_path,
            tree,
        });

        let popup = app.surfaces.popup(wid).expect("popup should stay open");
        assert_eq!(popup.tree.items[0].label, "Updated");
        assert_eq!(popup.tree.anchor, (12, 34));
        let _ = task;
    }

    #[test]
    fn bottom_edge_menu_popup_opens_above_parent_bar() {
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![crate::surface::menu_popup::MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor: (490, 30),
            screen_size: (500, 48),
        };
        let settings = super::menu_popup_window_settings(&crate::config::Edge::Bottom, 48, &tree);

        assert_eq!(settings.size, crate::surface::menu_popup::popup_size(&tree));
        assert_eq!(settings.position.0, 350);
        assert_eq!(
            settings.position.1,
            -(crate::surface::menu_popup::popup_size(&tree).1 as i32)
        );
    }

    #[test]
    fn top_edge_menu_popup_uses_shared_placement_clamping() {
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![crate::surface::menu_popup::MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor: (490, 30),
            screen_size: (500, 48),
        };
        let settings = super::menu_popup_window_settings(&crate::config::Edge::Top, 48, &tree);

        assert_eq!(settings.size, crate::surface::menu_popup::popup_size(&tree));
        assert_eq!(settings.position, (350, 48));
    }
}
