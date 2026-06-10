use iced::Task;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use std::time::Duration;

use super::app_effects::AppEffect;
use super::{App, Message};

pub(super) fn panel_search_input_id(window: iced::window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("panel-search-input-{window:?}"))
}

pub(super) fn panel_menu_scroll_id(window: iced::window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("panel-menu-scroll-{window:?}"))
}

pub(super) const MENU_ROW_EXTENT: f32 = 45.0;
const DEFAULT_MENU_VIEWPORT_HEIGHT: f32 = MENU_ROW_EXTENT * 12.0;
const DEFAULT_PANEL_FALLBACK_WIDTH: u32 = 360;
const DEFAULT_PANEL_FALLBACK_HEIGHT: u32 = 600;
const PANEL_OUTER_PADDING: u32 = 16;
const PANEL_MODULE_FRAME_H_PADDING: u32 = 20;
const PANEL_MODULE_FRAME_V_PADDING: u32 = 16;
const PANEL_ENTRY_SPACING: u32 = 8;
const PANEL_MENU_MIN_WIDTH: u32 = 240;
const PANEL_MENU_SEARCH_HEIGHT: u32 = 38;
const PANEL_MENU_LIST_PADDING: u32 = 16;
const PANEL_MENU_ROW_H_PADDING: u32 = 20;
const PANEL_MENU_CHAR_WIDTH: u32 = 8;
const PANEL_MENU_ICON_SLOT: u32 = 26;
const PANEL_MENU_SUBMENU_SLOT: u32 = 16;
const PANEL_SEARCH_FOCUS_RETRIES: u8 = 8;
const PANEL_SEARCH_FOCUS_RETRY_DELAY: Duration = Duration::from_millis(25);

pub(super) fn request_panel_search_focus() -> Task<Message> {
    Task::done(Message::Panel(
        super::messages::PanelEvent::FocusSearchInput {
            retries: PANEL_SEARCH_FOCUS_RETRIES,
        },
    ))
}

fn panel_origin_label(origin: &crate::surface::panel::PanelOrigin) -> &'static str {
    match origin {
        crate::surface::panel::PanelOrigin::Bar => "bar",
        crate::surface::panel::PanelOrigin::Module(_) => "module",
    }
}

fn panel_content_label(content: &crate::surface::panel::PanelContent) -> &'static str {
    match content {
        crate::surface::panel::PanelContent::BarPanel => "bar-panel",
        crate::surface::panel::PanelContent::ModulePanel(_) => "module-panel",
        crate::surface::panel::PanelContent::Menu(_) => "menu",
        crate::surface::panel::PanelContent::DmenuRequest => "dmenu-request",
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PanelContentSizeHint {
    preferred_width: Option<u32>,
    preferred_height: Option<u32>,
    fill_edge_span: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedPanelDimension {
    value: u32,
    fill: bool,
}

impl ResolvedPanelDimension {
    fn exact(value: u32) -> Self {
        Self { value, fill: false }
    }

    fn fill(span: u32) -> Self {
        Self {
            value: span,
            fill: true,
        }
    }

    fn layer_size(self) -> u32 {
        if self.fill { 0 } else { self.value }
    }
}

fn panel_window_settings(
    bar: &crate::config::BarConfig,
    tree: &crate::surface::panel::PanelTree,
    surface_origin: (i32, i32),
    hint: PanelContentSizeHint,
) -> NewLayerShellSettings {
    let fill_width = hint.fill_edge_span && bar.edge.is_horizontal();
    let fill_height = hint.fill_edge_span && !bar.edge.is_horizontal();
    let width = resolve_panel_dimension(
        bar.panel.width,
        bar.panel.max_width,
        hint.preferred_width,
        DEFAULT_PANEL_FALLBACK_WIDTH,
        fill_width,
        tree.screen_size.0,
    );
    let height = resolve_panel_dimension(
        bar.panel.height,
        bar.panel.max_height,
        hint.preferred_height,
        DEFAULT_PANEL_FALLBACK_HEIGHT,
        fill_height,
        tree.screen_size.1,
    );
    let anchor = panel_layer_anchor(&bar.edge, width.fill, height.fill);
    let position = crate::surface::placement::centered_position(
        &bar.edge,
        tree.anchor,
        crate::surface::placement::Size {
            width: width.value,
            height: height.value,
        },
        crate::surface::placement::Size {
            width: tree.screen_size.0,
            height: tree.screen_size.1,
        },
        crate::surface::placement::Point {
            x: surface_origin.0,
            y: surface_origin.1,
        },
        bar.collapsed_width,
        0,
    );
    let margin = panel_layer_margin(&bar.edge, position.layer_margin(), width, height);

    NewLayerShellSettings {
        anchor,
        exclusive_zone: Some(-1),
        size: Some((width.layer_size(), height.layer_size())),
        margin: Some(margin),
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
        output_option: if tree.origin_output_name.is_empty() {
            OutputOption::None
        } else {
            OutputOption::OutputName(tree.origin_output_name.clone())
        },
        namespace: Some(App::namespace()),
        ..Default::default()
    }
}

fn resolve_panel_dimension(
    exact: Option<u32>,
    max: Option<u32>,
    preferred: Option<u32>,
    fallback: u32,
    fill_requested: bool,
    span: u32,
) -> ResolvedPanelDimension {
    if let Some(exact) = exact {
        return ResolvedPanelDimension::exact(exact);
    }

    if fill_requested {
        if let Some(max) = max
            && (span == 0 || max < span)
        {
            return ResolvedPanelDimension::exact(max);
        }
        return ResolvedPanelDimension::fill(span);
    }

    let size = preferred.unwrap_or(fallback);
    ResolvedPanelDimension::exact(max.map(|max| size.min(max)).unwrap_or(size))
}

fn panel_layer_anchor(edge: &crate::config::Edge, fill_width: bool, fill_height: bool) -> Anchor {
    match (edge, fill_width, fill_height) {
        (crate::config::Edge::Top, false, false) => Anchor::Top | Anchor::Left,
        (crate::config::Edge::Top, true, false) => Anchor::Top | Anchor::Left | Anchor::Right,
        (crate::config::Edge::Top, false, true) => Anchor::Top | Anchor::Bottom | Anchor::Left,
        (crate::config::Edge::Top, true, true) => {
            Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right
        }
        (crate::config::Edge::Bottom, false, false) => Anchor::Bottom | Anchor::Left,
        (crate::config::Edge::Bottom, true, false) => Anchor::Bottom | Anchor::Left | Anchor::Right,
        (crate::config::Edge::Bottom, false, true) => Anchor::Top | Anchor::Bottom | Anchor::Left,
        (crate::config::Edge::Bottom, true, true) => {
            Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right
        }
        (crate::config::Edge::Left, false, false) => Anchor::Left | Anchor::Top,
        (crate::config::Edge::Left, true, false) => Anchor::Left | Anchor::Right | Anchor::Top,
        (crate::config::Edge::Left, false, true) => Anchor::Left | Anchor::Top | Anchor::Bottom,
        (crate::config::Edge::Left, true, true) => {
            Anchor::Left | Anchor::Right | Anchor::Top | Anchor::Bottom
        }
        (crate::config::Edge::Right, false, false) => Anchor::Right | Anchor::Top,
        (crate::config::Edge::Right, true, false) => Anchor::Left | Anchor::Right | Anchor::Top,
        (crate::config::Edge::Right, false, true) => Anchor::Right | Anchor::Top | Anchor::Bottom,
        (crate::config::Edge::Right, true, true) => {
            Anchor::Left | Anchor::Right | Anchor::Top | Anchor::Bottom
        }
    }
}

fn panel_layer_margin(
    edge: &crate::config::Edge,
    margin: (i32, i32, i32, i32),
    width: ResolvedPanelDimension,
    height: ResolvedPanelDimension,
) -> (i32, i32, i32, i32) {
    let (top, mut right, mut bottom, mut left) = margin;
    if width.fill && matches!(edge, crate::config::Edge::Top | crate::config::Edge::Bottom) {
        right = 0;
        left = 0;
    }
    if height.fill && matches!(edge, crate::config::Edge::Left | crate::config::Edge::Right) {
        bottom = 0;
    }
    (top, right, bottom, left)
}

fn panel_module_entry_size(
    content: crate::surface::panel::PanelPreferredSize,
) -> crate::surface::panel::PanelPreferredSize {
    crate::surface::panel::PanelPreferredSize {
        width: content.width + PANEL_MODULE_FRAME_H_PADDING,
        height: content.height + PANEL_MODULE_FRAME_V_PADDING,
    }
}

fn panel_module_window_size(
    content: crate::surface::panel::PanelPreferredSize,
) -> crate::surface::panel::PanelPreferredSize {
    panel_module_list_window_size(&[panel_module_entry_size(content)])
        .expect("single module detail produces one panel entry")
}

fn panel_module_list_window_size(
    entries: &[crate::surface::panel::PanelPreferredSize],
) -> Option<crate::surface::panel::PanelPreferredSize> {
    if entries.is_empty() {
        return None;
    }

    let width = entries.iter().map(|entry| entry.width).max().unwrap_or(0) + PANEL_OUTER_PADDING;
    let height = entries.iter().map(|entry| entry.height).sum::<u32>()
        + PANEL_ENTRY_SPACING * entries.len().saturating_sub(1) as u32
        + PANEL_OUTER_PADDING;

    Some(crate::surface::panel::PanelPreferredSize { width, height })
}

fn panel_menu_preferred_size(
    menu: &super::foreground::PanelMenuRenderState,
) -> crate::surface::panel::PanelPreferredSize {
    let prompt_chars = menu.prompt.as_deref().unwrap_or("Search").chars().count() as u32;
    let max_entry_chars = menu
        .entries
        .iter()
        .map(|entry| entry.label.chars().count() as u32)
        .max()
        .unwrap_or(0);
    let icon_width = if menu.show_icons {
        PANEL_MENU_ICON_SLOT
    } else {
        0
    };
    let submenu_width = if menu.entries.iter().any(|entry| entry.is_submenu) {
        PANEL_MENU_SUBMENU_SLOT
    } else {
        0
    };
    let row_width = max_entry_chars * PANEL_MENU_CHAR_WIDTH
        + PANEL_MENU_ROW_H_PADDING
        + PANEL_MENU_LIST_PADDING
        + icon_width
        + submenu_width;
    let search_width =
        prompt_chars * PANEL_MENU_CHAR_WIDTH + PANEL_MENU_ROW_H_PADDING + PANEL_MENU_LIST_PADDING;
    let width = row_width.max(search_width).max(PANEL_MENU_MIN_WIDTH);
    let row_extent = MENU_ROW_EXTENT.ceil() as u32;
    let height =
        PANEL_MENU_SEARCH_HEIGHT + PANEL_MENU_LIST_PADDING + row_extent * menu.entries.len() as u32;

    crate::surface::panel::PanelPreferredSize { width, height }
}

impl App {
    pub(super) fn suspend_bar_keyboard_interactivity(&mut self) -> Vec<Task<Message>> {
        let ids: Vec<iced::window::Id> = self.surfaces.keys().copied().collect();
        ids.into_iter()
            .map(|id| {
                Task::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::None,
                })
            })
            .collect()
    }

    pub(super) fn dispatch_panel_action(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        action: crate::config::PanelAction,
    ) -> Task<Message> {
        match action {
            crate::config::PanelAction::Command(command) => {
                Task::batch(self.apply_app_effects(vec![AppEffect::RunConfiguredCommand(command)]))
            }
            action => match panel_event_for_action(origin_window, origin, action, None) {
                Some(event) => self.handle_panel_event(event),
                None => Task::none(),
            },
        }
    }

    pub(super) fn close_active_panel(&mut self) -> Vec<Task<Message>> {
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::ClosePanel);
        self.apply_foreground_effects(effects)
    }

    pub(super) fn handle_panel_focus_gained(&mut self, window: iced::window::Id) -> Task<Message> {
        if self.foreground.is_menu_panel_window(window) {
            let effects = self
                .foreground
                .handle(super::foreground::ForegroundIntent::PanelRequestSearchFocus);
            return Task::batch(self.apply_foreground_effects(effects));
        }
        Task::none()
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
                let request = self.panel_open_request(origin_window, origin, content, anchor);
                let module_detail_sessions = self.module_detail_sessions_for_request(&request);
                let context = super::foreground::PanelOpenContext::new(
                    self.config.menus.as_slice(),
                    module_detail_sessions,
                );
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::TogglePanel { request, context });
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::Close => Task::batch(self.close_active_panel()),
            PanelEvent::SearchChanged(query) => {
                let effects = self.foreground.handle(
                    super::foreground::ForegroundIntent::PanelSearchChanged(query),
                );
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::MenuScrolled { y, viewport_height } => {
                let effects = self.foreground.handle(
                    super::foreground::ForegroundIntent::PanelMenuScrolled { y, viewport_height },
                );
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::FocusMoveNext => self.move_panel_focus(1),
            PanelEvent::FocusMovePrev => self.move_panel_focus(-1),
            PanelEvent::FocusIndex(index) => {
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::PanelFocusIndex(index));
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::FocusSearchInput { retries } => self.focus_panel_search_input(retries),
            PanelEvent::Activate => self.activate_panel_entry(None),
            PanelEvent::ActivateIndex(index) => self.activate_panel_entry(Some(index)),
            PanelEvent::Back => {
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::PanelBack);
                Task::batch(self.apply_foreground_effects(effects))
            }
        }
    }

    fn open_panel(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
    ) -> Task<Message> {
        let request = self.panel_open_request(origin_window, origin, content, anchor);
        let module_detail_sessions = self.module_detail_sessions_for_request(&request);
        let context = super::foreground::PanelOpenContext::new(
            self.config.menus.as_slice(),
            module_detail_sessions,
        );
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::OpenPanel { request, context });
        Task::batch(self.apply_foreground_effects(effects))
    }

    pub(super) fn panel_open_request(
        &self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
    ) -> super::foreground::PanelForegroundRequest {
        let origin_surface = self.surface(origin_window).cloned().unwrap_or_default();
        super::foreground::PanelForegroundRequest {
            origin_window,
            origin,
            content,
            anchor,
            origin_bar_name: origin_surface.bar_name,
            origin_output_name: origin_surface.output_name,
            screen_size: origin_surface.screen_size,
        }
    }

    #[cfg(test)]
    pub(super) fn prepare_panel_for_test(
        &mut self,
        request: super::foreground::PanelForegroundRequest,
    ) -> Vec<super::foreground::ForegroundEffect> {
        let module_detail_sessions = self.module_detail_sessions_for_request(&request);
        let context = super::foreground::PanelOpenContext::new(
            self.config.menus.as_slice(),
            module_detail_sessions,
        );
        self.foreground
            .handle(super::foreground::ForegroundIntent::OpenPanel { request, context })
    }

    pub(super) fn module_detail_sessions_for_request(
        &self,
        request: &super::foreground::PanelForegroundRequest,
    ) -> std::collections::HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession>
    {
        self.panel_module_layout_for_request(request)
            .into_iter()
            .filter_map(|entry| {
                self.module_detail_session_for(&entry.id)
                    .map(|session| (entry.id, session))
            })
            .collect()
    }

    fn module_detail_session_for(
        &self,
        id: &crate::modules::ModuleId,
    ) -> Option<crate::modules::ModuleDetailSession> {
        self.modules.create_module_detail_session(id)
    }

    pub(super) fn panel_module_layout_for_request(
        &self,
        request: &super::foreground::PanelForegroundRequest,
    ) -> Vec<crate::surface::panel::PanelModuleEntry> {
        self.panel_module_layout_for_content(
            request.origin_window,
            &request.origin,
            &request.content,
        )
    }

    pub(super) fn panel_module_layout_for_content(
        &self,
        origin_window: iced::window::Id,
        origin: &crate::surface::panel::PanelOrigin,
        content: &crate::surface::panel::PanelContent,
    ) -> Vec<crate::surface::panel::PanelModuleEntry> {
        match content {
            crate::surface::panel::PanelContent::BarPanel => {
                let origin_surface = self.surface(origin_window).cloned().unwrap_or_default();
                let bar = Self::bar_for_surface_name(&self.config.bars, &origin_surface.bar_name);
                self.effective_panel_module_layout_for_bar(bar)
                    .into_iter()
                    .map(|(zone, id)| {
                        let focused =
                            origin == &crate::surface::panel::PanelOrigin::Module(id.clone());
                        crate::surface::panel::PanelModuleEntry { id, zone, focused }
                    })
                    .collect()
            }
            crate::surface::panel::PanelContent::ModulePanel(id) => {
                vec![crate::surface::panel::PanelModuleEntry {
                    id: id.clone(),
                    zone: crate::modules::Zone::Center,
                    focused: origin == &crate::surface::panel::PanelOrigin::Module(id.clone()),
                }]
            }
            crate::surface::panel::PanelContent::Menu(_)
            | crate::surface::panel::PanelContent::DmenuRequest => Vec::new(),
        }
    }

    fn panel_content_size_hint(
        &self,
        tree: &crate::surface::panel::PanelTree,
        bar: &crate::config::BarConfig,
        origin_surface: &crate::surface::SurfaceState,
    ) -> PanelContentSizeHint {
        match &tree.content {
            crate::surface::panel::PanelContent::Menu(_)
            | crate::surface::panel::PanelContent::DmenuRequest => {
                let menu = self.foreground.active_panel_menu_render_state();
                let preferred = panel_menu_preferred_size(&menu);
                PanelContentSizeHint {
                    preferred_width: Some(preferred.width),
                    preferred_height: Some(preferred.height),
                    fill_edge_span: true,
                }
            }
            crate::surface::panel::PanelContent::ModulePanel(id) => self
                .module_panel_preferred_size(id, bar, origin_surface)
                .map(|preferred| PanelContentSizeHint {
                    preferred_width: Some(preferred.width),
                    preferred_height: Some(preferred.height),
                    fill_edge_span: false,
                })
                .unwrap_or_default(),
            crate::surface::panel::PanelContent::BarPanel => {
                let layout = self.panel_module_layout_for_content(
                    tree.origin_window,
                    &tree.origin,
                    &tree.content,
                );
                let mut hint = self
                    .bar_panel_preferred_size(layout, bar, origin_surface)
                    .map(|preferred| PanelContentSizeHint {
                        preferred_width: Some(preferred.width),
                        preferred_height: Some(preferred.height),
                        fill_edge_span: true,
                    })
                    .unwrap_or(PanelContentSizeHint {
                        fill_edge_span: true,
                        ..Default::default()
                    });
                hint.fill_edge_span = true;
                hint
            }
        }
    }

    fn module_panel_preferred_size(
        &self,
        id: &crate::modules::ModuleId,
        bar: &crate::config::BarConfig,
        origin_surface: &crate::surface::SurfaceState,
    ) -> Option<crate::surface::panel::PanelPreferredSize> {
        self.module_detail_preferred_content_size(id, bar, origin_surface)
            .map(panel_module_window_size)
    }

    fn bar_panel_preferred_size(
        &self,
        layout: Vec<crate::surface::panel::PanelModuleEntry>,
        bar: &crate::config::BarConfig,
        origin_surface: &crate::surface::SurfaceState,
    ) -> Option<crate::surface::panel::PanelPreferredSize> {
        let mut entries = Vec::new();
        for entry in layout {
            let content_size =
                self.module_detail_preferred_content_size(&entry.id, bar, origin_surface)?;
            entries.push(panel_module_entry_size(content_size));
        }
        panel_module_list_window_size(&entries)
    }

    fn module_detail_preferred_content_size(
        &self,
        id: &crate::modules::ModuleId,
        bar: &crate::config::BarConfig,
        origin_surface: &crate::surface::SurfaceState,
    ) -> Option<crate::surface::panel::PanelPreferredSize> {
        let kind = self.modules.kind_for(id)?;
        let state = self.module_render_state(kind, id, origin_surface);
        self.modules
            .module_detail_preferred_size(super::module_runtime::ModuleDetailSizeRequest {
                id: id.clone(),
                style: &bar.style,
                session: self.foreground.active_panel_module_detail_session(id),
                state_override: Some(state),
            })
    }

    pub(super) fn open_panel_window(
        &mut self,
        tree: crate::surface::panel::PanelTree,
    ) -> Task<Message> {
        let origin_surface = self
            .surface(tree.origin_window)
            .cloned()
            .unwrap_or_default();
        let bar = Self::bar_for_surface_name(&self.config.bars, &tree.origin_bar_name);
        let surface_origin = self.inferred_panel_surface_origin(&origin_surface, bar);
        let hint = self.panel_content_size_hint(&tree, bar, &origin_surface);
        let settings = panel_window_settings(bar, &tree, surface_origin, hint);
        let (panel_window, open_task) = Message::layershell_open(settings);
        self.surface_mut(tree.origin_window).tooltip_pending = None;
        tracing::debug!(
            target: "niribar::panel",
            origin = panel_origin_label(&tree.origin),
            content = panel_content_label(&tree.content),
            origin_bar = tree.origin_bar_name.as_str(),
            origin_output = tree.origin_output_name.as_str(),
            has_anchor = tree.anchor.is_some(),
            ?panel_window,
            "opening panel window"
        );

        let mut tasks = Vec::new();
        self.foreground
            .handle(super::foreground::ForegroundIntent::PanelWindowOpened {
                window: panel_window,
            });

        let open_sequence = open_task.chain(iced::window::gain_focus(panel_window));
        tasks.push(open_sequence);
        Task::batch(tasks)
    }

    pub(super) fn load_installed_apps_for_panel(
        &self,
    ) -> Vec<crate::desktop_apps::DesktopApplication> {
        #[cfg(test)]
        if let Some(apps) = self.foreground.installed_apps_for_test() {
            return apps.clone();
        }
        crate::desktop_apps::discover_installed_applications().unwrap_or_default()
    }

    fn inferred_panel_surface_origin(
        &self,
        origin_surface: &crate::surface::SurfaceState,
        bar: &crate::config::BarConfig,
    ) -> (i32, i32) {
        if origin_surface.output_name.is_empty() {
            return (0, 0);
        }

        let top_inset = match bar.edge {
            crate::config::Edge::Left | crate::config::Edge::Right => {
                self.config
                    .bars_for_output(&origin_surface.output_name)
                    .into_iter()
                    .filter(|candidate| matches!(candidate.edge, crate::config::Edge::Top))
                    .map(|candidate| candidate.collapsed_width)
                    .sum::<u32>() as i32
            }
            crate::config::Edge::Top | crate::config::Edge::Bottom => 0,
        };

        (0, top_inset)
    }

    pub(super) fn handle_panel_escape_pressed(&mut self) -> Task<Message> {
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::PanelEscape);
        Task::batch(self.apply_foreground_effects(effects))
    }

    fn move_panel_focus(&mut self, delta: isize) -> Task<Message> {
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::PanelFocusMove {
                delta,
                scroll: super::foreground::PanelMenuScrollContext {
                    row_extent: MENU_ROW_EXTENT,
                    default_viewport_height: DEFAULT_MENU_VIEWPORT_HEIGHT,
                },
            });
        Task::batch(self.apply_foreground_effects(effects))
    }

    fn activate_panel_entry(&mut self, index: Option<usize>) -> Task<Message> {
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::PanelActivate(index));
        Task::batch(self.apply_foreground_effects(effects))
    }

    fn focus_panel_search_input(&mut self, retries: u8) -> Task<Message> {
        if !self.foreground.panel_search_focus_pending() {
            return Task::none();
        }
        let Some(panel_window) = self.foreground.active_panel_window() else {
            self.foreground
                .handle(super::foreground::ForegroundIntent::PanelSearchFocusSettled);
            return Task::none();
        };
        let search_input_id = panel_search_input_id(panel_window);
        let mut tasks = vec![iced::widget::operation::focus(search_input_id)];
        if retries > 0 {
            tasks.push(Task::perform(
                async {
                    tokio::time::sleep(PANEL_SEARCH_FOCUS_RETRY_DELAY).await;
                },
                move |_| {
                    Message::Panel(super::messages::PanelEvent::FocusSearchInput {
                        retries: retries - 1,
                    })
                },
            ));
        } else {
            self.foreground
                .handle(super::foreground::ForegroundIntent::PanelSearchFocusSettled);
        }
        Task::batch(tasks)
    }
}

impl App {
    pub(super) fn handle_dmenu_selection_request(
        &mut self,
        request: crate::ipc::protocol::DmenuRequest,
        responder: super::messages::IpcResponder,
    ) -> Task<Message> {
        let panel_request = self.default_panel_origin_window().map(|origin_window| {
            self.panel_open_request(
                origin_window,
                crate::surface::panel::PanelOrigin::Bar,
                crate::surface::panel::PanelContent::DmenuRequest,
                None,
            )
        });
        let effects =
            self.foreground
                .handle(super::foreground::ForegroundIntent::BeginDmenuSelection {
                    request,
                    responder,
                    panel_request,
                });
        Task::batch(self.apply_foreground_effects(effects))
    }

    fn default_panel_origin_window(&self) -> Option<iced::window::Id> {
        self.surfaces.keys().copied().next()
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
    use super::super::panel_menu_session::{
        PanelMenuEntryTarget as MenuEntryTarget, dmenu_entry_views, search_menu_entries,
    };
    use super::*;

    fn panel_position_for_test(
        edge: crate::config::Edge,
        bar_size: u32,
        panel_size: (u32, u32),
        anchor: Option<iced::Rectangle>,
        screen_size: (u32, u32),
        surface_origin: (i32, i32),
    ) -> (i32, i32) {
        let placement = crate::surface::placement::centered_position(
            &edge,
            anchor,
            crate::surface::placement::Size {
                width: panel_size.0,
                height: panel_size.1,
            },
            crate::surface::placement::Size {
                width: screen_size.0,
                height: screen_size.1,
            },
            crate::surface::placement::Point {
                x: surface_origin.0,
                y: surface_origin.1,
            },
            bar_size,
            0,
        );
        (placement.position.x, placement.position.y)
    }

    fn active_menu_entries(app: &App) -> Vec<super::super::panel_menu_session::PanelMenuEntry> {
        app.foreground.active_panel_menu_entries_for_test()
    }

    #[test]
    fn dmenu_entries_use_input_as_label_and_value() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: Some("Run".into()),
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
                crate::ipc::protocol::DmenuEntry::plain("kitty"),
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
            ],
            allow_custom: true,
        };
        let entries = dmenu_entry_views(&request, "");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["firefox", "kitty", "firefox"]
        );
        assert!(matches!(entries[0].target, MenuEntryTarget::DmenuValue(_)));
    }

    #[test]
    fn dmenu_search_filters_entries_case_insensitively() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("Firefox"),
                crate::ipc::protocol::DmenuEntry::plain("Kitty"),
            ],
            allow_custom: true,
        };
        let entries = dmenu_entry_views(&request, "fire");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Firefox");
        assert_eq!(entries[1].label, "fire");
    }

    #[test]
    fn dmenu_custom_value_entry_appears_when_query_has_no_exact_match() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
            allow_custom: true,
        };
        let entries = dmenu_entry_views(&request, "kitty");

        assert_eq!(entries[0].label, "kitty");
        assert!(matches!(
            entries[0].target,
            MenuEntryTarget::DmenuCustomValue(_)
        ));
    }

    #[test]
    fn dmenu_no_custom_does_not_create_custom_entry() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
            allow_custom: false,
        };
        let entries = dmenu_entry_views(&request, "kitty");

        assert!(entries.is_empty());
    }

    #[test]
    fn dmenu_rich_entries_use_label_icon_keywords_and_value() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: Some("Run".into()),
            entries: vec![
                crate::ipc::protocol::DmenuEntry {
                    label: Some("Firefox".into()),
                    value: Some("org.mozilla.firefox".into()),
                    icon: Some("firefox".into()),
                    keywords: vec!["browser".into(), "web".into()],
                },
                crate::ipc::protocol::DmenuEntry {
                    label: Some("Kitty".into()),
                    value: Some("kitty".into()),
                    icon: None,
                    keywords: Vec::new(),
                },
            ],
            allow_custom: true,
        };

        let entries = dmenu_entry_views(&request, "browser");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Firefox");
        assert_eq!(entries[0].icon.as_deref(), Some("firefox"));
        assert_eq!(
            entries[0].target,
            MenuEntryTarget::DmenuValue("org.mozilla.firefox".into())
        );
        assert_eq!(entries[1].label, "browser");
        assert!(matches!(
            entries[1].target,
            MenuEntryTarget::DmenuCustomValue(_)
        ));
    }

    #[test]
    fn dmenu_search_uses_fuzzy_ranking() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("Telemetry Renderer Module"),
                crate::ipc::protocol::DmenuEntry::plain("Terminal"),
            ],
            allow_custom: false,
        };

        let entries = dmenu_entry_views(&request, "term");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Terminal", "Telemetry Renderer Module"]
        );
    }

    #[test]
    fn dmenu_custom_value_appears_after_fuzzy_matches() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("Firefox")],
            allow_custom: true,
        };

        let entries = dmenu_entry_views(&request, "firfox");

        assert_eq!(entries[0].label, "Firefox");
        assert_eq!(entries[1].label, "firfox");
        assert!(matches!(
            entries[1].target,
            MenuEntryTarget::DmenuCustomValue(_)
        ));
    }

    #[test]
    fn dmenu_blank_query_preserves_duplicate_source_order() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
                crate::ipc::protocol::DmenuEntry::plain("kitty"),
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
            ],
            allow_custom: true,
        };

        let entries = dmenu_entry_views(&request, " ");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["firefox", "kitty", "firefox"]
        );
    }

    #[test]
    fn dmenu_rich_search_does_not_match_opaque_value_when_label_exists() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry {
                label: Some("Firefox".into()),
                value: Some("org.mozilla.firefox".into()),
                icon: None,
                keywords: Vec::new(),
            }],
            allow_custom: false,
        };

        let entries = dmenu_entry_views(&request, "mozilla");

        assert!(entries.is_empty());
    }

    #[test]
    fn dmenu_value_only_rich_entry_matches_and_displays_value() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry {
                label: None,
                value: Some("org.mozilla.firefox".into()),
                icon: None,
                keywords: Vec::new(),
            }],
            allow_custom: false,
        };

        let entries = dmenu_entry_views(&request, "mozilla");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "org.mozilla.firefox");
    }

    #[test]
    fn dmenu_request_rejects_when_another_request_is_pending() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let (active_tx, _active_rx) = tokio::sync::oneshot::channel();
        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("active")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(active_tx),
        });
        assert!(app.foreground.has_dmenu_responder_for_test());
        let (tx, mut rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("x")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });

        let response = rx.try_recv().unwrap();
        assert!(matches!(
            response,
            crate::ipc::protocol::IpcResponse::Rejected { .. }
        ));
    }

    #[test]
    fn dmenu_request_keeps_responder_for_open_selection() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let (tx, _rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });

        assert!(app.foreground.has_dmenu_responder_for_test());
        assert_eq!(active_menu_entries(&app)[0].label, "firefox");
    }

    #[test]
    fn activating_dmenu_selection_sends_selected_response() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let (tx, mut rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });
        let _ = app.update(Message::Panel(
            crate::app::messages::PanelEvent::ActivateIndex(0),
        ));

        assert_eq!(
            rx.try_recv().unwrap(),
            crate::ipc::protocol::IpcResponse::Selected {
                value: "firefox".into()
            }
        );
        assert!(!app.foreground.has_dmenu_responder_for_test());
    }

    #[test]
    fn closing_dmenu_panel_sends_cancelled_response() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let (tx, mut rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });
        let panel = app
            .foreground
            .active_panel_window()
            .expect("dmenu panel window");

        let _ = app.handle_surface_event(crate::app::messages::SurfaceEvent::WindowClosed(panel));

        assert_eq!(
            rx.try_recv().unwrap(),
            crate::ipc::protocol::IpcResponse::Cancelled
        );
        assert!(!app.foreground.has_dmenu_responder_for_test());
    }

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
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            anchor: None,
        }));
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );
    }

    #[test]
    fn opening_panel_clears_origin_tooltip_pending() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin).tooltip_pending =
            Some((crate::modules::ModuleId::new("clock"), 0.0));

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new(
                "clock",
            )),
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            anchor: None,
        }));

        assert!(app.surface(origin).unwrap().tooltip_pending.is_none());
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

        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            0
        );
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
            (0, 0),
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
            (1872, 0),
        );

        assert_eq!(position.0, 1512);
        assert!(position.1 >= 0);
    }

    #[test]
    fn panel_window_uses_on_demand_keyboard_interactivity() {
        let bar = crate::config::BarConfig {
            edge: crate::config::Edge::Top,
            ..crate::config::BarConfig::default()
        };
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            origin_window: iced::window::Id::unique(),
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: Some(iced::Rectangle {
                x: 100.0,
                y: 0.0,
                width: 48.0,
                height: 32.0,
            }),
            screen_size: (1920, 48),
        };

        assert_eq!(
            panel_window_settings(&bar, &tree, (0, 0), PanelContentSizeHint::default())
                .keyboard_interactivity,
            KeyboardInteractivity::OnDemand
        );
    }

    #[test]
    fn panel_window_settings_keep_layer_shell_focusable_for_search_input() {
        let bar = crate::config::BarConfig {
            edge: crate::config::Edge::Top,
            ..crate::config::BarConfig::default()
        };
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            origin_window: iced::window::Id::unique(),
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 48),
        };

        let settings = panel_window_settings(
            &bar,
            &tree,
            (0, 0),
            PanelContentSizeHint {
                preferred_width: Some(300),
                preferred_height: Some(400),
                fill_edge_span: true,
            },
        );

        assert_eq!(settings.size, Some((0, 400)));
        assert!(settings.anchor.contains(Anchor::Left));
        assert!(settings.anchor.contains(Anchor::Right));
        assert_eq!(
            settings.keyboard_interactivity,
            KeyboardInteractivity::OnDemand
        );
    }

    #[test]
    fn bottom_panel_window_uses_layer_margin_above_bar() {
        let bar = crate::config::BarConfig {
            edge: crate::config::Edge::Bottom,
            ..crate::config::BarConfig::default()
        };
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            origin_window: iced::window::Id::unique(),
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: Some(iced::Rectangle {
                x: 100.0,
                y: 0.0,
                width: 48.0,
                height: 48.0,
            }),
            screen_size: (1920, 48),
        };

        let settings = panel_window_settings(
            &bar,
            &tree,
            (0, 0),
            PanelContentSizeHint {
                preferred_width: Some(300),
                preferred_height: Some(240),
                fill_edge_span: true,
            },
        );

        assert_eq!(settings.size, Some((0, 240)));
        assert_eq!(settings.margin, Some((0, 0, 48, 0)));
    }

    #[test]
    fn left_edge_panel_menu_fills_edge_span_height() {
        let bar = crate::config::BarConfig {
            edge: crate::config::Edge::Left,
            ..crate::config::BarConfig::default()
        };
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            origin_window: iced::window::Id::unique(),
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (48, 1080),
        };

        let settings = panel_window_settings(
            &bar,
            &tree,
            (0, 0),
            PanelContentSizeHint {
                preferred_width: Some(320),
                preferred_height: Some(400),
                fill_edge_span: true,
            },
        );

        assert_eq!(settings.size, Some((320, 0)));
        assert!(settings.anchor.contains(Anchor::Top));
        assert!(settings.anchor.contains(Anchor::Bottom));
        assert_eq!(settings.margin, Some((0, 0, 0, 48)));
    }

    #[test]
    fn exact_panel_dimension_overrides_fill_sizing() {
        let mut bar = crate::config::BarConfig {
            edge: crate::config::Edge::Top,
            ..crate::config::BarConfig::default()
        };
        bar.panel.width = Some(500);
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            origin_window: iced::window::Id::unique(),
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 48),
        };

        let settings = panel_window_settings(
            &bar,
            &tree,
            (0, 0),
            PanelContentSizeHint {
                preferred_width: Some(320),
                preferred_height: Some(400),
                fill_edge_span: true,
            },
        );

        assert_eq!(settings.size, Some((500, 400)));
        assert!(!settings.anchor.contains(Anchor::Right));
    }

    #[test]
    fn max_panel_dimension_caps_fill_sizing() {
        let mut bar = crate::config::BarConfig {
            edge: crate::config::Edge::Top,
            ..crate::config::BarConfig::default()
        };
        bar.panel.max_width = Some(800);
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            origin_window: iced::window::Id::unique(),
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 48),
        };

        let settings = panel_window_settings(
            &bar,
            &tree,
            (0, 0),
            PanelContentSizeHint {
                preferred_width: Some(320),
                preferred_height: Some(400),
                fill_edge_span: true,
            },
        );

        assert_eq!(settings.size, Some((800, 400)));
        assert!(!settings.anchor.contains(Anchor::Right));
    }

    #[test]
    fn clock_module_panel_uses_module_preferred_size() {
        let mut config = crate::config::Config::default();
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        {
            let surface = app.surface_mut(origin);
            surface.bar_name = "main".into();
            surface.output_name = "DP-1".into();
            surface.screen_size = (1920, 48);
        }
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new(
                "clock",
            )),
            content: crate::surface::panel::PanelContent::ModulePanel(
                crate::modules::ModuleId::new("clock"),
            ),
            origin_window: origin,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 48),
        };
        let bar = app.config.bars[0].clone();
        let origin_surface = app.surface(origin).cloned().unwrap();

        let hint = app.panel_content_size_hint(&tree, &bar, &origin_surface);
        let settings = panel_window_settings(&bar, &tree, (0, 0), hint);

        assert_eq!(hint.preferred_width, Some(336));
        assert_eq!(hint.preferred_height, Some(292));
        assert!(!hint.fill_edge_span);
        assert_eq!(settings.size, Some((336, 292)));
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
            (0, 0),
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
            (0, 0),
        );

        assert!(position.0 >= 0);
        assert_eq!(position.1, 448);
    }

    #[test]
    fn panel_position_top_centers_on_anchor_and_clamps() {
        let position = panel_position_for_test(
            crate::config::Edge::Top,
            48,
            (300, 600),
            Some(iced::Rectangle {
                x: 480.0,
                y: 4.0,
                width: 40.0,
                height: 40.0,
            }),
            (1920, 1080),
            (0, 0),
        );

        assert_eq!(position, (350, 48));
    }

    #[test]
    fn left_edge_bar_panel_starts_at_vertical_bar_top_when_top_bar_reserves_space() {
        let position = panel_position_for_test(
            crate::config::Edge::Left,
            48,
            (360, 600),
            None,
            (48, 1048),
            (0, 32),
        );

        assert_eq!(position, (48, 32));
    }

    #[test]
    fn side_panel_origin_includes_top_bars_on_same_output() {
        let mut config = crate::config::Config::default();
        config.bars[0].name = "top".into();
        config.bars[0].edge = crate::config::Edge::Top;
        config.bars[0].collapsed_width = 32;
        let side = crate::config::BarConfig {
            name: "side".into(),
            edge: crate::config::Edge::Left,
            collapsed_width: 48,
            ..Default::default()
        };
        config.bars.push(side);
        let (app, _) = App::boot(config, vec!["DP-1".into()]);
        let origin_surface = crate::surface::SurfaceState::new("DP-1".into(), "side".into());
        let bar = App::bar_for_surface_name(&app.config.bars, "side");

        assert_eq!(
            app.inferred_panel_surface_origin(&origin_surface, bar),
            (0, 32)
        );
    }

    #[test]
    fn configured_module_open_panel_action_opens_panel() {
        let mut config = crate::config::Config {
            modules: vec![crate::config::ModuleConfig {
                name: "button".into(),
                module_type: "button".into(),
                params: crate::config::ModuleParams::Button(crate::config::ButtonConfig {
                    label: "Launch".into(),
                    icon: ">".into(),
                    format: crate::config::FormatConfig::default(),
                    content_rotation: crate::config::ContentRotation::None,
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
        let id = crate::modules::ModuleId::new("button");
        let action = app
            .modules
            .action_for(&id, &crate::config::ActionTrigger::Click)
            .cloned()
            .expect("configured click action");

        let _ = app.dispatch_panel_action(
            origin,
            crate::surface::panel::PanelOrigin::Module(id),
            action,
        );

        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
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

    fn menu_with_apps_browser() -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "main".into(),
            show_icons: false,
            prompt: None,
            source: None,
            items: vec![crate::config::MenuItemConfig {
                label: "Apps".into(),
                icon: Some("applications-other".into()),
                keywords: vec![],
                action: None,
                children: vec![crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    icon: Some("firefox".into()),
                    keywords: vec!["web".into()],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::ConfiguredCommand::shell_for_test("firefox"),
                    )),
                    children: vec![],
                }],
            }],
        }
    }

    fn menu_with_two_entries() -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "main".into(),
            show_icons: false,
            prompt: None,
            source: None,
            items: vec![
                crate::config::MenuItemConfig {
                    label: "Terminal".into(),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![],
                },
                crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![],
                },
            ],
        }
    }

    fn menu_with_entries(count: usize) -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "main".into(),
            show_icons: false,
            prompt: None,
            source: None,
            items: (0..count)
                .map(|index| crate::config::MenuItemConfig {
                    label: format!("Entry {index}"),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![],
                })
                .collect(),
        }
    }

    fn config_with_menu(menu: crate::config::MenuConfig) -> crate::config::Config {
        crate::config::Config {
            menus: vec![menu],
            ..crate::config::Config::default()
        }
    }

    fn prepare_panel_content(
        app: &mut App,
        origin: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) {
        let request = app.panel_open_request(
            origin,
            crate::surface::panel::PanelOrigin::Bar,
            content,
            None,
        );
        let effects = app.prepare_panel_for_test(request);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            super::super::foreground::ForegroundEffect::OpenPanelWindow { .. }
        )));
    }

    fn register_panel_with_content(
        app: &mut App,
        panel: iced::window::Id,
        origin: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) {
        prepare_panel_content(app, origin, content);
        app.register_panel_popup(panel, origin);
    }

    fn register_menu_panel(app: &mut App, panel: iced::window::Id, origin: iced::window::Id) {
        if app.foreground.panel_menu_snapshot_for_test().is_none() {
            prepare_panel_content(
                app,
                origin,
                crate::surface::panel::PanelContent::Menu("main".into()),
            );
        }
        app.register_panel_popup(panel, origin);
    }

    fn set_panel_search(app: &mut App, query: &str) {
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::PanelSearchChanged(query.into()));
    }

    fn set_panel_focus(app: &mut App, index: usize) {
        if app.foreground.panel_menu_snapshot_for_test().is_none() {
            let origin = iced::window::Id::unique();
            prepare_panel_content(
                app,
                origin,
                crate::surface::panel::PanelContent::Menu("main".into()),
            );
        }
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::PanelFocusIndex(
                index,
            ));
    }

    fn panel_focus(app: &App) -> Option<usize> {
        app.foreground.focused_panel_index_for_test()
    }

    #[test]
    fn menu_search_flattens_nested_matches_with_breadcrumb() {
        let menu = menu_with_apps_browser();
        let results = search_menu_entries(&menu, "browser");

        assert_eq!(results[0].label, "Apps / Browser");
        assert_eq!(results[0].path, vec![0, 0]);
        assert_eq!(results[0].icon, Some("firefox".into()));
    }

    #[test]
    fn menu_search_matches_keywords_case_insensitively() {
        let menu = menu_with_apps_browser();
        let results = search_menu_entries(&menu, "WEB");

        assert_eq!(results[0].label, "Apps / Browser");
    }

    #[test]
    fn menu_search_ranks_label_matches_before_keyword_matches() {
        let mut menu = menu_with_apps_browser();
        menu.items.push(crate::config::MenuItemConfig {
            label: "Settings".into(),
            icon: None,
            keywords: vec!["browser".into()],
            action: Some(crate::config::PanelAction::Command(
                crate::config::ConfiguredCommand::shell_for_test("settings"),
            )),
            children: vec![],
        });

        let results = search_menu_entries(&menu, "browser");

        assert_eq!(results[0].label, "Apps / Browser");
        assert_eq!(results[1].label, "Settings");
    }

    #[test]
    fn menu_search_uses_ordered_fuzzy_matching() {
        let menu = menu_with_apps_browser();

        let results = search_menu_entries(&menu, "brsr");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "Apps / Browser");
        assert!(search_menu_entries(&menu, "rb").is_empty());
    }

    #[test]
    fn menu_search_matches_breadcrumb_as_lowest_priority() {
        let menu = crate::config::MenuConfig {
            name: "main".into(),
            show_icons: false,
            prompt: None,
            source: None,
            items: vec![
                crate::config::MenuItemConfig {
                    label: "Apps".into(),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![crate::config::MenuItemConfig {
                        label: "Firefox".into(),
                        icon: None,
                        keywords: vec![],
                        action: Some(crate::config::PanelAction::Command(
                            crate::config::ConfiguredCommand::shell_for_test("firefox"),
                        )),
                        children: vec![],
                    }],
                },
                crate::config::MenuItemConfig {
                    label: "App Settings".into(),
                    icon: None,
                    keywords: vec![],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::ConfiguredCommand::shell_for_test("settings"),
                    )),
                    children: vec![],
                },
            ],
        };

        let results = search_menu_entries(&menu, "app");

        assert_eq!(
            results
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["App Settings", "Apps", "Apps / Firefox"]
        );
    }

    #[test]
    fn menu_search_requires_every_query_token() {
        let menu = menu_with_apps_browser();

        assert_eq!(search_menu_entries(&menu, "apps browser").len(), 1);
        assert!(search_menu_entries(&menu, "apps terminal").is_empty());
    }

    fn installed_app(name: &str, id: &str) -> crate::desktop_apps::DesktopApplication {
        crate::desktop_apps::DesktopApplication {
            id: id.into(),
            name: name.into(),
            generic_name: None,
            comment: None,
            keywords: vec![],
            icon: None,
            exec: name.to_ascii_lowercase(),
        }
    }

    fn app_with_installed_apps_menu(apps: Vec<crate::desktop_apps::DesktopApplication>) -> App {
        let config = config_with_menu(crate::config::MenuConfig {
            name: "apps".into(),
            show_icons: false,
            prompt: None,
            source: Some(crate::config::MenuSourceConfig {
                kind: crate::config::MenuSourceKind::InstalledApps,
            }),
            items: vec![],
        });
        let (mut app, _) = App::boot(config, vec![]);
        app.foreground.set_installed_apps_for_test(apps);
        app
    }

    #[test]
    fn installed_apps_hover_render_path_does_not_rebuild_large_entry_list() {
        let apps = (0..5_000)
            .map(|index| installed_app(&format!("App {index:04}"), &format!("app-{index}.desktop")))
            .collect::<Vec<_>>();
        let mut app = app_with_installed_apps_menu(apps);
        open_apps_menu_panel(&mut app);
        let panel_window = app.foreground.active_panel_window().expect("panel window");

        let started = std::time::Instant::now();
        for index in 0..100 {
            let _ = app.update(Message::Panel(
                crate::app::messages::PanelEvent::FocusIndex(index),
            ));
            let model = app
                .panel_render_model(panel_window)
                .expect("panel render model");
            assert_eq!(model.menu_entries.len(), 5_000);
            assert_eq!(model.focused_index, Some(index));
        }

        assert!(
            started.elapsed() < std::time::Duration::from_millis(25),
            "hover render path rebuilt too much installed-app menu state: {:?}",
            started.elapsed()
        );
    }

    fn open_apps_menu_panel(app: &mut App) {
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("apps".into()),
            anchor: None,
        }));
    }

    #[test]
    fn toggle_installed_apps_menu_loads_entries() {
        let mut app = app_with_installed_apps_menu(vec![installed_app("Alpha", "alpha.desktop")]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Toggle {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("apps".into()),
            anchor: None,
        }));

        assert_eq!(
            active_menu_entries(&app)
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha"]
        );
    }

    #[test]
    fn installed_apps_menu_entries_use_open_snapshot() {
        let mut app = app_with_installed_apps_menu(vec![
            installed_app("Alpha", "alpha.desktop"),
            installed_app("Zed", "zed.desktop"),
        ]);

        open_apps_menu_panel(&mut app);

        let entries = active_menu_entries(&app);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha", "Zed"]
        );
    }

    #[test]
    fn installed_apps_search_matches_keywords_and_comment() {
        let mut browser = installed_app("Browser", "browser.desktop");
        browser.comment = Some("Browse the web".into());
        browser.keywords = vec!["internet".into()];
        let mut app =
            app_with_installed_apps_menu(vec![browser, installed_app("Editor", "editor.desktop")]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "internet");

        let entries = active_menu_entries(&app);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Browser");
    }

    #[test]
    fn installed_apps_search_ranks_visible_label_before_metadata() {
        let mut settings = installed_app("Settings", "settings.desktop");
        settings.keywords = vec!["network".into()];
        let mut network = installed_app("Network Connections", "network.desktop");
        network.comment = Some("Configure links".into());
        let mut app = app_with_installed_apps_menu(vec![settings, network]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "net");

        let entries = active_menu_entries(&app);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Network Connections", "Settings"]
        );
    }

    #[test]
    fn installed_apps_search_matches_tokens_across_fields() {
        let mut code = installed_app("Visual Studio Code", "code.desktop");
        code.keywords = vec!["editor".into()];
        let mut app = app_with_installed_apps_menu(vec![code]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "code editor");

        let entries = active_menu_entries(&app);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Visual Studio Code");
    }

    #[test]
    fn installed_apps_search_preserves_source_order_for_blank_query() {
        let mut app = app_with_installed_apps_menu(vec![
            installed_app("Zed", "zed.desktop"),
            installed_app("Alpha", "alpha.desktop"),
        ]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "   ");

        let entries = active_menu_entries(&app);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Zed", "Alpha"]
        );
    }

    #[test]
    fn installed_apps_entries_carry_valid_icons_only() {
        let mut alpha = installed_app("Alpha", "alpha.desktop");
        alpha.icon = Some("alpha-icon".into());
        let mut beta = installed_app("Beta", "beta.desktop");
        beta.icon = Some("/usr/share/icons/beta.png".into());
        let mut app = app_with_installed_apps_menu(vec![alpha, beta]);
        open_apps_menu_panel(&mut app);

        let entries = active_menu_entries(&app);
        assert_eq!(entries[0].icon, Some("alpha-icon".into()));
        assert_eq!(entries[1].icon, None);
    }

    #[test]
    fn activating_installed_app_closes_panel() {
        let mut app = app_with_installed_apps_menu(vec![installed_app("Alpha", "alpha.desktop")]);
        open_apps_menu_panel(&mut app);
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );

        let _ = app.update(Message::Panel(
            crate::app::messages::PanelEvent::ActivateIndex(0),
        ));

        assert_eq!(
            app.foreground.launched_installed_app_ids_for_test(),
            ["alpha.desktop"]
        );
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            0
        );
    }

    #[test]
    fn duplicate_installed_app_names_are_disambiguated() {
        let mut app = app_with_installed_apps_menu(vec![
            installed_app("Terminal", "org.gnome.Terminal.desktop"),
            installed_app("Terminal", "com.raggesilver.BlackBox.desktop"),
        ]);
        open_apps_menu_panel(&mut app);

        let entries = active_menu_entries(&app);

        assert_eq!(entries.len(), 2);
        assert!(entries[0].label.contains("Terminal"));
        assert!(entries[1].label.contains("Terminal"));
        assert_ne!(entries[0].label, entries[1].label);
    }

    #[test]
    fn installed_apps_menu_can_be_empty() {
        let mut app = app_with_installed_apps_menu(vec![]);
        open_apps_menu_panel(&mut app);

        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );
        assert!(active_menu_entries(&app).is_empty());
    }

    #[test]
    fn arrow_down_on_panel_window_moves_menu_focus_to_next_entry() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 0);
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(panel_focus(&app), Some(1));
    }

    #[test]
    fn arrow_down_on_panel_window_does_not_scroll_when_focused_entry_is_visible() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 0);
        app.foreground.reset_panel_menu_scroll_for_test();
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(panel_focus(&app), Some(1));
        assert_eq!(app.foreground.panel_menu_scroll_y_for_test(), 0.0);
    }

    #[test]
    fn arrow_down_on_panel_window_scrolls_only_enough_to_reveal_focused_entry() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 40);
        app.foreground.reset_panel_menu_scroll_for_test();
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(panel_focus(&app), Some(41));
        assert_eq!(
            app.foreground.panel_menu_scroll_y_for_test(),
            42.0 * MENU_ROW_EXTENT - DEFAULT_MENU_VIEWPORT_HEIGHT
        );
    }

    #[test]
    fn arrow_up_on_panel_window_scrolls_only_when_focused_entry_is_above_view() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 10);
        app.foreground.handle(
            super::super::foreground::ForegroundIntent::PanelMenuScrolled {
                y: 10.0 * MENU_ROW_EXTENT,
                viewport_height: DEFAULT_MENU_VIEWPORT_HEIGHT,
            },
        );
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMovePrev(panel));

        assert_eq!(panel_focus(&app), Some(9));
        assert_eq!(
            app.foreground.panel_menu_scroll_y_for_test(),
            9.0 * MENU_ROW_EXTENT
        );
    }

    #[test]
    fn focus_gained_on_panel_window_does_not_create_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::BarFocusGained(panel));

        assert!(app.surface(panel).is_none());
        assert_eq!(panel_focus(&app), Some(0));
    }

    #[test]
    fn focus_search_input_event_initializes_menu_focus_without_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::PanelRequestSearchFocus);

        let _ = app.handle_panel_event(super::super::messages::PanelEvent::FocusSearchInput {
            retries: 0,
        });

        assert!(app.surface(panel).is_none());
        assert_eq!(panel_focus(&app), Some(0));
        assert!(!app.foreground.panel_search_focus_pending());
    }

    #[test]
    fn hover_focus_cancels_pending_search_focus_retries() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::PanelRequestSearchFocus);

        let _ = app.handle_panel_event(super::super::messages::PanelEvent::FocusIndex(1));

        assert_eq!(panel_focus(&app), Some(1));
        assert!(!app.foreground.panel_search_focus_pending());
    }

    #[test]
    fn window_sized_on_panel_window_routes_to_panel_focus_without_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowSized(
            panel, 360, 600,
        ));

        assert!(app.surface(panel).is_none());
        assert_eq!(panel_focus(&app), Some(0));
    }

    #[test]
    fn close_active_panel_closes_active_foreground_panel() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let tracked_panel = iced::window::Id::unique();
        app.register_panel_popup(tracked_panel, origin);

        let tasks = app.close_active_panel();

        assert!(!tasks.is_empty());
        assert!(!app.foreground.is_panel_window(tracked_panel));
    }

    #[test]
    fn stale_events_from_closing_panel_window_do_not_create_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);

        let _ = app.close_active_panel();
        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::BarFocusLost(panel));
        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowSized(
            panel, 360, 600,
        ));

        assert!(app.surface(panel).is_none());
    }

    #[test]
    fn replacing_panel_ignores_stale_old_window_close() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin).bar_name = "main".into();
        app.surface_mut(origin).output_name = "DP-1".into();
        app.surface_mut(origin).screen_size = (1920, 1080);

        let first_window = iced::window::Id::unique();
        register_panel_with_content(
            &mut app,
            first_window,
            origin,
            crate::surface::panel::PanelContent::BarPanel,
        );

        let second_window = iced::window::Id::unique();
        register_panel_with_content(
            &mut app,
            second_window,
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowClosed(
            first_window,
        ));

        assert_eq!(app.foreground.active_panel_window(), Some(second_window));
        assert!(matches!(
            app.foreground.active_panel_tree().map(|tree| &tree.content),
            Some(crate::surface::panel::PanelContent::Menu(name)) if name == "main"
        ));
        assert!(!app.foreground.is_panel_window(first_window));
        assert!(app.foreground.is_panel_window(second_window));
    }

    #[test]
    fn escape_clears_search_before_closing_panel() {
        let mut panel = super::super::panel_runtime::PanelRuntime::default();
        let menus = [menu_with_two_entries()];
        panel.handle(
            super::super::panel_runtime::PanelRuntimeIntent::PrepareOpen {
                request: super::super::panel_runtime::PanelOpenRequest {
                    origin_window: iced::window::Id::unique(),
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::Menu("main".into()),
                    anchor: None,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    screen_size: (1920, 1080),
                },
                catalog: super::super::panel_runtime::PanelMenuCatalog::new(&menus),
                module_detail_sessions: std::collections::HashMap::new(),
            },
        );
        panel.handle(super::super::panel_runtime::PanelRuntimeIntent::SearchChanged("term".into()));

        assert!(matches!(
            panel
                .handle(super::super::panel_runtime::PanelRuntimeIntent::Escape)
                .as_slice(),
            [super::super::panel_runtime::PanelRuntimeEffect::RequestSearchFocus]
        ));
        assert_eq!(
            panel.panel_menu_snapshot_for_test().unwrap().search_query,
            ""
        );
    }

    #[test]
    fn escape_that_clears_search_requests_search_refocus() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.surface_mut(origin);
        register_menu_panel(&mut app, panel, origin);
        set_panel_search(&mut app, "term");
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::PanelSearchFocusSettled);

        let _ = app.handle_panel_escape_pressed();

        assert_eq!(
            app.foreground
                .panel_menu_snapshot_for_test()
                .unwrap()
                .search_query,
            ""
        );
        assert!(app.foreground.panel_search_focus_pending());
        assert!(app.foreground.is_panel_window(panel));
    }

    #[test]
    fn escape_without_first_escape_behavior_closes_panel_through_foreground() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.surface_mut(origin);
        register_panel_with_content(
            &mut app,
            panel,
            origin,
            crate::surface::panel::PanelContent::BarPanel,
        );

        let _ = app.handle_panel_escape_pressed();

        assert!(!app.foreground.is_panel_window(panel));
    }

    #[test]
    fn panel_search_input_id_is_stable_for_focus_task() {
        let first = iced::window::Id::unique();
        let second = iced::window::Id::unique();

        assert_eq!(panel_search_input_id(first), panel_search_input_id(first));
        assert_ne!(panel_search_input_id(first), panel_search_input_id(second));
    }
}
