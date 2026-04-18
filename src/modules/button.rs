/// Runtime state for the button module. Button appearance is config-driven, not data-driven.
#[derive(Debug, Clone)]
pub struct ButtonState;

/// Events emitted by the button module.
#[derive(Debug, Clone)]
pub enum ButtonEvent {
    /// Emitted once at startup to transition Loading → Button(ButtonState).
    Ready,
}

pub struct ButtonModule {
    id: crate::modules::ModuleId,
    label: String,
    icon: String,
    on_click: String,
}

impl ButtonModule {
    pub fn new(
        id: crate::modules::ModuleId,
        label: String,
        icon: String,
        on_click: String,
    ) -> Self {
        ButtonModule {
            id,
            label,
            icon,
            on_click,
        }
    }
}

fn spawn_button_command(command: &str) -> std::io::Result<()> {
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

impl crate::modules::Module for ButtonModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn module_type(&self) -> &str {
        "button"
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::run_with(self.id.clone(), |id: &crate::modules::ModuleId| {
            let id = id.clone();
            iced::futures::stream::unfold(Some(id), |state| async move {
                match state {
                    Some(id) => {
                        let msg = crate::app::Message::Module(
                            id,
                            crate::modules::ModuleEvent::Button(ButtonEvent::Ready),
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
                let _ = spawn_button_command(&self.on_click);
                None
            }
            _ => None,
        }
    }

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, text};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Button(_) => column![
                text(self.icon.clone()).size(16), // font.size.icon per UX-DR9
            ]
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .into(),
            _ => iced::widget::Space::new().into(), // Loading/Unavailable → empty (UX-DR16)
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
            ModuleState::Button(_) => row![
                text(self.icon.clone()).size(16), // icon left (UX-DR6)
                iced::widget::Space::new().width(Length::Fill),
                text(self.label.clone()).size(13), // label right, font.size.base (UX-DR9)
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
            ModuleState::Button(_) => {
                horizontal_icon_label(self.icon.clone(), crate::theme::TEXT, self.label.clone())
            }
            _ => iced::widget::Space::new().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    #[test]
    fn button_module_id_is_button() {
        let m = ButtonModule::new(
            crate::modules::ModuleId::new("button"),
            "Launch".into(),
            ">".into(),
            "kitty".into(),
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
    fn spawn_button_command_returns_ok_for_short_command() {
        assert!(spawn_button_command("true").is_ok());
    }
}
