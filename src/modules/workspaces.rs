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
pub(super) struct WorkspacesModule {
    id: crate::modules::ModuleId,
}

impl WorkspacesModule {
    pub(super) fn new(
        id: crate::modules::ModuleId,
        _config: crate::config::WorkspacesConfig,
    ) -> Self {
        WorkspacesModule { id }
    }
}

impl crate::modules::Module for WorkspacesModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> crate::modules::ModuleKind {
        crate::modules::ModuleKind::Workspaces
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }

    fn view_collapsed_with_style(
        &self,
        _state: &crate::modules::ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn view_panel_with_style(
        &self,
        _state: &crate::modules::ModuleState,
        _bar_style: &crate::config::Style,
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
pub(crate) struct WorkspaceItemModule {
    id: crate::modules::ModuleId,
    workspace: niri_ipc::Workspace,
    config: crate::config::WorkspacesConfig,
    config_name: String,
}

impl WorkspaceItemModule {
    pub(crate) fn module_id(config_name: &str, workspace_id: u64) -> crate::modules::ModuleId {
        if config_name == "workspaces" {
            crate::modules::ModuleId(format!("workspace:{workspace_id}"))
        } else {
            crate::modules::ModuleId(format!("workspace:{config_name}:{workspace_id}"))
        }
    }

    pub(crate) fn new(
        workspace: niri_ipc::Workspace,
        config: crate::config::WorkspacesConfig,
        config_name: String,
    ) -> Self {
        let id = Self::module_id(&config_name, workspace.id);
        WorkspaceItemModule {
            id,
            workspace,
            config,
            config_name,
        }
    }

    fn build_view(
        &self,
        horizontal: bool,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'static, crate::app::Message> {
        use iced::widget::container;
        use iced::{Alignment, Background, Length};

        let (bg, fg) = pill_colors(self.workspace.is_focused, self.workspace.is_active);
        let ws_num = self.workspace.idx as u32;
        let label_cfg = self.config.workspaces.get(&ws_num);
        let configured_icon = label_cfg.and_then(|c| c.icon.as_deref());
        let configured_label = label_cfg.and_then(|c| c.label.as_deref());
        let mode = if horizontal {
            crate::config::FormatMode::Horizontal
        } else {
            crate::config::FormatMode::Collapsed
        };
        let formatted = self.formatted_segments(mode);
        let styled = self.styled_segments_with_default(mode, bar_style);

        if horizontal {
            if let Some(segments) = styled {
                return container(crate::modules::view::styled_segments_row(segments, 10, fg))
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
                    .into();
            }
            let content = formatted
                .map(|segments| segments.join(" "))
                .unwrap_or_else(|| {
                    configured_icon
                        .map(|icon| icon.to_string())
                        .unwrap_or_else(|| format!("{ws_num}"))
                });
            container(crate::modules::view::styled_segment_text(
                content,
                10,
                fg,
                crate::modules::view::fallback_resolved_style(bar_style),
            ))
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
            if let Some(segments) = styled {
                return container(crate::modules::view::styled_segments_row(segments, 12, fg))
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
                    .into();
            }
            if let Some(content) = formatted.map(|segments| segments.join(" ")) {
                return container(crate::modules::view::styled_segment_text(
                    content,
                    12,
                    fg,
                    crate::modules::view::fallback_resolved_style(bar_style),
                ))
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
                .into();
            }
            let icon_str = configured_icon;
            let label_str = configured_label;

            let mut parts: Vec<iced::Element<'static, crate::app::Message>> = Vec::new();
            let style = crate::modules::view::fallback_resolved_style(bar_style);
            if let Some(icon) = icon_str {
                parts.push(crate::modules::view::styled_segment_text(
                    icon.to_string(),
                    12,
                    fg,
                    style,
                ));
            } else {
                parts.push(crate::modules::view::styled_segment_text(
                    format!("{ws_num}"),
                    12,
                    fg,
                    style,
                ));
            }
            if let Some(lbl) = label_str {
                parts.push(crate::modules::view::styled_segment_text(
                    lbl.to_string(),
                    12,
                    fg,
                    style,
                ));
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

    fn state_key(&self) -> &'static str {
        if self.workspace.is_focused {
            "focused"
        } else if self.workspace.is_active {
            "active"
        } else {
            "inactive"
        }
    }

    fn format_values(&self) -> Vec<(&'static str, String)> {
        let ws_num = self.workspace.idx as u32;
        let index = ws_num.to_string();
        let label_cfg = self.config.workspaces.get(&ws_num);
        let configured_icon = label_cfg.and_then(|c| c.icon.as_deref());
        let configured_label = label_cfg.and_then(|c| c.label.as_deref());
        let label = configured_label.unwrap_or(&index).to_string();
        let icon = configured_icon.unwrap_or(&index).to_string();
        let name = self.workspace.name.clone().unwrap_or_default();
        vec![
            ("icon", icon),
            ("value", label.clone()),
            ("index", index),
            ("name", name),
            ("label", label),
            ("state", self.state_key().to_string()),
        ]
    }

    fn formatted_segments(&self, mode: crate::config::FormatMode) -> Option<Vec<String>> {
        self.config
            .format
            .template_for(mode)
            .map(|template| template.render_segments(&self.format_values()))
    }

    fn styled_segments_with_default(
        &self,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        self.config.format.template_for(mode).map(|template| {
            template.render_styled_segments_with_default(
                &self.format_values(),
                Some(self.state_key()),
                default_style,
            )
        })
    }
}

impl crate::modules::Module for WorkspaceItemModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> crate::modules::ModuleKind {
        crate::modules::ModuleKind::WorkspaceItem
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }

    fn view_collapsed_with_style(
        &self,
        _state: &crate::modules::ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn view_panel_with_style(
        &self,
        _state: &crate::modules::ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn view_for_bar(
        &self,
        state: &crate::modules::ModuleState,
        _window: iced::window::Id,
        horizontal: bool,
        _expanded: bool,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        // state carries the surface output name; filter this workspace by output.
        let surface_output = match state {
            crate::modules::ModuleState::WorkspaceOutput(o) => o.as_str(),
            _ => "",
        };
        if !surface_output.is_empty() && self.workspace.output.as_deref() != Some(surface_output) {
            return iced::widget::Space::new().into();
        }
        self.build_view(horizontal, bar_style)
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
    use crate::config::{ColorValue, SegmentStyles, Style, ThemeToken};
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

    fn workspace_format_config() -> crate::config::WorkspacesConfig {
        let mut template = crate::config::FormatTemplate::parse(
            &["{label}"],
            &["icon", "value", "index", "name", "label", "state"],
            "test.format",
        )
        .unwrap();
        template.set_segment_styles(
            0,
            SegmentStyles::Stateful {
                default: None,
                by_state: vec![(
                    "focused".to_string(),
                    Style {
                        color: Some(ColorValue::Token(ThemeToken::Accent)),
                        ..Default::default()
                    },
                )],
            },
        );
        crate::config::WorkspacesConfig {
            format: crate::config::FormatConfig {
                collapsed: Some(template),
                ..Default::default()
            },
            ..Default::default()
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

    #[test]
    fn workspace_item_styled_segments_resolve_per_item_state() {
        let config = workspace_format_config();
        let focused =
            WorkspaceItemModule::new(make_workspace(0, true), config.clone(), "workspaces".into());
        let mut active_ws = make_workspace(1, false);
        active_ws.is_active = true;
        let active = WorkspaceItemModule::new(active_ws, config.clone(), "workspaces".into());
        let inactive =
            WorkspaceItemModule::new(make_workspace(2, false), config, "workspaces".into());

        assert_eq!(
            focused
                .styled_segments_with_default(
                    crate::config::FormatMode::Collapsed,
                    &crate::config::Style::default(),
                )
                .unwrap()[0]
                .1
                .color,
            Some(theme::ACCENT)
        );
        assert_eq!(
            active
                .styled_segments_with_default(
                    crate::config::FormatMode::Collapsed,
                    &crate::config::Style::default(),
                )
                .unwrap()[0]
                .1
                .color,
            None
        );
        assert_eq!(
            inactive
                .styled_segments_with_default(
                    crate::config::FormatMode::Collapsed,
                    &crate::config::Style::default(),
                )
                .unwrap()[0]
                .1
                .color,
            None
        );
    }
}
