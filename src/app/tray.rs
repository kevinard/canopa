use std::collections::BTreeMap;

use iced::Task;
use zbus::zvariant::OwnedObjectPath;

use super::{App, Message, messages};
use crate::dbus::sni::host::{ItemStatus, TrayEvent, TrayItem};
use crate::modules::ModuleKind;

#[derive(Debug, Clone, Default)]
pub(crate) struct TrayState {
    items: BTreeMap<String, TrayItem>,
}

impl TrayState {
    pub(crate) fn apply_host_event(&mut self, event: &TrayEvent) -> Vec<TrayItem> {
        match event {
            TrayEvent::Snapshot(items) => {
                self.items.clear();
                for item in items {
                    if item.status != ItemStatus::Passive {
                        self.items.insert(item.runtime_id(), item.clone());
                    }
                }
            }
            TrayEvent::ItemAdded(item) => {
                if item.status == ItemStatus::Passive {
                    self.items.remove(&item.runtime_id());
                } else {
                    self.items.insert(item.runtime_id(), item.clone());
                }
            }
            TrayEvent::ItemRemoved(service) => {
                self.items.remove(service);
            }
        }

        self.items.values().cloned().collect()
    }

    fn item(&self, service: &str) -> Option<&TrayItem> {
        self.items.get(service)
    }
}

impl App {
    fn tray_item(&self, id: &str) -> Option<&TrayItem> {
        self.tray.item(id)
    }

    fn tray_menu_target(
        &self,
        id: &str,
        menu_path: Option<OwnedObjectPath>,
    ) -> Option<(String, OwnedObjectPath)> {
        let item = self.tray_item(id)?;
        let menu_path = menu_path.or_else(|| item.menu_path.clone())?;
        Some((item.service.clone(), menu_path))
    }

    fn find_tray_slots(&self) -> Vec<(usize, crate::modules::Zone, String)> {
        self.modules
            .iter()
            .enumerate()
            .filter(|(_, (_, m))| m.kind() == ModuleKind::Tray)
            .map(|(i, (z, m))| (i, z.clone(), m.id().0.clone()))
            .collect()
    }

    pub(super) fn handle_tray_runtime_event(
        &mut self,
        event: messages::TrayRuntimeEvent,
    ) -> Task<Message> {
        match event {
            messages::TrayRuntimeEvent::Host(event) => self.handle_tray_event(event),
            messages::TrayRuntimeEvent::Activate { service } => self.handle_tray_activate(service),
            messages::TrayRuntimeEvent::MenuRequest {
                service,
                menu_path,
                source_window,
            } => self.handle_tray_menu_request(service, menu_path, source_window),
            messages::TrayRuntimeEvent::MenuRequestKeyboard {
                service,
                menu_path,
                source_window,
            } => self.handle_tray_menu_request(service, menu_path, source_window),
            messages::TrayRuntimeEvent::MenuRequestWithBounds {
                service,
                menu_path,
                bounds,
                screen_size,
                source_window,
            } => self.handle_tray_menu_request_with_bounds(
                service,
                menu_path,
                bounds,
                screen_size,
                source_window,
            ),
            messages::TrayRuntimeEvent::SecondaryActivate { service } => {
                self.handle_tray_secondary_activate(service)
            }
            messages::TrayRuntimeEvent::Scroll {
                service,
                delta,
                orientation,
            } => self.handle_tray_scroll(service, delta, orientation),
        }
    }

    pub(super) fn handle_tray_event(&mut self, event: TrayEvent) -> Task<Message> {
        use crate::modules::tray::TrayIconModule;

        if let Some(perf) = &self.perf {
            perf.record_tray_event();
        }

        let active_items = self.tray.apply_host_event(&event);

        match event {
            TrayEvent::Snapshot(_) => {
                self.modules
                    .retain(|(_, m)| m.kind() != ModuleKind::TrayIcon);
                self.clear_surface_module_refs_with_prefix("tray:");

                let tray_slots = self.find_tray_slots();
                for (slot_idx, zone, config_name) in tray_slots.iter().rev() {
                    let insert_at = slot_idx + 1;
                    for item in active_items.iter().rev() {
                        let icon_module = TrayIconModule::new(item.clone(), config_name.clone());
                        self.modules
                            .insert(insert_at, (zone.clone(), Box::new(icon_module)));
                    }
                }
            }
            TrayEvent::ItemAdded(item) => {
                let service_id = format!("tray:{}", item.runtime_id());
                self.remove_tray_icon(&service_id);

                if item.status != ItemStatus::Passive {
                    let tray_slots = self.find_tray_slots();
                    for (slot_idx, zone, config_name) in tray_slots.iter().rev() {
                        let insert_at = self.tray_insert_position(*slot_idx, config_name);
                        let icon_module = TrayIconModule::new(item.clone(), config_name.clone());
                        self.modules
                            .insert(insert_at, (zone.clone(), Box::new(icon_module)));
                    }
                }
            }
            TrayEvent::ItemRemoved(service) => {
                let service_id = format!("tray:{service}");
                self.remove_tray_icon(&service_id);
            }
        }

        Task::none()
    }

    pub(super) fn handle_tray_activate(&self, service: String) -> Task<Message> {
        let item = self.tray.item(&service).cloned();
        let conn = self.shared.dbus.session.clone();
        tokio::task::spawn(async move {
            if let (Some(conn), Some(item)) = (conn, item) {
                let _ =
                    crate::dbus::sni::host::call_activate(&conn, &item.service, &item.object_path)
                        .await;
            }
        });
        Task::none()
    }

    pub(super) fn handle_tray_secondary_activate(&self, service: String) -> Task<Message> {
        let item = self.tray.item(&service).cloned();
        let conn = self.shared.dbus.session.clone();
        tokio::task::spawn(async move {
            if let (Some(conn), Some(item)) = (conn, item) {
                let _ = crate::dbus::sni::host::call_secondary_activate(
                    &conn,
                    &item.service,
                    &item.object_path,
                )
                .await;
            }
        });
        Task::none()
    }

    pub(super) fn handle_tray_scroll(
        &self,
        service: String,
        delta: i32,
        orientation: String,
    ) -> Task<Message> {
        let item = self.tray.item(&service).cloned();
        let conn = self.shared.dbus.session.clone();
        tokio::task::spawn(async move {
            if let (Some(conn), Some(item)) = (conn, item) {
                let _ = crate::dbus::sni::host::call_scroll(
                    &conn,
                    &item.service,
                    &item.object_path,
                    delta,
                    &orientation,
                )
                .await;
            }
        });
        Task::none()
    }

    pub(super) fn handle_tray_menu_request(
        &self,
        service: String,
        menu_path: Option<OwnedObjectPath>,
        source_window: Option<iced::window::Id>,
    ) -> Task<Message> {
        let Some(source_window) = source_window else {
            return Task::none();
        };
        let screen_size = self
            .surfaces
            .surface(source_window)
            .map(|s| s.screen_size)
            .unwrap_or((0, 0));
        let target = crate::modules::tray::tray_icon_widget_id(source_window, &service);
        let op = super::widget_ops::FindBounds {
            target,
            result: None,
        };
        let svc = service.clone();
        let mp = menu_path.clone();
        iced::advanced::widget::operate(op).then(move |bounds| {
            Task::done(Message::Tray(
                messages::TrayRuntimeEvent::MenuRequestWithBounds {
                    service: svc.clone(),
                    menu_path: mp.clone(),
                    bounds,
                    screen_size,
                    source_window: Some(source_window),
                },
            ))
        })
    }

    pub(super) fn handle_tray_menu_request_with_bounds(
        &self,
        service: String,
        menu_path: Option<OwnedObjectPath>,
        bounds: Option<iced::Rectangle>,
        screen_size: (u32, u32),
        source_window: Option<iced::window::Id>,
    ) -> Task<Message> {
        let conn = self.shared.dbus.session.clone();
        if let (Some(conn), Some((dbus_service, menu_path))) =
            (conn, self.tray_menu_target(&service, menu_path))
        {
            let service_clone = dbus_service.clone();
            let menu_path_clone = menu_path.clone();
            let (anchor_x, anchor_y) = match bounds {
                Some(r) => {
                    if screen_size.0 > screen_size.1 {
                        (r.x as i32, (r.y + r.height) as i32)
                    } else {
                        ((r.x + r.width) as i32, r.y as i32)
                    }
                }
                None => {
                    let p = self
                        .surfaces
                        .last_cursor()
                        .map(|(_, p)| p)
                        .unwrap_or(iced::Point::ORIGIN);
                    (p.x as i32, p.y as i32)
                }
            };
            let bar_name = source_window
                .and_then(|w| self.surfaces.surface(w))
                .or_else(|| {
                    self.surfaces
                        .last_cursor()
                        .and_then(|(w, _)| self.surfaces.surface(w))
                })
                .map(|s| s.bar_name.clone())
                .unwrap_or_default();
            return Task::perform(
                async move {
                    crate::dbus::sni::host::fetch_menu(&conn, &service_clone, &menu_path_clone)
                        .await
                },
                move |tree_opt| {
                    if let Some(mut tree) = tree_opt {
                        tree.anchor = (anchor_x, anchor_y);
                        tree.screen_size = screen_size;
                        Message::Menu(messages::MenuEvent::Open {
                            service: service.clone(),
                            menu_path: menu_path.clone(),
                            tree,
                            bar_name: bar_name.clone(),
                        })
                    } else {
                        Message::Tray(messages::TrayRuntimeEvent::Activate {
                            service: service.clone(),
                        })
                    }
                },
            );
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Message;
    use crate::config::{Config, Edge, ModuleConfig, ModuleParams};
    use zbus::zvariant::OwnedObjectPath;

    fn make_tray_config() -> Config {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "tray".into(),
                module_type: "tray".into(),
                params: ModuleParams::Tray(crate::config::TrayConfig),
            }],
            ..Config::default()
        };
        config.bars[0].modules.bottom = vec!["tray".into()];
        config.bars[0].edge = Edge::Bottom;
        config
    }

    fn tray_item(service: &str) -> TrayItem {
        TrayItem {
            service: service.into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
        }
    }

    #[test]
    fn tray_not_in_needs_dbus_when_no_tray_module() {
        let (app, _) = App::boot(Config::default(), vec![]);
        assert!(
            !app.modules
                .iter()
                .any(|(_, m)| m.kind() == ModuleKind::Tray)
        );
    }

    #[test]
    fn tray_updated_message_stores_items_in_shared_state() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);

        let _ = app.update(Message::Tray(messages::TrayRuntimeEvent::Host(
            TrayEvent::Snapshot(vec![tray_item("org.test")]),
        )));

        assert!(app.tray.items.contains_key("org.test"));
    }

    #[test]
    fn tray_keyboard_menu_request_without_source_window_is_noop() {
        let (app, _) = App::boot(make_tray_config(), vec![]);
        let task = app.handle_tray_menu_request("org.test.App".into(), None, None);
        let _ = task;
    }

    #[test]
    fn tray_state_removes_path_qualified_registration_key() {
        let mut tray = TrayState::default();
        let item = TrayItem {
            service: ":1.42".into(),
            object_path: OwnedObjectPath::try_from("/CustomStatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
        };

        let _ = tray.apply_host_event(&TrayEvent::Snapshot(vec![item.clone()]));
        assert!(tray.item(":1.42/CustomStatusNotifierItem").is_some());

        let active = tray.apply_host_event(&TrayEvent::ItemRemoved(
            ":1.42/CustomStatusNotifierItem".into(),
        ));

        assert!(active.is_empty());
        assert!(tray.item(":1.42/CustomStatusNotifierItem").is_none());
    }

    #[test]
    fn tray_state_lookup_preserves_non_default_object_path() {
        let mut tray = TrayState::default();
        let item = TrayItem {
            service: ":1.24".into(),
            object_path: OwnedObjectPath::try_from("/AltStatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
        };

        let _ = tray.apply_host_event(&TrayEvent::ItemAdded(item));

        assert_eq!(
            tray.item(":1.24/AltStatusNotifierItem")
                .map(|item| item.object_path.as_str()),
            Some("/AltStatusNotifierItem")
        );
    }

    #[test]
    fn app_tray_lookup_preserves_dbus_service_for_runtime_id() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let item = TrayItem {
            service: ":1.24".into(),
            object_path: OwnedObjectPath::try_from("/AltStatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: Some(OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap()),
        };

        let _ = app.handle_tray_event(TrayEvent::ItemAdded(item));

        let stored = app
            .tray_item(":1.24/AltStatusNotifierItem")
            .expect("tray item should be stored by runtime id");
        assert_eq!(stored.service, ":1.24");
        assert_eq!(stored.object_path.as_str(), "/AltStatusNotifierItem");
    }

    #[test]
    fn tray_menu_target_uses_stored_dbus_service_for_runtime_id() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let item = TrayItem {
            service: ":1.24".into(),
            object_path: OwnedObjectPath::try_from("/AltStatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: Some(OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap()),
        };

        let _ = app.handle_tray_event(TrayEvent::ItemAdded(item));

        let (service, menu_path) = app
            .tray_menu_target(
                ":1.24/AltStatusNotifierItem",
                Some(OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap()),
            )
            .expect("tray menu target should resolve");
        assert_eq!(service, ":1.24");
        assert_eq!(menu_path.as_str(), "/com/canonical/dbusmenu");
    }

    #[test]
    fn tray_state_keeps_multiple_items_for_same_unique_name() {
        let mut tray = TrayState::default();
        let item_a = TrayItem {
            service: ":1.55".into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItemA").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app-a".into()),
            status: ItemStatus::Active,
            menu_path: None,
        };
        let item_b = TrayItem {
            service: ":1.55".into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItemB").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app-b".into()),
            status: ItemStatus::Active,
            menu_path: None,
        };

        let active = tray.apply_host_event(&TrayEvent::Snapshot(vec![item_a, item_b]));

        assert_eq!(active.len(), 2);
        assert!(tray.item(":1.55/StatusNotifierItemA").is_some());
        assert!(tray.item(":1.55/StatusNotifierItemB").is_some());
    }
}
