use super::*;
use crate::{
    print::worker::{Control, Controls, Destination, Media, SettingChoice},
    requests::Options,
};
use zbus::zvariant::Value;

fn prefs(values: &[(&str, &str)], page_setup: PageSetup) -> Preferences {
    let values: Options = values
        .iter()
        .map(|(k, v)| ((*k).into(), Value::from(*v).try_into_owned().unwrap()))
        .collect();
    Preferences::new(&SettingsHints::parse(values).unwrap(), &page_setup)
}
fn caps() -> Capabilities {
    fn control(values: &[&str]) -> Control {
        Control {
            choices: values
                .iter()
                .map(|v| SettingChoice {
                    value: (*v).into(),
                    label: (*v).into(),
                })
                .collect(),
            default: values.first().map(|v| (*v).into()),
        }
    }
    Capabilities {
        destination: Destination {
            queue: "Office".into(),
            instance: None,
        },
        copies: Some((1, 9999)),
        media: vec![Media {
            id: "iso_a4_210x297mm".into(),
            width_mm: 210.,
            height_mm: 297.,
            top_mm: 3.,
            bottom_mm: 4.,
            left_mm: 5.,
            right_mm: 6.,
        }],
        default_media: Some("iso_a4_210x297mm".into()),
        custom_media: true,
        default_custom_media: None,
        sides: vec![Sides::OneSided, Sides::LongEdge, Sides::ShortEdge],
        default_sides: Some(Sides::OneSided),
        colors: vec![false, true],
        default_color: Some(false),
        formats: vec!["application/pdf".into()],
        controls: Box::new(Controls {
            source: control(&["tray-1", "manual"]),
            paper_type: control(&["stationery", "labels"]),
            quality: control(&["3", "4", "5"]),
            resolution: control(&["300dpi", "600x1200dpi", "118x236dpcm", "600dpi"]),
        }),
        preferences: None,
        authentication: Default::default(),
    }
}
fn resolve(values: &[(&str, &str)]) -> PreferenceSelection {
    prefs(values, PageSetup::default()).resolve(&caps(), &[], &[])
}

#[test]
fn restores_supported_application_controls_without_changing_queue_defaults() {
    let caps = caps();
    let aliases = [
        (Field::Source, "manual".into(), "Manual".into()),
        (Field::Type, "labels".into(), "Labels".into()),
    ];
    let p = prefs(
        &[
            ("default-source", "Manual"),
            ("media-type", "Labels"),
            ("quality", "high"),
            ("resolution", "600"),
            ("resolution-x", "600"),
            ("resolution-y", "1200"),
            ("duplex", "horizontal"),
            ("use-color", "true"),
        ],
        PageSetup::default(),
    );
    let selected = p.resolve(&caps, &aliases, &[]);
    assert!(!selected.unavailable);
    assert_eq!(selected.options.settings.source.as_deref(), Some("manual"));
    assert_eq!(
        selected.options.settings.paper_type.as_deref(),
        Some("labels")
    );
    assert_eq!(selected.options.settings.quality.as_deref(), Some("5"));
    assert_eq!(
        selected.options.settings.resolution.as_deref(),
        Some("600x1200dpi")
    );
    assert_eq!(selected.options.sides, Some(Sides::ShortEdge));
    assert_eq!(selected.options.color, Some(true));
    assert_eq!(selected.options.copies, 1);
    assert!(caps.accepts(&selected.options));
    assert_eq!(caps.controls.source.default.as_deref(), Some("tray-1"));
}
#[test]
fn conflicting_and_unavailable_preferences_fall_back_per_field() {
    for values in [
        vec![("default-source", "manual"), ("media-source", "tray-1")],
        vec![("default-source", "unknown")],
        vec![("default-source", "manual,labels")],
    ] {
        let mut values = values;
        values.push(("quality", "high"));
        let selected = resolve(&values);
        assert!(selected.unavailable);
        assert_eq!(selected.options.settings.source.as_deref(), Some("tray-1"));
        assert_eq!(selected.options.settings.quality.as_deref(), Some("5"));
    }
    let mut caps = caps();
    caps.colors = vec![false];
    let selected = prefs(
        &[
            ("use-color", "true"),
            ("duplex", "vertical"),
            ("sides", "one-sided"),
        ],
        PageSetup::default(),
    )
    .resolve(&caps, &[], &[]);
    assert!(
        selected.unavailable
            && selected.options.color == Some(false)
            && selected.options.sides == Some(Sides::OneSided)
    );
}
#[test]
fn exact_dpcm_round_trips_but_rounded_dpi_cannot_select_dpcm() {
    let s = resolve(&[
        ("canopa-printer-resolution", "118x236dpcm"),
        ("resolution", "300"),
        ("resolution-x", "300"),
        ("resolution-y", "600"),
    ]);
    assert!(!s.unavailable);
    assert_eq!(
        s.options.settings.resolution.as_deref(),
        Some("118x236dpcm")
    );
    for values in [
        vec![("resolution-x", "300"), ("resolution-y", "600")],
        vec![("resolution-x", "600")],
        vec![
            ("resolution", "300"),
            ("resolution-x", "600"),
            ("resolution-y", "1200"),
        ],
        vec![
            ("canopa-printer-resolution", "118x236dpcm"),
            ("resolution", "600"),
        ],
    ] {
        assert!(resolve(&values).unavailable, "{values:?}");
    }
    let s = resolve(&[("printer-resolution", "600x600dpi"), ("resolution", "600")]);
    assert!(!s.unavailable);
    assert_eq!(s.options.settings.resolution.as_deref(), Some("600dpi"));
}
#[test]
fn printer_paper_uses_current_geometry_and_common_pwg_ppd_names() {
    let setup = PageSetup {
        ppd_name: Some("A4".into()),
        width: Some(210.),
        height: Some(297.),
        top: Some(20.),
        ..Default::default()
    };
    let selected = prefs(&[("paper-format", "iso_a4")], setup).resolve(
        &caps(),
        &[],
        &[("A4".into(), "iso_a4_210x297mm".into())],
    );
    assert!(!selected.unavailable && selected.custom_size.is_none());
    assert_eq!(selected.options.media.as_deref(), Some("iso_a4_210x297mm"));
    assert!(selected.options.custom_media.is_none());
    // Hints cannot inject margins or printable geometry into device options.
    assert_eq!(caps().media[0].top_mm, 3.);
}
#[test]
fn custom_dimensions_are_unconfirmed_and_inconsistent_paper_hints_fall_back() {
    let setup = PageSetup {
        name: Some("custom_own".into()),
        width: Some(125.),
        height: Some(225.),
        ..Default::default()
    };
    let selected = prefs(
        &[
            ("paper-format", "custom-custom_own"),
            ("paper-width", "125"),
            ("paper-height", "225"),
        ],
        setup,
    )
    .resolve(&caps(), &[], &[]);
    assert!(
        !selected.unavailable
            && selected.options.media.is_none()
            && selected.options.custom_media.is_none()
    );
    assert_eq!(
        selected.custom_size,
        Some(CustomSize {
            width: 12500,
            height: 22500
        })
    );
    for setup in [
        PageSetup {
            name: Some("iso_a4".into()),
            width: Some(125.),
            height: Some(225.),
            ..Default::default()
        },
        PageSetup {
            name: Some("missing".into()),
            ..Default::default()
        },
    ] {
        let selected = prefs(&[], setup).resolve(&caps(), &[], &[]);
        assert!(selected.unavailable && selected.custom_size.is_none());
    }
}
#[test]
fn ambiguous_aliases_and_equal_size_variants_are_not_guessed() {
    let mut caps = caps();
    let aliases = [
        (Field::Source, "manual".into(), "Same".into()),
        (Field::Source, "tray-1".into(), "Same".into()),
    ];
    assert!(
        prefs(&[("default-source", "Same")], PageSetup::default())
            .resolve(&caps, &aliases, &[])
            .unavailable
    );
    caps.media.push(Media {
        id: "iso_a4_210x297mm_borderless".into(),
        ..caps.media[0].clone()
    });
    assert!(
        prefs(&[("paper-format", "iso_a4")], PageSetup::default())
            .resolve(&caps, &[], &[])
            .unavailable
    );
}
#[test]
fn only_whitelisted_hints_enter_worker_protocol() {
    let p = prefs(
        &[
            ("printer", "elsewhere"),
            ("output-uri", "file:///unapproved"),
            ("cups-job-hold-until", "indefinite"),
            ("quality", "high"),
        ],
        PageSetup::default(),
    );
    assert_eq!(p.values.len(), 1);
    p.validate().unwrap();
    let mut hostile = p;
    hostile
        .values
        .insert("output-uri".into(), "file:///bad".into());
    assert!(hostile.validate().is_err());
}
