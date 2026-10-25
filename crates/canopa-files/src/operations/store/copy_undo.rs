//! An original Copy Undo creator binds its task before either name is consumed.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    history: original::Original,
    canonical: [u8; 32],
    pending: [u8; 32],
}
#[derive(Clone, Debug)]
pub(in crate::operations) struct Review {
    lane: IntentLane,
    pending: String,
    binding: String,
    root: fingerprint::Version,
    finishes_saved_return: bool,
}
impl Review {
    pub fn finishes_saved_return(&self) -> bool {
        self.finishes_saved_return
    }
}
pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection.execute_batch("CREATE TABLE IF NOT EXISTS copy_undo (lane INTEGER PRIMARY KEY CHECK(lane IN(0,1)),body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=262144));").map_err(db_error)
}
fn body(connection: &Connection, lane: IntentLane) -> io::Result<Option<String>> {
    connection
        .query_row(
            "SELECT body FROM copy_undo WHERE lane=?1",
            [lane.index()],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)
}
fn pending(connection: &Connection, lane: IntentLane) -> io::Result<Option<String>> {
    connection
        .query_row(
            match lane {
                IntentLane::Primary => "SELECT body FROM pending WHERE singleton=1",
                IntentLane::RecoveryCopy => "SELECT body FROM recovery_pending WHERE singleton=1",
            },
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)
}
fn digest(lane: IntentLane, removal: &undo::copied::Removal) -> io::Result<[u8; 32]> {
    let mut hash = Sha256::new();
    hash.update(b"canopa-files-original-copy-undo-v1\0");
    hash.update(encode(&(lane.index(), removal))?.as_bytes());
    Ok(hash.finalize().into())
}
fn canonical_digest(
    connection: &Connection,
    removal: &undo::copied::Removal,
) -> io::Result<[u8; 32]> {
    let canonical = stored_change(connection, removal.id, removal.ordinal)?;
    let mut approved = canonical.clone();
    if let (
        Change::Copied {
            retained: Some(expected),
            ..
        },
        Change::Copied {
            retained: Some(selected),
            ..
        },
    ) = (&mut approved, &removal.change)
        && selected.version.is_some()
        && !selected.recovery
    {
        // Only the exact version/recovery fields changed by the existing live
        // UseSavedVersion guard may differ. The creator separately verifies the
        // actual selected generation/version; the canonical digest stays old.
        expected.version = selected.version.clone();
        expected.recovery = false;
    }
    if encode(&approved)? != encode(&removal.change)? {
        return Err(invalid("Copy Undo differs from its original history step."));
    }
    Ok(Sha256::digest(encode(&canonical)?.as_bytes()).into())
}
fn canonical_matches(
    connection: &Connection,
    removal: &undo::copied::Removal,
    expected: [u8; 32],
) -> io::Result<bool> {
    let body: String = connection
        .query_row(
            "SELECT body FROM changes WHERE operation=?1 AND ordinal=?2",
            params![removal.id, removal.ordinal],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if <[u8; 32]>::from(Sha256::digest(body.as_bytes())) == expected {
        // Registration proved the selected task against this canonical Change.
        // The independent pending digest below preserves that selection through
        // every phase. Exact canonical bytes need no repeated deserialization.
        return Ok(true);
    }
    // Historical rows may have different JSON formatting or omitted defaults.
    // Preserve the original normalized check for those existing bindings.
    Ok(canonical_digest(connection, removal)? == expected)
}
pub(super) fn require(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    value: &Pending,
) -> io::Result<()> {
    let saved = body(connection, lane)?;
    let Pending::UndoCopied { removal } = value else {
        return if saved.is_none() {
            Ok(())
        } else {
            Err(invalid("Copy Undo lost its original journal."))
        };
    };
    removal.validate()?;
    let binding: Binding = decode(&saved.ok_or_else(|| {
        invalid("Copy Undo lost its original task binding. Its work folder is preserved.")
    })?)?;
    binding
        .history
        .require(&transfer_completion::history_file(connection, history)?)?;
    if !canonical_matches(connection, removal, binding.canonical)?
        || binding.pending != digest(lane, removal)?
    {
        return Err(invalid(
            "Copy Undo differs from its original task. Its files are preserved for Recovery.",
        ));
    }
    Ok(())
}
pub(super) fn clear(connection: &Connection, lane: IntentLane) -> io::Result<()> {
    connection
        .execute("DELETE FROM copy_undo WHERE lane=?1", [lane.index()])
        .map_err(db_error)?;
    if body(connection, lane)?.is_some() {
        return Err(invalid("The original Copy Undo binding was not retired."));
    }
    Ok(())
}
impl Store {
    pub(in crate::operations) fn copy_undo_review(&self, slot: u8) -> io::Result<Option<Review>> {
        let Some((lane, part)) = interrupted::pending_position(slot) else {
            return Ok(None);
        };
        if part != 0 || (self.hide_running_journals && self.recovery_problem(lane).is_none()) {
            return Ok(None);
        }
        let Some(value @ Pending::UndoCopied { .. }) = self.pending_in(lane)? else {
            return Ok(None);
        };
        require(&self.connection, self.identity, lane, &value)?;
        let Pending::UndoCopied { removal } = &value else {
            unreachable!()
        };
        Ok(Some(Review {
            lane,
            pending: encode(&value)?,
            binding: body(&self.connection, lane)?
                .ok_or_else(|| invalid("Missing Copy Undo binding."))?,
            root: staging::disposal::container_version(&removal.stage)?,
            finishes_saved_return: removal.kept_saved_cleanup()?.is_some(),
        }))
    }
    pub(in crate::operations) fn keep_copy_undo(&mut self, reviewed: &Review) -> io::Result<()> {
        self.ensure_location()?;
        let value: Pending = decode(&reviewed.pending)?;
        let Pending::UndoCopied { removal } = &value else {
            return Err(invalid("Expected Copy Undo work."));
        };
        let observe = || staging::disposal::container_version(&removal.stage);
        if observe()? != reviewed.root {
            return Err(invalid("Copy Undo work changed; refresh Recovery."));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        require(&transaction, self.identity, reviewed.lane, &value)?;
        if pending(&transaction, reviewed.lane)?.as_deref() != Some(reviewed.pending.as_str())
            || body(&transaction, reviewed.lane)?.as_deref() != Some(reviewed.binding.as_str())
        {
            return Err(invalid("Copy Undo changed; refresh Recovery."));
        }
        let mut record = value
            .record(0, "Kept interrupted Copy Undo work folder".into())
            .ok_or_else(|| invalid("Missing Copy Undo work folder."))?;
        record.retained.version = Some(reviewed.root.clone());
        let encoded = encode(&record)?;
        if transaction
            .execute("INSERT INTO garbage(body) VALUES(?1)", [&encoded])
            .map_err(db_error)?
            != 1
        {
            return Err(invalid("Copy Undo work was not retained."));
        }
        let entry = u64::try_from(transaction.last_insert_rowid()).map_err(io::Error::other)?;
        let actual: String = transaction
            .query_row("SELECT body FROM garbage WHERE id=?1", [entry], |r| {
                r.get(0)
            })
            .map_err(db_error)?;
        if actual != encoded {
            return Err(invalid("Retained Copy Undo work changed while saving."));
        }
        paths::index_record(&transaction, entry, &record)?;
        paths::require_retained_record(&transaction, entry, &record)?;
        if observe()? != reviewed.root {
            return Err(invalid("Copy Undo work changed while keeping it."));
        }
        if let Some(saved) = removal.kept_saved_cleanup()? {
            // The copied payload now has its nonexpiring archive owner. Commit
            // the recorded return, retaining only original saved-root cleanup;
            // no completed destination is opened or executed again.
            let cleanup = reverse_cleanup::completion(
                &transaction,
                self.identity,
                reviewed.lane,
                removal.id,
                removal.ordinal,
                vec![saved],
            )?;
            reverse_transaction(
                &transaction,
                reviewed.lane,
                removal.id,
                removal.ordinal,
                &removal.change,
                None,
                None,
            )?;
            cleanup.install(&transaction, reviewed.lane)?;
        } else {
            clear_intent(&transaction, reviewed.lane)?;
        }
        transaction.commit().map_err(db_error)?;
        self.problems
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[reviewed.lane.index()] = None;
        match reviewed.lane {
            IntentLane::Primary => {
                let result = reverse_cleanup::recover(self);
                self.recovery_result(&result);
                result?;
                self.restore_directory_access()
            }
            IntentLane::RecoveryCopy => self.in_recovery_copy(|state| {
                let result = reverse_cleanup::recover(state);
                state.recovery_result(&result);
                result?;
                state.restore_directory_access()
            }),
        }
    }
    pub(in crate::operations) fn begin_copy_undo(
        &mut self,
        prepared: &undo::copied::Prepared,
    ) -> io::Result<()> {
        self.ensure_location()?;
        prepared.require()?;
        if self.pending()?.is_some() || body(&self.connection, self.lane)?.is_some() {
            return Err(invalid("Another filesystem journal still owns this lane."));
        }
        let removal = prepared.removal();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let canonical = canonical_digest(&transaction, removal)?;
        let value = Pending::UndoCopied {
            removal: Box::new(removal.clone()),
        };
        source_retention::check_intent(&transaction, &value)?;
        context::prepare(&transaction, self.identity, self.lane, &value)?;
        let binding = Binding {
            history: original::Original::capture(&transfer_completion::history_file(
                &transaction,
                self.identity,
            )?)?,
            canonical,
            pending: digest(self.lane, removal)?,
        };
        let encoded = encode(&value)?;
        let bound = encode(&binding)?;
        transaction
            .execute(
                match self.lane {
                    IntentLane::Primary => "INSERT INTO pending VALUES(1,?1)",
                    IntentLane::RecoveryCopy => "INSERT INTO recovery_pending VALUES(1,?1)",
                },
                [&encoded],
            )
            .map_err(db_error)?;
        transaction
            .execute(
                "INSERT INTO copy_undo(lane,body) VALUES(?1,?2)",
                params![self.lane.index(), bound],
            )
            .map_err(db_error)?;
        if pending(&transaction, self.lane)?.as_deref() != Some(encoded.as_str())
            || body(&transaction, self.lane)?.as_deref() != Some(bound.as_str())
        {
            return Err(invalid(
                "The original Copy Undo task was not saved exactly.",
            ));
        }
        prepared.require()?;
        transaction.commit().map_err(db_error)
    }
    pub(in crate::operations) fn require_copy_undo(
        &self,
        removal: &undo::copied::Removal,
    ) -> io::Result<()> {
        let value = Pending::UndoCopied {
            removal: Box::new(removal.clone()),
        };
        if pending(&self.connection, self.lane)?.as_deref() != Some(encode(&value)?.as_str()) {
            return Err(invalid("The Copy Undo journal no longer owns this task."));
        }
        require(&self.connection, self.identity, self.lane, &value)
    }
    pub(in crate::operations) fn copy_undo_phase(
        &mut self,
        transition: &undo::copied::Transition<'_>,
    ) -> io::Result<()> {
        self.ensure_location()?;
        let before = Pending::UndoCopied {
            removal: Box::new(transition.before().clone()),
        };
        let after = Pending::UndoCopied {
            removal: Box::new(transition.after()),
        };
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let old = pending(&transaction, self.lane)?
            .ok_or_else(|| invalid("The Copy Undo journal is missing."))?;
        if old != encode(&before)? {
            return Err(invalid("Copy Undo changed before its phase was saved."));
        }
        require(&transaction, self.identity, self.lane, &before)?;
        source_retention::check_intent(&transaction, &after)?;
        transition.require()?;
        let old_binding = body(&transaction, self.lane)?
            .ok_or_else(|| invalid("The Copy Undo binding is missing."))?;
        let mut binding: Binding = decode(&old_binding)?;
        let Pending::UndoCopied {
            removal: after_removal,
        } = &after
        else {
            unreachable!();
        };
        binding.pending = digest(self.lane, after_removal)?;
        let next = encode(&after)?;
        let bound = encode(&binding)?;
        transaction
            .execute(
                match self.lane {
                    IntentLane::Primary => {
                        "UPDATE pending SET body=?1 WHERE singleton=1 AND body=?2"
                    }
                    IntentLane::RecoveryCopy => {
                        "UPDATE recovery_pending SET body=?1 WHERE singleton=1 AND body=?2"
                    }
                },
                params![next, old],
            )
            .map_err(db_error)?;
        transaction
            .execute(
                "UPDATE copy_undo SET body=?2 WHERE lane=?1 AND body=?3",
                params![self.lane.index(), bound, old_binding],
            )
            .map_err(db_error)?;
        if pending(&transaction, self.lane)?.as_deref() != Some(next.as_str())
            || body(&transaction, self.lane)?.as_deref() != Some(bound.as_str())
        {
            return Err(invalid(
                "The Copy Undo phase and binding were not saved exactly.",
            ));
        }
        transition.require()?;
        transaction.commit().map_err(db_error)
    }
}
