use crate::modules::view::collapsed_icon;
use crate::theme;

/// Runtime state for the focused-window module.
///
/// `title = None` means no window is focused → module renders nothing (UX-DR16).
/// Populated by WindowFocusChanged / WindowsChanged / WindowOpenedOrChanged / WindowClosed
/// events via app.rs update(). Story 4.5 may set Unavailable instead.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FocusedWindowState {
    pub title: Option<String>,
}

/// Focused-window module: shows the focused window title, fed by niri IPC via shared app state.
pub(super) struct FocusedWindowModule {
    id: crate::modules::ModuleId,
    format: crate::config::FormatConfig,
}

impl FocusedWindowModule {
    pub(super) fn new(id: crate::modules::ModuleId, format: crate::config::FormatConfig) -> Self {
        FocusedWindowModule { id, format }
    }

    fn formatted_text(&self, title: &str, mode: crate::config::FormatMode) -> Option<String> {
        self.format.template_for(mode).map(|template| {
            template.render_joined(&[
                ("icon", "□".to_string()),
                ("value", title.to_string()),
                ("title", title.to_string()),
            ])
        })
    }

    fn styled_segments(
        &self,
        title: &str,
        mode: crate::config::FormatMode,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        self.format.template_for(mode).map(|template| {
            template.render_styled_segments(
                &[
                    ("icon", "□".to_string()),
                    ("value", title.to_string()),
                    ("title", title.to_string()),
                ],
                None,
            )
        })
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

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;

        let title = match state {
            ModuleState::FocusedWindow(s) => &s.title,
            _ => return iced::widget::Space::new().into(), // Loading → empty
        };

        // No focused window → render nothing (AC #4, UX-DR16).
        if title.is_none() {
            return iced::widget::Space::new().into();
        }

        if let Some(title) = title.as_deref() {
            if let Some(segments) =
                self.styled_segments(title, crate::config::FormatMode::Collapsed)
            {
                return iced::widget::container(
                    iced::widget::row(
                        segments
                            .into_iter()
                            .map(|(segment, style)| {
                                crate::modules::view::styled_segment_text(
                                    segment,
                                    16,
                                    theme::TEXT,
                                    style,
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .width(iced::Length::Fill)
                .height(20)
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center)
                .into();
            }
            if let Some(formatted) =
                self.formatted_text(title, crate::config::FormatMode::Collapsed)
            {
                return collapsed_icon(formatted, theme::TEXT);
            }
        }

        // Window is focused -> show icon placeholder (AC #2).
        // "□" (U+25A1) is a window-shape placeholder; replaced when icon system is added.
        collapsed_icon("□", theme::TEXT)
    }

    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{container, text};
        use iced::{Alignment, Length};

        let title = match state {
            ModuleState::FocusedWindow(s) => &s.title,
            _ => return iced::widget::Space::new().into(),
        };

        // No focused window → render nothing (AC #4, UX-DR16).
        let Some(title_str) = title.clone() else {
            return iced::widget::Space::new().into();
        };

        if let Some(segments) =
            self.styled_segments(&title_str, crate::config::FormatMode::Expanded)
        {
            return container(
                iced::widget::row(
                    segments
                        .into_iter()
                        .map(|(segment, style)| {
                            crate::modules::view::styled_segment_text(
                                segment,
                                12,
                                theme::TEXT,
                                style,
                            )
                        })
                        .collect::<Vec<_>>(),
                )
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .height(20)
            .align_x(Alignment::Start)
            .align_y(Alignment::Center)
            .padding([0, 6])
            .into();
        }

        let display = self
            .formatted_text(&title_str, crate::config::FormatMode::Expanded)
            .unwrap_or(title_str);

        // Show title, clipped to module width (AC #3).
        // Iced clips text naturally when container width limits it.
        container(
            text(display)
                .size(12)
                .color(theme::TEXT)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Length::Fill)
        .height(20)
        .align_x(Alignment::Start)
        .align_y(Alignment::Center)
        .padding([0, 6])
        .into()
    }

    fn view_horizontal(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{container, text};
        use iced::{Alignment, Length};

        let title = match state {
            ModuleState::FocusedWindow(s) => &s.title,
            _ => return iced::widget::Space::new().into(),
        };

        let Some(title_str) = title.clone() else {
            return iced::widget::Space::new().into();
        };

        if let Some(segments) =
            self.styled_segments(&title_str, crate::config::FormatMode::Horizontal)
        {
            return container(
                iced::widget::row(
                    segments
                        .into_iter()
                        .map(|(segment, style)| {
                            crate::modules::view::styled_segment_text(
                                segment,
                                12,
                                theme::TEXT,
                                style,
                            )
                        })
                        .collect::<Vec<_>>(),
                )
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Start)
            .align_y(Alignment::Center)
            .padding([0, 6])
            .into();
        }

        let display = self
            .formatted_text(&title_str, crate::config::FormatMode::Horizontal)
            .unwrap_or(title_str);

        container(
            text(display)
                .size(12)
                .color(theme::TEXT)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Start)
        .align_y(Alignment::Center)
        .padding([0, 6])
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ColorValue, SegmentStyles, Style, ThemeToken, Weight};
    use crate::modules::Module;

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
        );
        assert_eq!(m.id().0, "focused-window");
        assert_eq!(m.module_type(), "focused-window");
    }

    #[test]
    fn focused_window_state_default_has_no_title() {
        let state = FocusedWindowState::default();
        assert!(state.title.is_none());
    }

    #[test]
    fn focused_window_state_stores_title() {
        let state = FocusedWindowState {
            title: Some("Firefox".to_string()),
        };
        assert_eq!(state.title.as_deref(), Some("Firefox"));
    }

    #[test]
    fn focused_window_styled_segments_resolve_inline_style() {
        let m = FocusedWindowModule::new(
            crate::modules::ModuleId::new("focused-window"),
            styled_format_config(),
        );
        let segment = &m
            .styled_segments("Firefox", crate::config::FormatMode::Collapsed)
            .unwrap()[0];

        assert_eq!(segment.1.color, Some(crate::theme::ACCENT));
        assert_eq!(segment.1.weight, Some(Weight::Bold));
    }
}
