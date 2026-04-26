/// Runtime state for the clock module.
#[derive(Debug, Clone, PartialEq)]
pub struct ClockState {
    pub time: String,
    // Used in view_expanded (collapsed view shows time only)
    pub date: String,
}

/// Events emitted by the clock subscription.
#[derive(Debug, Clone, PartialEq)]
pub enum ClockEvent {
    Tick { time: String, date: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ClockUpdateCadence {
    Minute,
    Second,
}

fn format_needs_second_updates(format: &str) -> bool {
    let mut chars = format.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            continue;
        }

        let Some(mut spec) = chars.next() else {
            break;
        };
        if spec == '%' {
            continue;
        }

        while matches!(spec, '-' | '_' | '0' | '^' | '#') {
            let Some(next) = chars.next() else {
                return false;
            };
            spec = next;
        }

        while spec.is_ascii_digit() {
            let Some(next) = chars.next() else {
                return false;
            };
            spec = next;
        }

        if spec == '.' {
            let Some(next) = chars.next() else {
                return true;
            };
            spec = next;
            while spec.is_ascii_digit() {
                let Some(next) = chars.next() else {
                    return true;
                };
                spec = next;
            }
        }

        if matches!(spec, 'S' | 'T' | 'X' | 'r' | 'c' | '+' | 's' | 'f') {
            return true;
        }
    }

    false
}

fn clock_update_cadence(time_format: &str, date_format: &str) -> ClockUpdateCadence {
    if format_needs_second_updates(time_format) || format_needs_second_updates(date_format) {
        ClockUpdateCadence::Second
    } else {
        ClockUpdateCadence::Minute
    }
}

fn duration_until_next_minute(now: chrono::DateTime<chrono::Local>) -> std::time::Duration {
    use chrono::Timelike;

    let elapsed_ns = (now.second() as u64 * 1_000_000_000) + now.nanosecond() as u64;
    let minute_ns = 60 * 1_000_000_000;
    let remaining_ns = if elapsed_ns == 0 {
        minute_ns
    } else {
        minute_ns - elapsed_ns
    };
    std::time::Duration::from_nanos(remaining_ns)
}

fn build_clock_message(
    id: crate::modules::ModuleId,
    time_fmt: &str,
    date_fmt: &str,
) -> crate::app::Message {
    let now = chrono::Local::now();
    let time = now.format(time_fmt).to_string();
    let date = now.format(date_fmt).to_string();
    crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Module(
        id,
        crate::modules::ModuleEvent::Clock(ClockEvent::Tick { time, date }),
    ))
}

/// Clock module: shows current time and date, updated every second via subscription.
pub(super) struct ClockModule {
    id: crate::modules::ModuleId,
    time_format: String,
    date_format: String,
    format: crate::config::FormatConfig,
}

impl ClockModule {
    pub(super) fn new(
        id: crate::modules::ModuleId,
        time_format: String,
        date_format: String,
        format: crate::config::FormatConfig,
    ) -> Self {
        ClockModule {
            id,
            time_format,
            date_format,
            format,
        }
    }

    fn formatted_text(
        &self,
        state: &ClockState,
        mode: crate::config::FormatMode,
    ) -> Option<String> {
        self.format.template_for(mode).map(|template| {
            template.render_joined(&[
                ("icon", "⌚".to_string()),
                ("value", state.time.clone()),
                ("time", state.time.clone()),
                ("date", state.date.clone()),
            ])
        })
    }
}

impl crate::modules::Module for ClockModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> crate::modules::ModuleKind {
        crate::modules::ModuleKind::Clock
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        // Tick only as often as the configured formats can visibly change.
        // The id is included in the key tuple so multiple clock instances subscribe independently.
        let cadence = clock_update_cadence(&self.time_format, &self.date_format);
        iced::Subscription::run_with(
            (
                self.id.clone(),
                self.time_format.clone(),
                self.date_format.clone(),
                cadence,
            ),
            |(id, time_fmt, date_fmt, cadence)| {
                use iced::futures::stream;

                stream::unfold(
                    (
                        id.clone(),
                        time_fmt.clone(),
                        date_fmt.clone(),
                        *cadence,
                        true,
                    ),
                    |(id, time_fmt, date_fmt, cadence, first)| async move {
                        if !first {
                            let sleep_for = match cadence {
                                ClockUpdateCadence::Second => std::time::Duration::from_secs(1),
                                ClockUpdateCadence::Minute => {
                                    duration_until_next_minute(chrono::Local::now())
                                }
                            };
                            tokio::time::sleep(sleep_for).await;
                        }

                        let msg = build_clock_message(id.clone(), &time_fmt, &date_fmt);
                        Some((msg, (id, time_fmt, date_fmt, cadence, false)))
                    },
                )
            },
        )
    }

    // Clock keeps custom views: collapsed renders icon + time in a column and
    // expanded renders time plus a second date row.
    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, text};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Clock(s) => {
                if let Some(formatted) =
                    self.formatted_text(s, crate::config::FormatMode::Collapsed)
                {
                    return column![text(formatted).size(11)]
                        .align_x(Alignment::Center)
                        .width(Length::Fill)
                        .into();
                }
                column![text("⌚").size(16), text(s.time.clone()).size(11),]
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
        use iced::widget::{column, row, text};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Clock(s) => {
                if let Some(formatted) = self.formatted_text(s, crate::config::FormatMode::Expanded)
                {
                    return text(formatted).size(12).into();
                }
                column![
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
            ModuleState::Clock(s) => {
                if let Some(formatted) =
                    self.formatted_text(s, crate::config::FormatMode::Horizontal)
                {
                    horizontal_icon_label("", crate::theme::TEXT, formatted)
                } else {
                    horizontal_icon_label("⌚", crate::theme::TEXT, &s.time)
                }
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
    fn clock_module_id_is_clock() {
        let m = ClockModule::new(
            crate::modules::ModuleId::new("clock"),
            "%H:%M".into(),
            "%a %d %b".into(),
            crate::config::FormatConfig::default(),
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
            crate::config::FormatConfig::default(),
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

    #[test]
    fn clock_update_cadence_defaults_to_minute() {
        assert_eq!(
            clock_update_cadence("%H:%M", "%a %d %b"),
            ClockUpdateCadence::Minute
        );
    }

    #[test]
    fn clock_update_cadence_uses_seconds_when_format_shows_seconds() {
        assert_eq!(
            clock_update_cadence("%H:%M:%S", "%a %d %b"),
            ClockUpdateCadence::Second
        );
        assert_eq!(
            clock_update_cadence("%H:%M", "%c"),
            ClockUpdateCadence::Second
        );
    }

    #[test]
    fn clock_update_cadence_ignores_escaped_percent() {
        assert_eq!(
            clock_update_cadence("%%S %H:%M", "%Y-%m-%d"),
            ClockUpdateCadence::Minute
        );
    }

    #[test]
    fn minute_cadence_sleeps_until_next_minute_boundary() {
        use chrono::{Local, TimeZone, Timelike};

        let now = Local
            .with_ymd_and_hms(2026, 4, 18, 12, 34, 56)
            .unwrap()
            .with_nanosecond(500_000_000)
            .unwrap();

        assert_eq!(
            duration_until_next_minute(now),
            std::time::Duration::from_millis(3_500)
        );
    }
}
