//! Accessibility of Browser rows and dialogs: tree-building helpers, and a
//! native AT-SPI check on a private accessibility bus. Private files only.
use super::accessibility_ui::{self as a11y, window_key};
use super::tests::{desktop, loaded};
use super::*;
use crate::browse::Entry;
use crate::preferences::{Detail, ViewSettings};
use canopa_ui::accessibility::testing::{self, atspi};
use canopa_ui::accessibility::{Bridge, Event as Accessibility, Harness};
use iced::advanced::renderer::Headless;
use std::fs;

const SIZE: Size = Size::new(1000.0, 640.0);

#[test]
fn rows_describe_type_size_and_hidden_state() {
    let root = tempfile::tempdir().unwrap();
    let report = root.path().join("report.pdf");
    let hidden = root.path().join(".notes");
    let folder = root.path().join("Projects");
    fs::write(&report, vec![0; 2048]).unwrap();
    fs::write(&hidden, b"x").unwrap();
    fs::create_dir(&folder).unwrap();
    let settings = ViewSettings {
        details: Vec::new(),
        ..ViewSettings::default()
    };
    assert_eq!(
        a11y::describe(&Entry::read(&report).unwrap(), &settings),
        "PDF file, 2.0 KiB"
    );
    assert_eq!(
        a11y::describe(&Entry::read(&hidden).unwrap(), &settings),
        "File, 1 B, hidden"
    );
    assert_eq!(
        a11y::describe(&Entry::read(&folder).unwrap(), &settings),
        "Folder"
    );
    let link = root.path().join("shortcut");
    std::os::unix::fs::symlink(&folder, &link).unwrap();
    assert_eq!(
        a11y::describe(&Entry::read(&link).unwrap(), &settings),
        "Link to folder"
    );
    let dated = ViewSettings {
        details: vec![Detail::Modified],
        ..ViewSettings::default()
    };
    assert!(
        a11y::describe(&Entry::read(&report).unwrap(), &dated)
            .starts_with("PDF file, 2.0 KiB, modified ")
    );
    // Keys follow the path bytes, not a lossy or translated label.
    let invalid = Path::new(std::ffi::OsStr::from_bytes(b"/tmp/caf\xe9"));
    assert_eq!(a11y::entry_key(invalid), "files-entry-/tmp/caf\\xe9");
}

#[test]
fn folder_announcements_follow_loading_selection_and_search() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    for name in ["alpha.txt", "beta.txt", ".hidden"] {
        fs::write(folder.join(name), b"x").unwrap();
    }
    let (mut desktop, window) = desktop(root.path(), vec![folder.clone()]);
    let tab = desktop.active(window).unwrap().id;
    assert_eq!(
        view::announcement(desktop.active(window).unwrap()),
        "Loading folder…"
    );
    loaded(&mut desktop, window, tab, 1, &folder);
    assert_eq!(
        view::announcement(desktop.active(window).unwrap()),
        "2 items"
    );
    let _ = desktop.update(Message::Select(window, folder.join("alpha.txt")));
    assert_eq!(
        view::announcement(desktop.active(window).unwrap()),
        "2 items, 1 selected"
    );
    // The keyboard cursor reports focus unless a text field holds it.
    let tab = desktop.active(window).unwrap();
    assert!(view::rows_have_focus(&desktop, window, tab));
    let _ = desktop.command(window, Command::Location);
    assert!(!view::rows_have_focus(
        &desktop,
        window,
        desktop.active(window).unwrap()
    ));
    // Every shown window is published, dialogs included.
    let dialog = window::Id::unique();
    desktop.delete_dialog = Some(delete_dialog::Dialog::for_selection(
        dialog,
        window,
        vec![folder.join("alpha.txt")],
    ));
    assert_eq!(
        desktop.window_ids().into_iter().collect::<Vec<_>>(),
        [window, dialog]
    );
}

#[test]
fn the_newest_operation_is_announced_once_and_failures_assertively() {
    use crate::operation_jobs::{JobSnapshot, Phase};
    let root = tempfile::tempdir().unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    assert_eq!(desktop.file_operations.announcement(), None);
    let job = |id, phase, error: Option<&str>| JobSnapshot {
        id,
        accepted_at: None,
        title: "copy 2 items".into(),
        phase,
        error: error.map(Into::into),
        paths: Vec::new().into(),
        detail: None,
        decision: None,
        progress: None,
        transfer: None,
        trash: None,
        restore: None,
        delete: None,
        recovery: None,
        recovery_actions: Default::default(),
        transfer_actions: Default::default(),
        dismissible: false,
    };
    desktop.file_operations.snapshot.jobs = vec![job(1, Phase::Finished, None)];
    assert_eq!(
        desktop.file_operations.announcement(),
        Some(("Copy 2 items: Completed".into(), false))
    );
    desktop
        .file_operations
        .snapshot
        .jobs
        .push(job(2, Phase::Failed, Some("Disk full")));
    assert_eq!(
        desktop.file_operations.announcement(),
        Some(("Copy 2 items: Could not finish. Disk full".into(), true))
    );
    // Only one window speaks: the focused one, else the first Browser.
    let other = window::Id::unique();
    assert!(desktop.announces_operations(window));
    desktop.focused_window = Some(other);
    assert!(!desktop.announces_operations(window));
}

async fn next(bridge: &Bridge<Message>) -> Accessibility<Message> {
    tokio::time::timeout(Duration::from_secs(5), bridge.next_event())
        .await
        .expect("no accessibility event")
        .expect("bridge closed")
}

async fn activated(bridge: &Bridge<Message>) {
    while !matches!(next(bridge).await, Accessibility::Activated) {}
}

async fn action(bridge: &Bridge<Message>) -> Message {
    loop {
        if let Accessibility::Action(message) = next(bridge).await {
            return *message;
        }
    }
}

#[tokio::test]
async fn native_rows_expose_selection_focus_and_the_delete_confirmation() {
    if !testing::isolated(
        "desktop::accessibility_tests::native_rows_expose_selection_focus_and_the_delete_confirmation",
    ) {
        return;
    }
    let connection = testing::connect().await;
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    let alpha = folder.join("alpha.txt");
    let beta = folder.join("beta.txt");
    fs::write(&alpha, b"alpha").unwrap();
    fs::write(&beta, b"beta").unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![folder.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &folder);
    let mut bridge = Bridge::new("files-test");
    bridge.open(window_key(window), &desktop.title(window), Some(window));
    bridge.set_window_focused(&window_key(window), true);
    // Nothing is published before an assistive technology asks.
    activated(&bridge).await;
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let publish = |desktop: &Desktop, id: window::Id, bridge: &mut Bridge<Message>| {
        let mut harness = Harness::new(desktop.view(id), &renderer, SIZE);
        bridge.publish_snapshot(&harness.collect());
    };
    publish(&desktop, window, &mut bridge);
    let browser = testing::named(&connection, "folder — Canopa Files").await;
    if std::env::var_os("CANOPA_ACCESSIBILITY_DUMP").is_some() {
        testing::dump(&browser).await;
    }

    // Rows are options of a multi-selectable list, described by type and size.
    testing::named_role(&connection, "folder", "list box").await;
    let row = testing::named_role(&connection, "alpha.txt", "list item").await;
    assert!(
        row.description()
            .await
            .starts_with("TXT file, 5 B, modified ")
    );
    assert!(!row.has_state(atspi::STATE_SELECTED).await);
    let (_, _, width, height) = row.extents().await;
    assert!(width > 0 && height > 0, "bounds for magnifiers");
    testing::named_role(&connection, "Home", "list item").await;
    testing::named_role(&connection, "Back", "push button").await;
    // The default action is the click's own message.
    assert!(row.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(&message, Message::Select(id, path) if *id == window && *path == alpha));
    let _ = desktop.update(message);
    publish(&desktop, window, &mut bridge);
    let row = testing::named_role(&connection, "alpha.txt", "list item").await;
    row.wait_for_state(atspi::STATE_SELECTED).await;
    // Focus follows the keyboard cursor held in the Browser model.
    row.wait_for_state(atspi::STATE_FOCUSED).await;
    // Shift+Down extends the selection; the status announces the change.
    let _ = desktop.update(Message::Key(
        window,
        keyboard::Key::Named(keyboard::key::Named::ArrowDown),
        keyboard::Modifiers::SHIFT,
        None,
    ));
    publish(&desktop, window, &mut bridge);
    let second = testing::named_role(&connection, "beta.txt", "list item").await;
    second.wait_for_state(atspi::STATE_SELECTED).await;
    second.wait_for_state(atspi::STATE_FOCUSED).await;
    assert!(row.has_state(atspi::STATE_SELECTED).await);
    testing::named_role(&connection, "2 items, 2 selected", "status bar").await;

    // A permanent deletion asks in its own window, an alert dialog whose
    // focused default is Cancel.
    let dialog = window::Id::unique();
    desktop.delete_dialog = Some(delete_dialog::Dialog::for_selection(
        dialog,
        window,
        vec![alpha.clone(), beta.clone()],
    ));
    bridge.open(window_key(dialog), &desktop.title(dialog), Some(dialog));
    bridge.set_window_focused(&window_key(window), false);
    bridge.set_window_focused(&window_key(dialog), true);
    activated(&bridge).await;
    publish(&desktop, dialog, &mut bridge);
    let alert = testing::named_role(&connection, "Delete permanently?", "alert").await;
    assert_eq!(
        alert.description().await,
        "Permanently delete 2 items? This can't be undone."
    );
    let cancel = testing::named_role(&connection, "Cancel", "push button").await;
    cancel.wait_for_state(atspi::STATE_FOCUSED).await;
    let confirm = testing::named_role(&connection, "Delete 2 items", "push button").await;
    assert!(confirm.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(
        message,
        Message::DeleteDialog(delete_dialog::Event::Confirm(id)) if id == dialog
    ));
    assert!(cancel.do_action(0).await);
    let message = action(&bridge).await;
    assert!(matches!(
        message,
        Message::DeleteDialog(delete_dialog::Event::Cancel(id)) if id == dialog
    ));

    bridge.close(&window_key(dialog));
    testing::gone(&connection, "Delete permanently? — Canopa Files").await;
    drop(bridge);
    testing::gone(&connection, "folder — Canopa Files").await;
}
