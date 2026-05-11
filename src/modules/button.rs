/// Runtime state for the button module. Button appearance is config-driven, not data-driven.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonState;

/// Events emitted by the button module.
#[derive(Debug, Clone, PartialEq)]
pub enum ButtonEvent {
    /// Emitted once at startup to transition Loading → Button(ButtonState).
    Ready,
}

pub(super) struct ButtonModule {
    id: crate::modules::ModuleId,
    label: String,
    icon: String,
    on_click: crate::config::CommandConfig,
    format: crate::config::FormatConfig,
}

impl ButtonModule {
    pub(super) fn new(
        id: crate::modules::ModuleId,
        label: String,
        icon: String,
        on_click: crate::config::CommandConfig,
        format: crate::config::FormatConfig,
    ) -> Self {
        ButtonModule {
            id,
            label,
            icon,
            on_click,
            format,
        }
    }

    fn formatted_text(&self, mode: crate::config::FormatMode) -> Option<String> {
        self.format.template_for(mode).map(|template| {
            template.render_joined(&[
                ("icon", self.icon.clone()),
                ("value", self.label.clone()),
                ("label", self.label.clone()),
            ])
        })
    }

    #[cfg(test)]
    fn styled_segments(
        &self,
        mode: crate::config::FormatMode,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        self.styled_segments_with_default(mode, &crate::config::Style::default())
    }

    fn styled_segments_with_default(
        &self,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        use std::borrow::Cow;

        crate::modules::view::render_format_segments(
            &self.format,
            mode,
            &[
                ("icon", Cow::Borrowed(self.icon.as_str())),
                ("value", Cow::Borrowed(self.label.as_str())),
                ("label", Cow::Borrowed(self.label.as_str())),
            ],
            None,
            default_style,
        )
    }
}

impl crate::modules::Module for ButtonModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> crate::modules::ModuleKind {
        crate::modules::ModuleKind::Button
    }

    fn tooltip_values(&self, _state: &crate::modules::ModuleState) -> Vec<(&'static str, String)> {
        vec![
            ("icon", self.icon.clone()),
            ("value", self.label.clone()),
            ("label", self.label.clone()),
        ]
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::run_with(self.id.clone(), |id: &crate::modules::ModuleId| {
            let id = id.clone();
            iced::futures::stream::unfold(Some(id), |state| async move {
                match state {
                    Some(id) => {
                        let msg = crate::app::Message::Modules(
                            crate::app::messages::ModuleRuntimeEvent::Module(
                                id,
                                crate::modules::ModuleEvent::Button(ButtonEvent::Ready),
                            ),
                        );
                        Some((msg, None))
                    }
                    None => {
                        // Keep subscription alive but emit nothing further.
                        std::future::pending::<()>().await;
                        unreachable!()
                    }
                }
            })
        })
    }

    fn on_interaction(
        &self,
        event: crate::modules::InteractionEvent,
    ) -> Option<crate::app::Message> {
        use crate::modules::InteractionEvent;
        match event {
            InteractionEvent::Clicked => {
                // Fire-and-forget from the UI, but reap the child in a background thread.
                let _ = crate::command::spawn_config_command_fire_and_forget(&self.on_click);
                None
            }
            _ => None,
        }
    }

    // Button keeps custom collapsed/panel views because its panel layout
    // right-aligns the label with a spacer instead of using the shared icon-label row.
    fn view_collapsed_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use crate::modules::view::{
            fallback_resolved_style, styled_segment_text, styled_segments_row,
        };
        use iced::widget::column;
        use iced::{Alignment, Length};

        match state {
            ModuleState::Button(_) => {
                if let Some(segments) = self
                    .styled_segments_with_default(crate::config::FormatMode::Collapsed, bar_style)
                {
                    return styled_segments_row(segments, 16, crate::theme::TEXT)
                        .width(Length::Fill)
                        .into();
                }
                let content = self
                    .formatted_text(crate::config::FormatMode::Collapsed)
                    .unwrap_or_else(|| self.icon.clone());
                column![styled_segment_text(
                    content,
                    16,
                    crate::theme::TEXT,
                    fallback_resolved_style(bar_style),
                )]
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            _ => iced::widget::Space::new().into(), // Loading/Unavailable → empty (UX-DR16)
        }
    }

    fn view_panel_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use crate::modules::view::{
            fallback_resolved_style, styled_segment_text, styled_segments_row,
        };
        use iced::widget::row;
        use iced::{Alignment, Length};

        match state {
            ModuleState::Button(_) => {
                if let Some(segments) = self
                    .styled_segments_with_default(crate::config::FormatMode::Expanded, bar_style)
                {
                    return styled_segments_row(segments, 13, crate::theme::TEXT)
                        .width(Length::Fill)
                        .into();
                }
                let style = fallback_resolved_style(bar_style);
                row![
                    styled_segment_text(self.icon.clone(), 16, crate::theme::TEXT, style),
                    iced::widget::Space::new().width(Length::Fill),
                    styled_segment_text(
                        self.formatted_text(crate::config::FormatMode::Expanded)
                            .unwrap_or_else(|| self.label.clone()),
                        13,
                        crate::theme::TEXT,
                        style,
                    ),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            _ => iced::widget::Space::new().into(),
        }
    }

    fn view_horizontal_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use crate::modules::view::{horizontal_icon_label_with_style, styled_segments_row};
        use iced::widget::container;
        use iced::{Alignment, Length};

        match state {
            ModuleState::Button(_) => {
                if let Some(segments) = self
                    .styled_segments_with_default(crate::config::FormatMode::Horizontal, bar_style)
                {
                    return container(styled_segments_row(segments, 12, crate::theme::TEXT))
                        .height(Length::Fill)
                        .align_y(Alignment::Center)
                        .padding([0, 6])
                        .into();
                }
                horizontal_icon_label_with_style(
                    self.icon.clone(),
                    crate::theme::TEXT,
                    self.formatted_text(crate::config::FormatMode::Horizontal)
                        .unwrap_or_else(|| self.label.clone()),
                    bar_style,
                )
            }
            _ => iced::widget::Space::new().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ColorValue, SegmentStyles, Style, ThemeToken, Weight};
    use crate::modules::Module;

    fn styled_format_config() -> crate::config::FormatConfig {
        let mut template = crate::config::FormatTemplate::parse(
            &["{label}"],
            &["icon", "value", "label"],
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
    fn button_module_id_is_button() {
        let m = ButtonModule::new(
            crate::modules::ModuleId::new("button"),
            "Launch".into(),
            ">".into(),
            crate::config::CommandConfig::Shell("kitty".into()),
            crate::config::FormatConfig::default(),
        );
        assert_eq!(m.id().0, "button");
        assert_eq!(m.module_type(), "button");
    }

    #[test]
    fn button_state_is_debug() {
        let s = ButtonState;
        // Just verify Debug formatting doesn't panic.
        let _ = format!("{:?}", s);
    }

    #[test]
    fn button_event_ready_variant() {
        // Verify ButtonEvent::Ready constructs and is Debug-formattable.
        let e = ButtonEvent::Ready;
        let _ = format!("{:?}", e);
    }

    #[test]
    fn button_styled_segments_resolve_inline_style() {
        let m = ButtonModule::new(
            crate::modules::ModuleId::new("button"),
            "Launch".into(),
            ">".into(),
            crate::config::CommandConfig::Shell("kitty".into()),
            styled_format_config(),
        );
        let segment = &m
            .styled_segments(crate::config::FormatMode::Collapsed)
            .unwrap()[0];

        assert_eq!(segment.1.color, Some(crate::theme::ACCENT));
        assert_eq!(segment.1.weight, Some(Weight::Bold));
    }

    #[test]
    fn spawn_command_fire_and_forget_preserves_shell_mode() {
        assert_eq!(
            crate::command::spawn_command_fire_and_forget(crate::command::CommandSpec::Shell(
                "true"
            ))
            .unwrap(),
            crate::command::SpawnResult::Spawned
        );
    }
}
