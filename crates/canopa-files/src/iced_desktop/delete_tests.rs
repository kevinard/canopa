//! Confirmation flows for permanent deletion, using private files only. The
//! Trash window's own reader is not used: it follows the session's data home.
use super::file_operations::{Event, Origin, Submission};
use super::tests::{desktop, loaded};
use super::*;
use crate::{
    operation_jobs::{JobSnapshot, OperationJobs, Phase, TrashFailure, TrashReport},
    trash::TrashLocation,
};
use std::{
    fs,
    time::{Duration, Instant},
};

fn wait(mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "the operation did not settle");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn attach(desktop: &mut Desktop, state: &Path) -> OperationJobs {
    let service =
        OperationJobs::start(state.join("history.sqlite3"), state.join("volumes.json")).unwrap();
    wait(|| !service.snapshot().initializing && service.storage().is_some());
    desktop.file_operations.storage = service.storage();
    desktop.file_operations.snapshot = service.snapshot();
    desktop.file_operations.service = Some(service.clone());
    assert!(desktop.file_operations.ready());
    service
}

fn finish(desktop: Desktop, service: OperationJobs) {
    drop(desktop);
    iced::futures::executor::block_on(service.shutdown().unwrap())
        .unwrap()
        .unwrap();
}

fn message(desktop: &Desktop) -> String {
    let dialog = desktop.delete_dialog.as_ref().expect("a confirmation");
    format!("{} | {}", dialog.title(), dialog.summary())
}

#[test]
fn shift_delete_asks_first_and_only_confirmation_deletes() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    let first = folder.join("first.txt");
    let second = folder.join("second.txt");
    fs::write(&first, b"x").unwrap();
    fs::write(&second, b"x").unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![folder.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &folder);
    let service = attach(&mut desktop, state.path());
    desktop
        .active_mut(window)
        .unwrap()
        .browser
        .as_mut()
        .unwrap()
        .set_selection([first.clone(), second.clone()]);
    let shift_delete = |desktop: &mut Desktop| {
        let _ = desktop.update(Message::Key(
            window,
            keyboard::Key::Named(keyboard::key::Named::Delete),
            keyboard::Modifiers::SHIFT,
            None,
        ));
    };
    shift_delete(&mut desktop);
    assert_eq!(
        message(&desktop),
        "Delete permanently? | 2 items will be deleted immediately. This can't be undone. | Delete 2 items"
    );
    let dialog = desktop.delete_dialog.as_ref().unwrap().window;
    // A second request never replaces the open confirmation.
    shift_delete(&mut desktop);
    assert_eq!(desktop.delete_dialog.as_ref().unwrap().window, dialog);
    let _ = desktop.update(Message::Key(
        dialog,
        keyboard::Key::Named(keyboard::key::Named::Escape),
        keyboard::Modifiers::empty(),
        None,
    ));
    assert!(desktop.delete_dialog.is_none());
    assert!(first.exists() && second.exists());

    shift_delete(&mut desktop);
    let dialog = desktop.delete_dialog.as_ref().unwrap().window;
    let _ = desktop.update(Message::DeleteDialog(delete_dialog::Event::Confirm(dialog)));
    assert!(desktop.delete_dialog.is_none());
    wait(|| !first.exists() && !second.exists());
    wait(|| {
        service
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.delete.is_some() && job.phase == Phase::Finished)
    });
    assert!(service.snapshot().undo.is_none());
    finish(desktop, service);
}

#[test]
fn a_volume_without_trash_offers_permanent_deletion_of_exactly_those_items() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("on a read-only volume.txt");
    fs::write(&file, b"x").unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, root.path());
    let origin = Origin::capture(&desktop, window).unwrap();
    desktop.file_operations.snapshot.jobs.push(JobSnapshot {
        id: 7,
        accepted_at: None,
        title: "Move 1 item to Trash".into(),
        phase: Phase::Running,
        error: None,
        paths: vec![file.clone()].into(),
        detail: None,
        decision: None,
        progress: None,
        transfer: None,
        trash: None,
        restore: None,
        delete: None,
        recovery: None,
        recovery_actions: Default::default(),
        archive: None,
        transfer_actions: Default::default(),
        dismissible: false,
    });
    let _ = desktop.update(Message::FileOperation(Event::Admitted(
        Arc::new(Submission {
            origin: Some(origin),
            owner: window,
            dialog: None,
            affected: vec![file.clone()],
            reveal: None,
            refresh_all: false,
        }),
        Some(Ok(7)),
    )));
    let job = &mut desktop.file_operations.snapshot.jobs[0];
    job.phase = Phase::Stopped;
    job.trash = Some(Arc::new(TrashReport {
        operation: None,
        completed: Vec::new(),
        failures: vec![TrashFailure {
            source: file.clone(),
            message: "Permission denied".into(),
        }],
        remaining: Vec::new(),
        cancelled: false,
        interrupted: None,
        unavailable: vec![file.clone()],
    }));
    let _ = desktop.update(Message::FileOperation(Event::Poll));
    assert_eq!(
        message(&desktop),
        "Can't move to Trash | “on a read-only volume.txt” is on a volume without a usable Trash, so it cannot be moved to Trash. Delete it permanently instead? This can't be undone. | Delete permanently"
    );
    assert!(file.exists());
    // Closing the Browser that asked retires its confirmation.
    let _ = desktop.update(Message::Close(window));
    assert!(desktop.delete_dialog.is_none());
    assert!(file.exists());
}

#[test]
fn empty_trash_names_the_count_and_removes_only_the_listed_items() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let data = root.path().join("private data");
    let location = TrashLocation::home(&data).unwrap();
    let engine =
        crate::operations::OperationEngine::open(state.path().join("engine.sqlite3")).unwrap();
    let sources: Vec<_> = ["a", "b"].map(|name| root.path().join(name)).into();
    for source in &sources {
        fs::write(source, b"x").unwrap();
    }
    engine
        .trash(&location, &sources, &crate::Cancellation::default())
        .unwrap();
    drop(engine);
    let items = location.list(&crate::Cancellation::default()).unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let service = attach(&mut desktop, state.path());
    let _ = desktop.confirm_delete_trash(window, items.clone(), true, false);
    assert_eq!(
        message(&desktop),
        "Empty Trash? | Permanently delete all 2 items in Trash? This can't be undone. | Empty Trash"
    );
    let dialog = desktop.delete_dialog.as_ref().unwrap().window;
    let _ = desktop.update(Message::DeleteDialog(delete_dialog::Event::Confirm(dialog)));
    wait(|| {
        location
            .list(&crate::Cancellation::default())
            .unwrap()
            .is_empty()
    });
    assert!(items.iter().all(|item| !item.path.exists()));
    let _ = desktop.confirm_delete_trash(window, Vec::new(), true, false);
    assert!(desktop.delete_dialog.is_none(), "nothing to confirm");
    finish(desktop, service);
}
