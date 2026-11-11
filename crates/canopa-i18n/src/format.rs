//! Locale-aware dates, times, numbers, sizes, durations and relative times.
//!
//! Dates and times use ICU4X with CLDR data compiled in, in the Gregorian
//! calendar, for the `LC_TIME` locale; numbers use ICU4X decimal formatting
//! for `LC_NUMERIC`. Units and relative times are Fluent messages
//! (`i18n/<language>/canopa_i18n.ftl`) with plural rules.
use crate::{Selection, generation, selection};
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Weekday};
use fixed_decimal::{Decimal, FloatPrecision};
use fluent::FluentValue;
use icu_calendar::{Date, Gregorian};
use icu_datetime::{
    FixedCalendarDateTimeFormatter, NoCalendarFormatter, fieldsets,
    input::{DateTime, Time},
    options::Length,
};
use icu_decimal::DecimalFormatter;
use icu_locale_core::Locale;
use std::{cell::RefCell, collections::HashMap};

/// How much of a date to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DateStyle {
    /// `11/11/26`, `11/11/2026` in French.
    Short,
    /// `Nov 11, 2026`, `11 nov. 2026`.
    Medium,
    /// `November 11, 2026`, `11 novembre 2026`.
    Long,
    /// `Wednesday, November 11, 2026`, `mercredi 11 novembre 2026`.
    Full,
}

/// Which hour cycle a time uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HourCycleChoice {
    /// The formats locale's own (`en-US` 12-hour, `fr` 24-hour).
    #[default]
    Locale,
    H12,
    H24,
}

fn locale(id: &unic_langid::LanguageIdentifier, cycle: HourCycleChoice) -> Locale {
    let mut locale: Locale = id
        .to_string()
        .parse()
        .unwrap_or_else(|_| "en".parse().expect("valid"));
    let hc = match cycle {
        HourCycleChoice::Locale => None,
        HourCycleChoice::H12 => Some("h12"),
        HourCycleChoice::H24 => Some("h23"),
    };
    if let Some(hc) = hc {
        locale.extensions.unicode.keywords.set(
            "hc".parse().expect("valid key"),
            hc.parse().expect("valid value"),
        );
    }
    locale
}

thread_local! {
    /// A formats locale set by [`with_formats`] on this thread.
    static OVERRIDE: RefCell<Option<unic_langid::LanguageIdentifier>> = const { RefCell::new(None) };
    static OVERRIDES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Run `f` with dates, times and numbers formatted for `formats` instead of
/// the selection, on this thread: for previews of another locale's formats
/// (Settings' Region & language page). Messages keep the selected language.
pub fn with_formats<R>(formats: &unic_langid::LanguageIdentifier, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<unic_langid::LanguageIdentifier>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OVERRIDE.with(|value| *value.borrow_mut() = self.0.take());
            OVERRIDES.with(|count| count.set(count.get() + 1));
        }
    }
    let previous = OVERRIDE.with(|value| value.borrow_mut().replace(formats.clone()));
    OVERRIDES.with(|count| count.set(count.get() + 1));
    let _restore = Restore(previous);
    f()
}

fn override_locale() -> Option<unic_langid::LanguageIdentifier> {
    OVERRIDE.with(|value| value.borrow().clone())
}

fn time_locale(cycle: HourCycleChoice) -> Locale {
    locale(
        &override_locale().unwrap_or_else(|| selection().time),
        cycle,
    )
}

fn number_locale() -> Locale {
    locale(
        &override_locale().unwrap_or_else(|| selection().numbers),
        HourCycleChoice::Locale,
    )
}

/// The cache token: the selection generation and this thread's overrides.
fn cache_token() -> (u64, u64) {
    (generation(), OVERRIDES.with(std::cell::Cell::get))
}

/// Formatters are cached per thread until the selection changes.
macro_rules! cached {
    ($key:ty, $value:ty, $key_expr:expr, $make:expr) => {{
        thread_local! {
            static CACHE: RefCell<((u64, u64), HashMap<$key, Option<$value>>)> =
                RefCell::new(((0, 0), HashMap::new()));
        }
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let current = cache_token();
            if cache.0 != current {
                *cache = (current, HashMap::new());
            }
            let key: $key = $key_expr;
            cache.1.entry(key).or_insert_with(|| $make(key)).clone()
        })
    }};
}

fn icu_date(date: NaiveDate) -> Option<Date<Gregorian>> {
    let month = u8::try_from(date.month()).ok()?;
    let day = u8::try_from(date.day()).ok()?;
    Date::try_new_gregorian(date.year(), month, day).ok()
}

fn icu_time(time: NaiveTime, seconds: bool) -> Option<Time> {
    Time::try_new(
        u8::try_from(time.hour()).ok()?,
        u8::try_from(time.minute()).ok()?,
        if seconds {
            u8::try_from(time.second().min(59)).ok()?
        } else {
            0
        },
        0,
    )
    .ok()
}

fn length(style: DateStyle) -> Length {
    match style {
        DateStyle::Short => Length::Short,
        DateStyle::Medium => Length::Medium,
        DateStyle::Long | DateStyle::Full => Length::Long,
    }
}

/// A date in the `LC_TIME` locale.
pub fn date(value: NaiveDate) -> String {
    date_with(value, DateStyle::Medium)
}

/// A date with the given amount of detail.
pub fn date_with(value: NaiveDate, style: DateStyle) -> String {
    let Some(input) = icu_date(value) else {
        return value.to_string();
    };
    let text = if style == DateStyle::Full {
        cached!(
            (),
            std::sync::Arc<FixedCalendarDateTimeFormatter<Gregorian, fieldsets::YMDE>>,
            (),
            |_| FixedCalendarDateTimeFormatter::try_new(
                time_locale(HourCycleChoice::Locale).into(),
                fieldsets::YMDE::long()
            )
            .ok()
            .map(std::sync::Arc::new)
        )
        .map(|f| f.format(&input).to_string())
    } else {
        cached!(
            DateStyle,
            std::sync::Arc<FixedCalendarDateTimeFormatter<Gregorian, fieldsets::YMD>>,
            style,
            |style| FixedCalendarDateTimeFormatter::try_new(
                time_locale(HourCycleChoice::Locale).into(),
                fieldsets::YMD::for_length(length(style))
            )
            .ok()
            .map(std::sync::Arc::new)
        )
        .map(|f| f.format(&input).to_string())
    };
    text.unwrap_or_else(|| value.to_string())
}

/// A time of day (hours and minutes) in the locale's hour cycle.
pub fn time(value: NaiveTime) -> String {
    time_with(value, false, HourCycleChoice::Locale)
}

/// A time of day, with seconds when asked, in `cycle`.
pub fn time_with(value: NaiveTime, seconds: bool, cycle: HourCycleChoice) -> String {
    let Some(input) = icu_time(value, seconds) else {
        return value.format("%H:%M").to_string();
    };
    cached!(
        (bool, HourCycleChoice),
        std::sync::Arc<NoCalendarFormatter<fieldsets::T>>,
        (seconds, cycle),
        |(seconds, cycle): (bool, HourCycleChoice)| {
            let fields = if seconds {
                fieldsets::T::hms()
            } else {
                fieldsets::T::hm()
            };
            NoCalendarFormatter::try_new(time_locale(cycle).into(), fields)
                .ok()
                .map(std::sync::Arc::new)
        }
    )
    .map(|f| f.format(&input).to_string())
    .unwrap_or_else(|| value.format("%H:%M").to_string())
}

/// A date and time (`Nov 11, 2026, 2:30 PM`; `11 nov. 2026, 14:30`).
pub fn date_time(value: NaiveDateTime) -> String {
    date_time_with(value, DateStyle::Medium)
}

pub fn date_time_with(value: NaiveDateTime, style: DateStyle) -> String {
    let (Some(date), Some(time)) = (icu_date(value.date()), icu_time(value.time(), false)) else {
        return value.to_string();
    };
    let input = DateTime { date, time };
    cached!(
        DateStyle,
        std::sync::Arc<FixedCalendarDateTimeFormatter<Gregorian, fieldsets::YMDT>>,
        style,
        |style| FixedCalendarDateTimeFormatter::try_new(
            time_locale(HourCycleChoice::Locale).into(),
            fieldsets::YMD::for_length(length(style)).with_time_hm()
        )
        .ok()
        .map(std::sync::Arc::new)
    )
    .map(|f| f.format(&input).to_string())
    .unwrap_or_else(|| value.to_string())
}

/// A weekday and month day (`Wed, Nov 11`; `mer. 11 nov.`).
pub fn weekday_month_day(value: NaiveDate) -> String {
    weekday_month_day_with(value, Length::Medium)
}

/// A weekday and month day in `length`: `Long` gives `Wednesday, November 11`
/// (`mercredi 11 novembre`), as the lock screen shows it.
pub fn weekday_month_day_with(value: NaiveDate, length: Length) -> String {
    let Some(input) = icu_date(value) else {
        return value.to_string();
    };
    cached!(
        Length,
        std::sync::Arc<FixedCalendarDateTimeFormatter<Gregorian, fieldsets::MDE>>,
        length,
        |length| FixedCalendarDateTimeFormatter::try_new(
            time_locale(HourCycleChoice::Locale).into(),
            fieldsets::MDE::for_length(length)
        )
        .ok()
        .map(std::sync::Arc::new)
    )
    .map(|f| f.format(&input).to_string())
    .unwrap_or_else(|| value.to_string())
}

/// A month and year (`November 2026`; `novembre 2026`).
pub fn month_year(value: NaiveDate) -> String {
    let Some(input) = icu_date(value) else {
        return value.format("%B %Y").to_string();
    };
    cached!(
        (),
        std::sync::Arc<FixedCalendarDateTimeFormatter<Gregorian, fieldsets::YM>>,
        (),
        |_| FixedCalendarDateTimeFormatter::try_new(
            time_locale(HourCycleChoice::Locale).into(),
            fieldsets::YM::long()
        )
        .ok()
        .map(std::sync::Arc::new)
    )
    .map(|f| f.format(&input).to_string())
    .unwrap_or_else(|| value.format("%B %Y").to_string())
}

/// A weekday's name: `Long` (`Monday`, `lundi`), `Medium` (`Mon`, `lun.`) or
/// `Short` (`M`, `L`).
pub fn weekday(day: Weekday, style: Length) -> String {
    // 2024-01-01 was a Monday.
    let date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid")
        + chrono::Days::new(u64::from(day.num_days_from_monday()));
    let Some(input) = icu_date(date) else {
        return day.to_string();
    };
    cached!(
        Length,
        std::sync::Arc<FixedCalendarDateTimeFormatter<Gregorian, fieldsets::E>>,
        style,
        |style| FixedCalendarDateTimeFormatter::try_new(
            time_locale(HourCycleChoice::Locale).into(),
            fieldsets::E::for_length(style)
        )
        .ok()
        .map(std::sync::Arc::new)
    )
    .map(|f| f.format(&input).to_string())
    .unwrap_or_else(|| day.to_string())
}

/// The first day of the week in the `LC_TIME` region (Sunday in the US,
/// Monday in France).
pub fn first_weekday() -> Weekday {
    let info = icu_calendar::week::WeekInformation::try_new(
        (&time_locale(HourCycleChoice::Locale)).into(),
    );
    match info.map(|info| info.first_weekday) {
        Ok(icu_calendar::types::Weekday::Sunday) => Weekday::Sun,
        Ok(icu_calendar::types::Weekday::Tuesday) => Weekday::Tue,
        Ok(icu_calendar::types::Weekday::Wednesday) => Weekday::Wed,
        Ok(icu_calendar::types::Weekday::Thursday) => Weekday::Thu,
        Ok(icu_calendar::types::Weekday::Friday) => Weekday::Fri,
        Ok(icu_calendar::types::Weekday::Saturday) => Weekday::Sat,
        _ => Weekday::Mon,
    }
}

/// Whether the `LC_TIME` locale writes times with a 24-hour clock.
pub fn uses_24_hour_clock() -> bool {
    let afternoon = NaiveTime::from_hms_opt(13, 0, 0).expect("valid");
    time(afternoon).contains("13")
}

fn decimal_formatter() -> Option<std::sync::Arc<DecimalFormatter>> {
    cached!((), std::sync::Arc<DecimalFormatter>, (), |_| {
        DecimalFormatter::try_new((&number_locale()).into(), Default::default())
            .ok()
            .map(std::sync::Arc::new)
    })
}

/// An integer with the locale's grouping (`1,234`; `1 234`).
pub fn integer(value: i64) -> String {
    match decimal_formatter() {
        Some(formatter) => formatter.format(&Decimal::from(value)).to_string(),
        None => value.to_string(),
    }
}

/// A number with at most `fraction_digits` decimals, trailing zeros removed
/// unless `keep_zeros` (`1.5`; `1,5`).
pub fn decimal(value: f64, fraction_digits: u8, keep_zeros: bool) -> String {
    let Ok(mut number) = Decimal::try_from_f64(value, FloatPrecision::RoundTrip) else {
        return value.to_string();
    };
    let position = -i16::from(fraction_digits);
    number.round(position);
    if keep_zeros {
        number.absolute.pad_end(position);
    } else {
        number.absolute.trim_end();
    }
    match decimal_formatter() {
        Some(formatter) => formatter.format(&number).to_string(),
        None => number.to_string(),
    }
}

/// A percentage of 0–100 (`45%`; `45 %`).
pub fn percent(value: f64) -> String {
    let number = decimal(value, 0, false);
    crate::i18n::LOADER
        .get()
        .get_args_concrete("format-percent", HashMap::from([("value", number.into())]))
}

fn unit(id: &str, number: String, count: f64) -> String {
    crate::i18n::LOADER.get().get_args_concrete(
        id,
        HashMap::from([
            ("value", number.into()),
            ("count", FluentValue::from(count)),
        ]),
    )
}

/// A size in binary units, as Files shows them (`512 B`, `1.5 KiB`, `2.0 GiB`;
/// `1,5 Kio`).
pub fn size(bytes: u64) -> String {
    if bytes < 1024 {
        return unit("format-size-bytes", integer(bytes as i64), bytes as f64);
    }
    let ids = [
        "format-size-kib",
        "format-size-mib",
        "format-size-gib",
        "format-size-tib",
    ];
    let mut amount = bytes as f64 / 1024.0;
    let mut index = 0;
    while amount >= 1024.0 && index + 1 < ids.len() {
        amount /= 1024.0;
        index += 1;
    }
    unit(ids[index], decimal(amount, 1, true), amount)
}

/// A size in decimal units, as disks are sold (`512 GB`, `1.0 TB`; `512 Go`).
pub fn decimal_size(bytes: u64) -> String {
    let ids = [
        "format-size-bytes",
        "format-size-kb",
        "format-size-mb",
        "format-size-gb",
        "format-size-tb",
        "format-size-pb",
    ];
    let mut value = bytes as f64;
    let mut index = 0;
    while value >= 1000.0 && index + 1 < ids.len() {
        value /= 1000.0;
        index += 1;
    }
    let number = if value >= 100.0 || index == 0 {
        decimal(value, 0, false)
    } else {
        decimal(value, 1, true)
    };
    unit(ids[index], number, value)
}

/// A duration in hours and minutes, or minutes and seconds under a minute
/// (`2 h 5 min`, `45 min`, `30 s`).
pub fn duration(seconds: u64) -> String {
    let messages = crate::i18n::LOADER.get();
    let (hours, minutes) = (seconds / 3600, seconds % 3600 / 60);
    if seconds < 60 {
        return messages.get_args_concrete(
            "format-duration-seconds",
            HashMap::from([("seconds", FluentValue::from(seconds))]),
        );
    }
    if hours == 0 {
        return messages.get_args_concrete(
            "format-duration-minutes",
            HashMap::from([("minutes", FluentValue::from(minutes))]),
        );
    }
    if minutes == 0 {
        return messages.get_args_concrete(
            "format-duration-hours",
            HashMap::from([("hours", FluentValue::from(hours))]),
        );
    }
    messages.get_args_concrete(
        "format-duration-hours-minutes",
        HashMap::from([
            ("hours", FluentValue::from(hours)),
            ("minutes", FluentValue::from(minutes)),
        ]),
    )
}

/// How long ago `seconds` was, or how far ahead for a negative value
/// (`now`, `5 minutes ago`, `in 2 hours`, `3 days ago`).
pub fn relative(seconds: i64) -> String {
    let messages = crate::i18n::LOADER.get();
    let future = seconds < 0;
    let distance = seconds.unsigned_abs();
    let (id, count) = match distance {
        0..=44 => return messages.get("format-relative-now"),
        45..=3_599 => ("minutes", distance.div_ceil(60).clamp(1, 59)),
        3_600..=86_399 => ("hours", distance / 3_600),
        86_400..=2_591_999 => ("days", distance / 86_400),
        2_592_000..=31_535_999 => ("months", distance / 2_592_000),
        _ => ("years", distance / 31_536_000),
    };
    let id = format!(
        "format-relative-{id}-{}",
        if future { "ahead" } else { "ago" }
    );
    messages.get_args_concrete(&id, HashMap::from([("count", FluentValue::from(count))]))
}

/// Fluent's number formatting: numeric arguments of every message use the
/// `LC_NUMERIC` locale's digits and separators.
pub(crate) fn fluent_value<M>(value: &FluentValue<'_>, _memoizer: &M) -> Option<String> {
    let FluentValue::Number(number) = value else {
        return None;
    };
    let digits = number.options.maximum_fraction_digits.unwrap_or(3).min(6) as u8;
    if !number.options.use_grouping {
        return None;
    }
    if number.value.fract() == 0.0 && number.value.abs() < 9e15 {
        Some(integer(number.value as i64))
    } else {
        Some(decimal(number.value, digits, false))
    }
}

/// The selection the formatting functions use (for tests and previews).
pub fn current() -> Selection {
    selection()
}
