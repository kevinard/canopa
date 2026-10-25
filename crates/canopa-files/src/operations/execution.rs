//! Request/journal association is evidence, never authority to replay a request.
use super::*;
use requests::RequestRecord;
mod mutation;
mod transfer_completion;
pub(crate) use mutation::{MutationConfirmation, MutationOutcome};
pub(crate) use transfer_completion::{
    CompletedTransfer, RestoredTransfer, TransferConfirmation, TransferEnd,
    TransferExecutionOutcome,
};

/// Currently retained confirmed changes for one exact accepted execution.
/// Empty changes or an absent journal do not establish successful completion.
#[derive(Clone, Debug)]
pub struct RequestEffects {
    pub revision: u64,
    pub operation: u64,
    pub label: String,
    pub payload_sha256: [u8; 32],
    pub confirmed_steps: usize,
    /// At most128 retained change paths; inspect Undo/Recovery for broader work.
    pub paths: Vec<PathBuf>,
    pub paths_truncated: bool,
    pub history_available: bool,
    pub journal_pending: bool,
}

pub(crate) struct Execution<'a> {
    pub(super) engine: &'a OperationEngine,
    pub(super) record: &'a RequestRecord,
}

impl OperationEngine {
    /// Inspect evidence for this request without acquiring execution ownership.
    /// Old records have no inferred association based on labels or pathnames.
    pub fn request_effects(&self, record: &RequestRecord) -> io::Result<Vec<RequestEffects>> {
        self.reads.snapshot(false)?.request_effects(record)
    }

    pub(crate) fn execution<'a>(&'a self, record: &'a RequestRecord) -> Execution<'a> {
        Execution {
            engine: self,
            record,
        }
    }
}

impl Execution<'_> {
    pub(crate) fn mutate(
        &self,
        request: OperationRequest,
        cancellation: &Cancellation,
    ) -> io::Result<MutationOutcome> {
        self.engine
            .execute_in_request(request, cancellation, Some(self.record), None)
    }

    pub(crate) fn rename(
        &self,
        source: PathBuf,
        destination: PathBuf,
        observed: &transfer::ObservedSources,
        cancellation: &Cancellation,
    ) -> io::Result<MutationOutcome> {
        self.engine.execute_in_request(
            OperationRequest::Rename {
                source,
                destination,
            },
            cancellation,
            Some(self.record),
            Some(observed),
        )
    }

    pub(crate) fn transfer(
        &self,
        observed: &transfer::ObservedTransfer,
        continuation: Option<&transfer::TransferContinuation>,
        cancellation: &Cancellation,
        check: impl FnMut(&Path, transfer::TransferAccess) -> io::Result<()>,
        interact: impl FnMut(transfer::TransferEvent) -> transfer::TransferReply,
    ) -> io::Result<TransferExecutionOutcome> {
        let initial;
        let plan = match continuation {
            Some(plan) => plan,
            None => {
                initial = transfer::TransferContinuation::unstarted(observed);
                &initial
            }
        };
        self.engine
            .resume_transfer_checked(
                plan,
                cancellation,
                Some(observed),
                Some(self.record),
                check,
                interact,
            )
            .and_then(|outcome| self.engine.transfer_outcome(self.record, plan, outcome))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use requests::RequestPhase;

    #[test]
    fn forward_rename_uses_only_its_own_reservation_and_undo_does_not_inherit_it() {
        use transfer::{
            ConflictChoice, TransferEvent, TransferKind, TransferReply, TransferRequest,
        };
        for other_owner in ["none", "clipboard", "request"] {
            let root = tempfile::tempdir().unwrap();
            let incoming = root.path().join("incoming");
            let destination = root.path().join("destination");
            fs::create_dir(&incoming).unwrap();
            fs::create_dir(&destination).unwrap();
            fs::write(incoming.join("note"), b"Current bytes").unwrap();
            fs::write(destination.join("note"), b"Saved bytes").unwrap();
            let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
            engine
                .transfer(
                    &TransferRequest {
                        kind: TransferKind::Copy,
                        sources: vec![incoming.join("note")],
                        destination: destination.clone(),
                    },
                    &Cancellation::default(),
                    |event| match event {
                        TransferEvent::Conflict { .. } => TransferReply::Resolve {
                            choice: ConflictChoice::Replace,
                            apply_to_remaining: false,
                        },
                        _ => TransferReply::Continue,
                    },
                )
                .unwrap();
            let saved = engine
                .recovery_items(None, &Cancellation::default())
                .unwrap()
                .items
                .into_iter()
                .find_map(|item| item.entry.map(|entry| entry.path))
                .unwrap();
            let sources = engine
                .capture_retained_sources(std::slice::from_ref(&saved))
                .unwrap();
            let clipboard = (other_owner == "clipboard")
                .then(|| engine.lease_retained_sources(&sources).unwrap());
            let other = (other_owner == "request").then(|| {
                engine
                    .queue_request_with_sources(&"another request", &sources)
                    .unwrap()
            });
            let queued = engine
                .queue_request_with_sources(&"Rename", &sources)
                .unwrap();
            let running = engine
                .checkpoint_request(&queued, RequestPhase::Running, &"Rename")
                .unwrap();
            let renamed = root.path().join("renamed saved bytes");
            let rename = || OperationRequest::Rename {
                source: saved.clone(),
                destination: renamed.clone(),
            };
            assert_eq!(
                engine
                    .execute(rename(), &Cancellation::default())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock,
                "an unassociated execution borrowed another request's reservation"
            );
            let result = engine
                .execution(&running)
                .mutate(rename(), &Cancellation::default());
            if other_owner != "none" {
                assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
                assert_eq!(fs::read(&saved).unwrap(), b"Saved bytes");
                assert!(!renamed.exists());
                if let Some(lease) = clipboard {
                    lease.release().unwrap();
                }
                if let Some(other) = other {
                    engine.dismiss_request(&other).unwrap();
                }
                // Refusal consumed this execution association, so releasing a
                // competing owner does not turn it into replay authority.
                assert!(
                    engine
                        .execution(&running)
                        .mutate(rename(), &Cancellation::default())
                        .is_err()
                );
            } else {
                result.unwrap();
                assert!(!saved.exists());
                assert_eq!(fs::read(&renamed).unwrap(), b"Saved bytes");
                assert_eq!(
                    engine.undo(&Cancellation::default()).unwrap_err().kind(),
                    io::ErrorKind::WouldBlock,
                    "Undo reused its original forward request's exception"
                );
                assert_eq!(fs::read(&renamed).unwrap(), b"Saved bytes");
            }
            engine
                .checkpoint_request(&running, RequestPhase::Finished, &"Reviewed")
                .unwrap();
            if other_owner == "none" {
                engine.undo(&Cancellation::default()).unwrap();
                assert_eq!(fs::read(&saved).unwrap(), b"Saved bytes");
                assert!(!renamed.exists());
            } else {
                engine.execute(rename(), &Cancellation::default()).unwrap();
                assert_eq!(fs::read(&renamed).unwrap(), b"Saved bytes");
            }
            assert_eq!(
                fs::read(destination.join("note")).unwrap(),
                b"Current bytes"
            );
        }
    }

    fn running(engine: &OperationEngine) -> RequestRecord {
        let queued = engine.queue_request(&"one accepted execution").unwrap();
        engine
            .checkpoint_request(&queued, RequestPhase::Running, &"one accepted execution")
            .unwrap()
    }

    #[test]
    fn stale_foreign_and_already_executed_revisions_cannot_create_more_work() {
        let root = tempfile::tempdir().unwrap();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let request = running(&engine);
        let first = root.path().join("first");
        engine
            .execution(&request)
            .mutate(
                OperationRequest::CreateFolder {
                    path: first.clone(),
                },
                &Cancellation::default(),
            )
            .unwrap();
        let effects = engine.request_effects(&request).unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].confirmed_steps, 1);
        assert!(effects[0].history_available);
        assert!(!effects[0].journal_pending);
        let later = root.path().join("must-not-exist");
        let attempt = |engine: &OperationEngine, request: &RequestRecord| {
            engine.execution(request).mutate(
                OperationRequest::CreateFolder {
                    path: later.clone(),
                },
                &Cancellation::default(),
            )
        };
        assert_eq!(
            attempt(&engine, &request).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        let advanced = engine
            .checkpoint_request(
                &request,
                RequestPhase::Running,
                &"a newer revision cannot replay the same accepted request",
            )
            .unwrap();
        assert_eq!(
            attempt(&engine, &advanced).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        let stale = running(&engine);
        engine
            .checkpoint_request(&stale, RequestPhase::Failed, &"newer review")
            .unwrap();
        assert_eq!(
            attempt(&engine, &stale).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        let foreign = OperationEngine::open(root.path().join("other-history")).unwrap();
        assert!(attempt(&foreign, &request).is_err());
        assert!(foreign.request_effects(&request).is_err());
        assert!(!later.exists());
        assert!(first.is_dir());
        assert_eq!(engine.latest_undo_id(), Some(effects[0].operation));
        assert!(engine.request_effects(&stale).unwrap().is_empty());
    }

    #[test]
    fn association_refusal_rolls_back_the_operation_and_cleans_preparation() {
        let root = tempfile::tempdir().unwrap();
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        let request = running(&engine);
        let database = rusqlite::Connection::open(&history).unwrap();
        database.execute_batch("CREATE TRIGGER refuse_execution BEFORE INSERT ON request_executions BEGIN SELECT RAISE(ABORT,'private association refusal'); END;").unwrap();
        let target = root.path().join("target");
        assert!(
            engine
                .execution(&request)
                .mutate(
                    OperationRequest::CreateFolder {
                        path: target.clone()
                    },
                    &Cancellation::default()
                )
                .is_err()
        );
        assert!(!target.exists());
        assert!(engine.request_effects(&request).unwrap().is_empty());
        assert_eq!(engine.latest_undo_id(), None);
        assert_eq!(
            database
                .query_row("SELECT count(*) FROM operations", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
        assert!(fs::read_dir(root.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .as_encoded_bytes()
                .starts_with(b".canopa-files-stage-")
        }));
        database
            .execute_batch("DROP TRIGGER refuse_execution;")
            .unwrap();
        engine
            .execution(&request)
            .mutate(
                OperationRequest::CreateFolder { path: target },
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(engine.request_effects(&request).unwrap().len(), 1);
    }

    #[test]
    fn history_expiry_does_not_make_an_execution_revision_reusable() {
        let root = tempfile::tempdir().unwrap();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let request = running(&engine);
        engine
            .execution(&request)
            .mutate(
                OperationRequest::CreateFolder {
                    path: root.path().join("original"),
                },
                &Cancellation::default(),
            )
            .unwrap();
        for index in 0..100 {
            engine
                .execute(
                    OperationRequest::CreateFolder {
                        path: root.path().join(format!("later-{index}")),
                    },
                    &Cancellation::default(),
                )
                .unwrap();
        }
        let effects = engine.request_effects(&request).unwrap();
        assert_eq!(effects.len(), 1);
        assert!(!effects[0].history_available);
        assert_eq!(effects[0].confirmed_steps, 0);
        let target = root.path().join("must-not-replay");
        assert!(
            engine
                .execution(&request)
                .mutate(
                    OperationRequest::CreateFolder {
                        path: target.clone()
                    },
                    &Cancellation::default()
                )
                .is_err()
        );
        assert!(!target.exists());
    }

    #[test]
    fn execution_metadata_obeys_the_existing_request_payload_budget() {
        // The final case fits the start association but leaves no room for the
        // typed mutation intent/end reservation. All must refuse before work.
        for available in [0, 250, 200] {
            let root = tempfile::tempdir().unwrap();
            let engine = OperationEngine::open(root.path().join("history")).unwrap();
            let payload = "x".repeat(4 * 1024 * 1024 - 2 - available);
            let queued = engine.queue_request(&payload).unwrap();
            let running = engine
                .checkpoint_request(&queued, RequestPhase::Running, &payload)
                .unwrap();
            let target = root.path().join(if available == 250 {
                "é".repeat(80)
            } else {
                "must-not-exist".to_owned()
            });
            let error = engine
                .execution(&running)
                .mutate(
                    OperationRequest::CreateFolder {
                        path: target.clone(),
                    },
                    &Cancellation::default(),
                )
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::StorageFull);
            assert!(!target.exists());
            assert!(engine.request_effects(&running).unwrap().is_empty());
            assert_eq!(engine.latest_undo_id(), None);
            assert_eq!(
                engine.request_page(None).unwrap()[0]
                    .payload::<String>()
                    .unwrap(),
                payload
            );
        }
    }

    #[test]
    fn original_transfer_facts_are_sealed_before_work_and_outlive_the_stopped_report() {
        let root = tempfile::tempdir().unwrap();
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        let running = running(&engine);
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("a-done"), b"Original bytes").unwrap();
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            source.join("b-failed"),
            rustix::fs::Mode::RUSR,
        )
        .unwrap();
        let request = transfer::TransferRequest {
            kind: transfer::TransferKind::Copy,
            sources: vec![source.clone()],
            destination,
        };
        let observed = request.observe_sources().unwrap();
        let expected = serde_json::to_value(observed.restart_observation()).unwrap();
        let db = rusqlite::Connection::open(&history).unwrap();
        db.execute_batch(&format!("CREATE TRIGGER require_original_before_journal BEFORE INSERT ON pending
            WHEN NOT EXISTS(SELECT 1 FROM request_transfer_ends WHERE request={} AND instr(intent,'\"original\":')>0)
            BEGIN SELECT RAISE(ABORT,'original selection was not sealed before file work'); END",running.id())).unwrap();
        let result = engine
            .execution(&running)
            .transfer(
                &observed,
                None,
                &Cancellation::default(),
                |_, _| Ok(()),
                |_| transfer::TransferReply::Continue,
            )
            .unwrap();
        assert!(
            result.outcome.completed > 0
                && result.outcome.failures.len() == 1
                && matches!(
                    result.confirmation,
                    Some(TransferConfirmation::RecordedStop)
                )
        );
        let read = || {
            db.query_row(
                "SELECT intent,intent_sha256 FROM request_transfer_ends WHERE request=?1",
                [running.id()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .unwrap()
        };
        let before = read();
        let intent: serde_json::Value = serde_json::from_str(&before.0).unwrap();
        assert_eq!(intent["original"], expected);
        use sha2::{Digest, Sha256};
        assert_eq!(before.1, Sha256::digest(before.0.as_bytes()).to_vec());
        fs::rename(&source, root.path().join("parked-original")).unwrap();
        fs::create_dir(&source).unwrap();
        fs::write(source.join("newer"), b"Replacement directory").unwrap();
        let stopped = engine
            .checkpoint_request(
                &running,
                RequestPhase::Failed,
                &"actual stopped report, not the initial plan",
            )
            .unwrap();
        assert_eq!(read(), before);
        assert_eq!(
            engine.request_effects(&stopped).unwrap()[0].revision,
            running.revision
        );
        assert_eq!(
            fs::read(source.join("newer")).unwrap(),
            b"Replacement directory"
        );
    }

    #[test]
    fn a_large_stopped_report_survives_above_the_step_budget_and_saves_atomically() {
        use sha2::{Digest, Sha256};
        let root = tempfile::tempdir().unwrap();
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        let running = running(&engine);
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        for index in 0..160 {
            let name = format!("{index:03}-{}", "f".repeat(180));
            rustix::fs::mkfifoat(rustix::fs::CWD, source.join(name), rustix::fs::Mode::RUSR)
                .unwrap();
        }
        let request = transfer::TransferRequest {
            kind: transfer::TransferKind::Copy,
            sources: vec![source],
            destination: destination.clone(),
        };
        let observed = request.observe_sources().unwrap();
        let result = engine
            .execution(&running)
            .transfer(
                &observed,
                None,
                &Cancellation::default(),
                |_, _| Ok(()),
                |_| transfer::TransferReply::Continue,
            )
            .unwrap();
        assert!(matches!(
            result.confirmation,
            Some(TransferConfirmation::RecordedStop)
        ));
        assert_eq!(result.outcome.failures.len(), 160);
        let stop = transfer::StoppedTransfer::new(&request, result.outcome);
        let db = rusqlite::Connection::open(&history).unwrap();
        let receipt = || {
            db.query_row(
                "SELECT body FROM request_transfer_stops WHERE request=?1",
                [running.id()],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
        };
        let original = receipt();
        assert!(original.len() > 128 * 1024 && original.len() < 4 * 1024 * 1024);
        assert_eq!(original, serde_json::to_string(&stop).unwrap());
        let mut changed = stop.clone();
        changed.report.bytes += 1;
        assert!(
            engine
                .save_stopped_transfer(&running, &changed, &changed)
                .is_err()
        );
        db.execute_batch("CREATE TRIGGER refuse_stopped_result BEFORE UPDATE ON requests BEGIN SELECT RAISE(ABORT,'private stopped result-save refusal'); END").unwrap();
        assert!(
            engine
                .save_stopped_transfer(&running, &stop, &stop)
                .is_err()
        );
        assert_eq!(receipt(), original);
        db.execute_batch("DROP TRIGGER refuse_stopped_result")
            .unwrap();
        let saved = engine
            .save_stopped_transfer(&running, &stop, &stop)
            .unwrap();
        assert_eq!(saved.phase(), RequestPhase::Failed);
        let (duplicate,revision,sha):(Option<String>,u64,Vec<u8>)=db.query_row("SELECT body,saved_revision,saved_body_sha256 FROM request_transfer_stops WHERE request=?1",[running.id()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
        assert!(duplicate.is_none());
        assert_eq!(revision, saved.revision);
        assert_eq!(sha, Sha256::digest(&saved.body).as_slice());
        assert!(engine.stopped_transfer(&saved).unwrap().is_none());
        assert_eq!(fs::read_dir(destination.join("source")).unwrap().count(), 0);
    }

    #[test]
    fn cancellation_after_the_last_file_records_its_result_and_retires_the_cursor() {
        use transfer::{TransferEvent, TransferKind, TransferReply, TransferRequest};
        let root = tempfile::tempdir().unwrap();
        let history = root.path().join("history");
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"Completed bytes").unwrap();
        fs::create_dir(&destination).unwrap();
        let engine = OperationEngine::open(&history).unwrap();
        let running = running(&engine);
        let observed = TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source],
            destination: destination.clone(),
        }
        .observe_sources()
        .unwrap();
        let result = engine
            .execution(&running)
            .transfer(
                &observed,
                None,
                &Cancellation::default(),
                |_, _| Ok(()),
                |event| match event {
                    TransferEvent::Progress { completed: 1, .. } => TransferReply::Cancel,
                    _ => TransferReply::Continue,
                },
            )
            .unwrap();
        assert!(
            result.outcome.cancelled
                && result.outcome.continuation.is_none()
                && result.outcome.retry_continuation.is_none()
        );
        assert!(
            matches!(result.confirmation, Some(TransferConfirmation::Recorded)),
            "the exact terminal result needs its receipt"
        );
        let completion = engine.completed_transfer(&running).unwrap().unwrap();
        assert!(completion.outcome().cancelled);
        assert_eq!(completion.completed, 1);
        assert_eq!(
            fs::read(destination.join("source")).unwrap(),
            b"Completed bytes"
        );
        let db = rusqlite::Connection::open(history).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM request_transfer_cursors", [], |row| {
                row.get::<_, u64>(0)
            })
            .unwrap(),
            0
        );
    }

    #[test]
    fn typed_handoff_refuses_a_changed_successor_and_keeps_its_sealed_part() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        use transfer::{
            TransferKind, TransferPart, TransferReply, TransferRequest, TransferResolution,
        };
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            source.join("failed"),
            rustix::fs::Mode::RUSR,
        )
        .unwrap();
        let engine = OperationEngine::open(
            root.path()
                .join(OsString::from_vec(b"history-\xfd".to_vec())),
        )
        .unwrap();
        let running = running(&engine);
        let request = TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source],
            destination,
        };
        let observed = request.observe_sources().unwrap();
        let outcome = engine
            .execution(&running)
            .transfer(
                &observed,
                None,
                &Cancellation::default(),
                |_, _| Ok(()),
                |_| TransferReply::Continue,
            )
            .unwrap()
            .outcome;
        let before = transfer::StoppedTransfer::new(&request, outcome);
        let record = engine
            .save_stopped_transfer(&running, &before, &before)
            .unwrap();
        let mut after = before.clone();
        after
            .release(TransferPart::Failed, TransferResolution::HandedOff)
            .unwrap();
        let mut successor =
            serde_json::json!({"Plan": {"transfer_checkpoint": before.retry.as_ref().unwrap()}});
        successor["Plan"]["transfer_checkpoint"]["pending"][0]["destination"] =
            serde_json::json!(root.path().join("unapproved"));
        assert!(
            engine
                .handoff_transfer(&record, &before, &after, &after, &successor)
                .is_err()
        );
        let restored = engine.restore_transfer_parts(&record, &before).unwrap();
        assert!(
            restored.retry.is_some() && restored.remaining.is_none() && restored.error.is_none()
        );
        assert!(!root.path().join("unapproved").exists());
        assert_eq!(engine.request_page(None).unwrap().len(), 1);
        let successor =
            serde_json::json!({"Plan": {"transfer_checkpoint": before.retry.as_ref().unwrap()}});
        let (released, successor) = engine
            .handoff_transfer(&record, &before, &after, &after, &successor)
            .unwrap();
        assert!(engine.restore_transfer_parts(&record, &before).is_err());
        let remaining = engine.restore_transfer_parts(&released, &after).unwrap();
        assert!(remaining.retry.is_none() && remaining.remaining.is_none());
        assert_eq!(successor.phase(), RequestPhase::Queued);
    }

    #[test]
    fn live_stopped_witness_saves_atomically_when_a_duplicate_end_exceeds_the_request_budget() {
        let root = tempfile::tempdir().unwrap();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("a-done"), b"Original bytes").unwrap();
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            source.join("b-failed"),
            rustix::fs::Mode::RUSR,
        )
        .unwrap();
        let request = transfer::TransferRequest {
            kind: transfer::TransferKind::Copy,
            sources: vec![source],
            destination: destination.clone(),
        };
        let observed = request.observe_sources().unwrap();
        let original = serde_json::to_string(&observed.restart_observation()).unwrap();
        let encoded_destination =
            serde_json::to_value(&request).unwrap()["destination"].to_string();
        let intent = format!(
            r#"{{"kind":"Copy","destination":{encoded_destination},"original":{original}}}"#
        );
        let label = format!("copy to {}", display_name(&destination));
        let padding = "x".repeat(4 * 1024 * 1024 - 2 - intent.len() - label.len() - 256 - 1024);
        let queued = engine.queue_request(&padding).unwrap();
        let running = engine
            .checkpoint_request(&queued, RequestPhase::Running, &padding)
            .unwrap();
        let result = engine
            .execution(&running)
            .transfer(
                &observed,
                None,
                &Cancellation::default(),
                |_, _| Ok(()),
                |_| transfer::TransferReply::Continue,
            )
            .unwrap();
        let Some(TransferConfirmation::Pending(pending)) = result.confirmation else {
            panic!("the duplicate stopped end must exceed the per-request budget")
        };
        assert!(engine.stopped_transfer(&running).unwrap().is_none());
        assert_eq!(
            engine
                .retry_transfer_confirmation(&pending)
                .unwrap_err()
                .kind(),
            io::ErrorKind::StorageFull
        );
        let stop = transfer::StoppedTransfer::new(&request, result.outcome);
        let mut changed = stop.clone();
        changed.report.bytes += 1;
        assert!(
            engine
                .save_pending_transfer_stop(&running, &pending, &changed, &changed)
                .is_err()
        );
        let foreign = tempfile::tempdir().unwrap();
        let other = OperationEngine::open(foreign.path().join("history")).unwrap();
        assert!(
            other
                .save_pending_transfer_stop(&running, &pending, &stop, &stop)
                .is_err()
        );
        let saved = engine
            .save_pending_transfer_stop(&running, &pending, &stop, &stop)
            .unwrap();
        assert_eq!(saved.phase(), RequestPhase::Failed);
        assert_eq!(
            fs::read(destination.join("source/a-done")).unwrap(),
            b"Original bytes"
        );
        assert!(!destination.join("source/b-failed").exists());
    }

    #[test]
    fn saving_a_completed_transfer_retires_its_duplicate_receipt_atomically() {
        let root = tempfile::tempdir().unwrap();
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        let running = running(&engine);
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"Original source").unwrap();
        fs::create_dir(&destination).unwrap();
        let observed = transfer::TransferRequest {
            kind: transfer::TransferKind::Copy,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }
        .observe_sources()
        .unwrap();
        let executed = engine
            .execution(&running)
            .transfer(
                &observed,
                None,
                &Cancellation::default(),
                |_, _| Ok(()),
                |_| transfer::TransferReply::Continue,
            )
            .unwrap();
        assert!(matches!(
            executed.confirmation,
            Some(TransferConfirmation::Recorded)
        ));
        let database = rusqlite::Connection::open(&history).unwrap();
        let receipt = || {
            database
                .query_row(
                    "SELECT result FROM request_transfer_ends WHERE request=?1",
                    [running.id()],
                    |row| row.get::<_, String>(0),
                )
                .unwrap()
        };
        let original = receipt();
        // This fits with the retained start association, but not a second copy
        // of the end report. Finishing must transfer ownership atomically.
        let payload = "x".repeat(4 * 1024 * 1024 - 2 - 180);
        database.execute_batch("CREATE TRIGGER refuse_transfer_result BEFORE UPDATE ON requests BEGIN SELECT RAISE(ABORT,'private result-save refusal'); END").unwrap();
        assert!(
            engine
                .finish_confirmed_transfer(&running, &payload)
                .is_err()
        );
        assert_eq!(receipt(), original);
        database
            .execute_batch("DROP TRIGGER refuse_transfer_result")
            .unwrap();
        let finished = engine
            .finish_confirmed_transfer(&running, &payload)
            .unwrap();
        assert_eq!(finished.phase(), RequestPhase::Finished);
        assert_eq!(
            database
                .query_row("SELECT count(*) FROM request_transfer_ends", [], |row| {
                    row.get::<_, usize>(0)
                })
                .unwrap(),
            0
        );
        assert_eq!(
            engine.request_effects(&finished).unwrap()[0].confirmed_steps,
            1
        );
        assert_eq!(
            fs::read(destination.join("source")).unwrap(),
            b"Original source"
        );
        assert_eq!(fs::read(source).unwrap(), b"Original source");
    }

    #[test]
    fn transfer_intent_reservation_obeys_the_request_budget_before_mutation() {
        let root = tempfile::tempdir().unwrap();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        // Enough room for the previous start association, but not its typed
        // Copy/Move reservation. No stage or confirmed work may escape rollback.
        let payload = "x".repeat(4 * 1024 * 1024 - 2 - 250);
        let queued = engine.queue_request(&payload).unwrap();
        let running = engine
            .checkpoint_request(&queued, RequestPhase::Running, &payload)
            .unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"Original source").unwrap();
        fs::create_dir(&destination).unwrap();
        let observed = transfer::TransferRequest {
            kind: transfer::TransferKind::Copy,
            sources: vec![source.clone()],
            destination: destination.clone(),
        }
        .observe_sources()
        .unwrap();
        let result = engine.execution(&running).transfer(
            &observed,
            None,
            &Cancellation::default(),
            |_, _| Ok(()),
            |_| transfer::TransferReply::Continue,
        );
        assert!(result.is_err_and(|error| error.kind() == io::ErrorKind::StorageFull));
        assert_eq!(fs::read(&source).unwrap(), b"Original source");
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        assert!(engine.request_effects(&running).unwrap().is_empty());
        assert_eq!(engine.latest_undo_id(), None);
    }

    #[test]
    fn execution_evidence_bounds_path_loading_without_losing_the_confirmed_count() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        for index in 0..130 {
            fs::write(source.join(format!("file-{index:03}")), b"Original bytes").unwrap();
        }
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let request = running(&engine);
        let observed = transfer::TransferRequest {
            kind: transfer::TransferKind::Copy,
            sources: vec![source],
            destination,
        }
        .observe_sources()
        .unwrap();
        engine
            .execution(&request)
            .transfer(
                &observed,
                None,
                &Cancellation::default(),
                |_, _| Ok(()),
                |_| transfer::TransferReply::Continue,
            )
            .unwrap();
        let effects = engine.request_effects(&request).unwrap();
        assert!(effects[0].confirmed_steps > 128);
        assert!(effects[0].paths.len() <= 128);
        assert!(effects[0].paths_truncated);
    }
}
