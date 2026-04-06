use crate::theme;

/// Runtime state for the workspaces module — current workspace list from niri.
///
/// Populated by the WorkspacesChanged niri IPC event via app.rs update().
/// Stories 4.2–4.3 read this directly; Story 4.5 may set Unavailable.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceState {
    pub workspaces: Vec<niri_ipc::Workspace>,
}

/// Returns `(background, foreground)` colours for a workspace pill.
///
/// Focused workspace: accent background with base-coloured text.
/// Unfocused workspace: transparent background with normal text colour.
fn pill_colors(is_focused: bool) -> (iced::Color, iced::Color) {
    if is_focused {
        (theme::ACCENT, theme::BASE)
    } else {
        (iced::Color::TRANSPARENT, theme::TEXT)
    }
}

/// Workspace module: shows all niri workspaces as pills, fed by niri IPC via SharedState.
pub struct WorkspacesModule {
    id: crate::modules::ModuleId,
    config: crate::config::WorkspacesConfig,
}

impl WorkspacesModule {
    pub fn new(id: crate::modules::ModuleId, config: crate::config::WorkspacesConfig) -> Self {
        WorkspacesModule { id, config }
    }
}

impl crate::modules::Module for WorkspacesModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn module_type(&self) -> &str {
        "workspaces"
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
        use iced::{Alignment, Background, Length};

        let workspaces = match state {
            ModuleState::Workspace(s) => &s.workspaces,
            _ => return iced::widget::Space::new().into(), // Loading/Unavailable → empty (UX-DR16)
        };

        // Sort by idx to match Niri's scrollable-tiling order.
        let mut sorted: Vec<_> = workspaces.iter().collect();
        sorted.sort_by_key(|ws| ws.idx);

        let pills: Vec<iced::Element<'_, crate::app::Message>> = sorted
            .iter()
            .map(|ws| {
                let (bg, fg) = pill_colors(ws.is_focused);
                let ws_num = ws.idx as u32;
                let label_cfg = self.config.workspaces.get(&ws_num);
                // Collapsed: show icon if configured, else number (AC #2, #4).
                let content = label_cfg
                    .and_then(|c| c.icon.as_deref())
                    .map(|icon| icon.to_string())
                    .unwrap_or_else(|| format!("{ws_num}"));
                container(text(content).size(10).color(fg))
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
        use iced::widget::{column, container, row, text};
        use iced::{Alignment, Background, Length};

        let workspaces = match state {
            ModuleState::Workspace(s) => &s.workspaces,
            _ => return iced::widget::Space::new().into(),
        };

        let mut sorted: Vec<_> = workspaces.iter().collect();
        sorted.sort_by_key(|ws| ws.idx);

        let pills: Vec<iced::Element<'_, crate::app::Message>> = sorted
            .iter()
            .map(|ws| {
                let (bg, fg) = pill_colors(ws.is_focused);
                let ws_num = ws.idx as u32;
                let label_cfg = self.config.workspaces.get(&ws_num);
                let icon_str = label_cfg.and_then(|c| c.icon.as_deref());
                let label_str = label_cfg.and_then(|c| c.label.as_deref());

                // Expanded: icon (if any) + number + label (if any), space-separated (AC #1, #3).
                let mut parts: Vec<iced::Element<'_, crate::app::Message>> = Vec::new();
                if let Some(icon) = icon_str {
                    parts.push(text(icon).size(12).color(fg).into());
                } else {
                    parts.push(text(format!("{ws_num}")).size(12).color(fg).into());
                }
                if let Some(lbl) = label_str {
                    parts.push(text(lbl).size(12).color(fg).into());
                }

                container(row(parts).spacing(4).align_y(Alignment::Center))
                    .width(Length::Fill)
                    .height(20)
                    .align_x(Alignment::Start)
                    .align_y(Alignment::Center)
                    .padding([0, 6])
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

    fn view_horizontal(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{container, row, text};
        use iced::{Alignment, Background, Length};

        let workspaces = match state {
            ModuleState::Workspace(s) => &s.workspaces,
            _ => return iced::widget::Space::new().into(),
        };

        let mut sorted: Vec<_> = workspaces.iter().collect();
        sorted.sort_by_key(|ws| ws.idx);

        let pills: Vec<iced::Element<'_, crate::app::Message>> = sorted
            .iter()
            .map(|ws| {
                let (bg, fg) = pill_colors(ws.is_focused);
                let ws_num = ws.idx as u32;
                let label_cfg = self.config.workspaces.get(&ws_num);
                // Horizontal: show icon if configured, else number — same as collapsed.
                let content = label_cfg
                    .and_then(|c| c.icon.as_deref())
                    .map(|icon| icon.to_string())
                    .unwrap_or_else(|| format!("{ws_num}"));
                container(text(content).size(10).color(fg))
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

        container(row(pills).spacing(2))
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .padding([0, 4])
            .into()
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
        let m = WorkspacesModule::new(
            crate::modules::ModuleId::new("workspaces"),
            crate::config::WorkspacesConfig::default(),
        );
        assert_eq!(m.id().0, "workspaces");
        assert_eq!(m.module_type(), "workspaces");
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

    #[test]
    fn pill_colors_focused_returns_accent_base() {
        let (bg, fg) = pill_colors(true);
        assert_eq!(bg, theme::ACCENT);
        assert_eq!(fg, theme::BASE);
    }

    #[test]
    fn pill_colors_unfocused_returns_transparent_text() {
        let (bg, fg) = pill_colors(false);
        assert_eq!(bg, iced::Color::TRANSPARENT);
        assert_eq!(fg, theme::TEXT);
    }

    // --- Story 4.3 tests: WorkspacesConfig ---

    #[test]
    fn workspaces_config_default_is_empty() {
        let cfg = crate::config::WorkspacesConfig::default();
        assert!(cfg.workspaces.is_empty());
    }

    #[test]
    fn workspaces_config_lookup_returns_none_for_unconfigured() {
        let cfg = crate::config::WorkspacesConfig::default();
        assert!(cfg.workspaces.get(&1).is_none());
    }

    #[test]
    fn workspaces_config_stores_label_and_icon() {
        use crate::config::{WorkspaceLabelConfig, WorkspacesConfig};
        let mut cfg = WorkspacesConfig::default();
        cfg.workspaces.insert(
            1,
            WorkspaceLabelConfig {
                label: Some("web".to_string()),
                icon: Some(">".to_string()),
            },
        );
        let ws1 = cfg.workspaces.get(&1).unwrap();
        assert_eq!(ws1.label.as_deref(), Some("web"));
        assert_eq!(ws1.icon.as_deref(), Some(">"));
    }

    #[test]
    fn workspaces_module_accepts_config() {
        use crate::config::{WorkspaceLabelConfig, WorkspacesConfig};
        let mut cfg = WorkspacesConfig::default();
        cfg.workspaces.insert(
            2,
            WorkspaceLabelConfig {
                label: Some("code".to_string()),
                icon: None,
            },
        );
        let m = WorkspacesModule::new(crate::modules::ModuleId::new("workspaces"), cfg);
        assert_eq!(m.id().0, "workspaces");
        assert!(m.config.workspaces.get(&2).is_some());
        assert!(m.config.workspaces.get(&1).is_none());
    }
}
