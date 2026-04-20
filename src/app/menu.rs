use iced::Task;
use iced_layershell::reexport::KeyboardInteractivity;

use super::{App, Message};

impl App {
    pub(super) fn close_all_menu_popups(&mut self) -> Vec<Task<Message>> {
        let ids: Vec<iced::window::Id> = self.menu_popups.keys().copied().collect();
        ids.into_iter()
            .map(|id| {
                self.menu_popups.remove(&id);
                self.menu_popup_hovered.remove(&id);
                self.menu_focused_item.remove(&id);
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

    pub(super) fn handle_menu_closed(&mut self) -> Task<Message> {
        let conn = self.shared.dbus.session.clone();
        for (service, menu_path, _, _) in self.menu_popups.values() {
            if let Some(conn) = conn.clone() {
                let service = service.clone();
                let menu_path = menu_path.clone();
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
