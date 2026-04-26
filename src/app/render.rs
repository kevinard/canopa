use iced::Element;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::mouse_area;

use super::{App, Message};
use crate::config::Edge;
use crate::modules::focused_window;
use crate::modules::{ModuleId, ModuleKind, ModuleState, Zone};
use crate::surface::frame::module_frame;
use crate::surface::menu_popup::{MenuTree, popup_size};

pub(super) struct PopupRenderModel<'a> {
    pub(super) service: &'a str,
    pub(super) menu_path: &'a zbus::zvariant::OwnedObjectPath,
    pub(super) tree: &'a MenuTree,
    pub(super) placement: PopupPlacementModel,
}

pub(super) struct PopupPlacementModel {
    pub(super) h_align: Horizontal,
    pub(super) v_align: Vertical,
    pub(super) padding: iced::Padding,
    pub(super) popup_width: u32,
    pub(super) popup_height: u32,
}

#[allow(dead_code)]
pub(super) struct ModuleRenderEntry<'a> {
    pub(super) id: ModuleId,
    pub(super) zone: Zone,
    pub(super) hovered: bool,
    pub(super) focused: bool,
    pub(super) element: Element<'a, Message>,
    #[cfg(test)]
    pub(super) state: ModuleState,
}

pub(super) struct BarRenderModel<'a> {
    pub(super) edge: Edge,
    pub(super) content_size: u32,
    pub(super) is_horizontal: bool,
    pub(super) top: Vec<ModuleRenderEntry<'a>>,
    pub(super) center: Vec<ModuleRenderEntry<'a>>,
    pub(super) bottom: Vec<ModuleRenderEntry<'a>>,
}

impl App {
    pub(super) fn popup_render_model(
        &self,
        window: iced::window::Id,
    ) -> Option<PopupRenderModel<'_>> {
        let meta = self.surfaces.popup(window)?;
        let bar = Self::bar_for_surface_name(&self.config.bars, &meta.bar_name);

        Some(PopupRenderModel {
            service: meta.service.as_str(),
            menu_path: &meta.menu_path,
            tree: &meta.tree,
            placement: popup_placement_model(&bar.edge, bar.collapsed_width as f32, &meta.tree),
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
        let mut top = Vec::new();
        let mut center = Vec::new();
        let mut bottom = Vec::new();

        for (_boot_zone, module) in &self.modules {
            let id = module.id();
            let Some(zone) = effective_zone.get(&id) else {
                continue;
            };

            let state = self.module_render_state(module.kind(), &id, &surface, &default_state);
            let hovered = surface.hovered_module.as_ref() == Some(&id);
            let focused = surface.focused_module.as_ref() == Some(&id);

            let module_view = module.view_for_bar(&state, window, is_horizontal, shows_expanded);
            let scroll_id = id.clone();
            let element: Element<'_, Message> =
                mouse_area(module_frame(module_view, hovered || focused))
                    .on_enter(Message::Surface(
                        super::messages::SurfaceEvent::ModuleHoverEnter(id.clone(), window),
                    ))
                    .on_exit(Message::Surface(
                        super::messages::SurfaceEvent::ModuleHoverExit(id.clone(), window),
                    ))
                    .on_press(Message::Modules(
                        super::messages::ModuleRuntimeEvent::ModuleClick(id.clone(), window),
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
                element,
                #[cfg(test)]
                state,
            };

            match zone {
                Zone::Top => top.push(entry),
                Zone::Center => center.push(entry),
                Zone::Bottom => bottom.push(entry),
            }
        }

        BarRenderModel {
            edge: bar.edge.clone(),
            content_size,
            is_horizontal,
            top,
            center,
            bottom,
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

fn popup_placement_model(edge: &Edge, bar_size: f32, tree: &MenuTree) -> PopupPlacementModel {
    let (popup_width, popup_height) = popup_size(tree);
    let screen_w = tree.screen_size.0 as f32;
    let screen_h = tree.screen_size.1 as f32;
    let raw_cursor_x = tree.anchor.0 as f32;
    let raw_cursor_y = tree.anchor.1 as f32;
    let cursor_x = if screen_w > 0.0 {
        raw_cursor_x.min(screen_w - popup_width as f32).max(0.0)
    } else {
        raw_cursor_x
    };
    let cursor_y = if screen_h > 0.0 {
        raw_cursor_y.min(screen_h - popup_height as f32).max(0.0)
    } else {
        raw_cursor_y
    };

    let (h_align, v_align, padding) = match edge {
        Edge::Left => (
            Horizontal::Left,
            Vertical::Top,
            iced::Padding {
                top: cursor_y,
                left: bar_size,
                ..Default::default()
            },
        ),
        Edge::Right => (
            Horizontal::Right,
            Vertical::Top,
            iced::Padding {
                top: cursor_y,
                right: bar_size,
                ..Default::default()
            },
        ),
        Edge::Top => (
            Horizontal::Left,
            Vertical::Top,
            iced::Padding {
                top: bar_size,
                left: cursor_x,
                ..Default::default()
            },
        ),
        Edge::Bottom => (
            Horizontal::Left,
            Vertical::Bottom,
            iced::Padding {
                bottom: bar_size,
                left: cursor_x,
                ..Default::default()
            },
        ),
    };

    PopupPlacementModel {
        h_align,
        v_align,
        padding,
        popup_width,
        popup_height,
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::config::{
        BarConfig, ClockConfig, Config, Edge, FocusedWindowConfig, ModuleConfig, ModuleParams,
        WorkspacesConfig,
    };
    use crate::modules::focused_window::FocusedWindowState;
    use crate::modules::{ModuleId, ModuleState, Zone};
    use crate::surface::SurfaceState;
    use crate::surface::menu_popup::{MenuItem, MenuTree};

    fn tree(anchor: (i32, i32), screen_size: (u32, u32)) -> MenuTree {
        MenuTree {
            items: vec![MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor,
            screen_size,
        }
    }

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
    fn popup_placement_uses_edge_relative_alignment_and_bar_padding() {
        let tree = tree((40, 30), (500, 400));

        let left = super::popup_placement_model(&Edge::Left, 48.0, &tree);
        assert_eq!(left.padding.left, 48.0);
        assert_eq!(left.padding.top, 30.0);

        let right = super::popup_placement_model(&Edge::Right, 48.0, &tree);
        assert_eq!(right.padding.right, 48.0);
        assert_eq!(right.padding.top, 30.0);

        let top = super::popup_placement_model(&Edge::Top, 48.0, &tree);
        assert_eq!(top.padding.top, 48.0);
        assert_eq!(top.padding.left, 40.0);

        let bottom = super::popup_placement_model(&Edge::Bottom, 48.0, &tree);
        assert_eq!(bottom.padding.bottom, 48.0);
        assert_eq!(bottom.padding.left, 40.0);
    }

    #[test]
    fn popup_placement_clamps_anchor_to_screen_bounds() {
        let tree = tree((490, 390), (500, 400));
        let placement = super::popup_placement_model(&Edge::Top, 48.0, &tree);

        assert_eq!(placement.padding.left, 350.0);
        assert_eq!(placement.padding.top, 48.0);
    }

    #[test]
    fn popup_placement_preserves_anchor_when_screen_size_is_unknown() {
        let tree = tree((490, 390), (0, 0));
        let placement = super::popup_placement_model(&Edge::Top, 48.0, &tree);

        assert_eq!(placement.padding.left, 490.0);
        assert_eq!(placement.padding.top, 48.0);
    }

    #[test]
    fn bar_render_model_uses_focused_title_for_focused_output() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "focused-window".into(),
                module_type: "focused-window".into(),
                params: ModuleParams::FocusedWindow(FocusedWindowConfig::default()),
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
            }],
            ..Default::default()
        };
        config.bars[0].modules.top = vec!["workspaces".into()];
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
            .top
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
            }],
        };
        config.bars[0].modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        app.surfaces
            .surfaces
            .insert(wid, SurfaceState::new("DP-1".into(), "secondary".into()));

        let model = app.bar_render_model(wid);

        assert!(model.top.is_empty());
        assert!(model.center.is_empty());
        assert!(model.bottom.is_empty());
    }

    #[test]
    fn bar_render_model_marks_hovered_and_focused_entries() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
            }],
            ..Default::default()
        };
        config.bars[0].modules.top = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let mut surface = SurfaceState::new(String::new(), String::new());
        surface.hovered_module = Some(ModuleId::new("clock"));
        surface.focused_module = Some(ModuleId::new("clock"));
        app.surfaces.surfaces.insert(wid, surface);

        let model = app.bar_render_model(wid);

        assert_eq!(model.top[0].zone, Zone::Top);
        assert!(model.top[0].hovered);
        assert!(model.top[0].focused);
    }
}
