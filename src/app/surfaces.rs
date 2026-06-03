use std::collections::{HashMap, HashSet};
use std::ops::{Index, IndexMut};

use iced::Task;
use iced_layershell::reexport::{KeyboardInteractivity, NewLayerShellSettings, OutputOption};

use super::bar_surface_plan::PlannedBarSurface;
use super::panel_runtime::PanelAttachmentId;
use super::{App, Message};
use crate::modules::ModuleKind;
use crate::surface::SurfaceState;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct BarSurfaceIdentity {
    pub(super) bar_name: String,
    pub(super) output_name: Option<String>,
}

impl BarSurfaceIdentity {
    pub(super) fn new(bar_name: impl Into<String>, output_name: Option<String>) -> Self {
        let bar_name = bar_name.into();
        Self {
            bar_name: if bar_name.is_empty() {
                "main".into()
            } else {
                bar_name
            },
            output_name: output_name.filter(|output| !output.is_empty()),
        }
    }

    pub(super) fn all_screens() -> Self {
        Self::new("main", None)
    }

    pub(super) fn from_surface_state(surface: &SurfaceState) -> Self {
        Self::new(surface.bar_name.clone(), Some(surface.output_name.clone()))
    }

    pub(super) fn output_name_for_surface_state(&self) -> String {
        self.output_name.clone().unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BarSurfaceLifecycleEffect {
    Open(PlannedBarSurface),
    RemoveWindow(iced::window::Id),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PopupMeta {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) tree: crate::surface::menu_popup::MenuTree,
    pub(super) origin: BarSurfaceIdentity,
}

#[derive(Debug, Clone)]
pub(super) struct PanelMeta {
    pub(super) attachment_id: PanelAttachmentId,
    pub(super) origin_window: iced::window::Id,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum RegistryEffect {
    PanelClosed {
        window: iced::window::Id,
        attachment_id: PanelAttachmentId,
    },
    MenuClosed {
        window: iced::window::Id,
        meta: PopupMeta,
    },
    TooltipClosed {
        window: iced::window::Id,
    },
    PendingMenuReady(PopupMeta),
    SuspendBarKeyboardInteractivity,
    RestoreBarKeyboardInteractivity,
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
    pub(super) closing_panel_popups: HashSet<iced::window::Id>,
    pub(super) closing_menu_popups: HashMap<iced::window::Id, PopupMeta>,
    pub(super) pending_menu_popup: Option<PopupMeta>,
    pub(super) tooltip_popups: HashMap<iced::window::Id, TooltipPopup>,
    pub(super) panel_popups: HashMap<iced::window::Id, PanelMeta>,
    pub(super) last_cursor: Option<(iced::window::Id, iced::Point)>,
}

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
            closing_panel_popups: HashSet::new(),
            closing_menu_popups: HashMap::new(),
            pending_menu_popup: None,
            tooltip_popups: HashMap::new(),
            panel_popups: HashMap::new(),
            last_cursor: None,
        }
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.surfaces.len()
    }

    #[cfg(test)]
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

    pub(super) fn bar_surface_identity_for_window(
        &self,
        window: iced::window::Id,
    ) -> Option<BarSurfaceIdentity> {
        self.surfaces
            .get(&window)
            .map(BarSurfaceIdentity::from_surface_state)
    }

    #[cfg(test)]
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

    pub(super) fn reconcile_bar_surfaces(
        &mut self,
        desired: Vec<PlannedBarSurface>,
    ) -> Vec<BarSurfaceLifecycleEffect> {
        let live: HashMap<BarSurfaceIdentity, iced::window::Id> = self
            .surfaces
            .iter()
            .map(|(window, surface)| (BarSurfaceIdentity::from_surface_state(surface), *window))
            .collect();
        let desired_identities: HashSet<_> = desired
            .iter()
            .map(|surface| surface.identity.clone())
            .collect();

        let mut effects = Vec::new();
        effects.extend(
            desired
                .into_iter()
                .filter(|surface| !live.contains_key(&surface.identity))
                .map(BarSurfaceLifecycleEffect::Open),
        );

        let mut stale: Vec<_> = live
            .into_iter()
            .filter(|(identity, _)| !desired_identities.contains(identity))
            .collect();
        stale.sort_by_key(|(_, window)| format!("{window:?}"));
        for (identity, window) in stale {
            self.surfaces.remove(&window);
            effects.extend(
                self.close_overlays_for_bar_surface(window, &identity)
                    .into_iter()
                    .filter_map(|effect| match effect {
                        RegistryEffect::PanelClosed { window, .. }
                        | RegistryEffect::MenuClosed { window, .. }
                        | RegistryEffect::TooltipClosed { window } => {
                            Some(BarSurfaceLifecycleEffect::RemoveWindow(window))
                        }
                        RegistryEffect::SuspendBarKeyboardInteractivity
                        | RegistryEffect::RestoreBarKeyboardInteractivity
                        | RegistryEffect::PendingMenuReady(_) => None,
                    }),
            );
            effects.push(BarSurfaceLifecycleEffect::RemoveWindow(window));
        }
        effects
    }

    pub(super) fn select_global_panel_toggle_surface(
        &self,
        focused_output: Option<&str>,
    ) -> Option<iced::window::Id> {
        let mut candidates: Vec<_> = self
            .surfaces
            .iter()
            .filter(|(_, surface)| {
                focused_output
                    .map(|output| surface.output_name == output)
                    .unwrap_or(true)
            })
            .map(|(window, surface)| (BarSurfaceIdentity::from_surface_state(surface), *window))
            .collect();
        if candidates.is_empty() && focused_output.is_some() {
            candidates = self
                .surfaces
                .iter()
                .map(|(window, surface)| (BarSurfaceIdentity::from_surface_state(surface), *window))
                .collect();
        }
        candidates.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| format!("{:?}", a.1).cmp(&format!("{:?}", b.1)))
        });
        candidates.into_iter().map(|(_, window)| window).next()
    }

    fn close_overlays_for_bar_surface(
        &mut self,
        target_surface: iced::window::Id,
        identity: &BarSurfaceIdentity,
    ) -> Vec<RegistryEffect> {
        let mut effects = self.close_tooltips_for_surface(target_surface);
        let closing_panels: Vec<_> = self
            .panel_popups
            .iter()
            .filter_map(|(window, meta)| (meta.origin_window == target_surface).then_some(*window))
            .collect();
        for window in &closing_panels {
            if let Some(meta) = self.panel_popups.remove(window) {
                self.closing_panel_popups.insert(*window);
                effects.push(RegistryEffect::PanelClosed {
                    window: *window,
                    attachment_id: meta.attachment_id,
                });
            }
        }
        if !closing_panels.is_empty() {
            effects.push(RegistryEffect::RestoreBarKeyboardInteractivity);
        }
        let closing_menus: Vec<_> = self
            .popups
            .iter()
            .filter_map(|(window, meta)| (&meta.origin == identity).then_some(*window))
            .collect();
        for window in &closing_menus {
            if let Some(meta) = self.popups.remove(window) {
                self.closing_menu_popups.insert(*window, meta);
            }
        }
        effects.extend(closing_menus.into_iter().filter_map(|window| {
            self.closing_menu_popups
                .get(&window)
                .cloned()
                .map(|meta| RegistryEffect::MenuClosed { window, meta })
        }));
        effects
    }

    pub(super) fn popup(&self, id: iced::window::Id) -> Option<&PopupMeta> {
        self.popups.get(&id)
    }

    pub(super) fn popup_values(&self) -> impl Iterator<Item = &PopupMeta> {
        self.popups.values()
    }

    #[cfg(test)]
    pub(super) fn insert_popup(&mut self, id: iced::window::Id, meta: PopupMeta) {
        self.popups.insert(id, meta);
    }

    pub(super) fn remove_popup(&mut self, id: iced::window::Id) -> Option<PopupMeta> {
        self.popups.remove(&id)
    }

    pub(super) fn has_popup(&self, id: iced::window::Id) -> bool {
        self.popups.contains_key(&id) || self.closing_menu_popups.contains_key(&id)
    }

    pub(super) fn has_any_popup(&self) -> bool {
        !self.popups.is_empty()
            || !self.closing_panel_popups.is_empty()
            || !self.closing_menu_popups.is_empty()
            || !self.tooltip_popups.is_empty()
            || !self.panel_popups.is_empty()
    }

    pub(super) fn has_any_menu_popup(&self) -> bool {
        !self.popups.is_empty() || !self.closing_menu_popups.is_empty()
    }

    pub(super) fn panel_popup(&self, window: iced::window::Id) -> Option<&PanelMeta> {
        self.panel_popups.get(&window)
    }

    #[cfg(test)]
    pub(super) fn insert_panel_popup(&mut self, window: iced::window::Id, meta: PanelMeta) {
        self.panel_popups.insert(window, meta);
    }

    pub(super) fn remove_panel_popup(&mut self, window: iced::window::Id) -> Option<PanelMeta> {
        self.panel_popups.remove(&window)
    }

    pub(super) fn close_panel_popup_window(&mut self, window: iced::window::Id) -> bool {
        let removed = self.panel_popups.remove(&window).is_some();
        if removed {
            self.closing_panel_popups.insert(window);
        }
        removed
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

    pub(super) fn tooltip_popup(&self, window: iced::window::Id) -> Option<&TooltipPopup> {
        self.tooltip_popups.get(&window)
    }

    pub(super) fn tooltip_popup_mut(
        &mut self,
        window: iced::window::Id,
    ) -> Option<&mut TooltipPopup> {
        self.tooltip_popups.get_mut(&window)
    }

    #[cfg(test)]
    pub(super) fn insert_tooltip_popup(&mut self, popup: TooltipPopup) {
        self.tooltip_popups.insert(popup.window, popup);
    }

    pub(super) fn remove_tooltip_popup(
        &mut self,
        window: iced::window::Id,
    ) -> Option<TooltipPopup> {
        self.tooltip_popups.remove(&window)
    }

    pub(super) fn open_panel_overlay(
        &mut self,
        origin_surface: iced::window::Id,
        panel_window: iced::window::Id,
        meta: PanelMeta,
    ) -> Vec<RegistryEffect> {
        self.pending_menu_popup = None;
        let mut effects = self.close_menu_overlays();
        effects.extend(self.close_tooltips_for_surface(origin_surface));
        effects.extend(self.close_panel_overlays_without_restore());
        self.panel_popups.insert(panel_window, meta);
        effects.push(RegistryEffect::SuspendBarKeyboardInteractivity);
        effects
    }

    pub(super) fn open_menu_overlay(
        &mut self,
        menu_window: iced::window::Id,
        meta: PopupMeta,
    ) -> Vec<RegistryEffect> {
        let mut effects = self.close_tooltip_overlays();
        effects.extend(self.close_panel_overlays_without_restore());
        if self.has_any_menu_popup() {
            self.pending_menu_popup = Some(meta);
            effects.extend(self.close_menu_overlays());
            return effects;
        }

        self.popups.insert(menu_window, meta);
        effects
    }

    pub(super) fn clear_pending_menu_popup(&mut self) {
        self.pending_menu_popup = None;
    }

    #[cfg(test)]
    pub(super) fn defer_menu_for_test(&mut self, meta: PopupMeta) {
        self.pending_menu_popup = Some(meta);
    }

    #[cfg(test)]
    pub(super) fn has_pending_menu_for_test(&self) -> bool {
        self.pending_menu_popup.is_some()
    }

    pub(super) fn take_ready_pending_menu_popup(&mut self) -> Option<PopupMeta> {
        if self.has_any_menu_popup() {
            return None;
        }
        self.pending_menu_popup.take()
    }

    pub(super) fn register_ready_menu_overlay(
        &mut self,
        menu_window: iced::window::Id,
        meta: PopupMeta,
    ) {
        self.popups.insert(menu_window, meta);
    }

    pub(super) fn open_tooltip_overlay(&mut self, popup: TooltipPopup) -> Vec<RegistryEffect> {
        let effects = self.close_tooltips_for_surface(popup.target_window);
        self.tooltip_popups.insert(popup.window, popup);
        effects
    }

    pub(super) fn close_panel_overlays(&mut self) -> Vec<RegistryEffect> {
        let mut effects = self.close_panel_overlays_without_restore();
        if !effects.is_empty() {
            effects.push(RegistryEffect::RestoreBarKeyboardInteractivity);
        }
        effects
    }

    fn close_panel_overlays_without_restore(&mut self) -> Vec<RegistryEffect> {
        self.panel_popups
            .drain()
            .map(|(window, meta)| {
                self.closing_panel_popups.insert(window);
                RegistryEffect::PanelClosed {
                    window,
                    attachment_id: meta.attachment_id,
                }
            })
            .collect()
    }

    pub(super) fn close_menu_overlays(&mut self) -> Vec<RegistryEffect> {
        self.popups
            .drain()
            .map(|(window, meta)| {
                self.closing_menu_popups.insert(window, meta.clone());
                RegistryEffect::MenuClosed { window, meta }
            })
            .collect()
    }

    pub(super) fn close_tooltip_for_surface(
        &mut self,
        target_surface: iced::window::Id,
    ) -> Vec<RegistryEffect> {
        self.close_tooltips_for_surface(target_surface)
    }

    pub(super) fn close_tooltip_for_target(
        &mut self,
        target_surface: iced::window::Id,
        target_module: &crate::modules::ModuleId,
    ) -> Vec<RegistryEffect> {
        let closing: Vec<_> = self
            .tooltip_popups
            .iter()
            .filter_map(|(window, popup)| {
                (popup.target_window == target_surface && &popup.target_module == target_module)
                    .then_some(*window)
            })
            .collect();
        for window in &closing {
            self.tooltip_popups.remove(window);
        }
        closing
            .into_iter()
            .map(|window| RegistryEffect::TooltipClosed { window })
            .collect()
    }

    pub(super) fn window_closed(&mut self, window: iced::window::Id) -> Vec<RegistryEffect> {
        if let Some(meta) = self.panel_popups.remove(&window) {
            return vec![
                RegistryEffect::PanelClosed {
                    window,
                    attachment_id: meta.attachment_id,
                },
                RegistryEffect::RestoreBarKeyboardInteractivity,
            ];
        }
        if let Some(meta) = self.popups.remove(&window) {
            return vec![RegistryEffect::MenuClosed { window, meta }];
        }
        if self.closing_panel_popups.remove(&window) {
            return Vec::new();
        }
        if self.closing_menu_popups.remove(&window).is_some() {
            return self
                .take_ready_pending_menu_popup()
                .map(RegistryEffect::PendingMenuReady)
                .into_iter()
                .collect();
        }
        if self.tooltip_popups.remove(&window).is_some() {
            return vec![RegistryEffect::TooltipClosed { window }];
        }
        Vec::new()
    }

    fn close_tooltip_overlays(&mut self) -> Vec<RegistryEffect> {
        self.tooltip_popups
            .drain()
            .map(|(window, _)| RegistryEffect::TooltipClosed { window })
            .collect()
    }

    fn close_tooltips_for_surface(
        &mut self,
        target_surface: iced::window::Id,
    ) -> Vec<RegistryEffect> {
        let closing: Vec<_> = self
            .tooltip_popups
            .iter()
            .filter_map(|(window, popup)| {
                (popup.target_window == target_surface).then_some(*window)
            })
            .collect();
        for window in &closing {
            self.tooltip_popups.remove(window);
        }
        closing
            .into_iter()
            .map(|window| RegistryEffect::TooltipClosed { window })
            .collect()
    }

    pub(super) fn window_role(
        &self,
        window: Option<iced::window::Id>,
    ) -> super::surface_input_router::WindowRole {
        let Some(window) = window else {
            return super::surface_input_router::WindowRole::Unknown;
        };

        if self.closing_panel_popups.contains(&window) {
            return super::surface_input_router::WindowRole::ClosingPanel;
        }
        if self.tooltip_popups.contains_key(&window) {
            return super::surface_input_router::WindowRole::Tooltip;
        }
        if self.panel_popups.contains_key(&window) {
            return super::surface_input_router::WindowRole::Panel;
        }
        if self.popups.contains_key(&window) || self.closing_menu_popups.contains_key(&window) {
            return super::surface_input_router::WindowRole::Menu;
        }
        if self.surfaces.contains_key(&window) {
            return super::surface_input_router::WindowRole::BarSurface;
        }
        super::surface_input_router::WindowRole::Unknown
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
    fn window_role_for_surface_input(
        &self,
        window: Option<iced::window::Id>,
    ) -> super::surface_input_router::WindowRole {
        self.surfaces.window_role(window)
    }

    pub(super) fn handle_surface_event(
        &mut self,
        event: super::messages::SurfaceEvent,
    ) -> Task<Message> {
        let effect = super::surface_input_router::route_for_role(
            |window| self.window_role_for_surface_input(window),
            &event,
        );
        self.apply_surface_input_effect(effect)
    }

    fn apply_surface_input_effect(
        &mut self,
        effect: super::surface_input_router::SurfaceInputEffect,
    ) -> Task<Message> {
        use super::surface_input_router::SurfaceInputEffect;

        match effect {
            SurfaceInputEffect::TrackCursor { window, position } => {
                let has_tray = self.modules.iter().any(|(_, module)| {
                    matches!(module.kind(), ModuleKind::Tray | ModuleKind::TrayIcon)
                });
                if has_tray && self.surfaces.last_cursor() != Some((window, position)) {
                    self.surfaces.set_last_cursor(Some((window, position)));
                }
                self.surface_mut(window).last_pointer_position =
                    (position.x as i32, position.y as i32);
                Task::none()
            }
            SurfaceInputEffect::TooltipResized {
                window,
                width,
                height,
            } => {
                if let Some(popup) = self.surfaces.tooltip_popup_mut(window) {
                    popup.tree.screen_size = (width, height);
                }
                Task::none()
            }
            SurfaceInputEffect::BarResized {
                window,
                width,
                height,
            } => {
                self.surfaces.surface_mut(window).screen_size = (width, height);
                Task::none()
            }
            SurfaceInputEffect::WindowClosed(window)
            | SurfaceInputEffect::ClosingPanelWindowClosed(window)
            | SurfaceInputEffect::MenuWindowClosed(window) => {
                let effects = self.surfaces.window_closed(window);
                Task::batch(self.apply_registry_effects(effects))
            }
            SurfaceInputEffect::ModuleHoverEnter(id, window) => {
                self.handle_module_hover_enter(id, window)
            }
            SurfaceInputEffect::ModuleHoverExit(id, window) => {
                self.handle_module_hover_exit(id, window)
            }
            SurfaceInputEffect::TooltipDwellElapsed { window, module } => {
                self.handle_tooltip_dwell_elapsed(window, module)
            }
            SurfaceInputEffect::CloseTooltip(window) => {
                Task::batch(self.close_tooltip_popup_if_open(window))
            }
            SurfaceInputEffect::BarClick(window) => self.handle_bar_click(window),
            SurfaceInputEffect::BarRightClick(window) => self.handle_bar_right_click(window),
            SurfaceInputEffect::BarHoverEnter(window) => self.handle_bar_hover_enter(window),
            SurfaceInputEffect::BarHoverExit(window) => self.handle_bar_hover_exit(window),
            SurfaceInputEffect::Tick => self.handle_tick(),
            SurfaceInputEffect::PanelFocusGained(window) => self.handle_panel_focus_gained(window),
            SurfaceInputEffect::BarFocusGained(window) => self.handle_bar_focus_gained(window),
            SurfaceInputEffect::CloseMenu => {
                Task::done(Message::Menu(super::messages::MenuEvent::Closed))
            }
            SurfaceInputEffect::BarFocusLost(window) => self.handle_bar_focus_lost(window),
            SurfaceInputEffect::Escape { window } => self.handle_escape_pressed(window),
            SurfaceInputEffect::MoveFocus { window, forward } => {
                self.handle_focus_move(window, forward)
            }
            SurfaceInputEffect::BarFocusZone { window, forward } => {
                self.handle_focus_zone(window, forward)
            }
            SurfaceInputEffect::KeyboardActivate { window } => {
                self.handle_keyboard_activate(window)
            }
            SurfaceInputEffect::KeyboardSecondaryActivate { window } => {
                self.handle_keyboard_secondary_activate(window)
            }
            SurfaceInputEffect::BarActionKey {
                window,
                modifiers,
                key,
            } => self.handle_bar_action_key(window, modifiers, key),
            SurfaceInputEffect::PanelBack => {
                self.handle_panel_event(super::messages::PanelEvent::Back)
            }
            SurfaceInputEffect::TogglePanel => self.handle_toggle_panel_ipc(),
            SurfaceInputEffect::Ignore => Task::none(),
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

        let focused_output = self.niri.state().focused_output();
        let Some(window) = self
            .surfaces
            .select_global_panel_toggle_surface(focused_output)
        else {
            return Task::none();
        };
        self.handle_panel_event(super::messages::PanelEvent::Open {
            origin_window: window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        })
    }

    pub(super) fn lifecycle_effects_to_tasks(
        config: &crate::config::Config,
        surfaces: &mut SurfaceManager,
        effects: Vec<BarSurfaceLifecycleEffect>,
    ) -> Vec<Task<Message>> {
        let mut tasks = Vec::new();
        for effect in effects {
            match effect {
                BarSurfaceLifecycleEffect::Open(desired) => {
                    let bar = &config.bars[desired.bar_index];
                    let output_name = desired.identity.output_name_for_surface_state();
                    tracing::debug!(
                        target: "niribar::surface",
                        bar = bar.name.as_str(),
                        output = output_name.as_str(),
                        "opening bar surface"
                    );
                    tasks.push(Self::spawn_bar_on_output(
                        bar,
                        &output_name,
                        &mut surfaces.pending_surface_keys,
                        &mut surfaces.surfaces,
                    ));
                }
                BarSurfaceLifecycleEffect::RemoveWindow(window) => {
                    tracing::debug!(
                        target: "niribar::surface",
                        ?window,
                        "removing bar surface"
                    );
                    tasks.push(Task::done(Message::RemoveWindow(window)));
                }
            }
        }
        tasks
    }

    fn apply_bar_surface_lifecycle_effects(
        &mut self,
        effects: Vec<BarSurfaceLifecycleEffect>,
    ) -> Vec<Task<Message>> {
        let mut tasks = Vec::new();
        for effect in effects {
            match effect {
                BarSurfaceLifecycleEffect::Open(desired) => {
                    let bar = &self.config.bars[desired.bar_index];
                    let output_name = desired.identity.output_name_for_surface_state();
                    tracing::debug!(
                        target: "niribar::surface",
                        bar = bar.name.as_str(),
                        output = output_name.as_str(),
                        "opening bar surface"
                    );
                    tasks.push(Self::spawn_bar_on_output(
                        bar,
                        &output_name,
                        &mut self.surfaces.pending_surface_keys,
                        &mut self.surfaces.surfaces,
                    ));
                }
                BarSurfaceLifecycleEffect::RemoveWindow(window) => {
                    tracing::debug!(
                        target: "niribar::surface",
                        ?window,
                        "removing bar surface"
                    );
                    tasks.push(Task::done(Message::RemoveWindow(window)));
                }
            }
        }
        tasks
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
    pub(super) fn open_new_output_windows_from_observed_outputs(
        &mut self,
        observed_outputs: Option<Vec<String>>,
    ) -> Vec<Task<Message>> {
        match super::eligible_outputs::resolve_eligible_outputs(observed_outputs, &[]) {
            super::eligible_outputs::EligibleOutputState::Unknown => Vec::new(),
            super::eligible_outputs::EligibleOutputState::Known(eligible_outputs) => {
                let desired = super::bar_surface_plan::plan_bar_surfaces(
                    &self.config.bars,
                    &super::eligible_outputs::EligibleOutputState::Known(eligible_outputs),
                );
                let effects = self.surfaces.reconcile_bar_surfaces(desired);
                self.apply_bar_surface_lifecycle_effects(effects)
            }
        }
    }

    pub(super) fn open_new_output_windows(
        &mut self,
        workspaces: &[niri_ipc::Workspace],
    ) -> Vec<Task<Message>> {
        let observed_outputs: Vec<String> = workspaces
            .iter()
            .filter_map(|ws| ws.output.clone())
            .collect();
        self.open_new_output_windows_from_observed_outputs(Some(observed_outputs))
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

    pub(super) fn bar_for_surface_identity<'a>(
        bars: &'a [crate::config::BarConfig],
        identity: &BarSurfaceIdentity,
    ) -> &'a crate::config::BarConfig {
        Self::bar_for_surface_name(bars, &identity.bar_name)
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
        if self.panel.active_panel_origin_for(target_window).is_some() {
            self.surface_mut(target_window).tooltip_pending = None;
            return Task::none();
        }

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
        let effects = self.surfaces.open_tooltip_overlay(TooltipPopup {
            window: popup_wid,
            target_window,
            target_module: module,
            tree,
        });
        let mut tasks = self.apply_registry_effects(effects);

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
        let effects = self.surfaces.close_tooltip_for_surface(target_window);
        self.apply_registry_effects(effects)
    }

    pub(super) fn close_tooltip_popup_if_open_for(
        &mut self,
        target_window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Vec<Task<Message>> {
        let effects = self
            .surfaces
            .close_tooltip_for_target(target_window, module);
        self.apply_registry_effects(effects)
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

    fn panel_meta_for_test(
        attachment_id: super::super::panel_runtime::PanelAttachmentId,
        origin_window: iced::window::Id,
    ) -> PanelMeta {
        PanelMeta {
            attachment_id,
            origin_window,
        }
    }

    fn attachment_id(value: u64) -> super::super::panel_runtime::PanelAttachmentId {
        super::super::panel_runtime::PanelAttachmentId::for_test(value)
    }

    fn popup_meta_for_identity(identity: BarSurfaceIdentity, service: &str) -> PopupMeta {
        PopupMeta {
            service: service.into(),
            menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
            tree: crate::surface::menu_popup::MenuTree {
                items: Vec::new(),
                anchor: (0, 0),
                screen_size: (1920, 1080),
            },
            origin: identity,
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

        app.surfaces
            .insert_panel_popup(panel, panel_meta_for_test(attachment_id(1), origin));

        assert!(app.surfaces.has_panel_popup(panel));
        assert_eq!(app.surfaces.panel_popup_ids().count(), 1);
        let meta = app.surfaces.panel_popup(panel).expect("panel popup");
        assert_eq!(meta.attachment_id, attachment_id(1));
        assert_eq!(meta.origin_window, origin);
    }

    #[test]
    fn panel_closed_effect_carries_attachment_id() {
        let origin = test_wid();
        let panel = test_wid();
        let attachment_id = attachment_id(7);
        let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new(), true);
        registry.insert_panel_popup(panel, panel_meta_for_test(attachment_id, origin));

        let effects = registry.window_closed(panel);

        assert_eq!(
            effects,
            vec![
                RegistryEffect::PanelClosed {
                    window: panel,
                    attachment_id,
                },
                RegistryEffect::RestoreBarKeyboardInteractivity,
            ]
        );
    }

    #[test]
    fn registry_opening_panel_replaces_existing_overlays() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let origin = test_wid();
        let menu = test_wid();
        let tooltip = test_wid();
        let panel = test_wid();
        app.surfaces
            .surfaces
            .insert(origin, SurfaceState::new("DP-1".into(), "main".into()));
        app.register_menu_popup(
            menu,
            PopupMeta {
                service: "org.test".into(),
                menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
                tree: crate::surface::menu_popup::MenuTree {
                    items: Vec::new(),
                    anchor: (0, 0),
                    screen_size: (1920, 1080),
                },
                origin: BarSurfaceIdentity::new("main", Some("DP-1".into())),
            },
        );
        app.register_tooltip_popup(
            tooltip,
            origin,
            ModuleId::new("clock"),
            tooltip_tree(crate::config::Edge::Top),
        );

        let effects = app.surfaces.open_panel_overlay(
            origin,
            panel,
            panel_meta_for_test(attachment_id(2), origin),
        );

        assert_eq!(
            effects,
            vec![
                super::RegistryEffect::MenuClosed {
                    window: menu,
                    meta: PopupMeta {
                        service: "org.test".into(),
                        menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
                        tree: crate::surface::menu_popup::MenuTree {
                            items: Vec::new(),
                            anchor: (0, 0),
                            screen_size: (1920, 1080),
                        },
                        origin: BarSurfaceIdentity::new("main", Some("DP-1".into())),
                    },
                },
                super::RegistryEffect::TooltipClosed { window: tooltip },
                super::RegistryEffect::SuspendBarKeyboardInteractivity,
            ]
        );
        assert!(app.surfaces.panel_popup(panel).is_some());
        assert!(app.surfaces.popup(menu).is_none());
        assert!(app.surfaces.tooltip_popup(tooltip).is_none());
    }

    #[test]
    fn opening_panel_closes_menu_and_tooltip_discards_pending_menu_and_suspends_keyboard() {
        let origin = test_wid();
        let panel = test_wid();
        let old_menu = test_wid();
        let tooltip = test_wid();
        let identity = BarSurfaceIdentity::new("main", Some("DP-1".into()));
        let mut registry = SurfaceManager::new(
            [(origin, SurfaceState::new("DP-1".into(), "main".into()))].into(),
            HashMap::new(),
            true,
        );
        registry.insert_popup(
            old_menu,
            popup_meta_for_identity(identity.clone(), "org.old"),
        );
        registry.insert_tooltip_popup(TooltipPopup {
            window: tooltip,
            target_window: origin,
            target_module: crate::modules::ModuleId::new("clock"),
            tree: tooltip_tree(crate::config::Edge::Top),
        });
        registry.defer_menu_for_test(popup_meta_for_identity(identity, "org.pending"));

        let effects = registry.open_panel_overlay(
            origin,
            panel,
            panel_meta_for_test(attachment_id(3), origin),
        );

        assert!(registry.popup(old_menu).is_none());
        assert!(registry.tooltip_popup(tooltip).is_none());
        assert!(!registry.has_pending_menu_for_test());
        assert_eq!(
            effects,
            vec![
                RegistryEffect::MenuClosed {
                    window: old_menu,
                    meta: popup_meta_for_identity(
                        BarSurfaceIdentity::new("main", Some("DP-1".into())),
                        "org.old"
                    ),
                },
                RegistryEffect::TooltipClosed { window: tooltip },
                RegistryEffect::SuspendBarKeyboardInteractivity,
            ]
        );
        assert!(registry.panel_popup(panel).is_some());
    }

    #[test]
    fn opening_menu_while_menu_is_live_defers_replacement_until_old_window_closes() {
        let old_menu = test_wid();
        let identity = BarSurfaceIdentity::new("main", Some("DP-1".into()));
        let old_meta = popup_meta_for_identity(identity.clone(), "org.old");
        let new_meta = popup_meta_for_identity(identity, "org.new");
        let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new(), true);
        registry.insert_popup(old_menu, old_meta.clone());

        let effects = registry.open_menu_overlay(test_wid(), new_meta.clone());

        assert!(registry.popup(old_menu).is_none());
        assert!(registry.has_pending_menu_for_test());
        assert_eq!(
            effects,
            vec![RegistryEffect::MenuClosed {
                window: old_menu,
                meta: old_meta,
            }]
        );

        let followup = registry.window_closed(old_menu);

        assert_eq!(followup, vec![RegistryEffect::PendingMenuReady(new_meta)]);
        assert!(!registry.has_pending_menu_for_test());
    }

    #[test]
    fn removing_bar_surface_closes_only_overlays_for_that_bar_surface_identity() {
        let removed_window = test_wid();
        let kept_window = test_wid();
        let removed_menu = test_wid();
        let kept_menu = test_wid();
        let mut registry = SurfaceManager::new(
            [
                (
                    removed_window,
                    SurfaceState::new("DP-1".into(), "main".into()),
                ),
                (
                    kept_window,
                    SurfaceState::new("HDMI-A-1".into(), "main".into()),
                ),
            ]
            .into(),
            HashMap::new(),
            true,
        );
        registry.insert_popup(
            removed_menu,
            popup_meta_for_identity(
                BarSurfaceIdentity::new("main", Some("DP-1".into())),
                "org.removed",
            ),
        );
        registry.insert_popup(
            kept_menu,
            popup_meta_for_identity(
                BarSurfaceIdentity::new("main", Some("HDMI-A-1".into())),
                "org.kept",
            ),
        );

        let effects = registry.reconcile_bar_surfaces(vec![PlannedBarSurface {
            identity: BarSurfaceIdentity::new("main", Some("HDMI-A-1".into())),
            bar_index: 0,
        }]);

        assert!(effects.contains(&BarSurfaceLifecycleEffect::RemoveWindow(removed_window)));
        assert!(registry.popup(removed_menu).is_none());
        assert!(registry.popup(kept_menu).is_some());
    }

    #[test]
    fn registry_classifies_closing_panel_windows() {
        let window = test_wid();
        let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new(), true);
        registry.closing_panel_popups.insert(window);

        assert_eq!(
            registry.window_role(Some(window)),
            crate::app::surface_input_router::WindowRole::ClosingPanel
        );
    }

    #[test]
    fn registry_classifies_closing_menu_windows_as_menu_for_input_routing() {
        let window = test_wid();
        let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new(), true);
        registry.closing_menu_popups.insert(
            window,
            popup_meta_for_identity(BarSurfaceIdentity::all_screens(), "org.test"),
        );

        assert_eq!(
            registry.window_role(Some(window)),
            crate::app::surface_input_router::WindowRole::Menu
        );
    }

    #[test]
    fn registry_classifies_missing_and_bar_surface_windows() {
        let window = test_wid();
        let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new(), true);

        assert_eq!(
            registry.window_role(None),
            crate::app::surface_input_router::WindowRole::Unknown
        );
        assert_eq!(
            registry.window_role(Some(window)),
            crate::app::surface_input_router::WindowRole::Unknown
        );

        registry
            .surfaces
            .insert(window, SurfaceState::new("DP-1".into(), "main".into()));
        assert_eq!(
            registry.window_role(Some(window)),
            crate::app::surface_input_router::WindowRole::BarSurface
        );
    }

    #[test]
    fn registry_prefers_tooltip_over_panel_and_menu_windows() {
        let window = test_wid();
        let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new(), true);
        registry.insert_popup(
            window,
            popup_meta_for_identity(BarSurfaceIdentity::all_screens(), "org.test"),
        );
        registry.insert_panel_popup(window, panel_meta_for_test(attachment_id(4), test_wid()));
        registry.insert_tooltip_popup(TooltipPopup {
            window,
            target_window: test_wid(),
            target_module: crate::modules::ModuleId::new("clock"),
            tree: tooltip_tree(crate::config::Edge::Top),
        });

        assert_eq!(
            registry.window_role(Some(window)),
            crate::app::surface_input_router::WindowRole::Tooltip
        );
    }

    #[test]
    fn removing_panel_popup_clears_global_popup_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces
            .insert_panel_popup(panel, panel_meta_for_test(attachment_id(5), origin));

        let removed = app.surfaces.remove_panel_popup(panel).expect("panel popup");

        assert_eq!(removed.origin_window, origin);
        assert_eq!(removed.attachment_id, attachment_id(5));
        assert!(!app.surfaces.has_panel_popup(panel));
        assert!(!app.surfaces.has_any_popup());
    }

    #[test]
    fn live_panel_window_closed_restores_bar_keyboard_interactivity_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces
            .surfaces
            .insert(origin, SurfaceState::new("DP-1".into(), "main".into()));
        app.surface_mut(origin).focused_module = Some(ModuleId::new("clock"));
        app.surfaces
            .insert_panel_popup(panel, panel_meta_for_test(attachment_id(6), origin));

        let _ = app.handle_surface_event(crate::app::messages::SurfaceEvent::WindowClosed(panel));

        assert!(app.surfaces.panel_popup(panel).is_none());
        assert!(app.surface(origin).unwrap().focused_module.is_none());
    }

    #[test]
    fn toggle_panel_ipc_closes_open_panel() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces
            .surfaces
            .insert(origin, SurfaceState::new("DP-1".into(), "main".into()));
        app.register_panel_popup(panel, origin, attachment_id(7));

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
        assert_eq!(panel.origin_window, first);
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
    fn bar_surface_identity_uses_implicit_bar_name_for_allscreens() {
        let identity = BarSurfaceIdentity::from_surface_state(&SurfaceState::default());

        assert_eq!(identity, BarSurfaceIdentity::all_screens());
        assert_eq!(identity.bar_name, "main");
        assert_eq!(identity.output_name, None);
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
    fn boot_with_known_empty_outputs_uses_manual_mode() {
        let (app, _) = App::boot_with_eligible_outputs(
            make_config(),
            crate::app::eligible_outputs::EligibleOutputState::Known(Vec::new()),
        );

        assert!(app.surfaces.manual_outputs());
        assert!(app.surfaces.is_empty());
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
    fn registry_reconciliation_keeps_live_surface_and_opens_missing_surface() {
        let existing = test_wid();
        let mut registry = SurfaceManager::new(
            HashMap::from([(existing, SurfaceState::new("DP-1".into(), "main".into()))]),
            HashMap::new(),
            true,
        );
        let desired = vec![
            PlannedBarSurface {
                identity: BarSurfaceIdentity::new("main", Some("DP-1".into())),
                bar_index: 0,
            },
            PlannedBarSurface {
                identity: BarSurfaceIdentity::new("side", Some("DP-1".into())),
                bar_index: 1,
            },
        ];

        let effects = registry.reconcile_bar_surfaces(desired);

        assert_eq!(
            effects,
            vec![BarSurfaceLifecycleEffect::Open(PlannedBarSurface {
                identity: BarSurfaceIdentity::new("side", Some("DP-1".into())),
                bar_index: 1,
            })]
        );
        assert!(registry.surface(existing).is_some());
    }

    #[test]
    fn registry_reconciliation_closes_stale_surface_and_associated_overlays() {
        let stale = test_wid();
        let tooltip = test_wid();
        let panel = test_wid();
        let menu = test_wid();
        let mut registry = SurfaceManager::new(
            HashMap::from([(stale, SurfaceState::new("DP-1".into(), "main".into()))]),
            HashMap::new(),
            true,
        );
        registry.insert_tooltip_popup(TooltipPopup {
            window: tooltip,
            target_window: stale,
            target_module: ModuleId::new("clock"),
            tree: tooltip_tree(crate::config::Edge::Top),
        });
        registry.insert_panel_popup(panel, panel_meta_for_test(attachment_id(8), stale));
        registry.insert_popup(
            menu,
            PopupMeta {
                service: "org.test".into(),
                menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
                tree: crate::surface::menu_popup::MenuTree {
                    items: Vec::new(),
                    anchor: (0, 0),
                    screen_size: (1920, 1080),
                },
                origin: BarSurfaceIdentity::new("main", Some("DP-1".into())),
            },
        );

        let effects = registry.reconcile_bar_surfaces(Vec::new());

        assert_eq!(
            effects,
            vec![
                BarSurfaceLifecycleEffect::RemoveWindow(tooltip),
                BarSurfaceLifecycleEffect::RemoveWindow(panel),
                BarSurfaceLifecycleEffect::RemoveWindow(menu),
                BarSurfaceLifecycleEffect::RemoveWindow(stale),
            ]
        );
        assert!(registry.surface(stale).is_none());
        assert!(registry.tooltip_popup(tooltip).is_none());
        assert!(registry.panel_popup(panel).is_none());
        assert!(registry.popup(menu).is_none());
    }

    #[test]
    fn registry_global_panel_toggle_prefers_focused_output_then_identity_order() {
        let first = test_wid();
        let focused_main = test_wid();
        let focused_side = test_wid();
        let registry = SurfaceManager::new(
            HashMap::from([
                (first, SurfaceState::new("DP-1".into(), "main".into())),
                (
                    focused_side,
                    SurfaceState::new("eDP-1".into(), "side".into()),
                ),
                (
                    focused_main,
                    SurfaceState::new("eDP-1".into(), "main".into()),
                ),
            ]),
            HashMap::new(),
            true,
        );

        assert_eq!(
            registry.select_global_panel_toggle_surface(Some("eDP-1")),
            Some(focused_main)
        );
        assert_eq!(
            registry.select_global_panel_toggle_surface(Some("missing")),
            Some(first)
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
    fn unknown_output_observation_does_not_reconcile_existing_surfaces() {
        let (mut app, _) = App::boot(make_config(), vec!["eDP-1".into()]);
        let surface_count = app.surfaces.len();

        let tasks = app.open_new_output_windows_from_observed_outputs(None);

        assert!(tasks.is_empty());
        assert_eq!(app.surfaces.len(), surface_count);
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
    fn tooltip_dwell_does_not_open_while_panel_active_for_origin() {
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
        let origin = test_wid();
        let panel = test_wid();
        let module = ModuleId::new("battery");
        app.modules.set_state(
            module.clone(),
            crate::modules::ModuleState::Battery(crate::modules::battery::BatteryState {
                percentage: 74.0,
                is_charging: false,
            }),
        );
        app.surface_mut(origin).hovered_module = Some(module.clone());
        app.surface_mut(origin).tooltip_pending = Some((module.clone(), 32.0));
        let request = app.panel_open_request(
            origin,
            crate::surface::panel::PanelOrigin::Module(module.clone()),
            crate::surface::panel::PanelContent::Menu("main".into()),
            None,
        );
        let effects = app
            .panel
            .handle(super::super::panel_runtime::PanelRuntimeIntent::Open {
                request,
                catalog: super::super::panel_runtime::PanelMenuCatalog::new(
                    app.config.menus.as_slice(),
                ),
                module_detail_sessions: HashMap::new(),
            });
        let attachment_id = effects
            .iter()
            .find_map(|effect| match effect {
                super::super::panel_runtime::PanelRuntimeEffect::OpenPanelWindow(open) => {
                    Some(open.attachment_id)
                }
                _ => None,
            })
            .expect("panel should request a window");
        app.panel.handle(
            super::super::panel_runtime::PanelRuntimeIntent::PanelWindowOpened {
                attachment_id,
                window: panel,
            },
        );
        app.register_panel_popup(panel, origin, attachment_id);

        let _ = app.handle_tooltip_dwell_elapsed(origin, module);

        assert!(app.surfaces.tooltip_popups.is_empty());
        assert!(app.surface(origin).unwrap().tooltip_pending.is_none());
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
