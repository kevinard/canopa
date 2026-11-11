//! The Date & time page: the time zone (searchable, or from the location
//! when GeoClue is installed), network time, the date and time when network
//! time is off, and the 12/24-hour clock for Canopa. timedated asks polkit
//! for each system change.
use super::{Message, State, field_id, live_page::LivePage, settings_group, toggle};
use crate::{
    clock_format::{ClockFormats, HourCycle},
    date_time::{DateTimeAction, DateTimeService, DateTimeState, local_usec, now_label},
    live::Event,
    storage::Snapshot,
};
use iced::{
    Alignment, Element, Length, Task,
    widget::{column, row, text},
};

pub(super) type DateTimePage = LivePage<DateTimeService>;

pub(super) fn message(event: Event<DateTimeService>) -> Message {
    Message::DateTime(event)
}

fn apply(action: DateTimeAction) -> Message {
    message(Event::Apply(action))
}

fn edit(edit: Edit) -> Message {
    Message::DateTimeForm(edit)
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
}

/// Time zones shown for a search, at most.
const MATCHES: usize = 12;

#[derive(Clone, Debug)]
pub(super) enum Edit {
    ZoneQuery(String),
    Date(String),
    Time(String),
    SetTime,
    HourCycle(HourCycle),
    HourCycleStored(Result<(Snapshot, Snapshot), String>),
    /// Redraw the current time.
    Tick,
}

/// The page's form input and the clock formats read from `config.kdl`.
#[derive(Default)]
pub(super) struct DateTimeForms {
    zone_query: String,
    date: String,
    time: String,
    /// `None` until read; an error when `config.kdl` cannot be read.
    formats: Option<Result<ClockFormats, String>>,
}

/// Zones matching every word of `query`, ignoring case, `_` and `/`.
pub(super) fn matching<'a>(zones: &'a [String], query: &str) -> Vec<&'a String> {
    let words: Vec<String> = query
        .split_whitespace()
        .map(|word| word.to_lowercase().replace('_', " "))
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    zones
        .iter()
        .filter(|zone| {
            let name = zone.to_lowercase().replace(['_', '/'], " ");
            words.iter().all(|word| name.contains(word.as_str()))
        })
        .take(MATCHES)
        .collect()
}

impl State {
    /// Read the clock formats and fill the manual date and time.
    pub(super) fn date_time_opened(&mut self) {
        let forms = &mut self.date_time_forms;
        forms.formats = Some(
            Snapshot::load(self.paths.common())
                .map_err(|error| format!("{error:#}"))
                .and_then(|source| {
                    ClockFormats::read(&source.text).map_err(|error| format!("{error:#}"))
                }),
        );
        let (date, time) = crate::date_time::now_fields();
        forms.date = date;
        forms.time = time;
    }

    pub(super) fn date_time_edit(&mut self, change: Edit) -> Task<Message> {
        let forms = &mut self.date_time_forms;
        match change {
            Edit::ZoneQuery(query) => forms.zone_query = query.chars().take(64).collect(),
            Edit::Date(value) => forms.date = value.chars().take(10).collect(),
            Edit::Time(value) => forms.time = value.chars().take(8).collect(),
            Edit::SetTime => {
                return match local_usec(&forms.date, &forms.time) {
                    Some(usec) => self
                        .date_time
                        .update(Event::Apply(DateTimeAction::Time(usec)), message),
                    None => {
                        self.notice = "Enter the date as YYYY-MM-DD and the time as HH:MM.".into();
                        Task::none()
                    }
                };
            }
            Edit::HourCycle(cycle) if !self.busy && self.trial.is_none() => {
                self.busy = true;
                self.notice.clear();
                let paths = self.paths.clone();
                return super::task(
                    move || crate::clock_format::set_hour_cycle(&paths, cycle),
                    |result| edit(Edit::HourCycleStored(result)),
                );
            }
            Edit::HourCycle(_) => {}
            Edit::HourCycleStored(result) => {
                self.busy = false;
                match result {
                    Ok((previous, saved)) => {
                        self.date_time_forms.formats =
                            Some(ClockFormats::read(&saved.text).map_err(|e| format!("{e:#}")));
                        self.notice = "Clock format updated.".into();
                        self.document_saved(
                            crate::Page::DateTime,
                            crate::registry::Document::Common,
                            Some((&previous, &saved)),
                        );
                        // Personalization shows the lock-screen clock format:
                        // an unsaved draft takes the new one with the file.
                        if let Some(lock) = crate::clock_format::lock_format(&saved.text) {
                            self.personal.follow(&previous, &saved, |draft| {
                                draft.values.lock.clock_format = lock;
                            });
                        }
                    }
                    Err(error) => self.notice = error,
                }
            }
            Edit::Tick => {}
        }
        Task::none()
    }

    fn twelve_hour(&self) -> bool {
        matches!(
            &self.date_time_forms.formats,
            Some(Ok(formats)) if formats.cycle() == Some(HourCycle::H12)
        )
    }

    fn time_zone_section<'a>(&'a self, state: &'a DateTimeState) -> Element<'a, Message> {
        let page = &self.date_time;
        let forms = &self.date_time_forms;
        let current = match page.live.pending("timezone") {
            Some(DateTimeAction::Timezone(zone)) => zone.as_str(),
            _ if state.clock.timezone.is_empty() => "Not set (UTC)",
            _ => state.clock.timezone.as_str(),
        };
        let mut body = column![
            text(format!("Time zone: {}", current.replace('_', " "))),
            muted(now_label(self.twelve_hour())),
        ]
        .spacing(10);
        if state.locate {
            body = body.push(
                row![
                    self.action(
                        "Use my location",
                        Some(apply(DateTimeAction::Locate)),
                        false
                    ),
                    muted("Sets the time zone of this computer's location once, through GeoClue."),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            );
        }
        body = body.push(field_id(
            "date-time-zone-search",
            "Find a time zone (city or region)",
            &forms.zone_query,
            |value| edit(Edit::ZoneQuery(value)),
        ));
        let found = matching(&state.zones, &forms.zone_query);
        if !forms.zone_query.trim().is_empty() && found.is_empty() {
            body = body.push(muted("No time zone matches."));
        }
        for zone in found {
            body = body.push(
                row![
                    text(zone.replace('_', " ")).width(Length::Fill),
                    if *zone == state.clock.timezone {
                        muted("In use")
                    } else {
                        super::button(
                            "Use",
                            Some(apply(DateTimeAction::Timezone(zone.clone()))),
                            canopa_ui::Action::Normal,
                            Length::Shrink,
                            false,
                        )
                    },
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            );
        }
        body = body.push(page.control_status("timezone", message));
        settings_group("Time zone", body)
    }

    fn clock_section<'a>(&'a self, state: &'a DateTimeState) -> Element<'a, Message> {
        let page = &self.date_time;
        let forms = &self.date_time_forms;
        let ntp = match page.live.pending("ntp") {
            Some(DateTimeAction::Ntp(value)) => *value,
            _ => state.clock.ntp,
        };
        let mut body = column![].spacing(10);
        if state.clock.can_ntp {
            body = body
                .push(toggle(
                    "date-time-ntp",
                    "Set the time automatically from the network",
                    ntp,
                    apply(DateTimeAction::Ntp(!ntp)),
                ))
                .push(muted(if !state.clock.ntp {
                    "Off: the clock keeps the time set below."
                } else if state.clock.ntp_synchronized {
                    "The clock is synchronized."
                } else {
                    "Waiting for a time server…"
                }))
                .push(page.control_status("ntp", message));
        } else {
            body = body.push(muted(
                "No network time service (systemd-timesyncd, chrony or ntpd) is installed.",
            ));
        }
        if !state.clock.ntp {
            body = body
                .push(
                    row![
                        field_id("date-time-date", "Date (YYYY-MM-DD)", &forms.date, |v| {
                            edit(Edit::Date(v))
                        }),
                        field_id("date-time-time", "Time (HH:MM)", &forms.time, |v| {
                            edit(Edit::Time(v))
                        }),
                    ]
                    .spacing(12),
                )
                .push(self.action("Set date and time", Some(edit(Edit::SetTime)), false))
                .push(page.control_status("time", message));
        }
        if state.clock.local_rtc {
            body = body.push(muted(
                "The hardware clock keeps local time, as Windows does; this can confuse daylight saving changes.",
            ));
        }
        settings_group("Date and time", body)
    }

    fn hour_cycle_section(&self) -> Element<'_, Message> {
        let body: Element<'_, Message> = match &self.date_time_forms.formats {
            None => muted("Reading config.kdl…"),
            Some(Err(error)) => muted(format!(
                "The clock formats are set in config.kdl, which could not be read: {error}"
            )),
            Some(Ok(formats)) => {
                let cycle = formats.cycle();
                let mut body = column![
                    row![
                        self.action(
                            "24-hour (14:30)",
                            (cycle != Some(HourCycle::H24))
                                .then(|| edit(Edit::HourCycle(HourCycle::H24))),
                            cycle == Some(HourCycle::H24),
                        ),
                        self.action(
                            "12-hour (2:30 PM)",
                            (cycle != Some(HourCycle::H12))
                                .then(|| edit(Edit::HourCycle(HourCycle::H12))),
                            cycle == Some(HourCycle::H12),
                        ),
                    ]
                    .spacing(10),
                    muted("For the bar clock and the lock screen. Applies at once."),
                ]
                .spacing(10);
                for clock in formats.custom() {
                    body = body.push(muted(format!(
                        "{} uses its own format, {}, which is kept.",
                        clock.place, clock.format
                    )));
                }
                body.into()
            }
        };
        settings_group("Clock format", body)
    }

    pub(super) fn date_time_view(&self) -> Element<'_, Message> {
        column![
            self.date_time.view(message, |state| {
                column![self.time_zone_section(state), self.clock_section(state)]
                    .spacing(14)
                    .into()
            }),
            self.hour_cycle_section(),
        ]
        .spacing(14)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_are_found_by_city_or_region() {
        let zones: Vec<String> = ["America/New_York", "America/Los_Angeles", "Europe/Paris"]
            .map(str::to_owned)
            .to_vec();
        assert_eq!(matching(&zones, "new york"), [&zones[0]]);
        assert_eq!(matching(&zones, "america").len(), 2);
        assert_eq!(matching(&zones, "PARIS"), [&zones[2]]);
        assert!(matching(&zones, "").is_empty());
        assert!(matching(&zones, "tokyo").is_empty());
    }
}
