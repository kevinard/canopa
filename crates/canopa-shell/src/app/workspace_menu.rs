//! A workspace pill's menu: rename the workspace or move it up or down among
//! the workspaces of its output. Names and moves are Niri actions; the rename
//! prompt is the Shell's own dmenu panel.
use iced::Task;

use super::{App, Message};
use crate::modules::workspaces::{self, MenuCommand, MenuRow, WorkspaceEvent};

/// The rows of the workspace menu on screen, for mapping a selection back.
pub(super) struct OpenWorkspaceMenu {
    id: u64,
    rows: Vec<MenuRow>,
}

impl App {
    pub(super) fn handle_workspace_event(&mut self, event: WorkspaceEvent) -> Task<Message> {
        match event {
            WorkspaceEvent::Menu { id, module, window } => {
                let target = super::widget_ops::module_widget_id(window, &module);
                let op = super::widget_ops::FindBounds {
                    target,
                    result: None,
                };
                iced::advanced::widget::operate(op).then(move |bounds| {
                    Task::done(Message::Workspaces(WorkspaceEvent::MenuAt {
                        id,
                        window,
                        bounds,
                    }))
                })
            }
            WorkspaceEvent::MenuAt { id, window, bounds } => {
                self.open_workspace_menu(id, window, bounds)
            }
            WorkspaceEvent::Renamed { id, response } => {
                if let Some(action) = rename_action(id, response) {
                    super::niri::run_niri_action(action);
                }
                Task::none()
            }
        }
    }

    /// The workspace `id` and its place among the workspaces of its output.
    fn workspace_place(&self, id: u64) -> Option<(niri_ipc::Workspace, usize, usize)> {
        let state = self.niri.state();
        let workspace = state.workspaces.iter().find(|ws| ws.id == id)?.clone();
        let mut siblings: Vec<_> = state
            .workspaces
            .iter()
            .filter(|ws| ws.output == workspace.output)
            .collect();
        siblings.sort_by_key(|ws| ws.idx);
        let position = siblings.iter().position(|ws| ws.id == id)?;
        Some((workspace, position, siblings.len()))
    }

    fn open_workspace_menu(
        &mut self,
        id: u64,
        window: iced::window::Id,
        bounds: Option<iced::Rectangle>,
    ) -> Task<Message> {
        let Some((workspace, position, count)) = self.workspace_place(id) else {
            return Task::none();
        };
        let Some(surface) = self.surfaces.surface(window) else {
            return Task::none();
        };
        let screen_size = surface.screen_size;
        let edge = Self::bar_for_surface_name(&self.config.bars, &surface.bar_name)
            .edge
            .clone();
        let anchor = match bounds {
            Some(bounds) => super::taskbar::menu_anchor(&edge, bounds),
            None => {
                let point = self
                    .surfaces
                    .last_cursor()
                    .map_or(iced::Point::ORIGIN, |(_, point)| point);
                (point.x as i32, point.y as i32)
            }
        };
        let rows = workspaces::menu(&workspace, position, count);
        let tree = workspaces::menu_tree(&rows, anchor, screen_size);
        self.workspace_menu = Some(OpenWorkspaceMenu { id, rows });
        self.open_menu_popup(
            workspaces::MENU_SERVICE.to_owned(),
            zbus::zvariant::OwnedObjectPath::try_from(workspaces::MENU_PATH)
                .expect("valid workspace menu path"),
            tree,
            window,
        )
    }

    /// Run the workspace menu row the person chose.
    pub(super) fn handle_workspace_menu_item(&mut self, item: i32) -> Task<Message> {
        let Some(menu) = self.workspace_menu.take() else {
            return Task::none();
        };
        let Some(Some((_, command, true))) = usize::try_from(item)
            .ok()
            .and_then(|position| menu.rows.get(position))
        else {
            return Task::none();
        };
        let Some((workspace, position, _)) = self.workspace_place(menu.id) else {
            return Task::none();
        };
        let reference = Some(niri_ipc::WorkspaceReferenceArg::Id(menu.id));
        match command {
            MenuCommand::Heading => Task::none(),
            // Niri's indices start at 1; `position` is this workspace's from 0.
            MenuCommand::MoveUp => {
                super::niri::run_niri_action(niri_ipc::Action::MoveWorkspaceToIndex {
                    index: position,
                    reference,
                });
                Task::none()
            }
            MenuCommand::MoveDown => {
                super::niri::run_niri_action(niri_ipc::Action::MoveWorkspaceToIndex {
                    index: position + 2,
                    reference,
                });
                Task::none()
            }
            MenuCommand::Rename => self.prompt_workspace_name(&workspace),
        }
    }

    /// Ask for a new name in the Shell's own prompt. A named workspace also
    /// offers to go back to its numbered name.
    pub(super) fn prompt_workspace_name(
        &mut self,
        workspace: &niri_ipc::Workspace,
    ) -> Task<Message> {
        let id = workspace.id;
        let named = workspace
            .name
            .as_deref()
            .is_some_and(|name| !name.trim().is_empty());
        let mut entries = Vec::new();
        if named {
            entries.push(crate::ipc::protocol::DmenuEntry {
                label: Some(fl!("shell-workspace-rename-default")),
                value: Some(String::new()),
                icon: None,
                keywords: Vec::new(),
            });
        }
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: Some(fl!(
                "shell-workspace-rename-prompt",
                name = workspaces::workspace_title(workspace)
            )),
            entries,
            allow_custom: true,
        };
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let responder = super::messages::ipc_responder(sender);
        Task::batch([
            Task::done(Message::Ipc(super::messages::IpcEvent::DmenuSelection {
                request,
                responder,
            })),
            Task::perform(receiver, move |response| {
                Message::Workspaces(WorkspaceEvent::Renamed {
                    id,
                    response: response.ok(),
                })
            }),
        ])
    }
}

/// The Niri action a rename prompt's answer asks for: a trimmed new name, the
/// numbered name for an empty answer, nothing when it was dismissed.
fn rename_action(
    id: u64,
    response: Option<crate::ipc::protocol::IpcResponse>,
) -> Option<niri_ipc::Action> {
    let crate::ipc::protocol::IpcResponse::Selected { value } = response? else {
        return None;
    };
    let reference = niri_ipc::WorkspaceReferenceArg::Id(id);
    let name = value.trim();
    Some(if name.is_empty() {
        niri_ipc::Action::UnsetWorkspaceName {
            reference: Some(reference),
        }
    } else {
        niri_ipc::Action::SetWorkspaceName {
            name: name.to_owned(),
            workspace: Some(reference),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::rename_action;
    use crate::ipc::protocol::IpcResponse;
    use niri_ipc::{Action, WorkspaceReferenceArg};

    #[test]
    fn rename_answers_set_clear_or_leave_the_name() {
        assert!(matches!(
            rename_action(7, Some(IpcResponse::Selected { value: "  Mail ".into() })),
            Some(Action::SetWorkspaceName { name, workspace: Some(WorkspaceReferenceArg::Id(7)) })
                if name == "Mail"
        ));
        assert!(matches!(
            rename_action(7, Some(IpcResponse::Selected { value: " ".into() })),
            Some(Action::UnsetWorkspaceName {
                reference: Some(WorkspaceReferenceArg::Id(7))
            })
        ));
        assert!(rename_action(7, Some(IpcResponse::Cancelled)).is_none());
        assert!(rename_action(7, None).is_none());
    }
}
