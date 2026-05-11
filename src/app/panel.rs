use iced::Task;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings,
};

use super::{App, Message};

#[derive(Debug, Default)]
pub(super) struct PanelSystem {
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) menu_path: Vec<usize>,
}

impl PanelSystem {
    pub(super) fn clear(&mut self) {
        self.search_query.clear();
        self.focused_index = None;
        self.menu_path.clear();
    }
}

fn panel_window_settings(
    bar: &crate::config::BarConfig,
    _tree: &crate::surface::panel::PanelTree,
) -> NewLayerShellSettings {
    let width = bar.panel.width;
    let height = bar.panel.height.or(bar.panel.max_height).unwrap_or(600);
    let anchor = match bar.edge {
        crate::config::Edge::Top => Anchor::Top | Anchor::Left,
        crate::config::Edge::Bottom => Anchor::Bottom | Anchor::Left,
        crate::config::Edge::Left => Anchor::Left | Anchor::Top,
        crate::config::Edge::Right => Anchor::Right | Anchor::Top,
    };

    NewLayerShellSettings {
        anchor,
        exclusive_zone: Some(-1),
        size: Some((width, height)),
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        namespace: Some(App::namespace()),
        ..Default::default()
    }
}

impl App {
    pub(super) fn dispatch_panel_action(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        action: crate::config::PanelAction,
    ) -> Task<Message> {
        match action {
            crate::config::PanelAction::Command(command) => {
                let _ = crate::command::spawn_config_command_fire_and_forget(&command);
                Task::none()
            }
            crate::config::PanelAction::OpenPanel => {
                let content = content_for_origin(&origin);
                self.handle_panel_event(super::messages::PanelEvent::Open {
                    origin_window,
                    origin,
                    content,
                    anchor: None,
                })
            }
            crate::config::PanelAction::TogglePanel => {
                let content = content_for_origin(&origin);
                self.handle_panel_event(super::messages::PanelEvent::Toggle {
                    origin_window,
                    origin,
                    content,
                    anchor: None,
                })
            }
            crate::config::PanelAction::ClosePanel => {
                self.handle_panel_event(super::messages::PanelEvent::Close)
            }
            crate::config::PanelAction::OpenMenu(name) => {
                self.handle_panel_event(super::messages::PanelEvent::Open {
                    origin_window,
                    origin,
                    content: crate::surface::panel::PanelContent::Menu(name),
                    anchor: None,
                })
            }
            crate::config::PanelAction::ToggleMenu(name) => {
                self.handle_panel_event(super::messages::PanelEvent::Toggle {
                    origin_window,
                    origin,
                    content: crate::surface::panel::PanelContent::Menu(name),
                    anchor: None,
                })
            }
        }
    }

    pub(super) fn close_all_panels(&mut self) -> Vec<Task<Message>> {
        let ids: Vec<_> = self.surfaces.panel_popup_ids().collect();
        self.panel.clear();
        ids.into_iter()
            .map(|id| {
                self.surfaces.remove_panel_popup(id);
                Task::done(Message::RemoveWindow(id))
            })
            .collect()
    }

    pub(super) fn handle_panel_event(
        &mut self,
        event: super::messages::PanelEvent,
    ) -> Task<Message> {
        use super::messages::PanelEvent;

        match event {
            PanelEvent::Open {
                origin_window,
                origin,
                content,
                anchor,
            } => self.open_panel(origin_window, origin, content, anchor),
            PanelEvent::Toggle {
                origin_window,
                origin,
                content,
                anchor,
            } => {
                let same_panel_open = self.surfaces.panel_popup_values().any(|meta| {
                    meta.tree.origin_window == origin_window
                        && meta.tree.origin == origin
                        && meta.tree.content == content
                });
                if same_panel_open {
                    Task::batch(self.close_all_panels())
                } else {
                    self.open_panel(origin_window, origin, content, anchor)
                }
            }
            PanelEvent::Close => Task::batch(self.close_all_panels()),
            PanelEvent::SearchChanged(query) => {
                self.panel.search_query = query;
                Task::none()
            }
            PanelEvent::FocusMoveNext
            | PanelEvent::FocusMovePrev
            | PanelEvent::Activate
            | PanelEvent::Back => Task::none(),
        }
    }

    fn open_panel(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
    ) -> Task<Message> {
        let origin_surface = self.surface(origin_window).cloned().unwrap_or_default();
        let tree = crate::surface::panel::PanelTree {
            origin,
            content,
            origin_window,
            origin_bar_name: origin_surface.bar_name.clone(),
            origin_output_name: origin_surface.output_name.clone(),
            anchor,
            screen_size: origin_surface.screen_size,
        };
        let bar = Self::bar_for_surface_name(&self.config.bars, &origin_surface.bar_name);
        let settings = panel_window_settings(bar, &tree);
        let (panel_window, open_task) = Message::layershell_open(settings);

        let mut tasks = self.close_tooltip_popup_if_open(origin_window);
        tasks.extend(self.close_all_menu_popups());
        tasks.extend(self.close_all_panels());
        self.surfaces.insert_panel_popup(
            panel_window,
            super::surfaces::PanelMeta { tree },
        );
        tasks.push(open_task);
        Task::batch(tasks)
    }
}

fn content_for_origin(
    origin: &crate::surface::panel::PanelOrigin,
) -> crate::surface::panel::PanelContent {
    match origin {
        crate::surface::panel::PanelOrigin::Bar => crate::surface::panel::PanelContent::BarPanel,
        crate::surface::panel::PanelOrigin::Module(id) => {
            crate::surface::panel::PanelContent::ModulePanel(id.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_panel_closes_existing_panel() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));
        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            anchor: None,
        }));
        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);
    }

    #[test]
    fn closing_panel_removes_panel_metadata() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Close));

        assert_eq!(app.surfaces.panel_popup_ids().count(), 0);
    }

    #[test]
    fn configured_module_click_open_panel_takes_precedence_over_builtin_click() {
        let legacy_path = format!(
            "/tmp/niribar-panel-legacy-click-{}",
            std::process::id()
        );
        let _ = std::fs::remove_file(&legacy_path);

        let mut config = crate::config::Config {
            modules: vec![crate::config::ModuleConfig {
                name: "button".into(),
                module_type: "button".into(),
                params: crate::config::ModuleParams::Button(crate::config::ButtonConfig {
                    label: "Launch".into(),
                    icon: ">".into(),
                    on_click: crate::config::CommandConfig::Shell(format!(
                        "printf legacy >{legacy_path}"
                    )),
                    format: crate::config::FormatConfig::default(),
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: crate::config::ModuleActionsConfig {
                    bindings: vec![crate::config::ActionBinding {
                        trigger: crate::config::ActionTrigger::Click,
                        action: crate::config::PanelAction::OpenPanel,
                    }],
                },
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["button".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Modules(
            crate::app::messages::ModuleRuntimeEvent::ModuleClick(
                crate::modules::ModuleId::new("button"),
                origin,
            ),
        ));

        let panel = app.surfaces.panel_popup_values().next().expect("panel");
        assert_eq!(
            panel.tree.content,
            crate::surface::panel::PanelContent::ModulePanel(crate::modules::ModuleId::new(
                "button"
            ))
        );
        assert!(
            !std::path::Path::new(&legacy_path).exists(),
            "legacy button command should not run when open-panel action is configured"
        );
    }
}
