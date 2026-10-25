//! Completion ends Undo execution rights, retaining only original empty roots.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::operations) struct Cleanup {
    pub id: u64,
    pub ordinal: u64,
    pub roots: Vec<Stage>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    history: original::Original,
    digest: [u8; 32],
}
#[derive(Clone, Debug)]
pub(in crate::operations) struct Review {
    lane: IntentLane,
    pending: String,
    binding: String,
    roots: Vec<Option<fingerprint::Version>>,
}
pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection.execute_batch("CREATE TABLE IF NOT EXISTS undo_cleanup (lane INTEGER PRIMARY KEY CHECK(lane IN (0,1)), body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=262144));").map_err(db_error)
}
pub(super) fn clear(connection: &Connection, lane: IntentLane) -> io::Result<()> {
    connection
        .execute("DELETE FROM undo_cleanup WHERE lane=?1", [lane.index()])
        .map_err(db_error)?;
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM undo_cleanup WHERE lane=?1)",
            [lane.index()],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    if exists {
        return Err(invalid(
            "The completed Undo cleanup binding was not retired.",
        ));
    }
    Ok(())
}
fn digest(lane: IntentLane, pending: &Pending) -> io::Result<[u8; 32]> {
    let mut hash = Sha256::new();
    hash.update(b"canopa-files-completed-undo-cleanup-v1\0");
    hash.update(encode(&(lane.index(), pending))?.as_bytes());
    Ok(hash.finalize().into())
}
pub(super) fn require(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
) -> io::Result<()> {
    let body: Option<String> = connection
        .query_row(
            "SELECT body FROM undo_cleanup WHERE lane=?1",
            [lane.index()],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    if !matches!(pending, Pending::UndoCleanup { .. }) {
        return if body.is_none() {
            Ok(())
        } else {
            Err(invalid("Completed Undo cleanup lost its original journal."))
        };
    }
    require_retired_step(connection, pending)?;
    let binding: Binding = decode(&body.ok_or_else(|| {
        invalid("Completed Undo cleanup lost its original binding. Its work folders are preserved.")
    })?)?;
    binding
        .history
        .require(&transfer_completion::history_file(connection, history)?)?;
    if binding.digest != digest(lane, pending)? {
        return Err(invalid(
            "Completed Undo cleanup differs from its original receipt. Its work folders are preserved.",
        ));
    }
    Ok(())
}
fn require_retired_step(connection: &Connection, pending: &Pending) -> io::Result<()> {
    if let Pending::UndoCleanup { cleanup } = pending {
        let occupied: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM changes WHERE operation=?1 AND ordinal=?2)",
                params![cleanup.id, cleanup.ordinal],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if occupied {
            return Err(invalid(
                "The completed Undo cleanup step is occupied again.",
            ));
        }
    }
    Ok(())
}
pub(super) struct Completion {
    pending: Pending,
    binding: Binding,
}
pub(super) fn prepare(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<Option<Completion>> {
    let body: Option<String> = connection
        .query_row(
            match lane {
                IntentLane::Primary => "SELECT body FROM pending WHERE singleton=1",
                IntentLane::RecoveryCopy => "SELECT body FROM recovery_pending WHERE singleton=1",
            },
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let Some(body) = body else {
        return Ok(None);
    };
    let pending: Pending = decode(&body)?;
    if let Pending::UndoSavedReturn { returning } = &pending {
        saved_return::require(connection, history, lane, &pending)?;
        let (owner, step, actual, _, _) = returning.parts()?;
        if owner != id
            || step != ordinal
            || encode(actual)? != encode(change)?
            || returning.phase != undo::saved_return::Phase::Returned
        {
            return Err(invalid("The selected saved return has not completed."));
        }
        return completion(
            connection,
            history,
            lane,
            id,
            ordinal,
            vec![returning.stage.clone()],
        )
        .map(Some);
    }
    if let Pending::UndoCopied { removal } = &pending {
        copy_undo::require(connection, history, lane, &pending)?;
        if removal.id != id
            || removal.ordinal != ordinal
            || encode(&removal.change)? != encode(change)?
            || !removal.complete()
        {
            return Err(invalid(
                "Copy Undo has not retired its namespace consumption rights.",
            ));
        }
        return completion(
            connection,
            history,
            lane,
            id,
            ordinal,
            removal.cleanup_roots()?,
        )
        .map(Some);
    }
    let Pending::Undo {
        id: owner,
        ordinal: step,
        change: actual,
        stage,
        ..
    } = &pending
    else {
        return Ok(None);
    };
    if *owner != id || *step != ordinal || encode(actual)? != encode(change)? {
        return Err(invalid(
            "Undo completion differs from its original pending step.",
        ));
    }
    #[cfg(test)]
    undo::abort_boundary::cleanup_boundary::before_completion(connection, &pending);
    require(connection, history, lane, &pending)?;
    if matches!(lane, IntentLane::Primary) {
        undo_work::require(connection, history, &pending)?;
    }
    let mut roots = Vec::new();
    let canonical = stored_change(connection, id, ordinal)?;
    let paired = cross::saved_only(connection, id, ordinal, change)?;
    let canonical_saved = paired
        .as_ref()
        .map(|(_, saved)| saved)
        .or_else(|| canonical.retained());
    for stage in stage.iter().chain(change.retained().map(|r| &r.stage)) {
        // Legacy stages need their own creator/selection upgrade before they
        // can grant restarted cleanup. Do not adopt today's path as that proof.
        if stage.ownership.is_none() {
            continue;
        }
        if !matches!(change, Change::SourceRemoved { .. })
            && stage.ownership.as_ref().map(|owner| owner.roots())
                != canonical_saved
                    .and_then(|r| r.stage.ownership.as_ref())
                    .map(|owner| owner.roots())
        {
            return Err(invalid(
                "Completed Undo cleanup differs from its original saved work folder.",
            ));
        }
        let stage = staging::completed_cleanup(stage)?;
        if roots.iter().any(|s: &Stage| s.directory == stage.directory) {
            continue;
        }
        roots.push(stage);
    }
    if roots.is_empty() {
        return Ok(None);
    }
    completion(connection, history, lane, id, ordinal, roots).map(Some)
}
pub(super) fn completion(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    id: u64,
    ordinal: u64,
    roots: Vec<Stage>,
) -> io::Result<Completion> {
    let pending = Pending::UndoCleanup {
        cleanup: Box::new(Cleanup { id, ordinal, roots }),
    };
    let binding = Binding {
        history: original::Original::capture(&transfer_completion::history_file(
            connection, history,
        )?)?,
        digest: digest(lane, &pending)?,
    };
    Ok(Completion { pending, binding })
}
impl Completion {
    pub(super) fn install(self, connection: &Connection, lane: IntentLane) -> io::Result<()> {
        // A refused/ignored history deletion must roll back the whole handoff,
        // before its original execution owner can be retired by this receipt.
        require_retired_step(connection, &self.pending)?;
        let pending = encode(&self.pending)?;
        let binding = encode(&self.binding)?;
        connection
            .execute(
                match lane {
                    IntentLane::Primary => "INSERT INTO pending VALUES(1,?1)",
                    IntentLane::RecoveryCopy => "INSERT INTO recovery_pending VALUES(1,?1)",
                },
                [&pending],
            )
            .map_err(db_error)?;
        connection
            .execute(
                "INSERT INTO undo_cleanup(lane,body) VALUES(?1,?2)",
                params![lane.index(), binding],
            )
            .map_err(db_error)?;
        let saved: String = connection
            .query_row(
                match lane {
                    IntentLane::Primary => "SELECT body FROM pending WHERE singleton=1",
                    IntentLane::RecoveryCopy => {
                        "SELECT body FROM recovery_pending WHERE singleton=1"
                    }
                },
                [],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let saved_binding: String = connection
            .query_row(
                "SELECT body FROM undo_cleanup WHERE lane=?1",
                [lane.index()],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if pending != saved || binding != saved_binding {
            return Err(invalid(
                "Completed Undo cleanup receipt was not saved exactly.",
            ));
        }
        Ok(())
    }
}
pub(in crate::operations) fn recover(state: &mut Store) -> io::Result<()> {
    let Some(Pending::UndoCleanup { cleanup }) = state.pending()? else {
        return Ok(());
    };
    require(
        &state.connection,
        state.identity,
        state.lane,
        &Pending::UndoCleanup {
            cleanup: cleanup.clone(),
        },
    )?;
    if cleanup.roots.is_empty() || cleanup.roots.len() > 2 {
        return Err(invalid("Invalid completed Undo cleanup roots."));
    }
    for stage in &cleanup.roots {
        #[cfg(test)]
        undo::abort_boundary::cleanup_boundary::point(stage);
        #[cfg(test)]
        undo::abort_boundary::copy_boundary::completed(stage);
        clean(stage)?;
    }
    #[cfg(test)]
    undo::abort_boundary::cleanup_boundary::after_roots(&cleanup.roots);
    state.clear_intent()
}
fn observe(cleanup: &Cleanup) -> io::Result<Vec<Option<fingerprint::Version>>> {
    cleanup
        .roots
        .iter()
        .map(|stage| {
            match staging::disposal::container_version(stage) {
                Ok(version) => Ok(Some(version)),
                Err(error) => {
                    // Verify the original parent even when this root is absent.
                    let owner = stage
                        .ownership
                        .as_ref()
                        .ok_or_else(|| invalid("Missing completed Undo root proof."))?;
                    owner.require_parent(&fs::File::open(
                        stage
                            .directory
                            .parent()
                            .ok_or_else(|| invalid("Missing completed Undo parent."))?,
                    )?)?;
                    match fs::symlink_metadata(&stage.directory) {
                        Err(missing) if missing.kind() == io::ErrorKind::NotFound => Ok(None),
                        _ => Err(error),
                    }
                }
            }
        })
        .collect()
}
impl Store {
    pub(in crate::operations) fn undo_cleanup_review(
        &self,
        slot: u8,
    ) -> io::Result<Option<Review>> {
        let Some((lane, _)) = interrupted::pending_position(slot) else {
            return Ok(None);
        };
        if self.hide_running_journals && self.recovery_problem(lane).is_none() {
            return Ok(None);
        }
        let Some(pending) = self.pending_in(lane)? else {
            return Ok(None);
        };
        let Pending::UndoCleanup { cleanup } = &pending else {
            return Ok(None);
        };
        require(&self.connection, self.identity, lane, &pending)?;
        let binding = self
            .connection
            .query_row(
                "SELECT body FROM undo_cleanup WHERE lane=?1",
                [lane.index()],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        Ok(Some(Review {
            lane,
            pending: encode(&pending)?,
            binding,
            roots: observe(cleanup)?,
        }))
    }
    pub(in crate::operations) fn keep_undo_cleanup(&mut self, reviewed: &Review) -> io::Result<()> {
        self.ensure_location()?;
        let pending: Pending = decode(&reviewed.pending)?;
        let Pending::UndoCleanup { cleanup } = &pending else {
            return Err(invalid("Expected completed Undo cleanup."));
        };
        if observe(cleanup)? != reviewed.roots {
            return Err(invalid(
                "Completed Undo work changed; refresh Recovery before keeping it.",
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        require(&transaction, self.identity, reviewed.lane, &pending)?;
        let actual: String = transaction
            .query_row(
                match reviewed.lane {
                    IntentLane::Primary => "SELECT body FROM pending WHERE singleton=1",
                    IntentLane::RecoveryCopy => {
                        "SELECT body FROM recovery_pending WHERE singleton=1"
                    }
                },
                [],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let binding: String = transaction
            .query_row(
                "SELECT body FROM undo_cleanup WHERE lane=?1",
                [reviewed.lane.index()],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if actual != reviewed.pending || binding != reviewed.binding {
            return Err(invalid("Completed Undo cleanup changed; refresh Recovery."));
        }
        for (part, version) in reviewed.roots.iter().enumerate() {
            let Some(version) = version else {
                continue;
            };
            let mut record = pending
                .record(part as u8, "Kept completed Undo work folder".into())
                .ok_or_else(|| invalid("Invalid completed Undo work folder."))?;
            record.retained.version = Some(version.clone());
            let body = encode(&record)?;
            if transaction
                .execute("INSERT INTO garbage(body) VALUES(?1)", [&body])
                .map_err(db_error)?
                != 1
            {
                return Err(invalid("The completed Undo work was not retained."));
            }
            let entry = u64::try_from(transaction.last_insert_rowid()).map_err(io::Error::other)?;
            let saved: String = transaction
                .query_row("SELECT body FROM garbage WHERE id=?1", [entry], |r| {
                    r.get(0)
                })
                .map_err(db_error)?;
            if saved != body {
                return Err(invalid(
                    "The retained completed Undo work changed while saving.",
                ));
            }
            paths::index_record(&transaction, entry, &record)?;
            paths::require_retained_record(&transaction, entry, &record)?;
        }
        if observe(cleanup)? != reviewed.roots {
            return Err(invalid("Completed Undo work changed while keeping it."));
        }
        clear_intent(&transaction, reviewed.lane)?;
        transaction.commit().map_err(db_error)?;
        self.problems
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[reviewed.lane.index()] = None;
        match reviewed.lane {
            IntentLane::Primary => self.restore_directory_access(),
            IntentLane::RecoveryCopy => {
                self.in_recovery_copy(|state| state.restore_directory_access())
            }
        }
    }
}
fn clean(stage: &Stage) -> io::Result<()> {
    use rustix::fs::{AtFlags, Mode, OFlags, open, openat, unlinkat};
    if staging::completed_cleanup(stage)? != *stage {
        return Err(invalid("Completed Undo cleanup retains payload rights."));
    }
    validate_stage(Some(stage), &stage.directory)?;
    let ownership = stage.ownership.as_ref().ok_or_else(|| invalid("The completed Undo work folder has no original generation proof. Keep its work folder in Recovery."))?;
    let parent = fs::File::from(open(
        stage
            .directory
            .parent()
            .ok_or_else(|| invalid("Missing cleanup parent."))?,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    ownership.require_parent(&parent)?;
    let name = stage
        .directory
        .file_name()
        .ok_or_else(|| invalid("Missing cleanup name."))?;
    let directory = match openat(
        &parent,
        name,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => Some(fs::File::from(fd)),
        Err(rustix::io::Errno::NOENT) => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(directory) = directory {
        ownership.require_directory(&directory)?;
        unlinkat(&parent, name, AtFlags::REMOVEDIR)?;
    }
    fs::File::from(openat(
        &parent,
        c".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
    .sync_all()
}
