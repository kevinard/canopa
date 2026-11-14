//! Compress and Extract through the Files dialogs, using private files only.
use super::file_operations::Event;
use super::tests::{desktop, loaded};
use super::*;
use crate::{archives::Format, operation_jobs::OperationJobs};
use std::{
    fs,
    time::{Duration, Instant},
};

fn wait(mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
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
    wait(|| !service.snapshot().holds_application());
    iced::futures::executor::block_on(service.shutdown().unwrap())
        .unwrap()
        .unwrap();
}

fn select(desktop: &mut Desktop, window: window::Id, paths: &[PathBuf]) {
    desktop
        .active_mut(window)
        .unwrap()
        .browser
        .as_mut()
        .unwrap()
        .set_selection(paths.iter().cloned());
}

#[test]
fn compress_proposes_a_name_remembers_the_format_and_extract_publishes_beside() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    fs::create_dir(&folder).unwrap();
    let report = folder.join("report.pdf");
    fs::write(&report, b"report").unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![folder.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &folder);
    let service = attach(&mut desktop, state.path());
    select(&mut desktop, window, std::slice::from_ref(&report));

    let _ = desktop.show_compress_dialog(window);
    let dialog = desktop
        .file_operations
        .name
        .as_ref()
        .expect("a Compress dialog");
    assert_eq!(dialog.title(), "Compress");
    // A single file is named after itself, without its extension.
    assert_eq!(dialog.value, "report");
    let id = dialog.window;
    let _ = desktop.update(Message::FileOperation(Event::NameFormat(
        id,
        Format::TarZst,
    )));
    assert_eq!(desktop.file_operations.archive_format, Format::TarZst);
    let _ = desktop.update(Message::FileOperation(Event::NameChanged(id, "a/b".into())));
    let _ = desktop.update(Message::FileOperation(Event::NameSubmit(id)));
    assert!(
        desktop
            .file_operations
            .name
            .as_ref()
            .is_some_and(|dialog| dialog.error.is_some() && !dialog.busy),
        "an invalid name is refused in the dialog"
    );
    let _ = desktop.update(Message::FileOperation(Event::NameChanged(
        id,
        "report".into(),
    )));
    let _ = desktop.update(Message::FileOperation(Event::NameSubmit(id)));
    let archive = folder.join("report.tar.zst");
    wait(|| {
        let jobs = service.snapshot().jobs;
        assert!(
            jobs.iter().all(|job| job.error.is_none()),
            "{:?}",
            jobs.iter()
                .map(|job| (&job.title, &job.error))
                .collect::<Vec<_>>()
        );
        archive.exists()
    });
    finish_dialog(&mut desktop, id);

    // The next Compress dialog starts from the last format.
    let _ = desktop.show_compress_dialog(window);
    let next = desktop.file_operations.name.as_ref().unwrap();
    assert!(matches!(
        next.archive,
        Some(name_dialog::Archive::Compress {
            format: Format::TarZst,
            ..
        })
    ));
    let next = next.window;
    let _ = desktop.close_name_dialog(next, false);

    let revision = desktop.active(window).unwrap().revision;
    loaded(&mut desktop, window, tab, revision, &folder);
    select(&mut desktop, window, std::slice::from_ref(&archive));
    let _ = desktop.extract_here(window);
    let extracted = folder.join("report (2).pdf");
    wait(|| extracted.exists());
    assert_eq!(fs::read(&extracted).unwrap(), b"report");
    finish(desktop, service);
}

fn finish_dialog(desktop: &mut Desktop, id: window::Id) {
    if desktop
        .file_operations
        .name
        .as_ref()
        .is_some_and(|dialog| dialog.window == id)
    {
        let _ = desktop.close_name_dialog(id, true);
    }
}

#[test]
fn extract_to_uses_the_chosen_folder() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    let target = root.path().join("target");
    fs::create_dir(&folder).unwrap();
    fs::create_dir(&target).unwrap();
    let source = folder.join("photos");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("a.jpg"), b"a").unwrap();
    let archive = crate::archives::run(
        &crate::archives::ArchiveRequest::compress(
            vec![source],
            folder.clone(),
            "photos".into(),
            Format::Zip,
        ),
        &Cancellation::default(),
        crate::archives::Limits::default(),
        &mut |_| {},
    )
    .unwrap()
    .output;
    let (mut desktop, window) = desktop(root.path(), vec![folder.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &folder);
    let service = attach(&mut desktop, state.path());
    select(&mut desktop, window, std::slice::from_ref(&archive));
    let _ = desktop.show_extract_dialog(window);
    let dialog = desktop
        .file_operations
        .name
        .as_ref()
        .expect("an Extract dialog");
    assert_eq!(dialog.title(), "Extract to");
    assert_eq!(Path::new(&dialog.value), folder);
    let id = dialog.window;
    let _ = desktop.update(Message::FileOperation(Event::NameChanged(
        id,
        String::new(),
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
    let _ = desktop.update(Message::FileOperation(Event::NameChanged(
        id,
        target.to_string_lossy().into_owned(),
    )));
    let _ = desktop.update(Message::FileOperation(Event::NameSubmit(id)));
    wait(|| target.join("photos/a.jpg").exists());
    finish_dialog(&mut desktop, id);
    finish(desktop, service);
}
