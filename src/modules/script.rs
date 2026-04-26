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
    /// Command exited non-zero or failed to spawn; previous output retained.
    Failed,
}

const MAX_SCRIPT_OUTPUT_BYTES: usize = crate::command::MAX_COMMAND_OUTPUT_BYTES;
const SCRIPT_COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

async fn run_script_command(cmd: &str, timeout: std::time::Duration) -> ScriptEvent {
    match crate::command::run_command_capture_stdout(
        crate::command::CommandSpec::Shell(cmd),
        timeout,
        MAX_SCRIPT_OUTPUT_BYTES,
    )
    .await
    {
        Ok(crate::command::CaptureResult::Success(out)) => ScriptEvent::Output(out),
        _ => ScriptEvent::Failed,
    }
}

pub(super) struct ScriptModule {
    id: crate::modules::ModuleId,
    command: String,
    interval: u64,
    format: crate::config::FormatConfig,
}

impl ScriptModule {
    pub(super) fn new(
        id: crate::modules::ModuleId,
        command: String,
        interval: u64,
        format: crate::config::FormatConfig,
    ) -> Self {
        ScriptModule {
            id,
            command,
            interval,
            format,
        }
    }

    fn formatted_text(
        &self,
        state: &ScriptState,
        mode: crate::config::FormatMode,
    ) -> Option<String> {
        self.format.template_for(mode).map(|template| {
            template.render_pairs(&[
                ("icon", ">_".to_string()),
                ("value", state.output.clone()),
                ("output", state.output.clone()),
            ])
        })
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

                        let event = run_script_command(&cmd, SCRIPT_COMMAND_TIMEOUT).await;

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

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, text};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Script(s) => {
                let output = self
                    .formatted_text(s, crate::config::FormatMode::Collapsed)
                    .unwrap_or_else(|| s.output.clone());
                let truncated: String = output.chars().take(5).collect();
                column![text(">_").size(16), text(truncated).size(11),]
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .into()
            }
            _ => iced::widget::Space::new().into(),
        }
    }

    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{row, text};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Script(s) => {
                let output = self
                    .formatted_text(s, crate::config::FormatMode::Expanded)
                    .unwrap_or_else(|| s.output.clone());
                row![
                    text(">_").size(16),
                    iced::widget::Space::new().width(Length::Fill),
                    text(output).size(13),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            _ => iced::widget::Space::new().into(),
        }
    }

    fn view_horizontal(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use crate::modules::view::horizontal_icon_label;

        match state {
            ModuleState::Script(s) => horizontal_icon_label(
                ">_",
                crate::theme::TEXT,
                self.formatted_text(s, crate::config::FormatMode::Horizontal)
                    .unwrap_or_else(|| s.output.clone()),
            ),
            _ => iced::widget::Space::new().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    #[test]
    fn script_module_id_is_script() {
        let m = ScriptModule::new(
            crate::modules::ModuleId::new("script"),
            "echo hi".into(),
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
            "date".into(),
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
    fn sanitize_script_stdout_trims_and_caps_output() {
        let output = format!("{} trailing\n", "x".repeat(MAX_SCRIPT_OUTPUT_BYTES + 32));

        let sanitized = crate::command::sanitize_stdout(output.as_bytes(), MAX_SCRIPT_OUTPUT_BYTES);

        assert_eq!(sanitized.len(), MAX_SCRIPT_OUTPUT_BYTES);
        assert!(sanitized.chars().all(|c| c == 'x'));
    }

    #[tokio::test]
    async fn run_script_command_times_out() {
        let event = run_script_command("sleep 2", std::time::Duration::from_millis(25)).await;

        assert!(matches!(event, ScriptEvent::Failed));
    }

    #[tokio::test]
    async fn run_script_command_preserves_shell_mode() {
        let event =
            run_script_command("printf '%s' script-ok", std::time::Duration::from_secs(1)).await;

        assert_eq!(event, ScriptEvent::Output("script-ok".to_string()));
    }

    #[tokio::test]
    async fn run_script_command_maps_nonzero_exit_to_failed() {
        let event = run_script_command("false", std::time::Duration::from_secs(1)).await;

        assert_eq!(event, ScriptEvent::Failed);
    }
}
