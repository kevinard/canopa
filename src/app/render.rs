use iced::Element;
use iced::widget::{container, mouse_area};

use super::{App, Message};
use crate::config::Edge;
use crate::modules::focused_window;
use crate::modules::{ModuleId, ModuleKind, ModuleState, Zone};
use crate::surface::frame::module_frame;
use crate::surface::menu_popup::MenuTree;

pub(super) struct PopupRenderModel<'a> {
    pub(super) service: &'a str,
    pub(super) menu_path: &'a zbus::zvariant::OwnedObjectPath,
    pub(super) tree: &'a MenuTree,
}

#[allow(dead_code)]
pub(super) struct ModuleRenderEntry<'a> {
    pub(super) id: ModuleId,
    pub(super) zone: Zone,
    pub(super) hovered: bool,
    pub(super) focused: bool,
    pub(super) active: bool,
    pub(super) element: Element<'a, Message>,
    #[cfg(test)]
    pub(super) state: ModuleState,
}

pub(super) struct BarRenderModel<'a> {
    pub(super) edge: Edge,
    pub(super) content_size: u32,
    pub(super) is_horizontal: bool,
    pub(super) start: Vec<ModuleRenderEntry<'a>>,
    pub(super) center: Vec<ModuleRenderEntry<'a>>,
    pub(super) end: Vec<ModuleRenderEntry<'a>>,
}

pub(super) struct PanelRenderModel<'a> {
    pub(super) tree: &'a crate::surface::panel::PanelTree,
    pub(super) entries: Vec<ModuleRenderEntry<'a>>,
    pub(super) bar_style: &'a crate::config::Style,
}

impl App {
    pub(super) fn popup_render_model(
        &self,
        window: iced::window::Id,
    ) -> Option<PopupRenderModel<'_>> {
        let meta = self.surfaces.popup(window)?;

        Some(PopupRenderModel {
            service: meta.service.as_str(),
            menu_path: &meta.menu_path,
            tree: &meta.tree,
        })
    }

    pub(super) fn panel_render_model(
        &self,
        window: iced::window::Id,
    ) -> Option<PanelRenderModel<'_>> {
        let meta = self.surfaces.panel_popup(window)?;
        let tree = &meta.tree;
        let bar = Self::bar_for_surface_name(&self.config.bars, &tree.origin_bar_name);
        let origin_surface = self
            .surfaces
            .surface(tree.origin_window)
            .cloned()
            .unwrap_or_else(|| {
                crate::surface::SurfaceState::new(
                    tree.origin_output_name.clone(),
                    tree.origin_bar_name.clone(),
                )
            });
        let default_state = ModuleState::default();

        let layout = match &tree.content {
            crate::surface::panel::PanelContent::BarPanel => {
                self.effective_panel_module_layout_for_bar(bar)
            }
            crate::surface::panel::PanelContent::ModulePanel(id) => {
                vec![(Zone::Center, id.clone())]
            }
            crate::surface::panel::PanelContent::Menu(_) => Vec::new(),
        };

        let entries = layout
            .into_iter()
            .filter_map(|(zone, id)| {
                let module = self.modules.get(&id)?;
                let state =
                    self.module_render_state(module.kind(), &id, &origin_surface, &default_state);
                let focused = tree.origin == crate::surface::panel::PanelOrigin::Module(id.clone());
                let element = crate::surface::panel::panel_module_frame(
                    module.view_panel_with_style(&state, &bar.style),
                    focused,
                );

                Some(ModuleRenderEntry {
                    id,
                    zone,
                    hovered: false,
                    focused,
                    active: focused,
                    element,
                    #[cfg(test)]
                    state,
                })
            })
            .collect();

        Some(PanelRenderModel {
            tree,
            entries,
            bar_style: &bar.style,
        })
    }

    pub(super) fn bar_render_model(&self, window: iced::window::Id) -> BarRenderModel<'_> {
        let surface = self.surface(window).cloned().unwrap_or_default();
        let bar = Self::bar_for_surface_name(&self.config.bars, &surface.bar_name);
        let is_horizontal = bar.edge.is_horizontal();

        let surface_size = surface
            .expand
            .current_width(bar.collapsed_width, bar.expanded_width);
        let content_size = surface_size.saturating_sub(1);
        let shows_expanded = surface.expand.shows_expanded_content();

        let effective_zone = self.effective_module_zones_for_bar(bar);
        let default_state = ModuleState::default();
        let mut start = Vec::new();
        let mut center = Vec::new();
        let mut end = Vec::new();

        for (_boot_zone, module) in &self.modules {
            let id = module.id();
            let Some(zone) = effective_zone.get(&id) else {
                continue;
            };

            let state = self.module_render_state(module.kind(), &id, &surface, &default_state);
            let hovered = surface.hovered_module.as_ref() == Some(&id);
            let focused = surface.focused_module.as_ref() == Some(&id);
            let active = self.surfaces.active_panel_origin_for(window)
                == Some(&crate::surface::panel::PanelOrigin::Module(id.clone()));

            let module_view =
                module.view_for_bar(&state, window, is_horizontal, shows_expanded, &bar.style);
            let framed_module = container(module_frame(module_view, hovered || focused || active))
                .id(super::widget_ops::module_widget_id(window, &id));
            let scroll_id = id.clone();
            let element: Element<'_, Message> = mouse_area(framed_module)
                .on_enter(Message::Surface(
                    super::messages::SurfaceEvent::ModuleHoverEnter(id.clone(), window),
                ))
                .on_exit(Message::Surface(
                    super::messages::SurfaceEvent::ModuleHoverExit(id.clone(), window),
                ))
                .on_press(Message::Modules(
                    super::messages::ModuleRuntimeEvent::ModuleClick(id.clone(), window),
                ))
                .on_right_press(Message::Modules(
                    super::messages::ModuleRuntimeEvent::ModuleRightClick(id.clone(), window),
                ))
                .on_scroll(move |delta| {
                    let up = match delta {
                        iced::mouse::ScrollDelta::Lines { y, .. }
                        | iced::mouse::ScrollDelta::Pixels { y, .. } => y > 0.0,
                    };
                    if up {
                        Message::Modules(super::messages::ModuleRuntimeEvent::ModuleScrollUp(
                            scroll_id.clone(),
                            window,
                        ))
                    } else {
                        Message::Modules(super::messages::ModuleRuntimeEvent::ModuleScrollDown(
                            scroll_id.clone(),
                            window,
                        ))
                    }
                })
                .into();

            let entry = ModuleRenderEntry {
                id,
                zone: zone.clone(),
                hovered,
                focused,
                active,
                element,
                #[cfg(test)]
                state,
            };

            match zone {
                Zone::Start => start.push(entry),
                Zone::Center => center.push(entry),
                Zone::End => end.push(entry),
            }
        }

        BarRenderModel {
            edge: bar.edge.clone(),
            content_size,
            is_horizontal,
            start,
            center,
            end,
        }
    }

    fn module_render_state(
        &self,
        kind: ModuleKind,
        id: &ModuleId,
        surface: &crate::surface::SurfaceState,
        default_state: &ModuleState,
    ) -> ModuleState {
        if kind == ModuleKind::WorkspaceItem {
            return ModuleState::WorkspaceOutput(surface.output_name.clone());
        }

        if let Some(ModuleState::FocusedWindow(fw)) = self.modules.state(id) {
            let title = if surface.output_name.is_empty() {
                fw.title.clone()
            } else {
                match self.niri.state().focused_window_id {
                    None => self
                        .niri
                        .state()
                        .active_window_title_for_output(&surface.output_name),
                    Some(_) => {
                        let focused_output = self.niri.state().focused_output();
                        if focused_output.is_none()
                            || focused_output == Some(surface.output_name.as_str())
                        {
                            fw.title.clone()
                        } else {
                            self.niri
                                .state()
                                .active_window_title_for_output(&surface.output_name)
                        }
                    }
                }
            };
            return ModuleState::FocusedWindow(focused_window::FocusedWindowState { title });
        }

        self.modules.state(id).unwrap_or(default_state).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::config::{
        BarConfig, ClockConfig, Config, FocusedWindowConfig, ModuleConfig, ModuleParams,
        WorkspacesConfig,
    };
    use crate::modules::focused_window::FocusedWindowState;
    use crate::modules::{ModuleId, ModuleState, Zone};
    use crate::surface::SurfaceState;

    fn test_wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn make_window(id: u64, title: &str, is_focused: bool) -> niri_ipc::Window {
        niri_ipc::Window {
            id,
            title: Some(title.to_string()),
            app_id: None,
            pid: None,
            workspace_id: None,
            is_focused,
            is_floating: false,
            is_urgent: false,
            layout: niri_ipc::WindowLayout {
                pos_in_scrolling_layout: None,
                tile_size: (0.0, 0.0),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
            },
            focus_timestamp: None,
        }
    }

    fn make_workspace(
        id: u64,
        idx: u8,
        output: &str,
        is_active: bool,
        is_focused: bool,
        active_window_id: Option<u64>,
    ) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id,
            idx,
            name: None,
            output: Some(output.to_string()),
            is_urgent: false,
            is_active,
            is_focused,
            active_window_id,
        }
    }

    #[test]
    fn bar_render_model_uses_focused_title_for_focused_output() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "focused-window".into(),
                module_type: "focused-window".into(),
                params: ModuleParams::FocusedWindow(FocusedWindowConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.center = vec!["focused-window".into()];
        let (mut app, _) = App::boot(config, vec![]);
        app.niri.state_mut().focused_window_id = Some(1);
        app.niri
            .state_mut()
            .windows
            .insert(1, make_window(1, "Focused", true));
        app.niri
            .state_mut()
            .windows
            .insert(2, make_window(2, "Other", false));
        app.niri.state_mut().workspaces = vec![
            make_workspace(10, 1, "eDP-1", true, true, Some(1)),
            make_workspace(20, 2, "DP-1", true, false, Some(2)),
        ];
        app.modules.set_state(
            ModuleId::new("focused-window"),
            ModuleState::FocusedWindow(FocusedWindowState {
                title: Some("Focused".into()),
            }),
        );
        let wid = test_wid();
        app.surfaces
            .surfaces
            .insert(wid, SurfaceState::new("eDP-1".into(), String::new()));

        let model = app.bar_render_model(wid);

        match &model.center[0].state {
            ModuleState::FocusedWindow(state) => {
                assert_eq!(state.title.as_deref(), Some("Focused"));
            }
            state => panic!("expected focused-window state, got {state:?}"),
        }
    }

    #[test]
    fn bar_render_model_uses_active_title_for_non_focused_output() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "focused-window".into(),
                module_type: "focused-window".into(),
                params: ModuleParams::FocusedWindow(FocusedWindowConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.center = vec!["focused-window".into()];
        let (mut app, _) = App::boot(config, vec![]);
        app.niri.state_mut().focused_window_id = Some(1);
        app.niri
            .state_mut()
            .windows
            .insert(1, make_window(1, "Focused", true));
        app.niri
            .state_mut()
            .windows
            .insert(2, make_window(2, "Other", false));
        app.niri.state_mut().workspaces = vec![
            make_workspace(10, 1, "eDP-1", true, true, Some(1)),
            make_workspace(20, 2, "DP-1", true, false, Some(2)),
        ];
        app.modules.set_state(
            ModuleId::new("focused-window"),
            ModuleState::FocusedWindow(FocusedWindowState {
                title: Some("Focused".into()),
            }),
        );
        let wid = test_wid();
        app.surfaces
            .surfaces
            .insert(wid, SurfaceState::new("DP-1".into(), String::new()));

        let model = app.bar_render_model(wid);

        match &model.center[0].state {
            ModuleState::FocusedWindow(state) => {
                assert_eq!(state.title.as_deref(), Some("Other"));
            }
            state => panic!("expected focused-window state, got {state:?}"),
        }
    }

    #[test]
    fn bar_render_model_passes_surface_output_to_workspace_items() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "workspaces".into(),
                module_type: "workspaces".into(),
                params: ModuleParams::Workspaces(WorkspacesConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let workspaces = vec![
            make_workspace(10, 1, "eDP-1", true, true, None),
            make_workspace(20, 2, "DP-1", true, false, None),
        ];
        app.rebuild_workspace_item_modules(&workspaces);
        let wid = test_wid();
        app.surfaces
            .surfaces
            .insert(wid, SurfaceState::new("eDP-1".into(), String::new()));

        let model = app.bar_render_model(wid);

        let workspace_states: Vec<_> = model
            .start
            .iter()
            .filter(|entry| entry.id.0.starts_with("workspace:"))
            .map(|entry| &entry.state)
            .collect();
        assert_eq!(workspace_states.len(), 2);
        assert!(
            workspace_states
                .iter()
                .all(|state| state == &&ModuleState::WorkspaceOutput("eDP-1".into()))
        );
    }

    #[test]
    fn panel_layout_can_reference_module_absent_from_bar_layout() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec![];
        config.bars[0].panel.modules.center = vec!["clock".into()];
        let (app, _) = App::boot(config, vec![]);

        let layout = app.effective_panel_module_layout_for_bar(&app.config.bars[0]);

        assert_eq!(layout, vec![(Zone::Center, ModuleId::new("clock"))]);
    }

    #[test]
    fn panel_layout_expands_workspaces_placeholder() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "workspaces".into(),
                module_type: "workspaces".into(),
                params: ModuleParams::Workspaces(WorkspacesConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec![];
        config.bars[0].panel.modules.start = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let workspaces = vec![
            make_workspace(10, 1, "eDP-1", true, true, None),
            make_workspace(20, 2, "DP-1", true, false, None),
        ];
        app.rebuild_workspace_item_modules(&workspaces);

        let layout = app.effective_panel_module_layout_for_bar(&app.config.bars[0]);
        let workspace_items: Vec<_> = layout
            .into_iter()
            .filter(|(_, id)| id.0.starts_with("workspace:"))
            .collect();

        assert_eq!(workspace_items.len(), 2);
        assert!(workspace_items.iter().all(|(zone, _)| zone == &Zone::Start));
    }

    #[test]
    fn panel_render_model_uses_bar_panel_layout() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].panel.modules.center = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces.insert_panel_popup(
            panel,
            super::super::surfaces::PanelMeta {
                tree: crate::surface::panel::PanelTree {
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::BarPanel,
                    origin_window: origin,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    anchor: None,
                    screen_size: (1920, 1080),
                },
            },
        );

        let model = app.panel_render_model(panel).expect("panel model");

        assert_eq!(model.entries.len(), 1);
        assert_eq!(model.entries[0].id, ModuleId::new("clock"));
        assert_eq!(model.entries[0].zone, Zone::Center);
    }

    #[test]
    fn panel_render_model_uses_module_panel_content() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces.insert_panel_popup(
            panel,
            super::super::surfaces::PanelMeta {
                tree: crate::surface::panel::PanelTree {
                    origin: crate::surface::panel::PanelOrigin::Module(ModuleId::new("clock")),
                    content: crate::surface::panel::PanelContent::ModulePanel(ModuleId::new(
                        "clock",
                    )),
                    origin_window: origin,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    anchor: None,
                    screen_size: (1920, 1080),
                },
            },
        );

        let model = app.panel_render_model(panel).expect("panel model");

        assert_eq!(model.entries.len(), 1);
        assert_eq!(model.entries[0].id, ModuleId::new("clock"));
    }

    #[test]
    fn bar_render_model_marks_module_panel_origin_active() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces
            .surfaces
            .insert(origin, SurfaceState::new("DP-1".into(), "main".into()));
        app.surfaces.insert_panel_popup(
            panel,
            super::super::surfaces::PanelMeta {
                tree: crate::surface::panel::PanelTree {
                    origin: crate::surface::panel::PanelOrigin::Module(ModuleId::new("clock")),
                    content: crate::surface::panel::PanelContent::ModulePanel(ModuleId::new(
                        "clock",
                    )),
                    origin_window: origin,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    anchor: None,
                    screen_size: (1920, 1080),
                },
            },
        );

        let model = app.bar_render_model(origin);

        assert!(model.start[0].active);
    }

    #[test]
    fn bar_render_model_omits_modules_absent_from_surface_bar_layout() {
        let mut config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    ..Default::default()
                },
                BarConfig {
                    name: "secondary".into(),
                    ..Default::default()
                },
            ],
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            menus: vec![],
            command_policy: crate::config::CommandPolicy::default(),
        };
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surfaces
            .surfaces
            .insert(wid, SurfaceState::new("DP-1".into(), "secondary".into()));

        let model = app.bar_render_model(wid);

        assert!(model.start.is_empty());
        assert!(model.center.is_empty());
        assert!(model.end.is_empty());
    }

    #[test]
    fn bar_render_model_marks_hovered_and_focused_entries() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let mut surface = SurfaceState::new(String::new(), String::new());
        surface.hovered_module = Some(ModuleId::new("clock"));
        surface.focused_module = Some(ModuleId::new("clock"));
        app.surfaces.surfaces.insert(wid, surface);

        let model = app.bar_render_model(wid);

        assert_eq!(model.start[0].zone, Zone::Start);
        assert!(model.start[0].hovered);
        assert!(model.start[0].focused);
    }
}
