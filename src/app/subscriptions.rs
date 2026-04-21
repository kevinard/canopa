use std::time::Duration;

use super::{App, Message};
use crate::ipc::ipc_subscription;
use crate::modules::ModuleKind;
use crate::niri::subscription::niri_subscription;
use crate::surface::input::keyboard_focus_subscription;

fn dbus_subscription<F>(
    enabled: bool,
    conn: Option<zbus::Connection>,
    build: F,
) -> iced::Subscription<Message>
where
    F: FnOnce(zbus::Connection) -> iced::Subscription<Message>,
{
    match (enabled, conn) {
        (true, Some(c)) => build(c),
        _ => iced::Subscription::none(),
    }
}

impl App {
    pub(crate) fn subscription(&self) -> iced::Subscription<Message> {
        let has_battery = !self.modules.ids_by_kind(ModuleKind::Battery).is_empty();
        let has_network = !self.modules.ids_by_kind(ModuleKind::Network).is_empty();
        let has_volume = !self.modules.ids_by_kind(ModuleKind::Volume).is_empty();
        let has_tray = !self.modules.ids_by_kind(ModuleKind::Tray).is_empty();

        let per_module: Vec<iced::Subscription<Message>> =
            self.modules.iter().map(|(_, m)| m.subscription()).collect();

        let module_subs = iced::Subscription::batch(per_module);
        let niri_sub = niri_subscription();

        let needs_dbus = has_battery || has_network || has_volume || has_tray;
        let dbus_sub = if needs_dbus {
            crate::dbus::dbus_init_subscription()
        } else {
            iced::Subscription::none()
        };

        let battery_sub = dbus_subscription(
            has_battery,
            self.shared.dbus.system.clone(),
            crate::dbus::upower::battery_subscription,
        );

        let network_sub = dbus_subscription(
            has_network,
            self.shared.dbus.system.clone(),
            crate::dbus::networkmanager::network_subscription,
        );

        let volume_sub = if has_volume {
            crate::dbus::wireplumber::volume_subscription()
        } else {
            iced::Subscription::none()
        };

        let sni_sub = dbus_subscription(
            has_tray,
            self.shared.dbus.session.clone(),
            crate::dbus::sni::sni_subscription,
        );

        let menu_watch_sub = if let Some(conn) = self.shared.dbus.session.clone() {
            if let Some(meta) = self.surfaces.popup_values().next() {
                crate::dbus::sni::menu_layout_subscription(
                    conn,
                    meta.service.clone(),
                    meta.menu_path.clone(),
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
            iced::time::every(Duration::from_millis(16))
                .map(|_| Message::Surface(super::messages::SurfaceEvent::Tick))
        } else {
            iced::Subscription::none()
        };

        let perf_sub = if self.perf.is_some() {
            iced::time::every(Duration::from_secs(5)).map(|_| Message::PerfReport)
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
            perf_sub,
        ])
    }
}
