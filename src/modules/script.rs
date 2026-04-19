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

pub const MAX_SCRIPT_OUTPUT_BYTES: usize = crate::command::MAX_COMMAND_OUTPUT_BYTES;
pub const SCRIPT_COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

async fn run_script_command(cmd: &str, timeout: std::time::Duration) -> ScriptEvent {
    match crate::command::run_shell_capture_stdout(cmd, timeout, MAX_SCRIPT_OUTPUT_BYTES).await {
        Ok(Some(out)) => ScriptEvent::Output(out),
        _ => ScriptEvent::Failed,
    }
}

pub struct ScriptModule {
    id: crate::modules::ModuleId,
    command: String,
    interval: u64,
}

impl ScriptModule {
    pub fn new(id: crate::modules::ModuleId, command: String, interval: u64) -> Self {
        ScriptModule {
            id,
            command,
            interval,
        }
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

                        let msg = crate::app::Message::Module(
                            id.clone(),
                            crate::modules::ModuleEvent::Script(event),
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
                let truncated: String = s.output.chars().take(5).collect();
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
            ModuleState::Script(s) => row![
                text(">_").size(16),
                iced::widget::Space::new().width(Length::Fill),
                text(s.output.clone()).size(13),
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into(),
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
            ModuleState::Script(s) => horizontal_icon_label(">_", crate::theme::TEXT, &s.output),
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
        let m = ScriptModule::new(crate::modules::ModuleId::new("script"), "echo hi".into(), 5);
        assert_eq!(m.id().0, "script");
        assert_eq!(m.module_type(), "script");
    }

    #[test]
    fn script_module_custom_id() {
        let m = ScriptModule::new(crate::modules::ModuleId::new("status"), "date".into(), 5);
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
}
