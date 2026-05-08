use std::collections::{HashMap, HashSet};
use std::ops::{Index, IndexMut};

use iced::Task;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, NewLayerShellSettings, OutputOption,
};

use super::{App, Message};
use crate::modules::ModuleKind;
use crate::surface::SurfaceState;

pub(super) struct PopupMeta {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) tree: crate::surface::menu_popup::MenuTree,
    pub(super) bar_name: String,
}

fn render_to_segments(
    template: &crate::config::FormatTemplate,
    values: &[(&str, String)],
    state: Option<&str>,
) -> Vec<crate::surface::tooltip_popup::RenderedSegment> {
    let owned: Vec<(&str, String)> = values.iter().map(|(k, v)| (*k, v.clone())).collect();
    template
        .render_styled_segments_with_default(&owned, state, &crate::config::Style::default())
        .into_iter()
        .map(|(text, style)| crate::surface::tooltip_popup::RenderedSegment { text, style })
        .collect()
}

fn tooltip_margin(
    tree: &crate::surface::tooltip_popup::TooltipTree,
    size: (u32, u32),
    gap: i32,
) -> (i32, i32, i32, i32) {
    let (w, h) = (size.0 as i32, size.1 as i32);
    let (bar_w, bar_h) = (tree.bar_size.0 as i32, tree.bar_size.1 as i32);
    let (ax, ay) = tree.anchor;
    match tree.bar_edge {
        crate::config::Edge::Top => {
            let left = (ax - w / 2).clamp(0, (bar_w - w).max(0));
            (bar_h + gap, 0, 0, left)
        }
        crate::config::Edge::Bottom => {
            let left = (ax - w / 2).clamp(0, (bar_w - w).max(0));
            (0, 0, bar_h + gap, left)
        }
        crate::config::Edge::Left => {
            let top = (ay - h / 2).clamp(0, (bar_h - h).max(0));
            (top, 0, 0, bar_w + gap)
        }
        crate::config::Edge::Right => {
            let top = (ay - h / 2).clamp(0, (bar_h - h).max(0));
            (top, bar_w + gap, 0, 0)
        }
    }
}

fn tooltip_anchor(tree: &crate::surface::tooltip_popup::TooltipTree) -> Anchor {
    match tree.bar_edge {
        crate::config::Edge::Top => Anchor::Left | Anchor::Top,
        crate::config::Edge::Bottom => Anchor::Left | Anchor::Bottom,
        crate::config::Edge::Left => Anchor::Left | Anchor::Top,
        crate::config::Edge::Right => Anchor::Right | Anchor::Top,
    }
}

fn tooltip_popup_window_settings(
    tree: &crate::surface::tooltip_popup::TooltipTree,
) -> NewLayerShellSettings {
    let size = crate::surface::tooltip_popup::popup_size(tree);
    NewLayerShellSettings {
        anchor: tooltip_anchor(tree),
        exclusive_zone: Some(-1),
        size: Some(size),
        margin: Some(tooltip_margin(tree, size, 4)),
        layer: iced_layershell::reexport::Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::None,
        ..Default::default()
    }
}

fn spawn_tooltip_popup_window(
    tree: &crate::surface::tooltip_popup::TooltipTree,
) -> (iced::window::Id, Task<Message>) {
    let settings = tooltip_popup_window_settings(tree);
    Message::layershell_open(settings)
}

#[derive(Debug, Clone)]
pub(super) struct TooltipPopup {
    pub window: iced::window::Id,
    pub target_window: iced::window::Id,
    pub target_module: crate::modules::ModuleId,
    pub tree: crate::surface::tooltip_popup::TooltipTree,
}

struct TooltipAnchor {
    anchor: (i32, i32),
    bar_size: (u32, u32),
    screen_size: (u32, u32),
    bar_edge: crate::config::Edge,
}

pub(super) struct SurfaceManager {
    pub(super) surfaces: HashMap<iced::window::Id, SurfaceState>,
    pub(super) pending_surface_keys: HashMap<iced::window::Id, (String, String)>,
    pub(super) manual_outputs: bool,
    pub(super) popups: HashMap<iced::window::Id, PopupMeta>,
    pub(super) tooltip_popups: HashMap<iced::window::Id, TooltipPopup>,
    pub(super) last_cursor: Option<(iced::window::Id, iced::Point)>,
}

#[allow(dead_code)]
impl SurfaceManager {
    pub(super) fn new(
        surfaces: HashMap<iced::window::Id, SurfaceState>,
        pending_surface_keys: HashMap<iced::window::Id, (String, String)>,
        manual_outputs: bool,
    ) -> Self {
        Self {
            surfaces,
            pending_surface_keys,
            manual_outputs,
            popups: HashMap::new(),
            tooltip_popups: HashMap::new(),
            last_cursor: None,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.surfaces.len()
    }

    pub(super) fn contains_key(&self, id: &iced::window::Id) -> bool {
        self.surfaces.contains_key(id)
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = &iced::window::Id> {
        self.surfaces.keys()
    }

    pub(super) fn values(&self) -> impl Iterator<Item = &SurfaceState> {
        self.surfaces.values()
    }

    pub(super) fn values_mut(&mut self) -> impl Iterator<Item = &mut SurfaceState> {
        self.surfaces.values_mut()
    }

    pub(super) fn get(&self, id: &iced::window::Id) -> Option<&SurfaceState> {
        self.surfaces.get(id)
    }

    pub(super) fn get_mut(&mut self, id: &iced::window::Id) -> Option<&mut SurfaceState> {
        self.surfaces.get_mut(id)
    }

    pub(super) fn manual_outputs(&self) -> bool {
        self.manual_outputs
    }

    pub(super) fn surface(&self, id: iced::window::Id) -> Option<&SurfaceState> {
        self.surfaces.get(&id)
    }

    pub(super) fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
        if !self.surfaces.contains_key(&id) {
            let (output_name, bar_name) = self.pending_surface_keys.remove(&id).unwrap_or_default();
            self.surfaces
                .insert(id, SurfaceState::new(output_name, bar_name));
        }
        self.surfaces.get_mut(&id).unwrap()
    }

    pub(super) fn surfaces(&self) -> impl Iterator<Item = (&iced::window::Id, &SurfaceState)> {
        self.surfaces.iter()
    }

    pub(super) fn surfaces_mut(&mut self) -> impl Iterator<Item = &mut SurfaceState> {
        self.surfaces.values_mut()
    }

    pub(super) fn last_cursor(&self) -> Option<(iced::window::Id, iced::Point)> {
        self.last_cursor
    }

    pub(super) fn set_last_cursor(&mut self, value: Option<(iced::window::Id, iced::Point)>) {
        self.last_cursor = value;
    }

    pub(super) fn set_manual_outputs(&mut self, value: bool) {
        self.manual_outputs = value;
    }

    pub(super) fn popup(&self, id: iced::window::Id) -> Option<&PopupMeta> {
        self.popups.get(&id)
    }

    pub(super) fn popup_ids(&self) -> impl Iterator<Item = iced::window::Id> + '_ {
        self.popups.keys().copied()
    }

    pub(super) fn popups(&self) -> impl Iterator<Item = (&iced::window::Id, &PopupMeta)> {
        self.popups.iter()
    }

    pub(super) fn popup_values(&self) -> impl Iterator<Item = &PopupMeta> {
        self.popups.values()
    }

    pub(super) fn insert_popup(&mut self, id: iced::window::Id, meta: PopupMeta) {
        self.popups.insert(id, meta);
    }

    pub(super) fn remove_popup(&mut self, id: iced::window::Id) -> Option<PopupMeta> {
        self.popups.remove(&id)
    }

    pub(super) fn has_popup(&self, id: iced::window::Id) -> bool {
        self.popups.contains_key(&id)
    }

    pub(super) fn has_any_popup(&self) -> bool {
        !self.popups.is_empty() || !self.tooltip_popups.is_empty()
    }

    pub(super) fn tooltip_popup(&self, window: iced::window::Id) -> Option<&TooltipPopup> {
        self.tooltip_popups.get(&window)
    }

    pub(super) fn tooltip_popup_mut(
        &mut self,
        window: iced::window::Id,
    ) -> Option<&mut TooltipPopup> {
        self.tooltip_popups.get_mut(&window)
    }

    pub(super) fn insert_tooltip_popup(&mut self, popup: TooltipPopup) {
        self.tooltip_popups.insert(popup.window, popup);
    }

    pub(super) fn remove_tooltip_popup(
        &mut self,
        window: iced::window::Id,
    ) -> Option<TooltipPopup> {
        self.tooltip_popups.remove(&window)
    }

    pub(super) fn tooltip_popup_for_target(
        &self,
        target_window: iced::window::Id,
    ) -> Option<&TooltipPopup> {
        self.tooltip_popups
            .values()
            .find(|p| p.target_window == target_window)
    }
}

impl Index<iced::window::Id> for SurfaceManager {
    type Output = SurfaceState;

    fn index(&self, index: iced::window::Id) -> &Self::Output {
        &self.surfaces[&index]
    }
}

impl Index<&iced::window::Id> for SurfaceManager {
    type Output = SurfaceState;

    fn index(&self, index: &iced::window::Id) -> &Self::Output {
        &self.surfaces[index]
    }
}

impl IndexMut<iced::window::Id> for SurfaceManager {
    fn index_mut(&mut self, index: iced::window::Id) -> &mut Self::Output {
        self.surfaces.get_mut(&index).expect("surface must exist")
    }
}

impl App {
    fn is_tooltip_popup_window(&self, window: iced::window::Id) -> bool {
        self.surfaces.tooltip_popup(window).is_some()
    }

    fn is_menu_popup_window(&self, window: iced::window::Id) -> bool {
        self.surfaces.has_popup(window)
    }

    pub(super) fn handle_surface_event(
        &mut self,
        event: super::messages::SurfaceEvent,
    ) -> Task<Message> {
        use super::messages::SurfaceEvent;

        match event {
            SurfaceEvent::CursorMoved(window, pos) => {
                if self.is_tooltip_popup_window(window) {
                    return Task::none();
                }
                let has_tray = self.modules.iter().any(|(_, module)| {
                    matches!(module.kind(), ModuleKind::Tray | ModuleKind::TrayIcon)
                });
                if has_tray && self.surfaces.last_cursor() != Some((window, pos)) {
                    self.surfaces.set_last_cursor(Some((window, pos)));
                }
                self.surface_mut(window).last_pointer_position = (pos.x as i32, pos.y as i32);
                Task::none()
            }
            SurfaceEvent::WindowSized(window, w, h) => {
                if let Some(popup) = self.surfaces.tooltip_popup_mut(window) {
                    popup.tree.screen_size = (w, h);
                } else {
                    self.surfaces.surface_mut(window).screen_size = (w, h);
                }
                Task::none()
            }
            SurfaceEvent::WindowClosed(window) if self.is_menu_popup_window(window) => {
                Task::done(Message::Menu(super::messages::MenuEvent::Closed))
            }
            SurfaceEvent::WindowClosed(window) if self.is_tooltip_popup_window(window) => {
                self.surfaces.remove_tooltip_popup(window);
                Task::none()
            }
            SurfaceEvent::WindowClosed(_) => Task::none(),
            SurfaceEvent::ModuleHoverEnter(id, window) => {
                self.handle_module_hover_enter(id, window)
            }
            SurfaceEvent::ModuleHoverExit(id, window) => self.handle_module_hover_exit(id, window),
            SurfaceEvent::TooltipDwellElapsed { window, module } => {
                self.handle_tooltip_dwell_elapsed(window, module)
            }
            SurfaceEvent::CloseTooltip { window } => {
                Task::batch(self.close_tooltip_popup_if_open(window))
            }
            // SurfaceEvent::BarClick(window) if self.is_menu_popup_window(window) => {
            //     Task::done(Message::Menu(super::messages::MenuEvent::Closed))
            // }
            // SurfaceEvent::BarClick(window) if self.is_tooltip_popup_window(window) => Task::none(),
            SurfaceEvent::BarClick(window) => self.handle_bar_click(window),
            SurfaceEvent::BarHoverEnter(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::BarHoverEnter(window) => self.handle_bar_hover_enter(window),
            SurfaceEvent::BarHoverExit(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::BarHoverExit(window) => self.handle_bar_hover_exit(window),
            SurfaceEvent::Tick => self.handle_tick(),
            SurfaceEvent::BarFocusGained(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::BarFocusGained(window) => self.handle_bar_focus_gained(window),
            SurfaceEvent::BarFocusLost(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::BarFocusLost(window) if self.is_menu_popup_window(window) => {
                Task::done(Message::Menu(super::messages::MenuEvent::Closed))
            }
            SurfaceEvent::BarFocusLost(window) => self.handle_bar_focus_lost(window),
            SurfaceEvent::EscapePressed(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::EscapePressed(window) => self.handle_escape_pressed(window),
            SurfaceEvent::FocusMoveNext(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::FocusMoveNext(window) => self.handle_focus_move(window, true),
            SurfaceEvent::FocusMovePrev(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::FocusMovePrev(window) => self.handle_focus_move(window, false),
            SurfaceEvent::FocusNextZone(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::FocusNextZone(window) => self.handle_focus_zone(window, true),
            SurfaceEvent::FocusPrevZone(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::FocusPrevZone(window) => self.handle_focus_zone(window, false),
            SurfaceEvent::KeyboardActivate(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::KeyboardActivate(window) => self.handle_keyboard_activate(window),
            SurfaceEvent::KeyboardSecondaryActivate(window)
                if self.is_tooltip_popup_window(window) =>
            {
                Task::none()
            }
            SurfaceEvent::KeyboardSecondaryActivate(window) => {
                self.handle_keyboard_secondary_activate(window)
            }
            SurfaceEvent::ToggleExpand => self.handle_toggle_expand(),
        }
    }

    /// Returns a mutable reference to the `SurfaceState` for `id`, inserting a default if absent.
    /// Uses the pending (output_name, bar_name) pair (Story 9.1) when initialising a new entry.
    // Thin compatibility accessors while App remains the iced root.
    pub(super) fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
        self.surfaces.surface_mut(id)
    }

    /// Returns a reference to the `SurfaceState` for `id`, or `None` if the surface is unknown.
    // Thin compatibility accessors while App remains the iced root.
    pub(super) fn surface(&self, id: iced::window::Id) -> Option<&SurfaceState> {
        self.surfaces.surface(id)
    }

    /// Opens a layer-shell window for one (bar, output) pair, records it in `pending` and
    /// `surfaces`, and returns the open task. Shared by `boot` and `open_new_output_windows`.
    pub(super) fn bar_window_settings(
        bar: &crate::config::BarConfig,
        output_name: &str,
    ) -> NewLayerShellSettings {
        let anchor = bar.edge.anchor();
        let collapsed = bar.collapsed_width;
        let exclusive_zone = i32::try_from(collapsed).unwrap_or(48);
        let size = if bar.edge.is_horizontal() {
            Some((0, collapsed))
        } else {
            Some((collapsed, 0))
        };
        NewLayerShellSettings {
            anchor,
            layer: bar.layer.layer_shell(),
            exclusive_zone: Some(exclusive_zone),
            size,
            output_option: OutputOption::OutputName(output_name.to_string()),
            namespace: Some(Self::namespace()),
            ..Default::default()
        }
    }

    /// Opens a layer-shell window for one (bar, output) pair, records it in `pending` and
    /// `surfaces`, and returns the open task. Shared by `boot` and `open_new_output_windows`.
    pub(super) fn spawn_bar_on_output(
        bar: &crate::config::BarConfig,
        output_name: &str,
        pending: &mut HashMap<iced::window::Id, (String, String)>,
        surfaces: &mut HashMap<iced::window::Id, SurfaceState>,
    ) -> Task<Message> {
        let settings = Self::bar_window_settings(bar, output_name);
        let (id, task) = Message::layershell_open(settings);
        pending.insert(id, (output_name.to_string(), bar.name.clone()));
        surfaces.insert(
            id,
            SurfaceState::new(output_name.to_string(), bar.name.clone()),
        );
        task
    }

    /// In manual-output mode, opens new layer-shell windows for any (output, bar) pairs present
    /// in `workspaces` that do not already have a surface.
    pub(super) fn open_new_output_windows(
        &mut self,
        workspaces: &[niri_ipc::Workspace],
    ) -> Vec<Task<Message>> {
        let known_pairs: HashSet<(String, String)> = self
            .surfaces
            .surfaces()
            .map(|(_, s)| s)
            .filter(|s| !s.output_name.is_empty())
            .map(|s| (s.output_name.clone(), s.bar_name.clone()))
            .collect();

        let mut to_spawn: Vec<(usize, String)> = workspaces
            .iter()
            .filter_map(|ws| ws.output.as_deref())
            .filter(|n| !n.is_empty())
            .flat_map(|output_name| {
                self.config
                    .bars
                    .iter()
                    .enumerate()
                    .filter(move |(_, b)| b.matches_output(output_name))
                    .filter(|(_, b)| {
                        !known_pairs.contains(&(output_name.to_string(), b.name.clone()))
                    })
                    .map(move |(i, _)| (i, output_name.to_string()))
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        to_spawn.sort_by_key(|(idx, out)| {
            let is_side = !self.config.bars[*idx].edge.is_horizontal();
            (out.clone(), is_side)
        });

        let mut tasks = Vec::with_capacity(to_spawn.len());
        for (bar_idx, output_name) in to_spawn {
            let task = Self::spawn_bar_on_output(
                &self.config.bars[bar_idx],
                &output_name,
                &mut self.surfaces.pending_surface_keys,
                &mut self.surfaces.surfaces,
            );
            tasks.push(task);
        }
        tasks
    }

    /// Look up the `BarConfig` for a surface by its `bar_name`.
    /// Falls back to the first bar when the name is empty (AllScreens mode) or not found.
    pub(super) fn bar_for_surface_name<'a>(
        bars: &'a [crate::config::BarConfig],
        bar_name: &str,
    ) -> &'a crate::config::BarConfig {
        if bar_name.is_empty() {
            bars.first().expect("at least one bar configured")
        } else {
            bars.iter()
                .find(|b| b.name == bar_name)
                .unwrap_or_else(|| bars.first().expect("at least one bar configured"))
        }
    }

    /// Returns a `SizeChange` task for the named surface's current expand state in overlay mode.
    pub(super) fn overlay_size_change_for(&self, window: iced::window::Id) -> Task<Message> {
        let surface = self.surfaces.surface(window);
        let expand = surface.map(|s| s.expand).unwrap_or_default();
        let bar_name = surface.map(|s| s.bar_name.as_str()).unwrap_or("");
        let bar = Self::bar_for_surface_name(&self.config.bars, bar_name);
        let size = expand.current_width(bar.collapsed_width, bar.expanded_width);
        if bar.edge.is_horizontal() {
            Task::done(Message::SizeChange {
                id: window,
                size: (0, size),
            })
        } else {
            Task::done(Message::SizeChange {
                id: window,
                size: (size, 0),
            })
        }
    }

    /// Remove the `tray_icon` module with the given module id from `self.modules`,
    /// and clear any surface hover/focus that pointed at it.
    pub(super) fn remove_tray_icon(&mut self, module_id: &str) {
        let had_it = self
            .modules
            .iter()
            .any(|(_, m)| m.kind() == ModuleKind::TrayIcon && m.id().0 == module_id);
        if !had_it {
            return;
        }
        self.modules
            .retain(|(_, m)| !(m.kind() == ModuleKind::TrayIcon && m.id().0 == module_id));
        for s in self.surfaces.surfaces_mut() {
            if s.focused_module.as_ref().map(|id| id.0.as_str()) == Some(module_id) {
                s.focused_module = None;
            }
            if s.hovered_module.as_ref().map(|id| id.0.as_str()) == Some(module_id) {
                s.hovered_module = None;
            }
        }
    }

    /// Return the index at which to insert a new tray icon after `slot_idx`.
    /// Scans forward past any existing `tray_icon` modules for the same slot.
    pub(super) fn tray_insert_position(&self, slot_idx: usize, config_name: &str) -> usize {
        let mut pos = slot_idx + 1;
        while pos < self.modules.len() {
            let (_, m) = &self.modules[pos];
            if m.kind() == ModuleKind::TrayIcon && m.config_name() == Some(config_name) {
                pos += 1;
            } else {
                break;
            }
        }
        pos
    }

    fn handle_module_hover_enter(
        &mut self,
        id: crate::modules::ModuleId,
        window: iced::window::Id,
    ) -> Task<Message> {
        let resolved = self.resolved_tooltip_for(window, &id);
        let s = self.surface_mut(window);
        s.hovered_module = Some(id.clone());

        let mut tasks = Vec::new();
        let close = self.close_tooltip_popup_if_open(window);
        if !close.is_empty() {
            tasks.extend(close);
        }

        if resolved.is_some() {
            self.surface_mut(window).tooltip_pending = Some((id, 0.0));
        } else {
            self.surface_mut(window).tooltip_pending = None;
        }

        Task::batch(tasks)
    }

    fn handle_module_hover_exit(
        &mut self,
        id: crate::modules::ModuleId,
        window: iced::window::Id,
    ) -> Task<Message> {
        let s = self.surface_mut(window);
        if s.hovered_module.as_ref() == Some(&id) {
            s.hovered_module = None;
        }
        if let Some((pending_id, _)) = &s.tooltip_pending
            && pending_id == &id
        {
            s.tooltip_pending = None;
        }
        let close = self.close_tooltip_popup_if_open_for(window, &id);
        Task::batch(close)
    }

    fn handle_tooltip_dwell_elapsed(
        &mut self,
        target_window: iced::window::Id,
        module: crate::modules::ModuleId,
    ) -> Task<Message> {
        if self
            .surface(target_window)
            .and_then(|s| s.hovered_module.as_ref())
            != Some(&module)
        {
            return Task::none();
        }

        let template = match self.resolved_tooltip_for(target_window, &module) {
            Some(t) => t,
            None => return Task::none(),
        };

        let values = self.tooltip_values_for(target_window, &module);
        let rendered = render_to_segments(&template, &values, None);

        if rendered.iter().all(|s| s.text.trim().is_empty()) {
            return Task::none();
        }

        let placement = self.tooltip_anchor_for(target_window);

        let tree = crate::surface::tooltip_popup::TooltipTree {
            segments: rendered,
            anchor: placement.anchor,
            bar_size: placement.bar_size,
            screen_size: placement.screen_size,
            bar_edge: placement.bar_edge,
        };

        let (popup_wid, open_task) = spawn_tooltip_popup_window(&tree);

        self.surfaces.insert_tooltip_popup(TooltipPopup {
            window: popup_wid,
            target_window,
            target_module: module,
            tree,
        });

        open_task
    }

    fn tooltip_values_for(
        &self,
        window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Vec<(&'static str, String)> {
        let m = match self.modules.get(module) {
            Some(m) => m,
            None => return Vec::new(),
        };
        let default_state = crate::modules::ModuleState::default();
        let state = self.modules.state(module).unwrap_or(&default_state);
        let _ = window;
        m.tooltip_values(state)
    }

    fn tooltip_anchor_for(&self, window: iced::window::Id) -> TooltipAnchor {
        let s = self.surface(window);
        let screen = s.map(|s| s.screen_size).unwrap_or((0, 0));
        let bar_size = screen;
        let anchor = s.map(|s| s.last_pointer_position).unwrap_or((0, 0));
        let edge = self.bar_for_window(window).edge.clone();
        TooltipAnchor {
            anchor,
            bar_size,
            screen_size: screen,
            bar_edge: edge,
        }
    }

    pub(super) fn resolved_tooltip_for(
        &self,
        window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Option<crate::config::FormatTemplate> {
        use crate::config::ModuleParams;
        use crate::config::tooltip_defaults as td;

        let config_name = self
            .modules
            .config_name_for(module)
            .unwrap_or(module.0.as_str());
        let mc = self.config.modules.iter().find(|m| m.name == config_name)?;

        let (setting, default) = match &mc.params {
            ModuleParams::Battery(c) => (&c.tooltip, td::battery_default()),
            ModuleParams::Volume(c) => (&c.tooltip, td::volume_default()),
            ModuleParams::Clock(c) => (&c.tooltip, td::clock_default()),
            ModuleParams::Network(c) => (&c.tooltip, td::network_default()),
            ModuleParams::FocusedWindow(c) => (&c.tooltip, td::focused_window_default()),
            ModuleParams::Workspaces(c) => (&c.tooltip, td::workspaces_default()),
            ModuleParams::Button(c) => (&c.tooltip, td::button_default()),
            ModuleParams::Script(c) => (&c.tooltip, td::script_default()),
            ModuleParams::Tray(c) => (&c.tooltip, td::tray_default()),
            ModuleParams::None => return None,
        };
        let _ = window;
        setting.resolve(default)
    }

    pub(super) fn close_tooltip_popup_if_open(
        &mut self,
        target_window: iced::window::Id,
    ) -> Vec<Task<Message>> {
        let popup_wid = self
            .surfaces
            .tooltip_popup_for_target(target_window)
            .map(|p| p.window);
        if let Some(wid) = popup_wid {
            self.surfaces.remove_tooltip_popup(wid);
            return vec![Task::done(Message::RemoveWindow(wid))];
        }
        Vec::new()
    }

    pub(super) fn close_tooltip_popup_if_open_for(
        &mut self,
        target_window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Vec<Task<Message>> {
        let popup_wid = self
            .surfaces
            .tooltip_popup_for_target(target_window)
            .filter(|p| &p.target_module == module)
            .map(|p| p.window);
        if let Some(wid) = popup_wid {
            self.surfaces.remove_tooltip_popup(wid);
            return vec![Task::done(Message::RemoveWindow(wid))];
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Edge;
    use crate::modules::ModuleId;
    use crate::surface::expand::ExpandState;

    fn make_config() -> crate::config::Config {
        crate::config::Config::default()
    }

    fn test_wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn tooltip_tree(edge: crate::config::Edge) -> crate::surface::tooltip_popup::TooltipTree {
        crate::surface::tooltip_popup::TooltipTree {
            segments: vec![],
            anchor: (24, 500),
            bar_size: (48, 1080),
            screen_size: (48, 1080),
            bar_edge: edge,
        }
    }

    #[test]
    fn app_starts_with_no_surfaces() {
        let (app, _task) = App::boot(make_config(), vec![]);
        assert!(app.surfaces.is_empty());
    }

    #[test]
    fn surface_mut_creates_default_state_for_new_window() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        assert!(!app.surfaces.contains_key(&wid));
        let s = app.surface_mut(wid);
        assert_eq!(s.expand, ExpandState::Collapsed);
    }

    #[test]
    fn two_surfaces_have_independent_expand_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid_a = test_wid();
        let wid_b = test_wid();
        app.surface_mut(wid_a).expand = ExpandState::Expanded;
        let b_state = app.surfaces.surface(wid_b);
        assert!(
            b_state
                .map(|s| s.expand == ExpandState::Collapsed)
                .unwrap_or(true)
        );
    }

    #[test]
    fn two_surfaces_have_independent_focus_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid_a = test_wid();
        let wid_b = test_wid();
        let focus_id = ModuleId::new("clock");
        app.surface_mut(wid_a).focused_module = Some(focus_id);
        let b_state = app.surfaces.surface(wid_b);
        assert!(b_state.map(|s| s.focused_module.is_none()).unwrap_or(true));
    }

    #[test]
    fn boot_with_empty_allowed_outputs_uses_allscreens_mode() {
        let config = make_config();
        let (app, _) = App::boot(config, vec![]);
        assert!(app.surfaces.pending_surface_keys.is_empty());
    }

    #[test]
    fn boot_manual_outputs_false_when_no_allowed_outputs() {
        let (app, _) = App::boot(make_config(), vec![]);
        assert!(!app.surfaces.manual_outputs());
    }

    #[test]
    fn boot_manual_outputs_true_when_allowed_outputs_given() {
        let (app, _) = App::boot(make_config(), vec!["eDP-1".into()]);
        assert!(app.surfaces.manual_outputs());
    }

    #[test]
    fn bar_for_surface_name_returns_first_when_name_empty() {
        let config = make_config();
        let bars = &config.bars;
        let bar = App::bar_for_surface_name(bars, "");
        assert_eq!(bar.name, "main");
    }

    #[test]
    fn bar_for_surface_name_returns_matching_bar() {
        use crate::config::BarConfig;
        let config = crate::config::Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "side".into(),
                    output_target: crate::config::OutputTarget::Include(vec!["DP-1".into()]),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
            command_policy: crate::config::CommandPolicy::default(),
        };
        let bar = App::bar_for_surface_name(&config.bars, "side");
        assert_eq!(bar.name, "side");
    }

    #[test]
    fn bar_for_surface_name_falls_back_to_first_on_miss() {
        use crate::config::BarConfig;
        let config = crate::config::Config {
            bars: vec![BarConfig {
                name: "main".into(),
                ..BarConfig::default()
            }],
            modules: vec![],
            command_policy: crate::config::CommandPolicy::default(),
        };
        let bar = App::bar_for_surface_name(&config.bars, "nonexistent");
        assert_eq!(bar.name, "main");
    }

    #[test]
    fn boot_manual_mode_creates_surface_per_bar_per_output() {
        use crate::config::BarConfig;
        let config = crate::config::Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "top".into(),
                    edge: Edge::Top,
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
            command_policy: crate::config::CommandPolicy::default(),
        };
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        assert_eq!(
            app.surfaces.len(),
            2,
            "two bars on one output -> two surfaces"
        );
    }

    #[test]
    fn bar_surface_window_settings_use_configured_bar_layer() {
        let bar = crate::config::BarConfig {
            layer: crate::config::BarLayer::Background,
            ..crate::config::BarConfig::default()
        };
        let settings = App::bar_window_settings(&bar, "eDP-1");
        assert_eq!(settings.layer, iced_layershell::reexport::Layer::Background);
    }

    #[test]
    fn surface_bar_name_set_from_pending_surface_key() {
        use crate::config::BarConfig;
        let config = crate::config::Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output_target: crate::config::OutputTarget::Include(vec!["eDP-1".into()]),
                ..BarConfig::default()
            }],
            modules: vec![],
            command_policy: crate::config::CommandPolicy::default(),
        };
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        let bar_names: Vec<&str> = app
            .surfaces
            .surfaces()
            .map(|(_, s)| s.bar_name.as_str())
            .collect();
        assert!(
            bar_names.contains(&"main"),
            "surface must carry bar_name 'main'; got: {bar_names:?}"
        );
    }

    #[test]
    fn surface_output_name_set_from_pending_surface_key() {
        use crate::config::BarConfig;
        let config = crate::config::Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output_target: crate::config::OutputTarget::Include(vec!["eDP-1".into()]),
                ..BarConfig::default()
            }],
            modules: vec![],
            command_policy: crate::config::CommandPolicy::default(),
        };
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        let output_names: Vec<&str> = app
            .surfaces
            .surfaces()
            .map(|(_, s)| s.output_name.as_str())
            .collect();
        assert!(
            output_names.contains(&"eDP-1"),
            "surface must carry output_name 'eDP-1'; got: {output_names:?}"
        );
    }

    #[test]
    fn boot_spawns_horizontal_bars_before_side_bars() {
        let config = crate::config::Config {
            bars: vec![
                crate::config::BarConfig {
                    name: "left".into(),
                    edge: Edge::Left,
                    ..crate::config::BarConfig::default()
                },
                crate::config::BarConfig {
                    name: "top".into(),
                    edge: Edge::Top,
                    ..crate::config::BarConfig::default()
                },
            ],
            modules: vec![],
            command_policy: crate::config::CommandPolicy::default(),
        };
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        let names: Vec<String> = app
            .surfaces
            .pending_surface_keys
            .values()
            .map(|(_, bar)| bar.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert!(names.contains(&"top".to_string()), "top bar not spawned");
        assert!(names.contains(&"left".to_string()), "left bar not spawned");
    }

    #[test]
    fn wrapped_module_hover_enter_sets_hovered_module() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        let id = crate::modules::ModuleId::new("clock");

        let _ = app.update(Message::Surface(
            super::super::messages::SurfaceEvent::ModuleHoverEnter(id.clone(), wid),
        ));

        assert_eq!(app.surface(wid).unwrap().hovered_module, Some(id));
    }

    #[test]
    fn tooltip_resolution_uses_dynamic_module_config_name() {
        let item = crate::dbus::sni::host::TrayItem {
            service: "org.test.App".into(),
            object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: crate::dbus::sni::host::ItemStatus::Active,
            menu_path: None,
            tooltip: Some(crate::dbus::sni::host::SniTooltip {
                title: "Test App".into(),
                description: String::new(),
            }),
        };
        let mut config = crate::config::Config {
            modules: vec![crate::config::ModuleConfig {
                name: "tray".into(),
                module_type: "tray".into(),
                params: crate::config::ModuleParams::Tray(crate::config::TrayConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..crate::config::Config::default()
        };
        config.bars[0].modules.end = vec!["tray".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.handle_tray_event(crate::dbus::sni::host::TrayEvent::ItemAdded(item));
        let tray_icon_id = app
            .modules
            .ids_by_kind(crate::modules::ModuleKind::TrayIcon)
            .first()
            .cloned()
            .expect("tray icon inserted");

        assert!(
            app.resolved_tooltip_for(test_wid(), &tray_icon_id)
                .is_some()
        );
    }

    #[test]
    fn stale_tooltip_dwell_does_not_open_popup() {
        let mut config = crate::config::Config {
            modules: vec![crate::config::ModuleConfig {
                name: "battery".into(),
                module_type: "battery".into(),
                params: crate::config::ModuleParams::Battery(
                    crate::config::BatteryConfig::default(),
                ),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..crate::config::Config::default()
        };
        config.bars[0].modules.end = vec!["battery".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let wid = test_wid();
        let module = ModuleId::new("battery");
        app.modules.set_state(
            module.clone(),
            crate::modules::ModuleState::Battery(crate::modules::battery::BatteryState {
                percentage: 74.0,
                is_charging: false,
            }),
        );
        app.surface_mut(wid).hovered_module = None;

        let _ = app.handle_tooltip_dwell_elapsed(wid, module);

        assert!(app.surfaces.tooltip_popups.is_empty());
    }

    #[test]
    fn tooltip_popup_window_size_updates_popup_tree_screen_size() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let popup = test_wid();
        app.surfaces.insert_tooltip_popup(TooltipPopup {
            window: popup,
            target_window: test_wid(),
            target_module: ModuleId::new("clock"),
            tree: crate::surface::tooltip_popup::TooltipTree {
                segments: vec![],
                anchor: (0, 0),
                bar_size: (48, 1080),
                screen_size: (48, 1080),
                bar_edge: crate::config::Edge::Left,
            },
        });

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowSized(
            popup, 1920, 1080,
        ));

        assert_eq!(
            app.surfaces
                .tooltip_popup(popup)
                .expect("tooltip popup")
                .tree
                .screen_size,
            (1920, 1080)
        );
    }

    #[test]
    fn tooltip_popup_window_is_not_keyboard_interactive() {
        let tree = tooltip_tree(crate::config::Edge::Left);
        assert_eq!(
            tooltip_popup_window_settings(&tree).keyboard_interactivity,
            KeyboardInteractivity::None
        );
    }

    #[test]
    fn tooltip_popup_window_is_sized_to_content_not_full_overlay() {
        let tree = tooltip_tree(crate::config::Edge::Left);
        let settings = tooltip_popup_window_settings(&tree);

        assert_eq!(settings.size, Some((32, 34)));
        assert_eq!(settings.anchor, Anchor::Left | Anchor::Top);
        assert_eq!(settings.margin, Some((483, 0, 0, 52)));
    }

    #[test]
    fn right_edge_tooltip_uses_right_anchor_margin() {
        let tree = tooltip_tree(crate::config::Edge::Right);
        let settings = tooltip_popup_window_settings(&tree);

        assert_eq!(settings.anchor, Anchor::Right | Anchor::Top);
        assert_eq!(settings.margin, Some((483, 52, 0, 0)));
    }

    #[test]
    fn tooltip_popup_focus_events_do_not_create_bar_surface() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let popup = test_wid();
        app.surfaces.insert_tooltip_popup(TooltipPopup {
            window: popup,
            target_window: test_wid(),
            target_module: ModuleId::new("clock"),
            tree: crate::surface::tooltip_popup::TooltipTree {
                segments: vec![],
                anchor: (0, 0),
                bar_size: (48, 1080),
                screen_size: (1920, 1080),
                bar_edge: crate::config::Edge::Left,
            },
        });

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::BarFocusGained(popup));

        assert!(app.surface(popup).is_none());
    }

    #[test]
    fn tooltip_popup_cursor_events_do_not_create_bar_surface() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let popup = test_wid();
        app.surfaces.insert_tooltip_popup(TooltipPopup {
            window: popup,
            target_window: test_wid(),
            target_module: ModuleId::new("clock"),
            tree: crate::surface::tooltip_popup::TooltipTree {
                segments: vec![],
                anchor: (0, 0),
                bar_size: (48, 1080),
                screen_size: (1920, 1080),
                bar_edge: crate::config::Edge::Left,
            },
        });

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::CursorMoved(
            popup,
            iced::Point::new(10.0, 20.0),
        ));

        assert!(app.surface(popup).is_none());
    }
}
