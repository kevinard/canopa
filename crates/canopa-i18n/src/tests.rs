use super::*;
use chrono::{NaiveDate, NaiveTime, Weekday};
use std::sync::{Mutex, MutexGuard};

/// The selection is process-wide; tests that change it take turns.
static SERIAL: Mutex<()> = Mutex::new(());

fn with(language: &str, formats: &str) -> MutexGuard<'static, ()> {
    let guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    select(Selection {
        languages: negotiate(&[language.parse().unwrap()]),
        time: formats.parse().unwrap(),
        numbers: formats.parse().unwrap(),
        pseudo: false,
    });
    guard
}

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 11).unwrap()
}

fn afternoon() -> NaiveTime {
    NaiveTime::from_hms_opt(14, 30, 5).unwrap()
}

#[test]
fn english_is_the_default_before_init() {
    let _guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    *SELECTION.write().unwrap() = None;
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    assert_eq!(selection(), Selection::default());
    assert_eq!(fl!("format-relative-now"), "now");
    assert_eq!(direction(), Direction::Ltr);
}

#[test]
fn messages_follow_the_selection_and_fall_back_to_english() {
    let _guard = with("fr-FR", "fr-FR");
    assert_eq!(fl!("format-relative-now"), "à l’instant");
    select_locale("de_DE.UTF-8", None);
    assert_eq!(fl!("format-relative-now"), "now");
    select_locale("fr_CA.UTF-8", Some("en_US.UTF-8"));
    assert_eq!(fl!("format-relative-now"), "à l’instant");
    assert_eq!(selection().time.to_string(), "en-US");
}

#[test]
fn plurals_follow_each_language() {
    let _guard = with("en", "en");
    assert_eq!(format::relative(60), "1 minute ago");
    assert_eq!(format::relative(5 * 60), "5 minutes ago");
    assert_eq!(format::relative(-2 * 3600), "in 2 hours");
    assert_eq!(format::relative(86_400), "yesterday");
    assert_eq!(format::size(1), "1 byte");
    assert_eq!(format::size(0), "0 bytes");
    drop(_guard);
    let _guard = with("fr", "fr");
    assert_eq!(format::relative(60), "il y a 1\u{a0}minute");
    assert_eq!(format::relative(5 * 60), "il y a 5\u{a0}minutes");
    assert_eq!(format::relative(3 * 86_400), "il y a 3\u{a0}jours");
    // French uses the singular for zero.
    assert_eq!(format::size(0), "0\u{a0}octet");
    assert_eq!(format::size(2), "2\u{a0}octets");
}

#[test]
fn dates_and_times_follow_the_formats_locale() {
    let _guard = with("en", "en");
    assert_eq!(format::date(date()), "Nov 11, 2026");
    assert_eq!(
        format::date_with(date(), format::DateStyle::Long),
        "November 11, 2026"
    );
    assert_eq!(format::time(afternoon()), "2:30\u{202f}PM");
    assert_eq!(
        format::time_with(afternoon(), true, format::HourCycleChoice::H24),
        "14:30:05"
    );
    assert!(!format::uses_24_hour_clock());
    assert_eq!(format::first_weekday(), Weekday::Sun);
    assert_eq!(
        format::weekday(Weekday::Mon, icu_datetime::options::Length::Medium),
        "Mon"
    );
    assert_eq!(format::month_year(date()), "November 2026");
    drop(_guard);
    let _guard = with("fr", "fr-FR");
    assert_eq!(format::date(date()), "11 nov. 2026");
    assert_eq!(
        format::date_with(date(), format::DateStyle::Full),
        "mercredi 11 novembre 2026"
    );
    assert_eq!(format::time(afternoon()), "14:30");
    assert_eq!(
        format::time_with(afternoon(), false, format::HourCycleChoice::H12),
        "2:30\u{202f}PM"
    );
    assert!(format::uses_24_hour_clock());
    assert_eq!(format::first_weekday(), Weekday::Mon);
    assert_eq!(
        format::weekday(Weekday::Mon, icu_datetime::options::Length::Long),
        "lundi"
    );
    drop(_guard);
    let _guard = with("en", "en-GB");
    assert_eq!(format::time(afternoon()), "14:30");
    assert_eq!(format::first_weekday(), Weekday::Mon);
}

#[test]
fn numbers_and_sizes_follow_the_numbers_locale() {
    let _guard = with("en", "en");
    assert_eq!(format::integer(1_234_567), "1,234,567");
    assert_eq!(format::decimal(1.26, 1, true), "1.3");
    assert_eq!(format::decimal(2.0, 1, false), "2");
    assert_eq!(format::size(1536), "1.5 KiB");
    assert_eq!(format::size(3 * 1024 * 1024 * 1024), "3.0 GiB");
    assert_eq!(format::decimal_size(512_000_000_000), "512 GB");
    assert_eq!(format::decimal_size(1_000_000_000_000), "1.0 TB");
    assert_eq!(format::percent(45.0), "45%");
    assert_eq!(format::duration(2 * 3600 + 5 * 60), "2 h 5 min");
    drop(_guard);
    let _guard = with("fr", "fr");
    assert_eq!(format::integer(1_234_567), "1\u{202f}234\u{202f}567");
    assert_eq!(format::size(1536), "1,5\u{a0}Kio");
    assert_eq!(format::decimal_size(512_000_000_000), "512\u{a0}Go");
    assert_eq!(format::percent(45.0), "45\u{202f}%");
    assert_eq!(format::duration(45 * 60), "45\u{a0}min");
}

#[test]
fn previews_format_another_locale_on_this_thread() {
    let _guard = with("en", "en");
    let fr: LanguageIdentifier = "fr-FR".parse().unwrap();
    let inside = format::with_formats(&fr, || (format::time(afternoon()), format::integer(1234)));
    assert_eq!(inside, ("14:30".to_owned(), "1\u{202f}234".to_owned()));
    assert_eq!(format::time(afternoon()), "2:30\u{202f}PM");
    assert_eq!(
        format::weekday_month_day_with(date(), icu_datetime::options::Length::Long),
        "Wednesday, November 11"
    );
    let long = format::with_formats(&fr, || {
        format::weekday_month_day_with(date(), icu_datetime::options::Length::Long)
    });
    assert_eq!(long, "mercredi 11 novembre");
}

#[test]
fn numeric_message_arguments_use_the_numbers_locale() {
    let _guard = with("fr", "fr");
    let relative = format::relative(1_500 * 86_400 * 365);
    assert_eq!(relative, "il y a 1\u{202f}500\u{a0}ans");
}

#[test]
fn the_pseudo_locale_is_accented_longer_and_right_to_left() {
    let _guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    select(Selection {
        pseudo: true,
        ..Selection::from_sources(&LocaleSources::default())
    });
    let text = fl!("format-relative-now");
    assert_ne!(text, "now");
    assert!(text.contains("ñöŵ"), "{text}");
    assert!(text.starts_with('\u{202B}') && text.ends_with('\u{202C}'));
    assert!(text.chars().count() > "now".len() + 2);
    assert_eq!(pseudo::strip(&text), "now");
    assert_eq!(direction(), Direction::Rtl);
    // Placeables are not accented.
    let minutes = format::relative(5 * 60);
    assert!(
        minutes.contains('5') && minutes.contains("ɱîñûţéš"),
        "{minutes}"
    );
    select(Selection::default());
}

#[test]
fn the_pseudo_locale_is_selected_by_environment() {
    let sources = LocaleSources {
        environment: vec![
            ("LANG".into(), "fr_FR.UTF-8".into()),
            ("CANOPA_PSEUDO_LOCALE".into(), "1".into()),
        ],
        user_file: None,
    };
    let selection = Selection::from_sources(&sources);
    assert!(selection.pseudo);
    assert_eq!(selection.direction(), Direction::Rtl);
    let off = LocaleSources {
        environment: vec![("CANOPA_PSEUDO_LOCALE".into(), "0".into())],
        user_file: None,
    };
    assert!(!Selection::from_sources(&off).pseudo);
}

#[test]
fn rtl_languages_are_recognized() {
    for tag in ["ar", "he-IL", "fa", "ur"] {
        assert!(is_rtl_language(&tag.parse().unwrap()), "{tag}");
    }
    for tag in ["en", "fr", "de", "ja"] {
        assert!(!is_rtl_language(&tag.parse().unwrap()), "{tag}");
    }
}

#[test]
fn shared_catalog_is_complete() {
    check_catalog(&i18n::LOADER).unwrap();
}
