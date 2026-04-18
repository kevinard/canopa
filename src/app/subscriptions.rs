use std::time::Duration;

use super::{App, Message};
use crate::ipc::ipc_subscription;
use crate::niri::subscription::niri_subscription;
use crate::surface::input::keyboard_focus_subscription;

impl App {
    pub fn subscription(&self) -> iced::Subscription<Message> {
        let module_subs =
            iced::Subscription::batch(self.modules.iter().map(|(_, m)| m.subscription()));
        let niri_sub = niri_subscription();

        let needs_dbus = self
            .modules
            .iter()
            .any(|(_, m)| matches!(m.module_type(), "battery" | "network" | "volume" | "tray"));
        let dbus_sub = if needs_dbus {
            crate::dbus::dbus_init_subscription()
        } else {
            iced::Subscription::none()
        };

        let battery_sub = if let Some(conn) = self.shared.dbus.system.clone() {
            if self
                .modules
                .iter()
                .any(|(_, m)| m.module_type() == "battery")
            {
                crate::dbus::upower::battery_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        let network_sub = if let Some(conn) = self.shared.dbus.system.clone() {
            if self
                .modules
                .iter()
                .any(|(_, m)| m.module_type() == "network")
            {
                crate::dbus::networkmanager::network_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        let volume_sub = if self
            .modules
            .iter()
            .any(|(_, m)| m.module_type() == "volume")
        {
            crate::dbus::wireplumber::volume_subscription()
        } else {
            iced::Subscription::none()
        };

        let sni_sub = if let Some(conn) = self.shared.dbus.session.clone() {
            if self.modules.iter().any(|(_, m)| m.module_type() == "tray") {
                crate::dbus::sni_host::sni_subscription(conn)
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        let menu_watch_sub = if let Some(conn) = self.shared.dbus.session.clone() {
            if let Some((service, menu_path, _, _)) = self.menu_popups.values().next() {
                crate::dbus::sni_host::menu_layout_subscription(
                    conn,
                    service.clone(),
                    menu_path.clone(),
                )
            } else {
                iced::Subscription::none()
            }
        } else {
            iced::Subscription::none()
        };

        let focus_sub = keyboard_focus_subscription();
        let ipc_sub = ipc_subscription();

        let needs_tick = self
            .surfaces
            .values()
            .any(|s| s.expand.is_animating() || s.hover_pending_ms.is_some());
        let tick_sub = if needs_tick {
            iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick)
        } else {
            iced::Subscription::none()
        };

        iced::Subscription::batch([
            module_subs,
            niri_sub,
            dbus_sub,
            battery_sub,
            network_sub,
            volume_sub,
            sni_sub,
            menu_watch_sub,
            focus_sub,
            ipc_sub,
            tick_sub,
        ])
    }
}
