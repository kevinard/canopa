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
    /// A day chosen on the grid: the agenda shows its events.
    selected: Option<chrono::NaiveDate>,
    /// The agenda lists every coming event, not just the next few.
    expanded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClockDetailMessage {
    PreviousMonth,
    NextMonth,
    Today,
    Select(chrono::NaiveDate),
    ToggleAgenda,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ClockUpdateCadence {
    Minute,
    Second,
}

fn format_needs_second_updates(format: &str) -> bool {
    use chrono::format::{Fixed, Item, Numeric, StrftimeItems};

    // Use the same directive expansion as rendering and configuration validation.
    // Composite directives (%c, %T, ...) expand to their constituent items.
    StrftimeItems::new(format).any(|item| {
        matches!(
            item,
            Item::Numeric(
                Numeric::Second | Numeric::Nanosecond | Numeric::Timestamp,
                _
            ) | Item::Fixed(
                Fixed::Nanosecond
                        | Fixed::Nanosecond3
                        | Fixed::Nanosecond6
                        | Fixed::Nanosecond9
                        | Fixed::RFC2822
                        | Fixed::RFC3339
                        // Valid output directives with opaque fixed items are
                        // fractional seconds without a dot (%3f, %6f, %9f).
                        | Fixed::Internal(_)
            )
        )
    })
}

/// Locale formats show hours and minutes, so only explicit patterns can need
/// second updates.
fn clock_update_cadence(
    time_format: Option<&str>,
    date_format: Option<&str>,
) -> ClockUpdateCadence {
    if time_format.is_some_and(format_needs_second_updates)
        || date_format.is_some_and(format_needs_second_updates)
    {
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

/// The full date a calendar cell reads as, such as "Today, Wednesday, November 11, 2026".
pub(super) fn calendar_day_label(year: i32, month: u32, day: u32, today: bool) -> String {
    let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month, day) else {
        return canopa_i18n::format::integer(i64::from(day));
    };
    let date = canopa_i18n::format::date_with(date, canopa_i18n::format::DateStyle::Full);
    if today {
        fl!("shell-calendar-today-cell", date = date)
    } else {
        date
    }
}

/// The clock's time: the person's Chrono pattern, or the language's format.
fn format_time(now: &chrono::DateTime<chrono::Local>, pattern: Option<&str>) -> String {
    match pattern {
        Some(pattern) => now.format(pattern).to_string(),
        None => canopa_i18n::format::time(now.time()),
    }
}

/// The clock's date: the person's Chrono pattern, or the language's weekday,
/// month and day ("Wednesday, November 11"; "mercredi 11 novembre").
fn format_date(now: &chrono::DateTime<chrono::Local>, pattern: Option<&str>) -> String {
    match pattern {
        Some(pattern) => now.format(pattern).to_string(),
        None => canopa_i18n::format::weekday_month_day_with(
            now.date_naive(),
            canopa_i18n::format::Length::Long,
        ),
    }
}

/// The week's days in calendar order, from the region's first weekday.
fn calendar_weekdays(first: chrono::Weekday) -> [chrono::Weekday; 7] {
    let mut days = [first; 7];
    for index in 1..7 {
        days[index] = days[index - 1].succ();
    }
    days
}

/// The weekday columns left to right: calendar order from the region's first
/// weekday, mirrored for right-to-left languages.
fn visual_weekdays() -> Vec<chrono::Weekday> {
    canopa_ui::direction::ordered(calendar_weekdays(canopa_i18n::format::first_weekday()))
}

pub(super) fn calendar_month_for(today: chrono::NaiveDate, month_offset: i32) -> CalendarMonth {
    calendar_month_from(today, month_offset, canopa_i18n::format::first_weekday())
}

/// The month around `today`, with weeks starting on `first_weekday`.
pub(super) fn calendar_month_from(
    today: chrono::NaiveDate,
    month_offset: i32,
    first_weekday: chrono::Weekday,
) -> CalendarMonth {
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
    let leading_blanks = (first.weekday().num_days_from_monday() + 7
        - first_weekday.num_days_from_monday()) as usize
        % 7;

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
    time_fmt: Option<&str>,
    date_fmt: Option<&str>,
) -> crate::app::Message {
    let now = chrono::Local::now();
    let time = format_time(&now, time_fmt);
    let date = format_date(&now, date_fmt);
    crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Module(
        id,
        crate::modules::ModuleEvent::Clock(ClockEvent::Tick { time, date }),
    ))
}

/// Clock module: shows current time and date, updated every second via subscription.
pub(super) struct ClockModule {
    id: crate::modules::ModuleId,
    time_format: Option<String>,
    date_format: Option<String>,
    format: crate::config::FormatConfig,
    content_rotation: crate::config::ContentRotation,
}

impl ClockModule {
    #[cfg(test)]
    pub(super) fn new(
        id: crate::modules::ModuleId,
        time_format: String,
        date_format: String,
        format: crate::config::FormatConfig,
    ) -> Self {
        Self::new_with_content_rotation(
            id,
            Some(time_format),
            Some(date_format),
            format,
            crate::config::ContentRotation::None,
        )
    }

    pub(super) fn new_with_content_rotation(
        id: crate::modules::ModuleId,
        time_format: Option<String>,
        date_format: Option<String>,
        format: crate::config::FormatConfig,
        content_rotation: crate::config::ContentRotation,
    ) -> Self {
        // Start reading local calendars now, so the agenda is ready when the
        // calendar first opens.
        let _ = super::agenda::snapshot();
        ClockModule {
            id,
            time_format,
            date_format,
            format,
            content_rotation,
        }
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
        selected: Option<chrono::NaiveDate>,
        expanded: bool,
    ) -> iced::Element<'_, crate::app::Message> {
        use canopa_ui::accessibility::{Live, Node, Role, semantic};
        use iced::widget::{column, container, row};
        use iced::{Alignment, Length};

        let crate::modules::ModuleState::Clock(_) = state else {
            return iced::widget::Space::new().into();
        };

        let today = chrono::Local::now().date_naive();
        // The panel's title is today's date; a configured expanded format adds
        // its own line above the month.
        let summary = self.format.expanded.is_some().then(|| {
            <Self as crate::modules::render::ModuleRender>::render_panel_with_style(
                self, state, bar_style,
            )
        });
        let month = calendar_month_for(today, month_offset);
        let first = chrono::NaiveDate::from_ymd_opt(month.year, month.month, 1)
            .expect("calendar month is valid");
        let title = canopa_i18n::format::month_year(first);

        let action = |label: String,
                      name: &str,
                      key: &str,
                      event: ClockDetailMessage,
                      width: f32| {
            let message =
                crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Detail {
                    target: self.id.clone(),
                    message: crate::modules::ModuleDetailMessage::new(event),
                });
            semantic(
                Node::button(name)
                    .key(format!("calendar-{key}"))
                    .on_click(Some(message.clone())),
                crate::theme::action(
                    container(crate::theme::strong_text(label).size(crate::theme::label_size()))
                        .center(Length::Fill),
                    Some(message),
                    canopa_ui::Action::Normal,
                    false,
                )
                .height(32)
                .padding(0)
                .width(width)
                // The same filled round control as the panel's Close button.
                .style(|theme, status| {
                    let colors = canopa_ui::colors(theme);
                    let engaged = matches!(
                        status,
                        iced::widget::button::Status::Hovered
                            | iced::widget::button::Status::Pressed
                    );
                    iced::widget::button::Style {
                        background: Some(
                            colors
                                .text
                                .scale_alpha(if engaged { 0.16 } else { 0.07 })
                                .into(),
                        ),
                        text_color: colors.text,
                        border: iced::Border::default().rounded(16),
                        ..Default::default()
                    }
                }),
            )
        };
        let today_label = fl!("shell-calendar-today");
        let today_width =
            (crate::theme::text_width(&today_label, crate::theme::label_size()) + 22.0).max(54.0);
        // Mirrored for right-to-left languages: the previous month is on the right.
        let controls = canopa_ui::direction::row([
            // The month is announced as it changes.
            semantic(
                Node::heading(title.clone(), 2)
                    .key("calendar-month")
                    .live(Live::Polite),
                crate::theme::strong_text(title.clone()),
            ),
            // The ISO week, where weeks start on Monday, of the month's first
            // shown week or of today.
            if canopa_i18n::format::first_weekday() == chrono::Weekday::Mon {
                use chrono::Datelike;
                let week = if month_offset == 0 { today } else { first };
                crate::theme::text(fl!(
                    "shell-calendar-week",
                    number = canopa_i18n::format::integer(i64::from(week.iso_week().week()))
                ))
                .size(crate::theme::label_size())
                .color(crate::theme::palette().muted)
                .wrapping(iced::widget::text::Wrapping::None)
                .into()
            } else {
                iced::widget::Space::new().into()
            },
            iced::widget::Space::new().width(Length::Fill).into(),
            // "Today" returns to today; already there, it steps aside.
            if month_offset == 0 && selected.is_none() {
                iced::widget::Space::new().width(today_width).into()
            } else {
                action(
                    today_label.clone(),
                    &today_label,
                    "today",
                    ClockDetailMessage::Today,
                    today_width,
                )
            },
            action(
                canopa_ui::direction::back_chevron().to_owned(),
                &fl!("shell-calendar-previous-month"),
                "previous",
                ClockDetailMessage::PreviousMonth,
                32.0,
            ),
            action(
                canopa_ui::direction::forward_chevron().to_owned(),
                &fl!("shell-calendar-next-month"),
                "next",
                ClockDetailMessage::NextMonth,
                32.0,
            ),
        ])
        .spacing(5)
        .align_y(Alignment::Center)
        .width(Length::Fill);

        let mut calendar = column![controls].spacing(10).width(Length::Fill);
        let mut weekdays = row![].spacing(4).width(Length::Fill);
        for day in visual_weekdays() {
            use canopa_i18n::format::{Length as Name, weekday};
            let (label, name) = (weekday(day, Name::Short), weekday(day, Name::Long));
            weekdays = weekdays.push(semantic(
                Node::new(Role::ColumnHeader).label(name).hide_text(),
                container(
                    crate::theme::strong_text(label)
                        .size(crate::theme::label_size())
                        .style(canopa_ui::muted),
                )
                .width(Length::FillPortion(1))
                .center_x(Length::Fill),
            ));
        }
        let mut grid = column![
            semantic(Node::new(Role::Row).key("calendar-weekdays"), weekdays),
            iced::widget::Space::new().height(2),
        ]
        .spacing(4)
        .width(Length::Fill);

        // The days of the neighbouring months fill the blank cells, dimmed.
        let previous_length = {
            use chrono::Datelike;
            first.pred_opt().map_or(31, |last| last.day())
        };
        let mut weeks: Vec<[(Option<u32>, Option<u32>); 7]> = Vec::new();
        let mut next = 1;
        for (index, week) in month.weeks.iter().enumerate() {
            let leading = week.iter().take_while(|day| day.is_none()).count() as u32;
            let mut cells = [(None, None); 7];
            for (column, day) in week.iter().enumerate() {
                cells[column] = match day {
                    Some(day) => (Some(*day), None),
                    None if index == 0 => {
                        (None, Some(previous_length + 1 + column as u32 - leading))
                    }
                    None => {
                        next += 1;
                        (None, Some(next - 1))
                    }
                };
            }
            weeks.push(cells);
        }
        let event_days: std::collections::HashSet<chrono::NaiveDate> = super::agenda::snapshot()
            .events
            .iter()
            .flat_map(|event| {
                // Every day an event covers, its last moment excluded.
                let last = (event.end - chrono::Duration::seconds(1)).date();
                event
                    .start
                    .date()
                    .iter_days()
                    .take_while(move |day| *day <= last)
            })
            .collect();
        let _ = next;
        for (index, week) in weeks.into_iter().enumerate() {
            let mut row = row![].spacing(4).width(Length::Fill);
            for (index_in_week, (day, adjacent)) in
                canopa_ui::direction::ordered(week).into_iter().enumerate()
            {
                let is_today = day.is_some() && day == month.today;
                let label = day
                    .or(adjacent)
                    .map(|day| canopa_i18n::format::integer(i64::from(day)))
                    .unwrap_or_default();
                let palette = crate::theme::palette();
                let weekend = matches!(
                    visual_weekdays()[index_in_week],
                    chrono::Weekday::Sat | chrono::Weekday::Sun
                );
                let side = (crate::theme::body_size() * 2.3).ceil().max(32.0);
                let date = day
                    .and_then(|day| chrono::NaiveDate::from_ymd_opt(month.year, month.month, day));
                // A neighbouring month's day, for its event dot.
                let shown = date.or_else(|| {
                    use chrono::Datelike;
                    let adjacent = adjacent?;
                    let neighbour = if index == 0 {
                        first.pred_opt()?
                    } else {
                        first.checked_add_months(chrono::Months::new(1))?
                    };
                    neighbour.with_day(adjacent)
                });
                let is_selected = date.is_some() && date == selected;
                // A day with events carries a small dot under its number.
                let busy = shown.is_some_and(|date| event_days.contains(&date));
                let text = if is_today {
                    crate::theme::strong_text(label).color(palette.on_accent)
                } else if day.is_none() {
                    // Another month's day is dimmed, all alike; its dot says
                    // whether it has events.
                    crate::theme::text(label).color(palette.muted.scale_alpha(0.78))
                } else if weekend {
                    // Weekends are set apart by weight, never dimmed like
                    // the days of other months.
                    crate::theme::text(label).color(palette.text.scale_alpha(0.86))
                } else {
                    crate::theme::text(label)
                };
                let dot =
                    container(iced::widget::Space::new().width(6).height(6)).style(move |_| {
                        container::Style::default()
                            .background(if !busy {
                                iced::Color::TRANSPARENT
                            } else if is_today {
                                palette.on_accent
                            } else {
                                palette.accent
                            })
                            .border(iced::Border::default().rounded(3.0))
                    });
                let number = column![text.size(crate::theme::body_size()), dot]
                    .spacing(1)
                    .align_x(Alignment::Center);
                let disc = container(number)
                    .width(side)
                    .height(side)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(move |_| {
                        if is_today {
                            container::Style::default()
                                .background(palette.accent)
                                .border(iced::Border::default().rounded(side / 2.0))
                        } else if is_selected {
                            // The chosen day: a ring, so today keeps its fill.
                            container::Style::default()
                                .background(palette.accent.scale_alpha(0.14))
                                .border(
                                    iced::Border::default()
                                        .rounded(side / 2.0)
                                        .width(1.5)
                                        .color(palette.accent),
                                )
                        } else {
                            container::Style::default()
                        }
                    });
                // A day of the month is a button: it shows that day's events.
                let disc: iced::Element<'_, crate::app::Message> = match date {
                    Some(date) => iced::widget::button(disc)
                        .padding(0)
                        .on_press(crate::app::Message::Modules(
                            crate::app::messages::ModuleRuntimeEvent::Detail {
                                target: self.id.clone(),
                                message: crate::modules::ModuleDetailMessage::new(
                                    ClockDetailMessage::Select(date),
                                ),
                            },
                        ))
                        .style(move |_, status| iced::widget::button::Style {
                            background: matches!(
                                status,
                                iced::widget::button::Status::Hovered
                                    | iced::widget::button::Status::Pressed
                            )
                            .then(|| palette.text.scale_alpha(0.08).into()),
                            text_color: palette.text,
                            border: iced::Border::default().rounded(side / 2.0),
                            ..Default::default()
                        })
                        .into(),
                    None => disc.into(),
                };
                let cell = container(disc)
                    .width(Length::FillPortion(1))
                    .align_x(Alignment::Center);
                // Blank cells keep each day in its weekday's column.
                let node = match day {
                    Some(day) => Node::new(Role::GridCell)
                        .label(calendar_day_label(month.year, month.month, day, is_today))
                        .selected(is_today),
                    None => Node::new(Role::GridCell),
                };
                row = row.push(semantic(node, cell));
            }
            grid = grid.push(semantic(
                Node::new(Role::Row).key(format!("calendar-week-{index}")),
                row,
            ));
        }
        calendar = calendar.push(semantic(Node::grid(title).key("calendar-grid"), grid));

        let mut detail = column![].spacing(16).width(Length::Fill);
        if let Some(summary) = summary {
            detail = detail.push(container(summary).padding([4, 0]).width(Length::Fill));
        }
        // What comes next leads; the month follows to look further ahead.
        let mut detail = detail;
        let agenda = super::agenda::snapshot();
        if agenda.has_calendars {
            let toggle =
                crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Detail {
                    target: self.id.clone(),
                    message: crate::modules::ModuleDetailMessage::new(
                        ClockDetailMessage::ToggleAgenda,
                    ),
                });
            detail = detail.push(agenda_view(&agenda, today, selected, expanded, toggle));
        }
        detail = detail.push(
            container(calendar)
                .padding([12, 10])
                .width(Length::Fill)
                .style(|_| {
                    container::Style::default()
                        .background(crate::theme::palette().text.scale_alpha(0.045))
                        .border(iced::Border::default().rounded(18))
                }),
        );
        detail.into()
    }
}

/// Events the agenda lists when no day is chosen.
const UPCOMING: usize = 4;

/// How many of the coming events the agenda lists: `UPCOMING`, plus the rest
/// of the last day shown (two at most), so a day is never split.
fn listed_count(agenda: &super::agenda::Agenda) -> usize {
    let mut count = UPCOMING.min(agenda.events.len());
    if let Some(last) = agenda.events.get(count.wrapping_sub(1)) {
        let day = last.start.date();
        while count < UPCOMING + 2
            && agenda
                .events
                .get(count)
                .is_some_and(|event| event.start.date() == day)
        {
            count += 1;
        }
    }
    count
}

/// Height of the agenda below the month grid, for the panel's size.
fn agenda_height(agenda: &super::agenda::Agenda) -> f32 {
    if !agenda.has_calendars {
        return 0.0;
    }
    let upcoming = &agenda.events[..listed_count(agenda)];
    let rows = upcoming.len().max(1) as f32;
    let mut days: Vec<_> = upcoming.iter().map(|event| event.start.date()).collect();
    days.dedup();
    // The gap above, the heading, each event, each day's name and the "more"
    // link when the week has more.
    let week_end = chrono::Local::now().date_naive() + chrono::Duration::days(7);
    let more = agenda.events[upcoming.len()..]
        .iter()
        .any(|event| event.start.date() < week_end);
    // The coming days head their own events; only an empty agenda has a
    // heading of its own.
    let heading = if upcoming.is_empty() { 24.0 } else { 0.0 };
    16.0 + heading
        + rows * 44.0
        + days.len() as f32 * 26.0
        + days.len().saturating_sub(1) as f32 * 6.0
        + if more { 30.0 } else { 0.0 }
}

/// The coming week's events from local calendars: what, when and where.
fn agenda_view<'a>(
    agenda: &super::agenda::Agenda,
    today: chrono::NaiveDate,
    selected: Option<chrono::NaiveDate>,
    expanded: bool,
    toggle: crate::app::Message,
) -> iced::Element<'a, crate::app::Message> {
    use canopa_ui::accessibility::{Node, semantic};
    // Expanded, the agenda lists the coming week; the panel scrolls.
    let shown = if expanded { 24 } else { UPCOMING };
    use iced::widget::{column, container, row};
    use iced::{Alignment, Length};
    let palette = crate::theme::palette();
    // A chosen day lists its own events; otherwise what is coming next.
    let events: Vec<&super::agenda::Event> = match selected {
        Some(day) => agenda.on(day).take(shown).collect(),
        None => {
            let count = if expanded {
                shown.min(agenda.events.len())
            } else {
                listed_count(agenda)
            };
            agenda.events[..count]
                .iter()
                .filter(|event| !expanded || event.start.date() < today + chrono::Duration::days(7))
                .collect()
        }
    };
    let heading = match selected {
        Some(day) => {
            canopa_i18n::format::weekday_month_day_with(day, canopa_i18n::format::Length::Long)
        }
        None => fl!("shell-calendar-upcoming"),
    };
    // The coming days head their own events: "Upcoming" above "Today" would
    // say the same twice.
    let mut list = column![].spacing(6).width(Length::Fill);
    if selected.is_some() || events.is_empty() {
        list = list.push(semantic(
            Node::heading(heading.clone(), 3).key("calendar-upcoming"),
            crate::theme::strong_text(heading).size(crate::theme::body_size()),
        ));
    }
    if events.is_empty() {
        list = list.push(
            crate::theme::text(if selected.is_some() {
                fl!("shell-calendar-no-events-day")
            } else {
                fl!("shell-calendar-no-events")
            })
            .size(crate::theme::body_size())
            .color(palette.muted),
        );
    }
    let listed = events.len();
    let mut previous_day = None;
    for event in events {
        let day = selected.unwrap_or(event.start.date());
        // Later days read with their date: "Tomorrow · November 28", "Monday,
        // November 30". Today's date heads the panel already.
        let day_label = if day == today {
            fl!("shell-calendar-today")
        } else if Some(day) == today.succ_opt() {
            format!(
                "{} · {}",
                fl!("shell-calendar-tomorrow"),
                canopa_i18n::format::month_day(day, canopa_i18n::format::Length::Long)
            )
        } else {
            canopa_i18n::format::weekday_month_day_with(day, canopa_i18n::format::Length::Long)
        };
        let when = if event.all_day {
            fl!("shell-calendar-all-day")
        } else {
            let end = canopa_i18n::format::time(event.end.time());
            // An event that runs past midnight says so.
            let end = if event.end.date() > event.start.date()
                && event.end.time() != chrono::NaiveTime::MIN
            {
                fl!("shell-calendar-ends-next-day", time = end)
            } else {
                end
            };
            format!(
                "{} – {}",
                canopa_i18n::format::time(event.start.time()),
                end
            )
        };
        // What is under way or about to begin says so first.
        let now = chrono::Local::now().naive_local();
        let soon = (!event.all_day)
            .then(|| {
                if event.start <= now && now < event.end {
                    Some(fl!("shell-calendar-now"))
                } else {
                    let minutes = (event.start - now).num_minutes();
                    (0..60)
                        .contains(&minutes)
                        .then(|| fl!("shell-calendar-in-minutes", minutes = (minutes.max(1))))
                }
            })
            .flatten();
        let when = match &soon {
            Some(_) => format!(" · {when}"),
            None => when,
        };
        // Events are grouped under their day's name (a chosen day has its
        // name in the heading already).
        if selected.is_none() && previous_day != Some(day) {
            // Later days stand apart from the events above them.
            let later_group = previous_day.is_some();
            previous_day = Some(day);
            list = list.push(
                container(semantic(
                    Node::heading(day_label.clone(), 3),
                    crate::theme::strong_text(day_label.clone())
                        .size(crate::theme::label_size() + 1.0)
                        .color(if day == today {
                            palette.accent
                        } else {
                            palette.muted
                        }),
                ))
                .padding(iced::Padding {
                    top: if later_group { 8.0 } else { 0.0 },
                    ..Default::default()
                }),
            );
        }
        let mut detail = when;
        if let Some(location) = &event.location {
            detail.push_str(" · ");
            detail.push_str(location);
        }
        let room = 300.0;
        let summary = crate::theme::ellipsize(&event.summary, room, crate::theme::body_size());
        let detail_size = crate::theme::label_size() + 1.0;
        // What is under way or about to begin leads, in the accent: it is the
        // panel's most useful fact.
        let soon_width = soon.as_ref().map_or(0.0, |soon| {
            crate::theme::text_width(soon, detail_size) + 2.0
        });
        let detail_line = crate::theme::ellipsize(&detail, room - soon_width, detail_size);
        let detail_text = match &soon {
            Some(soon) => format!("{soon}{detail}"),
            None => detail.clone(),
        };
        let bar = container(iced::widget::Space::new().width(3).height(32)).style(move |_| {
            container::Style::default()
                .background(if day == today {
                    palette.accent
                } else {
                    palette.accent.scale_alpha(0.45)
                })
                .border(iced::Border::default().rounded(2))
        });
        list = list.push(semantic(
            Node::new(canopa_ui::accessibility::Role::ListItem)
                .label(format!("{}, {}, {}", event.summary, day_label, detail_text)),
            row![
                bar,
                column![
                    crate::theme::strong_text(summary)
                        .size(crate::theme::body_size())
                        .wrapping(iced::widget::text::Wrapping::None),
                    row(soon.map(|soon| {
                        crate::theme::strong_text(soon)
                            .size(detail_size)
                            .color(palette.accent)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .into()
                    }))
                    .push(
                        // Times read clearly; the muted tone is for headings.
                        crate::theme::text(detail_line)
                            .size(detail_size)
                            .color(palette.text.scale_alpha(0.8))
                            .wrapping(iced::widget::text::Wrapping::None),
                    ),
                ]
                .spacing(2),
            ]
            .spacing(10)
            .height(38)
            .align_y(Alignment::Center),
        ));
    }
    // What the list leaves out this week is counted, never silently dropped.
    if selected.is_none() {
        let week_end = today + chrono::Duration::days(7);
        let more = agenda
            .events
            .iter()
            .skip(listed)
            .filter(|event| event.start.date() < week_end)
            .count();
        // A link that shows the rest in place, or folds them away again.
        if more > 0 {
            let label = if expanded {
                fl!("shell-calendar-fewer-events")
            } else {
                fl!("shell-calendar-more-events", count = (more as i64))
            };
            list = list.push(semantic(
                Node::button(label.clone())
                    .key("calendar-more")
                    .on_click(Some(toggle.clone())),
                iced::widget::button(
                    row![
                        crate::theme::strong_text(label)
                            .size(crate::theme::label_size() + 1.0)
                            .color(palette.accent),
                        crate::theme::text(if expanded { "⌃" } else { "⌄" })
                            .size(crate::theme::label_size() + 1.0)
                            .color(palette.accent),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([4, 8])
                .on_press(toggle)
                .style(move |_, status| iced::widget::button::Style {
                    background: matches!(
                        status,
                        iced::widget::button::Status::Hovered
                            | iced::widget::button::Status::Pressed
                    )
                    .then(|| palette.accent.scale_alpha(0.12).into()),
                    border: iced::Border::default().rounded(8),
                    text_color: palette.accent,
                    ..Default::default()
                }),
            ));
        }
    }
    container(list).padding([0, 4]).width(Length::Fill).into()
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
        let cadence =
            clock_update_cadence(self.time_format.as_deref(), self.date_format.as_deref());
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

                        let msg = build_clock_message(
                            id.clone(),
                            time_fmt.as_deref(),
                            date_fmt.as_deref(),
                        );
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
        matches!(state, crate::modules::ModuleState::Clock(_)).then(|| {
            Box::new(ClockDetailSession {
                month_offset: 0,
                selected: None,
                expanded: false,
            }) as _
        })
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
                session.selected = None;
            }
            // Choosing the selected day again returns to what is coming.
            ClockDetailMessage::Select(day) => {
                session.selected = (session.selected != Some(*day)).then_some(*day);
            }
            ClockDetailMessage::ToggleAgenda => {
                session.expanded = !session.expanded;
            }
        }

        crate::modules::ModuleDetailUpdate::default()
    }
}

impl crate::modules::render::ModuleRender for ClockModule {
    fn presentation(&self) -> Option<&dyn crate::modules::view::IconLabel> {
        Some(self)
    }

    /// On a vertical spine a 12-hour time carries its day period as a small
    /// suffix ("11:58ᴾᴹ"), with the short date below, so it fits the rail.
    fn render_bar_collapsed_with_style(
        &self,
        state: &crate::modules::ModuleState,
        context: &crate::modules::render::BarRenderContext<'_>,
        style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        let crate::modules::ModuleState::Clock(clock) = state else {
            return None;
        };
        if context.edge.is_horizontal()
            || self.content_rotation != crate::config::ContentRotation::None
        {
            return None;
        }
        let segments =
            self.styled_segments_with_default(clock, crate::config::FormatMode::Collapsed, style)?;
        let [(text, resolved)] = segments.as_slice() else {
            return None;
        };
        if text.trim() != clock.time.trim() {
            return None;
        }
        // ICU separates the day period with a (narrow) no-break space.
        let (digits, period) = match text.trim().rsplit_once(char::is_whitespace) {
            Some((digits, period))
                if !period.is_empty()
                    && period.chars().all(char::is_alphabetic)
                    && !digits.chars().any(char::is_alphabetic) =>
            {
                (digits.trim_end(), Some(period))
            }
            _ => (text.trim(), None),
        };
        let color = resolved.color.unwrap_or(crate::theme::palette().text);
        let today = chrono::Local::now().date_naive();
        // "Nov 12": the short month and day, under the time; the tooltip and
        // the calendar give the full date.
        let date = canopa_i18n::format::month_day_short(today);
        // The day period follows the digits as a small suffix ("11:58ᴾᴹ"), so
        // the time stays on one line.
        let mut time = iced::widget::row![
            crate::theme::strong_text(digits.to_owned())
                .size(resolved.size.unwrap_or(14.0).max(14.0))
                .color(color)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(3)
        .align_y(iced::Alignment::End);
        if let Some(period) = period {
            time = time.push(
                iced::widget::container(
                    crate::theme::strong_text(period.to_owned())
                        .size(9)
                        .color(color.scale_alpha(0.75))
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .padding(iced::Padding {
                    bottom: 2.0,
                    ..Default::default()
                }),
            );
        }
        let mut column = iced::widget::column![time]
            .align_x(iced::Alignment::Center)
            .width(iced::Length::Fill);
        column = column.push(
            iced::widget::container(
                crate::theme::text(date)
                    .size(12)
                    .color(crate::theme::palette().muted)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .padding(iced::Padding {
                top: 3.0,
                ..Default::default()
            }),
        );
        Some(column.into())
    }

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

    fn render_detail_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        Some(self.view_clock_detail_with_month_offset(state, bar_style, 0, None, false))
    }

    fn render_detail_session_with_style(
        &self,
        state: &crate::modules::ModuleState,
        bar_style: &crate::config::Style,
        session: &crate::modules::ModuleDetailSession,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        let (month_offset, selected, expanded) = session
            .downcast_ref::<ClockDetailSession>()
            .map(|session| (session.month_offset, session.selected, session.expanded))
            .unwrap_or_default();

        Some(self.view_clock_detail_with_month_offset(
            state,
            bar_style,
            month_offset,
            selected,
            expanded,
        ))
    }

    fn detail_preferred_size(
        &self,
        _state: &crate::modules::ModuleState,
        _bar_style: &crate::config::Style,
        session: Option<&crate::modules::ModuleDetailSession>,
    ) -> Option<crate::surface::panel::PanelPreferredSize> {
        // Five or six week rows, as the shown month needs.
        let month_offset = session
            .and_then(|session| session.downcast_ref::<ClockDetailSession>())
            .map_or(0, |session| session.month_offset);
        let rows = calendar_month_for(chrono::Local::now().date_naive(), month_offset)
            .weeks
            .len() as f32;
        Some(crate::surface::panel::PanelPreferredSize {
            width: (324.0
                * (crate::theme::body_size() / 14.0).max(crate::theme::label_size() / 12.0))
            .ceil() as u32,
            height: ((76.0 + 37.0 * rows + agenda_height(&super::agenda::snapshot()))
                * (crate::theme::body_size() / 14.0).max(crate::theme::label_size() / 12.0))
            .ceil() as u32,
        })
    }
}

impl crate::modules::view::IconLabel for ClockModule {
    fn content_rotation(&self) -> crate::config::ContentRotation {
        self.content_rotation
    }
    fn accepts_state(&self, state: &crate::modules::ModuleState) -> bool {
        matches!(state, crate::modules::ModuleState::Clock(_))
    }
    fn icon(&self, _state: &crate::modules::ModuleState) -> String {
        "⌚".into()
    }
    fn label(&self, state: &crate::modules::ModuleState) -> String {
        if let crate::modules::ModuleState::Clock(s) = state {
            s.time.clone()
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
        if let crate::modules::ModuleState::Clock(s) = state {
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
    fn formatted_size(&self, _layout: crate::modules::view::IconLabelLayout) -> u32 {
        crate::theme::label_size() as u32
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
                    self.label(state),
                    self.color(state),
                    style,
                )
            }
            crate::modules::view::IconLabelLayout::Expanded => {
                let crate::modules::ModuleState::Clock(clock) = state else {
                    return iced::widget::Space::new().into();
                };
                iced::widget::column![
                    crate::modules::view::spaced_icon_label(
                        self.icon(state),
                        self.label(state),
                        style
                    ),
                    crate::modules::view::styled_segment_text(
                        clock.date.clone(),
                        11,
                        self.color(state),
                        crate::modules::view::fallback_resolved_style(style)
                    ),
                ]
                .width(iced::Length::Fill)
                .into()
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
    fn calendar_cells_read_as_full_dates() {
        let _english = crate::i18n_testing::english();
        assert_eq!(
            calendar_day_label(2026, 11, 11, true),
            "Today, Wednesday, November 11, 2026"
        );
        assert_eq!(
            calendar_day_label(2026, 2, 1, false),
            "Sunday, February 1, 2026"
        );
        assert_eq!(calendar_day_label(2026, 2, 30, false), "30");
    }

    #[test]
    fn clock_module_id_is_clock() {
        let m = ClockModule::new(
            crate::modules::ModuleId::new("clock"),
            "%H:%M".into(),
            "%a %d %b".into(),
            crate::config::FormatConfig::default(),
        );
        assert_eq!(m.id().display_name(), "clock");
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
        assert_eq!(m.id().display_name(), "my-clock");
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

        assert_eq!(segment.1.color, Some(crate::theme::palette().accent));
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
            clock_update_cadence(Some("%H:%M"), Some("%a %d %b")),
            ClockUpdateCadence::Minute
        );
        // The language's formats show minutes.
        assert_eq!(clock_update_cadence(None, None), ClockUpdateCadence::Minute);
        assert_eq!(
            clock_update_cadence(None, Some("%T")),
            ClockUpdateCadence::Second
        );
    }

    #[test]
    fn clock_update_cadence_uses_seconds_when_format_shows_seconds() {
        assert_eq!(
            clock_update_cadence(Some("%H:%M:%S"), Some("%a %d %b")),
            ClockUpdateCadence::Second
        );
        assert_eq!(
            clock_update_cadence(Some("%H:%M"), Some("%c")),
            ClockUpdateCadence::Second
        );
    }

    #[test]
    fn clock_update_cadence_follows_chrono_composites_padding_and_fractions() {
        for format in [
            "%_S", "%T", "%X", "%r", "%c", "%+", "%s", "%f", "%.3f", "%3f", "%6f", "%9f",
        ] {
            assert_eq!(
                clock_update_cadence(Some(format), Some("%Y")),
                ClockUpdateCadence::Second,
                "{format}"
            );
        }
        for format in ["%R", "%%S", "%:z", "%::z", "%:::z", "%Z"] {
            assert_eq!(
                clock_update_cadence(Some(format), Some("%Y")),
                ClockUpdateCadence::Minute,
                "{format}"
            );
        }
    }

    #[test]
    fn clock_update_cadence_ignores_escaped_percent() {
        assert_eq!(
            clock_update_cadence(Some("%%S %H:%M"), Some("%Y-%m-%d")),
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

        let month = calendar_month_from(
            NaiveDate::from_ymd_opt(2026, 5, 23).unwrap(),
            0,
            chrono::Weekday::Mon,
        );

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
    fn calendar_weeks_start_on_the_regions_first_weekday() {
        use chrono::{NaiveDate, Weekday};

        let may = NaiveDate::from_ymd_opt(2026, 5, 23).unwrap();
        let english = crate::i18n_testing::english();
        // English formats are US: weeks start on Sunday. May 1, 2026 is a Friday.
        assert_eq!(canopa_i18n::format::first_weekday(), Weekday::Sun);
        assert_eq!(
            calendar_month_for(may, 0).weeks[0],
            [None, None, None, None, None, Some(1), Some(2)]
        );
        assert_eq!(
            calendar_weekdays(Weekday::Sun),
            [
                Weekday::Sun,
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
                Weekday::Sat
            ]
        );
        drop(english);
        // Saturday-first regions.
        assert_eq!(
            calendar_month_from(may, 0, Weekday::Sat).weeks[0],
            [None, None, None, None, None, None, Some(1)]
        );
    }

    #[test]
    fn french_calendar_starts_on_monday_with_french_names() {
        use chrono::{NaiveDate, TimeZone, Weekday};

        let _french = crate::i18n_testing::select("fr", false);
        let may = NaiveDate::from_ymd_opt(2026, 5, 23).unwrap();
        assert_eq!(canopa_i18n::format::first_weekday(), Weekday::Mon);
        assert_eq!(
            calendar_month_for(may, 0).weeks[0],
            [None, None, None, None, Some(1), Some(2), Some(3)]
        );
        assert_eq!(
            calendar_day_label(2026, 11, 11, true),
            "Aujourd’hui, mercredi 11 novembre 2026"
        );
        assert_eq!(canopa_i18n::format::month_year(may), "mai 2026");
        let now = chrono::Local
            .with_ymd_and_hms(2026, 11, 11, 14, 30, 0)
            .unwrap();
        assert_eq!(format_time(&now, None), "14:30");
        assert_eq!(format_date(&now, None), "mercredi 11 novembre");
        // A person's own pattern stays as written.
        assert_eq!(format_time(&now, Some("%I.%M")), "02.30");
    }

    #[test]
    fn right_to_left_calendar_mirrors_weekdays_and_month_navigation() {
        use chrono::Weekday;

        // The pseudo-locale is right to left with US English formats.
        let _pseudo = crate::i18n_testing::select("en", true);
        let days = visual_weekdays();
        assert_eq!(days.first(), Some(&Weekday::Sat));
        assert_eq!(days.last(), Some(&Weekday::Sun));
        // "Previous month" points right, toward the earlier days.
        assert_eq!(canopa_ui::direction::back_chevron(), "›");
        assert_eq!(canopa_ui::direction::forward_chevron(), "‹");
    }

    #[test]
    fn english_clock_follows_the_locale_hour_cycle() {
        use chrono::TimeZone;

        let _english = crate::i18n_testing::english();
        let now = chrono::Local
            .with_ymd_and_hms(2026, 11, 11, 14, 30, 0)
            .unwrap();
        assert_eq!(format_time(&now, None).replace('\u{202f}', " "), "2:30 PM");
        assert_eq!(format_date(&now, None), "Wednesday, November 11");
        assert_eq!(format_time(&now, Some("%H:%M")), "14:30");
    }

    #[test]
    fn calendar_month_offset_browses_without_marking_today_in_other_months() {
        use chrono::NaiveDate;

        let month = calendar_month_from(
            NaiveDate::from_ymd_opt(2026, 5, 23).unwrap(),
            -1,
            chrono::Weekday::Mon,
        );

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
        let mut session: crate::modules::ModuleDetailSession = Box::new(ClockDetailSession {
            month_offset: 0,
            selected: None,
            expanded: false,
        });

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
