use crate::theme;

/// Returns `(background, foreground)` colours for a workspace pill.
///
/// Focused workspace: accent background with base-coloured text.
/// Active (visible on its output) but not globally focused: surface1 background with normal text.
/// Inactive workspace: transparent background with normal text colour.
fn pill_colors(is_focused: bool, is_active: bool) -> (iced::Color, iced::Color) {
    if is_focused {
        (theme::ACCENT, theme::BASE)
    } else if is_active {
        (theme::SURFACE1, theme::TEXT)
    } else {
        (iced::Color::TRANSPARENT, theme::TEXT)
    }
}

// ---------------------------------------------------------------------------
// WorkspacesModule -- placeholder registered from config; replaced by WorkspaceItemModules
// ---------------------------------------------------------------------------

/// Config-level placeholder for the workspaces list. Replaced at runtime by one
/// `WorkspaceItemModule` per workspace whenever workspace state changes.
pub struct WorkspacesModule {
    id: crate::modules::ModuleId,
}

impl WorkspacesModule {
    pub fn new(id: crate::modules::ModuleId, _config: crate::config::WorkspacesConfig) -> Self {
        WorkspacesModule { id }
    }
}

impl crate::modules::Module for WorkspacesModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn module_type(&self) -> &str {
        "workspaces"
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }

    fn view_collapsed(
        &self,
        _state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn view_expanded(
        &self,
        _state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn on_interaction(
        &self,
        _event: crate::modules::InteractionEvent,
    ) -> Option<crate::app::Message> {
        None
    }
}

// ---------------------------------------------------------------------------
// WorkspaceItemModule -- one per workspace, acts as a full independent module
// ---------------------------------------------------------------------------

/// One workspace pill, rendered as an independent bar module.
///
/// Instances are created dynamically by workspace state change handlers and inserted
/// into `App::modules` at the same zone position as the `WorkspacesModule` placeholder.
/// Each carries its workspace data directly so no shared-state lookup is needed in view.
pub struct WorkspaceItemModule {
    id: crate::modules::ModuleId,
    pub workspace: niri_ipc::Workspace,
    config: crate::config::WorkspacesConfig,
    config_name: String,
}

impl WorkspaceItemModule {
    pub fn new(
        workspace: niri_ipc::Workspace,
        config: crate::config::WorkspacesConfig,
        config_name: String,
    ) -> Self {
        let id = crate::modules::ModuleId(format!("workspace:{}", workspace.id));
        WorkspaceItemModule {
            id,
            workspace,
            config,
            config_name,
        }
    }

    fn build_view(&self, horizontal: bool) -> iced::Element<'static, crate::app::Message> {
        use iced::widget::{container, text};
        use iced::{Alignment, Background, Length};

        let (bg, fg) = pill_colors(self.workspace.is_focused, self.workspace.is_active);
        let ws_num = self.workspace.idx as u32;
        let label_cfg = self.config.workspaces.get(&ws_num);

        if horizontal {
            let content = label_cfg
                .and_then(|c| c.icon.as_deref())
                .map(|icon| icon.to_string())
                .unwrap_or_else(|| format!("{ws_num}"));
            container(text(content).size(10).color(fg))
                .width(32)
                .height(Length::Fill)
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
        } else {
            let icon_str = label_cfg.and_then(|c| c.icon.as_deref());
            let label_str = label_cfg.and_then(|c| c.label.as_deref());

            let mut parts: Vec<iced::Element<'static, crate::app::Message>> = Vec::new();
            if let Some(icon) = icon_str {
                parts.push(text(icon.to_string()).size(12).color(fg).into());
            } else {
                parts.push(text(format!("{ws_num}")).size(12).color(fg).into());
            }
            if let Some(lbl) = label_str {
                parts.push(text(lbl.to_string()).size(12).color(fg).into());
            }

            container(
                iced::widget::row(parts)
                    .spacing(4)
                    .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .height(20)
            .align_x(Alignment::Center)
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
        }
    }
}

impl crate::modules::Module for WorkspaceItemModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn module_type(&self) -> &str {
        "workspace_item"
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }

    fn view_collapsed(
        &self,
        _state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn view_expanded(
        &self,
        _state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn view_for_bar(
        &self,
        state: &crate::modules::ModuleState,
        _window: iced::window::Id,
        horizontal: bool,
        _expanded: bool,
    ) -> iced::Element<'_, crate::app::Message> {
        // state carries the surface output name; filter this workspace by output.
        let surface_output = match state {
            crate::modules::ModuleState::WorkspaceOutput(o) => o.as_str(),
            _ => "",
        };
        if !surface_output.is_empty() && self.workspace.output.as_deref() != Some(surface_output) {
            return iced::widget::Space::new().into();
        }
        self.build_view(horizontal)
    }

    fn on_interaction(
        &self,
        event: crate::modules::InteractionEvent,
    ) -> Option<crate::app::Message> {
        match event {
            crate::modules::InteractionEvent::Clicked
            | crate::modules::InteractionEvent::Activated(_) => {
                Some(crate::app::Message::NiriFocusWorkspace {
                    id: self.workspace.id,
                })
            }
            _ => None,
        }
    }

    fn update_workspace(&mut self, workspace: &niri_ipc::Workspace) {
        if self.workspace.id == workspace.id {
            self.workspace = workspace.clone();
        }
    }

    fn config_name(&self) -> Option<&str> {
        Some(&self.config_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    fn make_workspace(idx: u8, is_focused: bool) -> niri_ipc::Workspace {
        make_workspace_on(idx, is_focused, "eDP-1")
    }

    fn make_workspace_on(idx: u8, is_focused: bool, output: &str) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id: idx as u64 + 1,
            idx,
            name: None,
            output: Some(output.to_string()),
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
    fn pill_colors_focused_returns_accent_base() {
        let (bg, fg) = pill_colors(true, true);
        assert_eq!(bg, theme::ACCENT);
        assert_eq!(fg, theme::BASE);
    }

    #[test]
    fn pill_colors_active_not_focused_returns_surface1_text() {
        let (bg, fg) = pill_colors(false, true);
        assert_eq!(bg, theme::SURFACE1);
        assert_eq!(fg, theme::TEXT);
    }

    #[test]
    fn pill_colors_inactive_returns_transparent_text() {
        let (bg, fg) = pill_colors(false, false);
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
        assert!(!cfg.workspaces.contains_key(&1));
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
        let m = WorkspacesModule::new(crate::modules::ModuleId::new("workspaces"), cfg.clone());
        assert_eq!(m.id().0, "workspaces");
        assert!(cfg.workspaces.contains_key(&2));
        assert!(!cfg.workspaces.contains_key(&1));
    }

    // --- WorkspaceItemModule tests ---

    #[test]
    fn workspace_item_module_id_includes_workspace_id() {
        let ws = make_workspace(0, true);
        let m = WorkspaceItemModule::new(
            ws,
            crate::config::WorkspacesConfig::default(),
            "workspaces".into(),
        );
        assert_eq!(m.id().0, "workspace:1");
        assert_eq!(m.module_type(), "workspace_item");
        assert_eq!(m.config_name(), Some("workspaces"));
    }

    #[test]
    fn workspace_item_click_focuses_workspace() {
        let ws = make_workspace(0, false);
        let m = WorkspaceItemModule::new(
            ws,
            crate::config::WorkspacesConfig::default(),
            "workspaces".into(),
        );
        let msg = m
            .on_interaction(crate::modules::InteractionEvent::Clicked)
            .unwrap();
        assert!(matches!(msg, crate::app::Message::NiriFocusWorkspace { id } if id == 1));
    }

    #[test]
    fn workspace_item_activated_focuses_workspace() {
        let ws = make_workspace(0, false);
        let m = WorkspaceItemModule::new(
            ws,
            crate::config::WorkspacesConfig::default(),
            "workspaces".into(),
        );
        let msg = m
            .on_interaction(crate::modules::InteractionEvent::Activated(
                iced::window::Id::unique(),
            ))
            .unwrap();
        assert!(matches!(msg, crate::app::Message::NiriFocusWorkspace { id } if id == 1));
    }

    #[test]
    fn workspace_item_update_workspace_refreshes_active_state() {
        let ws = make_workspace(0, false);
        let mut m = WorkspaceItemModule::new(
            ws,
            crate::config::WorkspacesConfig::default(),
            "workspaces".into(),
        );
        let updated = make_workspace(0, true);

        m.update_workspace(&updated);

        assert!(m.workspace.is_active);
        assert!(m.workspace.is_focused);
    }
}
