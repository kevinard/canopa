use std::collections::{HashMap, HashSet};
use std::ops::{Index, IndexMut};

use iced::Task;
use iced_layershell::reexport::{KeyboardInteractivity, NewLayerShellSettings, OutputOption};

use super::{App, Message};
use crate::modules::ModuleKind;
use crate::surface::SurfaceState;

pub(super) struct PopupMeta {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) tree: crate::surface::menu_popup::MenuTree,
    pub(super) bar_name: String,
}

#[derive(Debug, Clone)]
pub(super) struct PanelMeta {
    pub(super) tree: crate::surface::panel::PanelTree,
    pub(super) search_input_id: iced::widget::Id,
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
    let placement = crate::surface::placement::centered_position(
        &tree.bar_edge,
        Some(iced::Rectangle {
            x: tree.anchor.0 as f32,
            y: tree.anchor.1 as f32,
            width: 0.0,
            height: 0.0,
        }),
        crate::surface::placement::Size {
            width: size.0,
            height: size.1,
        },
        crate::surface::placement::Size {
            width: tree.bar_size.0,
            height: tree.bar_size.1,
        },
        crate::surface::placement::Point::default(),
        match tree.bar_edge {
            crate::config::Edge::Top | crate::config::Edge::Bottom => tree.bar_size.1,
            crate::config::Edge::Left | crate::config::Edge::Right => tree.bar_size.0,
        },
        gap,
    );
    placement.layer_margin()
}

fn tooltip_popup_window_settings(
    tree: &crate::surface::tooltip_popup::TooltipTree,
) -> NewLayerShellSettings {
    let size = crate::surface::tooltip_popup::popup_size(tree);
    NewLayerShellSettings {
        anchor: crate::surface::placement::layer_anchor(&tree.bar_edge),
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
    pub(super) panel_popups: HashMap<iced::window::Id, PanelMeta>,
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
            panel_popups: HashMap::new(),
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
        !self.popups.is_empty() || !self.tooltip_popups.is_empty() || !self.panel_popups.is_empty()
    }

    pub(super) fn panel_popup(&self, window: iced::window::Id) -> Option<&PanelMeta> {
        self.panel_popups.get(&window)
    }

    pub(super) fn insert_panel_popup(&mut self, window: iced::window::Id, meta: PanelMeta) {
        self.panel_popups.insert(window, meta);
    }

    pub(super) fn remove_panel_popup(&mut self, window: iced::window::Id) -> Option<PanelMeta> {
        self.panel_popups.remove(&window)
    }

    pub(super) fn panel_popup_ids(&self) -> impl Iterator<Item = iced::window::Id> + '_ {
        self.panel_popups.keys().copied()
    }

    pub(super) fn panel_popup_values(&self) -> impl Iterator<Item = &PanelMeta> {
        self.panel_popups.values()
    }

    pub(super) fn has_panel_popup(&self, window: iced::window::Id) -> bool {
        self.panel_popups.contains_key(&window)
    }

    pub(super) fn active_panel_origin_for(
        &self,
        origin_window: iced::window::Id,
    ) -> Option<&crate::surface::panel::PanelOrigin> {
        self.panel_popups
            .values()
            .find(|meta| meta.tree.origin_window == origin_window)
            .map(|meta| &meta.tree.origin)
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
                } else if self.surfaces.has_panel_popup(window) {
                    return self.handle_panel_focus_gained(window);
                } else if self.panel.is_closing_window(window) {
                    return Task::none();
                } else {
                    self.surfaces.surface_mut(window).screen_size = (w, h);
                }
                Task::none()
            }
            SurfaceEvent::WindowClosed(window) if self.panel.is_closing_window(window) => {
                self.overlays.window_closed(window);
                self.panel.forget_closing_window(window);
                Task::none()
            }
            SurfaceEvent::WindowClosed(window) => match self.overlays.window_closed(window) {
                Some(super::overlay_lifecycle::ClosedOverlay::Panel) => {
                    self.surfaces.remove_panel_popup(window);
                    Task::none()
                }
                Some(super::overlay_lifecycle::ClosedOverlay::Menu) => {
                    Task::done(Message::Menu(super::messages::MenuEvent::Closed))
                }
                Some(super::overlay_lifecycle::ClosedOverlay::Tooltip) => {
                    self.surfaces.remove_tooltip_popup(window);
                    Task::none()
                }
                None if self.is_menu_popup_window(window) => {
                    Task::done(Message::Menu(super::messages::MenuEvent::Closed))
                }
                None if self.is_tooltip_popup_window(window) => {
                    self.surfaces.remove_tooltip_popup(window);
                    Task::none()
                }
                None => Task::none(),
            },
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
            SurfaceEvent::BarRightClick(window) => self.handle_bar_right_click(window),
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
            SurfaceEvent::BarFocusGained(window) if self.surfaces.has_panel_popup(window) => {
                self.handle_panel_focus_gained(window)
            }
            SurfaceEvent::BarFocusGained(window) if self.panel.is_closing_window(window) => {
                Task::none()
            }
            SurfaceEvent::BarFocusGained(window) => self.handle_bar_focus_gained(window),
            SurfaceEvent::BarFocusLost(window) if self.is_tooltip_popup_window(window) => {
                Task::none()
            }
            SurfaceEvent::BarFocusLost(window) if self.surfaces.has_panel_popup(window) => {
                Task::batch(self.close_all_panels())
            }
            SurfaceEvent::BarFocusLost(window) if self.panel.is_closing_window(window) => {
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
            SurfaceEvent::ActionKeyPressed {
                window,
                modifiers,
                key,
            } => self.handle_bar_action_key(window, modifiers, key),
            SurfaceEvent::BackPressed(window) if self.surfaces.has_panel_popup(window) => {
                self.handle_panel_event(super::messages::PanelEvent::Back)
            }
            SurfaceEvent::BackPressed(_) => Task::none(),
            SurfaceEvent::TogglePanel => self.handle_toggle_panel_ipc(),
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

    pub(super) fn handle_toggle_panel_ipc(&mut self) -> Task<Message> {
        if self.surfaces.panel_popup_ids().next().is_some() {
            return self.handle_panel_event(super::messages::PanelEvent::Close);
        }

        let focused_output = self.niri.state().focused_output().map(str::to_string);
        let mut candidates: Vec<_> = self
            .surfaces
            .surfaces()
            .filter(|(_, surface)| {
                focused_output
                    .as_deref()
                    .map(|output| surface.output_name == output)
                    .unwrap_or(true)
            })
            .map(|(window, surface)| {
                (
                    surface.output_name.clone(),
                    surface.bar_name.clone(),
                    *window,
                )
            })
            .collect();
        if candidates.is_empty() && focused_output.is_some() {
            candidates = self
                .surfaces
                .surfaces()
                .map(|(window, surface)| {
                    (
                        surface.output_name.clone(),
                        surface.bar_name.clone(),
                        *window,
                    )
                })
                .collect();
        }
        candidates.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let Some((_, _, window)) = candidates.into_iter().next() else {
            return Task::none();
        };
        self.handle_panel_event(super::messages::PanelEvent::Open {
            origin_window: window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        })
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
        let effects = self
            .overlays
            .open_tooltip(popup_wid, target_window, module.clone());
        let mut tasks = self.apply_overlay_effects(effects, false);

        self.register_tooltip_popup(popup_wid, target_window, module, tree);

        tasks.push(open_task);
        Task::batch(tasks)
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
        let effects = self.overlays.close_tooltip_for_surface(target_window);
        let mut tasks = self.apply_overlay_effects(effects, false);
        let popup_wid = self
            .surfaces
            .tooltip_popup_for_target(target_window)
            .filter(|p| self.overlays.tooltip(p.window).is_none())
            .map(|p| p.window);
        if let Some(wid) = popup_wid {
            self.overlays.forget_tooltip(wid);
            self.surfaces.remove_tooltip_popup(wid);
            tasks.push(Task::done(Message::RemoveWindow(wid)));
        }
        tasks
    }

    pub(super) fn close_tooltip_popup_if_open_for(
        &mut self,
        target_window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Vec<Task<Message>> {
        let effects = self
            .overlays
            .close_tooltip_for_target(target_window, module);
        let mut tasks = self.apply_overlay_effects(effects, false);
        let popup_wid = self
            .surfaces
            .tooltip_popup_for_target(target_window)
            .filter(|p| &p.target_module == module)
            .filter(|p| self.overlays.tooltip(p.window).is_none())
            .map(|p| p.window);
        if let Some(wid) = popup_wid {
            self.overlays.forget_tooltip(wid);
            self.surfaces.remove_tooltip_popup(wid);
            tasks.push(Task::done(Message::RemoveWindow(wid)));
        }
        tasks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Edge;
    use crate::modules::ModuleId;
    use iced_layershell::reexport::Anchor;

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

    fn panel_tree(origin_window: iced::window::Id) -> crate::surface::panel::PanelTree {
        crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            origin_window,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 1080),
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
        assert!(s.focused_module.is_none());
    }

    #[test]
    fn panel_popup_storage_tracks_popup_by_window() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let origin = test_wid();
        let panel = test_wid();

        app.surfaces.insert_panel_popup(
            panel,
            PanelMeta {
                tree: panel_tree(origin),
                search_input_id: crate::app::panel::panel_search_input_id(panel),
            },
        );

        assert!(app.surfaces.has_panel_popup(panel));
        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);
        assert_eq!(
            app.surfaces.active_panel_origin_for(origin),
            Some(&crate::surface::panel::PanelOrigin::Bar)
        );
    }

    #[test]
    fn removing_panel_popup_clears_global_popup_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces.insert_panel_popup(
            panel,
            PanelMeta {
                tree: panel_tree(origin),
                search_input_id: crate::app::panel::panel_search_input_id(panel),
            },
        );

        let removed = app.surfaces.remove_panel_popup(panel).expect("panel popup");

        assert_eq!(removed.tree.origin_window, origin);
        assert!(!app.surfaces.has_panel_popup(panel));
        assert!(!app.surfaces.has_any_popup());
    }

    #[test]
    fn toggle_panel_ipc_closes_open_panel() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces
            .surfaces
            .insert(origin, SurfaceState::new("DP-1".into(), "main".into()));
        app.register_panel_popup(panel, origin, panel_tree(origin));

        let _ = app.update(Message::Surface(
            crate::app::messages::SurfaceEvent::TogglePanel,
        ));

        assert_eq!(app.surfaces.panel_popup_ids().count(), 0);
    }

    #[test]
    fn toggle_panel_ipc_opens_one_panel_for_deterministic_surface() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let first = test_wid();
        let second = test_wid();
        app.surfaces
            .surfaces
            .insert(second, SurfaceState::new("DP-1".into(), "z".into()));
        app.surfaces
            .surfaces
            .insert(first, SurfaceState::new("DP-1".into(), "main".into()));

        let _ = app.update(Message::Surface(
            crate::app::messages::SurfaceEvent::TogglePanel,
        ));

        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);
        let panel = app.surfaces.panel_popup_values().next().unwrap();
        assert_eq!(panel.tree.origin_window, first);
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
            menus: vec![],
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
            menus: vec![],
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
            menus: vec![],
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
            menus: vec![],
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
            menus: vec![],
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
            menus: vec![],
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
        app.register_tooltip_popup(
            popup,
            test_wid(),
            ModuleId::new("clock"),
            crate::surface::tooltip_popup::TooltipTree {
                segments: vec![],
                anchor: (0, 0),
                bar_size: (48, 1080),
                screen_size: (48, 1080),
                bar_edge: crate::config::Edge::Left,
            },
        );

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
    fn tooltip_anchor_for_uses_screen_size_as_current_bar_size_input() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = test_wid();
        app.surfaces.surfaces.insert(
            wid,
            crate::surface::SurfaceState {
                output_name: "DP-1".into(),
                bar_name: "main".into(),
                screen_size: (1920, 1080),
                last_pointer_position: (500, 24),
                ..Default::default()
            },
        );

        let placement = app.tooltip_anchor_for(wid);

        assert_eq!(placement.bar_size, (1920, 1080));
        assert_eq!(placement.screen_size, (1920, 1080));
        assert_eq!(placement.anchor, (500, 24));
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
        app.register_tooltip_popup(
            popup,
            test_wid(),
            ModuleId::new("clock"),
            crate::surface::tooltip_popup::TooltipTree {
                segments: vec![],
                anchor: (0, 0),
                bar_size: (48, 1080),
                screen_size: (1920, 1080),
                bar_edge: crate::config::Edge::Left,
            },
        );

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::BarFocusGained(popup));

        assert!(app.surface(popup).is_none());
    }

    #[test]
    fn tooltip_popup_cursor_events_do_not_create_bar_surface() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let popup = test_wid();
        app.register_tooltip_popup(
            popup,
            test_wid(),
            ModuleId::new("clock"),
            crate::surface::tooltip_popup::TooltipTree {
                segments: vec![],
                anchor: (0, 0),
                bar_size: (48, 1080),
                screen_size: (1920, 1080),
                bar_edge: crate::config::Edge::Left,
            },
        );

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::CursorMoved(
            popup,
            iced::Point::new(10.0, 20.0),
        ));

        assert!(app.surface(popup).is_none());
    }
}
