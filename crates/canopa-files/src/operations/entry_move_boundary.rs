//! Real whole-entry transfer rename boundaries, using private filesystem data.
use super::*;
use std::cell::RefCell;
use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt, process::ExitStatusExt};

type Hook = Box<dyn FnOnce(&Path, &Path)>;
thread_local! {
    static PHASE: RefCell<Option<(&'static str, Hook)>> = const { RefCell::new(None) };
    static CANCEL_PUBLISH: RefCell<Option<Cancellation>> = const { RefCell::new(None) };
}
pub(super) fn at_phase(phase: &str, source: &Path, payload: &Path) {
    super::transfer::move_boundary::boundary(phase, source);
    super::transfer::saved_boundary::boundary(phase, source, payload);
    if phase == "publishing"
        && let Some(token) = CANCEL_PUBLISH.with(|slot| slot.borrow_mut().take())
    {
        token.cancel();
    }
    let hook = PHASE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot
            .as_ref()
            .is_some_and(|(expected, _)| *expected == phase)
        {
            slot.take().map(|(_, hook)| hook)
        } else {
            None
        }
    });
    if let Some(hook) = hook {
        hook(source, payload);
    }
}

#[derive(Serialize, Deserialize)]
struct Marker {
    #[serde(with = "crate::path_serde")]
    payload: PathBuf,
    identity: Identity,
}

fn leaf() -> std::ffi::OsString {
    std::ffi::OsString::from_vec(b"item-\xfe".to_vec())
}
fn link_target() -> PathBuf {
    std::ffi::OsString::from_vec(b"../missing-\xff".to_vec()).into()
}

#[test]
fn killed_whole_entry_moves_reconcile_publication_and_return_across_two_restarts() {
    for shape in ["file", "link", "folder"] {
        for point in [
            "before",
            "ready",
            "publishing",
            "published",
            "returning",
            "returned",
            "returned-missing",
            "publishing-edited",
            "published-edited",
        ] {
            if (shape == "link" && point.ends_with("edited"))
                || (shape != "file" && point == "returned-missing")
            {
                continue;
            }
            let root = tempfile::Builder::new()
                .prefix("canopa-entry-move-restart-")
                .tempdir()
                .unwrap();
            let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "operations::entry_move_boundary::entry_move_crash_helper",
                    ])
                    .env("CANOPA_ENTRY_MOVE_ROOT", root.path())
                    .env("CANOPA_ENTRY_MOVE_POINT", point)
                    .env("CANOPA_ENTRY_MOVE_SHAPE", shape)
                    .status()
                    .unwrap();
                assert_eq!(status.signal(), Some(9), "{shape}/{point}");
                let marker: Marker =
                    serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap())
                        .unwrap();
                let source = root.path().join("incoming").join(leaf());
                let destination = root.path().join("outgoing").join(leaf());
                let edited = point.ends_with("edited");
                if edited {
                    if shape == "folder" {
                        fs::write(marker.payload.join("later-entry"), b"Newer folder work")
                            .unwrap();
                    } else {
                        fs::write(&marker.payload, b"Newer file work").unwrap();
                    }
                }
                if point == "returned-missing" {
                    fs::remove_file(&source).unwrap();
                }
                let published = matches!(point, "publishing" | "published" | "published-edited");
                let pending = point == "returned-missing";
                for _ in 0..2 {
                    let engine = OperationEngine::open(root.path().join("history")).unwrap();
                    assert_eq!(
                        !engine
                            .interrupted_operations(&Cancellation::default())
                            .unwrap()
                            .is_empty(),
                        pending,
                        "{shape}/{point}"
                    );
                    assert_eq!(
                        engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                        usize::from(published),
                        "{shape}/{point}"
                    );
                    if pending {
                        assert!(!source.exists() && !destination.exists());
                        continue;
                    }
                    let current = if published { &destination } else { &source };
                    assert!(marker.identity.matches(current), "{shape}/{point}");
                    if shape == "link" {
                        assert_eq!(fs::read_link(current).unwrap(), link_target());
                    } else if shape == "folder" {
                        assert_eq!(
                            fs::read(current.join("child")).unwrap(),
                            b"Original folder child"
                        );
                        if edited {
                            assert_eq!(
                                fs::read(current.join("later-entry")).unwrap(),
                                b"Newer folder work"
                            );
                        }
                        assert_eq!(
                            fs::metadata(current).unwrap().permissions().mode() & 0o777,
                            0o755
                        );
                    } else {
                        assert_eq!(
                            fs::read(current).unwrap(),
                            if edited {
                                b"Newer file work".as_slice()
                            } else {
                                b"Original file bytes".as_slice()
                            }
                        );
                    }
                    let other = if published { &source } else { &destination };
                    assert!(
                        fs::symlink_metadata(other)
                            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
                    );
                    assert!(
                        !fs::read_dir(root.path().join("outgoing"))
                            .unwrap()
                            .any(|entry| entry
                                .unwrap()
                                .file_name()
                                .as_encoded_bytes()
                                .starts_with(b".canopa-files-stage-")),
                        "{shape}/{point}"
                    );
                }
            }));
            if let Err(panic) = checked {
                eprintln!(
                    "whole-entry crash fixture {shape}/{point} retained at {}",
                    root.keep().display()
                );
                std::panic::resume_unwind(panic);
            }
        }
    }
}

#[test]
#[ignore = "parent kills the real worker at whole-entry Move journal boundaries"]
fn entry_move_crash_helper() {
    use std::os::unix::ffi::OsStrExt;
    let Some(root) = std::env::var_os("CANOPA_ENTRY_MOVE_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(
        root.file_name()
            .unwrap()
            .as_bytes()
            .starts_with(b"canopa-entry-move-restart-")
    );
    let point = std::env::var("CANOPA_ENTRY_MOVE_POINT").unwrap();
    let shape = std::env::var("CANOPA_ENTRY_MOVE_SHAPE").unwrap();
    let incoming = root.join("incoming");
    let outgoing = root.join("outgoing");
    fs::create_dir(&incoming).unwrap();
    fs::create_dir(&outgoing).unwrap();
    let source = incoming.join(leaf());
    match shape.as_str() {
        "file" => fs::write(&source, b"Original file bytes").unwrap(),
        "link" => std::os::unix::fs::symlink(link_target(), &source).unwrap(),
        "folder" => {
            fs::create_dir(&source).unwrap();
            fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
            fs::write(source.join("child"), b"Original folder child").unwrap();
        }
        _ => panic!("unknown private fixture shape"),
    }
    let identity = Identity::read(&source).unwrap();
    let cancellation = Cancellation::default();
    if point.starts_with("return") {
        CANCEL_PUBLISH.with(|slot| *slot.borrow_mut() = Some(cancellation.clone()));
    }
    let at_gate = root.clone();
    let kill = move |_: &Path, payload: &Path| {
        fs::write(
            at_gate.join("marker.json"),
            serde_json::to_vec(&Marker {
                payload: payload.to_owned(),
                identity,
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
            "ready" => "ready",
            "publishing" | "publishing-edited" => "publishing",
            "published" | "published-edited" => "published",
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
                kind: transfer::TransferKind::Move,
                sources: vec![source],
                destination: outgoing,
            },
            &cancellation,
            |_| transfer::TransferReply::Continue,
        )
        .unwrap();
    panic!("whole-entry Move boundary was not reached");
}

#[test]
fn restart_refuses_replacement_generations_and_inconsistent_publication_facts() {
    for replaced in [
        "payload",
        "source-parent",
        "destination-parent",
        "stage",
        "published",
        "published-scalar",
        "role-less",
    ] {
        let root = tempfile::Builder::new()
            .prefix("canopa-entry-move-restart-")
            .tempdir()
            .unwrap();
        let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let published = replaced.starts_with("published");
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::entry_move_boundary::entry_move_crash_helper",
                ])
                .env("CANOPA_ENTRY_MOVE_ROOT", root.path())
                .env(
                    "CANOPA_ENTRY_MOVE_POINT",
                    if published { "published" } else { "publishing" },
                )
                .env("CANOPA_ENTRY_MOVE_SHAPE", "file")
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let marker: Marker =
                serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap())
                    .unwrap();
            let before = fs::symlink_metadata(&marker.payload).unwrap();
            let incoming = root.path().join("incoming");
            let outgoing = root.path().join("outgoing");
            let parked = root.path().join("parked");
            let connection = rusqlite::Connection::open(root.path().join("history")).unwrap();
            let body: String = connection
                .query_row("SELECT body FROM pending WHERE singleton=1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            let mut body: serde_json::Value = serde_json::from_str(&body).unwrap();
            let stage_name = if published {
                let pending: Pending = serde_json::from_value(body.clone()).unwrap();
                let Pending::Forward {
                    stage: Some(stage), ..
                } = pending
                else {
                    panic!("missing Move claim");
                };
                stage.directory.file_name().unwrap().to_owned()
            } else {
                marker
                    .payload
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_owned()
            };
            let (original, replacement) = match replaced {
                "payload" | "published" => {
                    fs::rename(&marker.payload, &parked).unwrap();
                    (parked.clone(), Some(marker.payload.clone()))
                }
                "published-scalar" => (marker.payload.clone(), Some(root.path().join("decoy"))),
                "role-less" => (marker.payload.clone(), Some(outgoing.join(leaf()))),
                "source-parent" => {
                    fs::rename(&incoming, &parked).unwrap();
                    fs::create_dir(&incoming).unwrap();
                    fs::write(incoming.join("foreign"), b"Unselected source folder data").unwrap();
                    (marker.payload.clone(), None)
                }
                "destination-parent" => {
                    fs::rename(&outgoing, &parked).unwrap();
                    fs::create_dir(&outgoing).unwrap();
                    fs::create_dir(outgoing.join(&stage_name)).unwrap();
                    (
                        parked.join(&stage_name).join("folder"),
                        Some(outgoing.join(&stage_name).join("folder")),
                    )
                }
                "stage" => {
                    fs::rename(outgoing.join(&stage_name), &parked).unwrap();
                    fs::create_dir(outgoing.join(&stage_name)).unwrap();
                    (
                        parked.join("folder"),
                        Some(outgoing.join(&stage_name).join("folder")),
                    )
                }
                _ => unreachable!(),
            };
            if let Some(replacement) = &replacement {
                fs::write(replacement, b"Original file bytes").unwrap();
                fs::set_permissions(replacement, before.permissions()).unwrap();
                fs::File::open(replacement)
                    .unwrap()
                    .set_times(fs::FileTimes::new().set_modified(before.modified().unwrap()))
                    .unwrap();
                let identity = serde_json::to_value(Identity::read(replacement).unwrap()).unwrap();
                body["change"]["identity"] = identity.clone();
                body["stage"]["payload_identity"] = identity.clone();
                body["change"]["move_facts"]["version"]["identity"] = identity.clone();
                body["stage"]["entry_move"]["facts"]["version"]["identity"] = identity;
                let version: fingerprint::Version =
                    serde_json::from_value(body["change"]["move_facts"]["version"].clone())
                        .unwrap();
                assert!(
                    version
                        .matches_opened(&fs::File::open(replacement).unwrap(), true)
                        .unwrap()
                );
            }
            if replaced == "source-parent" {
                let identity = serde_json::to_value(Identity::read(&incoming).unwrap()).unwrap();
                body["change"]["move_facts"]["source_parent"] = identity.clone();
                body["stage"]["entry_move"]["facts"]["source_parent"] = identity;
            }
            if replaced == "destination-parent" {
                let identity = serde_json::to_value(Identity::read(&outgoing).unwrap()).unwrap();
                body["change"]["move_facts"]["destination_parent"] = identity.clone();
                body["stage"]["entry_move"]["facts"]["destination_parent"] = identity.clone();
                body["stage"]["parent_identity"] = identity;
            }
            if matches!(replaced, "stage" | "destination-parent") {
                body["stage"]["identity"] =
                    serde_json::to_value(Identity::read(&outgoing.join(&stage_name)).unwrap())
                        .unwrap();
            }
            if replaced == "role-less" {
                body["stage"] = serde_json::Value::Null;
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
                    "{replaced}"
                );
                assert_eq!(
                    engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                    0,
                    "{replaced}"
                );
                assert!(marker.identity.matches(&original), "{replaced}");
                assert_eq!(fs::read(&original).unwrap(), b"Original file bytes");
                if let Some(replacement) = &replacement {
                    assert_eq!(fs::read(replacement).unwrap(), b"Original file bytes");
                }
                if replaced == "source-parent" {
                    assert_eq!(
                        fs::read(incoming.join("foreign")).unwrap(),
                        b"Unselected source folder data"
                    );
                }
            }
        }));
        if let Err(panic) = checked {
            eprintln!(
                "whole-entry generation fixture {replaced} retained at {}",
                root.keep().display()
            );
            std::panic::resume_unwind(panic);
        }
    }
}

#[test]
fn a_same_filesystem_move_preserves_a_replaced_source_parent() {
    replaced_parent(true);
}

#[test]
fn a_same_filesystem_move_preserves_a_replaced_destination_parent() {
    replaced_parent(false);
}

#[test]
fn admitted_move_keeps_its_original_parent_even_when_a_new_parent_links_the_same_file() {
    let root = tempfile::Builder::new()
        .prefix("canopa-admitted-move-parent-")
        .tempdir()
        .unwrap();
    let incoming = root.path().join("incoming");
    let outgoing = root.path().join("outgoing");
    let parked = root.path().join("original-parent");
    fs::create_dir(&incoming).unwrap();
    fs::create_dir(&outgoing).unwrap();
    let source = incoming.join("item");
    fs::write(&source, b"Original observed object").unwrap();
    let identity = Identity::read(&source).unwrap();
    let request = transfer::TransferRequest {
        kind: transfer::TransferKind::Move,
        sources: vec![source.clone()],
        destination: outgoing.clone(),
    };
    let observed = request.observe_sources().unwrap();
    fs::rename(&incoming, &parked).unwrap();
    fs::create_dir(&incoming).unwrap();
    fs::hard_link(parked.join("item"), &source).unwrap();
    assert!(identity.matches(&source));
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let result = engine.transfer_observed_with_check(
        &observed,
        &Cancellation::default(),
        |_, _| Ok(()),
        |_| transfer::TransferReply::Continue,
    );
    if !identity.matches(&source) || outgoing.join("item").exists() {
        panic!(
            "admitted Move adopted a replacement source folder with a matching file identity: {result:?}; retained {}",
            root.keep().display()
        );
    }
    assert!(identity.matches(&parked.join("item")));
    assert_eq!(fs::read(&source).unwrap(), b"Original observed object");
}

#[test]
fn whole_entry_selection_requires_named_operands_before_opening_them() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::write(&source, b"Private selection").unwrap();
    for invalid in ["/", "", ".", ".."] {
        assert!(entry_move::Selection::open(Path::new(invalid), &destination).is_err());
        assert!(entry_move::Selection::open(&source, Path::new(invalid)).is_err());
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

fn replaced_parent(source_parent: bool) {
    let root = tempfile::Builder::new()
        .prefix("canopa-entry-move-boundary-")
        .tempdir()
        .unwrap();
    let incoming = root.path().join("incoming");
    let outgoing = root.path().join("outgoing");
    fs::create_dir(&incoming).unwrap();
    fs::create_dir(&outgoing).unwrap();
    let source = incoming.join("item");
    let destination = outgoing.join("item");
    fs::write(&source, b"Original selected file").unwrap();
    let selected = fs::File::open(&source).unwrap();
    let identity = Identity::from_metadata(&selected.metadata().unwrap());
    let replaced = if source_parent { &incoming } else { &outgoing }.clone();
    let parked = root.path().join("parked");
    let parked_at_gate = parked.clone();
    let _reset = rename_boundary::install(move |_, _| {
        fs::rename(&replaced, &parked_at_gate).unwrap();
        fs::create_dir(&replaced).unwrap();
        fs::write(replaced.join("sentinel"), b"Foreign folder").unwrap();
        if source_parent {
            fs::write(replaced.join("item"), b"Unselected replacement").unwrap();
        }
    });
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let result = engine.transfer(
        &transfer::TransferRequest {
            kind: transfer::TransferKind::Move,
            sources: vec![source.clone()],
            destination: outgoing.clone(),
        },
        &Cancellation::default(),
        |_| transfer::TransferReply::Continue,
    );
    let original = if source_parent {
        parked.join("item")
    } else {
        source.clone()
    };
    let preserved = identity.matches(&original)
        && fs::read(&original).ok().as_deref() == Some(b"Original selected file")
        && !destination.exists()
        && (!source_parent || fs::read(&source).ok().as_deref() == Some(b"Unselected replacement"));
    if !preserved {
        panic!(
            "same-filesystem Move adopted a replacement parent (source={source_parent}): {result:?}; retained {}",
            root.keep().display()
        );
    }
    if let Ok(outcome) = result {
        assert_eq!(outcome.completed, 0);
    }
    drop(engine);
    let _reopened = OperationEngine::open(root.path().join("history")).unwrap();
    assert!(identity.matches(&original));
    assert!(!destination.exists());
}
