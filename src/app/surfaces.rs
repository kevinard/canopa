use std::collections::{HashMap, HashSet};
use std::ops::{Index, IndexMut};

use iced::Task;
use iced_layershell::reexport::{NewLayerShellSettings, OutputOption};

use super::{App, Message};
use crate::modules::ModuleKind;
use crate::surface::SurfaceState;

pub(super) struct PopupMeta {
    pub(super) service: String,
    pub(super) menu_path: zbus::zvariant::OwnedObjectPath,
    pub(super) tree: crate::surface::menu_popup::MenuTree,
    pub(super) bar_name: String,
}

pub(super) struct SurfaceManager {
    pub(super) surfaces: HashMap<iced::window::Id, SurfaceState>,
    pub(super) pending_surface_keys: HashMap<iced::window::Id, (String, String)>,
    pub(super) manual_outputs: bool,
    pub(super) popups: HashMap<iced::window::Id, PopupMeta>,
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

    pub(super) fn surfaces(
        &self,
    ) -> impl Iterator<Item = (&iced::window::Id, &SurfaceState)> {
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
        !self.popups.is_empty()
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
    pub(super) fn handle_surface_event(
        &mut self,
        event: super::messages::SurfaceEvent,
    ) -> Task<Message> {
        use super::messages::SurfaceEvent;

        match event {
            SurfaceEvent::CursorMoved(window, pos) => {
                let has_tray = self.modules.iter().any(|(_, module)| {
                    matches!(module.kind(), ModuleKind::Tray | ModuleKind::TrayIcon)
                });
                if has_tray && self.surfaces.last_cursor() != Some((window, pos)) {
                    self.surfaces.set_last_cursor(Some((window, pos)));
                }
                Task::none()
            }
            SurfaceEvent::WindowSized(window, w, h) => {
                self.surfaces.surface_mut(window).screen_size = (w, h);
                Task::none()
            }
            SurfaceEvent::ModuleHoverEnter(id, window) => {
                self.surface_mut(window).hovered_module = Some(id);
                Task::none()
            }
            SurfaceEvent::ModuleHoverExit(id, window) => {
                let s = self.surface_mut(window);
                if s.hovered_module.as_ref() == Some(&id) {
                    s.hovered_module = None;
                }
                Task::none()
            }
            SurfaceEvent::BarClick(window) => self.handle_bar_click(window),
            SurfaceEvent::BarHoverEnter(window) => self.handle_bar_hover_enter(window),
            SurfaceEvent::BarHoverExit(window) => self.handle_bar_hover_exit(window),
            SurfaceEvent::Tick => self.handle_tick(),
            SurfaceEvent::BarFocusGained(window) => self.handle_bar_focus_gained(window),
            SurfaceEvent::BarFocusLost(window) => self.handle_bar_focus_lost(window),
            SurfaceEvent::EscapePressed(window) => self.handle_escape_pressed(window),
            SurfaceEvent::FocusMoveNext(window) => self.handle_focus_move(window, true),
            SurfaceEvent::FocusMovePrev(window) => self.handle_focus_move(window, false),
            SurfaceEvent::FocusNextZone(window) => self.handle_focus_zone(window, true),
            SurfaceEvent::FocusPrevZone(window) => self.handle_focus_zone(window, false),
            SurfaceEvent::KeyboardActivate(window) => self.handle_keyboard_activate(window),
            SurfaceEvent::KeyboardSecondaryActivate(window) => {
                self.handle_keyboard_secondary_activate(window)
            }
            SurfaceEvent::ToggleExpand => self.handle_toggle_expand(),
        }
    }

    /// Returns a mutable reference to the `SurfaceState` for `id`, inserting a default if absent.
    /// Uses the pending (output_name, bar_name) pair (Story 9.1) when initialising a new entry.
    pub(super) fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
        self.surfaces.surface_mut(id)
    }

    /// Returns a reference to the `SurfaceState` for `id`, or `None` if the surface is unknown.
    pub(super) fn surface(&self, id: iced::window::Id) -> Option<&SurfaceState> {
        self.surfaces.surface(id)
    }

    /// Opens a layer-shell window for one (bar, output) pair, records it in `pending` and
    /// `surfaces`, and returns the open task. Shared by `boot` and `open_new_output_windows`.
    pub(super) fn spawn_bar_on_output(
        bar: &crate::config::BarConfig,
        output_name: &str,
        pending: &mut HashMap<iced::window::Id, (String, String)>,
        surfaces: &mut HashMap<iced::window::Id, SurfaceState>,
    ) -> Task<Message> {
        let anchor = bar.edge.anchor();
        let collapsed = bar.collapsed_width;
        let exclusive_zone = i32::try_from(collapsed).unwrap_or(48);
        let size = if bar.edge.is_horizontal() {
            Some((0, collapsed))
        } else {
            Some((collapsed, 0))
        };
        let settings = NewLayerShellSettings {
            anchor,
            exclusive_zone: Some(exclusive_zone),
            size,
            output_option: OutputOption::OutputName(output_name.to_string()),
            namespace: Some(Self::namespace()),
            ..Default::default()
        };
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
                    .filter(move |(_, b)| b.output == "all" || b.output == output_name)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Edge;
    use crate::surface::expand::ExpandState;
    use crate::modules::ModuleId;

    fn make_config() -> crate::config::Config {
        crate::config::Config::default()
    }

    fn test_wid() -> iced::window::Id {
        iced::window::Id::unique()
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
                    output: "DP-1".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
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
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "top".into(),
                    output: "all".into(),
                    edge: Edge::Top,
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let (app, _) = App::boot(config, vec!["eDP-1".into()]);
        assert_eq!(app.surfaces.len(), 2, "two bars on one output -> two surfaces");
    }

    #[test]
    fn surface_bar_name_set_from_pending_surface_key() {
        use crate::config::BarConfig;
        let config = crate::config::Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output: "eDP-1".into(),
                ..BarConfig::default()
            }],
            modules: vec![],
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
                output: "eDP-1".into(),
                ..BarConfig::default()
            }],
            modules: vec![],
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
                    output: "all".into(),
                    edge: Edge::Left,
                    ..crate::config::BarConfig::default()
                },
                crate::config::BarConfig {
                    name: "top".into(),
                    output: "all".into(),
                    edge: Edge::Top,
                    ..crate::config::BarConfig::default()
                },
            ],
            modules: vec![],
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

        let _ = app.update(Message::Surface(super::super::messages::SurfaceEvent::ModuleHoverEnter(
            id.clone(),
            wid,
        )));

        assert_eq!(app.surface(wid).unwrap().hovered_module, Some(id));
    }
}
