//! New document and bulk Rename through the Files dialogs, using private files.
use super::bulk_rename_ui::{Field, Mode};
use super::file_operations::Event;
use super::tests::{desktop, loaded};
use super::*;
use crate::operation_jobs::{OperationJobs, Request};
use std::{
    fs,
    time::{Duration, Instant, SystemTime},
};

fn wait(done: impl FnMut() -> bool) {
    wait_on(None, done)
}

/// Wait for `done`, describing `service`'s jobs if it never happens.
fn wait_on(service: Option<&OperationJobs>, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done() {
        assert!(
            Instant::now() < deadline,
            "the operation did not settle: {:?}",
            service.map(|service| service
                .snapshot()
                .jobs
                .into_iter()
                .map(|job| (job.title, job.phase, job.error, job.detail))
                .collect::<Vec<_>>())
        );
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
    wait(|| !service.snapshot().holds_application());
    iced::futures::executor::block_on(service.shutdown().unwrap())
        .unwrap()
        .unwrap();
}

fn close(desktop: &mut Desktop) {
    if let Some(id) = desktop.file_operations.name.as_ref().map(|d| d.window) {
        let _ = desktop.close_name_dialog(id, true);
    }
}

fn failures(service: &OperationJobs) -> Vec<(String, String)> {
    service
        .snapshot()
        .jobs
        .into_iter()
        .filter_map(|job| Some((job.title, job.error?)))
        .collect()
}

/// Undo the newest operation, which must be `label`, and wait for it.
fn undo(service: &OperationJobs, label: &str) {
    wait_on(Some(service), || {
        service
            .snapshot()
            .undo
            .is_some_and(|undo| undo.label == format!("Undo {label}"))
    });
    let expected = service.snapshot().undo.unwrap().id;
    let receiver = service.submit(Request::Undo { expected }, None).unwrap();
    iced::futures::executor::block_on(receiver)
        .unwrap()
        .unwrap();
    wait(|| {
        let snapshot = service.snapshot();
        assert!(failures(service).is_empty(), "{:?}", failures(service));
        snapshot
            .undo
            .as_ref()
            .is_none_or(|undo| undo.id != expected)
            && !snapshot.jobs.iter().any(|job| {
                matches!(
                    job.phase,
                    crate::operation_jobs::Phase::Queued | crate::operation_jobs::Phase::Running
                )
            })
    });
}

#[test]
fn new_documents_copy_a_template_under_a_free_name_and_undo_removes_them() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("Letter.odt"), b"mine").unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![folder.clone()]);
    let templates = desktop.templates_folder().expect("a Templates folder");
    fs::create_dir_all(&templates).unwrap();
    let template = templates.join("Letter.odt");
    fs::write(&template, b"template").unwrap();
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    fs::File::options()
        .write(true)
        .open(&template)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &folder);
    let service = attach(&mut desktop, state.path());

    let listed = crate::templates::list(&templates, root.path());
    assert_eq!(listed.len(), 1);
    let _ = desktop.show_document_dialog(window, Some(listed[0].clone()));
    let dialog = desktop.file_operations.name.as_ref().expect("a dialog");
    assert_eq!(dialog.title(), "New document");
    // The template's name is taken here, so the next free one is proposed.
    assert_eq!(dialog.value, "Letter (2).odt");
    let id = dialog.window;
    // A name typed over an item of the folder is refused in the dialog.
    let _ = desktop.update(Message::FileOperation(Event::NameChanged(
        id,
        "Letter.odt".into(),
    )));
    let _ = desktop.update(Message::FileOperation(Event::NameSubmit(id)));
    let dialog = desktop.file_operations.name.as_ref().unwrap();
    assert!(dialog.error.is_some() && !dialog.busy);
    let _ = desktop.update(Message::FileOperation(Event::NameChanged(
        id,
        "Letter (2).odt".into(),
    )));
    let _ = desktop.update(Message::FileOperation(Event::NameSubmit(id)));
    let created = folder.join("Letter (2).odt");
    wait_on(Some(&service), || {
        assert!(failures(&service).is_empty(), "{:?}", failures(&service));
        created.exists()
    });
    assert_eq!(fs::read(&created).unwrap(), b"template");
    // A new document is dated now, not when its template was made.
    assert!(fs::metadata(&created).unwrap().modified().unwrap() > old);
    assert_eq!(fs::read(folder.join("Letter.odt")).unwrap(), b"mine");
    close(&mut desktop);

    let _ = desktop.show_document_dialog(window, None);
    let dialog = desktop.file_operations.name.as_ref().unwrap();
    assert_eq!(dialog.value, "New document");
    let id = dialog.window;
    let _ = desktop.update(Message::FileOperation(Event::NameSubmit(id)));
    let empty = folder.join("New document");
    wait_on(Some(&service), || empty.exists());
    assert_eq!(fs::metadata(&empty).unwrap().len(), 0);
    close(&mut desktop);

    undo(&service, "create file New document");
    assert!(!empty.exists());
    undo(&service, "create file Letter (2).odt");
    assert!(!created.exists());
    assert!(template.exists());
    finish(desktop, service);
}

#[test]
fn several_items_rename_together_after_their_problems_are_fixed_and_undo_together() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    let selected: Vec<_> = ["a.txt", "b.txt", "c.txt"]
        .map(|name| folder.join(name))
        .into();
    for path in &selected {
        fs::write(path, path.file_name().unwrap().as_encoded_bytes()).unwrap();
    }
    fs::write(folder.join("Trip 2.txt"), b"other").unwrap();
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
        .set_selection(selected.iter().cloned());

    let _ = desktop.show_name_dialog(window, true);
    let dialog = desktop.file_operations.name.as_ref().expect("a dialog");
    assert_eq!(dialog.title(), "Rename 3 items");
    let id = dialog.window;
    let _ = desktop.update(Message::FileOperation(Event::BatchMode(id, Mode::Number)));
    let _ = desktop.update(Message::FileOperation(Event::BatchField(
        id,
        Field::Base,
        "Trip".into(),
    )));
    assert_eq!(
        desktop.batch_preview().unwrap(),
        [
            "Trip 1.txt",
            "Trip 2.txt [Name already used in this folder]",
            "Trip 3.txt"
        ]
    );
    let _ = desktop.update(Message::FileOperation(Event::NameSubmit(id)));
    let dialog = desktop.file_operations.name.as_ref().unwrap();
    assert!(
        dialog.error.is_some() && !dialog.busy,
        "a problem blocks Rename"
    );

    let _ = desktop.update(Message::FileOperation(Event::BatchField(
        id,
        Field::Start,
        "x".into(),
    )));
    let _ = desktop.update(Message::FileOperation(Event::NameSubmit(id)));
    assert!(
        desktop
            .file_operations
            .name
            .as_ref()
            .unwrap()
            .error
            .is_some()
    );
    let _ = desktop.update(Message::FileOperation(Event::BatchField(
        id,
        Field::Start,
        "8".into(),
    )));
    assert_eq!(
        desktop.batch_preview().unwrap(),
        ["Trip 08.txt", "Trip 09.txt", "Trip 10.txt"]
    );
    let _ = desktop.update(Message::FileOperation(Event::NameSubmit(id)));
    let renamed: Vec<_> = ["Trip 08.txt", "Trip 09.txt", "Trip 10.txt"]
        .map(|name| folder.join(name))
        .into();
    wait_on(Some(&service), || {
        assert!(failures(&service).is_empty(), "{:?}", failures(&service));
        renamed.iter().all(|path| path.exists())
    });
    assert_eq!(fs::read(&renamed[1]).unwrap(), b"b.txt");
    assert!(selected.iter().all(|path| !path.exists()));
    close(&mut desktop);

    // One Undo returns every name.
    undo(&service, "rename 3 items");
    for path in &selected {
        assert_eq!(
            fs::read(path).unwrap(),
            path.file_name().unwrap().as_encoded_bytes()
        );
    }
    assert!(renamed.iter().all(|path| !path.exists()));
    finish(desktop, service);
}

#[test]
fn replace_and_add_modes_preview_and_keep_extensions() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir_all(folder.join("IMG folder")).unwrap();
    for name in ["IMG_1.jpg", "IMG_2.jpg"] {
        fs::write(folder.join(name), b"x").unwrap();
    }
    let (mut desktop, window) = desktop(root.path(), vec![folder.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &folder);
    let state = tempfile::tempdir().unwrap();
    let service = attach(&mut desktop, state.path());
    desktop
        .active_mut(window)
        .unwrap()
        .browser
        .as_mut()
        .unwrap()
        .set_selection(
            ["IMG folder", "IMG_1.jpg", "IMG_2.jpg"]
                .into_iter()
                .map(|name| folder.join(name)),
        );
    let _ = desktop.show_name_dialog(window, true);
    let id = desktop.file_operations.name.as_ref().unwrap().window;
    let _ = desktop.update(Message::FileOperation(Event::BatchField(
        id,
        Field::Find,
        "IMG".into(),
    )));
    let _ = desktop.update(Message::FileOperation(Event::BatchField(
        id,
        Field::With,
        "Rome".into(),
    )));
    let mut preview = desktop.batch_preview().unwrap();
    preview.sort();
    assert_eq!(preview, ["Rome folder", "Rome_1.jpg", "Rome_2.jpg"]);
    let _ = desktop.update(Message::FileOperation(Event::BatchMode(id, Mode::Add)));
    let _ = desktop.update(Message::FileOperation(Event::BatchField(
        id,
        Field::Suffix,
        " copy".into(),
    )));
    let mut preview = desktop.batch_preview().unwrap();
    preview.sort();
    assert_eq!(
        preview,
        ["IMG folder copy", "IMG_1 copy.jpg", "IMG_2 copy.jpg"]
    );
    close(&mut desktop);
    finish(desktop, service);
}
