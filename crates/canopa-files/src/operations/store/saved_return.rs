//! Independently bind selected saved-return execution to its original history.
use super::*;
use sha2::{Digest, Sha256};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    history: original::Original,
    canonical: [u8; 32],
    pending: [u8; 32],
}
pub(super) fn create(c: &Connection) -> io::Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS saved_return(lane INTEGER PRIMARY KEY CHECK(lane IN(0,1)),body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=262144));").map_err(db_error)
}
fn body(c: &Connection, lane: IntentLane) -> io::Result<Option<String>> {
    c.query_row(
        "SELECT body FROM saved_return WHERE lane=?1",
        [lane.index()],
        |r| r.get(0),
    )
    .optional()
    .map_err(db_error)
}
fn pending(c: &Connection, lane: IntentLane) -> io::Result<Option<String>> {
    c.query_row(
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
fn digest(lane: IntentLane, task: &undo::saved_return::Return) -> io::Result<[u8; 32]> {
    let mut hash = Sha256::new();
    hash.update(b"canopa-files-saved-return-undo-v1\0");
    hash.update(encode(&(lane.index(), task))?.as_bytes());
    Ok(hash.finalize().into())
}
fn canonical(c: &Connection, task: &undo::saved_return::Return) -> io::Result<[u8; 32]> {
    let (id, ordinal, change, relocation, _) = task.parts()?;
    let canonical = stored_change(c, id, ordinal)?;
    let mut paired_copy = None;
    let mut expected = match (&canonical, change) {
        (
            Change::Copied {
                path,
                identity,
                retained: Some(retained),
                ..
            }
            | Change::Rename {
                current: path,
                identity,
                retained: Some(retained),
                ..
            },
            Change::RetainedOnly { .. },
        ) => Change::RetainedOnly {
            path: path.clone(),
            retained: retained.clone(),
            replaced: Some(*identity),
        },
        (
            Change::SourceRemoved {
                current,
                copy_identity,
                ..
            },
            Change::RetainedOnly { .. },
        ) => {
            let (_, saved) = cross::saved_only(c, id, ordinal, change)?
                .ok_or_else(|| invalid("Missing original paired saved destination."))?;
            let (_, copied) = cross::paired(c, id, ordinal)?
                .ok_or_else(|| invalid("Missing original paired Copy."))?;
            paired_copy = Some(copied);
            Change::RetainedOnly {
                path: current.clone(),
                retained: saved,
                replaced: Some(*copy_identity),
            }
        }
        _ => canonical.clone(),
    };
    if let (
        Change::Rename {
            reviewed: selected, ..
        },
        Change::Rename { reviewed, .. },
    ) = (change, &mut expected)
    {
        *reviewed = selected.clone();
    }
    if let (Some(selected), Some(retained)) = (change.retained(), expected.retained_mut())
        && selected.version.is_some()
        && !selected.recovery
    {
        retained.version = selected.version.clone();
        retained.recovery = false;
    }
    if let Some(movement) = relocation {
        movement.validate()?;
        let original = match &mut expected {
            Change::Rename { original, .. } => original,
            Change::RetainedOnly { path, .. } => path,
            _ => return Err(invalid("Unsupported saved return relocation.")),
        };
        if original != &movement.from {
            return Err(invalid(
                "Saved return relocation differs from its original history.",
            ));
        }
        *original = movement.to.clone();
    }
    if encode(&expected)? != encode(change)? {
        return Err(invalid(
            "Saved return differs from its original Undo history.",
        ));
    }
    Ok(Sha256::digest(
        match paired_copy {
            Some(copied) => encode(&("saved-only-pair-v1", &canonical, copied))?,
            None => encode(&canonical)?,
        }
        .as_bytes(),
    )
    .into())
}
fn canonical_matches(
    c: &Connection,
    task: &undo::saved_return::Return,
    expected: [u8; 32],
) -> io::Result<bool> {
    let (id, ordinal, _, _, _) = task.parts()?;
    let body: String = c
        .query_row(
            "SELECT body FROM changes WHERE operation=?1 AND ordinal=?2",
            params![id, ordinal],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    if <[u8; 32]>::from(Sha256::digest(body.as_bytes())) == expected {
        // Registration checked this selection against the canonical history.
        // Its separate task digest preserves that exact selection in each phase.
        return Ok(true);
    }
    // Existing rows can have different formatting or omitted default fields.
    Ok(canonical(c, task)? == expected)
}
pub(super) fn require(
    c: &Connection,
    history: Identity,
    lane: IntentLane,
    value: &Pending,
) -> io::Result<()> {
    let saved = body(c, lane)?;
    let Pending::UndoSavedReturn { returning } = value else {
        return if saved.is_none() {
            Ok(())
        } else {
            Err(invalid("The saved return lost its original journal."))
        };
    };
    returning.validate()?;
    let binding: Binding =
        decode(&saved.ok_or_else(|| {
            invalid("The saved return lost its independent original task binding.")
        })?)?;
    binding
        .history
        .require(&transfer_completion::history_file(c, history)?)?;
    if !canonical_matches(c, returning, binding.canonical)?
        || binding.pending != digest(lane, returning)?
    {
        return Err(invalid(
            "The saved return differs from its original task. Its data is preserved.",
        ));
    }
    Ok(())
}
pub(super) fn clear(c: &Connection, lane: IntentLane) -> io::Result<()> {
    c.execute("DELETE FROM saved_return WHERE lane=?1", [lane.index()])
        .map_err(db_error)?;
    if body(c, lane)?.is_some() {
        return Err(invalid("The saved return binding was not retired."));
    }
    Ok(())
}
impl Store {
    pub(in crate::operations) fn begin_saved_return(
        &mut self,
        prepared: &undo::saved_return::Prepared,
    ) -> io::Result<()> {
        self.ensure_location()?;
        prepared.require()?;
        let task = prepared.task();
        let (id, ordinal, _, _, _) = task.parts()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if pending(&transaction, self.lane)?.as_deref()
            != Some(encode(task.origin.as_ref())?.as_str())
            || body(&transaction, self.lane)?.is_some()
        {
            return Err(invalid(
                "The original Undo no longer owns this saved return.",
            ));
        }
        undo_work::require(&transaction, self.identity, task.origin.as_ref())?;
        context::require_change_history(&transaction, self.identity, id, ordinal)?;
        if let Change::Rename {
            current,
            identity,
            reviewed,
            ..
        } = task.parts()?.2
        {
            let expected = match reviewed {
                Some(version) => version.identity,
                None => references::resolve(&transaction, id, ordinal, current, *identity)?,
            };
            if task.incoming != Some(expected) {
                return Err(invalid(
                    "The saved return lost its recorded incoming identity.",
                ));
            }
        }
        let binding = Binding {
            history: original::Original::capture(&transfer_completion::history_file(
                &transaction,
                self.identity,
            )?)?,
            canonical: canonical(&transaction, task)?,
            pending: digest(self.lane, task)?,
        };
        let value = Pending::UndoSavedReturn {
            returning: Box::new(task.clone()),
        };
        source_retention::check_intent(&transaction, &value)?;
        let encoded = encode(&value)?;
        let bound = encode(&binding)?;
        transaction
            .execute(
                match self.lane {
                    IntentLane::Primary => "UPDATE pending SET body=?1 WHERE singleton=1",
                    IntentLane::RecoveryCopy => {
                        "UPDATE recovery_pending SET body=?1 WHERE singleton=1"
                    }
                },
                [&encoded],
            )
            .map_err(db_error)?;
        transaction
            .execute(
                "INSERT INTO saved_return(lane,body) VALUES(?1,?2)",
                params![self.lane.index(), bound],
            )
            .map_err(db_error)?;
        if pending(&transaction, self.lane)?.as_deref() != Some(encoded.as_str())
            || body(&transaction, self.lane)?.as_deref() != Some(bound.as_str())
        {
            return Err(invalid("The original saved return was not stored exactly."));
        }
        undo_work::clear(&transaction)?;
        prepared.require()?;
        transaction.commit().map_err(db_error)
    }
    pub(in crate::operations) fn require_saved_return(
        &self,
        task: &undo::saved_return::Return,
    ) -> io::Result<()> {
        let value = Pending::UndoSavedReturn {
            returning: Box::new(task.clone()),
        };
        if pending(&self.connection, self.lane)?.as_deref() != Some(encode(&value)?.as_str()) {
            return Err(invalid("The saved return no longer owns its journal."));
        }
        require(&self.connection, self.identity, self.lane, &value)
    }
    pub(in crate::operations) fn saved_return_phase(
        &mut self,
        transition: &undo::saved_return::Transition<'_>,
    ) -> io::Result<()> {
        self.ensure_location()?;
        let before = Pending::UndoSavedReturn {
            returning: Box::new(transition.before().clone()),
        };
        let next_task = transition.after();
        let after = Pending::UndoSavedReturn {
            returning: Box::new(next_task.clone()),
        };
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let old = encode(&before)?;
        if pending(&transaction, self.lane)?.as_deref() != Some(old.as_str()) {
            return Err(invalid(
                "The saved return journal changed before its phase was stored.",
            ));
        }
        require(&transaction, self.identity, self.lane, &before)?;
        if next_task.phase == undo::saved_return::Phase::Returning {
            source_retention::require_released(&transaction, next_task.stage.identity)?;
            source_retention::check_intent(&transaction, &after)?;
        }
        transition.require()?;
        let mut binding: Binding = decode(
            &body(&transaction, self.lane)?
                .ok_or_else(|| invalid("Missing saved return binding."))?,
        )?;
        binding.pending = digest(self.lane, &next_task)?;
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
                "UPDATE saved_return SET body=?2 WHERE lane=?1",
                params![self.lane.index(), bound],
            )
            .map_err(db_error)?;
        if pending(&transaction, self.lane)?.as_deref() != Some(next.as_str())
            || body(&transaction, self.lane)?.as_deref() != Some(bound.as_str())
        {
            return Err(invalid("The saved return phase was not stored exactly."));
        }
        transition.require()?;
        transaction.commit().map_err(db_error)
    }
}

#[derive(Clone, Debug)]
pub(in crate::operations) struct Review {
    pending: String,
    binding: String,
    roots: Vec<fingerprint::Version>,
}
fn roots(task: &undo::saved_return::Return) -> io::Result<Vec<&Stage>> {
    let mut roots = vec![&task.stage];
    if let Some(displaced) = task.parts()?.4 {
        if displaced.retained.stage.directory == task.stage.directory
            || displaced.retained.stage.identity == task.stage.identity
        {
            return Err(invalid(
                "Saved and displaced work need separate original roots.",
            ));
        }
        roots.push(&displaced.retained.stage);
    }
    Ok(roots)
}
fn observe(task: &undo::saved_return::Return) -> io::Result<Vec<fingerprint::Version>> {
    roots(task)?
        .into_iter()
        .map(staging::disposal::container_version)
        .collect()
}
fn whole_root(mut record: recovery::Record, version: &fingerprint::Version) -> recovery::Record {
    record.retained.identity = record.retained.stage.identity;
    record.retained.physical_identity = None;
    record.retained.replacement_facts = None;
    record.retained.recovery = true;
    record.retained.version = Some(version.clone());
    record.payload = true;
    record.container = true;
    record
}
impl Store {
    pub(in crate::operations) fn saved_return_review(
        &self,
        slot: u8,
    ) -> io::Result<Option<Review>> {
        let Some((lane, part)) = interrupted::pending_position(slot) else {
            return Ok(None);
        };
        if part != 0 || (self.hide_running_journals && self.recovery_problem(lane).is_none()) {
            return Ok(None);
        }
        let Some(value @ Pending::UndoSavedReturn { .. }) = self.pending_in(lane)? else {
            return Ok(None);
        };
        require(&self.connection, self.identity, lane, &value)?;
        let Pending::UndoSavedReturn { returning } = &value else {
            unreachable!()
        };
        Ok(Some(Review {
            pending: encode(&value)?,
            binding: body(&self.connection, lane)?
                .ok_or_else(|| invalid("Missing saved return binding."))?,
            roots: observe(returning)?,
        }))
    }
    pub(in crate::operations) fn keep_saved_return(&mut self, review: &Review) -> io::Result<()> {
        self.ensure_location()?;
        let value: Pending = decode(&review.pending)?;
        let Pending::UndoSavedReturn { returning } = &value else {
            return Err(invalid("Expected selected saved return work."));
        };
        if observe(returning)? != review.roots {
            return Err(invalid("Saved return work changed; refresh Recovery."));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        require(&transaction, self.identity, self.lane, &value)?;
        if pending(&transaction, self.lane)?.as_deref() != Some(review.pending.as_str())
            || body(&transaction, self.lane)?.as_deref() != Some(review.binding.as_str())
        {
            return Err(invalid("The saved return changed; refresh Recovery."));
        }
        let (id, ordinal, change, relocation, displaced) = returning.parts()?;
        let record = whole_root(
            value
                .record(0, "Kept interrupted saved destination return".into())
                .ok_or_else(|| invalid("Missing saved return work root."))?,
            &review.roots[0],
        );
        archive(&transaction, Some(&record))?;
        if let Some(displaced) = displaced {
            let record = whole_root(
                displaced.clone(),
                review
                    .roots
                    .get(1)
                    .ok_or_else(|| invalid("Missing displaced root review."))?,
            );
            archive(&transaction, Some(&record))?;
        }
        // Registration followed the actual incoming return. Keeping uncertain
        // saved work commits that source return only, never a saved destination
        // return that has not been proved. Both roots receive archive owners.
        let followed = if matches!(change, Change::Rename { .. }) {
            context::returned_recorded(
                &transaction,
                id,
                ordinal,
                returning
                    .incoming
                    .ok_or_else(|| invalid("Missing completed incoming return."))?,
            )?;
            follow_archived(&transaction, id, ordinal, change, true)?
        } else {
            BTreeSet::new()
        };
        if let Some(relocation) = relocation {
            relocate(&transaction, id, ordinal, relocation, &followed)?;
        }
        if observe(returning)? != review.roots {
            return Err(invalid("Saved return work changed while keeping it."));
        }
        complete_reversal(&transaction, id, ordinal, self.lane)?;
        transaction.commit().map_err(db_error)
    }
}

pub(super) fn returned_context(
    c: &Connection,
    lane: IntentLane,
    id: u64,
    ordinal: u64,
    change: &Change,
) -> io::Result<bool> {
    let Some(value) = pending(c, lane)? else {
        return Ok(false);
    };
    let value: Pending = decode(&value)?;
    let Pending::UndoSavedReturn { returning } = &value else {
        return Ok(false);
    };
    returning.validate()?;
    let (owner, step, actual, _, _) = returning.parts()?;
    if owner != id
        || step != ordinal
        || encode(actual)? != encode(change)?
        || returning.phase != undo::saved_return::Phase::Returned
    {
        return Err(invalid(
            "The recorded saved return differs from this completion.",
        ));
    }
    if let Some(identity) = returning.incoming {
        context::returned_recorded(c, id, ordinal, identity)?;
    }
    Ok(true)
}
