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
    pub(super) kind: ModuleKind,
    pub(super) zone: Zone,
    pub(super) hovered: bool,
    pub(super) focused: bool,
    pub(super) active: bool,
    pub(super) paired_toggle: bool,
    pub(super) element: Element<'a, Message>,
    #[cfg(test)]
    pub(super) state: ModuleState,
}

pub(super) struct BarRenderModel<'a> {
    pub(super) geometry: crate::surface::bar_geometry::BarGeometry,
    pub(super) ui: canopa_theme::ResolvedStyle,
    pub(super) edge: Edge,
    pub(super) is_horizontal: bool,
    pub(super) start: Vec<ModuleRenderEntry<'a>>,
    pub(super) center: Vec<ModuleRenderEntry<'a>>,
    pub(super) end: Vec<ModuleRenderEntry<'a>>,
}

pub(super) struct PanelRenderModel<'a> {
    pub(super) ui: canopa_theme::ResolvedStyle,
    pub(super) menu_metrics: crate::theme::MenuMetrics,
    pub(super) title: String,
    pub(super) drawer: bool,
    pub(super) adaptive: bool,
    pub(super) compact_at_end: bool,
    pub(super) confirmation: bool,
    pub(super) max_height: f32,
    pub(super) content_width: f32,
    pub(super) tree: &'a crate::surface::panel::PanelTree,
    pub(super) search_input_id: iced::widget::Id,
    pub(super) entries: Vec<ModuleRenderEntry<'a>>,
    pub(super) footer_start: usize,
    pub(super) external_content: Option<&'a crate::surface::panel::ExternalPanelContent>,
    pub(super) menu_entries: std::sync::Arc<[super::panel_menu_session::PanelMenuEntry]>,
    pub(super) icons: &'a super::panel_icons::PanelIcons,
    pub(super) show_icons: bool,
    pub(super) menu_prompt: Option<String>,
    pub(super) launcher_session: Option<u64>,
    pub(super) launcher_status: String,
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) keyboard_focus: bool,
    pub(super) can_go_back: bool,
    pub(super) return_to_bar: bool,
    pub(super) menu_scroll_id: iced::widget::Id,
    pub(super) menu_scroll_y: f32,
    pub(super) menu_viewport_height: f32,
    pub(super) bar_style: &'a crate::config::Style,
}

impl App {
    pub(super) fn panel_ui(
        &self,
        tree: &crate::surface::panel::PanelTree,
    ) -> canopa_theme::ResolvedStyle {
        let bar = Self::bar_for_surface_name(&self.config.bars, &tree.origin_bar_name);
        let mut ui = self.config.appearance.style(canopa_theme::UiKind::Panel);
        if matches!(
            tree.content,
            crate::surface::panel::PanelContent::Menu(_)
                | crate::surface::panel::PanelContent::DmenuRequest
        ) {
            ui = ui.apply(
                self.config.appearance.customization.ui[canopa_theme::UiKind::Menu as usize],
            );
        }
        ui = ui.apply(bar.panel.ui);
        if let crate::surface::panel::PanelContent::Menu(name) = &tree.content {
            ui = ui.apply(
                self.config
                    .element_ui
                    .menus
                    .get(name)
                    .copied()
                    .unwrap_or_default(),
            );
        }
        ui
    }
    pub(super) fn panel_menu_metrics(
        &self,
        tree: &crate::surface::panel::PanelTree,
    ) -> crate::theme::MenuMetrics {
        let ui = self.panel_ui(tree);
        let mut metrics = crate::theme::menu_metrics_with_gap(ui.custom_gap.then_some(ui.gap));
        if !metrics.row.custom_padding
            && let crate::surface::panel::PanelContent::Menu(name) = &tree.content
            && self.config.menus.iter().any(|menu| {
                &menu.name == name
                    && menu.source.as_ref().is_some_and(|source| {
                        matches!(&source.kind, crate::config::MenuSourceKind::Launcher(_))
                    })
            })
        {
            let labels = (crate::theme::body_size() + crate::theme::label_size()) * 1.3 + 1.0;
            metrics.extent = labels.ceil().max(32.0)
                + 6.0
                + metrics.row.margin.vertical()
                + 2.0 * metrics.row.border_width.px()
                + metrics.gap;
        }
        metrics
    }

    pub(super) fn active_menu_metrics(&self) -> crate::theme::MenuMetrics {
        self.foreground
            .active_panel_tree()
            .map(|tree| self.panel_menu_metrics(tree))
            .unwrap_or_else(crate::theme::menu_metrics)
    }
    pub(super) fn popup_render_model(
        &self,
        window: iced::window::Id,
    ) -> Option<PopupRenderModel<'_>> {
        let meta = self.foreground.active_menu_meta_for_window(window)?;

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
        let panel = self.foreground.panel_render_snapshot(window)?;
        let tree = panel.tree;
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
        let layout =
            self.panel_module_layout_for_content(tree.origin_window, &tree.origin, &tree.content);

        let is_bar_panel = matches!(tree.content, crate::surface::panel::PanelContent::BarPanel);
        let adaptive = !bar.edge.is_horizontal()
            && bar.panel.presentation == crate::config::PanelPresentation::Adaptive;
        let ui = self.panel_ui(tree);
        let panel_width = bar
            .panel
            .width
            .unwrap_or_else(|| bar.panel.max_width.unwrap_or(360).min(360));
        let content_width = panel_width as f32 - ui.padding.horizontal();
        let paired_toggles = if is_bar_panel && adaptive {
            layout.windows(2).find(|pair| {
                pair[0].zone == pair[1].zone
                    && matches!(
                        (
                            self.modules.kind_for(&pair[0].id),
                            self.modules.kind_for(&pair[1].id)
                        ),
                        (
                            Some(ModuleKind::Notifications),
                            Some(ModuleKind::IdleInhibitor)
                        ) | (
                            Some(ModuleKind::IdleInhibitor),
                            Some(ModuleKind::Notifications)
                        )
                    )
            })
        } else {
            None
        }
        .filter(|_| {
            let tile_width =
                (crate::theme::text_width("Do Not Disturb", crate::theme::body_size()) + 64.0)
                    .max(168.0);
            content_width >= 2.0 * tile_width + ui.gap.px()
        })
        .map(|pair| [pair[0].id.clone(), pair[1].id.clone()]);
        // Use the configured order so an invisible module still separates
        // controls from the trailing actions.
        let footer: Vec<_> = if is_bar_panel {
            self.effective_panel_module_layout_for_bar(bar)
                .into_iter()
                .rev()
                .take_while(|(_, id)| self.modules.kind_for(id) == Some(ModuleKind::Button))
                .map(|(_, id)| id)
                .collect()
        } else {
            Vec::new()
        };
        let entries: Vec<_> = layout
            .into_iter()
            .filter_map(|entry| {
                let id = entry.id;
                let kind = self.modules.kind_for(&id)?;
                let in_footer = footer.contains(&id);
                let paired_toggle = paired_toggles
                    .as_ref()
                    .is_some_and(|pair| pair.contains(&id));
                let state = self.module_render_state(kind, &id, &origin_surface);
                let request = super::module_runtime::ModuleDetailRenderRequest {
                    id: id.clone(),
                    style: &bar.style,
                    session: self.foreground.panel_detail_for_render(window, &id),
                    state_override: Some(state.clone()),
                };
                let module_view = if is_bar_panel {
                    self.modules.render_bar_panel_module(
                        super::module_runtime::BarPanelModuleRenderRequest {
                            detail: request,
                            window,
                            focused: entry.focused,
                            footer: in_footer,
                            paired_toggle,
                        },
                    )?
                } else {
                    self.modules.render_module_detail(request)?
                };
                let element = if in_footer {
                    module_view
                } else if matches!(
                    tree.content,
                    crate::surface::panel::PanelContent::ModulePanel(_)
                ) {
                    // Single-module content uses the same body in both presentations.
                    container(module_view).width(iced::Length::Fill).into()
                } else {
                    crate::surface::panel::panel_module_frame(module_view, entry.focused)
                };

                Some(ModuleRenderEntry {
                    id,
                    kind,
                    zone: entry.zone,
                    hovered: false,
                    focused: entry.focused,
                    active: entry.focused,
                    paired_toggle,
                    element,
                    #[cfg(test)]
                    state,
                })
            })
            .collect();
        let footer_start = entries
            .iter()
            .position(|entry| footer.contains(&entry.id))
            .unwrap_or(entries.len());

        let menu = panel.menu;
        let confirmation = bar.panel.presentation == crate::config::PanelPresentation::Adaptive
            && matches!(&tree.content, crate::surface::panel::PanelContent::Menu(name)
                if self.config.menus.iter().any(|menu|
                    &menu.name == name
                    && menu.source.is_none()
                    && menu.prompt.is_some()
                    && matches!(menu.items.as_slice(), [cancel, confirm]
                        if cancel.children.is_empty() && confirm.children.is_empty()
                        && cancel.action == Some(crate::config::PanelAction::OpenBarPanel)
                        && matches!(confirm.action, Some(crate::config::PanelAction::Command(_))))));
        let compact_at_end = !bar.edge.is_horizontal()
            && matches!(&tree.origin, crate::surface::panel::PanelOrigin::Module(origin)
                if self.effective_module_layout_for_bar(bar).iter().any(|(zone, id)|
                    *zone == Zone::End && id == origin));

        let title = {
            use crate::surface::panel::PanelContent;
            match &tree.content {
                PanelContent::BarPanel => bar
                    .panel
                    .title
                    .clone()
                    .unwrap_or_else(|| "Overview".to_owned()),
                PanelContent::ModulePanel(id) => match self.modules.kind_for(id) {
                    Some(ModuleKind::Clock) => "Calendar".to_owned(),
                    Some(ModuleKind::Volume) => "Sound".to_owned(),
                    Some(ModuleKind::Network) => "Network".to_owned(),
                    Some(ModuleKind::Notifications) => "Notifications".to_owned(),
                    Some(ModuleKind::IdleInhibitor) => "Idle inhibitor".to_owned(),
                    Some(ModuleKind::Battery) => "Battery".to_owned(),
                    Some(ModuleKind::Workspaces | ModuleKind::WorkspaceItem) => {
                        "Workspaces".to_owned()
                    }
                    Some(ModuleKind::FocusedWindow) => "Focused window".to_owned(),
                    _ => self
                        .modules
                        .configured_module_name_for_instance(id)
                        .to_owned(),
                },
                PanelContent::Menu(name)
                    if name == "Applications" && menu.launcher_session.is_some() =>
                {
                    "Launcher".into()
                }
                PanelContent::Menu(name) => name.clone(),
                PanelContent::DmenuRequest => "Choose an item".to_owned(),
                PanelContent::ExternalProvider(content) => content
                    .document
                    .canopa()
                    .map(|snapshot| snapshot.content.title.clone())
                    .unwrap_or_else(|| match content.provider_name.as_str() {
                        "wifi" => "Wi-Fi".into(),
                        _ => content.provider_name.clone(),
                    }),
            }
        };

        Some(PanelRenderModel {
            ui,
            menu_metrics: self.panel_menu_metrics(tree),
            title,
            drawer: bar.uses_drawer_for(&tree.content),
            adaptive,
            compact_at_end,
            confirmation,
            content_width,
            max_height: bar.panel.height.unwrap_or_else(|| {
                bar.panel
                    .max_height
                    .unwrap_or(u32::MAX)
                    .min(tree.screen_size.1)
            }) as f32,
            tree,
            search_input_id: super::panel::panel_search_input_id(window),
            entries,
            footer_start,
            external_content: match &tree.content {
                crate::surface::panel::PanelContent::ExternalProvider(content) => Some(content),
                _ => None,
            },
            menu_entries: menu.entries,
            icons: &self.panel_icons,
            show_icons: menu.show_icons,
            menu_prompt: menu.prompt,
            launcher_session: menu.launcher_session,
            launcher_status: menu.launcher_status,
            search_query: menu.search_query,
            focused_index: menu.focused_index,
            keyboard_focus: menu.keyboard_focus,
            can_go_back: menu.can_go_back,
            return_to_bar: self.foreground.panel_return_to_bar(),
            menu_scroll_id: super::panel::panel_menu_scroll_id(window),
            menu_scroll_y: menu.scroll_y,
            menu_viewport_height: menu.viewport_height,
            bar_style: &bar.style,
        })
    }

    pub(super) fn bar_render_model(&self, window: iced::window::Id) -> BarRenderModel<'_> {
        let surface = self.surface(window).cloned().unwrap_or_default();
        let bar = Self::bar_for_surface_name(&self.config.bars, &surface.bar_name);
        let is_horizontal = bar.edge.is_horizontal();

        let shows_expanded = false;

        let mut start = Vec::new();
        let mut center = Vec::new();
        let mut end = Vec::new();
        let panel_open = self.foreground.active_panel_window().is_some();

        for (zone, id) in self.effective_module_layout_for_bar(bar) {
            let Some(kind) = self.modules.kind_for(&id) else {
                continue;
            };

            let state = self.module_render_state(kind, &id, &surface);
            let mode = if is_horizontal {
                crate::config::FormatMode::Horizontal
            } else {
                crate::config::FormatMode::Collapsed
            };
            if !self.modules.has_content(&id, &state, mode) {
                continue;
            }
            let hovered = !panel_open && surface.hovered_module.as_ref() == Some(&id);
            let focused = !panel_open && surface.focused_module.as_ref() == Some(&id);
            let active = self.foreground.active_panel_origin_for(window)
                == Some(&crate::surface::panel::PanelOrigin::Module(id.clone()));

            let local = self
                .config
                .element_ui
                .modules
                .get(self.modules.configured_module_name_for_instance(&id))
                .copied()
                .unwrap_or_default();
            let ui = self
                .config
                .appearance
                .style(canopa_theme::UiKind::Module)
                .apply(local);
            let mut style = bar.style;
            let mut bar_ui =
                self.config.appearance.customization.ui[canopa_theme::UiKind::Bar as usize];
            bar_ui.overlay(bar.ui);
            if style.color.is_none()
                && let Some(color) = bar_ui.foreground
            {
                style.color = Some(crate::config::style::ColorValue::Rgba(
                    canopa_ui::resolve_color(&self.config.appearance, color),
                ));
            }
            let mut overrides =
                self.config.appearance.customization.ui[canopa_theme::UiKind::Module as usize];
            overrides.overlay(local);
            let state_ui = ui.state(active, hovered, false, false, focused);
            if overrides.foreground.is_some()
                || [active, hovered, false, false, focused]
                    .into_iter()
                    .zip(ui.states)
                    .any(|(on, state)| on && state.foreground.is_some())
            {
                style.color = Some(crate::config::style::ColorValue::Rgba(
                    canopa_ui::resolve_color(&self.config.appearance, state_ui.foreground),
                ));
            }
            let Some(module_view) =
                self.modules
                    .render_bar_module(super::module_runtime::BarModuleRenderRequest {
                        id: id.clone(),
                        window,
                        edge: &bar.edge,
                        expanded: shows_expanded,
                        style: &style,
                        state_override: Some(state.clone()),
                    })
            else {
                continue;
            };
            let framed_module = container(module_frame(module_view, hovered, focused, active, ui))
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
                kind,
                zone: zone.clone(),
                hovered,
                focused,
                active,
                paired_toggle: false,
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
            geometry: crate::surface::bar_geometry::BarGeometry::resolve(
                bar,
                surface.screen_size,
                surface.drawer_width,
            ),
            ui: self
                .config
                .appearance
                .style(canopa_theme::UiKind::Bar)
                .apply(bar.ui),
            edge: bar.edge.clone(),
            is_horizontal,
            start,
            center,
            end,
        }
    }

    pub(super) fn module_render_state(
        &self,
        kind: ModuleKind,
        id: &ModuleId,
        surface: &crate::surface::SurfaceState,
    ) -> ModuleState {
        if kind == ModuleKind::WorkspaceItem {
            return ModuleState::WorkspaceOutput(surface.output_name.clone());
        }

        let stored_state = self.modules.render_state_for(id);

        if matches!(stored_state, ModuleState::FocusedWindow(_)) {
            let niri = self.niri.state();
            let window = if surface.output_name.is_empty() {
                niri.focused_or_active_window()
            } else {
                let focused_output = niri.focused_output();
                if focused_output.is_some_and(|output| output == surface.output_name.as_str()) {
                    niri.focused_or_active_window()
                } else {
                    niri.active_window_for_output(&surface.output_name)
                }
            };
            return ModuleState::FocusedWindow(focused_window::FocusedWindowState {
                window: window.map(Self::focused_window_info_from_window),
            });
        }

        stored_state
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::config::{
        BarConfig, ClockConfig, Config, FocusedWindowConfig, MenuConfig, MenuItemConfig,
        ModuleConfig, ModuleParams, WorkspacesConfig,
    };
    use crate::modules::focused_window;
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

    fn focused_window_state_with_title(title: &str) -> focused_window::FocusedWindowState {
        focused_window::FocusedWindowState {
            window: Some(focused_window::FocusedWindowInfo {
                title: Some(title.into()),
                app_id: None,
                application_icon: None,
            }),
        }
    }

    fn focused_window_state_title(state: &focused_window::FocusedWindowState) -> Option<&str> {
        state.window.as_ref().and_then(|info| info.title.as_deref())
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

    fn open_and_register_panel(
        app: &mut App,
        panel: iced::window::Id,
        tree: crate::surface::panel::PanelTree,
    ) {
        let origin = tree.origin_window;
        {
            let surface = app.surface_mut(origin);
            surface.bar_name = tree.origin_bar_name.clone();
            surface.output_name = tree.origin_output_name.clone();
            surface.screen_size = tree.screen_size;
        }
        let request = app.panel_open_request(
            origin,
            tree.origin.clone(),
            tree.content.clone(),
            tree.anchor,
        );
        let effects = app.prepare_panel_for_test(request);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            crate::app::foreground::ForegroundEffect::OpenPanelWindow { .. }
        )));
        app.register_panel_popup(panel, origin);
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
        let _ = app.modules.handle(
            super::super::module_runtime::ModuleRuntimeIntent::ApplyModuleState {
                id: ModuleId::new("focused-window"),
                state: ModuleState::FocusedWindow(focused_window_state_with_title("Focused")),
            },
        );
        let wid = test_wid();
        app.surfaces
            .surfaces
            .insert(wid, SurfaceState::new("eDP-1".into(), String::new()));

        let model = app.bar_render_model(wid);

        match &model.center[0].state {
            ModuleState::FocusedWindow(state) => {
                assert_eq!(focused_window_state_title(state), Some("Focused"));
            }
            state => panic!("expected focused-window state, got {state:?}"),
        }
    }

    #[test]
    fn bar_render_model_uses_active_title_for_focused_output_without_toplevel_focus() {
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
        app.niri
            .state_mut()
            .windows
            .insert(1, make_window(1, "Active", false));
        app.niri.state_mut().workspaces = vec![make_workspace(10, 1, "eDP-1", true, true, Some(1))];
        let _ = app.modules.handle(
            super::super::module_runtime::ModuleRuntimeIntent::ApplyModuleState {
                id: ModuleId::new("focused-window"),
                state: ModuleState::FocusedWindow(focused_window::FocusedWindowState {
                    window: None,
                }),
            },
        );
        let wid = test_wid();
        app.surfaces
            .surfaces
            .insert(wid, SurfaceState::new("eDP-1".into(), String::new()));

        let model = app.bar_render_model(wid);

        match &model.center[0].state {
            ModuleState::FocusedWindow(state) => {
                assert_eq!(focused_window_state_title(state), Some("Active"));
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
        let _ = app.modules.handle(
            super::super::module_runtime::ModuleRuntimeIntent::ApplyModuleState {
                id: ModuleId::new("focused-window"),
                state: ModuleState::FocusedWindow(focused_window_state_with_title("Focused")),
            },
        );
        let wid = test_wid();
        app.surfaces
            .surfaces
            .insert(wid, SurfaceState::new("DP-1".into(), String::new()));

        let model = app.bar_render_model(wid);

        match &model.center[0].state {
            ModuleState::FocusedWindow(state) => {
                assert_eq!(focused_window_state_title(state), Some("Other"));
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
            .filter(|entry| entry.id.display_name().starts_with("workspace:"))
            .map(|entry| &entry.state)
            .collect();
        assert_eq!(workspace_states.len(), 1);
        assert_eq!(model.start[0].id, ModuleId::workspace("workspaces", 10));
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
            .filter(|(_, id)| id.display_name().starts_with("workspace:"))
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
        crate::app::tests::ready_clock(&mut app);
        let origin = test_wid();
        let panel = test_wid();
        open_and_register_panel(
            &mut app,
            panel,
            crate::surface::panel::PanelTree {
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::BarPanel,
                origin_window: origin,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                anchor: None,
                screen_size: (1920, 1080),
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
        crate::app::tests::ready_clock(&mut app);
        let origin = test_wid();
        let panel = test_wid();
        open_and_register_panel(
            &mut app,
            panel,
            crate::surface::panel::PanelTree {
                origin: crate::surface::panel::PanelOrigin::Module(ModuleId::new("clock")),
                content: crate::surface::panel::PanelContent::ModulePanel(ModuleId::new("clock")),
                origin_window: origin,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                anchor: None,
                screen_size: (1920, 1080),
            },
        );

        let model = app.panel_render_model(panel).expect("panel model");

        assert_eq!(model.entries.len(), 1);
        assert_eq!(model.entries[0].id, ModuleId::new("clock"));
    }

    #[test]
    fn panel_render_model_exposes_menu_icon_rendering() {
        let config = Config {
            menus: vec![MenuConfig {
                name: "main".into(),
                show_icons: true,
                prompt: None,
                source: None,
                items: vec![MenuItemConfig {
                    label: "Terminal".into(),
                    icon: Some("utilities-terminal".into()),
                    keywords: vec![],
                    action: None,
                    children: vec![],
                }],
            }],
            ..Default::default()
        };
        let (mut app, _) = App::boot(config, vec![]);
        let origin = test_wid();
        let panel = test_wid();
        open_and_register_panel(
            &mut app,
            panel,
            crate::surface::panel::PanelTree {
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::Menu("main".into()),
                origin_window: origin,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                anchor: None,
                screen_size: (1920, 1080),
            },
        );

        let model = app.panel_render_model(panel).expect("panel model");

        assert!(model.show_icons);
        assert_eq!(
            model.menu_entries[0].icon,
            Some("utilities-terminal".into())
        );
    }

    #[test]
    fn panel_render_model_exposes_parent_navigation_for_submenu() {
        let config = Config {
            menus: vec![MenuConfig {
                name: "main".into(),
                show_icons: false,
                prompt: None,
                source: None,
                items: vec![MenuItemConfig {
                    label: "Apps".into(),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![MenuItemConfig {
                        label: "Browser".into(),
                        icon: None,
                        keywords: vec![],
                        action: None,
                        children: vec![],
                    }],
                }],
            }],
            ..Default::default()
        };
        let (mut app, _) = App::boot(config, vec![]);
        let origin = test_wid();
        let panel = test_wid();
        open_and_register_panel(
            &mut app,
            panel,
            crate::surface::panel::PanelTree {
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::Menu("main".into()),
                origin_window: origin,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                anchor: None,
                screen_size: (1920, 1080),
            },
        );
        assert!(
            !app.panel_render_model(panel)
                .expect("panel model")
                .can_go_back
        );

        let _ = app.update(crate::app::Message::Panel(
            crate::app::messages::PanelEvent::ActivateIndex(0),
        ));

        let model = app.panel_render_model(panel).expect("panel model");
        assert!(model.can_go_back);
        assert_eq!(model.menu_entries[0].label, "Browser");
    }

    #[test]
    fn dmenu_request_with_entry_icon_enables_menu_icons() {
        let (mut app, _) = App::boot(Config::default(), Vec::new());
        let (sender, _receiver) = tokio::sync::oneshot::channel();
        let origin = test_wid();
        let panel = test_wid();
        {
            let surface = app.surface_mut(origin);
            surface.bar_name = "main".into();
            surface.output_name = "DP-1".into();
            surface.screen_size = (1920, 1080);
        }
        let panel_request = app.panel_open_request(
            origin,
            crate::surface::panel::PanelOrigin::Bar,
            crate::surface::panel::PanelContent::DmenuRequest,
            None,
        );
        let effects = app.foreground.handle(
            crate::app::foreground::ForegroundIntent::BeginDmenuSelection {
                request: crate::ipc::protocol::DmenuRequest {
                    prompt: None,
                    entries: vec![crate::ipc::protocol::DmenuEntry {
                        label: Some("Firefox".into()),
                        value: Some("firefox".into()),
                        icon: Some("firefox".into()),
                        keywords: Vec::new(),
                    }],
                    allow_custom: true,
                },
                responder: crate::app::messages::ipc_responder(sender),
                panel_request: Some(panel_request),
            },
        );
        assert!(effects.iter().any(|effect| matches!(
            effect,
            crate::app::foreground::ForegroundEffect::OpenPanelWindow { .. }
        )));
        app.register_panel_popup(panel, origin);

        let model = app.panel_render_model(panel).expect("panel model");

        assert!(model.show_icons);
        assert_eq!(model.menu_entries[0].icon, Some("firefox".into()));
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
        crate::app::tests::ready_clock(&mut app);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces
            .surfaces
            .insert(origin, SurfaceState::new("DP-1".into(), "main".into()));
        open_and_register_panel(
            &mut app,
            panel,
            crate::surface::panel::PanelTree {
                origin: crate::surface::panel::PanelOrigin::Module(ModuleId::new("clock")),
                content: crate::surface::panel::PanelContent::ModulePanel(ModuleId::new("clock")),
                origin_window: origin,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                anchor: None,
                screen_size: (1920, 1080),
            },
        );

        let model = app.bar_render_model(origin);

        assert!(model.start[0].active);
    }

    #[test]
    fn panel_render_model_ignores_stale_panel_window_attachment() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let origin = test_wid();
        let stale_panel = test_wid();
        let current_panel = test_wid();

        open_and_register_panel(
            &mut app,
            stale_panel,
            crate::surface::panel::PanelTree {
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::BarPanel,
                origin_window: origin,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                anchor: None,
                screen_size: (1920, 1080),
            },
        );
        open_and_register_panel(
            &mut app,
            current_panel,
            crate::surface::panel::PanelTree {
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::Menu("main".into()),
                origin_window: origin,
                origin_bar_name: "main".into(),
                origin_output_name: "DP-1".into(),
                anchor: None,
                screen_size: (1920, 1080),
            },
        );

        assert!(app.panel_render_model(stale_panel).is_none());
        assert!(app.panel_render_model(current_panel).is_some());
    }

    #[test]
    fn bar_render_model_omits_modules_absent_from_surface_bar_layout() {
        let mut config = Config {
            appearance: Default::default(),
            element_ui: Default::default(),
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
            panel_providers: vec![],
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
    fn bar_render_model_uses_surface_bar_order_for_dynamic_tray_icons() {
        let mut config = Config {
            appearance: Default::default(),
            element_ui: Default::default(),
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
            modules: vec![
                ModuleConfig {
                    name: "clock".into(),
                    module_type: "clock".into(),
                    params: ModuleParams::Clock(ClockConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "tray".into(),
                    module_type: "tray".into(),
                    params: ModuleParams::Tray(crate::config::TrayConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            menus: vec![],
            panel_providers: vec![],
        };
        config.bars[0].modules.end = vec!["clock".into(), "tray".into()];
        config.bars[1].modules.end = vec!["tray".into(), "clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        crate::app::tests::ready_clock(&mut app);
        let wid = test_wid();
        app.surfaces
            .surfaces
            .insert(wid, SurfaceState::new("DP-1".into(), "secondary".into()));

        let item = crate::dbus::sni::host::TrayItem {
            service: "org.test".into(),
            object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: crate::dbus::sni::host::ItemStatus::Active,
            item_is_menu: false,
            menu_path: None,
            tooltip: None,
        };
        let _ = app.update(crate::app::Message::Tray(
            crate::app::messages::TrayRuntimeEvent::Host(
                crate::dbus::sni::host::TrayEvent::Snapshot(vec![item]),
            ),
        ));

        let model = app.bar_render_model(wid);
        let end_ids: Vec<_> = model
            .end
            .iter()
            .map(|entry| entry.id.display_name())
            .collect();

        assert_eq!(end_ids, vec!["tray:org.test", "clock"]);
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
        crate::app::tests::ready_clock(&mut app);
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

#[cfg(test)]
mod theme_tests {
    use super::*;
    use canopa_theme::{ColorValue, Pixels, Rgb, UiKind, UiStyle};
    #[test]
    fn local_panel_and_menu_overrides_win_and_share_the_menu_gap() {
        let mut config = crate::config::Config::default();
        std::sync::Arc::make_mut(&mut config.appearance.customization).ui[UiKind::Menu as usize] =
            UiStyle {
                background: Some(ColorValue::Rgba(Rgb(1, 2, 3), 255)),
                gap: Some(Pixels::whole(5)),
                ..Default::default()
            };
        config.bars[0].panel.ui = UiStyle {
            background: Some(ColorValue::Rgba(Rgb(4, 5, 6), 255)),
            gap: Some(Pixels::whole(19)),
            ..Default::default()
        };
        let (mut app, _) = App::boot(config, vec![]);
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("custom".into()),
            origin_window: iced::window::Id::unique(),
            origin_bar_name: "main".into(),
            origin_output_name: "".into(),
            anchor: None,
            screen_size: (1280, 40),
        };
        assert_eq!(
            app.panel_ui(&tree).background,
            ColorValue::Rgba(Rgb(4, 5, 6), 255)
        );
        assert_eq!(app.panel_menu_metrics(&tree).gap, 19.0);
        app.config.element_ui.menus.insert(
            "custom".into(),
            UiStyle {
                gap: Some(Pixels::ZERO),
                ..Default::default()
            },
        );
        assert_eq!(app.panel_menu_metrics(&tree).gap, 0.0);
        assert_eq!(
            app.panel_ui(&tree).background,
            ColorValue::Rgba(Rgb(4, 5, 6), 255)
        );
    }
}
