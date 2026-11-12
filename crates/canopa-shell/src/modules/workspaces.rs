use crate::theme;
use crate::theme::bar_metrics::ICON_SIZE;

/// Returns `(background, foreground)` colours for a workspace pill.
///
/// Focused workspace: accent background with base-coloured text.
/// Active (visible on its output) but not globally focused: surface1 background with normal text
/// and a contrasting outline from `pill_border`.
/// Inactive workspace: transparent background with normal text colour.
fn pill_colors(is_focused: bool, is_active: bool) -> (iced::Color, iced::Color) {
    if is_focused {
        (theme::palette().accent, theme::palette().on_accent)
    } else if is_active {
        (theme::palette().raised, theme::palette().text)
    } else {
        (iced::Color::TRANSPARENT, theme::palette().text)
    }
}

fn pill_border(is_focused: bool, is_active: bool) -> iced::Border {
    iced::Border {
        color: if is_active && !is_focused {
            theme::palette().muted
        } else {
            iced::Color::TRANSPARENT
        },
        width: 1.0,
        radius: 10.0.into(),
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
}

impl crate::modules::render::ModuleRender for WorkspacesModule {
    fn has_content(
        &self,
        _state: &crate::modules::ModuleState,
        _mode: crate::config::FormatMode,
    ) -> bool {
        false
    }

    fn render_collapsed_with_style(
        &self,
        _state: &crate::modules::ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn render_panel_with_style(
        &self,
        _state: &crate::modules::ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
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
}

impl WorkspaceItemModule {
    pub(crate) fn module_id(config_name: &str, workspace_id: u64) -> crate::modules::ModuleId {
        crate::modules::ModuleId::workspace(config_name, workspace_id)
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
        }
    }

    fn build_view(
        &self,
        horizontal: bool,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'static, crate::app::Message> {
        use iced::widget::container;
        use iced::{Alignment, Background, Length};

        let (mut bg, mut fg) = pill_colors(self.workspace.is_focused, self.workspace.is_active);
        // Occupied workspaces are soft pills; an empty one recedes to its number.
        if !self.workspace.is_focused {
            if self.workspace.active_window_id.is_some() {
                if bg == iced::Color::TRANSPARENT {
                    bg = theme::palette().text.scale_alpha(0.12);
                }
            } else {
                fg = theme::palette().muted;
            }
        }
        let mut border = pill_border(self.workspace.is_focused, self.workspace.is_active);
        // Fill grades the states: accent for the focused workspace, a filled
        // pill when occupied, an outline when empty.
        if !self.workspace.is_focused && self.workspace.active_window_id.is_none() {
            bg = iced::Color::TRANSPARENT;
            border.color = theme::palette().text.scale_alpha(0.16);
            border.width = border.width.max(1.0);
        }
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
                return container(crate::modules::view::styled_segments_row(
                    segments,
                    crate::theme::label_size() as u32,
                    fg,
                ))
                .width(32)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(bg)),
                    border,
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
                if configured_icon.is_some() {
                    ICON_SIZE
                } else {
                    crate::theme::label_size() as u32
                },
                fg,
                crate::modules::view::fallback_resolved_style(bar_style),
            ))
            .width(32)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(bg)),
                border,
                ..Default::default()
            })
            .into()
        } else {
            if let Some(segments) = styled {
                return container(crate::modules::view::styled_segments_row(
                    segments,
                    crate::theme::label_size() as u32,
                    fg,
                ))
                .width(Length::Fill)
                .height(30)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .padding([0, 6])
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(bg)),
                    border,
                    ..Default::default()
                })
                .into();
            }
            if let Some(content) = formatted.map(|segments| segments.join(" ")) {
                return container(crate::modules::view::styled_segment_text(
                    content,
                    crate::theme::label_size() as u32,
                    fg,
                    crate::modules::view::fallback_resolved_style(bar_style),
                ))
                .width(Length::Fill)
                .height(30)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .padding([0, 6])
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(bg)),
                    border,
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
                    ICON_SIZE,
                    fg,
                    style,
                ));
            } else {
                parts.push(crate::modules::view::styled_segment_text(
                    format!("{ws_num}"),
                    13,
                    fg,
                    crate::config::ResolvedStyle {
                        weight: style.weight.or(Some(crate::config::Weight::Bold)),
                        ..style
                    },
                ));
            }
            if let Some(lbl) = label_str {
                parts.push(crate::modules::view::styled_segment_text(
                    lbl.to_string(),
                    crate::theme::label_size() as u32,
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
            .height(30)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .padding([0, 6])
            .style(move |_theme| container::Style {
                background: Some(Background::Color(bg)),
                border,
                ..Default::default()
            })
            .into()
        }
    }

    fn build_rotated_format_view(
        &self,
        horizontal: bool,
        bar_style: &crate::config::Style,
        rotation: crate::config::ResolvedContentRotation,
    ) -> Option<iced::Element<'static, crate::app::Message>> {
        use iced::widget::container;
        use iced::{Alignment, Background, Length};

        let (mut bg, mut fg) = pill_colors(self.workspace.is_focused, self.workspace.is_active);
        // Occupied workspaces are soft pills; an empty one recedes to its number.
        if !self.workspace.is_focused {
            if self.workspace.active_window_id.is_some() {
                if bg == iced::Color::TRANSPARENT {
                    bg = theme::palette().text.scale_alpha(0.12);
                }
            } else {
                fg = theme::palette().muted;
            }
        }
        let mut border = pill_border(self.workspace.is_focused, self.workspace.is_active);
        // Fill grades the states: accent for the focused workspace, a filled
        // pill when occupied, an outline when empty.
        if !self.workspace.is_focused && self.workspace.active_window_id.is_none() {
            bg = iced::Color::TRANSPARENT;
            border.color = theme::palette().text.scale_alpha(0.16);
            border.width = border.width.max(1.0);
        }
        let mode = if horizontal {
            crate::config::FormatMode::Horizontal
        } else {
            crate::config::FormatMode::Collapsed
        };
        let segments = self.styled_segments_with_default(mode, bar_style)?;
        let content = crate::modules::view::formatted_segments_content(
            segments,
            crate::theme::label_size() as u32,
            fg,
            rotation,
        );

        if horizontal {
            Some(
                container(content)
                    .width(32)
                    .height(Length::Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(bg)),
                        border,
                        ..Default::default()
                    })
                    .into(),
            )
        } else {
            Some(
                container(content)
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .padding([0, 6])
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(bg)),
                        border,
                        ..Default::default()
                    })
                    .into(),
            )
        }
    }

    fn matches_output_state(&self, state: &crate::modules::ModuleState) -> bool {
        let surface_output = match state {
            crate::modules::ModuleState::WorkspaceOutput(output) => output.as_str(),
            _ => "",
        };
        surface_output.is_empty() || self.workspace.output.as_deref() == Some(surface_output)
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
            let mut segments = template.render_styled_segments_with_default(
                &self.format_values(),
                Some(self.state_key()),
                default_style,
            );
            let has_icon = self
                .config
                .workspaces
                .get(&(self.workspace.idx as u32))
                .is_some_and(|workspace| workspace.icon.is_some());
            for (index, (_, style)) in segments.iter_mut().enumerate() {
                // {icon} falls back to the workspace number when no icon is configured.
                style.size.get_or_insert(
                    if has_icon && template.segment_is_exact_placeholder(index, "icon") {
                        ICON_SIZE as f32
                    } else {
                        (crate::theme::label_size() as u32) as f32
                    },
                );
            }
            segments
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

    fn interaction_handler(
        &self,
    ) -> Option<&dyn crate::modules::interaction::ModuleInteractionHandler> {
        Some(self)
    }

    fn dynamic_instance(&self) -> Option<&dyn crate::modules::dynamic::DynamicModuleInstance> {
        Some(self)
    }

    fn dynamic_instance_mut(
        &mut self,
    ) -> Option<&mut (dyn crate::modules::dynamic::DynamicModuleInstance + '_)> {
        Some(self)
    }
}

impl crate::modules::interaction::ModuleInteractionHandler for WorkspaceItemModule {
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
}

impl crate::modules::dynamic::DynamicModuleInstance for WorkspaceItemModule {
    fn update_workspace(&mut self, workspace: &niri_ipc::Workspace) {
        if self.workspace.id == workspace.id {
            self.workspace = workspace.clone();
        }
    }

    fn config_name(&self) -> &str {
        self.id.configured_name()
    }
}

impl crate::modules::render::ModuleRender for WorkspaceItemModule {
    fn accessible_values(
        &self,
        _state: &crate::modules::ModuleState,
    ) -> Vec<(&'static str, String)> {
        let mut values = self.format_values();
        if self.workspace.is_urgent {
            values.push(("urgent", "true".into()));
        }
        values
    }

    fn has_content(
        &self,
        state: &crate::modules::ModuleState,
        mode: crate::config::FormatMode,
    ) -> bool {
        mode != crate::config::FormatMode::Expanded && self.matches_output_state(state)
    }

    fn render_collapsed_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        if !self.matches_output_state(state) {
            return iced::widget::Space::new().into();
        }
        self.build_view(false, bar_style)
    }

    fn render_bar_collapsed_with_style(
        &self,
        state: &crate::modules::ModuleState,
        context: &crate::modules::render::BarRenderContext<'_>,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        let rotation = self.config.content_rotation.resolve_for_edge(context.edge);
        if rotation == crate::config::ResolvedContentRotation::None
            || self
                .config
                .format
                .template_for(crate::config::FormatMode::Collapsed)
                .is_none()
            || !self.matches_output_state(state)
        {
            return None;
        }
        self.build_rotated_format_view(false, bar_style, rotation)
    }

    fn render_panel_with_style(
        &self,
        _state: &crate::modules::ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn render_horizontal_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        if !self.matches_output_state(state) {
            return Some(iced::widget::Space::new().into());
        }
        Some(self.build_view(true, bar_style))
    }

    fn render_bar_horizontal_with_style(
        &self,
        state: &crate::modules::ModuleState,
        context: &crate::modules::render::BarRenderContext<'_>,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        let rotation = self.config.content_rotation.resolve_for_edge(context.edge);
        if rotation == crate::config::ResolvedContentRotation::None
            || self
                .config
                .format
                .template_for(crate::config::FormatMode::Horizontal)
                .is_none()
            || !self.matches_output_state(state)
        {
            return None;
        }
        self.build_rotated_format_view(true, bar_style, rotation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ColorValue, SegmentStyles, Style, ThemeToken};
    use crate::modules::Module;
    use crate::modules::dynamic::DynamicModuleInstance;
    use crate::modules::interaction::ModuleInteractionHandler;
    use crate::modules::render::{BarRenderContext, ModuleRender};

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
    fn workspace_formats_size_number_fallback_as_text_and_configured_icon_as_icon() {
        use crate::config::{FormatMode, FormatTemplate, WorkspaceLabelConfig};
        let mut config = crate::config::WorkspacesConfig::default();
        config.format.collapsed = Some(
            FormatTemplate::parse(&["{icon}", "{label}"], &["icon", "label"], "test.format")
                .unwrap(),
        );
        for icon in [None, Some("◆".to_owned())] {
            config.workspaces.insert(
                1,
                WorkspaceLabelConfig {
                    icon: icon.clone(),
                    label: Some("web".into()),
                },
            );
            let module = WorkspaceItemModule::new(
                make_workspace(1, true),
                config.clone(),
                "workspaces".into(),
            );
            for mode in [FormatMode::Collapsed, FormatMode::Horizontal] {
                let parts = module
                    .styled_segments_with_default(mode, &Style::default())
                    .unwrap();
                assert_eq!(parts[0].0, icon.as_deref().unwrap_or("1"));
                assert_eq!(
                    parts[0].1.size,
                    Some(if icon.is_some() { 20.0 } else { 12.0 })
                );
                assert_eq!(parts[1].1.size, Some(12.0));
                let parts = module
                    .styled_segments_with_default(
                        mode,
                        &Style {
                            size: Some(18.0),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert!(parts.iter().all(|(_, style)| style.size == Some(18.0)));
            }
        }
    }

    #[test]
    fn workspaces_module_id_is_workspaces() {
        let m = WorkspacesModule::new(
            crate::modules::ModuleId::new("workspaces"),
            crate::config::WorkspacesConfig::default(),
        );
        assert_eq!(m.id().display_name(), "workspaces");
        assert_eq!(m.module_type(), "workspaces");
    }

    #[test]
    fn pill_colors_focused_returns_accent_base() {
        let (bg, fg) = pill_colors(true, true);
        assert_eq!(bg, theme::palette().accent);
        assert_eq!(fg, theme::palette().on_accent);
    }

    #[test]
    fn pill_colors_active_not_focused_returns_surface1_text() {
        let (bg, fg) = pill_colors(false, true);
        assert_eq!(bg, theme::palette().raised);
        assert_eq!(fg, theme::palette().text);
    }

    #[test]
    fn pill_colors_inactive_returns_transparent_text() {
        let (bg, fg) = pill_colors(false, false);
        assert_eq!(bg, iced::Color::TRANSPARENT);
        assert_eq!(fg, theme::palette().text);
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
        assert_eq!(m.id().display_name(), "workspaces");
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
        assert_eq!(m.id().display_name(), "workspace:1");
        assert_eq!(m.module_type(), "workspace_item");
        assert_eq!(m.config_name(), "workspaces");
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
            Some(theme::palette().accent)
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

    #[test]
    fn workspace_item_auto_rotation_overrides_side_bar_collapsed_format() {
        let mut config = workspace_format_config();
        config.content_rotation = crate::config::ContentRotation::Auto;
        let m = WorkspaceItemModule::new(make_workspace(0, true), config, "workspaces".into());
        let state = crate::modules::ModuleState::WorkspaceOutput("eDP-1".into());
        let edge = crate::config::Edge::Left;
        let context = BarRenderContext {
            window: iced::window::Id::unique(),
            edge: &edge,
        };

        assert!(
            m.render_bar_collapsed_with_style(&state, &context, &crate::config::Style::default())
                .is_some()
        );
    }

    #[test]
    fn workspace_item_explicit_rotation_overrides_horizontal_format() {
        let horizontal = crate::config::FormatTemplate::parse(
            &["{label}"],
            &["icon", "value", "index", "name", "label", "state"],
            "test.format.horizontal",
        )
        .unwrap();
        let config = crate::config::WorkspacesConfig {
            format: crate::config::FormatConfig {
                horizontal: Some(horizontal),
                ..Default::default()
            },
            content_rotation: crate::config::ContentRotation::Clockwise,
            ..Default::default()
        };
        let m = WorkspaceItemModule::new(make_workspace(0, true), config, "workspaces".into());
        let state = crate::modules::ModuleState::WorkspaceOutput("eDP-1".into());
        let edge = crate::config::Edge::Top;
        let context = BarRenderContext {
            window: iced::window::Id::unique(),
            edge: &edge,
        };

        assert!(
            m.render_bar_horizontal_with_style(&state, &context, &crate::config::Style::default())
                .is_some()
        );
    }
}
