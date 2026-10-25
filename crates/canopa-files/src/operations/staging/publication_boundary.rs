//! Real public transfers killed on either side of the publication syscall.
use super::*;
use std::cell::RefCell;
use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt, process::ExitStatusExt};
type Hook = Box<dyn FnOnce(&Path, &Path)>;
thread_local! {
    static HOOK: RefCell<Option<(&'static str, Hook)>> = const { RefCell::new(None) };
}
pub(super) fn at_phase(phase: &str, target: &Path, payload: &Path) {
    let hook = HOOK.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|(point, _)| *point == phase) {
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
    stage: PathBuf,
    source: Identity,
    copy: Identity,
}
fn leaf() -> std::ffi::OsString {
    std::ffi::OsString::from_vec(b"item-\xfd".to_vec())
}
fn link() -> PathBuf {
    std::ffi::OsString::from_vec(b"../target-\xfe".to_vec()).into()
}
fn assert_contents(path: &Path, shape: &str, newer: bool) {
    if shape == "link" {
        assert_eq!(fs::read_link(path).unwrap(), link());
    } else {
        assert_eq!(
            fs::read(path).unwrap(),
            if newer {
                b"Newer completed bytes".as_slice()
            } else {
                b"Selected source bytes".as_slice()
            }
        );
    }
}
fn probe(case: &str, shape: &str) {
    for kind in ["copy", "cross"] {
        let root = tempfile::Builder::new()
            .prefix("canopa-publication-restart-")
            .tempdir()
            .unwrap();
        let output = if kind == "cross" {
            tempfile::Builder::new()
                .prefix(".canopa-publication-output-")
                .tempdir_in(std::env::current_dir().unwrap())
                .unwrap()
        } else {
            tempfile::Builder::new()
                .prefix("canopa-publication-output-")
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
            let point = if matches!(case, "edited-payload" | "prepared" | "occupied") {
                "before"
            } else {
                "after"
            };
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::staging::publication_boundary::publication_child",
                ])
                .env("CANOPA_PUBLICATION_ROOT", root.path())
                .env("CANOPA_PUBLICATION_OUTPUT", output.path())
                .env("CANOPA_PUBLICATION_KIND", kind)
                .env("CANOPA_PUBLICATION_POINT", point)
                .env("CANOPA_PUBLICATION_SHAPE", shape)
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let marker: Marker =
                serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap())
                    .unwrap();
            let destination = output.path().join("destination");
            let target = destination.join(leaf());
            let payload = marker.stage.join("folder");
            {
                let state = Store::open(&root.path().join("history")).unwrap();
                state.require_bound_forward().unwrap();
                let pending = state.pending().unwrap().unwrap();
                let Pending::Forward {
                    change:
                        Change::Copied {
                            identity,
                            fingerprint,
                            ..
                        },
                    stage: Some(stage),
                    ..
                } = &pending
                else {
                    panic!("expected an actual prepared Copy step");
                };
                assert_eq!(*identity, marker.copy);
                assert_eq!(stage.payload_identity, Some(marker.copy));
                assert!(stage.ownership.as_ref().unwrap().publication().is_some());
                assert!(
                    fingerprint
                        .matches(
                            if point == "before" { &payload } else { &target },
                            &Cancellation::default()
                        )
                        .unwrap()
                );
                fs::write(
                    root.path().join("pending-before.json"),
                    serde_json::to_vec_pretty(&pending).unwrap(),
                )
                .unwrap();
            }
            let parked = output.path().join("parked-destination");
            if case == "parent" {
                fs::rename(&destination, &parked).unwrap();
                fs::create_dir(&destination).unwrap();
                fs::hard_link(parked.join(leaf()), &target).unwrap();
                fs::create_dir(destination.join(marker.stage.file_name().unwrap())).unwrap();
                assert!(marker.copy.matches(&target));
            } else if case == "edited-payload" {
                fs::write(&payload, b"Newer staged work").unwrap();
                assert!(marker.copy.matches(&payload));
            } else if case == "newer-output" {
                fs::write(&target, b"Newer completed bytes").unwrap();
                fs::set_permissions(&target, fs::Permissions::from_mode(0o500)).unwrap();
            } else if case == "missing-stage" {
                fs::remove_dir(&marker.stage).unwrap();
            } else if case == "stage" {
                fs::rename(&marker.stage, output.path().join("parked-work")).unwrap();
                fs::create_dir(&marker.stage).unwrap();
                fs::write(
                    marker.stage.join("foreign-entry"),
                    b"Foreign work directory",
                )
                .unwrap();
            } else if case == "replaced-output" {
                fs::rename(&target, output.path().join("parked-output")).unwrap();
                fs::write(&target, b"Foreign output").unwrap();
            } else if case == "occupied" {
                fs::write(&target, b"Foreign output").unwrap();
            } else {
                assert!(matches!(case, "prepared" | "published"));
            }
            let completed = matches!(
                case,
                "prepared" | "published" | "newer-output" | "missing-stage"
            );
            let foreign = matches!(case, "occupied" | "replaced-output")
                .then(|| Identity::read(&target).unwrap());
            for _ in 0..2 {
                let engine = OperationEngine::open(root.path().join("history")).unwrap();
                assert_eq!(
                    engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                    usize::from(completed),
                    "{kind}/{shape}/{case}: incorrect recovery completion"
                );
                if completed {
                    assert!(marker.copy.matches(&target));
                    assert_contents(&target, shape, case == "newer-output");
                    if case == "newer-output" {
                        assert_eq!(
                            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                            0o500
                        );
                    }
                    assert!(!marker.stage.exists());
                } else if case == "edited-payload" {
                    assert!(
                        marker.copy.matches(&payload),
                        "recovery deleted newer prepared bytes"
                    );
                    assert_eq!(fs::read(&payload).unwrap(), b"Newer staged work");
                    assert!(!target.exists());
                } else if case == "parent" {
                    assert!(marker.copy.matches(&target));
                    assert!(marker.copy.matches(&parked.join(leaf())));
                    assert_contents(&target, shape, false);
                } else if case == "stage" {
                    assert!(marker.copy.matches(&target));
                    assert_contents(&target, shape, false);
                    assert_eq!(
                        fs::read(marker.stage.join("foreign-entry")).unwrap(),
                        b"Foreign work directory"
                    );
                } else {
                    assert!(foreign.unwrap().matches(&target));
                    assert_eq!(fs::read(&target).unwrap(), b"Foreign output");
                    if case == "occupied" {
                        assert!(marker.copy.matches(&payload));
                        assert_contents(&payload, shape, false);
                    } else {
                        assert!(marker.copy.matches(&output.path().join("parked-output")));
                    }
                }
                assert!(
                    marker
                        .source
                        .matches(&root.path().join("incoming").join(leaf()))
                );
                assert_contents(&root.path().join("incoming").join(leaf()), shape, false);
                assert_eq!(
                    engine
                        .interrupted_operations(&Cancellation::default())
                        .unwrap()
                        .is_empty(),
                    completed,
                    "{kind}/{shape}/{case}"
                );
            }
        }));
        if let Err(panic) = checked {
            eprintln!(
                "publication fixture {kind}/{shape}/{case} retained at {} and {}",
                root.keep().display(),
                output.keep().display()
            );
            std::panic::resume_unwind(panic);
        }
    }
}
#[test]
fn a_published_copy_cannot_adopt_a_new_parent_linking_the_original_output() {
    probe("parent", "file");
}
#[test]
fn newer_prepared_copy_bytes_survive_restart() {
    probe("edited-payload", "file");
}

#[test]
fn plain_copy_recovers_original_files_and_links_without_replaying_or_replacing_them() {
    for shape in ["file", "link"] {
        for case in [
            "prepared",
            "published",
            "newer-output",
            "missing-stage",
            "stage",
            "replaced-output",
            "occupied",
            "parent",
        ] {
            if case == "newer-output" && shape == "link" {
                continue;
            }
            probe(case, shape);
        }
    }
}

#[test]
#[ignore = "real transfer child killed at its actual prepared-copy publication boundary"]
fn publication_child() {
    let Some(root) = std::env::var_os("CANOPA_PUBLICATION_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-publication-restart-")
    );
    let output = PathBuf::from(std::env::var_os("CANOPA_PUBLICATION_OUTPUT").unwrap());
    let incoming = root.join("incoming");
    fs::create_dir(&incoming).unwrap();
    let source = incoming.join(leaf());
    if std::env::var("CANOPA_PUBLICATION_SHAPE").unwrap_or_else(|_| "file".into()) == "link" {
        std::os::unix::fs::symlink(link(), &source).unwrap();
    } else {
        fs::write(&source, b"Selected source bytes").unwrap();
    }
    let source_id = Identity::read(&source).unwrap();
    let destination = output.join("destination");
    fs::create_dir(&destination).unwrap();
    let point = std::env::var("CANOPA_PUBLICATION_POINT").unwrap();
    let phase = if point == "before" { "before" } else { "after" };
    HOOK.with(|slot| {
        *slot.borrow_mut() = Some((
            phase,
            Box::new(move |target, payload| {
                fs::write(
                    root.join("marker.json"),
                    serde_json::to_vec(&Marker {
                        stage: payload.parent().unwrap().to_owned(),
                        source: source_id,
                        copy: Identity::read(if phase == "before" { payload } else { target })
                            .unwrap(),
                    })
                    .unwrap(),
                )
                .unwrap();
                rustix::process::kill_process(
                    rustix::process::getpid(),
                    rustix::process::Signal::KILL,
                )
                .unwrap();
                unreachable!();
            }),
        ))
    });
    let engine = OperationEngine::open(
        PathBuf::from(std::env::var_os("CANOPA_PUBLICATION_ROOT").unwrap()).join("history"),
    )
    .unwrap();
    engine
        .transfer(
            &transfer::TransferRequest {
                kind: if std::env::var("CANOPA_PUBLICATION_KIND").unwrap() == "copy" {
                    transfer::TransferKind::Copy
                } else {
                    transfer::TransferKind::Move
                },
                sources: vec![source],
                destination,
            },
            &Cancellation::default(),
            |_| transfer::TransferReply::Continue,
        )
        .unwrap();
    panic!("publication boundary was not reached");
}
