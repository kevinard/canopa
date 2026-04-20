use iced::Task;

use super::{App, Message};
use crate::modules::ModuleKind;

impl App {
    fn find_tray_slots(&self) -> Vec<(usize, crate::modules::Zone, String)> {
        self.modules
            .iter()
            .enumerate()
            .filter(|(_, (_, m))| m.kind() == ModuleKind::Tray)
            .map(|(i, (z, m))| (i, z.clone(), m.id().0.clone()))
            .collect()
    }

    pub(super) fn handle_tray_event(
        &mut self,
        event: crate::dbus::sni::host::TrayEvent,
    ) -> Task<Message> {
        use crate::dbus::sni::host::{ItemStatus, TrayEvent};
        use crate::modules::tray::TrayIconModule;

        if let Some(perf) = &self.perf {
            perf.record_tray_event();
        }

        match event {
            TrayEvent::Snapshot(items) => {
                self.modules
                    .retain(|(_, m)| m.kind() != ModuleKind::TrayIcon);
                self.clear_surface_module_refs_with_prefix("tray:");

                let tray_slots = self.find_tray_slots();

                let active_items: Vec<&crate::dbus::sni::host::TrayItem> = items
                    .iter()
                    .filter(|i| i.status != ItemStatus::Passive)
                    .collect();

                for (slot_idx, zone, config_name) in tray_slots.iter().rev() {
                    let insert_at = slot_idx + 1;
                    for item in active_items.iter().rev() {
                        let icon_module = TrayIconModule::new((*item).clone(), config_name.clone());
                        self.modules
                            .insert(insert_at, (zone.clone(), Box::new(icon_module)));
                    }
                }
            }
            TrayEvent::ItemAdded(item) => {
                let service_id = format!("tray:{}", item.service);
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
}
