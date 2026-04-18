use iced::Task;

use super::{App, Message};
use crate::modules::ModuleState;
use crate::modules::workspaces::WorkspaceItemModule;
use crate::niri::NiriEvent;

pub(super) fn workspace_render_state_equal(
    a: &[niri_ipc::Workspace],
    b: &[niri_ipc::Workspace],
) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(left, right)| {
            left.id == right.id
                && left.idx == right.idx
                && left.name == right.name
                && left.output == right.output
                && left.is_active == right.is_active
                && left.is_focused == right.is_focused
                && left.is_urgent == right.is_urgent
                && left.active_window_id == right.active_window_id
        })
}

impl App {
    pub(super) fn handle_niri_event(&mut self, event: NiriEvent) -> Task<Message> {
        match event {
            NiriEvent::Connected => {
                self.shared.niri.connected = true;
                Task::none()
            }
            NiriEvent::Unavailable => {
                self.shared.niri.connected = false;
                for (_, module) in &self.modules {
                    if matches!(module.module_type(), "workspaces" | "focused-window") {
                        self.shared
                            .modules
                            .insert(module.id(), ModuleState::Unavailable);
                    }
                }
                Task::none()
            }
            NiriEvent::Event(event) => {
                use niri_ipc::Event;
                match event {
                    Event::WorkspacesChanged { workspaces } => {
                        let changed = !workspace_render_state_equal(
                            &self.shared.niri.workspaces,
                            &workspaces,
                        );
                        let open_tasks = if changed && self.manual_outputs {
                            self.open_new_output_windows(&workspaces)
                        } else {
                            vec![]
                        };
                        if changed {
                            self.rebuild_workspace_item_modules(&workspaces);
                            self.shared.niri.workspaces = workspaces;
                        }
                        if !open_tasks.is_empty() {
                            return Task::batch(open_tasks);
                        }
                    }
                    Event::WindowFocusChanged { id } => {
                        self.shared.niri.focused_window_id = id;
                        if id.is_some() {
                            self.push_focused_window_state();
                        }
                    }
                    Event::WorkspaceActivated { id, focused } => {
                        let output = self
                            .shared
                            .niri
                            .workspaces
                            .iter()
                            .find(|ws| ws.id == id)
                            .and_then(|ws| ws.output.clone());

                        for ws in &mut self.shared.niri.workspaces {
                            if output.is_some() && ws.output == output {
                                ws.is_active = ws.id == id;
                            }
                            if focused {
                                ws.is_focused = ws.id == id;
                            }
                        }

                        let updated = self.shared.niri.workspaces.clone();
                        for workspace in &updated {
                            for (_, module) in &mut self.modules {
                                module.update_workspace(workspace);
                            }
                        }
                    }
                    Event::WindowsChanged { windows } => {
                        self.shared.niri.focused_window_id =
                            windows.iter().find(|w| w.is_focused).map(|w| w.id);
                        self.shared.niri.windows = windows.into_iter().map(|w| (w.id, w)).collect();
                        self.push_focused_window_state();
                    }
                    Event::WindowOpenedOrChanged { window } => {
                        if window.is_focused {
                            self.shared.niri.focused_window_id = Some(window.id);
                        }
                        self.shared.niri.windows.insert(window.id, window);
                        self.push_focused_window_state();
                    }
                    Event::WindowClosed { id } => {
                        self.shared.niri.windows.remove(&id);
                        if self.shared.niri.focused_window_id == Some(id) {
                            self.shared.niri.focused_window_id = None;
                        }
                        self.push_focused_window_state();
                    }
                    _ => {}
                }
                Task::none()
            }
        }
    }

    /// Rebuild one `WorkspaceItemModule` per workspace for each `WorkspacesModule` placeholder.
    /// Mirrors the `TrayUpdated` handler: find placeholder slots, remove stale items, insert fresh ones.
    pub(super) fn rebuild_workspace_item_modules(&mut self, workspaces: &[niri_ipc::Workspace]) {
        use crate::config::ModuleParams;

        if let Some(perf) = &self.perf {
            perf.record_workspace_rebuild();
        }

        // Collect placeholder slots: (index, zone, config_name, WorkspacesConfig clone).
        let slots: Vec<(
            usize,
            crate::modules::Zone,
            String,
            crate::config::WorkspacesConfig,
        )> = self
            .modules
            .iter()
            .enumerate()
            .filter(|(_, (_, m))| m.module_type() == "workspaces")
            .map(|(i, (z, m))| {
                let name = m.id().0.clone();
                let cfg = self
                    .config
                    .modules
                    .iter()
                    .find(|mc| mc.name == name)
                    .and_then(|mc| match &mc.params {
                        ModuleParams::Workspaces(c) => Some(c.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                (i, z.clone(), name, cfg)
            })
            .collect();

        self.modules
            .retain(|(_, m)| m.module_type() != "workspace_item");

        // Clear focused/hovered module on every surface if they pointed at a workspace item
        // that may no longer exist after this rebuild.
        for s in self.surfaces.values_mut() {
            if s.focused_module
                .as_ref()
                .map(|id| id.0.starts_with("workspace:"))
                .unwrap_or(false)
            {
                s.focused_module = None;
            }
            if s.hovered_module
                .as_ref()
                .map(|id| id.0.starts_with("workspace:"))
                .unwrap_or(false)
            {
                s.hovered_module = None;
            }
        }

        // Sort workspaces by idx to match Niri's scrollable-tiling order.
        let mut sorted: Vec<niri_ipc::Workspace> = workspaces.to_vec();
        sorted.sort_by_key(|ws| ws.idx);

        // Insert in reverse slot order so earlier slots' indices remain valid.
        for (slot_idx, zone, config_name, cfg) in slots.iter().rev() {
            let insert_at = slot_idx + 1;
            for ws in sorted.iter().rev() {
                let item = WorkspaceItemModule::new(ws.clone(), cfg.clone(), config_name.clone());
                self.modules
                    .insert(insert_at, (zone.clone(), Box::new(item)));
            }
        }
    }
}
