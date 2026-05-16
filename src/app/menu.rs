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
    pub(super) tree: crate::surface::menu_popup::MenuTree,
    pub(super) bar_name: String,
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
    hovered: HashMap<iced::window::Id, String>,
    focused: HashMap<iced::window::Id, String>,
}

impl MenuSystem {
    pub(super) fn hovered_item(&self, window: iced::window::Id) -> Option<&str> {
        self.hovered.get(&window).map(String::as_str)
    }

    pub(super) fn focused_item(&self, window: iced::window::Id) -> Option<&str> {
        self.focused.get(&window).map(String::as_str)
    }

    pub(super) fn hover_enter(&mut self, window: iced::window::Id, item_id: String) {
        self.hovered.insert(window, item_id);
    }

    pub(super) fn hover_exit(&mut self, window: iced::window::Id) {
        self.hovered.remove(&window);
    }

    pub(super) fn focus(&mut self, window: iced::window::Id, item_id: String) {
        self.focused.insert(window, item_id);
    }

    pub(super) fn clear_window(&mut self, window: iced::window::Id) {
        self.hovered.remove(&window);
        self.focused.remove(&window);
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
    pub(super) fn menu_model(&self, window: iced::window::Id) -> Option<MenuModel> {
        let popup = self.surfaces.popup(window)?;
        Some(MenuModel {
            service: popup.service.clone(),
            menu_path: popup.menu_path.clone(),
            tree: popup.tree.clone(),
            bar_name: popup.bar_name.clone(),
        })
    }

    pub(super) fn close_all_menu_popups(&mut self) -> Vec<Task<Message>> {
        let ids: Vec<iced::window::Id> = self.surfaces.popup_ids().collect();
        ids.into_iter()
            .map(|id| {
                self.surfaces.remove_popup(id);
                self.menu.clear_window(id);
                Task::done(Message::RemoveWindow(id))
            })
            .collect()
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
            MenuEvent::LayoutChanged { service, menu_path } => {
                self.handle_menu_layout_changed(service, menu_path)
            }
            MenuEvent::Open {
                service,
                menu_path,
                tree,
                bar_name,
            } => self.open_menu_popup(service, menu_path, tree, bar_name),
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
        bar_name: String,
    ) -> Task<Message> {
        let close_tt: Vec<Task<Message>> = self
            .surfaces
            .surfaces()
            .map(|(wid, _)| *wid)
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|wid| self.close_tooltip_popup_if_open(wid))
            .collect();

        let existing_ids: Vec<iced::window::Id> = self.surfaces.popup_ids().collect();
        let mut tasks: Vec<Task<Message>> = close_tt;
        tasks.extend(existing_ids.into_iter().map(|id| {
            self.surfaces.remove_popup(id);
            self.menu.clear_window(id);
            Task::done(Message::RemoveWindow(id))
        }));

        let bar = Self::bar_for_surface_name(&self.config.bars, &bar_name);
        let settings = menu_popup_window_settings(&bar.edge, bar.collapsed_width, &tree);
        let (popup_id, open_task) = Message::popup_open(settings);
        let model = MenuModel {
            service,
            menu_path,
            tree,
            bar_name,
        };
        self.surfaces.insert_popup(
            popup_id,
            super::surfaces::PopupMeta {
                service: model.service,
                menu_path: model.menu_path,
                tree: model.tree,
                bar_name: model.bar_name,
            },
        );
        tasks.push(open_task);
        Task::batch(tasks)
    }

    pub(super) fn update_menu_popup(
        &mut self,
        service: String,
        menu_path: zbus::zvariant::OwnedObjectPath,
        tree: crate::surface::menu_popup::MenuTree,
    ) -> Task<Message> {
        let Some((_, popup)) = self
            .surfaces
            .popups
            .iter_mut()
            .find(|(_, popup)| popup.service == service && popup.menu_path == menu_path)
        else {
            return Task::none();
        };

        popup.tree = tree;
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
        let conn = self.shared.dbus.session.clone();
        for meta in self.surfaces.popup_values() {
            if let Some(conn) = conn.clone() {
                let service = meta.service.clone();
                let menu_path = meta.menu_path.clone();
                tokio::task::spawn(async move {
                    let _ = crate::dbus::sni::host::send_menu_event(
                        &conn, &service, &menu_path, 0, "closed",
                    )
                    .await;
                });
            }
        }

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
                        on_click: crate::config::CommandConfig::Shell("true".into()),
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
            items: vec![MenuItem {
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
        let path = zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap();
        app.surfaces.insert_popup(
            wid,
            surfaces::PopupMeta {
                service: "org.test".into(),
                menu_path: path,
                tree,
                bar_name: String::new(),
            },
        );
        wid
    }

    #[test]
    fn menu_item_hover_enter_sets_hovered_item() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        let _ = app.update(Message::Menu(messages::MenuEvent::ItemHoverEnter {
            window: wid,
            item_id: "1".into(),
        }));
        assert_eq!(app.menu.hovered_item(wid), Some("1"));
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
        assert_eq!(app.menu.hovered_item(wid), Some("2"));
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
        app.surfaces.insert_popup(
            popup_wid,
            surfaces::PopupMeta {
                service: "svc".into(),
                menu_path,
                tree,
                bar_name: String::new(),
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
        app.surfaces.insert_popup(
            popup_wid,
            surfaces::PopupMeta {
                service: "svc".into(),
                menu_path,
                tree,
                bar_name: String::new(),
            },
        );
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid), Some("1"));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid), Some("2"));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid), Some("1"));
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
        app.surfaces.insert_popup(
            popup_wid,
            surfaces::PopupMeta {
                service: "svc".into(),
                menu_path,
                tree,
                bar_name: String::new(),
            },
        );
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMovePrev(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid), Some("2"));
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
        app.surfaces.insert_popup(
            popup_wid,
            surfaces::PopupMeta {
                service: "svc".into(),
                menu_path,
                tree,
                bar_name: String::new(),
            },
        );
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid), Some("1"));
        let _ = app.update(Message::Surface(messages::SurfaceEvent::FocusMoveNext(
            popup_wid,
        )));
        assert_eq!(app.menu.focused_item(popup_wid), Some("4"));
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
        app.surfaces.insert_popup(
            popup_wid,
            surfaces::PopupMeta {
                service: "svc".into(),
                menu_path,
                tree,
                bar_name: String::new(),
            },
        );
        app.menu.focus(popup_wid, "1".into());
        let _ = app.update(Message::Menu(messages::MenuEvent::Closed));
        assert!(app.surfaces[&wid].focused_module.is_none());
        assert!(!app.surfaces[&wid].module_activated);
        assert!(app.menu.focused.is_empty());
    }

    #[test]
    fn close_all_menu_popups_removes_popup_and_focus_maps() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = make_popup_wid(&mut app);
        app.menu.hover_enter(wid, "1".into());
        app.menu.focus(wid, "1".into());

        let tasks = app.close_all_menu_popups();

        assert!(app.surfaces.popup(wid).is_none());
        assert!(app.menu.hovered.is_empty());
        assert!(app.menu.focused.is_empty());
        assert_eq!(tasks.len(), 1);
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
