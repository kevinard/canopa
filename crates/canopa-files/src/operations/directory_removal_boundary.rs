//! Replace the source namespace at the actual empty-folder removal boundary.
use super::*;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::process::ExitStatusExt;
use std::{
    cell::{Cell, RefCell},
    os::unix::fs::PermissionsExt,
    rc::Rc,
};
type Hook = Box<dyn FnOnce(&Path, &Path)>;
thread_local! { static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) }; }
thread_local! {
    static PHASE: RefCell<Option<(&'static str, Hook)>> = const { RefCell::new(None) };
    static CANCEL: RefCell<Option<Cancellation>> = const { RefCell::new(None) };
}
pub(super) fn at_phase(point: &str, source: &Path, payload: &Path) {
    super::transfer::directory_boundary::removal_boundary(point, source);
    if point == "claimed"
        && let Some(cancel) = CANCEL.with(|slot| slot.borrow_mut().take())
    {
        cancel.cancel();
    }
    let hook = PHASE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|(name, _)| *name == point) {
            slot.take().map(|(_, hook)| hook)
        } else {
            None
        }
    });
    if let Some(hook) = hook {
        hook(source, payload);
    }
}
pub(super) fn before_remove(source: &Path, destination: &Path) {
    if let Some(hook) = HOOK.with(|slot| slot.borrow_mut().take()) {
        hook(source, destination);
    }
}
fn refuses_changed_folder(parent: bool) {
    let root = tempfile::Builder::new()
        .prefix("canopa-directory-cleanup-boundary-")
        .tempdir()
        .unwrap();
    let output = tempfile::Builder::new()
        .prefix(".canopa-directory-cleanup-output-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_ne!(
            fs::metadata(root.path()).unwrap().dev(),
            fs::metadata(output.path()).unwrap().dev()
        );
        let incoming = root.path().join("incoming");
        let source = incoming.join("bundle");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("note"), b"Selected source bytes").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o750)).unwrap();
        let original = Identity::read(&source).unwrap();
        let foreign = Rc::new(Cell::new(None));
        let marker = foreign.clone();
        let parked = root.path().join("parked-original");
        let kept = parked.clone();
        HOOK.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move |source, _| {
                if parent {
                    fs::rename(source.parent().unwrap(), &kept).unwrap();
                    fs::create_dir(source.parent().unwrap()).unwrap();
                } else {
                    fs::rename(source, &kept).unwrap();
                }
                fs::create_dir(source).unwrap();
                fs::set_permissions(source, fs::Permissions::from_mode(0o711)).unwrap();
                marker.set(Some(Identity::read(source).unwrap()));
            }))
        });
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let result = engine
            .transfer(
                &transfer::TransferRequest {
                    kind: transfer::TransferKind::Move,
                    sources: vec![source.clone()],
                    destination: output.path().to_owned(),
                },
                &Cancellation::default(),
                |_| transfer::TransferReply::Continue,
            )
            .unwrap();
        let foreign = foreign
            .get()
            .expect("actual removal boundary was not reached");
        assert!(
            foreign.matches(&source),
            "source cleanup deleted a replacement folder"
        );
        assert_eq!(
            fs::metadata(&source).unwrap().permissions().mode() & 0o777,
            0o711
        );
        assert!(original.matches(&if parent {
            parked.join("bundle")
        } else {
            parked.clone()
        }));
        assert_eq!(
            fs::read(output.path().join("bundle/note")).unwrap(),
            b"Selected source bytes"
        );
        assert!(result.needs_attention || !result.failures.is_empty());
    }));
    if let Err(panic) = checked {
        eprintln!(
            "directory cleanup fixture parent={parent} retained at {} and {}",
            root.keep().display(),
            output.keep().display()
        );
        std::panic::resume_unwind(panic);
    }
}
#[test]
fn final_source_cleanup_preserves_a_replacement_folder() {
    refuses_changed_folder(false);
}
#[test]
fn final_source_cleanup_preserves_a_replacement_parent_and_its_folder() {
    refuses_changed_folder(true);
}

#[derive(Serialize, Deserialize)]
struct Marker {
    source: Identity,
    before: usize,
}
#[derive(Serialize, Deserialize)]
struct RestartMarker {
    source: Identity,
    before: usize,
    #[serde(with = "crate::path_serde")]
    path: PathBuf,
    #[serde(with = "crate::path_serde")]
    target: PathBuf,
    #[serde(with = "crate::path_serde")]
    payload: PathBuf,
    #[serde(with = "crate::path_serde")]
    work: PathBuf,
}

#[test]
fn directory_claims_recover_verified_removal_or_return_and_preserve_newer_work() {
    for point in [
        "before",
        "claimed",
        "verified",
        "removed",
        "returning",
        "returned",
        "returned-missing",
        "new-source",
        "new-source-after",
        "dest",
        "dest-after",
        "new-entry",
        "foreign-payload",
        "work",
        "parent",
    ] {
        let root = tempfile::Builder::new()
            .prefix("canopa-directory-cleanup-restart-")
            .tempdir()
            .unwrap();
        let output = tempfile::Builder::new()
            .prefix(".canopa-directory-cleanup-output-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_ne!(
                fs::metadata(root.path()).unwrap().dev(),
                fs::metadata(output.path()).unwrap().dev()
            );
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::directory_removal_boundary::directory_crash_child",
                ])
                .env("CANOPA_DIRECTORY_CLEANUP_ROOT", root.path())
                .env("CANOPA_DIRECTORY_CLEANUP_OUTPUT", output.path())
                .env("CANOPA_DIRECTORY_CLEANUP_POINT", point)
                .env("CANOPA_DIRECTORY_CLEANUP_RAW", "1")
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9), "{point}");
            let m: RestartMarker =
                serde_json::from_slice(&fs::read(root.path().join("restart-marker.json")).unwrap())
                    .unwrap();
            let mut foreign_source = None;
            let mut foreign_payload = None;
            match point {
                "new-source" | "new-source-after" => {
                    fs::create_dir(&m.path).unwrap();
                    fs::set_permissions(&m.path, fs::Permissions::from_mode(0o711)).unwrap();
                    foreign_source = Some(Identity::read(&m.path).unwrap());
                }
                "dest" | "dest-after" => {
                    fs::rename(&m.target, output.path().join("parked-destination")).unwrap();
                    fs::create_dir(&m.target).unwrap();
                    fs::write(m.target.join("foreign"), b"New destination data").unwrap();
                }
                "new-entry" => fs::write(m.payload.join("new"), b"New source-folder data").unwrap(),
                "foreign-payload" => {
                    fs::rename(&m.payload, root.path().join("parked-source")).unwrap();
                    fs::create_dir(&m.payload).unwrap();
                    foreign_payload = Some(Identity::read(&m.payload).unwrap());
                }
                "work" => {
                    fs::rename(&m.work, root.path().join("parked-work")).unwrap();
                    fs::create_dir(&m.work).unwrap();
                    fs::write(m.work.join("foreign"), b"New work directory").unwrap();
                }
                "parent" => {
                    fs::rename(m.path.parent().unwrap(), root.path().join("parked-parent"))
                        .unwrap();
                    fs::create_dir(m.path.parent().unwrap()).unwrap();
                    fs::create_dir(&m.path).unwrap();
                    foreign_source = Some(Identity::read(&m.path).unwrap());
                }
                "returned-missing" => fs::remove_dir(&m.path).unwrap(),
                _ => {}
            }
            let completed = matches!(
                point,
                "verified" | "removed" | "new-source" | "new-source-after" | "dest-after"
            );
            let pending = matches!(
                point,
                "returned-missing" | "foreign-payload" | "work" | "parent"
            );
            for _ in 0..2 {
                let engine = OperationEngine::open(root.path().join("history")).unwrap();
                assert_eq!(
                    engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                    m.before + usize::from(completed),
                    "{point}"
                );
                assert_eq!(
                    !engine
                        .interrupted_operations(&Cancellation::default())
                        .unwrap()
                        .is_empty(),
                    pending,
                    "{point}"
                );
                if let Some(foreign) = foreign_source {
                    assert!(foreign.matches(&m.path), "{point}");
                } else if matches!(
                    point,
                    "before" | "claimed" | "returning" | "returned" | "dest" | "new-entry"
                ) {
                    assert!(m.source.matches(&m.path), "{point}");
                    assert_eq!(
                        fs::metadata(&m.path).unwrap().permissions().mode() & 0o777,
                        0o750
                    );
                } else {
                    assert!(
                        fs::symlink_metadata(&m.path)
                            .is_err_and(|e| e.kind() == io::ErrorKind::NotFound),
                        "{point}"
                    );
                }
                if point == "new-entry" {
                    assert_eq!(
                        fs::read(m.path.join("new")).unwrap(),
                        b"New source-folder data"
                    );
                }
                if point == "foreign-payload" {
                    assert!(m.source.matches(&root.path().join("parked-source")));
                    assert!(foreign_payload.unwrap().matches(&m.payload));
                }
                if point == "work" {
                    assert!(m.source.matches(&m.payload));
                    assert_eq!(
                        fs::read(m.work.join("foreign")).unwrap(),
                        b"New work directory"
                    );
                }
                if point == "parent" {
                    assert!(
                        m.source.matches(
                            &root
                                .path()
                                .join("parked-parent")
                                .join(m.payload.file_name().unwrap())
                        )
                    );
                }
                let destination = if matches!(point, "dest" | "dest-after") {
                    output.path().join("parked-destination")
                } else {
                    m.target.clone()
                };
                assert_eq!(
                    fs::read(destination.join("note")).unwrap(),
                    b"Selected source bytes"
                );
                if matches!(point, "dest" | "dest-after") {
                    assert_eq!(
                        fs::read(m.target.join("foreign")).unwrap(),
                        b"New destination data"
                    );
                }
            }
        }));
        if let Err(panic) = checked {
            eprintln!(
                "directory claim {point} retained at {} and {}",
                root.keep().display(),
                output.keep().display()
            );
            std::panic::resume_unwind(panic);
        }
    }
}

#[test]
fn an_unverified_directory_removal_is_not_completed_when_its_name_changes_after_crash() {
    let root = tempfile::Builder::new()
        .prefix("canopa-directory-cleanup-restart-")
        .tempdir()
        .unwrap();
    let output = tempfile::Builder::new()
        .prefix(".canopa-directory-cleanup-output-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_ne!(
            fs::metadata(root.path()).unwrap().dev(),
            fs::metadata(output.path()).unwrap().dev()
        );
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::directory_removal_boundary::directory_crash_child",
            ])
            .env("CANOPA_DIRECTORY_CLEANUP_ROOT", root.path())
            .env("CANOPA_DIRECTORY_CLEANUP_OUTPUT", output.path())
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let marker: Marker =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        let source = root.path().join("incoming/bundle");
        let parked = root.path().join("parked-original");
        fs::rename(&source, &parked).unwrap();
        fs::create_dir(&source).unwrap();
        let foreign = Identity::read(&source).unwrap();
        for _ in 0..2 {
            let engine = OperationEngine::open(root.path().join("history")).unwrap();
            assert_eq!(
                engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                marker.before,
                "recovery recorded an unverified source-directory removal"
            );
            assert!(marker.source.matches(&parked));
            assert!(foreign.matches(&source));
            assert_eq!(
                fs::read(output.path().join("bundle/note")).unwrap(),
                b"Selected source bytes"
            );
        }
    }));
    if let Err(panic) = checked {
        eprintln!(
            "directory cleanup restart fixture retained at {} and {}",
            root.keep().display(),
            output.keep().display()
        );
        std::panic::resume_unwind(panic);
    }
}

#[test]
#[ignore = "actual transfer child killed after the directory-removal intent and before its syscall"]
fn directory_crash_child() {
    let Some(root) = std::env::var_os("CANOPA_DIRECTORY_CLEANUP_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-directory-cleanup-restart-")
    );
    let output = PathBuf::from(std::env::var_os("CANOPA_DIRECTORY_CLEANUP_OUTPUT").unwrap());
    let source = if std::env::var_os("CANOPA_DIRECTORY_CLEANUP_RAW").is_some() {
        root.join("incoming")
            .join(std::ffi::OsString::from_vec(b"bundle-\xfd".to_vec()))
    } else {
        root.join("incoming/bundle")
    };
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("note"), b"Selected source bytes").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o750)).unwrap();
    let at_gate = root.clone();
    let kill: Hook = Box::new(move |_, _| {
        let connection = rusqlite::Connection::open(at_gate.join("history")).unwrap();
        let before = connection
            .query_row(
                "SELECT count(*) FROM changes WHERE operation=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let body: String = connection
            .query_row("SELECT body FROM pending WHERE singleton=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let pending: Pending = serde_json::from_str(&body).unwrap();
        let Pending::Forward {
            change:
                Change::RemovedDirectory {
                    path,
                    identity,
                    moved: Some(moved),
                    ..
                },
            stage,
            ..
        } = pending
        else {
            panic!("missing removal intent");
        };
        fs::write(
            at_gate.join("marker.json"),
            serde_json::to_vec(&Marker {
                source: identity,
                before,
            })
            .unwrap(),
        )
        .unwrap();
        if let Some(stage) = stage {
            fs::write(
                at_gate.join("restart-marker.json"),
                serde_json::to_vec(&RestartMarker {
                    source: identity,
                    before,
                    path,
                    target: moved.current,
                    payload: stage.payload(),
                    work: stage.directory,
                })
                .unwrap(),
            )
            .unwrap();
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
        unreachable!();
    });
    let point = std::env::var("CANOPA_DIRECTORY_CLEANUP_POINT").unwrap_or_else(|_| "before".into());
    let phase = match point.as_str() {
        "claimed" => "claimed",
        "verified" | "new-source" | "dest" | "new-entry" | "foreign-payload" | "work"
        | "parent" => "verified",
        "removed" | "new-source-after" | "dest-after" => "removed",
        "returning" => "returning",
        "returned" | "returned-missing" => "returned",
        _ => "before",
    };
    let cancellation = Cancellation::default();
    if point.starts_with("return") {
        CANCEL.with(|slot| *slot.borrow_mut() = Some(cancellation.clone()));
    }
    if phase == "before" {
        HOOK.with(|slot| *slot.borrow_mut() = Some(kill));
    } else {
        PHASE.with(|slot| *slot.borrow_mut() = Some((phase, kill)));
    }
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine
        .transfer(
            &transfer::TransferRequest {
                kind: transfer::TransferKind::Move,
                sources: vec![source],
                destination: output,
            },
            &cancellation,
            |_| transfer::TransferReply::Continue,
        )
        .unwrap();
    panic!("the actual source-directory removal boundary was not reached");
}
