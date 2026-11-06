use super::*;

fn item(status: RecoveryStatus) -> RecoveryItem {
    RecoveryItem {
        id: "0123456789abcdef0123456789abcdef".into(),
        destination: b"/home/fixture/report.dat".to_vec(),
        directory: b"/home/fixture/.canopa-print-0123456789abcdef0123456789abcdef".to_vec(),
        status,
        document: true,
        displaced: true,
    }
}
fn app(runtime: &tokio::runtime::Runtime) -> Recovery {
    let client = Client::new(DialogProcess {
        executable: "/fixture/not-executable".into(),
        arguments: vec![],
    });
    let (_, cancelled) = watch::channel(false);
    let mut app = Recovery::new(Theme::Dark, client, runtime.handle().clone(), cancelled);
    let item = item(RecoveryStatus::Uncertain);
    app.selected = Some(item.id.clone());
    app.items = vec![item];
    app
}

#[test]
fn active_missing_and_uncheckable_work_never_offer_a_copy() {
    for status in [
        RecoveryStatus::Busy,
        RecoveryStatus::Missing,
        RecoveryStatus::Unavailable,
    ] {
        let item = item(status);
        assert!(!available(&item, RecoveryBody::Document));
        assert!(!available(&item, RecoveryBody::Displaced));
    }
    assert!(!dismissible(&item(RecoveryStatus::Busy)));
    assert!(dismissible(&item(RecoveryStatus::Unavailable)));
    assert!(dismissible(&item(RecoveryStatus::Missing)));
}

#[test]
fn dismissal_needs_confirmation_and_keep_does_not_remove_the_work() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut app = app(&runtime);
    let _ = app.update(Message::AskDismiss);
    assert!(matches!(app.screen, Screen::Dismiss));
    assert_eq!(app.items.len(), 1);
    let _ = app.update(Message::Keep);
    assert!(matches!(app.screen, Screen::List));
    assert_eq!(app.items.len(), 1);
    app.items[0].status = RecoveryStatus::Busy;
    let _ = app.update(Message::AskDismiss);
    assert!(matches!(app.screen, Screen::List));
}

#[test]
fn stale_results_do_not_report_a_save_or_revive_a_closed_review() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut app = app(&runtime);
    app.generation = 4;
    app.screen = Screen::Saving;
    app.busy = true;
    let _ = app.update(Message::Saved(3, b"/fixture/copy".to_vec(), Ok(())));
    assert!(app.notice.is_none() && matches!(app.screen, Screen::Saving));
    let _ = app.update(Message::Close);
    let _ = app.update(Message::Listed(4, Ok(vec![])));
    assert!(app.closed);
    assert_eq!(app.items.len(), 1);
}

#[test]
fn back_cannot_hide_an_in_progress_publication() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut app = app(&runtime);
    app.screen = Screen::Saving;
    app.busy = true;
    let generation = app.generation;
    let _ = app.update(Message::Back);
    assert!(app.busy && matches!(app.screen, Screen::Saving));
    assert_eq!(app.generation, generation);
}

#[test]
fn a_failed_refresh_disables_stale_copy_actions_and_a_failed_snapshot_retires_its_candidate() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut app = app(&runtime);
    app.busy = true;
    let _ = app.update(Message::Listed(0, Err("unavailable".into())));
    assert_eq!(app.items[0].status, RecoveryStatus::Unavailable);
    assert!(!available(&app.items[0], RecoveryBody::Document));
    app.items[0] = item(RecoveryStatus::Uncertain);
    app.selected = Some(app.items[0].id.clone());
    app.screen = Screen::Preparing;
    app.busy = true;
    let _ = app.update(Message::Snapshot(
        0,
        RecoveryBody::Document,
        Err("changed".into()),
    ));
    assert!(!app.items[0].document && app.items[0].displaced);
    assert!(app.error.is_some() && !app.busy && matches!(app.screen, Screen::List));
}

#[test]
fn recovery_suggestions_keep_opaque_extensions_and_never_default_to_the_old_name() {
    let mut item = item(RecoveryStatus::Uncertain);
    item.destination = b"/fixture/old\nname.opaque".to_vec();
    assert_eq!(
        suggested(&item, RecoveryBody::Document),
        "Recovered-old_name.opaque"
    );
    assert_eq!(
        suggested(&item, RecoveryBody::Displaced),
        "Displaced-old_name.opaque"
    );
}

#[test]
fn picker_handoff_keeps_the_reviewed_destination_and_refuses_a_late_occupant() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("copy.dat");
    let reviewed = Arc::new(Reviewed::open(&path).unwrap());
    let pin = handoff(&reviewed).unwrap();
    assert_eq!(pin.selection(), reviewed.selection());
    std::fs::write(&path, b"late occupant").unwrap();
    assert!(pin.verify().is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"late occupant");
}

fn printer(status: PrinterRecoveryStatus) -> PrinterRecoveryItem {
    PrinterRecoveryItem {
        id: "a".repeat(32),
        title: "Quarterly report".into(),
        destination: crate::print::worker::Destination {
            queue: "Office".into(),
            instance: None,
        },
        server: "/fixture/cups.sock".into(),
        job: Some(7),
        status,
        revision: [1; 32],
    }
}

#[test]
fn printer_only_review_opens_the_printer_tab_even_when_file_catalog_loading_fails() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut app = app(&runtime);
    app.items.clear();
    app.selected = None;
    app.busy = true;
    let _ = app.update(Message::Refreshed(
        0,
        Listing {
            files: Err("bad file catalog".into()),
            printers: Ok(vec![printer(PrinterRecoveryStatus::Unconfirmed)]),
        },
    ));
    assert_eq!(app.tab, Tab::Printers);
    assert!(app.file_load_error.is_some());
    assert!(app.printer().unwrap().dismissible());
    assert!(app.editable());
    assert!(app.item().is_none());
}

#[test]
fn printer_removal_requires_its_own_confirmation_and_keep_preserves_the_record() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut app = app(&runtime);
    app.tab = Tab::Printers;
    app.printers = vec![printer(PrinterRecoveryStatus::Unconfirmed)];
    app.selected_printer = Some(app.printers[0].id.clone());
    let _ = app.update(Message::DismissPrinter);
    assert!(matches!(app.screen, Screen::List));
    let _ = app.update(Message::AskDismiss);
    assert!(matches!(app.screen, Screen::List));
    let _ = app.update(Message::AskPrinterDismiss);
    assert!(matches!(app.screen, Screen::PrinterDismiss));
    let _ = app.update(Message::Keep);
    assert!(matches!(app.screen, Screen::List));
    assert_eq!(app.printers.len(), 1);
    for status in [
        PrinterRecoveryStatus::Busy,
        PrinterRecoveryStatus::Unavailable,
    ] {
        app.printers[0].status = status;
        let _ = app.update(Message::AskPrinterDismiss);
        assert!(matches!(app.screen, Screen::List));
    }
}

#[test]
fn failed_printer_refresh_disables_stale_removal_and_leaves_file_copies_available() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut app = app(&runtime);
    app.printers = vec![printer(PrinterRecoveryStatus::Unconfirmed)];
    app.busy = true;
    let _ = app.update(Message::Refreshed(
        0,
        Listing {
            files: Ok(vec![item(RecoveryStatus::Interrupted)]),
            printers: Err("unavailable".into()),
        },
    ));
    assert_eq!(app.printers[0].status, PrinterRecoveryStatus::Unavailable);
    assert!(!app.printers[0].dismissible());
    assert!(available(app.item().unwrap(), RecoveryBody::Document));
    assert!(app.printer_load_error.is_some());
}

#[test]
fn stale_printer_completion_and_back_cannot_hide_pending_record_mutation() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut app = app(&runtime);
    app.screen = Screen::RemovingPrinter;
    app.busy = true;
    app.generation = 4;
    let _ = app.update(Message::Back);
    let _ = app.update(Message::PrinterDismissed(3, Ok(())));
    assert!(app.busy && matches!(app.screen, Screen::RemovingPrinter));
    assert!(app.notice.is_none());
    let _ = app.update(Message::Close);
    let _ = app.update(Message::PrinterDismissed(4, Ok(())));
    assert!(app.closed && app.notice.is_none());
}
