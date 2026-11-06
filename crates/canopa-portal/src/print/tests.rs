use super::*;
use zbus::zvariant::{OwnedValue, Value};

fn strings(values: &[(&str, &str)]) -> Options {
    values
        .iter()
        .map(|(key, value)| {
            (
                (*key).into(),
                Value::from((*value).to_owned()).try_into().unwrap(),
            )
        })
        .collect()
}
fn prepare(settings: Options, options: Options) -> fdo::Result<PrepareRequest> {
    PrepareRequest::parse(
        String::new(),
        "wayland:fixture".into(),
        "Print a document".into(),
        settings,
        Options::new(),
        options,
    )
}
fn string_value(values: &Options, key: &str) -> String {
    values[key].try_clone().unwrap().try_into().unwrap()
}

#[test]
fn settings_require_string_values_and_reject_invalid_numeric_or_boolean_hints() {
    assert!(SettingsHints::parse(Options::from([("n-copies".into(), 2_u32.into())])).is_err());
    for (key, value) in [
        ("n-copies", "0"),
        ("n-copies", "-1"),
        ("scale", "NaN"),
        ("scale", "inf"),
        ("collate", "1"),
        ("number-up-layout", "unknown"),
    ] {
        assert!(SettingsHints::parse(strings(&[(key, value)])).is_err());
    }
    let values = SettingsHints::parse(strings(&[
        ("n-copies", "2"),
        ("collate", "true"),
        ("reverse", "false"),
        ("scale", "95.5"),
    ]))
    .unwrap();
    assert_eq!(values.layout.copies, 2);
    assert!(values.layout.collate);
    assert_eq!(values.layout.scale, 95.5);
}

#[test]
fn advisory_destination_and_cups_fields_cannot_escape_the_layout_projection() {
    let hints = SettingsHints::parse(strings(&[
        ("output-uri", "file:///tmp/not-selected.pdf"),
        ("printer", "not-selected"),
        ("cups-device-uri", "ipp://not-selected/ipp/print"),
        ("duplex", "vertical"),
        ("n-copies", "2"),
    ]))
    .unwrap();
    assert_eq!(
        hints.advisory("output-uri"),
        Some("file:///tmp/not-selected.pdf")
    );
    let result = hints.layout.options(PageChoices::default()).unwrap();
    for key in ["output-uri", "printer", "cups-device-uri", "duplex"] {
        assert!(!result.contains_key(key));
    }
    assert_eq!(string_value(&result, "n-copies"), "2");
}

#[test]
fn unsupported_initial_page_modes_fall_back_but_approval_cannot_grant_them() {
    for (mode, choice) in [("current", Pages::Current), ("selection", Pages::Selection)] {
        let request = prepare(strings(&[("print-pages", mode)]), Options::new()).unwrap();
        assert_eq!(request.settings.layout.pages, Pages::All);
        let mut approved = request.settings.layout;
        approved.pages = choice;
        assert!(approved.options(request.page_choices).is_err());
    }
    let request = prepare(
        strings(&[("print-pages", "current")]),
        Options::from([("has_current_page".into(), true.into())]),
    )
    .unwrap();
    assert_eq!(request.settings.layout.pages, Pages::Current);
    assert!(
        request
            .settings
            .layout
            .options(request.page_choices)
            .is_ok()
    );
}

#[test]
fn page_ranges_keep_zero_based_wire_values_and_convert_to_cups_once() {
    let hints = SettingsHints::parse(strings(&[
        ("print-pages", "ranges"),
        ("page-ranges", "0-2,4,9-11"),
    ]))
    .unwrap();
    assert_eq!(
        PageRange::cups(&hints.layout.ranges).unwrap(),
        "1-3,5,10-12"
    );
    assert_eq!(
        string_value(
            &hints.layout.options(PageChoices::default()).unwrap(),
            "page-ranges"
        ),
        "0-2,4,9-11"
    );
    for value in ["3-2", "-1", "4294967295", "1-2-3", "1,,2"] {
        assert!(PageRange::parse(value).is_err(), "{value}");
    }
    assert!(
        PageRange::cups(&[PageRange {
            first: 0,
            last: u32::MAX
        }])
        .is_err()
    );
    assert!(PageRange::parse(&vec!["0"; 129].join(",")).is_err());
}

#[test]
fn page_setup_uses_double_millimetres_and_refuses_impossible_printable_areas() {
    let page = PageSetup::parse(Options::from([
        ("Width".into(), 210_f64.into()),
        ("Height".into(), 297_f64.into()),
        ("MarginLeft".into(), 10_f64.into()),
        ("MarginRight".into(), 10_f64.into()),
        (
            "Orientation".into(),
            Value::from("reverse-landscape").try_into().unwrap(),
        ),
    ]))
    .unwrap();
    let output = page.options().unwrap();
    assert_eq!(
        f64::try_from(output["Width"].try_clone().unwrap()).unwrap(),
        210.
    );
    assert_eq!(string_value(&output, "Orientation"), "reverse-landscape");
    assert!(PageSetup::parse(strings(&[("Width", "210"), ("Height", "297")])).is_err());
    for value in [f64::NAN, f64::INFINITY, 0., -1.] {
        assert!(
            PageSetup::parse(Options::from([
                ("Width".into(), value.into()),
                ("Height".into(), 297_f64.into())
            ]))
            .is_err()
        );
    }
    let mut invalid = page;
    invalid.left = Some(210.);
    assert!(invalid.options().is_err());
}

#[test]
fn settings_and_page_setup_keep_their_distinct_orientation_spellings() {
    let settings = SettingsHints::parse(strings(&[("orientation", "reverse_landscape")])).unwrap();
    assert_eq!(
        string_value(
            &settings.layout.options(PageChoices::default()).unwrap(),
            "orientation"
        ),
        "reverse_landscape"
    );
    assert!(SettingsHints::parse(strings(&[("orientation", "reverse-landscape")])).is_err());
    assert!(PageSetup::parse(strings(&[("Orientation", "reverse_landscape")])).is_err());
}

#[test]
fn file_format_defaults_and_empty_offers_preserve_the_published_contract() {
    let request = prepare(Options::new(), Options::new()).unwrap();
    assert_eq!(request.formats, [Format::Pdf, Format::Ps, Format::Svg]);
    let request = prepare(
        Options::new(),
        Options::from([(
            "supported_output_file_formats".into(),
            Value::from(Vec::<String>::new()).try_into().unwrap(),
        )]),
    )
    .unwrap();
    assert!(request.formats.is_empty());
    assert!(
        prepare(
            Options::new(),
            Options::from([(
                "supported_output_file_formats".into(),
                Value::from(vec!["PDF"]).try_into().unwrap(),
            )])
        )
        .is_err()
    );
}

#[test]
fn request_text_and_token_types_are_bounded_without_rejecting_native_app_ids() {
    assert!(
        prepare(Options::new(), Options::new())
            .unwrap()
            .context
            .app_id
            .is_empty()
    );
    assert!(
        PrintOptions::parse(
            String::new(),
            String::new(),
            "Print".into(),
            Options::from([("token".into(), OwnedValue::from(7_u64))]),
        )
        .is_err()
    );
    assert!(
        PrepareRequest::parse(
            "a".repeat(257),
            String::new(),
            "Print".into(),
            Options::new(),
            Options::new(),
            Options::new()
        )
        .is_err()
    );
    let values: Options = (0..17)
        .map(|index| {
            (
                format!("extension-{index}"),
                Value::from("x".repeat(4096)).try_into().unwrap(),
            )
        })
        .collect();
    assert!(SettingsHints::parse(values).is_err());
    let invalid = Layout {
        scale: f64::INFINITY,
        ..Layout::default()
    };
    assert!(invalid.options(PageChoices::default()).is_err());
}
