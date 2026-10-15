use crate::modules::view::collapsed_icon_with_style;
use crate::theme;
use std::borrow::Cow;
use unicode_segmentation::UnicodeSegmentation;

fn ellipsize_title(title: &str, max_length: u32) -> Cow<'_, str> {
    let max_length = max_length as usize;
    if max_length == 0 || title.graphemes(true).count() <= max_length {
        return Cow::Borrowed(title);
    }

    let retained = max_length - 1;
    let prefix: String = title.graphemes(true).take(retained.div_ceil(2)).collect();
    let suffix: String = title
        .graphemes(true)
        .rev()
        .take(retained / 2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Cow::Owned(format!("{prefix}…{suffix}"))
}

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
    max_length: u32,
}

impl FocusedWindowModule {
    pub(super) fn new(
        id: crate::modules::ModuleId,
        config: crate::config::FocusedWindowConfig,
    ) -> Self {
        FocusedWindowModule {
            id,
            format: config.format,
            content_rotation: config.content_rotation,
            fallback_icon: config.fallback_icon,
            max_length: config.max_length,
        }
    }

    fn display_title<'a>(&self, info: &'a FocusedWindowInfo) -> Cow<'a, str> {
        ellipsize_title(info.title.as_deref().unwrap_or_default(), self.max_length)
    }

    fn formatted_text(
        &self,
        info: &FocusedWindowInfo,
        mode: crate::config::FormatMode,
    ) -> Option<String> {
        let title = self.display_title(info);
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
        let title = self.display_title(info);
        let app_id = info.app_id.as_deref().unwrap_or_default();
        crate::modules::view::render_format_segments_with_text_size(
            &self.format,
            mode,
            &[
                ("icon", Cow::Borrowed(self.fallback_icon.as_str())),
                ("value", Cow::Borrowed(title.as_ref())),
                ("title", Cow::Borrowed(title.as_ref())),
                ("app-id", Cow::Borrowed(app_id)),
            ],
            None,
            default_style,
            crate::theme::title_size() as u32,
        )
    }

    fn formatted_segment_visuals(
        &self,
        info: &FocusedWindowInfo,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<crate::modules::view::FormattedSegmentVisual>> {
        let template = self.format.template_for(mode)?;
        let title = self.display_title(info);
        let app_id = info.app_id.as_deref().unwrap_or_default();
        let values = [
            ("icon", Cow::Borrowed(self.fallback_icon.as_str())),
            ("value", Cow::Borrowed(title.as_ref())),
            ("title", Cow::Borrowed(title.as_ref())),
            ("app-id", Cow::Borrowed(app_id)),
        ];
        let parts = crate::modules::view::render_format_segments_with_text_size(
            &self.format,
            mode,
            &values,
            None,
            default_style,
            crate::theme::title_size() as u32,
        )?
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
            crate::theme::title_size() as u32,
            theme::palette().text,
        ) {
            return iced::widget::container(content)
                .width(iced::Length::Fill)
                .height(20)
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center)
                .into();
        }
        if let Some(formatted) = self.formatted_text(info, crate::config::FormatMode::Collapsed) {
            return collapsed_icon_with_style(formatted, theme::palette().text, bar_style);
        }

        // Window is focused -> show icon placeholder (AC #2).
        // "□" (U+25A1) is a window-shape placeholder; replaced when icon system is added.
        collapsed_icon_with_style(self.fallback_icon.clone(), theme::palette().text, bar_style)
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
                crate::theme::title_size() as u32,
                theme::palette().text,
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
        let title_str = self.display_title(info).into_owned();

        if let Some(content) = self.formatted_segments_content_with_icons_default(
            info,
            crate::config::FormatMode::Expanded,
            bar_style,
            12,
            theme::palette().text,
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

        // Apply the title limit even when no format template is available.
        container(crate::modules::view::styled_segment_text(
            display,
            12,
            theme::palette().text,
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
        let title_str = self.display_title(info).into_owned();

        if let Some(content) = self.formatted_segments_content_with_icons_default(
            info,
            crate::config::FormatMode::Horizontal,
            bar_style,
            crate::theme::title_size() as u32,
            theme::palette().text,
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
                crate::theme::title_size() as u32,
                theme::palette().text,
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
                crate::theme::title_size() as u32,
                theme::palette().text,
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
    use crate::config::{
        ColorValue, FocusedWindowConfig, SegmentStyles, Style, ThemeToken, Weight,
    };
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
    fn middle_ellipsis_respects_boundaries_and_small_or_disabled_limits() {
        for (limit, expected) in [
            (0, "abcdef"),
            (1, "…"),
            (2, "a…"),
            (3, "a…f"),
            (4, "ab…f"),
            (5, "ab…ef"),
            (6, "abcdef"),
            (7, "abcdef"),
        ] {
            assert_eq!(ellipsize_title("abcdef", limit), expected, "limit={limit}");
            assert_eq!(ellipsize_title("", limit), "");
        }
        assert!(matches!(ellipsize_title("short", 50), Cow::Borrowed(_)));
    }

    #[test]
    fn middle_ellipsis_preserves_combining_marks_and_complete_emoji_sequences() {
        let title = "e\u{301}👩🏽‍💻中🇫🇷o\u{308}👨‍👩‍👧‍👦";
        let shortened = ellipsize_title(title, 5);
        assert_eq!(shortened, "e\u{301}👩🏽‍💻…o\u{308}👨‍👩‍👧‍👦");
        assert_eq!(shortened.graphemes(true).count(), 5);
        assert_eq!(ellipsize_title(title, 6), title);
    }

    #[test]
    fn custom_formats_limit_both_title_aliases_and_preserve_other_values_and_styles() {
        let mut template = crate::config::FormatTemplate::parse(
            &["Title: {title}", "{value}", "{app-id}"],
            &["title", "value", "app-id"],
            "test.format",
        )
        .unwrap();
        template.set_segment_styles(
            0,
            SegmentStyles::Static(Style {
                weight: Some(Weight::Bold),
                ..Default::default()
            }),
        );
        let module = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            FocusedWindowConfig {
                max_length: 5,
                format: crate::config::FormatConfig {
                    collapsed: Some(template),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let info = FocusedWindowInfo {
            title: Some("abcdefghij".into()),
            app_id: Some("org.example.Application".into()),
            application_icon: None,
        };
        for mode in [
            crate::config::FormatMode::Collapsed,
            crate::config::FormatMode::Expanded,
            crate::config::FormatMode::Horizontal,
        ] {
            let segments = module
                .styled_segments_with_default(&info, mode, &Style::default())
                .unwrap();
            assert_eq!(segments[0].0, "Title: ab…ij");
            assert_eq!(segments[0].1.weight, Some(Weight::Bold));
            assert_eq!(segments[1].0, "ab…ij");
            assert_eq!(segments[2].0, "org.example.Application");
            assert_eq!(
                module.formatted_text(&info, mode).unwrap(),
                "Title: ab…ij ab…ij org.example.Application"
            );
        }
        let missing_title = FocusedWindowInfo {
            title: None,
            ..info
        };
        assert_eq!(module.display_title(&missing_title), "");
    }

    #[test]
    fn long_titles_are_bounded_in_every_render_mode_but_tooltips_keep_the_full_title() {
        let module = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            FocusedWindowConfig::default(),
        );
        let title = format!("{} — Firefox", "Very long window title ".repeat(100));
        let info = FocusedWindowInfo {
            title: Some(title.clone()),
            app_id: Some("firefox".into()),
            application_icon: None,
        };
        for mode in [
            crate::config::FormatMode::Collapsed,
            crate::config::FormatMode::Horizontal,
            crate::config::FormatMode::Expanded,
        ] {
            let segments = module
                .formatted_segment_visuals(&info, mode, &Style::default())
                .expect("inherited title format");
            let crate::modules::view::FormattedSegmentVisual::Text { text, .. } = &segments[1]
            else {
                panic!("expected title text");
            };
            assert_eq!(text.chars().count(), 50, "{mode:?}");
            assert!(text.starts_with("Very long window title "));
            assert!(text.contains('…'));
            assert!(text.ends_with(" — Firefox"));
        }
        let state =
            crate::modules::ModuleState::FocusedWindow(FocusedWindowState { window: Some(info) });
        for (name, value) in module.tooltip_values(&state) {
            if matches!(name, "title" | "value") {
                assert_eq!(value, title);
            }
        }
    }

    #[test]
    fn focused_window_module_id_is_focused_window() {
        let m = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            FocusedWindowConfig::default(),
        );
        assert_eq!(m.id().display_name(), "focused-window");
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
            FocusedWindowConfig {
                fallback_icon: "app".into(),
                ..Default::default()
            },
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
            FocusedWindowConfig {
                format: styled_format_config(),
                ..Default::default()
            },
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

        assert_eq!(segment.1.color, Some(crate::theme::palette().accent));
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
            FocusedWindowConfig {
                format: crate::config::FormatConfig {
                    collapsed: Some(template),
                    ..Default::default()
                },
                ..Default::default()
            },
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
    fn application_icon_and_window_title_preserve_bar_defaults_and_overrides() {
        let template = crate::config::FormatTemplate::parse(
            &["{icon}", "{title}"],
            &["icon", "title"],
            "test.format",
        )
        .unwrap();
        let module = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            FocusedWindowConfig {
                format: crate::config::FormatConfig {
                    collapsed: Some(template),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        for path in [None, Some(std::path::PathBuf::from("/tmp/firefox.svg"))] {
            let info = FocusedWindowInfo {
                title: Some("Quarterly planning".into()),
                app_id: Some("firefox".into()),
                application_icon: Some(crate::focused_window_icons::ApplicationIcon {
                    name: "firefox".into(),
                    path,
                }),
            };
            for mode in [
                crate::config::FormatMode::Collapsed,
                crate::config::FormatMode::Horizontal,
            ] {
                let parts = module
                    .formatted_segment_visuals(&info, mode, &Style::default())
                    .unwrap();
                match &parts[0] {
                    crate::modules::view::FormattedSegmentVisual::Image { style, .. }
                    | crate::modules::view::FormattedSegmentVisual::Text { style, .. } => {
                        assert_eq!(style.size, Some(20.0))
                    }
                }
                assert!(
                    matches!(&parts[1], crate::modules::view::FormattedSegmentVisual::Text { style, .. }
                    if style.size == Some(20.0))
                );
                let overridden = module
                    .formatted_segment_visuals(
                        &info,
                        mode,
                        &Style {
                            size: Some(18.0),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert!(overridden.iter().all(|part| match part {
                    crate::modules::view::FormattedSegmentVisual::Image { style, .. }
                    | crate::modules::view::FormattedSegmentVisual::Text { style, .. } =>
                        style.size == Some(18.0),
                }));
            }
        }
    }

    #[test]
    fn focused_window_auto_rotation_overrides_side_bar_collapsed_format() {
        let m = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            FocusedWindowConfig {
                format: styled_format_config(),
                content_rotation: crate::config::ContentRotation::Auto,
                ..Default::default()
            },
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
        let m = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            FocusedWindowConfig {
                format: crate::config::FormatConfig {
                    horizontal: Some(horizontal),
                    ..Default::default()
                },
                content_rotation: crate::config::ContentRotation::Clockwise,
                ..Default::default()
            },
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
        };

        assert!(
            m.render_bar_horizontal_with_style(&state, &context, &crate::config::Style::default())
                .is_some()
        );
    }
}
