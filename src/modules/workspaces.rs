/// Runtime state for the workspaces module — current workspace list from niri.
///
/// Populated by the WorkspacesChanged niri IPC event via app.rs update().
/// Stories 4.2–4.3 read this directly; Story 4.5 may set Unavailable.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceState {
    pub workspaces: Vec<niri_ipc::Workspace>,
}

/// Workspace module: shows all niri workspaces as pills, fed by niri IPC via SharedState.
pub struct WorkspacesModule {
    id: crate::modules::ModuleId,
}

impl WorkspacesModule {
    pub fn new() -> Self {
        WorkspacesModule {
            id: crate::modules::ModuleId::new("workspaces"),
        }
    }
}

impl crate::modules::Module for WorkspacesModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    /// Workspace module has no independent data source — fed via Message::Niri in app.rs.
    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, container, text};
        use iced::{Alignment, Background, Color, Length};

        let workspaces = match state {
            ModuleState::Workspace(s) => &s.workspaces,
            _ => return iced::widget::Space::new().into(), // Loading/Unavailable → empty (UX-DR16)
        };

        // Sort by idx to match Niri's scrollable-tiling order (AC #6).
        let mut sorted: Vec<_> = workspaces.iter().collect();
        sorted.sort_by_key(|ws| ws.idx);

        let pills: Vec<iced::Element<'_, crate::app::Message>> = sorted
            .iter()
            .map(|ws| {
                let is_focused = ws.is_focused;
                let bg = if is_focused {
                    Color::from_rgb8(0x89, 0xb4, 0xfa) // color.accent
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.0) // transparent
                };
                let fg = if is_focused {
                    Color::from_rgb8(0x1e, 0x1e, 0x2e) // color.background (dark on accent)
                } else {
                    Color::from_rgb8(0xcd, 0xd6, 0xf4) // color.text
                };
                container(text(format!("{}", ws.idx + 1)).size(10).color(fg))
                    .width(32)
                    .height(20)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(bg)),
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .into()
            })
            .collect();

        column(pills).spacing(2).width(Length::Fill).into()
    }

    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, container, text};
        use iced::{Alignment, Background, Color, Length};

        let workspaces = match state {
            ModuleState::Workspace(s) => &s.workspaces,
            _ => return iced::widget::Space::new().into(),
        };

        let mut sorted: Vec<_> = workspaces.iter().collect();
        sorted.sort_by_key(|ws| ws.idx);

        let pills: Vec<iced::Element<'_, crate::app::Message>> = sorted
            .iter()
            .map(|ws| {
                let is_focused = ws.is_focused;
                let bg = if is_focused {
                    Color::from_rgb8(0x89, 0xb4, 0xfa)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.0)
                };
                let fg = if is_focused {
                    Color::from_rgb8(0x1e, 0x1e, 0x2e)
                } else {
                    Color::from_rgb8(0xcd, 0xd6, 0xf4)
                };
                // Story 4.3 will add label alongside number.
                // For 4.2, full-width pill shows number only.
                container(text(format!("{}", ws.idx + 1)).size(12).color(fg))
                    .width(Length::Fill)
                    .height(20)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(bg)),
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .into()
            })
            .collect();

        column(pills).spacing(2).width(Length::Fill).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    fn make_workspace(idx: u8, is_focused: bool) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id: idx as u64 + 1,
            idx,
            name: None,
            output: Some("eDP-1".to_string()),
            is_urgent: false,
            is_active: is_focused,
            is_focused,
            active_window_id: None,
        }
    }

    #[test]
    fn workspaces_module_id_is_workspaces() {
        let m = WorkspacesModule::new();
        assert_eq!(m.id().0, "workspaces");
    }

    #[test]
    fn workspace_state_default_is_empty() {
        let state = WorkspaceState::default();
        assert!(state.workspaces.is_empty());
    }

    #[test]
    fn workspace_state_stores_workspaces() {
        let ws = make_workspace(0, true);
        let state = WorkspaceState {
            workspaces: vec![ws],
        };
        assert_eq!(state.workspaces.len(), 1);
        assert!(state.workspaces[0].is_focused);
    }
}
