/// Runtime state for the clock module.
#[derive(Debug, Clone, PartialEq)]
pub struct ClockState {
    pub time: String,
    // Used when rendering detailed clock text.
    pub date: String,
}

/// Events emitted by the clock subscription.
#[derive(Debug, Clone, PartialEq)]
pub enum ClockEvent {
    Tick { time: String, date: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClockDetailSession {
    month_offset: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClockDetailMessage {
    PreviousMonth,
    NextMonth,
    Today,
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

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub(super) struct CalendarMonth {
    pub(super) year: i32,
    pub(super) month: u32,
    pub(super) weeks: Vec<[Option<u32>; 7]>,
    pub(super) today: Option<u32>,
}

pub(super) fn calendar_month_for(today: chrono::NaiveDate, month_offset: i32) -> CalendarMonth {
    use chrono::{Datelike, NaiveDate};

    let month_index = today.year() * 12 + today.month0() as i32 + month_offset;
    let year = month_index.div_euclid(12);
    let month0 = month_index.rem_euclid(12) as u32;
    let month = month0 + 1;
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("computed month is valid");
    let next_month_index = month_index + 1;
    let next_year = next_month_index.div_euclid(12);
    let next_month = next_month_index.rem_euclid(12) as u32 + 1;
    let next_first =
        NaiveDate::from_ymd_opt(next_year, next_month, 1).expect("computed next month is valid");
    let days_in_month = (next_first - chrono::Duration::days(1)).day();
    let leading_blanks = first.weekday().num_days_from_monday() as usize;

    let mut weeks = Vec::new();
    let mut day = 1;
    while day <= days_in_month {
        let mut week = [None; 7];
        for (weekday, cell) in week.iter_mut().enumerate() {
            if weeks.is_empty() && weekday < leading_blanks {
                continue;
            }
            if day <= days_in_month {
                *cell = Some(day);
                day += 1;
            }
        }
        weeks.push(week);
    }

    CalendarMonth {
        year,
        month,
        weeks,
        today: if year == today.year() && month == today.month() {
            Some(today.day())
        } else {
            None
        },
    }
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

    fn styled_segments_with_default(
        &self,
        state: &ClockState,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        use std::borrow::Cow;

        crate::modules::view::render_format_segments(
            &self.format,
            mode,
            &[
                ("icon", Cow::Borrowed("⌚")),
                ("value", Cow::Borrowed(state.time.as_str())),
                ("time", Cow::Borrowed(state.time.as_str())),
                ("date", Cow::Borrowed(state.date.as_str())),
            ],
            None,
            default_style,
        )
    }

    fn view_clock_detail_with_month_offset(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
        month_offset: i32,
    ) -> iced::Element<'_, crate::app::Message> {
        use iced::widget::{button, column, container, row, text};
        use iced::{Alignment, Length};

        let crate::modules::ModuleState::Clock(_) = state else {
            return iced::widget::Space::new().into();
        };

        let summary = <Self as crate::modules::render::ModuleRender>::render_panel_with_style(
            self, state, bar_style,
        );
        let today = chrono::Local::now().date_naive();
        let month = calendar_month_for(today, month_offset);
        let first = chrono::NaiveDate::from_ymd_opt(month.year, month.month, 1)
            .expect("calendar month is valid");
        let title = first.format("%B %Y").to_string();

        let controls = row![
            button(text("<")).on_press(crate::app::Message::Modules(
                crate::app::messages::ModuleRuntimeEvent::Detail {
                    target: self.id.clone(),
                    message: crate::modules::ModuleDetailMessage::new(
                        ClockDetailMessage::PreviousMonth,
                    ),
                }
            )),
            text(title).width(Length::Fill),
            button(text("Today")).on_press(crate::app::Message::Modules(
                crate::app::messages::ModuleRuntimeEvent::Detail {
                    target: self.id.clone(),
                    message: crate::modules::ModuleDetailMessage::new(ClockDetailMessage::Today),
                }
            )),
            button(text(">")).on_press(crate::app::Message::Modules(
                crate::app::messages::ModuleRuntimeEvent::Detail {
                    target: self.id.clone(),
                    message: crate::modules::ModuleDetailMessage::new(
                        ClockDetailMessage::NextMonth
                    ),
                }
            )),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill);

        let mut calendar = column![controls].spacing(6).width(Length::Fill);
        let mut weekdays = row![].spacing(4).width(Length::Fill);
        for label in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
            weekdays = weekdays.push(
                container(text(label).size(10))
                    .width(Length::FillPortion(1))
                    .center_x(Length::Fill),
            );
        }
        calendar = calendar.push(weekdays);

        for week in month.weeks {
            let mut row = row![].spacing(4).width(Length::Fill);
            for day in week {
                let is_today = day == month.today;
                let label = day.map(|day| day.to_string()).unwrap_or_default();
                let cell = container(text(label).size(11))
                    .width(Length::FillPortion(1))
                    .center_x(Length::Fill);
                let cell = cell.padding([3, 0]).style(move |_| {
                    if is_today {
                        iced::widget::container::Style {
                            background: Some(iced::Background::Color(crate::theme::SURFACE0)),
                            border: iced::Border {
                                color: crate::theme::ACCENT,
                                width: 1.0,
                                radius: iced::border::Radius::from(4.0),
                            },
                            ..Default::default()
                        }
                    } else {
                        iced::widget::container::Style {
                            border: iced::Border {
                                color: iced::Color::TRANSPARENT,
                                width: 1.0,
                                radius: iced::border::Radius::from(4.0),
                            },
                            ..Default::default()
                        }
                    }
                });
                row = row.push(cell);
            }
            calendar = calendar.push(row);
        }

        column![summary, calendar]
            .spacing(12)
            .width(Length::Fill)
            .into()
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

    fn module_detail(&self) -> Option<&dyn crate::modules::detail::ModuleDetailLifecycle> {
        Some(self)
    }

    fn module_detail_mut(
        &mut self,
    ) -> Option<&mut (dyn crate::modules::detail::ModuleDetailLifecycle + '_)> {
        Some(self)
    }
}

impl crate::modules::detail::ModuleDetailLifecycle for ClockModule {
    fn create_module_detail_session(
        &self,
        state: &crate::modules::ModuleState,
    ) -> Option<crate::modules::ModuleDetailSession> {
        matches!(state, crate::modules::ModuleState::Clock(_))
            .then(|| Box::new(ClockDetailSession { month_offset: 0 }) as _)
    }

    fn update_module_detail_session(
        &mut self,
        session: &mut crate::modules::ModuleDetailSession,
        message: crate::modules::ModuleDetailMessage,
    ) -> crate::modules::ModuleDetailUpdate {
        let Some(session) = session.downcast_mut::<ClockDetailSession>() else {
            return crate::modules::ModuleDetailUpdate::default();
        };
        let Some(message) = message.downcast_ref::<ClockDetailMessage>() else {
            return crate::modules::ModuleDetailUpdate::default();
        };

        match message {
            ClockDetailMessage::PreviousMonth => {
                session.month_offset = session.month_offset.saturating_sub(1);
            }
            ClockDetailMessage::NextMonth => {
                session.month_offset = session.month_offset.saturating_add(1);
            }
            ClockDetailMessage::Today => {
                session.month_offset = 0;
            }
        }

        crate::modules::ModuleDetailUpdate::default()
    }
}

impl crate::modules::render::ModuleRender for ClockModule {
    fn tooltip_values(&self, state: &crate::modules::ModuleState) -> Vec<(&'static str, String)> {
        if let crate::modules::ModuleState::Clock(c) = state {
            vec![
                ("icon", "⌚".into()),
                ("value", c.time.clone()),
                ("time", c.time.clone()),
                ("date", c.date.clone()),
            ]
        } else {
            Vec::new()
        }
    }

    // Clock keeps custom views: collapsed renders icon + time in a column and
    // panel renders time plus a second date row.
    fn render_collapsed_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::column;
        use iced::{Alignment, Length};

        match state {
            ModuleState::Clock(s) => {
                if let Some(segments) = self.styled_segments_with_default(
                    s,
                    crate::config::FormatMode::Collapsed,
                    bar_style,
                ) {
                    return crate::modules::view::styled_segments_row(
                        segments,
                        11,
                        crate::theme::TEXT,
                    )
                    .width(Length::Fill)
                    .into();
                }
                if let Some(formatted) =
                    self.formatted_text(s, crate::config::FormatMode::Collapsed)
                {
                    return column![crate::modules::view::styled_segment_text(
                        formatted,
                        11,
                        crate::theme::TEXT,
                        crate::modules::view::fallback_resolved_style(bar_style),
                    )]
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .into();
                }
                let style = crate::modules::view::fallback_resolved_style(bar_style);
                column![
                    crate::modules::view::styled_segment_text(
                        "⌚".to_string(),
                        16,
                        crate::theme::TEXT,
                        style,
                    ),
                    crate::modules::view::styled_segment_text(
                        s.time.clone(),
                        11,
                        crate::theme::TEXT,
                        style,
                    ),
                ]
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            _ => iced::widget::Space::new().into(),
        }
    }

    fn render_panel_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{column, row};
        use iced::{Alignment, Length};

        match state {
            ModuleState::Clock(s) => {
                if let Some(segments) = self.styled_segments_with_default(
                    s,
                    crate::config::FormatMode::Expanded,
                    bar_style,
                ) {
                    return crate::modules::view::styled_segments_row(
                        segments,
                        12,
                        crate::theme::TEXT,
                    )
                    .width(Length::Fill)
                    .into();
                }
                if let Some(formatted) = self.formatted_text(s, crate::config::FormatMode::Expanded)
                {
                    return crate::modules::view::styled_segment_text(
                        formatted,
                        12,
                        crate::theme::TEXT,
                        crate::modules::view::fallback_resolved_style(bar_style),
                    );
                }
                let style = crate::modules::view::fallback_resolved_style(bar_style);
                column![
                    row![
                        crate::modules::view::styled_segment_text(
                            "⌚".to_string(),
                            16,
                            crate::theme::TEXT,
                            style,
                        ),
                        iced::widget::Space::new().width(Length::Fill),
                        crate::modules::view::styled_segment_text(
                            s.time.clone(),
                            13,
                            crate::theme::TEXT,
                            style,
                        ),
                    ]
                    .align_y(Alignment::Center)
                    .width(Length::Fill),
                    crate::modules::view::styled_segment_text(
                        s.date.clone(),
                        11,
                        crate::theme::TEXT,
                        style,
                    ),
                ]
                .width(Length::Fill)
                .into()
            }
            _ => iced::widget::Space::new().into(),
        }
    }

    fn render_detail_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        Some(self.view_clock_detail_with_month_offset(state, bar_style, 0))
    }

    fn render_detail_session_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
        session: &crate::modules::ModuleDetailSession,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        let month_offset = session
            .downcast_ref::<ClockDetailSession>()
            .map(|session| session.month_offset)
            .unwrap_or_default();

        Some(self.view_clock_detail_with_month_offset(state, bar_style, month_offset))
    }

    fn detail_preferred_size(
        &self,
        _state: &crate::modules::ModuleState,
        _bar_style: &crate::config::Style,
        _session: Option<&crate::modules::ModuleDetailSession>,
    ) -> Option<crate::surface::panel::PanelPreferredSize> {
        Some(crate::surface::panel::PanelPreferredSize {
            width: 300,
            height: 260,
        })
    }

    fn render_horizontal_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        use crate::modules::ModuleState;
        use crate::modules::view::horizontal_icon_label_with_style;

        match state {
            ModuleState::Clock(s) => {
                if let Some(segments) = self.styled_segments_with_default(
                    s,
                    crate::config::FormatMode::Horizontal,
                    bar_style,
                ) {
                    return Some(
                        iced::widget::container(crate::modules::view::styled_segments_row(
                            segments,
                            12,
                            crate::theme::TEXT,
                        ))
                        .height(iced::Length::Fill)
                        .align_y(iced::Alignment::Center)
                        .padding([0, 6])
                        .into(),
                    );
                }
                if let Some(formatted) =
                    self.formatted_text(s, crate::config::FormatMode::Horizontal)
                {
                    Some(horizontal_icon_label_with_style(
                        "",
                        crate::theme::TEXT,
                        formatted,
                        bar_style,
                    ))
                } else {
                    Some(horizontal_icon_label_with_style(
                        "⌚",
                        crate::theme::TEXT,
                        &s.time,
                        bar_style,
                    ))
                }
            }
            _ => Some(iced::widget::Space::new().into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ColorValue, SegmentStyles, Style, ThemeToken, Weight};
    use crate::modules::Module;
    use crate::modules::detail::ModuleDetailLifecycle;

    fn styled_format_config() -> crate::config::FormatConfig {
        let mut template = crate::config::FormatTemplate::parse(
            &["{time}"],
            &["icon", "value", "time", "date"],
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
    fn clock_styled_segments_resolve_inline_style() {
        let m = ClockModule::new(
            crate::modules::ModuleId::new("clock"),
            "%H:%M".into(),
            "%a".into(),
            styled_format_config(),
        );
        let state = ClockState {
            time: "12:34".into(),
            date: "Mon".into(),
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

    #[test]
    fn calendar_month_uses_monday_first_blank_outside_cells_and_marks_today() {
        use chrono::NaiveDate;

        let month = calendar_month_for(NaiveDate::from_ymd_opt(2026, 5, 23).unwrap(), 0);

        assert_eq!(month.year, 2026);
        assert_eq!(month.month, 5);
        assert_eq!(month.today, Some(23));
        assert_eq!(
            month.weeks[0],
            [None, None, None, None, Some(1), Some(2), Some(3)]
        );
        assert_eq!(
            *month.weeks.last().unwrap(),
            [
                Some(25),
                Some(26),
                Some(27),
                Some(28),
                Some(29),
                Some(30),
                Some(31)
            ]
        );
    }

    #[test]
    fn calendar_month_offset_browses_without_marking_today_in_other_months() {
        use chrono::NaiveDate;

        let month = calendar_month_for(NaiveDate::from_ymd_opt(2026, 5, 23).unwrap(), -1);

        assert_eq!(month.year, 2026);
        assert_eq!(month.month, 4);
        assert_eq!(month.today, None);
        assert_eq!(
            month.weeks[0],
            [None, None, Some(1), Some(2), Some(3), Some(4), Some(5)]
        );
    }

    #[test]
    fn clock_detail_session_initializes_current_month() {
        let module = ClockModule::new(
            crate::modules::ModuleId::new("clock"),
            "%H:%M".into(),
            "%a %d %b".into(),
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Clock(ClockState {
            time: "12:34".into(),
            date: "Sat 23 May".into(),
        });

        let session = module
            .create_module_detail_session(&state)
            .expect("clock opens a Module Detail Session");
        let session = session
            .downcast_ref::<ClockDetailSession>()
            .expect("clock owns the detail session type");

        assert_eq!(session.month_offset, 0);
    }

    #[test]
    fn clock_detail_session_browses_months_from_opaque_messages() {
        let mut module = ClockModule::new(
            crate::modules::ModuleId::new("clock"),
            "%H:%M".into(),
            "%a %d %b".into(),
            crate::config::FormatConfig::default(),
        );
        let mut session: crate::modules::ModuleDetailSession =
            Box::new(ClockDetailSession { month_offset: 0 });

        module.update_module_detail_session(
            &mut session,
            crate::modules::ModuleDetailMessage::new(ClockDetailMessage::PreviousMonth),
        );
        assert_eq!(
            session
                .downcast_ref::<ClockDetailSession>()
                .expect("clock session")
                .month_offset,
            -1
        );

        module.update_module_detail_session(
            &mut session,
            crate::modules::ModuleDetailMessage::new(ClockDetailMessage::NextMonth),
        );
        module.update_module_detail_session(
            &mut session,
            crate::modules::ModuleDetailMessage::new(ClockDetailMessage::NextMonth),
        );
        assert_eq!(
            session
                .downcast_ref::<ClockDetailSession>()
                .expect("clock session")
                .month_offset,
            1
        );

        module.update_module_detail_session(
            &mut session,
            crate::modules::ModuleDetailMessage::new(ClockDetailMessage::Today),
        );
        assert_eq!(
            session
                .downcast_ref::<ClockDetailSession>()
                .expect("clock session")
                .month_offset,
            0
        );
    }

    #[test]
    fn clock_detail_session_resets_when_reopened() {
        let mut module = ClockModule::new(
            crate::modules::ModuleId::new("clock"),
            "%H:%M".into(),
            "%a %d %b".into(),
            crate::config::FormatConfig::default(),
        );
        let state = crate::modules::ModuleState::Clock(ClockState {
            time: "12:34".into(),
            date: "Sat 23 May".into(),
        });
        let mut first_session = module
            .create_module_detail_session(&state)
            .expect("clock opens a Module Detail Session");

        module.update_module_detail_session(
            &mut first_session,
            crate::modules::ModuleDetailMessage::new(ClockDetailMessage::NextMonth),
        );
        assert_eq!(
            first_session
                .downcast_ref::<ClockDetailSession>()
                .expect("clock session")
                .month_offset,
            1
        );

        let reopened_session = module
            .create_module_detail_session(&state)
            .expect("clock reopens a fresh Module Detail Session");
        assert_eq!(
            reopened_session
                .downcast_ref::<ClockDetailSession>()
                .expect("clock session")
                .month_offset,
            0
        );
    }
}
