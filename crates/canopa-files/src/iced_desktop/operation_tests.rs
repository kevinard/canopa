use super::file_operations::{Event, Origin, Submission};
use super::tests::{desktop, loaded};
use super::*;
use crate::operation_jobs::{JobSnapshot, Phase};
use std::{fs, os::unix::fs::symlink};

#[test]
fn manual_recovery_dismissal_consumes_a_late_startup_attention_reply() {
    let root = tempfile::tempdir().unwrap();
    let service = crate::operation_jobs::OperationJobs::start(
        root.path().join("history"),
        root.path().join("volumes"),
    )
    .unwrap();
    let (mut desktop, _) = desktop(root.path(), vec![root.path().to_owned()]);
    desktop.file_operations.service = Some(service.clone());
    let _ = desktop.show_recovery();
    let recovery = desktop.recovery_window.as_ref().unwrap().window;
    let _ = desktop.close_recovery(recovery);
    assert!(desktop.recovery_window.is_none());
    let _ = desktop.update(Message::FileOperation(Event::InitialRecovery(Ok(true))));
    assert!(
        desktop.recovery_window.is_none(),
        "late inspection reopened manually dismissed Recovery"
    );
    drop(desktop);
    iced::futures::executor::block_on(service.shutdown().unwrap())
        .unwrap()
        .unwrap();
}

#[test]
fn startup_recovery_waits_for_a_browser_and_dismissal_survives_later_openings() {
    let root = tempfile::tempdir().unwrap();
    let service = crate::operation_jobs::OperationJobs::start(
        root.path().join("history"),
        root.path().join("volumes"),
    )
    .unwrap();
    let (mut desktop, _) = desktop(root.path(), vec![root.path().to_owned()]);
    desktop.file_operations.service = Some(service.clone());
    desktop.windows.clear();
    let _ = desktop.update(Message::FileOperation(Event::InitialRecovery(Ok(true))));
    assert!(
        desktop.recovery_window.is_none(),
        "inspection alone initialized a window"
    );
    let _ = desktop.new_window(vec![root.path().to_owned()]);
    let recovery = desktop.recovery_window.as_ref().unwrap().window;
    let _ = desktop.close_recovery(recovery);
    let _ = desktop.new_window(vec![root.path().to_owned()]);
    let _ = desktop.update(Message::FileOperation(Event::Poll));
    assert!(
        desktop.recovery_window.is_none(),
        "a later Browser reopened dismissed startup attention"
    );
    drop(desktop);
    iced::futures::executor::block_on(service.shutdown().unwrap())
        .unwrap()
        .unwrap();
}

fn admit_result(desktop: &mut Desktop, owner: window::Id, origin: Arc<Origin>, path: PathBuf) {
    admit_phase(desktop, owner, origin, path, Phase::Finished);
}

fn admit_phase(
    desktop: &mut Desktop,
    owner: window::Id,
    origin: Arc<Origin>,
    path: PathBuf,
    phase: Phase,
) {
    desktop.file_operations.snapshot.jobs.push(JobSnapshot {
        id: 1,
        accepted_at: None,
        title: "Create folder".into(),
        phase,
        error: None,
        paths: vec![path.clone()].into(),
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
    });
    let _ = desktop.update(Message::FileOperation(Event::Admitted(
        Arc::new(Submission {
            origin: Some(origin),
            owner,
            dialog: None,
            affected: vec![path.clone()],
            reveal: Some(path),
            refresh_all: false,
        }),
        Some(Ok(1)),
    )));
}

#[test]
fn stopped_and_interrupted_results_refresh_the_origin_and_explain_its_partial_changes() {
    for phase in [Phase::Stopped, Phase::Interrupted] {
        let root = tempfile::tempdir().unwrap();
        let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
        let tab = desktop.active(window).unwrap().id;
        loaded(&mut desktop, window, tab, 1, root.path());
        let origin = Origin::capture(&desktop, window).unwrap();
        admit_phase(
            &mut desktop,
            window,
            origin,
            root.path().join("created prefix"),
            Phase::Running,
        );
        assert!(!desktop.active(window).unwrap().loading);
        desktop.file_operations.snapshot.jobs[0].phase = phase;
        desktop.file_operations.snapshot.jobs[0].error =
            Some("Some changes completed; review unfinished work.".into());
        let _ = desktop.update(Message::FileOperation(Event::Poll));
        let source = desktop.active(window).unwrap();
        assert!(
            source.loading,
            "the confirmed prefix left its browser stale"
        );
        assert_eq!(source.revision, 2);
        assert_eq!(
            source.error.as_deref(),
            Some("Some changes completed; review unfinished work.")
        );
        let _ = desktop.update(Message::FileOperation(Event::Poll));
        assert_eq!(
            desktop.active(window).unwrap().revision,
            2,
            "the same result refreshed twice"
        );
    }
}

#[test]
fn retrying_a_failed_start_save_keeps_the_later_completion_attached_to_its_view() {
    let root = tempfile::tempdir().unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, root.path());
    let origin = Origin::capture(&desktop, window).unwrap();
    admit_phase(
        &mut desktop,
        window,
        origin,
        root.path().join("created"),
        Phase::StartSaveFailed,
    );
    assert!(
        !desktop.active(window).unwrap().loading,
        "work has not started"
    );
    desktop.file_operations.snapshot.jobs[0].phase = Phase::Finished;
    let _ = desktop.update(Message::FileOperation(Event::Poll));
    assert!(
        desktop.active(window).unwrap().loading,
        "completed work lost its source view"
    );
    assert_eq!(desktop.active(window).unwrap().revision, 2);
}

#[test]
fn completion_preserves_newer_navigation_and_refreshes_its_eventual_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![a.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &a);
    let origin = Origin::capture(&desktop, window).unwrap();
    let _ = desktop.navigate(window, b.clone(), Navigation::Visit, None);
    let revision = desktop.active(window).unwrap().revision;
    let pending = desktop.active(window).unwrap().scan_cancel.clone();
    admit_result(&mut desktop, window, origin, a.join("created"));
    assert_eq!(desktop.active(window).unwrap().revision, revision);
    assert!(
        !pending.is_cancelled(),
        "completion cancelled newer navigation"
    );
    loaded(&mut desktop, window, tab, revision, &b);
    assert_eq!(desktop.active(window).unwrap().path(), b);
    assert_eq!(desktop.active(window).unwrap().revision, revision + 1);
    assert!(desktop.active(window).unwrap().loading);
}

#[test]
fn completion_refreshes_another_browser_using_an_alias_of_the_same_directory() {
    let root = tempfile::tempdir().unwrap();
    let real = root.path().join("real");
    let alias = root.path().join("alias");
    fs::create_dir(&real).unwrap();
    symlink(&real, &alias).unwrap();
    let (mut desktop, window) = desktop(root.path(), vec![real.clone(), alias.clone()]);
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &real);
    let origin = Origin::capture(&desktop, window).unwrap();
    let _ = desktop.update(Message::SwitchTab(window, 1));
    let tab = desktop.active(window).unwrap().id;
    loaded(&mut desktop, window, tab, 1, &alias);
    admit_result(&mut desktop, window, origin, real.join("created"));
    assert!(
        desktop.active(window).unwrap().loading,
        "alias view stayed stale"
    );
    assert_eq!(desktop.active(window).unwrap().path(), alias);
    assert!(desktop.windows[&window].tabs[0].needs_refresh);
}
