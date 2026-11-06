use super::super::Reviewed;
use super::*;
use std::{
    fs,
    os::unix::{fs::symlink, net::UnixStream},
};

fn prepare(path: &std::path::Path) -> Publication {
    let review = Reviewed::open(path).unwrap();
    let (send, receive) = UnixStream::pair().unwrap();
    review.send(&send).unwrap();
    drop(send);
    let pin = Pinned::receive(&receive, review.selection().clone()).unwrap();
    Publication::prepare(pin).unwrap()
}
fn stage(publication: &Publication) {
    publication
        .document
        .write_all_at(b"%PDF-1.7\nnew document", 0)
        .unwrap();
}
fn stopped() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "injected interruption")
}
fn work(publication: &Publication) -> PathBuf {
    PathBuf::from(OsString::from_vec(publication.recovery()))
}

#[test]
fn a_new_document_is_published_complete_with_private_permissions_and_no_leftovers() {
    let folder = tempfile::tempdir().unwrap();
    let target = folder.path().join("new.pdf");
    let publication = prepare(&target);
    let directory = work(&publication);
    assert!(!target.exists());
    stage(&publication);
    publication.commit(|| Ok(())).unwrap();
    let result = publication.settle(true).unwrap();
    assert!(result.published && !result.uncertain && result.recovery.is_none());
    assert_eq!(fs::read(&target).unwrap(), b"%PDF-1.7\nnew document");
    assert_eq!(fs::metadata(target).unwrap().mode() & 0o777, 0o600);
    assert!(!directory.exists());
}

#[test]
fn replacing_the_reviewed_name_does_not_modify_old_hardlinks_or_open_files() {
    let folder = tempfile::tempdir().unwrap();
    let target = folder.path().join("existing.pdf");
    let link = folder.path().join("original.pdf");
    fs::write(&target, b"original bytes").unwrap();
    fs::hard_link(&target, &link).unwrap();
    let old = File::open(&target).unwrap();
    let publication = prepare(&target);
    stage(&publication);
    publication.commit(|| Ok(())).unwrap();
    let result = publication.settle(true).unwrap();
    assert!(result.published && result.recovery.is_none());
    assert_eq!(fs::read(&target).unwrap(), b"%PDF-1.7\nnew document");
    assert_eq!(fs::read(link).unwrap(), b"original bytes");
    let mut bytes = [0; 14];
    old.read_exact_at(&mut bytes, 0).unwrap();
    assert_eq!(&bytes, b"original bytes");
}

#[test]
fn a_late_file_or_symlink_never_becomes_replacement_consent() {
    for link in [false, true] {
        let folder = tempfile::tempdir().unwrap();
        let target = folder.path().join("new.pdf");
        let other = folder.path().join("other.pdf");
        fs::write(&other, b"keep").unwrap();
        let publication = prepare(&target);
        stage(&publication);
        if link {
            symlink(&other, &target).unwrap();
        } else {
            fs::write(&target, b"late").unwrap();
        }
        assert!(publication.commit(|| Ok(())).is_err());
        assert!(!publication.settle(false).unwrap().published);
        assert_eq!(fs::read(other).unwrap(), b"keep");
        assert_eq!(
            fs::read(target).unwrap(),
            if link { b"keep" } else { b"late" }
        );
    }
}

#[test]
fn cancellation_before_commit_discards_the_stage_and_after_claim_restores_the_original() {
    for at in [Phase::Ready, Phase::Claimed] {
        let folder = tempfile::tempdir().unwrap();
        let target = folder.path().join("existing.pdf");
        fs::write(&target, b"original").unwrap();
        let publication = prepare(&target);
        stage(&publication);
        let claimed = std::cell::Cell::new(false);
        assert!(
            publication
                .commit_at(
                    &mut || if at == Phase::Ready || claimed.get() {
                        Err(stopped())
                    } else {
                        Ok(())
                    },
                    &mut |phase| {
                        if phase == Phase::Claimed {
                            claimed.set(true);
                        }
                        Ok(())
                    },
                )
                .is_err()
        );
        assert_eq!(claimed.get(), at == Phase::Claimed);
        let result = publication.settle(false).unwrap();
        assert!(!result.published && !result.uncertain && result.recovery.is_none());
        assert_eq!(fs::read(&target).unwrap(), b"original");
        assert!(!work(&publication).exists());
    }
}

#[test]
fn an_unreviewed_entry_caught_by_the_claim_is_restored_instead_of_deleted() {
    let folder = tempfile::tempdir().unwrap();
    let target = folder.path().join("existing.pdf");
    fs::write(&target, b"reviewed").unwrap();
    let publication = prepare(&target);
    stage(&publication);
    assert!(
        publication
            .commit_at(&mut || Ok(()), &mut |phase| {
                if phase == Phase::Claiming {
                    fs::rename(&target, folder.path().join("reviewed.pdf"))?;
                    fs::write(&target, b"unreviewed")?;
                }
                Ok(())
            })
            .is_err()
    );
    let result = publication.settle(false).unwrap();
    assert!(!result.published && result.recovery.is_none());
    assert_eq!(fs::read(target).unwrap(), b"unreviewed");
    assert_eq!(
        fs::read(folder.path().join("reviewed.pdf")).unwrap(),
        b"reviewed"
    );
}

#[test]
fn a_conflict_after_claim_preserves_both_the_late_file_and_saved_original() {
    let folder = tempfile::tempdir().unwrap();
    let target = folder.path().join("existing.pdf");
    fs::write(&target, b"reviewed").unwrap();
    let publication = prepare(&target);
    stage(&publication);
    assert!(
        publication
            .commit_at(&mut || Ok(()), &mut |phase| {
                if phase == Phase::Claimed {
                    fs::write(&target, b"late occupant")?;
                }
                Ok(())
            })
            .is_err()
    );
    let result = publication.settle(false).unwrap();
    assert!(!result.published && result.uncertain && result.recovery.is_some());
    assert_eq!(fs::read(target).unwrap(), b"late occupant");
    assert_eq!(
        fs::read(work(&publication).join(PREVIOUS)).unwrap(),
        b"reviewed"
    );
    assert!(work(&publication).join(RECORD).exists());
}

#[test]
fn a_lost_reply_after_link_is_reconciled_but_a_lost_commit_is_not_replayed() {
    for linked in [false, true] {
        let folder = tempfile::tempdir().unwrap();
        let target = folder.path().join("existing.pdf");
        fs::write(&target, b"reviewed").unwrap();
        let publication = prepare(&target);
        stage(&publication);
        assert!(
            publication
                .commit_at(&mut || Ok(()), &mut |phase| {
                    if phase
                        == (if linked {
                            Phase::Published
                        } else {
                            Phase::Publishing
                        })
                    {
                        return Err(stopped());
                    }
                    Ok(())
                })
                .is_err()
        );
        let result = publication.settle(false).unwrap();
        assert_eq!(result.published, linked);
        assert_eq!(result.uncertain, !linked);
        if linked {
            assert_eq!(fs::read(target).unwrap(), b"%PDF-1.7\nnew document");
            assert!(result.recovery.is_none());
        } else {
            assert!(!target.exists());
            assert!(result.recovery.is_some());
            assert_eq!(
                fs::read(work(&publication).join(PREVIOUS)).unwrap(),
                b"reviewed"
            );
        }
    }
}

#[test]
fn moved_parent_and_changed_original_fail_without_writing_the_target() {
    for move_parent in [false, true] {
        let folder = tempfile::tempdir().unwrap();
        let parent = folder.path().join("chosen");
        fs::create_dir(&parent).unwrap();
        let target = parent.join("existing.pdf");
        fs::write(&target, b"original").unwrap();
        let publication = prepare(&target);
        stage(&publication);
        if move_parent {
            fs::rename(&parent, folder.path().join("moved")).unwrap();
            fs::create_dir(&parent).unwrap();
            fs::write(&target, b"another directory").unwrap();
        } else {
            fs::write(&target, b"changed original").unwrap();
        }
        assert!(publication.commit(|| Ok(())).is_err());
        assert!(!publication.settle(false).unwrap().published);
        assert_eq!(
            fs::read(target).unwrap(),
            if move_parent {
                b"another directory" as &[u8]
            } else {
                b"changed original"
            }
        );
    }
}

#[test]
fn corrupt_journals_and_foreign_children_survive_cleanup() {
    for corrupt in [false, true] {
        let folder = tempfile::tempdir().unwrap();
        let publication = prepare(&folder.path().join("new.pdf"));
        let directory = work(&publication);
        if corrupt {
            publication
                .record
                .write_all_at(b"partial", publication.record.metadata().unwrap().len())
                .unwrap();
        } else {
            fs::write(directory.join("foreign"), b"keep").unwrap();
        }
        let result = publication.settle(false).unwrap();
        assert!(result.recovery.is_some());
        assert_eq!(result.uncertain, corrupt);
        assert!(directory.join(RECORD).exists());
        assert!(directory.join(DOCUMENT).exists());
        if !corrupt {
            assert_eq!(fs::read(directory.join("foreign")).unwrap(), b"keep");
        }
    }
}

#[test]
fn cleanup_can_resume_after_its_own_document_unlink() {
    let folder = tempfile::tempdir().unwrap();
    let target = folder.path().join("new.pdf");
    let publication = prepare(&target);
    stage(&publication);
    publication.commit(|| Ok(())).unwrap();
    fs::remove_file(work(&publication).join(DOCUMENT)).unwrap();
    let copies = publication
        .files()
        .into_iter()
        .map(|file| file.try_clone().unwrap())
        .collect();
    let inherited = Publication::inherited(publication.info.clone(), copies).unwrap();
    assert!(inherited.settle(true).unwrap().recovery.is_none());
    assert_eq!(fs::read(target).unwrap(), b"%PDF-1.7\nnew document");
}

#[test]
fn completed_cleanup_is_idempotent_even_if_another_directory_reuses_its_name() {
    let folder = tempfile::tempdir().unwrap();
    let target = folder.path().join("new.pdf");
    let publication = prepare(&target);
    let directory = work(&publication);
    stage(&publication);
    publication.commit(|| Ok(())).unwrap();
    assert!(publication.settle(false).unwrap().published);
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("foreign"), b"preserve").unwrap();
    let files = publication
        .files()
        .into_iter()
        .map(|file| file.try_clone().unwrap())
        .collect();
    let next = Publication::inherited(publication.info.clone(), files).unwrap();
    let settled = next.settle(false).unwrap();
    assert!(settled.published && settled.recovery.is_none());
    let interrupted = publication.interrupted(false);
    assert!(interrupted.published && interrupted.recovery.is_none());
    assert_eq!(fs::read(directory.join("foreign")).unwrap(), b"preserve");
}

#[test]
fn positive_file_identity_survives_a_damaged_journal_without_discarding_the_original() {
    let folder = tempfile::tempdir().unwrap();
    let target = folder.path().join("existing.pdf");
    fs::write(&target, b"original").unwrap();
    let publication = prepare(&target);
    stage(&publication);
    publication.commit(|| Ok(())).unwrap();
    publication
        .record
        .write_all_at(b"partial", publication.record.metadata().unwrap().len())
        .unwrap();
    let result = publication.settle(false).unwrap();
    assert!(result.published && !result.uncertain && result.recovery.is_some());
    assert_eq!(fs::read(&target).unwrap(), b"%PDF-1.7\nnew document");
    assert_eq!(
        fs::read(work(&publication).join(PREVIOUS)).unwrap(),
        b"original"
    );
}

#[test]
fn a_missing_target_after_a_lost_commit_reply_never_triggers_automatic_rollback() {
    let folder = tempfile::tempdir().unwrap();
    let target = folder.path().join("existing.pdf");
    fs::write(&target, b"original").unwrap();
    let publication = prepare(&target);
    stage(&publication);
    assert!(
        publication
            .commit_at(&mut || Ok(()), &mut |phase| {
                if phase == Phase::Published {
                    fs::remove_file(&target)?;
                    return Err(stopped());
                }
                Ok(())
            })
            .is_err()
    );
    let result = publication.settle(false).unwrap();
    assert!(!result.published && result.uncertain && result.recovery.is_some());
    assert!(!target.exists());
    assert_eq!(
        fs::read(work(&publication).join(PREVIOUS)).unwrap(),
        b"original"
    );
}

// Reuse this workspace's compiled harness instead of building a second native
// Rust executable. Only the test boundary can stop between the actual syscalls.
#[test]
#[ignore = "child entrypoint for killed_transaction_workers_keep_recoverable_bytes"]
fn transaction_worker_fixture() {
    let info =
        serde_json::from_slice(&fs::read(std::env::var_os("CANOPA_FILE_INFO").unwrap()).unwrap())
            .unwrap();
    let descriptors: Vec<i32> =
        serde_json::from_str(&std::env::var("CANOPA_FILE_FDS").unwrap()).unwrap();
    let files = descriptors
        .into_iter()
        .map(|fd| {
            // SAFETY: fcntl duplicates the inherited original without adopting it.
            let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
            assert!(copy >= 0);
            use std::os::fd::FromRawFd;
            // SAFETY: the duplicate is new and uniquely owned.
            unsafe { File::from_raw_fd(copy) }
        })
        .collect();
    let publication = Publication::inherited(info, files).unwrap();
    let phase: Phase = serde_json::from_str(&std::env::var("CANOPA_FILE_STOP").unwrap()).unwrap();
    publication
        .commit_at(&mut || Ok(()), &mut |current| {
            if current == phase {
                fs::write(std::env::var_os("CANOPA_FILE_READY").unwrap(), b"ready")?;
                // SAFETY: stop only this explicitly spawned, disposable child.
                unsafe {
                    libc::raise(libc::SIGSTOP);
                }
            }
            Ok(())
        })
        .unwrap();
    panic!("fixture should have been killed at the selected boundary");
}

#[test]
fn killed_transaction_workers_keep_recoverable_bytes() {
    use std::{
        os::unix::process::CommandExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    for phase in [Phase::Claimed, Phase::Publishing, Phase::Published] {
        let folder = tempfile::tempdir().unwrap();
        let target = folder.path().join("existing.pdf");
        fs::write(&target, b"reviewed").unwrap();
        let publication = prepare(&target);
        stage(&publication);
        let info = folder.path().join("info.json");
        fs::write(&info, serde_json::to_vec(publication.info()).unwrap()).unwrap();
        let ready = folder.path().join("ready");
        let descriptors = publication
            .files()
            .into_iter()
            .map(AsRawFd::as_raw_fd)
            .collect::<Vec<_>>();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "print::file_target::publication::tests::transaction_worker_fixture",
                "--exact",
                "--ignored",
                "--nocapture",
            ])
            .env("CANOPA_FILE_INFO", info)
            .env(
                "CANOPA_FILE_FDS",
                serde_json::to_string(&descriptors).unwrap(),
            )
            .env("CANOPA_FILE_STOP", serde_json::to_string(&phase).unwrap())
            .env("CANOPA_FILE_READY", &ready)
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        // SAFETY: before exec this callback only changes flags on descriptors
        // owned by the parent and explicitly authorized for this child.
        unsafe {
            command.pre_exec(move || {
                for fd in &descriptors {
                    if libc::fcntl(*fd, libc::F_SETFD, 0) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() && Instant::now() < deadline {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("child stopped before boundary: {status}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let reached = ready.exists();
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        assert!(reached, "transaction worker did not reach {phase:?}");
        let disposition = publication.settle(false).unwrap();
        match phase {
            Phase::Claimed => {
                assert!(
                    !disposition.published
                        && !disposition.uncertain
                        && disposition.recovery.is_none()
                );
                assert_eq!(fs::read(&target).unwrap(), b"reviewed");
            }
            Phase::Publishing => {
                assert!(
                    !disposition.published
                        && disposition.uncertain
                        && disposition.recovery.is_some()
                );
                assert!(!target.exists());
                assert_eq!(
                    fs::read(work(&publication).join(PREVIOUS)).unwrap(),
                    b"reviewed"
                );
                assert_eq!(
                    fs::read(work(&publication).join(DOCUMENT)).unwrap(),
                    b"%PDF-1.7\nnew document"
                );
            }
            Phase::Published => {
                assert!(
                    disposition.published
                        && !disposition.uncertain
                        && disposition.recovery.is_none()
                );
                assert_eq!(fs::read(&target).unwrap(), b"%PDF-1.7\nnew document");
            }
            _ => unreachable!(),
        }
    }
}
