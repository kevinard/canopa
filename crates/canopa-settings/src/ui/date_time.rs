//! The Date & time page: the time zone (searchable, or from the location
//! when GeoClue is installed), network time, the date and time when network
//! time is off, and the 12/24-hour clock for Canopa. timedated asks polkit
//! for each system change.
use super::{Message, State, described_toggle, field, kit, live_page::LivePage, settings_group};
use crate::{
    clock_format::{ClockChoice, ClockFormats, HourCycle},
    date_time::{DateTimeAction, DateTimeService, DateTimeState, local_usec, now_label},
    live::Event,
    storage::Snapshot,
};
use canopa_i18n::format::HourCycleChoice;
use canopa_ui::direction;
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
    text(label).size(13).style(canopa_ui::muted).into()
}

/// Time zones shown for a search, at most.
const MATCHES: usize = 12;

#[derive(Clone, Debug)]
pub(super) enum Edit {
    ZoneQuery(String),
    Date(String),
    Time(String),
    SetTime,
    Clock(ClockChoice),
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

/// A time zone as people say it: "Europe/Paris" → "Paris · Europe",
/// "America/Argentina/Buenos_Aires" → "Buenos Aires · America".
fn readable_zone(zone: &str) -> String {
    let parts: Vec<_> = zone.split('/').map(|part| part.replace('_', " ")).collect();
    match parts.as_slice() {
        [region, .., city] => format!("{city} · {region}"),
        _ => zone.replace('_', " "),
    }
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
                        self.notice = fl!("settings-date-time-bad-entry");
                        Task::none()
                    }
                };
            }
            Edit::Clock(choice) if !self.busy && self.trial.is_none() => {
                self.busy = true;
                self.notice.clear();
                let paths = self.paths.clone();
                return super::task(
                    move || crate::clock_format::set_choice(&paths, choice),
                    |result| edit(Edit::HourCycleStored(result)),
                );
            }
            Edit::Clock(_) => {}
            Edit::HourCycleStored(result) => {
                self.busy = false;
                match result {
                    Ok((previous, saved)) => {
                        self.date_time_forms.formats =
                            Some(ClockFormats::read(&saved.text).map_err(|e| format!("{e:#}")));
                        self.notice = fl!("settings-clock-updated");
                        self.document_saved(
                            crate::Page::DateTime,
                            crate::registry::Document::Common,
                            Some((&previous, &saved)),
                        );
                        // Personalization shows the lock-screen clock format:
                        // an unsaved draft takes the new one with the file.
                        if let Some(lock) = crate::clock_format::lock_format(&saved.text) {
                            self.personal.follow(&previous, &saved, |draft| {
                                draft.values.lock.clock_format = lock.clone();
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

    /// The hour cycle the clocks show: a fixed choice, else the language's.
    fn hour_cycle(&self) -> HourCycleChoice {
        match &self.date_time_forms.formats {
            Some(Ok(formats)) => match formats.choice() {
                Some(ClockChoice::Fixed(HourCycle::H12)) => HourCycleChoice::H12,
                Some(ClockChoice::Fixed(HourCycle::H24)) => HourCycleChoice::H24,
                _ => HourCycleChoice::Locale,
            },
            _ => HourCycleChoice::Locale,
        }
    }

    fn time_zone_section<'a>(&'a self, state: &'a DateTimeState) -> Element<'a, Message> {
        let page = &self.date_time;
        let forms = &self.date_time_forms;
        let current = match page.live.pending("timezone") {
            Some(DateTimeAction::Timezone(zone)) => readable_zone(zone),
            _ if state.clock.timezone.is_empty() => fl!("settings-date-time-zone-unset"),
            _ => readable_zone(&state.clock.timezone),
        };
        let mut body = column![kit::row(
            current,
            Some(now_label(self.hour_cycle())),
            iced::widget::space(),
        )]
        .spacing(12);
        if state.locate {
            body = body
                .push(iced::widget::rule::horizontal(1).style(kit::divider))
                .push(kit::row(
                    fl!("settings-date-time-locate-label"),
                    Some(fl!("settings-date-time-locate-hint")),
                    self.action(
                        "settings-button-use-my-location",
                        fl!("settings-date-time-locate"),
                        Some(apply(DateTimeAction::Locate)),
                        false,
                    ),
                ));
        }
        body = body.push(iced::widget::rule::horizontal(1).style(kit::divider));
        body = body.push(super::hinted_field(
            "date-time-zone-search",
            fl!("settings-date-time-zone-search"),
            &fl!("settings-date-time-zone-placeholder"),
            &forms.zone_query,
            |value| edit(Edit::ZoneQuery(value)),
        ));
        let found = matching(&state.zones, &forms.zone_query);
        if !forms.zone_query.trim().is_empty() && found.is_empty() {
            body = body.push(muted(fl!("settings-date-time-zone-none")));
        }
        for zone in found {
            body = body.push(
                direction::row([
                    text(zone.replace('_', " "))
                        .width(Length::Fill)
                        .align_x(direction::text_start())
                        .into(),
                    if *zone == state.clock.timezone {
                        muted(fl!("settings-date-time-zone-in-use"))
                    } else {
                        super::button(
                            "settings-button-use",
                            fl!("settings-date-time-zone-use"),
                            Some(apply(DateTimeAction::Timezone(zone.clone()))),
                            canopa_ui::Action::Normal,
                            Length::Shrink,
                            false,
                        )
                    },
                ])
                .spacing(10)
                .align_y(Alignment::Center),
            );
        }
        body = body.push(page.control_status("timezone", message));
        settings_group(fl!("settings-date-time-zone"), body)
    }

    fn clock_section<'a>(&'a self, state: &'a DateTimeState) -> Element<'a, Message> {
        let page = &self.date_time;
        let forms = &self.date_time_forms;
        let ntp = match page.live.pending("ntp") {
            Some(DateTimeAction::Ntp(value)) => *value,
            _ => state.clock.ntp,
        };
        let mut body = column![].spacing(6);
        if state.clock.can_ntp {
            body = body
                .push(described_toggle(
                    "date-time-ntp",
                    fl!("settings-date-time-ntp"),
                    Some(if !state.clock.ntp {
                        fl!("settings-date-time-ntp-off")
                    } else if state.clock.ntp_synchronized {
                        fl!("settings-date-time-ntp-synchronized")
                    } else {
                        fl!("settings-date-time-ntp-waiting")
                    }),
                    ntp,
                    apply(DateTimeAction::Ntp(!ntp)),
                ))
                .push(page.control_status("ntp", message));
        } else {
            body = body.push(muted(fl!("settings-date-time-ntp-missing")));
        }
        if !state.clock.ntp {
            body = body
                .push(
                    row![
                        field(
                            "date-time-date",
                            fl!("settings-date-time-date-field"),
                            &forms.date,
                            |v| { edit(Edit::Date(v)) }
                        ),
                        field(
                            "date-time-time",
                            fl!("settings-date-time-time-field"),
                            &forms.time,
                            |v| { edit(Edit::Time(v)) }
                        ),
                    ]
                    .spacing(12),
                )
                .push(self.action(
                    "settings-button-set-date-and-time",
                    fl!("settings-date-time-set"),
                    Some(edit(Edit::SetTime)),
                    false,
                ))
                .push(page.control_status("time", message));
        }
        if state.clock.local_rtc {
            body = body.push(muted(fl!("settings-date-time-local-rtc")));
        }
        settings_group(fl!("settings-date-time-clock"), body)
    }

    fn hour_cycle_section(&self) -> Element<'_, Message> {
        let body: Element<'_, Message> = match &self.date_time_forms.formats {
            None => muted(fl!("settings-clock-reading")),
            Some(Err(error)) => muted(fl!("settings-clock-unreadable", error = error.as_str())),
            Some(Ok(formats)) => {
                let choice = formats.choice();
                let afternoon = "14:30:00".parse().expect("a valid time");
                let example = |cycle| canopa_i18n::format::time_with(afternoon, false, cycle);
                let option = |key: &'static str, label: String, option: ClockChoice| {
                    self.segment(
                        key,
                        label,
                        (choice != Some(option)).then(|| edit(Edit::Clock(option))),
                        choice == Some(option),
                    )
                };
                let language = if canopa_i18n::format::uses_24_hour_clock() {
                    fl!(
                        "settings-clock-automatic-24",
                        example = example(HourCycleChoice::Locale)
                    )
                } else {
                    fl!(
                        "settings-clock-automatic-12",
                        example = example(HourCycleChoice::Locale)
                    )
                };
                let segments = kit::segmented([
                    option(
                        "settings-button-clock-automatic",
                        fl!("settings-clock-automatic"),
                        ClockChoice::Automatic,
                    ),
                    option(
                        "settings-button-24-hour-14-30",
                        fl!("settings-clock-24", example = example(HourCycleChoice::H24)),
                        ClockChoice::Fixed(HourCycle::H24),
                    ),
                    option(
                        "settings-button-12-hour-2-30-pm",
                        fl!("settings-clock-12", example = example(HourCycleChoice::H12)),
                        ClockChoice::Fixed(HourCycle::H12),
                    ),
                ]);
                // The choice is wide; it sits under its explanation.
                let mut body = column![
                    column![
                        text(fl!("settings-clock-format-label")),
                        kit::caption(language),
                    ]
                    .spacing(2),
                    segments,
                ]
                .spacing(12);
                for clock in formats.custom() {
                    body = body.push(muted(fl!(
                        "settings-clock-custom",
                        place = clock.place.label(),
                        format = clock.format.clone().unwrap_or_default()
                    )));
                }
                body.into()
            }
        };
        super::noted_group(
            fl!("settings-clock-format"),
            body,
            [fl!("settings-clock-applies")],
        )
    }

    pub(super) fn date_time_view(&self) -> Element<'_, Message> {
        column![
            self.date_time.view(message, |state| {
                column![self.time_zone_section(state), self.clock_section(state)]
                    .spacing(28)
                    .into()
            }),
            self.hour_cycle_section(),
        ]
        .spacing(28)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_read_city_first() {
        assert_eq!(readable_zone("Europe/Paris"), "Paris · Europe");
        assert_eq!(
            readable_zone("America/Argentina/Buenos_Aires"),
            "Buenos Aires · America"
        );
        assert_eq!(readable_zone("UTC"), "UTC");
    }

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

    #[test]
    fn automatic_clocks_follow_the_language_again() {
        let (_root, mut state) = crate::ui::tests::fresh();
        let _ = state.update(Message::Page(crate::Page::DateTime));
        // As set_choice does, without the owner's validator.
        let store = |state: &mut State, choice| {
            let source = Snapshot::load(state.paths.common()).unwrap();
            let text = crate::clock_format::with_choice(&source.text, choice).unwrap();
            let result = Ok((source.clone(), source.save(&text, |_| Ok(())).unwrap()));
            state.busy = true;
            let _ = state.date_time_edit(Edit::HourCycleStored(result));
        };
        store(&mut state, ClockChoice::Fixed(HourCycle::H12));
        assert_eq!(state.hour_cycle(), HourCycleChoice::H12, "{}", state.notice);
        assert_eq!(state.personal.value.values.lock.clock_format, "%-I:%M %p");
        store(&mut state, ClockChoice::Automatic);
        assert_eq!(state.notice, "Clock format updated.");
        let Some(Ok(formats)) = &state.date_time_forms.formats else {
            panic!("the clock formats are read");
        };
        assert_eq!(formats.choice(), Some(ClockChoice::Automatic));
        assert_eq!(state.hour_cycle(), HourCycleChoice::Locale);
        // The lock screen follows too; Personalization's draft takes it.
        assert_eq!(state.personal.value.values.lock.clock_format, "");
        let saved = std::fs::read_to_string(state.paths.common()).unwrap();
        let reread = ClockFormats::read(&saved).unwrap();
        assert!(reread.clocks.iter().all(|clock| clock.format.is_none()));
        assert!(saved.contains("// keep this note"));
        drop(state.view());
    }
}
