use std::time::Duration;

use super::{App, Message, messages};
use crate::ipc::ipc_subscription;
use crate::modules::{ModuleCapabilities, ModuleKind, descriptor_for_kind};
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
    fn has_module_kind(&self, kind: ModuleKind) -> bool {
        !self.modules.ids_by_kind(kind).is_empty()
    }

    fn has_capability(&self, predicate: impl Fn(ModuleCapabilities) -> bool) -> bool {
        self.modules
            .iter()
            .filter_map(|(_, module)| descriptor_for_kind(module.kind()))
            .any(|descriptor| predicate(descriptor.capabilities))
    }

    fn needs_niri_subscription(&self) -> bool {
        self.has_capability(|capabilities| capabilities.needs_niri)
            || self.surfaces.manual_outputs()
    }

    fn needs_keyboard_focus_subscription(&self) -> bool {
        self.config
            .bars
            .iter()
            .any(|bar| bar.expand_keybind.is_some())
            || self.has_capability(|capabilities| capabilities.needs_keyboard_focus)
    }

    fn needs_ipc_subscription(&self) -> bool {
        self.config
            .bars
            .iter()
            .any(|bar| bar.expand && bar.expanded_width > bar.collapsed_width)
    }

    pub(crate) fn subscription(&self) -> iced::Subscription<Message> {
        let has_battery = self.has_module_kind(ModuleKind::Battery);
        let has_network = self.has_module_kind(ModuleKind::Network);
        let has_volume = self.has_module_kind(ModuleKind::Volume);
        let has_tray = self.has_module_kind(ModuleKind::Tray);
        let needs_system_dbus = self.has_capability(|capabilities| capabilities.needs_system_dbus);
        let needs_session_dbus =
            self.has_capability(|capabilities| capabilities.needs_session_dbus);
        let needs_wireplumber = self.has_capability(|capabilities| capabilities.needs_wireplumber);

        let per_module: Vec<iced::Subscription<Message>> =
            self.modules.iter().map(|(_, m)| m.subscription()).collect();

        let module_subs = iced::Subscription::batch(per_module);
        let niri_sub = if self.needs_niri_subscription() {
            niri_subscription()
        } else {
            iced::Subscription::none()
        };

        let needs_dbus = needs_system_dbus || needs_session_dbus || needs_wireplumber;
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

        let focus_sub = if self.needs_keyboard_focus_subscription() {
            keyboard_focus_subscription()
        } else {
            iced::Subscription::none()
        };
        let ipc_sub = if self.needs_ipc_subscription() {
            ipc_subscription()
        } else {
            iced::Subscription::none()
        };

        let needs_tick = self.surfaces.values().any(|s| {
            s.expand.is_animating() || s.hover_pending_ms.is_some() || s.tooltip_pending.is_some()
        });
        let tick_sub = if needs_tick {
            iced::time::every(Duration::from_millis(16))
                .map(|_| Message::Surface(messages::SurfaceEvent::Tick))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ModuleConfig, ModuleParams};

    fn app_with_modules(modules: Vec<ModuleConfig>, bar_module_names: Vec<&str>) -> App {
        let mut config = Config {
            modules,
            ..Config::default()
        };
        config.bars[0].modules.top = bar_module_names.into_iter().map(str::to_string).collect();
        config.bars[0].modules.center.clear();
        config.bars[0].modules.bottom.clear();
        App::boot(config, vec![]).0
    }

    fn module(name: &str, params: ModuleParams) -> ModuleConfig {
        ModuleConfig {
            name: name.into(),
            module_type: name.into(),
            params,
            actions: crate::config::ModuleActionsConfig::default(),
        }
    }

    #[test]
    fn clock_only_config_does_not_need_niri_subscription() {
        let app = app_with_modules(
            vec![module(
                "clock",
                ModuleParams::Clock(crate::config::ClockConfig::default()),
            )],
            vec!["clock"],
        );

        assert!(!app.needs_niri_subscription());
    }

    #[test]
    fn workspaces_config_needs_niri_subscription() {
        let app = app_with_modules(
            vec![module(
                "workspaces",
                ModuleParams::Workspaces(crate::config::WorkspacesConfig::default()),
            )],
            vec!["workspaces"],
        );

        assert!(app.needs_niri_subscription());
    }

    #[test]
    fn focused_window_config_needs_niri_subscription() {
        let app = app_with_modules(
            vec![module("focused-window", ModuleParams::None)],
            vec!["focused-window"],
        );

        assert!(app.needs_niri_subscription());
    }

    #[test]
    fn manual_output_mode_needs_niri_subscription_for_hotplug() {
        let (app, _) = App::boot(Config::default(), vec!["eDP-1".into()]);

        assert!(app.needs_niri_subscription());
    }

    #[test]
    fn clock_only_config_does_not_need_keyboard_focus_subscription() {
        let app = app_with_modules(
            vec![module(
                "clock",
                ModuleParams::Clock(crate::config::ClockConfig::default()),
            )],
            vec!["clock"],
        );

        assert!(!app.needs_keyboard_focus_subscription());
    }

    #[test]
    fn interactive_modules_need_keyboard_focus_subscription() {
        let app = app_with_modules(
            vec![module(
                "volume",
                ModuleParams::Volume(crate::config::VolumeConfig::default()),
            )],
            vec!["volume"],
        );

        assert!(app.needs_keyboard_focus_subscription());
    }

    #[test]
    fn expand_keybind_needs_keyboard_focus_subscription() {
        let mut config = Config::default();
        config.bars[0].expand_keybind = Some("Super+e".into());
        let (app, _) = App::boot(config, vec![]);

        assert!(app.needs_keyboard_focus_subscription());
    }

    #[test]
    fn disabled_expand_does_not_need_ipc_subscription() {
        let mut config = Config::default();
        config.bars[0].expand = false;
        let (app, _) = App::boot(config, vec![]);

        assert!(!app.needs_ipc_subscription());
    }

    #[test]
    fn expandable_bar_needs_ipc_subscription() {
        let (app, _) = App::boot(Config::default(), vec![]);

        assert!(app.needs_ipc_subscription());
    }
}
