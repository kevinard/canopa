/// Runtime state for the script module.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptState {
    pub output: String,
}

/// Events emitted by the script subscription.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptEvent {
    /// Command exited 0; stdout (trimmed) is the new output.
    Output(String),
    /// Command exited non-zero or failed to spawn.
    Failed,
}

const MAX_SCRIPT_OUTPUT_BYTES: usize = crate::command::MAX_COMMAND_OUTPUT_BYTES;

async fn run_script_command(command: &crate::config::ConfiguredCommand) -> ScriptEvent {
    match crate::command::run_configured_command_capture_stdout(command, MAX_SCRIPT_OUTPUT_BYTES)
        .await
    {
        Ok(crate::command::CaptureResult::Success(out)) => ScriptEvent::Output(out),
        _ => ScriptEvent::Failed,
    }
}

pub(super) struct ScriptModule {
    id: crate::modules::ModuleId,
    command: crate::config::ConfiguredCommand,
    interval: u64,
    format: crate::config::FormatConfig,
    content_rotation: crate::config::ContentRotation,
}

impl ScriptModule {
    #[cfg(test)]
    pub(super) fn new(
        id: crate::modules::ModuleId,
        command: crate::config::ConfiguredCommand,
        interval: u64,
        format: crate::config::FormatConfig,
    ) -> Self {
        Self::new_with_content_rotation(
            id,
            command,
            interval,
            format,
            crate::config::ContentRotation::None,
        )
    }

    pub(super) fn new_with_content_rotation(
        id: crate::modules::ModuleId,
        command: crate::config::ConfiguredCommand,
        interval: u64,
        format: crate::config::FormatConfig,
        content_rotation: crate::config::ContentRotation,
    ) -> Self {
        ScriptModule {
            id,
            command,
            interval,
            format,
            content_rotation,
        }
    }

    fn formatted_text(
        &self,
        state: &ScriptState,
        mode: crate::config::FormatMode,
    ) -> Option<String> {
        self.format.template_for(mode).map(|template| {
            template.render_joined(&[
                ("icon", ">_".to_string()),
                ("value", state.output.clone()),
                ("output", state.output.clone()),
            ])
        })
    }

    fn styled_segments_with_default(
        &self,
        state: &ScriptState,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        use std::borrow::Cow;

        crate::modules::view::render_format_segments(
            &self.format,
            mode,
            &[
                ("icon", Cow::Borrowed(">_")),
                ("value", Cow::Borrowed(state.output.as_str())),
                ("output", Cow::Borrowed(state.output.as_str())),
            ],
            None,
            default_style,
        )
    }
}

impl crate::modules::Module for ScriptModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> crate::modules::ModuleKind {
        crate::modules::ModuleKind::Script
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        use iced::futures::stream;

        // The id is included in the key tuple so multiple script instances subscribe independently.
        iced::Subscription::run_with(
            (self.id.clone(), self.command.clone(), self.interval),
            |(id, cmd, interval)| {
                let id = id.clone();
                let cmd = cmd.clone();
                let interval = *interval;

                stream::unfold(
                    (id, cmd, interval, true),
                    |(id, cmd, interval, first)| async move {
                        if !first {
                            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                        }

                        let event = run_script_command(&cmd).await;

                        let msg = crate::app::Message::Modules(
                            crate::app::messages::ModuleRuntimeEvent::Module(
                                id.clone(),
                                crate::modules::ModuleEvent::Script(event),
                            ),
                        );
                        Some((msg, (id, cmd, interval, false)))
                    },
                )
            },
        )
    }
}

impl crate::modules::render::ModuleRender for ScriptModule {
    fn has_content(
        &self,
        state: &crate::modules::ModuleState,
        mode: crate::config::FormatMode,
    ) -> bool {
        let crate::modules::ModuleState::Script(state) = state else {
            return false;
        };
        !self
            .formatted_text(state, mode)
            .as_deref()
            .unwrap_or(&state.output)
            .trim()
            .is_empty()
    }

    fn tooltip_values(&self, state: &crate::modules::ModuleState) -> Vec<(&'static str, String)> {
        if let crate::modules::ModuleState::Script(s) = state {
            vec![
                ("icon", ">_".into()),
                ("value", s.output.clone()),
                ("output", s.output.clone()),
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
        use crate::modules::view::{
            fallback_resolved_style, styled_segment_text, styled_segments_row,
        };
        use iced::widget::column;
        use iced::{Alignment, Length};

        match state {
            ModuleState::Script(s) => {
                if let Some(segments) = self.styled_segments_with_default(
                    s,
                    crate::config::FormatMode::Collapsed,
                    bar_style,
                ) {
                    return styled_segments_row(segments, 11, crate::theme::TEXT)
                        .width(Length::Fill)
                        .into();
                }
                let output = self
                    .formatted_text(s, crate::config::FormatMode::Collapsed)
                    .unwrap_or_else(|| s.output.clone());
                let truncated: String = output.chars().take(5).collect();
                let style = fallback_resolved_style(bar_style);
                column![
                    styled_segment_text(">_".to_string(), 16, crate::theme::TEXT, style),
                    styled_segment_text(truncated, 11, crate::theme::TEXT, style),
                ]
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            _ => iced::widget::Space::new().into(),
        }
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
        let crate::modules::ModuleState::Script(script) = state else {
            return None;
        };
        let segments = self.styled_segments_with_default(
            script,
            crate::config::FormatMode::Collapsed,
            bar_style,
        )?;
        Some(
            iced::widget::container(crate::modules::view::formatted_segments_content(
                segments,
                11,
                crate::theme::TEXT,
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
        use crate::modules::view::{
            fallback_resolved_style, styled_segment_text, styled_segments_row,
        };
        use iced::widget::row;
        use iced::{Alignment, Length};

        match state {
            ModuleState::Script(s) => {
                if let Some(segments) = self.styled_segments_with_default(
                    s,
                    crate::config::FormatMode::Expanded,
                    bar_style,
                ) {
                    return styled_segments_row(segments, 13, crate::theme::TEXT)
                        .width(Length::Fill)
                        .into();
                }
                let output = self
                    .formatted_text(s, crate::config::FormatMode::Expanded)
                    .unwrap_or_else(|| s.output.clone());
                let style = fallback_resolved_style(bar_style);
                row![
                    styled_segment_text(">_".to_string(), 16, crate::theme::TEXT, style),
                    iced::widget::Space::new().width(Length::Fill),
                    styled_segment_text(output, 13, crate::theme::TEXT, style),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            _ => iced::widget::Space::new().into(),
        }
    }

    fn render_horizontal_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        use crate::modules::ModuleState;
        use crate::modules::view::{horizontal_icon_label_with_style, styled_segments_row};
        use iced::widget::container;
        use iced::{Alignment, Length};

        match state {
            ModuleState::Script(s) => {
                if let Some(segments) = self.styled_segments_with_default(
                    s,
                    crate::config::FormatMode::Horizontal,
                    bar_style,
                ) {
                    return Some(
                        container(styled_segments_row(segments, 12, crate::theme::TEXT))
                            .height(Length::Fill)
                            .align_y(Alignment::Center)
                            .padding([0, 6])
                            .into(),
                    );
                }
                Some(horizontal_icon_label_with_style(
                    ">_",
                    crate::theme::TEXT,
                    self.formatted_text(s, crate::config::FormatMode::Horizontal)
                        .unwrap_or_else(|| s.output.clone()),
                    bar_style,
                ))
            }
            _ => Some(iced::widget::Space::new().into()),
        }
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
        let crate::modules::ModuleState::Script(script) = state else {
            return None;
        };
        let segments = self.styled_segments_with_default(
            script,
            crate::config::FormatMode::Horizontal,
            bar_style,
        )?;
        Some(
            iced::widget::container(crate::modules::view::formatted_segments_content(
                segments,
                12,
                crate::theme::TEXT,
                rotation,
            ))
            .height(iced::Length::Fill)
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
            &["{output}"],
            &["icon", "value", "output"],
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
    fn script_module_id_is_script() {
        let m = ScriptModule::new(
            crate::modules::ModuleId::new("script"),
            crate::config::ConfiguredCommand::shell_for_test("echo hi"),
            5,
            crate::config::FormatConfig::default(),
        );
        assert_eq!(m.id().0, "script");
        assert_eq!(m.module_type(), "script");
    }

    #[test]
    fn script_module_custom_id() {
        let m = ScriptModule::new(
            crate::modules::ModuleId::new("status"),
            crate::config::ConfiguredCommand::shell_for_test("date"),
            5,
            crate::config::FormatConfig::default(),
        );
        assert_eq!(m.id().0, "status");
        assert_eq!(m.module_type(), "script");
    }

    #[test]
    fn script_state_stores_output() {
        let s = ScriptState {
            output: "hello".into(),
        };
        assert_eq!(s.output, "hello");
    }

    #[test]
    fn script_event_failed_variant() {
        // Ensure the Failed variant constructs without issue.
        let _ev = ScriptEvent::Failed;
    }

    #[test]
    fn script_styled_segments_resolve_inline_style() {
        let m = ScriptModule::new(
            crate::modules::ModuleId::new("script"),
            crate::config::ConfiguredCommand::shell_for_test("echo hi"),
            5,
            styled_format_config(),
        );
        let state = ScriptState {
            output: "ok".into(),
        };
        let segment = &m
            .styled_segments_with_default(
                &state,
                crate::config::FormatMode::Collapsed,
                &crate::config::Style::default(),
            )
            .unwrap()[0];

        assert_eq!(segment.1.color, Some(crate::theme::ACCENT));
        assert_eq!(segment.1.weight, Some(Weight::Bold));
    }

    #[test]
    fn script_auto_rotation_overrides_side_bar_collapsed_format() {
        let m = ScriptModule::new_with_content_rotation(
            crate::modules::ModuleId::new("script"),
            crate::config::ConfiguredCommand::shell_for_test("echo hi"),
            5,
            styled_format_config(),
            crate::config::ContentRotation::Auto,
        );
        let state = crate::modules::ModuleState::Script(ScriptState {
            output: "ok".into(),
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
    fn script_explicit_rotation_overrides_horizontal_format() {
        let horizontal = crate::config::FormatTemplate::parse(
            &["{output}"],
            &["icon", "value", "output"],
            "test.format.horizontal",
        )
        .unwrap();
        let m = ScriptModule::new_with_content_rotation(
            crate::modules::ModuleId::new("script"),
            crate::config::ConfiguredCommand::shell_for_test("echo hi"),
            5,
            crate::config::FormatConfig {
                horizontal: Some(horizontal),
                ..Default::default()
            },
            crate::config::ContentRotation::Clockwise,
        );
        let state = crate::modules::ModuleState::Script(ScriptState {
            output: "ok".into(),
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

    #[test]
    fn sanitize_script_stdout_trims_and_caps_output() {
        let output = format!("{} trailing\n", "x".repeat(MAX_SCRIPT_OUTPUT_BYTES + 32));

        let sanitized = crate::command::sanitize_stdout(output.as_bytes(), MAX_SCRIPT_OUTPUT_BYTES);

        assert_eq!(sanitized.len(), MAX_SCRIPT_OUTPUT_BYTES);
        assert!(sanitized.chars().all(|c| c == 'x'));
    }

    #[tokio::test]
    async fn run_script_command_times_out() {
        let command = crate::config::ConfiguredCommand::shell_with_timeout_for_test("sleep 2", 25);
        let event = run_script_command(&command).await;

        assert!(matches!(event, ScriptEvent::Failed));
    }

    #[tokio::test]
    async fn run_script_command_preserves_shell_mode() {
        let command = crate::config::ConfiguredCommand::shell_with_timeout_for_test(
            "printf '%s' script-ok",
            1000,
        );
        let event = run_script_command(&command).await;

        assert_eq!(event, ScriptEvent::Output("script-ok".to_string()));
    }

    #[tokio::test]
    async fn run_script_command_maps_nonzero_exit_to_failed() {
        let command = crate::config::ConfiguredCommand::shell_with_timeout_for_test("false", 1000);
        let event = run_script_command(&command).await;

        assert_eq!(event, ScriptEvent::Failed);
    }
}
