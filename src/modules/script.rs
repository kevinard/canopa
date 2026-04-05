/// Runtime state for the script module.
#[derive(Debug, Clone)]
pub struct ScriptState {
    pub output: String,
}

/// Events emitted by the script subscription.
#[derive(Debug, Clone)]
pub enum ScriptEvent {
    /// Command exited 0; stdout (trimmed) is the new output.
    Output(String),
    /// Command exited non-zero or failed to spawn; previous output retained.
    Failed,
}

pub struct ScriptModule {
    id: crate::modules::ModuleId,
    command: String,
    interval: u64,
}

impl ScriptModule {
    pub fn new(command: String, interval: u64) -> Self {
        ScriptModule {
            id: crate::modules::ModuleId::new("script"),
            command,
            interval,
        }
    }
}

impl crate::modules::Module for ScriptModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        use iced::futures::stream;

        // Pass command and interval as subscription identity data.
        // `run_with` takes a fn pointer (not closure) so the builder must not
        // capture any outer variables — it receives only its `&D` parameter.
        iced::Subscription::run_with((self.command.clone(), self.interval), |(cmd, interval)| {
            let cmd = cmd.clone();
            let interval = *interval;

            // `unfold` carries the state forward each iteration, avoiding
            // captures in the outer (fn-pointer) builder.
            // `first` skips the initial sleep so the bar populates within ~1s
            // of startup, satisfying the subscription contract.
            stream::unfold((cmd, interval, true), |(cmd, interval, first)| async move {
                if !first {
                    tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                }

                let event = match tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .output()
                    .await
                {
                    Ok(out) if out.status.success() => {
                        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        ScriptEvent::Output(text)
                    }
                    _ => ScriptEvent::Failed,
                };

                let msg = crate::app::Message::Module(
                    crate::modules::ModuleId::new("script"),
                    crate::modules::ModuleEvent::Script(event),
                );
                Some((msg, (cmd, interval, false)))
            })
        })
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
        let m = ScriptModule::new("echo hi".into(), 5);
        assert_eq!(m.id().0, "script");
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
}
