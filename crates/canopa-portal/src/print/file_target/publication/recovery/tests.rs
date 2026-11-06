use super::*;
use crate::print::file_target::Reviewed;
use std::{
    fs,
    os::unix::{
        fs::{PermissionsExt, symlink},
        net::UnixStream,
    },
};

fn pin(path: &Path) -> Pinned {
    let reviewed = Reviewed::open(path).unwrap();
    let (send, receive) = UnixStream::pair().unwrap();
    reviewed.send(&send).unwrap();
    drop(send);
    Pinned::receive(&receive, reviewed.selection().clone()).unwrap()
}

struct Fixture {
    folder: tempfile::TempDir,
    store: RecoveryStore,
    publication: Publication,
}
impl Fixture {
    fn new(replace: bool) -> Self {
        let folder = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(&folder.path().join("state/print")).unwrap();
        let target = folder.path().join("output.pdf");
        if replace {
            fs::write(&target, b"previous bytes").unwrap();
        }
        let pin = pin(&target);
        let preparation = Preparation::new(&pin).unwrap();
        store.begin(&preparation, std::process::id()).unwrap();
        let publication = Publication::prepare_with(pin, preparation).unwrap();
        store
            .active(publication.info(), std::process::id())
            .unwrap();
        Self {
            folder,
            store,
            publication,
        }
    }
    fn stage(&self) {
        self.publication
            .document
            .write_all_at(b"%PDF-1.7\ncomplete", 0)
            .unwrap();
    }
    fn interrupted(&self) {
        self.store
            .change(false, |catalog| {
                // Simulate persisted identities from a former boot without relying
                // on arbitrary live host PIDs or killing unrelated processes.
                catalog.entries[0].owner.fixture_retire();
                catalog.entries[0].worker.fixture_retire();
                Ok(())
            })
            .unwrap();
    }
    fn item(&self) -> RecoveryItem {
        self.store.list().unwrap().remove(0)
    }
}

#[test]
fn listing_unconfigured_state_does_not_create_it() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("missing/state");
    assert!(
        RecoveryStore::new(&path)
            .unwrap()
            .list()
            .unwrap()
            .is_empty()
    );
    assert!(!path.exists());
}

#[test]
fn intent_precedes_setup_and_a_live_owner_or_worker_remains_busy() {
    let fixture = Fixture::new(false);
    assert_eq!(fixture.item().status, RecoveryStatus::Busy);
    fixture
        .store
        .change(false, |catalog| {
            catalog.entries[0].worker.fixture_retire();
            Ok(())
        })
        .unwrap();
    assert_eq!(fixture.item().status, RecoveryStatus::Busy);
    fixture.interrupted();
    let item = fixture.item();
    assert_eq!(item.status, RecoveryStatus::Incomplete);
    assert!(!item.document && !item.displaced);
}

#[test]
fn a_preparation_that_never_created_a_directory_stays_visible() {
    let folder = tempfile::tempdir().unwrap();
    let store = RecoveryStore::new(&folder.path().join("state")).unwrap();
    let preparation = Preparation::new(&pin(&folder.path().join("output.pdf"))).unwrap();
    store.begin(&preparation, std::process::id()).unwrap();
    store
        .change(false, |catalog| {
            catalog.entries[0].worker.fixture_retire();
            catalog.entries[0].owner.fixture_retire();
            Ok(())
        })
        .unwrap();
    assert_eq!(store.list().unwrap()[0].status, RecoveryStatus::Missing);
}

#[test]
fn a_partial_document_is_never_offered_as_a_complete_copy() {
    let fixture = Fixture::new(false);
    fixture.stage();
    fixture.interrupted();
    assert_eq!(fixture.item().status, RecoveryStatus::Incomplete);
    assert!(!fixture.item().document);
    assert!(!fixture.folder.path().join("output.pdf").exists());
}

#[test]
fn publishing_uncertainty_preserves_both_bodies_without_replaying_the_destination() {
    let fixture = Fixture::new(true);
    fixture.stage();
    let error = fixture
        .publication
        .commit_at(&mut || Ok(()), &mut |phase| {
            if phase == Phase::Publishing {
                Err(io::Error::other("interrupted"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "interrupted");
    fixture.interrupted();
    let item = fixture.item();
    assert_eq!(item.status, RecoveryStatus::Uncertain);
    assert!(item.document && item.displaced);
    assert!(!fixture.folder.path().join("output.pdf").exists());
    assert_eq!(
        fs::read(PathBuf::from(OsString::from_vec(item.directory)).join(PREVIOUS)).unwrap(),
        b"previous bytes"
    );
}

#[test]
fn published_classification_does_not_consume_retained_files() {
    let fixture = Fixture::new(false);
    fixture.stage();
    fixture.publication.commit(|| Ok(())).unwrap();
    fixture.interrupted();
    let item = fixture.item();
    assert_eq!(item.status, RecoveryStatus::Published);
    assert!(item.document);
    assert_eq!(
        fs::read(fixture.folder.path().join("output.pdf")).unwrap(),
        b"%PDF-1.7\ncomplete"
    );
    assert!(
        PathBuf::from(OsString::from_vec(item.directory))
            .join(DOCUMENT)
            .exists()
    );
}

#[test]
fn changed_payload_or_replaced_evidence_cannot_be_offered_for_export() {
    for marker in [false, true] {
        let fixture = Fixture::new(false);
        fixture.stage();
        fixture.publication.commit(|| Ok(())).unwrap();
        let directory = PathBuf::from(OsString::from_vec(fixture.publication.recovery()));
        if marker {
            fs::rename(directory.join(PAYLOAD), directory.join("saved-marker")).unwrap();
            fs::write(directory.join(PAYLOAD), b"unrelated").unwrap();
        } else {
            fixture
                .publication
                .document
                .write_all_at(b"changed", 0)
                .unwrap();
        }
        fixture.interrupted();
        let item = fixture.item();
        assert!(!item.document);
        if marker {
            assert_eq!(item.status, RecoveryStatus::Unavailable);
        }
    }
}

#[test]
fn completion_digest_covers_the_exact_document_and_preserves_offsets() {
    use std::io::{Seek, SeekFrom};
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(b"%PDF-1.7\ncomplete").unwrap();
    file.seek(SeekFrom::Start(4)).unwrap();
    let payload = digest(&file, &mut || Ok(())).unwrap();
    assert_eq!(
        payload.sha256,
        <[u8; 32]>::from(Sha256::digest(b"%PDF-1.7\ncomplete"))
    );
    assert_eq!(file.stream_position().unwrap(), 4);
    assert!(digest(&file, &mut || Err(io::Error::other("cancelled"))).is_err());
}

#[test]
fn moved_work_and_substituted_symlinks_are_reported_without_following_them() {
    for link in [false, true] {
        let fixture = Fixture::new(false);
        let directory = PathBuf::from(OsString::from_vec(fixture.publication.recovery()));
        let moved = fixture.folder.path().join("moved");
        fs::rename(&directory, &moved).unwrap();
        if link {
            symlink(&moved, &directory).unwrap();
        }
        fixture.interrupted();
        assert_eq!(
            fixture.item().status,
            if link {
                RecoveryStatus::Unavailable
            } else {
                RecoveryStatus::Missing
            }
        );
        assert!(moved.join(RECORD).exists());
    }
}

#[test]
fn successful_settlement_removes_the_index_but_retention_keeps_it() {
    for retained in [false, true] {
        let fixture = Fixture::new(false);
        fixture.stage();
        fixture.publication.commit(|| Ok(())).unwrap();
        if retained {
            let directory = PathBuf::from(OsString::from_vec(fixture.publication.recovery()));
            fs::write(directory.join("foreign"), b"keep me").unwrap();
        }
        let disposition = fixture.publication.settle(true).unwrap();
        fixture
            .store
            .settled(fixture.publication.info(), std::process::id(), &disposition)
            .unwrap();
        assert_eq!(fixture.store.list().unwrap().len(), usize::from(retained));
        if retained {
            fixture
                .store
                .change(false, |catalog| {
                    catalog.entries[0].worker.fixture_retire();
                    Ok(())
                })
                .unwrap();
            assert_eq!(fixture.item().status, RecoveryStatus::Published);
        }
    }
}

#[test]
fn foreign_marker_is_preserved_during_settlement() {
    let fixture = Fixture::new(false);
    fixture.stage();
    fixture.publication.commit(|| Ok(())).unwrap();
    let directory = PathBuf::from(OsString::from_vec(fixture.publication.recovery()));
    let held = File::open(directory.join(PAYLOAD)).unwrap();
    fs::remove_file(directory.join(PAYLOAD)).unwrap();
    fs::write(directory.join(PAYLOAD), b"foreign").unwrap();
    let disposition = fixture.publication.settle(true).unwrap();
    assert!(disposition.published && disposition.recovery.is_some());
    assert_eq!(fs::read(directory.join(PAYLOAD)).unwrap(), b"foreign");
    drop(held);
}

#[test]
fn catalog_is_bounded_and_corruption_does_not_authorize_new_work() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("state");
    let store = RecoveryStore::new(&path).unwrap();
    for _ in 0..NORMAL_ENTRIES {
        store
            .begin(
                &Preparation::new(&pin(&folder.path().join("output.pdf"))).unwrap(),
                std::process::id(),
            )
            .unwrap();
    }
    assert_eq!(store.list().unwrap().len(), NORMAL_ENTRIES);
    assert!(
        store
            .begin(
                &Preparation::new(&pin(&folder.path().join("output.pdf"))).unwrap(),
                std::process::id()
            )
            .is_err()
    );
    fs::write(path.join("catalog.json"), b"broken").unwrap();
    assert!(store.list().is_err());
    assert!(
        store
            .begin(
                &Preparation::new(&pin(&folder.path().join("output.pdf"))).unwrap(),
                std::process::id()
            )
            .is_err()
    );
    assert!(!folder.path().join("output.pdf").exists());
}

#[test]
fn insecure_or_linked_catalog_objects_are_rejected_and_preserved() {
    for mode in [0, 1, 2, 3] {
        let fixture = Fixture::new(false);
        let state = fixture.folder.path().join("state/print");
        match mode {
            0 => fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).unwrap(),
            1 => fs::set_permissions(
                state.join("catalog.json"),
                fs::Permissions::from_mode(0o644),
            )
            .unwrap(),
            2 => {
                fs::rename(state.join("catalog.json"), state.join("original")).unwrap();
                symlink("original", state.join("catalog.json")).unwrap();
            }
            _ => fs::hard_link(state.join("catalog.json"), state.join("linked")).unwrap(),
        }
        assert!(fixture.store.list().is_err());
        assert!(state.join("catalog.json").exists());
    }
}

#[test]
fn a_busy_catalog_lock_fails_promptly_and_stale_scratch_is_bounded() {
    let fixture = Fixture::new(false);
    let directory = fixture.store.open(false).unwrap().unwrap();
    let lock = read_only(&directory, "catalog.lock").unwrap();
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).unwrap();
    let started = std::time::Instant::now();
    assert!(fixture.store.change(false, |_| Ok(())).is_err());
    assert!(fixture.store.list().is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    drop(lock);
    fresh(&directory, "catalog.next")
        .unwrap()
        .write_all(b"abandoned")
        .unwrap();
    fixture.store.change(false, |_| Ok(())).unwrap();
    assert!(
        any(&directory, OsStr::new("catalog.next"))
            .unwrap()
            .is_none()
    );
    symlink(
        "catalog.json",
        fixture.folder.path().join("state/print/catalog.next"),
    )
    .unwrap();
    assert!(fixture.store.change(false, |_| Ok(())).is_err());
    assert!(
        fixture
            .folder
            .path()
            .join("state/print/catalog.next")
            .is_symlink()
    );
}

#[test]
fn a_retired_owner_or_missing_lock_never_permits_reactivation() {
    let fixture = Fixture::new(false);
    fixture.interrupted();
    assert!(
        fixture
            .store
            .active(fixture.publication.info(), std::process::id())
            .is_err()
    );
    fs::remove_file(fixture.folder.path().join("state/print/catalog.lock")).unwrap();
    assert!(fixture.store.list().is_err());
    assert!(!fixture.folder.path().join("output.pdf").exists());
}

fn uncertain() -> Fixture {
    let fixture = Fixture::new(true);
    fixture.stage();
    fixture
        .publication
        .commit_at(&mut || Ok(()), &mut |phase| {
            if phase == Phase::Publishing {
                Err(io::Error::other("interrupted"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    fixture.interrupted();
    fixture
}

fn recovered(fixture: &Fixture, body: RecoveryBody) -> anyhow::Result<File> {
    let (_send, receive) = UnixStream::pair()?;
    let cancel =
        crate::print::worker::Cancellation::fixture(std::os::fd::OwnedFd::from(receive).into());
    let target = File::from(rustix::fs::memfd_create(
        c"recovery-test",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )?);
    fixture
        .store
        .snapshot(&fixture.item().id, body, target.try_clone()?, &cancel)?;
    crate::print::worker::document::verify_recovery(&target)?;
    Ok(target)
}

#[test]
fn verified_snapshots_are_sealed_and_do_not_consume_either_retained_body() {
    let fixture = uncertain();
    for (body, expected) in [
        (RecoveryBody::Document, b"%PDF-1.7\ncomplete".as_slice()),
        (RecoveryBody::Displaced, b"previous bytes".as_slice()),
    ] {
        let target = recovered(&fixture, body).unwrap();
        let mut bytes = vec![0; expected.len()];
        target.read_exact_at(&mut bytes, 0).unwrap();
        assert_eq!(bytes, expected);
        assert!(target.write_all_at(b"changed", 0).is_err());
    }
    assert_eq!(fixture.store.list().unwrap().len(), 1);
    let work = PathBuf::from(OsString::from_vec(fixture.item().directory));
    assert_eq!(fs::read(work.join(PREVIOUS)).unwrap(), b"previous bytes");
    assert_eq!(
        fs::read(work.join(DOCUMENT)).unwrap(),
        b"%PDF-1.7\ncomplete"
    );
}

#[test]
fn a_completion_hash_mismatch_is_rejected_even_when_length_and_mtime_match() {
    let fixture = uncertain();
    let before = fixture.publication.document.metadata().unwrap();
    fixture.publication.document.write_all_at(b"!", 0).unwrap();
    let times = [
        libc::timespec {
            tv_sec: 0,
            tv_nsec: libc::UTIME_OMIT,
        },
        libc::timespec {
            tv_sec: before.mtime(),
            tv_nsec: before.mtime_nsec(),
        },
    ];
    // SAFETY: the owned descriptor and the two initialized timestamps are valid.
    assert_eq!(
        unsafe { libc::futimens(fixture.publication.document.as_raw_fd(), times.as_ptr()) },
        0
    );
    assert!(fixture.item().document, "metadata alone can still match");
    assert!(
        recovered(&fixture, RecoveryBody::Document)
            .unwrap_err()
            .to_string()
            .contains("completion check")
    );
    assert!(!fixture.folder.path().join("output.pdf").exists());
}

#[test]
fn empty_displaced_files_and_empty_recovery_publications_remain_distinct_from_print_jobs() {
    let fixture = uncertain();
    let work = PathBuf::from(OsString::from_vec(fixture.item().directory));
    File::options()
        .write(true)
        .open(work.join(PREVIOUS))
        .unwrap()
        .set_len(0)
        .unwrap();
    let empty = recovered(&fixture, RecoveryBody::Displaced).unwrap();
    assert_eq!(empty.metadata().unwrap().len(), 0);
    assert!(crate::print::worker::document::verify(&empty).is_err());
    let publication = Fixture::new(false);
    assert!(publication.publication.commit(|| Ok(())).is_err());
    publication.publication.commit_recovery(|| Ok(())).unwrap();
    publication.interrupted();
    assert!(publication.item().document);
    assert_eq!(
        recovered(&publication, RecoveryBody::Document)
            .unwrap()
            .metadata()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn dismissal_refuses_busy_work_and_never_removes_retained_files() {
    let fixture = Fixture::new(false);
    let id = fixture.item().id;
    assert!(fixture.store.dismiss(&id).is_err());
    fixture.interrupted();
    let directory = PathBuf::from(OsString::from_vec(fixture.item().directory));
    fixture.store.dismiss(&id).unwrap();
    assert!(fixture.store.list().unwrap().is_empty());
    assert!(directory.join(DOCUMENT).exists() && directory.join(RECORD).exists());
}

#[test]
fn a_missing_document_does_not_hide_the_displaced_file() {
    let fixture = uncertain();
    let work = PathBuf::from(OsString::from_vec(fixture.item().directory));
    fs::remove_file(work.join(DOCUMENT)).unwrap();
    assert!(!fixture.item().document && fixture.item().displaced);
    let snapshot = recovered(&fixture, RecoveryBody::Displaced).unwrap();
    let mut bytes = [0; 14];
    snapshot.read_exact_at(&mut bytes, 0).unwrap();
    assert_eq!(&bytes, b"previous bytes");
}

#[test]
fn the_source_work_directory_is_never_an_export_destination() {
    let fixture = uncertain();
    let protection = RecoveryProtection {
        directory: fixture.publication.info.directory,
    };
    let work = PathBuf::from(OsString::from_vec(fixture.item().directory));
    assert!(!protection.allows(pin(&work.join(DOCUMENT)).selection()));
    assert!(!protection.allows(pin(&work.join("new-copy")).selection()));
    assert!(protection.allows(pin(&fixture.folder.path().join("safe-copy")).selection()));
    let preparation = Preparation::new(&pin(&work.join("another-copy"))).unwrap();
    assert!(
        fixture
            .store
            .begin_recovery(&preparation, std::process::id())
            .is_err()
    );
    assert!(
        fixture
            .store
            .begin(&preparation, std::process::id())
            .is_err()
    );
    assert_eq!(fixture.store.list().unwrap().len(), 1);
}

#[test]
fn recovery_can_use_the_two_reserved_catalog_entries_without_discarding_old_work() {
    let folder = tempfile::tempdir().unwrap();
    let store = RecoveryStore::new(&folder.path().join("state")).unwrap();
    for index in 0..MAX_ENTRIES {
        let preparation = Preparation::new(&pin(&folder.path().join("output"))).unwrap();
        if index < NORMAL_ENTRIES {
            store.begin(&preparation, std::process::id()).unwrap();
        } else {
            assert!(store.begin(&preparation, std::process::id()).is_err());
            store
                .begin_recovery(&preparation, std::process::id())
                .unwrap();
        }
    }
    assert_eq!(store.list().unwrap().len(), MAX_ENTRIES);
    assert!(
        store
            .begin_recovery(
                &Preparation::new(&pin(&folder.path().join("output"))).unwrap(),
                std::process::id()
            )
            .is_err()
    );
}

#[test]
fn a_known_moved_retained_location_is_saved_without_becoming_write_authority() {
    let fixture = uncertain();
    // Restore the actual fixture owner for a real retention update.
    fixture
        .store
        .change(false, |catalog| {
            catalog.entries[0].owner = Process::read(std::process::id())?;
            Ok(())
        })
        .unwrap();
    let original = PathBuf::from(OsString::from_vec(fixture.item().directory));
    let parent = fixture.folder.path().join("moved-parent");
    fs::create_dir(&parent).unwrap();
    let moved = parent.join(original.file_name().unwrap());
    fs::rename(&original, &moved).unwrap();
    fixture
        .store
        .settled(
            fixture.publication.info(),
            std::process::id(),
            &Disposition {
                published: false,
                uncertain: true,
                recovery: Some(moved.clone().into_os_string().into_vec()),
            },
        )
        .unwrap();
    assert_eq!(fixture.item().status, RecoveryStatus::Uncertain);
    assert!(recovered(&fixture, RecoveryBody::Document).is_ok());
    assert!(!fixture.folder.path().join("output.pdf").exists());
}

#[test]
#[ignore = "owned producer entrypoint for native_recovery_client_fixture"]
fn recovery_producer_fixture() {
    let root = PathBuf::from(std::env::var_os("CANOPA_PRINT_RECOVERY_ROOT").unwrap());
    let target = root.join("interrupted.pdf");
    let previous: &[u8] = if std::env::var_os("CANOPA_PRINT_RECOVERY_EMPTY").is_some() {
        b""
    } else {
        b"previous bytes"
    };
    fs::write(&target, previous).unwrap();
    let store = RecoveryStore::new(&root.join("state")).unwrap();
    let pin = pin(&target);
    let preparation = Preparation::new(&pin).unwrap();
    store.begin(&preparation, std::process::id()).unwrap();
    let publication = Publication::prepare_with(pin, preparation).unwrap();
    store
        .active(publication.info(), std::process::id())
        .unwrap();
    publication
        .document
        .write_all_at(b"%PDF-1.7\nretained after process loss", 0)
        .unwrap();
    publication
        .commit_at(&mut || Ok(()), &mut |phase| {
            if phase == Phase::Publishing {
                fs::write(root.join("ready"), b"ready")?;
                // SAFETY: only this explicitly spawned disposable fixture stops.
                unsafe {
                    libc::raise(libc::SIGSTOP);
                }
            }
            Ok(())
        })
        .unwrap();
    panic!("producer should have been killed at the publication boundary");
}

#[tokio::test]
#[ignore = "requires CANOPA_PRINT_BINARY and a private owned native fixture"]
async fn native_recovery_client_fixture() {
    use crate::{print::worker::Client, process::DialogProcess};
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    use tokio::sync::watch;
    let root = PathBuf::from(std::env::var_os("CANOPA_PRINT_RECOVERY_ROOT").unwrap());
    assert!(root.is_absolute() && root.is_dir());
    let previous: &[u8] = if std::env::var_os("CANOPA_PRINT_RECOVERY_EMPTY").is_some() {
        b""
    } else {
        b"previous bytes"
    };
    let store = RecoveryStore::new(&root.join("state")).unwrap();
    let client = Client::new(DialogProcess {
        executable: std::env::var_os("CANOPA_PRINT_BINARY").unwrap().into(),
        arguments: vec![],
    })
    .with_recovery(store);
    let (_cancel, cancelled) = watch::channel(false);
    assert!(
        client
            .recovery(cancelled.clone())
            .await
            .unwrap()
            .unwrap()
            .is_empty()
    );
    assert!(!root.join("state").exists());
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "print::file_target::publication::recovery::tests::recovery_producer_fixture",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let pid = child.id();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !root.join("ready").exists() && Instant::now() < deadline {
        assert!(child.try_wait().unwrap().is_none());
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let reached = root.join("ready").exists();
    if !reached {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("producer did not reach boundary");
    }
    let busy = client.recovery(cancelled.clone()).await.unwrap().unwrap();
    assert_eq!(busy.len(), 1);
    assert_eq!(busy[0].status, RecoveryStatus::Busy);
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    let entries = client.recovery(cancelled.clone()).await.unwrap().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].status, RecoveryStatus::Uncertain);
    assert!(entries[0].document && entries[0].displaced);
    assert!(!root.join("interrupted.pdf").exists());
    let work = PathBuf::from(OsString::from_vec(entries[0].directory.clone()));
    assert_eq!(fs::read(work.join(PREVIOUS)).unwrap(), previous);
    assert_eq!(
        fs::read(work.join(DOCUMENT)).unwrap(),
        b"%PDF-1.7\nretained after process loss"
    );
    let source = root.join("source.pdf");
    fs::write(&source, b"%PDF-1.7\nordinary complete output").unwrap();
    let frozen = client
        .snapshot(File::open(source).unwrap().into(), cancelled.clone())
        .await
        .unwrap()
        .unwrap();
    let target = root.join("normal.pdf");
    let disposition = client
        .publish_file(
            &frozen,
            pin(&target),
            crate::print::Format::Pdf,
            cancelled.clone(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(disposition.published && disposition.recovery.is_none());
    assert_eq!(
        fs::read(target).unwrap(),
        b"%PDF-1.7\nordinary complete output"
    );
    let left = root.join("concurrent-left.pdf");
    let right = root.join("concurrent-right.pdf");
    let (left_result, right_result) = tokio::join!(
        client.publish_file(
            &frozen,
            pin(&left),
            crate::print::Format::Pdf,
            cancelled.clone()
        ),
        client.publish_file(
            &frozen,
            pin(&right),
            crate::print::Format::Pdf,
            cancelled.clone()
        ),
    );
    assert!(left_result.unwrap().unwrap().published && right_result.unwrap().unwrap().published);
    assert_eq!(
        fs::read(left).unwrap(),
        b"%PDF-1.7\nordinary complete output"
    );
    assert_eq!(
        fs::read(right).unwrap(),
        b"%PDF-1.7\nordinary complete output"
    );
    let after = client.recovery(cancelled.clone()).await.unwrap().unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].id, entries[0].id);
    drop(frozen);
    client.idle().await.unwrap();
    let document = client
        .recover_snapshot(
            entries[0].id.clone(),
            RecoveryBody::Document,
            cancelled.clone(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(
        client
            .publish_recovered(&document, pin(&work.join(DOCUMENT)), cancelled.clone())
            .await
            .is_err()
    );
    let displaced = client
        .recover_snapshot(
            entries[0].id.clone(),
            RecoveryBody::Displaced,
            cancelled.clone(),
        )
        .await
        .unwrap()
        .unwrap();
    let document_copy = root.join("exported-document.pdf");
    let displaced_copy = root.join("exported-displaced.bin");
    assert!(
        client
            .publish_recovered(&document, pin(&document_copy), cancelled.clone())
            .await
            .unwrap()
            .unwrap()
            .published
    );
    assert!(
        client
            .publish_recovered(&displaced, pin(&displaced_copy), cancelled.clone())
            .await
            .unwrap()
            .unwrap()
            .published
    );
    assert_eq!(
        fs::read(document_copy).unwrap(),
        b"%PDF-1.7\nretained after process loss"
    );
    assert_eq!(fs::read(displaced_copy).unwrap(), previous);
    assert_eq!(
        client
            .recovery(cancelled.clone())
            .await
            .unwrap()
            .unwrap()
            .len(),
        1
    );
    assert!(
        client
            .dismiss_recovery(entries[0].id.clone(), cancelled.clone())
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        client
            .recovery(cancelled.clone())
            .await
            .unwrap()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fs::read(work.join(DOCUMENT)).unwrap(),
        b"%PDF-1.7\nretained after process loss"
    );
    assert_eq!(fs::read(work.join(PREVIOUS)).unwrap(), previous);
    assert!(!root.join("interrupted.pdf").exists());
    assert_eq!(client.shutdown().await, 0);
    fs::write(
        root.join("receipt.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "producer_pid": pid, "producer_terminal": true, "busy_before_kill": true,
            "status_after_confirmed_death": entries[0].status, "retained_id": entries[0].id,
            "normal_publication_removed_only_its_own_index": true,
            "two_simultaneous_publications_share_the_catalog": true,
            "verified_document_export": true, "opaque_or_empty_displaced_export": true,
            "dismissal_left_files_intact": true,
            "fresh_client_worker_discovery": true, "automatic_replay": false,
            "payload_and_displaced_bytes_preserved": true
        }))
        .unwrap(),
    )
    .unwrap();
}
