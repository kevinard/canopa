//! Upcoming events from local iCalendar files, for the calendar's agenda.
//!
//! The Shell reads calendars that already live on this computer: Evolution's
//! local calendars (`$XDG_DATA_HOME/evolution/calendar/*/calendar.ics`, which
//! GNOME Calendar also uses for its "Personal" calendar) and vdir folders such
//! as those vdirsyncer and khal keep (`$XDG_DATA_HOME/calendars/**/*.ics`). It
//! never writes them and never goes online. A background thread rereads them
//! every minute, off the UI thread; views read the latest snapshot. It keeps
//! the next six weeks, the most the month grid can show from today.
//!
//! The parser covers what local calendars contain in practice: one-off and
//! all-day events, and daily, weekly (with `BYDAY`), monthly and yearly
//! repetition with `INTERVAL`, `COUNT` and `UNTIL`. Times with a `TZID` are
//! read as local time; `Z` times are converted from UTC.
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Weekday};

/// One occurrence of an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub summary: String,
    pub location: Option<String>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub all_day: bool,
}

/// What the agenda knows: whether any calendar exists, and the occurrences
/// from the start of today through the next six weeks, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Agenda {
    pub has_calendars: bool,
    pub events: Vec<Event>,
}

impl Agenda {
    /// The occurrences that touch `day`.
    pub fn on(&self, day: NaiveDate) -> impl Iterator<Item = &Event> {
        self.events.iter().filter(move |event| event.covers(day))
    }
}

impl Event {
    /// Whether the event takes place on `day` (its end moment excluded).
    pub fn covers(&self, day: NaiveDate) -> bool {
        let last = (self.end - Duration::seconds(1))
            .date()
            .max(self.start.date());
        self.start.date() <= day && day <= last
    }
}

/// Events from the start of today to this many days ahead: the whole month
/// grid the calendar can show from today.
const DAYS_AHEAD: i64 = 42;
/// At most this many occurrences are kept.
const MAX_EVENTS: usize = 256;
const MAX_FILES: usize = 256;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DEPTH: usize = 4;

/// The latest agenda. The first call starts the reader thread.
pub fn snapshot() -> Arc<Agenda> {
    static AGENDA: OnceLock<Arc<RwLock<Arc<Agenda>>>> = OnceLock::new();
    let shared = AGENDA.get_or_init(|| {
        let shared = Arc::new(RwLock::new(Arc::new(Agenda::default())));
        // Unit tests render calendars without reading this computer's files.
        #[cfg(not(test))]
        {
            let writer = shared.clone();
            let _ = std::thread::Builder::new()
                .name("canopa-agenda".into())
                .spawn(move || {
                    loop {
                        let agenda = Arc::new(read(&roots(), Local::now().naive_local()));
                        if let Ok(mut current) = writer.write() {
                            *current = agenda;
                        }
                        std::thread::sleep(std::time::Duration::from_secs(60));
                    }
                });
        }
        shared
    });
    shared
        .read()
        .map(|agenda| agenda.clone())
        .unwrap_or_default()
}

#[cfg(not(test))]
fn roots() -> Vec<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    data.map(|data| vec![data.join("evolution/calendar"), data.join("calendars")])
        .unwrap_or_default()
}

/// Read every calendar under `roots` and keep what is coming after `now`.
pub fn read(roots: &[PathBuf], now: NaiveDateTime) -> Agenda {
    let mut files = Vec::new();
    for root in roots {
        collect(root, 0, &mut files);
    }
    let mut events = Vec::new();
    for file in &files {
        let Ok(metadata) = std::fs::metadata(file) else {
            continue;
        };
        if metadata.len() > MAX_FILE_BYTES {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(file) {
            events.extend(occurrences(&text, now));
        }
    }
    events.sort_by(|a, b| (a.start, &a.summary).cmp(&(b.start, &b.summary)));
    events.dedup();
    events.truncate(MAX_EVENTS);
    Agenda {
        has_calendars: !files.is_empty(),
        events,
    }
}

fn collect(directory: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || files.len() >= MAX_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            collect(&path, depth + 1, files);
        } else if kind.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ics"))
            && files.len() < MAX_FILES
        {
            files.push(path);
        }
    }
}

/// Unfolded content lines: a line starting with a space or tab continues the previous one.
fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        let raw = raw.trim_end_matches('\r');
        if let Some(rest) = raw.strip_prefix([' ', '\t'])
            && let Some(last) = lines.last_mut()
        {
            last.push_str(rest);
        } else {
            lines.push(raw.to_owned());
        }
    }
    lines
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push(' '),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_owned()
}

#[derive(Default)]
struct Raw {
    summary: Option<String>,
    location: Option<String>,
    start: Option<(String, String)>,
    end: Option<(String, String)>,
    rule: Option<String>,
    cancelled: bool,
}

/// Every occurrence in `text` from the start of `now`'s day to `DAYS_AHEAD` days later.
pub fn occurrences(text: &str, now: NaiveDateTime) -> Vec<Event> {
    let window_start = now.date().and_time(NaiveTime::MIN);
    let window_end = window_start + Duration::days(DAYS_AHEAD + 1);
    let mut events = Vec::new();
    let mut current: Option<Raw> = None;
    for line in unfold(text) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let (name, params) = name.split_once(';').unwrap_or((name, ""));
        match (name.to_ascii_uppercase().as_str(), &mut current) {
            ("BEGIN", _) if value.eq_ignore_ascii_case("VEVENT") => current = Some(Raw::default()),
            ("END", Some(_)) if value.eq_ignore_ascii_case("VEVENT") => {
                if let Some(raw) = current.take() {
                    expand(raw, now, window_start, window_end, &mut events);
                }
            }
            ("SUMMARY", Some(raw)) => raw.summary = Some(unescape(value)),
            ("LOCATION", Some(raw)) => {
                raw.location = Some(unescape(value)).filter(|location| !location.is_empty())
            }
            ("DTSTART", Some(raw)) => raw.start = Some((params.to_owned(), value.to_owned())),
            ("DTEND", Some(raw)) => raw.end = Some((params.to_owned(), value.to_owned())),
            ("RRULE", Some(raw)) => raw.rule = Some(value.to_owned()),
            ("STATUS", Some(raw)) => raw.cancelled = value.eq_ignore_ascii_case("CANCELLED"),
            _ => {}
        }
    }
    events
}

/// A date or date-time value: (local start, all-day).
fn parse_time(params: &str, value: &str) -> Option<(NaiveDateTime, bool)> {
    let value = value.trim();
    if params.to_ascii_uppercase().contains("VALUE=DATE") && !params.contains("DATE-TIME")
        || value.len() == 8
    {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        return Some((date.and_time(NaiveTime::MIN), true));
    }
    if let Some(utc) = value.strip_suffix(['Z', 'z']) {
        let time = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        let local = Local.from_utc_datetime(&time).naive_local();
        return Some((local, false));
    }
    let time = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
    Some((time, false))
}

fn expand(
    raw: Raw,
    now: NaiveDateTime,
    window_start: NaiveDateTime,
    window_end: NaiveDateTime,
    events: &mut Vec<Event>,
) {
    if raw.cancelled {
        return;
    }
    let Some((params, value)) = &raw.start else {
        return;
    };
    let Some((start, all_day)) = parse_time(params, value) else {
        return;
    };
    let end = raw
        .end
        .as_ref()
        .and_then(|(params, value)| parse_time(params, value))
        .map(|(end, _)| end)
        .filter(|end| *end > start)
        .unwrap_or(
            start
                + if all_day {
                    Duration::days(1)
                } else {
                    Duration::hours(1)
                },
        );
    let length = end - start;
    let summary = raw.summary.clone().unwrap_or_default();
    let mut push = |start: NaiveDateTime| {
        let end = start + length;
        // Keep what is still to come or under way; an all-day event lasts its day.
        if end > now.max(window_start) && start < window_end {
            events.push(Event {
                summary: summary.clone(),
                location: raw.location.clone(),
                start,
                end,
                all_day,
            });
        }
    };
    let Some(rule) = &raw.rule else {
        push(start);
        return;
    };
    let mut frequency = None;
    let mut interval = 1i64;
    let mut count = None;
    let mut until = None;
    let mut by_day: Vec<Weekday> = Vec::new();
    for part in rule.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key.to_ascii_uppercase().as_str() {
            "FREQ" => frequency = Some(value.to_ascii_uppercase()),
            "INTERVAL" => interval = value.parse().unwrap_or(1).max(1),
            "COUNT" => count = value.parse::<u32>().ok(),
            "UNTIL" => until = parse_time("", value).map(|(until, _)| until),
            "BYDAY" => {
                by_day = value
                    .split(',')
                    .filter_map(|day| {
                        let code = day.trim_start_matches(|c: char| {
                            c == '-' || c == '+' || c.is_ascii_digit()
                        });
                        match code.to_ascii_uppercase().as_str() {
                            "MO" => Some(Weekday::Mon),
                            "TU" => Some(Weekday::Tue),
                            "WE" => Some(Weekday::Wed),
                            "TH" => Some(Weekday::Thu),
                            "FR" => Some(Weekday::Fri),
                            "SA" => Some(Weekday::Sat),
                            "SU" => Some(Weekday::Sun),
                            _ => None,
                        }
                    })
                    .collect()
            }
            _ => {}
        }
    }
    let Some(frequency) = frequency else {
        push(start);
        return;
    };
    // Walk occurrences in order until past the window, the count or UNTIL.
    let mut produced = 0u32;
    let mut emit = |occurrence: NaiveDateTime| -> bool {
        if until.is_some_and(|until| occurrence > until)
            || count.is_some_and(|count| produced >= count)
            || occurrence >= window_end
        {
            return false;
        }
        produced += 1;
        push(occurrence);
        true
    };
    match frequency.as_str() {
        "DAILY" => {
            let mut occurrence = start;
            for _ in 0..20_000 {
                if !emit(occurrence) {
                    break;
                }
                occurrence += Duration::days(interval);
            }
        }
        "WEEKLY" => {
            let days = if by_day.is_empty() {
                vec![start.weekday()]
            } else {
                by_day
            };
            let week_start =
                start.date() - Duration::days(i64::from(start.weekday().num_days_from_monday()));
            'weeks: for week in 0..5_000i64 {
                let monday = week_start + Duration::weeks(week * interval);
                let mut week_days: Vec<_> = days
                    .iter()
                    .map(|day| monday + Duration::days(i64::from(day.num_days_from_monday())))
                    .collect();
                week_days.sort();
                for day in week_days {
                    let occurrence = day.and_time(start.time());
                    if occurrence < start {
                        continue;
                    }
                    if !emit(occurrence) {
                        break 'weeks;
                    }
                }
            }
        }
        "MONTHLY" | "YEARLY" => {
            let step = if frequency == "MONTHLY" {
                interval
            } else {
                12 * interval
            };
            for index in 0..2_400i64 {
                let months = start.month0() as i64 + index * step;
                let year = start.year() as i64 + months.div_euclid(12);
                let month = months.rem_euclid(12) as u32 + 1;
                // Months without the day (the 31st, February 29th) are skipped.
                let Some(date) = NaiveDate::from_ymd_opt(year as i32, month, start.day()) else {
                    continue;
                };
                if !emit(date.and_time(start.time())) {
                    break;
                }
            }
        }
        _ => push(start),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap()
    }

    const CALENDAR: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Design review\r\nLOCATION:Room 3\\, east wing\r\nDTSTART;TZID=Europe/Paris:20261127T190000\r\nDTEND;TZID=Europe/Paris:20261127T193000\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Stand-\r\n up\r\nDTSTART:20261101T090000\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Holiday\r\nDTSTART;VALUE=DATE:20261130\r\nDTEND;VALUE=DATE:20261201\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Gone\r\nDTSTART:20261127T080000\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Cancelled\r\nSTATUS:CANCELLED\r\nDTSTART:20261128T080000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn upcoming_events_unfold_repeat_and_skip_past_and_cancelled_ones() {
        let events = occurrences(CALENDAR, at("2026-11-27 18:30"));
        let names: Vec<_> = events
            .iter()
            .map(|event| (event.summary.as_str(), event.start))
            .collect();
        assert!(names.contains(&("Design review", at("2026-11-27 19:00"))));
        assert!(names.contains(&("Stand-up", at("2026-11-30 09:00"))));
        assert!(names.contains(&("Stand-up", at("2026-12-02 09:00"))));
        assert!(
            !names
                .iter()
                .any(|(name, _)| *name == "Gone" || *name == "Cancelled")
        );
        let holiday = events
            .iter()
            .find(|event| event.summary == "Holiday")
            .unwrap();
        assert!(holiday.all_day);
        assert_eq!(holiday.end, at("2026-12-01 00:00"));
        let review = events
            .iter()
            .find(|event| event.summary == "Design review")
            .unwrap();
        assert_eq!(review.location.as_deref(), Some("Room 3, east wing"));
    }

    #[test]
    fn counts_and_until_bound_repetition() {
        let text = "BEGIN:VEVENT\nSUMMARY:Run\nDTSTART:20261125T070000\nRRULE:FREQ=DAILY;COUNT=4\nEND:VEVENT\nBEGIN:VEVENT\nSUMMARY:Rent\nDTSTART;VALUE=DATE:20260131\nRRULE:FREQ=MONTHLY;UNTIL=20270101\nEND:VEVENT\n";
        let events = occurrences(text, at("2026-11-27 06:00"));
        let runs = events.iter().filter(|event| event.summary == "Run").count();
        // The 25th to the 28th: the 27th and 28th are still ahead.
        assert_eq!(runs, 2);
        // November has no 31st; December's is the next one.
        let rent: Vec<_> = events
            .iter()
            .filter(|event| event.summary == "Rent")
            .collect();
        assert_eq!(rent.len(), 1);
        assert_eq!(
            rent[0].start.date(),
            NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()
        );
    }

    #[test]
    fn calendars_are_found_in_folders_and_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("evolution/calendar/system");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("calendar.ics"), CALENDAR).unwrap();
        let agenda = read(
            &[
                temp.path().join("evolution/calendar"),
                temp.path().join("calendars"),
            ],
            at("2026-11-27 18:30"),
        );
        assert!(agenda.has_calendars);
        assert_eq!(agenda.events[0].summary, "Design review");
        let monday = NaiveDate::from_ymd_opt(2026, 11, 30).unwrap();
        let names: Vec<_> = agenda
            .on(monday)
            .map(|event| event.summary.as_str())
            .collect();
        assert_eq!(names, ["Holiday", "Stand-up"]);
        let empty = read(&[temp.path().join("nothing")], at("2026-11-27 18:30"));
        assert_eq!(empty, Agenda::default());
    }
}
