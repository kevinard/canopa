use iced::Task;

use super::{App, Message};
use crate::modules::ModuleState;
use crate::modules::workspaces;
use crate::niri::NiriEvent;

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
                        for (_, module) in &self.modules {
                            if module.module_type() == "workspaces" {
                                self.shared.modules.insert(
                                    module.id(),
                                    ModuleState::Workspace(workspaces::WorkspaceState {
                                        workspaces: workspaces.clone(),
                                    }),
                                );
                            }
                        }

                        let open_tasks = if self.manual_outputs {
                            self.open_new_output_windows(&workspaces)
                        } else {
                            vec![]
                        };
                        self.shared.niri.workspaces = workspaces;
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
                        for (_, module) in &self.modules {
                            if module.module_type() == "workspaces" {
                                self.shared.modules.insert(
                                    module.id(),
                                    ModuleState::Workspace(workspaces::WorkspaceState {
                                        workspaces: updated.clone(),
                                    }),
                                );
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
}
