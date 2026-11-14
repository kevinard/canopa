//! Kill points inside a batched Copy (ADR0180). A child test process sets
//! `CANOPA_COPY_BATCH_ROOT` to its private root and `CANOPA_COPY_BATCH_POINT`
//! to a point; reaching that point inside that root ends the process at once,
//! as a crash would, after the journal and filesystem reached that state.
use super::*;
use crate::operations::copy_batch::CopyBatch;

fn armed(point: &str, batch: &CopyBatch) -> bool {
    let Some(root) = std::env::var_os("CANOPA_COPY_BATCH_ROOT").map(PathBuf::from) else {
        return false;
    };
    std::env::var("CANOPA_COPY_BATCH_POINT").as_deref() == Ok(point)
        && batch.stage.directory.starts_with(&root)
}

/// `registered`, `payloads`, `forward` or `appended`.
pub(in crate::operations) fn point(point: &str, batch: &CopyBatch) {
    if armed(point, batch) {
        kill();
    }
}

fn kill() {
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
        .unwrap();
    unreachable!("the copy batch boundary was not killed");
}

/// After `count` files of the batch were published (`published-COUNT`).
pub(in crate::operations) fn published(batch: &CopyBatch, count: usize) {
    if armed(&format!("published-{count}"), batch) {
        kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfers::{
        ConflictChoice, TransferEvent, TransferKind, TransferReply, TransferRequest,
    };
    use std::os::unix::process::ExitStatusExt;

    const FILES: usize = 12;

    fn fixture(root: &Path) -> (PathBuf, PathBuf) {
        let source = root.join("source");
        let destination = root.join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        for index in 0..FILES {
            fs::write(
                source.join(format!("f{index:02}")),
                format!("bytes {index}"),
            )
            .unwrap();
        }
        (source, destination)
    }

    fn sources(source: &Path) -> Vec<PathBuf> {
        (0..FILES)
            .map(|index| source.join(format!("f{index:02}")))
            .collect()
    }

    fn copy(
        engine: &OperationEngine,
        source: &Path,
        destination: &Path,
        cancellation: &Cancellation,
        reply: impl FnMut(TransferEvent) -> TransferReply,
    ) -> crate::operations::transfer::TransferOutcome {
        engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: sources(source),
                    destination: destination.to_owned(),
                },
                cancellation,
                reply,
            )
            .unwrap()
    }

    fn stages(folder: &Path) -> usize {
        fs::read_dir(folder)
            .unwrap()
            .filter(|entry| {
                entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b".canopa-files-stage-")
            })
            .count()
    }

    fn steps(history: &Path) -> Vec<u64> {
        let db = rusqlite::Connection::open(history).unwrap();
        let mut statement = db
            .prepare("SELECT ordinal FROM changes ORDER BY ordinal")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    fn pending(history: &Path) -> u64 {
        let db = rusqlite::Connection::open(history).unwrap();
        db.query_row("SELECT COUNT(*) FROM pending", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn many_small_files_share_one_batch_and_one_undo_returns_them() {
        let root = tempfile::tempdir().unwrap();
        let (source, destination) = fixture(root.path());
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        crate::operations::durability::take_counts();
        let result = copy(
            &engine,
            &source,
            &destination,
            &Cancellation::default(),
            |_| TransferReply::Continue,
        );
        assert!(result.failures.is_empty() && !result.cancelled);
        assert_eq!(result.completed, FILES);
        for index in 0..FILES {
            assert_eq!(
                fs::read_to_string(destination.join(format!("f{index:02}"))).unwrap(),
                format!("bytes {index}")
            );
        }
        assert_eq!(stages(&destination), 0);
        assert_eq!(steps(&history), (0..FILES as u64).collect::<Vec<_>>());
        assert_eq!(pending(&history), 0);
        engine.undo(&Cancellation::default()).unwrap();
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&source).unwrap().count(), FILES);
    }

    #[test]
    fn long_paths_still_batch_inside_the_journal_budget() {
        let root = tempfile::tempdir().unwrap();
        let long = "l".repeat(200);
        let source = root.path().join(&long).join(&long);
        let destination = root.path().join(format!("{long}-out")).join(&long);
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&destination).unwrap();
        let count = super::super::super::copy_batch::MAX_ITEMS * 2;
        let sources: Vec<_> = (0..count)
            .map(|index| {
                let path = source.join(format!("{long}-{index:03}"));
                fs::write(&path, format!("bytes {index}")).unwrap();
                path
            })
            .collect();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        crate::operations::copy_batch::PUBLISHED.set(0);
        let result = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources,
                    destination: destination.clone(),
                },
                &Cancellation::default(),
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert_eq!(result.completed, count);
        assert!(result.failures.is_empty());
        assert_eq!(
            crate::operations::copy_batch::PUBLISHED.get(),
            count,
            "every file went through a batch"
        );
        assert_eq!(stages(&destination), 0);
    }

    #[test]
    fn a_name_taken_during_the_batch_is_asked_about_and_the_rest_publish() {
        let root = tempfile::tempdir().unwrap();
        let (source, destination) = fixture(root.path());
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        let mut taken = false;
        let mut asked = 0;
        let result = copy(
            &engine,
            &source,
            &destination,
            &Cancellation::default(),
            |event| match event {
                TransferEvent::Progress { .. } if !taken => {
                    fs::write(destination.join("f05"), b"someone else").unwrap();
                    taken = true;
                    TransferReply::Continue
                }
                TransferEvent::Conflict { .. } => {
                    asked += 1;
                    TransferReply::Resolve {
                        choice: ConflictChoice::Skip,
                        apply_to_remaining: false,
                    }
                }
                _ => TransferReply::Continue,
            },
        );
        assert_eq!(asked, 1);
        assert_eq!(result.completed, FILES - 1);
        assert_eq!(result.skipped, vec![source.join("f05")]);
        assert_eq!(fs::read(destination.join("f05")).unwrap(), b"someone else");
        assert_eq!(stages(&destination), 0);
        assert_eq!(pending(&history), 0);
        engine.undo(&Cancellation::default()).unwrap();
        let left: Vec<_> = fs::read_dir(&destination)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(left, vec![std::ffi::OsString::from("f05")]);
    }

    #[test]
    fn a_source_changed_during_the_batch_fails_alone() {
        let root = tempfile::tempdir().unwrap();
        let (source, destination) = fixture(root.path());
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        let changed = source.join("f03");
        let mut done = false;
        let result = copy(
            &engine,
            &source,
            &destination,
            &Cancellation::default(),
            |event| {
                if !done
                    && matches!(&event, TransferEvent::Progress { path, .. } if path == &changed)
                {
                    fs::write(&changed, b"a newer, longer version").unwrap();
                    done = true;
                }
                TransferReply::Continue
            },
        );
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].source, changed);
        assert_eq!(result.completed, FILES - 1);
        assert!(!destination.join("f03").exists());
        assert_eq!(stages(&destination), 0);
        assert_eq!(pending(&history), 0);
    }

    #[test]
    fn cancelling_during_the_batch_keeps_the_finished_copies() {
        let root = tempfile::tempdir().unwrap();
        let (source, destination) = fixture(root.path());
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        let cancellation = Cancellation::default();
        let stop = source.join("f04");
        let result = copy(&engine, &source, &destination, &cancellation, |event| {
            if matches!(&event, TransferEvent::Progress { path, .. } if path == &stop) {
                TransferReply::Cancel
            } else {
                TransferReply::Continue
            }
        });
        assert!(result.cancelled);
        assert_eq!(result.completed, 4);
        assert_eq!(result.remaining.len(), FILES - 4);
        for index in 0..FILES {
            assert_eq!(destination.join(format!("f{index:02}")).exists(), index < 4);
        }
        assert_eq!(stages(&destination), 0);
        assert_eq!(steps(&history), vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_process_killed_at_every_batch_phase_restarts_to_a_proved_state() {
        for (point, published) in [
            ("registered", 0),
            ("payloads", 0),
            ("forward", 0),
            ("published-1", 1),
            ("published-5", 5),
            ("appended", FILES),
        ] {
            let root = tempfile::tempdir().unwrap();
            let (source, destination) = fixture(root.path());
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::transfer::copy_batch_boundary::tests::killed_batch_child",
                ])
                .env("CANOPA_COPY_BATCH_ROOT", root.path())
                .env("CANOPA_COPY_BATCH_POINT", point)
                .status()
                .unwrap();
            assert_eq!(
                status.signal(),
                Some(9),
                "{point}: the child was not killed"
            );
            let history = root.path().join("history");
            assert_eq!(pending(&history), 1, "{point}: the batch was not pending");
            // Opening the history reconciles the interrupted batch.
            let engine = OperationEngine::open(&history).unwrap();
            assert_eq!(pending(&history), 0, "{point}: the batch stayed pending");
            assert_eq!(stages(&destination), 0, "{point}: a stage was left behind");
            let present: Vec<_> = (0..FILES)
                .filter(|index| destination.join(format!("f{index:02}")).exists())
                .collect();
            assert_eq!(present, (0..published).collect::<Vec<_>>(), "{point}");
            assert_eq!(
                steps(&history),
                (0..published as u64).collect::<Vec<_>>(),
                "{point}: every published copy has exactly one Undo step"
            );
            for index in 0..published {
                assert_eq!(
                    fs::read_to_string(destination.join(format!("f{index:02}"))).unwrap(),
                    format!("bytes {index}")
                );
            }
            if published > 0 {
                engine.undo(&Cancellation::default()).unwrap();
                assert_eq!(fs::read_dir(&destination).unwrap().count(), 0, "{point}");
            }
            assert_eq!(fs::read_dir(&source).unwrap().count(), FILES, "{point}");
        }
    }

    #[test]
    fn a_batch_with_unknown_work_is_kept_in_recovery_untouched() {
        let root = tempfile::tempdir().unwrap();
        let (_source, destination) = fixture(root.path());
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::transfer::copy_batch_boundary::tests::killed_batch_child",
            ])
            .env("CANOPA_COPY_BATCH_ROOT", root.path())
            .env("CANOPA_COPY_BATCH_POINT", "payloads")
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let stage = fs::read_dir(&destination)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .as_encoded_bytes()
                    .starts_with(b".canopa-files-stage-")
            })
            .unwrap();
        fs::write(stage.join("unknown"), b"not the batch's").unwrap();
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        assert_eq!(pending(&history), 1, "unknown work keeps the batch pending");
        let page = engine
            .recovery_items(None, &Cancellation::default())
            .unwrap();
        let item = page
            .items
            .into_iter()
            .find(|item| item.can_keep_in_recovery())
            .expect("the stuck batch offers Keep");
        engine
            .keep_in_recovery(&item, &Cancellation::default())
            .unwrap();
        assert_eq!(pending(&history), 0);
        assert_eq!(fs::read(stage.join("unknown")).unwrap(), b"not the batch's");
        let kept: u64 = rusqlite::Connection::open(&history)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM garbage", [], |row| row.get(0))
            .unwrap();
        assert_eq!(kept, 1);
    }

    #[test]
    #[ignore = "child killed at a copy batch boundary"]
    fn killed_batch_child() {
        let root = PathBuf::from(std::env::var_os("CANOPA_COPY_BATCH_ROOT").unwrap());
        assert!(root.starts_with(std::env::temp_dir()));
        let engine = OperationEngine::open(root.join("history")).unwrap();
        copy(
            &engine,
            &root.join("source"),
            &root.join("destination"),
            &Cancellation::default(),
            |_| TransferReply::Continue,
        );
        panic!("the batch reached no boundary");
    }

    mod jobs {
        use super::*;
        use crate::{
            local_activity::LocalActivity,
            operation_jobs::{OperationJobs, Phase, Request},
            operations::transfer::TransferPart,
            storage::Storage,
        };
        use std::time::{Duration, Instant};

        fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
            let end = Instant::now() + Duration::from_secs(60);
            loop {
                if let Some(value) = probe() {
                    return value;
                }
                assert!(Instant::now() < end, "the batched transfer did not settle");
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        fn receive<T>(mut receiver: futures_channel::oneshot::Receiver<T>) -> T {
            wait(|| receiver.try_recv().expect("the owner dropped its reply"))
        }

        fn start(root: &Path) -> OperationJobs {
            let jobs = OperationJobs::start_with_activity(
                root.join("history"),
                Storage::load(root.join("volumes")).unwrap(),
                LocalActivity::default(),
            )
            .unwrap();
            wait(|| (!jobs.snapshot().initializing).then_some(()));
            assert!(jobs.snapshot().error.is_none(), "{:?}", jobs.snapshot());
            jobs
        }

        fn settled(jobs: &OperationJobs, id: u64) -> crate::operation_jobs::JobSnapshot {
            wait(|| {
                jobs.snapshot()
                    .jobs
                    .into_iter()
                    .find(|job| job.id == id && !job.phase.holds_application())
            })
        }

        #[test]
        fn a_killed_batched_job_resumes_every_unpublished_file_exactly_once() {
            for point in ["payloads", "forward", "published-5", "appended"] {
                let root = tempfile::tempdir().unwrap();
                let (source, destination) = fixture(root.path());
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "operations::transfer::copy_batch_boundary::tests::jobs::killed_batched_job_child",
                    ])
                    .env("CANOPA_COPY_BATCH_ROOT", root.path())
                    .env("CANOPA_COPY_BATCH_POINT", point)
                    .status()
                    .unwrap();
                assert_eq!(
                    status.signal(),
                    Some(9),
                    "{point}: the child was not killed"
                );
                let jobs = start(root.path());
                let id = wait(|| jobs.snapshot().jobs.first().map(|job| job.id));
                let mut restored = settled(&jobs, id);
                if restored.phase == Phase::Interrupted {
                    // Interrupted work waits for explicit reconciliation.
                    let review = receive(jobs.submit(Request::Reconcile, None).unwrap()).unwrap();
                    let done = settled(&jobs, review);
                    assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
                    restored = settled(&jobs, id);
                }
                if restored.transfer_actions.remaining {
                    let next = receive(jobs.resume_transfer(id, TransferPart::Remaining).unwrap())
                        .unwrap();
                    let done = settled(&jobs, next);
                    assert_eq!(done.phase, Phase::Finished, "{point}: {done:?}");
                } else {
                    assert_eq!(restored.phase, Phase::Finished, "{point}: {restored:?}");
                }
                let mut names: Vec<_> = fs::read_dir(&destination)
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                    .collect();
                names.sort();
                let expected: Vec<_> = (0..FILES).map(|index| format!("f{index:02}")).collect();
                assert_eq!(
                    names, expected,
                    "{point}: every file once, no stage, no copies"
                );
                for index in 0..FILES {
                    assert_eq!(
                        fs::read_to_string(destination.join(format!("f{index:02}"))).unwrap(),
                        format!("bytes {index}"),
                        "{point}"
                    );
                }
                assert_eq!(pending(&root.path().join("history")), 0, "{point}");
                assert_eq!(fs::read_dir(&source).unwrap().count(), FILES, "{point}");
            }
        }

        #[test]
        #[ignore = "job service killed at a copy batch boundary"]
        fn killed_batched_job_child() {
            let root = PathBuf::from(std::env::var_os("CANOPA_COPY_BATCH_ROOT").unwrap());
            assert!(root.starts_with(std::env::temp_dir()));
            let jobs = start(&root);
            receive(
                jobs.submit(
                    Request::Transfer(TransferRequest {
                        kind: TransferKind::Copy,
                        sources: sources(&root.join("source")),
                        destination: root.join("destination"),
                    }),
                    None,
                )
                .unwrap(),
            )
            .unwrap();
            std::thread::sleep(Duration::from_secs(30));
            panic!("the batched job reached no boundary");
        }
    }
}
