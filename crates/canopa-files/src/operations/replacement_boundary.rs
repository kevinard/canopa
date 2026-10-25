//! Private replacement races at the actual backup rename.
use super::*;
use std::cell::RefCell;
use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt, process::ExitStatusExt};
use std::time::{Duration, SystemTime};
mod journal;
type Hook = Box<dyn FnOnce(&Path, &Path)>;
thread_local! {
    static PHASE:RefCell<Option<(&'static str,Hook)>>=const{RefCell::new(None)};
    static CANCEL:RefCell<Option<Cancellation>>=const{RefCell::new(None)};
}
pub(super) fn at_phase(phase: &str, target: &Path, payload: &Path) {
    super::transfer::replacement_boundary::boundary(phase, target);
    if phase == "backup"
        && let Some(cancel) = CANCEL.with(|slot| slot.borrow_mut().take())
    {
        cancel.cancel();
    }
    let hook = PHASE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|(name, _)| *name == phase) {
            slot.take().map(|(_, hook)| hook)
        } else {
            None
        }
    });
    if let Some(hook) = hook {
        hook(target, payload);
    }
}

#[derive(Serialize, Deserialize)]
struct Marker {
    #[serde(with = "crate::path_serde")]
    directory: PathBuf,
    source: Identity,
    previous: Identity,
}
fn leaf() -> std::ffi::OsString {
    std::ffi::OsString::from_vec(b"item-\xfd".to_vec())
}
fn link(previous: bool) -> PathBuf {
    std::ffi::OsString::from_vec(if previous {
        b"../old-\xff".to_vec()
    } else {
        b"../new-\xfe".to_vec()
    })
    .into()
}

#[test]
fn killed_replacements_restore_or_finish_both_objects_without_replaying_completed_output() {
    for kind in ["copy", "move", "cross"] {
        for shape in ["file", "link"] {
            for point in [
                "before",
                "backup",
                "source",
                "publishing",
                "published",
                "returning",
                "returned",
                "returned-missing",
                "backup-edited",
                "publishing-backup-edited",
                "publishing-output-edited",
                "published-output-edited",
            ] {
                if (point == "source" && kind != "move")
                    || (shape == "link"
                        && (point.ends_with("edited") || point == "returned-missing"))
                {
                    continue;
                }
                let root = tempfile::Builder::new()
                    .prefix("canopa-replacement-restart-")
                    .tempdir()
                    .unwrap();
                let output = if kind == "cross" {
                    tempfile::Builder::new()
                        .prefix(".canopa-replacement-output-")
                        .tempdir_in(std::env::current_dir().unwrap())
                        .unwrap()
                } else {
                    tempfile::Builder::new()
                        .prefix("canopa-replacement-output-")
                        .tempdir()
                        .unwrap()
                };
                let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if kind == "cross" {
                        assert_ne!(
                            fs::metadata(root.path()).unwrap().dev(),
                            fs::metadata(output.path()).unwrap().dev()
                        );
                    }
                    let status = std::process::Command::new(std::env::current_exe().unwrap())
                        .args([
                            "--ignored",
                            "--exact",
                            "operations::replacement_boundary::replacement_crash_helper",
                        ])
                        .env("CANOPA_REPLACEMENT_ROOT", root.path())
                        .env("CANOPA_REPLACEMENT_OUTPUT", output.path())
                        .env("CANOPA_REPLACEMENT_KIND", kind)
                        .env("CANOPA_REPLACEMENT_SHAPE", shape)
                        .env("CANOPA_REPLACEMENT_POINT", point)
                        .status()
                        .unwrap();
                    assert_eq!(status.signal(), Some(9), "{kind}/{shape}/{point}");
                    let marker: Marker =
                        serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap())
                            .unwrap();
                    let source = root.path().join("incoming").join(leaf());
                    let target = output.path().join(leaf());
                    let previous = marker.directory.join("previous");
                    let payload = marker.directory.join("folder");
                    let output_edited = point.ends_with("output-edited");
                    let backup_edited =
                        point.ends_with("backup-edited") || point == "backup-edited";
                    if backup_edited {
                        fs::write(&previous, b"Newer previous bytes").unwrap();
                    }
                    if output_edited {
                        fs::write(
                            if point.starts_with("published-") {
                                &target
                            } else {
                                &payload
                            },
                            b"Newer incoming bytes",
                        )
                        .unwrap();
                    }
                    if point == "returned-missing" {
                        fs::remove_file(&target).unwrap();
                    }
                    let pending = point == "returned-missing"
                        || (point == "publishing-output-edited" && kind != "move");
                    let completed = matches!(
                        point,
                        "publishing"
                            | "published"
                            | "publishing-backup-edited"
                            | "published-output-edited"
                    );
                    for _ in 0..2 {
                        let engine = OperationEngine::open(root.path().join("history")).unwrap();
                        assert_eq!(
                            !engine
                                .interrupted_operations(&Cancellation::default())
                                .unwrap()
                                .is_empty(),
                            pending,
                            "{kind}/{shape}/{point}"
                        );
                        assert_eq!(
                            engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                            usize::from(completed),
                            "{kind}/{shape}/{point}"
                        );
                        if pending {
                            assert_eq!(
                                engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                                0
                            );
                            assert!(marker.source.matches(&source));
                            if point == "returned-missing" {
                                assert!(
                                    fs::symlink_metadata(&target)
                                        .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
                                );
                            } else {
                                assert_eq!(fs::read(&payload).unwrap(), b"Newer incoming bytes");
                                assert!(marker.previous.matches(&previous));
                                assert_eq!(
                                    fs::read(&previous).unwrap(),
                                    b"Selected previous bytes"
                                );
                                assert!(
                                    fs::symlink_metadata(&target)
                                        .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
                                );
                            }
                            continue;
                        }
                        let current = if completed { &target } else { &source };
                        if kind == "move" {
                            assert!(marker.source.matches(current));
                        }
                        if shape == "link" {
                            assert_eq!(fs::read_link(current).unwrap(), link(false));
                        } else {
                            assert_eq!(
                                fs::read(current).unwrap(),
                                if output_edited {
                                    b"Newer incoming bytes".as_slice()
                                } else {
                                    b"Selected incoming bytes".as_slice()
                                }
                            );
                        }
                        let saved = if completed { &previous } else { &target };
                        assert!(marker.previous.matches(saved), "{kind}/{shape}/{point}");
                        if shape == "link" {
                            assert_eq!(fs::read_link(saved).unwrap(), link(true));
                        } else {
                            assert_eq!(
                                fs::read(saved).unwrap(),
                                if backup_edited {
                                    b"Newer previous bytes".as_slice()
                                } else {
                                    b"Selected previous bytes".as_slice()
                                }
                            );
                        }
                        if kind != "move" || !completed {
                            assert!(marker.source.matches(&source));
                        } else {
                            assert!(
                                fs::symlink_metadata(&source)
                                    .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
                            );
                        }
                    }
                }));
                if let Err(panic) = checked {
                    eprintln!(
                        "replacement crash fixture {kind}/{shape}/{point} retained at {} and {}",
                        root.keep().display(),
                        output.keep().display()
                    );
                    std::panic::resume_unwind(panic);
                }
            }
        }
    }
}

#[test]
#[ignore = "parent kills actual replacement worker at durable claim/return/publication boundaries"]
fn replacement_crash_helper() {
    let Some(root) = std::env::var_os("CANOPA_REPLACEMENT_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let output = PathBuf::from(std::env::var_os("CANOPA_REPLACEMENT_OUTPUT").unwrap());
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-replacement-restart-")
    );
    let kind = std::env::var("CANOPA_REPLACEMENT_KIND").unwrap();
    let shape = std::env::var("CANOPA_REPLACEMENT_SHAPE").unwrap();
    let point = std::env::var("CANOPA_REPLACEMENT_POINT").unwrap();
    let incoming = root.join("incoming");
    fs::create_dir(&incoming).unwrap();
    let source = incoming.join(leaf());
    let target = output.join(leaf());
    if shape == "link" {
        std::os::unix::fs::symlink(link(false), &source).unwrap();
        std::os::unix::fs::symlink(link(true), &target).unwrap();
    } else {
        fs::write(&source, b"Selected incoming bytes").unwrap();
        fs::write(&target, b"Selected previous bytes").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    }
    let source_id = Identity::read(&source).unwrap();
    let previous = Identity::read(&target).unwrap();
    let cancellation = Cancellation::default();
    if point.starts_with("return") {
        CANCEL.with(|slot| *slot.borrow_mut() = Some(cancellation.clone()));
    }
    let at_gate = root.clone();
    let kill = move |_: &Path, payload: &Path| {
        fs::write(
            at_gate.join("marker.json"),
            serde_json::to_vec(&Marker {
                directory: payload.parent().unwrap().to_owned(),
                source: source_id,
                previous,
            })
            .unwrap(),
        )
        .unwrap();
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
        unreachable!();
    };
    let _before = if point == "before" {
        Some(rename_boundary::install(kill))
    } else {
        let phase = match point.as_str() {
            "unclaimed" => "unclaimed",
            "backup" | "backup-edited" => "backup",
            "source" => "source",
            "publishing" | "publishing-backup-edited" | "publishing-output-edited" => "publishing",
            "published" | "published-output-edited" => "published",
            "returning" => "returning",
            "returned" | "returned-missing" => "returned",
            _ => panic!("unknown private boundary"),
        };
        PHASE.with(|slot| *slot.borrow_mut() = Some((phase, Box::new(kill))));
        None
    };
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine
        .transfer(
            &transfer::TransferRequest {
                kind: if kind == "copy" {
                    transfer::TransferKind::Copy
                } else {
                    transfer::TransferKind::Move
                },
                sources: vec![source],
                destination: output,
            },
            &cancellation,
            |event| match event {
                transfer::TransferEvent::Conflict { .. } => transfer::TransferReply::Resolve {
                    choice: transfer::ConflictChoice::Replace,
                    apply_to_remaining: false,
                },
                _ => transfer::TransferReply::Continue,
            },
        )
        .unwrap();
    panic!("replacement boundary was not reached");
}

#[test]
fn a_restart_before_claim_preserves_a_changed_destination_without_returning_it() {
    for kind in ["copy", "move", "cross"] {
        let root = tempfile::Builder::new()
            .prefix("canopa-replacement-restart-")
            .tempdir()
            .unwrap();
        let output = if kind == "cross" {
            tempfile::Builder::new()
                .prefix(".canopa-replacement-output-")
                .tempdir_in(std::env::current_dir().unwrap())
                .unwrap()
        } else {
            tempfile::Builder::new()
                .prefix("canopa-replacement-output-")
                .tempdir()
                .unwrap()
        };
        let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if kind == "cross" {
                assert_ne!(
                    fs::metadata(root.path()).unwrap().dev(),
                    fs::metadata(output.path()).unwrap().dev()
                );
            }
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::replacement_boundary::replacement_crash_helper",
                ])
                .env("CANOPA_REPLACEMENT_ROOT", root.path())
                .env("CANOPA_REPLACEMENT_OUTPUT", output.path())
                .env("CANOPA_REPLACEMENT_KIND", kind)
                .env("CANOPA_REPLACEMENT_SHAPE", "file")
                .env("CANOPA_REPLACEMENT_POINT", "unclaimed")
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let target = output.path().join(leaf());
            let parked = output.path().join("saved-previous");
            fs::rename(&target, &parked).unwrap();
            fs::write(&target, b"newer work").unwrap();
            for _ in 0..2 {
                let engine = OperationEngine::open(root.path().join("history")).unwrap();
                assert!(
                    engine
                        .interrupted_operations(&Cancellation::default())
                        .unwrap()
                        .is_empty(),
                    "{kind}"
                );
                assert_eq!(
                    engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                    0
                );
                assert_eq!(fs::read(&target).unwrap(), b"newer work");
                assert_eq!(fs::read(&parked).unwrap(), b"Selected previous bytes");
                assert_eq!(
                    fs::read(root.path().join("incoming").join(leaf())).unwrap(),
                    b"Selected incoming bytes"
                );
                assert_eq!(fs::read_dir(output.path()).unwrap().count(), 2);
            }
        }));
        if let Err(panic) = checked {
            eprintln!(
                "unclaimed fixture {kind} retained at {} and {}",
                root.keep().display(),
                output.keep().display()
            );
            std::panic::resume_unwind(panic);
        }
    }
}

#[test]
fn restarted_replacements_refuse_cloned_generations_and_missing_roles() {
    for kind in ["copy", "move"] {
        for replaced in [
            "previous",
            "payload",
            "published",
            "previous-published",
            "role",
        ] {
            let root = tempfile::Builder::new()
                .prefix("canopa-replacement-restart-")
                .tempdir()
                .unwrap();
            let output = tempfile::Builder::new()
                .prefix("canopa-replacement-output-")
                .tempdir()
                .unwrap();
            let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let published = replaced.contains("published");
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "operations::replacement_boundary::replacement_crash_helper",
                    ])
                    .env("CANOPA_REPLACEMENT_ROOT", root.path())
                    .env("CANOPA_REPLACEMENT_OUTPUT", output.path())
                    .env("CANOPA_REPLACEMENT_KIND", kind)
                    .env("CANOPA_REPLACEMENT_SHAPE", "file")
                    .env(
                        "CANOPA_REPLACEMENT_POINT",
                        if published { "published" } else { "publishing" },
                    )
                    .status()
                    .unwrap();
                assert_eq!(status.signal(), Some(9));
                let marker: Marker =
                    serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap())
                        .unwrap();
                let previous = marker.directory.join("previous");
                let payload = marker.directory.join("folder");
                let target = output.path().join(leaf());
                let connection = rusqlite::Connection::open(root.path().join("history")).unwrap();
                let body: String = connection
                    .query_row("SELECT body FROM pending WHERE singleton=1", [], |row| {
                        row.get(0)
                    })
                    .unwrap();
                let mut body: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert!(body["stage"]["replacement"]["facts"]["originals"].is_object());
                let original = if replaced.starts_with("previous") {
                    &previous
                } else if published {
                    &target
                } else {
                    &payload
                };
                let parked = root.path().join("original-object");
                let bytes = fs::read(original).unwrap();
                if replaced == "role" {
                    body["stage"]["replacement"] = serde_json::Value::Null;
                } else {
                    let metadata = fs::metadata(original).unwrap();
                    fs::rename(original, &parked).unwrap();
                    fs::write(original, &bytes).unwrap();
                    fs::set_permissions(original, metadata.permissions()).unwrap();
                    fs::File::open(original)
                        .unwrap()
                        .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
                        .unwrap();
                    let identity = serde_json::to_value(Identity::read(original).unwrap()).unwrap();
                    if replaced.starts_with("previous") {
                        body["change"]["retained"]["identity"] = identity.clone();
                        body["change"]["retained"]["version"]["identity"] = identity.clone();
                        body["change"]["retained"]["replacement_facts"]["version"]["identity"] =
                            identity.clone();
                        body["stage"]["replacement"]["facts"]["version"]["identity"] = identity;
                        let version: fingerprint::Version = serde_json::from_value(
                            body["stage"]["replacement"]["facts"]["version"].clone(),
                        )
                        .unwrap();
                        assert!(
                            version
                                .matches_opened(&fs::File::open(original).unwrap(), true)
                                .unwrap()
                        );
                    } else {
                        body["change"]["identity"] = identity.clone();
                        body["stage"]["payload_identity"] = identity.clone();
                        if kind == "move" {
                            body["change"]["move_facts"]["version"]["identity"] = identity.clone();
                            body["stage"]["entry_move"]["facts"]["version"]["identity"] = identity;
                        } else {
                            body["stage"]["replacement"]["copy"]["identity"] = identity;
                        }
                    }
                }
                connection
                    .execute(
                        "UPDATE pending SET body=?1 WHERE singleton=1",
                        [serde_json::to_string(&body).unwrap()],
                    )
                    .unwrap();
                drop(connection);
                for _ in 0..2 {
                    let engine = OperationEngine::open(root.path().join("history")).unwrap();
                    assert!(
                        !engine
                            .interrupted_operations(&Cancellation::default())
                            .unwrap()
                            .is_empty(),
                        "{kind}/{replaced}"
                    );
                    assert_eq!(
                        engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                        0,
                        "{kind}/{replaced}"
                    );
                    assert_eq!(fs::read(original).unwrap(), bytes);
                    if replaced != "role" {
                        assert_eq!(fs::read(&parked).unwrap(), bytes);
                    }
                    if !published {
                        assert!(
                            fs::symlink_metadata(&target)
                                .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
                        );
                    }
                }
            }));
            if let Err(panic) = checked {
                eprintln!(
                    "replacement generation fixture {kind}/{replaced} retained at {} and {}",
                    root.keep().display(),
                    output.keep().display()
                );
                std::panic::resume_unwind(panic);
            }
        }
    }
}

#[test]
fn replacing_a_copy_does_not_mutate_a_replacement_destination_folder() {
    replaced_parent(transfer::TransferKind::Copy);
}

#[test]
fn replacing_a_move_does_not_mutate_a_replacement_destination_folder() {
    replaced_parent(transfer::TransferKind::Move);
}

fn replaced_parent(kind: transfer::TransferKind) {
    let root = tempfile::Builder::new()
        .prefix("canopa-replacement-boundary-")
        .tempdir()
        .unwrap();
    let incoming = root.path().join("incoming");
    let outgoing = root.path().join("outgoing");
    let parked = root.path().join("original-destination");
    fs::create_dir(&incoming).unwrap();
    fs::create_dir(&outgoing).unwrap();
    let source = incoming.join("item");
    let destination = outgoing.join("item");
    fs::write(&source, b"Selected incoming bytes").unwrap();
    fs::write(&destination, b"Selected previous bytes").unwrap();
    let source_identity = Identity::read(&source).unwrap();
    let previous_identity = Identity::read(&destination).unwrap();
    let moved_parent = outgoing.clone();
    let original_parent = parked.clone();
    let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
    let target = destination.clone();
    let _guard = rename_boundary::install(move |actual, backup| {
        assert_eq!(actual, target);
        assert_eq!(backup.file_name().unwrap(), "previous");
        let name = backup.parent().unwrap().file_name().unwrap().to_owned();
        fs::rename(&moved_parent, &original_parent).unwrap();
        fs::create_dir(&moved_parent).unwrap();
        fs::create_dir(moved_parent.join(name)).unwrap();
        fs::hard_link(original_parent.join("item"), moved_parent.join("item")).unwrap();
        fs::File::open(&moved_parent)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(expected))
            .unwrap();
    });
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let result = engine.transfer(
        &transfer::TransferRequest {
            kind,
            sources: vec![source.clone()],
            destination: outgoing.clone(),
        },
        &Cancellation::default(),
        |event| match event {
            transfer::TransferEvent::Conflict { .. } => transfer::TransferReply::Resolve {
                choice: transfer::ConflictChoice::Replace,
                apply_to_remaining: false,
            },
            _ => transfer::TransferReply::Continue,
        },
    );
    let preserved = source_identity.matches(&source)
        && previous_identity.matches(&destination)
        && previous_identity.matches(&parked.join("item"))
        && fs::metadata(&outgoing).unwrap().modified().unwrap() == expected;
    if !preserved {
        panic!(
            "{kind:?} replacement mutated an unselected destination folder: {result:?}; retained {}",
            root.keep().display()
        );
    }
    assert_eq!(fs::read(&source).unwrap(), b"Selected incoming bytes");
    assert_eq!(fs::read(&destination).unwrap(), b"Selected previous bytes");
}
