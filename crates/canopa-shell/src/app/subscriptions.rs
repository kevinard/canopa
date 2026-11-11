use std::time::Duration;

use super::{App, Message, messages};
use crate::ipc::ipc_subscription;
use crate::modules::{ModuleCapabilities, ModuleKind};
use crate::niri::subscription::niri_subscription;
use crate::surface::input::surface_input_subscription;

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
        self.modules.has_kind(kind)
    }

    fn has_capability(&self, predicate: impl Fn(ModuleCapabilities) -> bool) -> bool {
        self.modules.has_capability(predicate)
    }

    fn has_launcher_menu(&self) -> bool {
        self.config.menus.iter().any(|menu| {
            matches!(
                menu.source.as_ref().map(|source| &source.kind),
                Some(crate::config::MenuSourceKind::Launcher(_))
            )
        })
    }

    fn needs_niri_subscription(&self) -> bool {
        self.has_capability(|capabilities| capabilities.needs_niri) || self.has_launcher_menu()
    }

    fn needs_ipc_subscription(&self) -> bool {
        true
    }

    pub(crate) fn subscription(&self) -> iced::Subscription<Message> {
        let has_tray = self.has_module_kind(ModuleKind::Tray);
        let needs_system_dbus = self.has_capability(|capabilities| capabilities.needs_system_dbus);
        let needs_session_dbus =
            self.has_capability(|capabilities| capabilities.needs_session_dbus);
        let needs_wireplumber = self.has_capability(|capabilities| capabilities.needs_wireplumber);

        let module_subs = self.modules.module_subscriptions();
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

        let module_data_source_subs = self.data_sources.subscriptions(&self.shared);

        let sni_sub = dbus_subscription(
            has_tray,
            self.shared.dbus.session.clone(),
            crate::dbus::sni::sni_subscription,
        );

        let menu_watch_sub = if let Some(conn) = self.shared.dbus.session.clone() {
            if let Some(meta) = self.foreground.active_menu_meta() {
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

        // Window lifetime, geometry, pointer and keyboard input are essential
        // surface events, independent of optional module data subscriptions.
        let input_sub = surface_input_subscription();
        let ipc_sub = if self.needs_ipc_subscription() {
            ipc_subscription()
        } else {
            iced::Subscription::none()
        };

        let needs_tick = self.surfaces.values().any(|s| s.tooltip_pending.is_some());
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
            if self.has_launcher_menu() {
                self.launcher_clipboard
                    .saved_subscription()
                    .map(Message::LauncherClipboardLoaded)
            } else {
                iced::Subscription::none()
            },
            if self.has_launcher_menu() {
                self.launcher_clipboard
                    .subscription()
                    .map(Message::LauncherClipboardObserved)
            } else {
                iced::Subscription::none()
            },
            self.launcher_preferences
                .subscription(
                    self.config
                        .menus
                        .iter()
                        .filter_map(|menu| match menu.source.as_ref().map(|s| &s.kind) {
                            Some(crate::config::MenuSourceKind::Launcher(config)) => {
                                Some(config.clone())
                            }
                            _ => None,
                        })
                        .collect(),
                )
                .map(Message::LauncherPreferences),
            self.launcher_history
                .subscription()
                .map(Message::LauncherHistory),
            self.foreground.launcher().map_or_else(
                iced::Subscription::none,
                super::launcher::Session::subscription,
            ),
            self.configuration.subscription(),
            // Always installed: it owns the Shell's notification IDs for the
            // process lifetime and is idle until a notice is queued.
            self.notices.subscription().map(Message::Notice),
            crate::wallpaper::subscription(),
            super::osd::OsdRuntime::subscription(self.config.osd.enabled),
            self.canopa_accessibility.as_ref().map_or_else(
                iced::Subscription::none,
                super::canopa_accessibility::PanelAccessibility::subscription,
            ),
            module_subs,
            niri_sub,
            dbus_sub,
            module_data_source_subs,
            sni_sub,
            menu_watch_sub,
            input_sub,
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
        config.bars[0].modules.start = bar_module_names.into_iter().map(str::to_string).collect();
        config.bars[0].modules.center.clear();
        config.bars[0].modules.end.clear();
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
    fn output_reconciliation_does_not_need_niri_subscription() {
        let (app, _) = App::boot(Config::default(), vec!["eDP-1".into()]);

        assert!(!app.needs_niri_subscription());
    }

    #[test]
    fn shipped_defaults_run_the_tray_watcher_and_status_sources() {
        // The StatusNotifierWatcher only runs while a tray module is placed, so
        // the shipped bar must place one or tray-only applications have no icon.
        let (config, _) = crate::config::load(&canopa_config::Request::Defaults, false).unwrap();
        let (app, _) = App::boot(config, vec![]);
        assert!(app.has_module_kind(ModuleKind::Tray));
        for kind in [ModuleKind::Network, ModuleKind::Volume, ModuleKind::Battery] {
            assert!(
                app.has_module_kind(kind),
                "{kind:?} is not on the default bar"
            );
        }
        assert!(app.has_capability(|capabilities| capabilities.needs_session_dbus));
        assert!(app.has_capability(|capabilities| capabilities.needs_system_dbus));
    }

    #[test]
    fn app_needs_ipc_subscription_for_panel_commands() {
        let (app, _) = App::boot(Config::default(), vec![]);

        assert!(app.needs_ipc_subscription());
    }
    #[tokio::test]
    async fn passive_bars_observe_surface_events_through_the_installed_subscription() {
        use iced::advanced::subscription::{self, Recipe};
        use iced::futures::{StreamExt, stream};
        use std::hash::Hasher;
        fn identity(recipe: &dyn Recipe<Output = Message>) -> u64 {
            let mut hasher = subscription::Hasher::default();
            recipe.hash(&mut hasher);
            hasher.finish()
        }
        for (definitions, placement) in [
            (vec![], vec![]),
            (
                vec![module("clock", ModuleParams::Clock(Default::default()))],
                vec!["clock"],
            ),
            (
                vec![module("network", ModuleParams::Network(Default::default()))],
                vec!["network"],
            ),
        ] {
            let mut app = app_with_modules(definitions, placement);
            let window = iced::window::Id::unique();
            app.surfaces.surfaces.insert(
                window,
                crate::surface::SurfaceState::new("DP-1".into(), "main".into()),
            );
            let expected = identity(
                &*subscription::into_recipes(surface_input_subscription())
                    .pop()
                    .unwrap(),
            );
            let recipe = subscription::into_recipes(app.subscription())
                .into_iter()
                .find(|recipe| identity(&**recipe) == expected)
                .expect("passive bars still need window, pointer and focus events");
            let input = vec![
                iced::Event::Window(iced::window::Event::Opened {
                    position: None,
                    size: iced::Size::new(800.0, 28.0),
                }),
                iced::Event::Window(iced::window::Event::Resized(iced::Size::new(960.0, 36.0))),
                iced::Event::Mouse(iced::mouse::Event::CursorMoved {
                    position: iced::Point::new(73.0, 12.0),
                }),
                iced::Event::Window(iced::window::Event::Closed),
            ];
            let mut events = recipe.stream(Box::pin(stream::iter(input.into_iter().map(
                move |event| subscription::Event::Interaction {
                    window,
                    event,
                    status: iced::event::Status::Ignored,
                },
            ))));
            for _ in 0..3 {
                let _ = app.update(events.next().await.unwrap());
            }
            let surface = app.surface(window).unwrap();
            assert_eq!(surface.screen_size, (960, 36));
            assert_eq!(surface.last_pointer_position, (73, 12));
            let _ = app.update(events.next().await.unwrap());
            assert!(
                app.surface(window).is_none(),
                "closed passive surface was retained"
            );
        }
    }

    #[test]
    fn navigation_uses_configured_actions_and_inspection_and_skips_inert_content() {
        use crate::config::{ActionBinding, ActionTrigger, PanelAction, TooltipSetting};
        use crate::modules::{ModuleId, ModuleState, Zone};
        let mut definitions: Vec<ModuleConfig> = ["inert", "inspect", "click", "key"]
            .into_iter()
            .map(|name| ModuleConfig {
                name: name.into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(crate::config::ClockConfig {
                    tooltip: if name == "inspect" {
                        TooltipSetting::Default
                    } else {
                        TooltipSetting::Disabled
                    },
                    ..Default::default()
                }),
                actions: crate::config::ModuleActionsConfig {
                    bindings: match name {
                        "click" => vec![ActionBinding {
                            trigger: ActionTrigger::Click,
                            action: PanelAction::TogglePanel,
                        }],
                        "key" => vec![ActionBinding {
                            trigger: ActionTrigger::Key("Ctrl+E".into()),
                            action: PanelAction::TogglePanel,
                        }],
                        _ => vec![],
                    },
                },
            })
            .collect();
        definitions.push(ModuleConfig {
            name: "inert-button".into(),
            module_type: "button".into(),
            params: ModuleParams::Button(crate::config::ButtonConfig {
                label: "Inert button".into(),
                icon: String::new(),
                format: Default::default(),
                content_rotation: Default::default(),
                tooltip: TooltipSetting::Disabled,
            }),
            actions: Default::default(),
        });
        let mut app = app_with_modules(
            definitions,
            vec!["inert-button", "inert", "inspect", "click", "key"],
        );
        app.modules.set_state(
            ModuleId::new("inert-button"),
            ModuleState::Button(crate::modules::button::ButtonState),
        );
        let window = iced::window::Id::unique();
        app.surfaces.surfaces.insert(
            window,
            crate::surface::SurfaceState::new("DP-1".into(), "main".into()),
        );
        for name in ["inert", "inspect", "click", "key"] {
            app.modules.set_state(
                ModuleId::new(name),
                ModuleState::Clock(crate::modules::clock::ClockState {
                    time: "12:00".into(),
                    date: "October 1".into(),
                }),
            );
        }
        assert_eq!(
            app.effective_nav_list(window),
            ["inspect", "click", "key"]
                .into_iter()
                .map(|name| (Zone::Start, ModuleId::new(name)))
                .collect::<Vec<_>>()
        );
        let _ = app.handle_bar_focus_gained(window);
        assert_eq!(
            app.surface(window).unwrap().focused_module,
            Some(ModuleId::new("inspect"))
        );
    }
}
