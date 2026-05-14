use iced::Task;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings};

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
    tree: &crate::surface::panel::PanelTree,
) -> NewLayerShellSettings {
    let width = bar.panel.width;
    let height = bar.panel.height.or(bar.panel.max_height).unwrap_or(600);
    let anchor = match bar.edge {
        crate::config::Edge::Top => Anchor::Top | Anchor::Left,
        crate::config::Edge::Bottom => Anchor::Bottom | Anchor::Left,
        crate::config::Edge::Left => Anchor::Left | Anchor::Top,
        crate::config::Edge::Right => Anchor::Right | Anchor::Top,
    };
    let position = panel_position(
        &bar.edge,
        bar.collapsed_width,
        (width, height),
        tree.anchor,
        tree.screen_size,
    );
    let margin = panel_margin(&bar.edge, position, (width, height), tree.screen_size);

    NewLayerShellSettings {
        anchor,
        exclusive_zone: Some(-1),
        size: Some((width, height)),
        margin: Some(margin),
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        namespace: Some(App::namespace()),
        ..Default::default()
    }
}

fn panel_position(
    edge: &crate::config::Edge,
    bar_size: u32,
    panel_size: (u32, u32),
    anchor: Option<iced::Rectangle>,
    screen_size: (u32, u32),
) -> (i32, i32) {
    let bar_size = bar_size as i32;
    let (panel_w, panel_h) = (panel_size.0 as i32, panel_size.1 as i32);
    let (screen_w, screen_h) = (screen_size.0 as i32, screen_size.1 as i32);
    let anchor = anchor.unwrap_or(iced::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    });
    let anchor_center_x = (anchor.x + anchor.width / 2.0).round() as i32;
    let anchor_center_y = (anchor.y + anchor.height / 2.0).round() as i32;

    match edge {
        crate::config::Edge::Left => (
            bar_size,
            (anchor_center_y - panel_h / 2).clamp(0, (screen_h - panel_h).max(0)),
        ),
        crate::config::Edge::Right => (
            (screen_w - bar_size - panel_w).max(0),
            (anchor_center_y - panel_h / 2).clamp(0, (screen_h - panel_h).max(0)),
        ),
        crate::config::Edge::Top => (
            (anchor_center_x - panel_w / 2).clamp(0, (screen_w - panel_w).max(0)),
            bar_size,
        ),
        crate::config::Edge::Bottom => (
            (anchor_center_x - panel_w / 2).clamp(0, (screen_w - panel_w).max(0)),
            (screen_h - bar_size - panel_h).max(0),
        ),
    }
}

fn panel_margin(
    edge: &crate::config::Edge,
    position: (i32, i32),
    panel_size: (u32, u32),
    screen_size: (u32, u32),
) -> (i32, i32, i32, i32) {
    let (x, y) = position;
    let (panel_w, panel_h) = (panel_size.0 as i32, panel_size.1 as i32);
    let (screen_w, screen_h) = (screen_size.0 as i32, screen_size.1 as i32);

    match edge {
        crate::config::Edge::Top => (y, 0, 0, x),
        crate::config::Edge::Bottom => (0, 0, (screen_h - y - panel_h).max(0), x),
        crate::config::Edge::Left => (y, 0, 0, x),
        crate::config::Edge::Right => (y, (screen_w - x - panel_w).max(0), 0, 0),
    }
}

#[cfg(test)]
fn panel_position_for_test(
    edge: crate::config::Edge,
    bar_size: u32,
    panel_size: (u32, u32),
    anchor: Option<iced::Rectangle>,
    screen_size: (u32, u32),
) -> (i32, i32) {
    panel_position(&edge, bar_size, panel_size, anchor, screen_size)
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
            action => match panel_event_for_action(origin_window, origin, action, None) {
                Some(event) => self.handle_panel_event(event),
                None => Task::none(),
            },
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
        self.surfaces
            .insert_panel_popup(panel_window, super::surfaces::PanelMeta { tree });
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

pub(super) fn panel_action_needs_anchor(action: &crate::config::PanelAction) -> bool {
    matches!(
        action,
        crate::config::PanelAction::OpenPanel
            | crate::config::PanelAction::TogglePanel
            | crate::config::PanelAction::OpenMenu(_)
            | crate::config::PanelAction::ToggleMenu(_)
    )
}

pub(super) fn panel_event_for_action(
    origin_window: iced::window::Id,
    origin: crate::surface::panel::PanelOrigin,
    action: crate::config::PanelAction,
    anchor: Option<iced::Rectangle>,
) -> Option<super::messages::PanelEvent> {
    match action {
        crate::config::PanelAction::OpenPanel => {
            let content = content_for_origin(&origin);
            Some(super::messages::PanelEvent::Open {
                origin_window,
                origin,
                content,
                anchor,
            })
        }
        crate::config::PanelAction::TogglePanel => {
            let content = content_for_origin(&origin);
            Some(super::messages::PanelEvent::Toggle {
                origin_window,
                origin,
                content,
                anchor,
            })
        }
        crate::config::PanelAction::ClosePanel => Some(super::messages::PanelEvent::Close),
        crate::config::PanelAction::OpenMenu(name) => Some(super::messages::PanelEvent::Open {
            origin_window,
            origin,
            content: crate::surface::panel::PanelContent::Menu(name),
            anchor,
        }),
        crate::config::PanelAction::ToggleMenu(name) => Some(super::messages::PanelEvent::Toggle {
            origin_window,
            origin,
            content: crate::surface::panel::PanelContent::Menu(name),
            anchor,
        }),
        crate::config::PanelAction::Command(_) => None,
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
    fn left_edge_panel_opens_right_of_bar_and_aligns_to_anchor_y() {
        let position = panel_position_for_test(
            crate::config::Edge::Left,
            48,
            (360, 600),
            Some(iced::Rectangle {
                x: 0.0,
                y: 100.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
        );

        assert_eq!(position.0, 48);
        assert!(position.1 >= 0);
    }

    #[test]
    fn right_edge_panel_opens_left_of_bar_and_aligns_to_anchor_y() {
        let position = panel_position_for_test(
            crate::config::Edge::Right,
            48,
            (360, 600),
            Some(iced::Rectangle {
                x: 1872.0,
                y: 100.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
        );

        assert_eq!(position.0, 1512);
        assert!(position.1 >= 0);
    }

    #[test]
    fn top_edge_panel_opens_below_bar_and_aligns_to_anchor_x() {
        let position = panel_position_for_test(
            crate::config::Edge::Top,
            32,
            (360, 600),
            Some(iced::Rectangle {
                x: 100.0,
                y: 0.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
        );

        assert!(position.0 >= 0);
        assert_eq!(position.1, 32);
    }

    #[test]
    fn bottom_edge_panel_opens_above_bar_and_aligns_to_anchor_x() {
        let position = panel_position_for_test(
            crate::config::Edge::Bottom,
            32,
            (360, 600),
            Some(iced::Rectangle {
                x: 100.0,
                y: 1048.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
        );

        assert!(position.0 >= 0);
        assert_eq!(position.1, 448);
    }

    #[test]
    fn configured_module_click_open_panel_takes_precedence_over_builtin_click() {
        let legacy_path = format!("/tmp/niribar-panel-legacy-click-{}", std::process::id());
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

        assert!(
            !std::path::Path::new(&legacy_path).exists(),
            "legacy button command should not run when open-panel action is configured"
        );
    }

    #[test]
    fn panel_event_for_module_open_panel_carries_anchor() {
        let anchor = iced::Rectangle {
            x: 8.0,
            y: 16.0,
            width: 24.0,
            height: 32.0,
        };
        let event = panel_event_for_action(
            iced::window::Id::unique(),
            crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new("button")),
            crate::config::PanelAction::OpenPanel,
            Some(anchor),
        )
        .expect("panel event");

        match event {
            crate::app::messages::PanelEvent::Open {
                origin,
                content,
                anchor: event_anchor,
                ..
            } => {
                assert_eq!(
                    origin,
                    crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new(
                        "button"
                    ))
                );
                assert_eq!(
                    content,
                    crate::surface::panel::PanelContent::ModulePanel(
                        crate::modules::ModuleId::new("button")
                    )
                );
                assert_eq!(event_anchor, Some(anchor));
            }
            other => panic!("expected open event, got {other:?}"),
        }
    }
}
