use crate::modules::view::collapsed_icon_with_style;
use crate::theme;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FocusedWindowState {
    pub window: Option<FocusedWindowInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FocusedWindowInfo {
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub application_icon: Option<crate::focused_window_icons::ApplicationIcon>,
}

impl FocusedWindowInfo {
    pub(crate) fn from_window(
        window: &niri_ipc::Window,
        application_icon: Option<crate::focused_window_icons::ApplicationIcon>,
    ) -> Self {
        Self {
            title: window.title.clone(),
            app_id: window.app_id.clone(),
            application_icon,
        }
    }
}

fn exact_application_icon_segment<'a>(
    template: &crate::config::FormatTemplate,
    index: usize,
    info: &'a FocusedWindowInfo,
) -> Option<&'a crate::focused_window_icons::ApplicationIcon> {
    if !template.segment_is_exact_placeholder(index, "icon") {
        return None;
    }
    info.application_icon
        .as_ref()
        .filter(|icon| icon.path.is_some())
}

/// Focused-window module: shows the focused window title, fed by niri IPC via shared app state.
pub(super) struct FocusedWindowModule {
    id: crate::modules::ModuleId,
    format: crate::config::FormatConfig,
    content_rotation: crate::config::ContentRotation,
    fallback_icon: String,
}

impl FocusedWindowModule {
    #[cfg(test)]
    pub(super) fn new(
        id: crate::modules::ModuleId,
        format: crate::config::FormatConfig,
        fallback_icon: String,
    ) -> Self {
        Self::new_with_content_rotation(
            id,
            format,
            crate::config::ContentRotation::None,
            fallback_icon,
        )
    }

    pub(super) fn new_with_content_rotation(
        id: crate::modules::ModuleId,
        format: crate::config::FormatConfig,
        content_rotation: crate::config::ContentRotation,
        fallback_icon: String,
    ) -> Self {
        FocusedWindowModule {
            id,
            format,
            content_rotation,
            fallback_icon,
        }
    }

    fn formatted_text(
        &self,
        info: &FocusedWindowInfo,
        mode: crate::config::FormatMode,
    ) -> Option<String> {
        let title = info.title.as_deref().unwrap_or_default();
        let app_id = info.app_id.as_deref().unwrap_or_default();
        self.format.template_for(mode).map(|template| {
            template.render_joined(&[
                ("icon", self.fallback_icon.clone()),
                ("value", title.to_string()),
                ("title", title.to_string()),
                ("app-id", app_id.to_string()),
            ])
        })
    }

    #[cfg(test)]
    fn styled_segments_with_default(
        &self,
        info: &FocusedWindowInfo,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        use std::borrow::Cow;

        let title = info.title.as_deref().unwrap_or_default();
        let app_id = info.app_id.as_deref().unwrap_or_default();
        crate::modules::view::render_format_segments(
            &self.format,
            mode,
            &[
                ("icon", Cow::Borrowed(self.fallback_icon.as_str())),
                ("value", Cow::Borrowed(title)),
                ("title", Cow::Borrowed(title)),
                ("app-id", Cow::Borrowed(app_id)),
            ],
            None,
            default_style,
        )
    }

    fn formatted_segment_visuals(
        &self,
        info: &FocusedWindowInfo,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<crate::modules::view::FormattedSegmentVisual>> {
        use std::borrow::Cow;

        let template = self.format.template_for(mode)?;
        let title = info.title.as_deref().unwrap_or_default();
        let app_id = info.app_id.as_deref().unwrap_or_default();
        let values = [
            ("icon", Cow::Borrowed(self.fallback_icon.as_str())),
            ("value", Cow::Borrowed(title)),
            ("title", Cow::Borrowed(title)),
            ("app-id", Cow::Borrowed(app_id)),
        ];
        let parts = template
            .render_styled_cow_segments_with_default(&values, None, default_style)
            .into_iter()
            .enumerate()
            .map(|(index, (segment, style))| {
                if let Some(icon) = exact_application_icon_segment(template, index, info)
                    && let Some(path) = icon.path.as_ref()
                {
                    return crate::modules::view::FormattedSegmentVisual::Image {
                        path: path.clone(),
                        fallback_text: segment,
                        style,
                    };
                }
                crate::modules::view::FormattedSegmentVisual::Text {
                    text: segment,
                    style,
                }
            })
            .collect::<Vec<_>>();

        Some(parts)
    }

    fn formatted_segments_content_with_icons(
        &self,
        info: &FocusedWindowInfo,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
        fallback_size: u32,
        fallback_color: iced::Color,
        rotation: crate::config::ResolvedContentRotation,
    ) -> Option<iced::Element<'static, crate::app::Message>> {
        self.formatted_segment_visuals(info, mode, default_style)
            .map(|segments| {
                crate::modules::view::formatted_segment_visuals_content(
                    segments,
                    fallback_size,
                    fallback_color,
                    rotation,
                )
            })
    }

    fn formatted_segments_content_with_icons_default(
        &self,
        info: &FocusedWindowInfo,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
        fallback_size: u32,
        fallback_color: iced::Color,
    ) -> Option<iced::Element<'static, crate::app::Message>> {
        self.formatted_segments_content_with_icons(
            info,
            mode,
            default_style,
            fallback_size,
            fallback_color,
            crate::config::ResolvedContentRotation::None,
        )
    }
}

impl crate::modules::Module for FocusedWindowModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> crate::modules::ModuleKind {
        crate::modules::ModuleKind::FocusedWindow
    }

    /// No independent data source — fed via Message::NiriEvent in app.rs (same pattern as workspaces).
    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }
}

impl crate::modules::render::ModuleRender for FocusedWindowModule {
    fn tooltip_values(&self, state: &crate::modules::ModuleState) -> Vec<(&'static str, String)> {
        if let crate::modules::ModuleState::FocusedWindow(s) = state {
            let Some(info) = s.window.as_ref() else {
                return Vec::new();
            };
            let title = info.title.clone().unwrap_or_default();
            let app_id = info.app_id.clone().unwrap_or_default();
            vec![
                ("icon", self.fallback_icon.clone()),
                ("value", title.clone()),
                ("title", title),
                ("app-id", app_id),
            ]
        } else {
            Vec::new()
        }
    }

    fn render_collapsed_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;

        let info = match state {
            ModuleState::FocusedWindow(s) => s.window.as_ref(),
            _ => return iced::widget::Space::new().into(), // Empty/pending render as empty
        };

        // No focused window → render nothing (AC #4, UX-DR16).
        let Some(info) = info else {
            return iced::widget::Space::new().into();
        };

        if let Some(content) = self.formatted_segments_content_with_icons_default(
            info,
            crate::config::FormatMode::Collapsed,
            bar_style,
            16,
            theme::TEXT,
        ) {
            return iced::widget::container(content)
                .width(iced::Length::Fill)
                .height(20)
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center)
                .into();
        }
        if let Some(formatted) = self.formatted_text(info, crate::config::FormatMode::Collapsed) {
            return collapsed_icon_with_style(formatted, theme::TEXT, bar_style);
        }

        // Window is focused -> show icon placeholder (AC #2).
        // "□" (U+25A1) is a window-shape placeholder; replaced when icon system is added.
        collapsed_icon_with_style(self.fallback_icon.clone(), theme::TEXT, bar_style)
    }

    fn render_bar_collapsed_with_style(
        &self,
        state: &crate::modules::ModuleState,
        context: &crate::modules::render::BarRenderContext<'_>,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        let rotation = self.content_rotation.resolve_for_edge(context.edge);
        if rotation == crate::config::ResolvedContentRotation::None
            || self
                .format
                .template_for(crate::config::FormatMode::Collapsed)
                .is_none()
        {
            return None;
        }
        let crate::modules::ModuleState::FocusedWindow(focused_window) = state else {
            return None;
        };
        let info = focused_window.window.as_ref()?;
        let segments =
            self.formatted_segment_visuals(info, crate::config::FormatMode::Collapsed, bar_style)?;
        Some(
            iced::widget::container(crate::modules::view::formatted_segment_visuals_content(
                segments,
                16,
                theme::TEXT,
                rotation,
            ))
            .width(iced::Length::Fill)
            .height(iced::Length::Shrink)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center)
            .into(),
        )
    }

    fn render_panel_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::container;
        use iced::{Alignment, Length};

        let info = match state {
            ModuleState::FocusedWindow(s) => s.window.as_ref(),
            _ => return iced::widget::Space::new().into(),
        };

        // No focused window → render nothing (AC #4, UX-DR16).
        let Some(info) = info else {
            return iced::widget::Space::new().into();
        };
        let title_str = info.title.clone().unwrap_or_default();

        if let Some(content) = self.formatted_segments_content_with_icons_default(
            info,
            crate::config::FormatMode::Expanded,
            bar_style,
            12,
            theme::TEXT,
        ) {
            return container(content)
                .width(Length::Fill)
                .height(20)
                .align_x(Alignment::Start)
                .align_y(Alignment::Center)
                .padding([0, 6])
                .into();
        }

        let display = self
            .formatted_text(info, crate::config::FormatMode::Expanded)
            .unwrap_or(title_str);

        // Show title, clipped to module width (AC #3).
        // Iced clips text naturally when container width limits it.
        container(crate::modules::view::styled_segment_text(
            display,
            12,
            theme::TEXT,
            crate::modules::view::fallback_resolved_style(bar_style),
        ))
        .width(Length::Fill)
        .height(20)
        .align_x(Alignment::Start)
        .align_y(Alignment::Center)
        .padding([0, 6])
        .into()
    }

    fn render_horizontal_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        use crate::modules::ModuleState;
        use iced::widget::container;
        use iced::{Alignment, Length};

        let info = match state {
            ModuleState::FocusedWindow(s) => s.window.as_ref(),
            _ => return Some(iced::widget::Space::new().into()),
        };

        let Some(info) = info else {
            return Some(iced::widget::Space::new().into());
        };
        let title_str = info.title.clone().unwrap_or_default();

        if let Some(content) = self.formatted_segments_content_with_icons_default(
            info,
            crate::config::FormatMode::Horizontal,
            bar_style,
            12,
            theme::TEXT,
        ) {
            return Some(
                container(content)
                    .width(Length::Shrink)
                    .height(Length::Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .padding([0, 6])
                    .into(),
            );
        }

        let display = self
            .formatted_text(info, crate::config::FormatMode::Horizontal)
            .unwrap_or(title_str);

        Some(
            container(crate::modules::view::styled_segment_text(
                display,
                12,
                theme::TEXT,
                crate::modules::view::fallback_resolved_style(bar_style),
            ))
            .width(Length::Shrink)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .padding([0, 6])
            .into(),
        )
    }

    fn render_bar_horizontal_with_style(
        &self,
        state: &crate::modules::ModuleState,
        context: &crate::modules::render::BarRenderContext<'_>,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        let rotation = self.content_rotation.resolve_for_edge(context.edge);
        if rotation == crate::config::ResolvedContentRotation::None
            || self
                .format
                .template_for(crate::config::FormatMode::Horizontal)
                .is_none()
        {
            return None;
        }
        let crate::modules::ModuleState::FocusedWindow(focused_window) = state else {
            return None;
        };
        let info = focused_window.window.as_ref()?;
        let segments =
            self.formatted_segment_visuals(info, crate::config::FormatMode::Horizontal, bar_style)?;
        Some(
            iced::widget::container(crate::modules::view::formatted_segment_visuals_content(
                segments,
                12,
                theme::TEXT,
                rotation,
            ))
            .width(iced::Length::Shrink)
            .height(iced::Length::Fill)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center)
            .padding([0, 6])
            .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ColorValue, SegmentStyles, Style, ThemeToken, Weight};
    use crate::modules::Module;
    use crate::modules::render::{BarRenderContext, ModuleRender};

    fn styled_format_config() -> crate::config::FormatConfig {
        let mut template = crate::config::FormatTemplate::parse(
            &["{title}"],
            &["icon", "value", "title"],
            "test.format",
        )
        .unwrap();
        template.set_segment_styles(
            0,
            SegmentStyles::Static(Style {
                color: Some(ColorValue::Token(ThemeToken::Accent)),
                weight: Some(Weight::Bold),
                ..Default::default()
            }),
        );
        crate::config::FormatConfig {
            collapsed: Some(template),
            ..Default::default()
        }
    }

    #[test]
    fn focused_window_module_id_is_focused_window() {
        let m = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            crate::config::FormatConfig::default(),
            "□".into(),
        );
        assert_eq!(m.id().0, "focused-window");
        assert_eq!(m.module_type(), "focused-window");
    }

    #[test]
    fn focused_window_state_default_has_no_window() {
        let state = FocusedWindowState::default();
        assert!(state.window.is_none());
    }

    #[test]
    fn focused_window_state_stores_window_info() {
        let state = FocusedWindowState {
            window: Some(FocusedWindowInfo {
                title: Some("Firefox".to_string()),
                app_id: Some("firefox.desktop".to_string()),
                application_icon: None,
            }),
        };
        let info = state.window.as_ref().expect("window info");
        assert_eq!(info.title.as_deref(), Some("Firefox"));
        assert_eq!(info.app_id.as_deref(), Some("firefox.desktop"));
    }

    #[test]
    fn tooltip_values_keep_missing_title_empty_when_window_is_present() {
        let m = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            crate::config::FormatConfig::default(),
            "app".into(),
        );
        let state = crate::modules::ModuleState::FocusedWindow(FocusedWindowState {
            window: Some(FocusedWindowInfo {
                title: None,
                app_id: Some("org.example.App".into()),
                application_icon: None,
            }),
        });

        let values = m.tooltip_values(&state);

        assert_eq!(
            values,
            vec![
                ("icon", "app".into()),
                ("value", String::new()),
                ("title", String::new()),
                ("app-id", "org.example.App".into()),
            ]
        );
    }

    #[test]
    fn focused_window_styled_segments_resolve_inline_style() {
        let m = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            styled_format_config(),
            "□".into(),
        );
        let info = FocusedWindowInfo {
            title: Some("Firefox".into()),
            app_id: None,
            application_icon: None,
        };
        let segment = &m
            .styled_segments_with_default(
                &info,
                crate::config::FormatMode::Collapsed,
                &crate::config::Style::default(),
            )
            .unwrap()[0];

        assert_eq!(segment.1.color, Some(crate::theme::ACCENT));
        assert_eq!(segment.1.weight, Some(Weight::Bold));
    }

    #[test]
    fn exact_icon_segment_uses_application_icon_path_only_when_exact() {
        let exact = crate::config::FormatTemplate::parse(
            &["{icon}", "{title}"],
            &["icon", "value", "title", "app-id"],
            "test.format",
        )
        .unwrap();
        let mixed = crate::config::FormatTemplate::parse(
            &["{icon}: {title}"],
            &["icon", "value", "title", "app-id"],
            "test.format",
        )
        .unwrap();
        let info = FocusedWindowInfo {
            title: Some("Firefox".into()),
            app_id: Some("firefox".into()),
            application_icon: Some(crate::focused_window_icons::ApplicationIcon {
                name: "firefox".into(),
                path: Some(std::path::PathBuf::from("/tmp/firefox.svg")),
            }),
        };

        assert!(exact_application_icon_segment(&exact, 0, &info).is_some());
        assert!(exact_application_icon_segment(&exact, 1, &info).is_none());
        assert!(exact_application_icon_segment(&mixed, 0, &info).is_none());
    }

    #[test]
    fn exact_icon_segment_without_resolved_path_uses_text_fallback() {
        let exact = crate::config::FormatTemplate::parse(
            &["{icon}"],
            &["icon", "value", "title", "app-id"],
            "test.format",
        )
        .unwrap();
        let info = FocusedWindowInfo {
            title: Some("Firefox".into()),
            app_id: Some("firefox".into()),
            application_icon: Some(crate::focused_window_icons::ApplicationIcon {
                name: "firefox".into(),
                path: None,
            }),
        };

        assert!(exact_application_icon_segment(&exact, 0, &info).is_none());
    }

    #[test]
    fn exact_icon_visual_segment_preserves_segment_size() {
        let mut template = crate::config::FormatTemplate::parse(
            &["{icon}"],
            &["icon", "value", "title", "app-id"],
            "test.format",
        )
        .unwrap();
        template.set_segment_styles(
            0,
            SegmentStyles::Static(Style {
                size: Some(18.0),
                ..Default::default()
            }),
        );
        let m = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            crate::config::FormatConfig {
                collapsed: Some(template),
                ..Default::default()
            },
            "□".into(),
        );
        let info = FocusedWindowInfo {
            title: Some("Firefox".into()),
            app_id: Some("firefox".into()),
            application_icon: Some(crate::focused_window_icons::ApplicationIcon {
                name: "firefox".into(),
                path: Some(std::path::PathBuf::from("/tmp/firefox.svg")),
            }),
        };

        let segments = m
            .formatted_segment_visuals(
                &info,
                crate::config::FormatMode::Collapsed,
                &crate::config::Style::default(),
            )
            .unwrap();

        assert!(matches!(
            &segments[0],
            crate::modules::view::FormattedSegmentVisual::Image { style, .. }
                if style.size == Some(18.0)
        ));
    }

    #[test]
    fn focused_window_auto_rotation_overrides_side_bar_collapsed_format() {
        let m = FocusedWindowModule::new_with_content_rotation(
            crate::modules::ModuleId::new("focused-window"),
            styled_format_config(),
            crate::config::ContentRotation::Auto,
            "□".into(),
        );
        let state = crate::modules::ModuleState::FocusedWindow(FocusedWindowState {
            window: Some(FocusedWindowInfo {
                title: Some("Firefox".into()),
                app_id: None,
                application_icon: None,
            }),
        });
        let edge = crate::config::Edge::Left;
        let context = BarRenderContext {
            window: iced::window::Id::unique(),
            edge: &edge,
            horizontal: false,
        };

        assert!(
            m.render_bar_collapsed_with_style(&state, &context, &crate::config::Style::default())
                .is_some()
        );
    }

    #[test]
    fn focused_window_explicit_rotation_overrides_horizontal_format() {
        let horizontal = crate::config::FormatTemplate::parse(
            &["{title}"],
            &["icon", "value", "title", "app-id"],
            "test.format.horizontal",
        )
        .unwrap();
        let m = FocusedWindowModule::new_with_content_rotation(
            crate::modules::ModuleId::new("focused-window"),
            crate::config::FormatConfig {
                horizontal: Some(horizontal),
                ..Default::default()
            },
            crate::config::ContentRotation::Clockwise,
            "□".into(),
        );
        let state = crate::modules::ModuleState::FocusedWindow(FocusedWindowState {
            window: Some(FocusedWindowInfo {
                title: Some("Firefox".into()),
                app_id: None,
                application_icon: None,
            }),
        });
        let edge = crate::config::Edge::Top;
        let context = BarRenderContext {
            window: iced::window::Id::unique(),
            edge: &edge,
            horizontal: true,
        };

        assert!(
            m.render_bar_horizontal_with_style(&state, &context, &crate::config::Style::default())
                .is_some()
        );
    }
}
