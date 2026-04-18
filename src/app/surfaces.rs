use std::collections::{HashMap, HashSet};

use iced::Task;
use iced_layershell::reexport::{NewLayerShellSettings, OutputOption};

use super::{App, Message};
use crate::surface::SurfaceState;

impl App {
    /// Returns a mutable reference to the `SurfaceState` for `id`, inserting a default if absent.
    /// Uses the pending (output_name, bar_name) pair (Story 9.1) when initialising a new entry.
    pub(super) fn surface_mut(&mut self, id: iced::window::Id) -> &mut SurfaceState {
        if !self.surfaces.contains_key(&id) {
            let (output_name, bar_name) = self.pending_surface_keys.remove(&id).unwrap_or_default();
            self.surfaces
                .insert(id, SurfaceState::new(output_name, bar_name));
        }
        self.surfaces.get_mut(&id).unwrap()
    }

    /// Returns a reference to the `SurfaceState` for `id`, or `None` if the surface is unknown.
    pub(super) fn surface(&self, id: iced::window::Id) -> Option<&SurfaceState> {
        self.surfaces.get(&id)
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
            .values()
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
                &mut self.pending_surface_keys,
                &mut self.surfaces,
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
        let surface = self.surfaces.get(&window);
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
            .any(|(_, m)| m.module_type() == "tray_icon" && m.id().0 == module_id);
        if !had_it {
            return;
        }
        self.modules
            .retain(|(_, m)| !(m.module_type() == "tray_icon" && m.id().0 == module_id));
        for s in self.surfaces.values_mut() {
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
            if m.module_type() == "tray_icon" && m.config_name() == Some(config_name) {
                pos += 1;
            } else {
                break;
            }
        }
        pos
    }
}
