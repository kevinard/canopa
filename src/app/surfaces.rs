use std::collections::{HashMap, HashSet};
use std::ops::{Index, IndexMut};

use iced::Task;
use iced_layershell::reexport::{KeyboardInteractivity, NewLayerShellSettings};

use super::app_effects::AppEffect;
use super::bar_surface_plan::PlannedBarSurface;
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
    BarSurfaceRemoved {
        identity: BarSurfaceIdentity,
        window: iced::window::Id,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum RegistryEffect {
    TooltipClosed { window: iced::window::Id },
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
    crate::surface::placement::usable_area_margin(&tree.bar_edge, placement.layer_margin(), gap)
}

pub(super) fn tooltip_popup_window_settings(
    tree: &crate::surface::tooltip_popup::TooltipTree,
) -> NewLayerShellSettings {
    let size = crate::surface::tooltip_popup::popup_size(tree);
    NewLayerShellSettings {
        anchor: crate::surface::placement::layer_anchor(&tree.bar_edge),
        exclusive_zone: Some(0),
        size: Some(size),
        margin: Some(tooltip_margin(tree, size, 4)),
        layer: iced_layershell::reexport::Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::None,
        ..Default::default()
    }
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
    pub(super) tooltip_popups: HashMap<iced::window::Id, TooltipPopup>,
    pub(super) last_cursor: Option<(iced::window::Id, iced::Point)>,
}

impl SurfaceManager {
    pub(super) fn new(
        surfaces: HashMap<iced::window::Id, SurfaceState>,
        pending_surface_keys: HashMap<iced::window::Id, (String, String)>,
    ) -> Self {
        Self {
            surfaces,
            pending_surface_keys,
            tooltip_popups: HashMap::new(),
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

    pub(super) fn get_mut(&mut self, id: &iced::window::Id) -> Option<&mut SurfaceState> {
        self.surfaces.get_mut(id)
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
                self.close_tooltips_for_surface(window)
                    .into_iter()
                    .map(|effect| match effect {
                        RegistryEffect::TooltipClosed { window } => {
                            BarSurfaceLifecycleEffect::RemoveWindow(window)
                        }
                    }),
            );
            effects.push(BarSurfaceLifecycleEffect::BarSurfaceRemoved { identity, window });
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

    pub(super) fn open_tooltip_overlay(&mut self, popup: TooltipPopup) -> Vec<RegistryEffect> {
        let effects = self.close_tooltips_for_surface(popup.target_window);
        self.tooltip_popups.insert(popup.window, popup);
        effects
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
        if self.tooltip_popups.remove(&window).is_some() {
            return vec![RegistryEffect::TooltipClosed { window }];
        }
        Vec::new()
    }

    pub(super) fn close_tooltip_overlays(&mut self) -> Vec<RegistryEffect> {
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

        if self.tooltip_popups.contains_key(&window) {
            return super::surface_input_router::WindowRole::Tooltip;
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
        let foreground_roles = self.foreground.role_snapshot();
        if let Some(role) = foreground_roles.role_for(window) {
            return role;
        }
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
                let has_tray = self.modules.has_kind(ModuleKind::Tray)
                    || self.modules.has_kind(ModuleKind::TrayIcon);
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
            SurfaceInputEffect::PanelWindowClosed(window) => {
                if self.panel_window_is_external_provider(window) {
                    self.close_external_panel_session_for_replacement();
                }
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::PanelWindowClosed { window });
                Task::batch(self.apply_foreground_effects(effects))
            }
            SurfaceInputEffect::MenuWindowClosed(window) => {
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::MenuWindowClosed { window });
                Task::batch(self.apply_foreground_effects(effects))
            }
            SurfaceInputEffect::RegistryWindowClosed(window) => {
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
            SurfaceInputEffect::BarEscape { window } => self.handle_bar_escape_pressed(window),
            SurfaceInputEffect::PanelEscape => self.handle_panel_escape_pressed(),
            SurfaceInputEffect::BarMoveFocus { window, forward } => {
                self.handle_bar_focus_move(window, forward)
            }
            SurfaceInputEffect::PanelMoveFocus { forward } => self.handle_panel_focus_move(forward),
            SurfaceInputEffect::MenuMoveFocus { window, forward } => {
                self.handle_menu_focus_move(window, forward)
            }
            SurfaceInputEffect::BarFocusZone { window, forward } => {
                self.handle_focus_zone(window, forward)
            }
            SurfaceInputEffect::BarKeyboardActivate { window } => {
                self.handle_bar_keyboard_activate(window)
            }
            SurfaceInputEffect::PanelKeyboardActivate => self.handle_panel_keyboard_activate(),
            SurfaceInputEffect::MenuKeyboardActivate { window } => {
                self.handle_menu_keyboard_activate(window)
            }
            SurfaceInputEffect::BarKeyboardSecondaryActivate { window } => {
                self.handle_bar_keyboard_secondary_activate(window)
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
        if self.foreground.has_active_panel() {
            return Task::batch(self.close_active_panel());
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

    /// Reconciles live **Bar Surfaces** from a Wayland Output Snapshot.
    pub(super) fn handle_wayland_output_snapshot(&mut self, outputs: Vec<String>) -> Task<Message> {
        Task::batch(self.reconcile_bar_surfaces_from_observed_outputs(Some(outputs)))
    }

    pub(super) fn reconcile_bar_surfaces_from_observed_outputs(
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

    /// Look up the `BarConfig` for a surface by its `bar_name`.
    /// Falls back to the first bar when the name is empty or not found.
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
            .foreground
            .active_panel_origin_for(target_window)
            .is_some()
        {
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

        Task::batch(self.apply_app_effects(vec![AppEffect::OpenTooltipWindow {
            target_window,
            target_module: module,
            tree,
        }]))
    }

    fn tooltip_values_for(
        &self,
        window: iced::window::Id,
        module: &crate::modules::ModuleId,
    ) -> Vec<(&'static str, String)> {
        let _ = window;
        self.modules.tooltip_values_for(module)
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

        if !self
            .effective_nav_list(window)
            .iter()
            .any(|(_, id)| id == module)
        {
            return None;
        }
        let config_name = self.modules.configured_module_name_for_instance(module);
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
            #[cfg(test)]
            ModuleParams::None => return None,
        };
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

    fn planned_bar_surface(name: &str, output_name: &str) -> PlannedBarSurface {
        let bar = crate::config::BarConfig {
            name: name.into(),
            ..crate::config::BarConfig::default()
        };
        PlannedBarSurface::from_bar(&bar, output_name)
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
    fn removing_bar_surface_closes_only_tooltips_for_that_bar_surface() {
        let removed_window = test_wid();
        let kept_window = test_wid();
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
        );

        let effects =
            registry.reconcile_bar_surfaces(vec![planned_bar_surface("main", "HDMI-A-1")]);

        assert!(
            effects.contains(&BarSurfaceLifecycleEffect::BarSurfaceRemoved {
                identity: BarSurfaceIdentity::new("main", Some("DP-1".into())),
                window: removed_window,
            })
        );
    }

    #[test]
    fn registry_classifies_missing_and_bar_surface_windows() {
        let window = test_wid();
        let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new());

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
    fn registry_classifies_tooltip_windows() {
        let window = test_wid();
        let mut registry = SurfaceManager::new(HashMap::new(), HashMap::new());
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
    fn live_panel_window_closed_restores_bar_keyboard_interactivity_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces
            .surfaces
            .insert(origin, SurfaceState::new("DP-1".into(), "main".into()));
        app.surface_mut(origin).focused_module = Some(ModuleId::new("clock"));
        app.register_panel_popup(panel, origin);

        let _ = app.handle_surface_event(crate::app::messages::SurfaceEvent::WindowClosed(panel));

        assert!(!app.foreground.is_panel_window(panel));
        assert!(app.surface(origin).unwrap().focused_module.is_none());
    }

    #[test]
    fn wayland_output_snapshot_closes_panel_when_origin_bar_surface_is_removed() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces
            .surfaces
            .insert(origin, SurfaceState::new("DP-1".into(), "main".into()));
        app.register_panel_popup(panel, origin);

        let _tasks = app.reconcile_bar_surfaces_from_observed_outputs(Some(Vec::new()));

        assert!(app.surface(origin).is_none());
        assert!(app.foreground.active_panel_window().is_none());
    }

    #[test]
    fn toggle_panel_ipc_closes_open_panel() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let origin = test_wid();
        let panel = test_wid();
        app.surfaces
            .surfaces
            .insert(origin, SurfaceState::new("DP-1".into(), "main".into()));
        app.register_panel_popup(panel, origin);

        let _ = app.update(Message::Surface(
            crate::app::messages::SurfaceEvent::TogglePanel,
        ));

        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            0
        );
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

        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );
        let panel = app.foreground.active_panel_tree().unwrap();
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
    fn boot_without_allowed_outputs_waits_for_snapshot() {
        let (app, _) = App::boot(make_config(), vec![]);
        assert!(app.surfaces.is_empty());
    }

    #[test]
    fn boot_with_allowed_outputs_creates_bar_surfaces() {
        let (app, _) = App::boot(make_config(), vec!["eDP-1".into()]);
        assert_eq!(app.surfaces.len(), 1);
    }

    #[test]
    fn boot_with_known_empty_outputs_creates_no_bar_surfaces() {
        let (app, _) = App::boot_with_eligible_outputs(
            make_config(),
            crate::app::eligible_outputs::EligibleOutputState::Known(Vec::new()),
        );

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
            panel_providers: vec![],
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
            panel_providers: vec![],
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
            panel_providers: vec![],
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
        );
        let desired = vec![
            planned_bar_surface("main", "DP-1"),
            planned_bar_surface("side", "DP-1"),
        ];

        let effects = registry.reconcile_bar_surfaces(desired);

        assert_eq!(
            effects,
            vec![BarSurfaceLifecycleEffect::Open(planned_bar_surface(
                "side", "DP-1"
            ))]
        );
        assert!(registry.surface(existing).is_some());
    }

    #[test]
    fn registry_reconciliation_closes_stale_surface_and_associated_tooltips() {
        let stale = test_wid();
        let tooltip = test_wid();
        let mut registry = SurfaceManager::new(
            HashMap::from([(stale, SurfaceState::new("DP-1".into(), "main".into()))]),
            HashMap::new(),
        );
        registry.insert_tooltip_popup(TooltipPopup {
            window: tooltip,
            target_window: stale,
            target_module: ModuleId::new("clock"),
            tree: tooltip_tree(crate::config::Edge::Top),
        });

        let effects = registry.reconcile_bar_surfaces(Vec::new());

        assert_eq!(
            effects,
            vec![
                BarSurfaceLifecycleEffect::RemoveWindow(tooltip),
                BarSurfaceLifecycleEffect::BarSurfaceRemoved {
                    identity: BarSurfaceIdentity::new("main", Some("DP-1".into())),
                    window: stale,
                },
            ]
        );
        assert!(registry.surface(stale).is_none());
        assert!(registry.tooltip_popup(tooltip).is_none());
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
            panel_providers: vec![],
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
            panel_providers: vec![],
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

        let tasks = app.reconcile_bar_surfaces_from_observed_outputs(None);

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
            panel_providers: vec![],
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
            item_is_menu: false,
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
            .first_id_by_kind(crate::modules::ModuleKind::TrayIcon)
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
        let _ = app.modules.handle(
            super::super::module_runtime::ModuleRuntimeIntent::ApplyModuleState {
                id: module.clone(),
                state: crate::modules::ModuleState::Battery(
                    crate::modules::battery::BatteryState {
                        percentage: 74.0,
                        power_state: crate::modules::battery::BatteryPowerState::Discharging,
                        present: true,
                    },
                ),
            },
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
        let _ = app.modules.handle(
            super::super::module_runtime::ModuleRuntimeIntent::ApplyModuleState {
                id: module.clone(),
                state: crate::modules::ModuleState::Battery(
                    crate::modules::battery::BatteryState {
                        percentage: 74.0,
                        power_state: crate::modules::battery::BatteryPowerState::Discharging,
                        present: true,
                    },
                ),
            },
        );
        app.surface_mut(origin).hovered_module = Some(module.clone());
        app.surface_mut(origin).tooltip_pending = Some((module.clone(), 32.0));
        let request = app.panel_open_request(
            origin,
            crate::surface::panel::PanelOrigin::Module(module.clone()),
            crate::surface::panel::PanelContent::Menu("main".into()),
            None,
        );
        let effects = app.prepare_panel_for_test(request);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            super::super::foreground::ForegroundEffect::OpenPanelWindow { .. }
        )));
        app.register_panel_popup(panel, origin);

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
    fn tooltip_window_respects_other_bars_reserved_space() {
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            let settings = tooltip_popup_window_settings(&tooltip_tree(edge.clone()));
            assert_eq!(settings.exclusive_zone, Some(0), "{edge:?}");
            let (top, right, bottom, left) = settings.margin.unwrap();
            let distance = match edge {
                Edge::Top => top,
                Edge::Bottom => bottom,
                Edge::Left => left,
                Edge::Right => right,
            };
            assert_eq!(
                distance, 4,
                "only the tooltip gap is added to the reserved area"
            );
        }
    }

    #[test]
    fn tooltip_popup_window_is_sized_to_content_not_full_overlay() {
        let tree = tooltip_tree(crate::config::Edge::Left);
        let settings = tooltip_popup_window_settings(&tree);

        assert_eq!(settings.size, Some((32, 34)));
        assert_eq!(settings.anchor, Anchor::Left | Anchor::Top);
        assert_eq!(settings.margin, Some((483, 0, 0, 4)));
    }

    #[test]
    fn right_edge_tooltip_uses_right_anchor_margin() {
        let tree = tooltip_tree(crate::config::Edge::Right);
        let settings = tooltip_popup_window_settings(&tree);

        assert_eq!(settings.anchor, Anchor::Right | Anchor::Top);
        assert_eq!(settings.margin, Some((483, 4, 0, 0)));
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
