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
    fn presentation(&self) -> Option<&dyn crate::modules::view::IconLabel> {
        Some(self)
    }

    fn has_content(
        &self,
        state: &crate::modules::ModuleState,
        mode: crate::config::FormatMode,
    ) -> bool {
        let crate::modules::ModuleState::Script(state) = state else {
            return false;
        };
        self.styled_segments_with_default(state, mode, &crate::config::Style::default())
            .map_or_else(
                || !state.output.trim().is_empty(),
                |segments| segments.iter().any(|(text, _)| !text.trim().is_empty()),
            )
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
}

impl crate::modules::view::IconLabel for ScriptModule {
    fn content_rotation(&self) -> crate::config::ContentRotation {
        self.content_rotation
    }
    fn accepts_state(&self, state: &crate::modules::ModuleState) -> bool {
        matches!(state, crate::modules::ModuleState::Script(_))
    }
    fn icon(&self, _state: &crate::modules::ModuleState) -> String {
        ">_".into()
    }
    fn label(&self, state: &crate::modules::ModuleState) -> String {
        if let crate::modules::ModuleState::Script(s) = state {
            s.output.clone()
        } else {
            String::new()
        }
    }
    fn styled_segments_for_layout(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
        style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        if let crate::modules::ModuleState::Script(s) = state {
            self.styled_segments_with_default(s, layout.into(), style)
        } else {
            None
        }
    }
    fn formatted_color(
        &self,
        _state: &crate::modules::ModuleState,
        _layout: crate::modules::view::IconLabelLayout,
    ) -> iced::Color {
        crate::theme::palette().text
    }
    fn formatted_size(&self, layout: crate::modules::view::IconLabelLayout) -> u32 {
        if layout == crate::modules::view::IconLabelLayout::Expanded {
            13
        } else {
            crate::theme::label_size() as u32
        }
    }
    fn fallback_view(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
        style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match layout {
            crate::modules::view::IconLabelLayout::Collapsed => {
                crate::modules::view::stacked_icon_label(
                    self.icon(state),
                    self.label(state).chars().take(5).collect(),
                    self.color(state),
                    style,
                )
            }
            crate::modules::view::IconLabelLayout::Expanded => {
                crate::modules::view::spaced_icon_label(self.icon(state), self.label(state), style)
            }
            _ => crate::modules::view::default_icon_label_view(self, state, layout, style),
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
        assert_eq!(m.id().display_name(), "script");
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
        assert_eq!(m.id().display_name(), "status");
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

        assert_eq!(segment.1.color, Some(crate::theme::palette().accent));
        assert_eq!(segment.1.weight, Some(Weight::Bold));
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
