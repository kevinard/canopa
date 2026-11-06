use super::*;
use crate::print::{
    PageRange, Pages,
    worker::{Media, PrinterState, Sides},
};

fn request() -> Request {
    Request {
        prepare: PrepareRequest::parse(
            "org.canopa.Test".into(),
            String::new(),
            "Fixture document".into(),
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .unwrap(),
        purpose: Purpose::Prepare,
        channel: None,
        cups_runtime: Default::default(),
    }
}
fn dialog(runtime: &tokio::runtime::Runtime) -> Dialog {
    let (_, cancelled) = watch::channel(false);
    Dialog::new(
        request(),
        Theme::Dark,
        Arc::default(),
        Client::new(DialogProcess {
            executable: "/fixture-only/not-executable".into(),
            arguments: vec![],
        }),
        runtime.handle().clone(),
        cancelled,
        crate::print::authentication::Broker::new(
            DialogProcess {
                executable: "/fixture-only/not-executable".into(),
                arguments: vec![],
            },
            request().prepare.context,
        ),
    )
}
fn file(dialog: &mut Dialog) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let reviewed = Arc::new(
        crate::print::file_target::Reviewed::open(&directory.path().join("approved.pdf")).unwrap(),
    );
    dialog.target = Some(DestinationChoice::File);
    dialog.draft.path = Some(reviewed.selection().path().to_vec());
    dialog.draft.reviewed = Some(reviewed);
    directory
}
fn destination() -> Destination {
    Destination {
        queue: "Office".into(),
        instance: None,
    }
}
fn caps() -> Capabilities {
    Capabilities {
        destination: destination(),
        copies: Some((1, 3)),
        media: vec![],
        default_media: None,
        custom_media: false,
        default_custom_media: None,
        sides: vec![Sides::OneSided],
        default_sides: Some(Sides::OneSided),
        colors: vec![false],
        default_color: Some(false),
        formats: vec![Format::Pdf.mime().into()],
        controls: Default::default(),
        preferences: None,
        authentication: Default::default(),
    }
}
fn key(key: keyboard::key::Named) -> Message {
    Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(key),
        modified_key: keyboard::Key::Named(key),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::default(),
        text: None,
        repeat: false,
    }))
}

/// Deliver a successful current worker response to the dialog model. Native
/// worker checks separately establish device support and driver constraints.
fn confirm(dialog: &mut Dialog) {
    let (destination, mut options, size) = dialog.check_request().unwrap();
    if let Some(size) = size {
        let paper = custom_paper();
        assert!(size.matches(&paper));
        options.custom_media = Some(paper);
    }
    dialog.running = None;
    dialog.loading = true;
    let _ = dialog.update(Message::Metadata(
        dialog.generation,
        Ok(Metadata::CheckedPrinter(destination, options)),
    ));
}
fn checked_paper(dialog: &Dialog) -> JobOptions {
    JobOptions {
        copies: 1,
        custom_media: Some(custom_paper()),
        ..dialog.draft.device.clone()
    }
}

#[test]
fn only_explicit_current_approval_completes_and_invalid_edits_revoke_it() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    let _directory = file(&mut dialog);
    assert!(dialog.grant().is_some());
    let _ = dialog.update(key(keyboard::key::Named::Enter));
    assert!(dialog.output.lock().unwrap().is_none());
    let old = dialog.generation;
    let _ = dialog.update(Message::Edit(draft::Edit::Copies("0".into())));
    assert!(dialog.grant().is_none());
    let _ = dialog.update(Message::Approve(old));
    assert!(!dialog.completed);
    let _ = dialog.update(Message::Edit(draft::Edit::Copies("2".into())));
    let _ = dialog.update(Message::Approve(dialog.generation));
    assert!(dialog.completed);
    let approved = dialog.output.lock().unwrap();
    let grant = &approved.as_ref().unwrap().grant;
    assert_eq!(grant.layout.copies, 2);
    assert_eq!(grant.device.copies, 2);
    assert!(
        grant.device.media.is_none()
            && grant.device.sides.is_none()
            && grant.device.color.is_none()
    );
}

#[test]
fn cancelled_dialog_cannot_be_revived_by_metadata_or_approval() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    let _directory = file(&mut dialog);
    dialog.loading = true;
    let _ = dialog.update(Message::Cancel);
    let _ = dialog.update(Message::Metadata(
        dialog.generation,
        Ok(Metadata::Capabilities(caps())),
    ));
    let _ = dialog.update(Message::Approve(dialog.generation));
    assert!(dialog.output.lock().unwrap().is_none());
}

#[test]
fn late_metadata_and_other_destination_cannot_replace_selected_capabilities() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    dialog.target = Some(DestinationChoice::Printer(destination()));
    dialog.generation = 4;
    dialog.loading = true;
    let _ = dialog.update(Message::Metadata(3, Ok(Metadata::Capabilities(caps()))));
    assert!(dialog.loading && dialog.capabilities.is_none());
    let mut wrong = caps();
    wrong.destination.queue = "Other".into();
    let _ = dialog.update(Message::Metadata(4, Ok(Metadata::Capabilities(wrong))));
    assert!(dialog.capabilities.is_none());
    dialog.loading = true;
    let _ = dialog.update(Message::Metadata(4, Ok(Metadata::Capabilities(caps()))));
    assert_eq!(
        dialog.capabilities.as_ref().unwrap().destination,
        destination()
    );
}

#[test]
fn printer_grant_requires_current_capabilities_and_an_accepting_queue() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    dialog.target = Some(DestinationChoice::Printer(destination()));
    assert!(dialog.grant().is_none());
    dialog.capabilities = Some(caps());
    dialog.printers.push(Printer {
        destination: destination(),
        label: "Office".into(),
        location: String::new(),
        accepting: false,
        is_default: true,
        state: PrinterState::Idle,
    });
    assert!(dialog.grant().is_none());
    dialog.printers[0].accepting = true;
    assert!(dialog.grant().is_none());
    confirm(&mut dialog);
    assert!(dialog.grant().is_some());
    let _ = dialog.update(Message::Edit(draft::Edit::Copies("4".into())));
    assert!(dialog.grant().is_none());
    confirm(&mut dialog);
    let _ = dialog.update(Message::Edit(draft::Edit::Copies("2".into())));
    confirm(&mut dialog);
    let _ = dialog.update(Message::Edit(draft::Edit::Sides(Choice {
        value: Some(Sides::LongEdge),
        label: String::new(),
    })));
    assert!(dialog.grant().is_none());
}

#[test]
fn device_reply_must_match_current_options_even_at_the_same_generation() {
    use crate::print::worker::{Control, SettingChoice, SettingField};
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    dialog.target = Some(DestinationChoice::Printer(destination()));
    let mut caps = caps();
    caps.controls.source = Control {
        choices: ["tray-1", "manual"]
            .map(|v| SettingChoice {
                value: v.into(),
                label: v.into(),
            })
            .into(),
        default: Some("tray-1".into()),
    };
    dialog.draft.install(&caps);
    dialog.capabilities = Some(caps);
    let old = dialog.draft.device.clone();
    let _ = dialog.update(Message::Edit(draft::Edit::Setting(
        SettingField::Source,
        Choice {
            value: Some("manual".into()),
            label: "Manual".into(),
        },
    )));
    let _ = dialog.update(Message::Metadata(
        dialog.generation,
        Ok(Metadata::CheckedPrinter(destination(), old)),
    ));
    assert!(dialog.checked.is_none() && dialog.error.is_some());
    confirm(&mut dialog);
    assert_eq!(
        dialog.checked.as_ref().unwrap().settings.source.as_deref(),
        Some("manual")
    );
    let _ = dialog.update(Message::Edit(draft::Edit::Setting(
        SettingField::Source,
        Choice {
            value: None,
            label: "Printer default".into(),
        },
    )));
    assert!(dialog.checked.is_none());
    confirm(&mut dialog);
    assert_eq!(
        dialog.checked.as_ref().unwrap().settings.source.as_deref(),
        Some("tray-1")
    );
}

#[test]
fn multiple_printer_copies_capture_a_known_side_setting() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    dialog.target = Some(DestinationChoice::Printer(destination()));
    let mut capabilities = caps();
    capabilities.default_sides = None;
    dialog.capabilities = Some(capabilities);
    dialog.printers.push(Printer {
        destination: destination(),
        label: "Office".into(),
        location: String::new(),
        accepting: true,
        is_default: true,
        state: PrinterState::Idle,
    });
    confirm(&mut dialog);
    assert!(dialog.grant().is_some());
    let _ = dialog.update(Message::Edit(draft::Edit::Copies("2".into())));
    assert!(dialog.grant().is_none());
    dialog.capabilities.as_mut().unwrap().default_sides = Some(Sides::OneSided);
    confirm(&mut dialog);
    assert_eq!(dialog.grant().unwrap().device.sides, Some(Sides::OneSided));
    dialog.capabilities.as_mut().unwrap().default_sides = None;
    let _ = dialog.update(Message::Edit(draft::Edit::Sides(Choice {
        value: Some(Sides::OneSided),
        label: "One-sided".into(),
    })));
    confirm(&mut dialog);
    assert_eq!(dialog.grant().unwrap().device.sides, Some(Sides::OneSided));
}

#[test]
fn display_ranges_convert_once_and_unsupported_page_modes_never_grant() {
    let request = request();
    let mut draft = Draft::new(&request);
    draft.layout.pages = Pages::Ranges;
    draft.ranges = "1-3, 5".into();
    let (layout, _, _) = draft.values(&request, true).unwrap();
    assert_eq!(
        layout.ranges,
        [
            PageRange { first: 0, last: 2 },
            PageRange { first: 4, last: 4 }
        ]
    );
    assert_eq!(PageRange::cups(&layout.ranges).unwrap(), "1-3,5");
    draft.ranges = "0-3".into();
    assert!(draft.values(&request, true).is_err());
    draft.layout.pages = Pages::Current;
    assert!(draft.values(&request, true).is_err());
    draft.layout.pages = Pages::Selection;
    assert!(draft.values(&request, true).is_err());
    draft.layout.pages = Pages::All;
    draft.scale = "NaN".into();
    assert!(draft.values(&request, true).is_err());
}

#[test]
fn file_output_needs_picker_path_and_format_change_discards_previous_path() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    dialog.target = Some(DestinationChoice::File);
    assert!(dialog.grant().is_none());
    let _directory = file(&mut dialog);
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let path = _directory
        .path()
        .join(std::ffi::OsString::from_vec(b"non-UTF8-\xff.pdf".to_vec()));
    let bytes = path.as_os_str().as_bytes().to_vec();
    dialog.draft.path = Some(bytes.clone());
    dialog.draft.reviewed = Some(Arc::new(
        crate::print::file_target::Reviewed::open(&path).unwrap(),
    ));
    let retired = Arc::downgrade(dialog.draft.reviewed.as_ref().unwrap());
    let encoded = serde_json::to_vec(&dialog.grant().unwrap()).unwrap();
    let grant: Grant = serde_json::from_slice(&encoded).unwrap();
    assert!(matches!(grant.target, Target::File { path, .. } if path == bytes));
    let _ = dialog.update(Message::Edit(draft::Edit::Format(Choice {
        value: Format::Ps,
        label: String::new(),
    })));
    assert!(dialog.grant().is_none() && dialog.draft.path.is_none());
    assert!(retired.upgrade().is_none());
    let _directory = file(&mut dialog);
    dialog.draft.geometry[0] = "0".into();
    assert!(dialog.grant().is_none());
}

#[test]
fn direct_known_documents_offer_supported_conversion_formats() {
    let mut request = request();
    request.purpose = Purpose::Direct {
        format: Some(Format::Ps),
    };
    assert_eq!(request.formats(), request.prepare.formats);
    request.prepare.formats = vec![Format::Pdf];
    assert_eq!(request.formats(), [Format::Pdf]);
    request.purpose = Purpose::Direct { format: None };
    assert!(request.formats().is_empty());
}

#[test]
fn custom_file_geometry_has_a_serializable_paper_identity_and_retires_old_ppd_aliases() {
    let mut request = request();
    request.prepare.page_setup.ppd_name = Some("Letter".into());
    let mut draft = Draft::new(&request);
    draft.edit(draft::Edit::Geometry(0, "120".into()));
    draft.edit(draft::Edit::Geometry(1, "200".into()));
    let (_, setup, _) = draft.values(&request, true).unwrap();
    assert!(setup.ppd_name.is_none());
    assert_eq!(setup.name.as_deref(), Some("canopa-custom"));
    assert_eq!((setup.width, setup.height), (Some(120.), Some(200.)));
    let wire = setup.options().unwrap();
    assert!(wire.contains_key("Name") && wire.contains_key("DisplayName"));
    assert!(!wire.contains_key("PPDName"));
}

fn custom_paper() -> Media {
    Media {
        id: "custom_canopa_125x225mm".into(),
        width_mm: 125.,
        height_mm: 225.,
        top_mm: 3.18,
        bottom_mm: 12.7,
        left_mm: 3.18,
        right_mm: 3.18,
    }
}

#[test]
fn saved_custom_paper_preserves_concrete_geometry_and_edits_revoke_confirmation() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    dialog.target = Some(DestinationChoice::Printer(destination()));
    dialog.printers.push(Printer {
        destination: destination(),
        label: "Office".into(),
        location: String::new(),
        accepting: true,
        is_default: true,
        state: PrinterState::Idle,
    });
    let caps = Capabilities {
        custom_media: true,
        default_custom_media: Some(custom_paper()),
        media: vec![Media {
            id: "iso_a4_210x297mm".into(),
            width_mm: 210.,
            height_mm: 297.,
            top_mm: 0.,
            bottom_mm: 0.,
            left_mm: 0.,
            right_mm: 0.,
        }],
        ..caps()
    };
    dialog.draft.install(&caps);
    dialog.capabilities = Some(caps);
    assert!(dialog.grant().is_none());
    confirm(&mut dialog);
    let grant = dialog.grant().unwrap();
    assert_eq!(grant.page_setup.width, Some(125.));
    assert_eq!(grant.page_setup.bottom, Some(12.7));
    assert!(grant.page_setup.ppd_name.is_none());
    assert!(grant.device.media.is_none());
    let _ = dialog.update(Message::Edit(draft::Edit::CustomDimension(
        0,
        "125.001".into(),
    )));
    assert!(dialog.grant().is_none() && dialog.custom_size().is_none());
    let _ = dialog.update(Message::Edit(draft::Edit::CustomDimension(0, "125".into())));
    assert!(dialog.grant().is_none());
    dialog.loading = true;
    let _ = dialog.update(Message::Metadata(
        dialog.generation,
        Ok(Metadata::CheckedPrinter(
            destination(),
            checked_paper(&dialog),
        )),
    ));
    assert!(dialog.grant().is_some());
    let _ = dialog.update(Message::Edit(draft::Edit::Sides(Choice {
        value: Some(Sides::OneSided),
        label: String::new(),
    })));
    assert!(dialog.grant().is_none());
    let _ = dialog.update(Message::Edit(draft::Edit::Custom(false)));
    confirm(&mut dialog);
    assert!(
        dialog
            .grant()
            .is_some_and(|grant| grant.device.custom_media.is_none())
    );
}

#[test]
fn custom_paper_reply_cannot_confirm_a_later_size_or_other_printer() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    dialog.target = Some(DestinationChoice::Printer(destination()));
    dialog.capabilities = Some(Capabilities {
        custom_media: true,
        ..caps()
    });
    dialog.draft.edit(draft::Edit::Custom(true));
    dialog.draft.custom_dimensions = ["125".into(), "225".into()];
    dialog.generation = 5;
    dialog.loading = true;
    let _ = dialog.update(Message::Metadata(
        4,
        Ok(Metadata::CheckedPrinter(
            destination(),
            checked_paper(&dialog),
        )),
    ));
    assert!(dialog.loading && dialog.draft.device.custom_media.is_none());
    let other = Destination {
        queue: "Other".into(),
        instance: None,
    };
    let _ = dialog.update(Message::Metadata(
        5,
        Ok(Metadata::CheckedPrinter(other, checked_paper(&dialog))),
    ));
    assert!(dialog.draft.device.custom_media.is_none());
    dialog.loading = true;
    dialog.draft.custom_dimensions[0] = "126".into();
    let _ = dialog.update(Message::Metadata(
        5,
        Ok(Metadata::CheckedPrinter(
            destination(),
            checked_paper(&dialog),
        )),
    ));
    assert!(dialog.draft.device.custom_media.is_none());
}

#[test]
fn choosing_printer_default_restores_saved_custom_size_and_rechecks_changed_sides() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    dialog.target = Some(DestinationChoice::Printer(destination()));
    dialog.printers.push(Printer {
        destination: destination(),
        label: "Office".into(),
        location: String::new(),
        accepting: true,
        is_default: true,
        state: PrinterState::Idle,
    });
    let caps = Capabilities {
        custom_media: true,
        default_custom_media: Some(custom_paper()),
        media: vec![Media {
            id: "iso_a4_210x297mm".into(),
            width_mm: 210.,
            height_mm: 297.,
            top_mm: 0.,
            bottom_mm: 0.,
            left_mm: 0.,
            right_mm: 0.,
        }],
        sides: vec![Sides::OneSided, Sides::LongEdge],
        ..caps()
    };
    dialog.draft.install(&caps);
    dialog.capabilities = Some(caps);
    let _ = dialog.update(Message::Edit(draft::Edit::Custom(false)));
    assert!(!dialog.draft.custom);
    confirm(&mut dialog);
    let fixed = dialog.grant().unwrap();
    assert_eq!(fixed.device.media.as_deref(), Some("iso_a4_210x297mm"));
    assert_eq!(fixed.page_setup.width, Some(210.));
    let _ = dialog.update(Message::Edit(draft::Edit::Media(Choice {
        value: None,
        label: "Printer default".into(),
    })));
    assert!(dialog.draft.custom);
    confirm(&mut dialog);
    assert_eq!(dialog.grant().unwrap().page_setup.width, Some(125.));
    let _ = dialog.update(Message::Edit(draft::Edit::Custom(false)));
    confirm(&mut dialog);
    let _ = dialog.update(Message::Edit(draft::Edit::Sides(Choice {
        value: Some(Sides::LongEdge),
        label: String::new(),
    })));
    confirm(&mut dialog);
    let _ = dialog.update(Message::Edit(draft::Edit::Media(Choice {
        value: None,
        label: "Printer default".into(),
    })));
    assert!(dialog.draft.custom && dialog.grant().is_none());
    assert_eq!(dialog.draft.custom_dimensions, ["125", "225"]);
}

fn initial_settings(dialog: &mut Dialog, values: &[(&str, &str)]) {
    use zbus::zvariant::Value;
    let settings = values
        .iter()
        .map(|(k, v)| ((*k).into(), Value::from(*v).try_into_owned().unwrap()))
        .collect();
    dialog.request.prepare.settings = crate::print::SettingsHints::parse(settings).unwrap();
}
fn available() -> Printer {
    Printer {
        destination: destination(),
        label: "Office".into(),
        location: String::new(),
        is_default: true,
        accepting: true,
        state: PrinterState::Idle,
    }
}
#[test]
fn unavailable_remembered_printer_does_not_redirect_to_default() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    initial_settings(&mut dialog, &[("printer", "Missing")]);
    dialog.loading = true;
    let _ = dialog.update(Message::Metadata(
        dialog.generation,
        Ok(Metadata::Printers(vec![available()])),
    ));
    assert!(dialog.target.is_none() && dialog.notice.is_some() && !dialog.loading);
    assert!(dialog.grant().is_none());
}
#[test]
fn exact_remembered_instance_is_selected_and_file_preference_needs_fresh_picker() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    initial_settings(&mut dialog, &[("printer", "Office/Saved")]);
    let mut saved = available();
    saved.destination.instance = Some("Saved".into());
    saved.is_default = false;
    dialog.loading = true;
    let _ = dialog.update(Message::Metadata(
        dialog.generation,
        Ok(Metadata::Printers(vec![available(), saved.clone()])),
    ));
    assert_eq!(
        dialog.target,
        Some(DestinationChoice::Printer(saved.destination))
    );
    dialog.running = None;
    initial_settings(
        &mut dialog,
        &[
            ("printer", "Print to File"),
            ("output-uri", "file:///never-approved"),
        ],
    );
    dialog.loading = true;
    let _ = dialog.update(Message::Metadata(
        dialog.generation,
        Ok(Metadata::Printers(vec![available()])),
    ));
    assert_eq!(dialog.target, Some(DestinationChoice::File));
    assert!(
        dialog.draft.path.is_none() && dialog.draft.reviewed.is_none() && dialog.grant().is_none()
    );
}
#[test]
fn initial_preferences_are_checked_once_and_do_not_replay_after_user_edits() {
    use crate::print::worker::PreferenceSelection;
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut dialog = dialog(&runtime);
    dialog.target = Some(DestinationChoice::Printer(destination()));
    dialog.printers.push(available());
    let mut caps = caps();
    caps.colors.push(true);
    caps.preferences = Some(Box::new(PreferenceSelection {
        options: JobOptions {
            sides: Some(Sides::OneSided),
            color: Some(true),
            ..Default::default()
        },
        custom_size: None,
        unavailable: true,
    }));
    dialog.loading = true;
    let _ = dialog.update(Message::Metadata(
        dialog.generation,
        Ok(Metadata::Capabilities(caps.clone())),
    ));
    assert_eq!(dialog.draft.device.color, Some(true));
    assert!(
        dialog.checked.is_none()
            && dialog.grant().is_none()
            && !dialog.initial
            && dialog.notice.is_some()
    );
    confirm(&mut dialog);
    assert!(dialog.grant().is_some() && dialog.notice.is_some());
    let _ = dialog.update(Message::Edit(draft::Edit::Color(Choice {
        value: Some(false),
        label: "Monochrome".into(),
    })));
    confirm(&mut dialog);
    assert_eq!(dialog.grant().unwrap().device.color, Some(false));
    // Refresh resets queue defaults and consumes old app preferences. A later
    // response carrying those preferences cannot install them again.
    let _ = dialog.update(Message::Refresh);
    dialog.target = Some(DestinationChoice::Printer(destination()));
    dialog.loading = true;
    let _ = dialog.update(Message::Metadata(
        dialog.generation,
        Ok(Metadata::Capabilities(caps)),
    ));
    assert_eq!(dialog.draft.device.color, Some(false));
}
#[test]
fn requested_file_format_must_be_currently_offered() {
    let mut request = request();
    use zbus::zvariant::Value;
    request.prepare.settings = crate::print::SettingsHints::parse(
        [(
            "output-file-format".into(),
            Value::from("ps").try_into_owned().unwrap(),
        )]
        .into(),
    )
    .unwrap();
    assert_eq!(Draft::new(&request).format, Some(Format::Ps));
    request.prepare.formats = vec![Format::Pdf];
    assert_eq!(Draft::new(&request).format, Some(Format::Pdf));
}
