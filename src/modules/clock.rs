/// Runtime state for the clock module.
#[derive(Debug, Clone)]
pub struct ClockState {
    pub time: String,
    // Used in view_expanded (collapsed view shows time only)
    #[allow(dead_code)]
    pub date: String,
}

/// Events emitted by the clock subscription.
#[derive(Debug, Clone)]
pub enum ClockEvent {
    Tick { time: String, date: String },
}

/// Clock module: shows current time and date, updated every second via subscription.
pub struct ClockModule {
    id: crate::modules::ModuleId,
    time_format: String,
    date_format: String,
}

impl ClockModule {
    pub fn new(id: crate::modules::ModuleId, time_format: String, date_format: String) -> Self {
        ClockModule {
            id,
            time_format,
            date_format,
        }
    }
}

impl crate::modules::Module for ClockModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn module_type(&self) -> &str {
        "clock"
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        // Tick every second; first tick fires at t=1s (within the 1-second contract).
        // The id is included in the key tuple so multiple clock instances subscribe independently.
        iced::time::every(std::time::Duration::from_secs(1))
            .with((
                self.id.clone(),
                self.time_format.clone(),
                self.date_format.clone(),
            ))
            .map(|((id, time_fmt, date_fmt), _)| {
                let now = chrono::Local::now();
                let time = now.format(&time_fmt).to_string();
                let date = now.format(&date_fmt).to_string();
                crate::app::Message::Module(
                    id,
                    crate::modules::ModuleEvent::Clock(ClockEvent::Tick { time, date }),
                )
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
            ModuleState::Clock(s) => column![text("⌚").size(16), text(s.time.clone()).size(11),]
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .into(),
            _ => iced::widget::Space::new().into(),
        }
    }

    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, row, text};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Clock(s) => column![
                row![
                    text("⌚").size(16),
                    iced::widget::Space::new().width(Length::Fill),
                    text(s.time.clone()).size(13),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill),
                text(s.date.clone()).size(11),
            ]
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
            ModuleState::Clock(s) => horizontal_icon_label("⌚", crate::theme::TEXT, &s.time),
            _ => iced::widget::Space::new().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    #[test]
    fn clock_module_id_is_clock() {
        let m = ClockModule::new(
            crate::modules::ModuleId::new("clock"),
            "%H:%M".into(),
            "%a %d %b".into(),
        );
        assert_eq!(m.id().0, "clock");
        assert_eq!(m.module_type(), "clock");
    }

    #[test]
    fn clock_module_custom_id() {
        let m = ClockModule::new(
            crate::modules::ModuleId::new("my-clock"),
            "%H:%M".into(),
            "%a %d %b".into(),
        );
        assert_eq!(m.id().0, "my-clock");
        assert_eq!(m.module_type(), "clock");
    }

    #[test]
    fn clock_state_stores_formatted_strings() {
        let state = ClockState {
            time: "12:34".into(),
            date: "Mon 01 Jan".into(),
        };
        assert_eq!(state.time, "12:34");
        assert_eq!(state.date, "Mon 01 Jan");
    }
}
