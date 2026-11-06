//! Creator-bound Undo return work. Retention retires metadata, never file names.
use super::*;
mod displaced;
mod disposal;
pub(super) use disposal::{reproduce, transition as disposal_transition};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    history: original::Original,
    operation: u64,
    ordinal: u64,
    #[serde(serialize_with = "serialized::bytes")]
    canonical: [u8; 32],
    #[serde(serialize_with = "serialized::bytes")]
    pending: [u8; 32],
    source: Option<fingerprint::source::Reproduction>,
}
#[derive(Clone, Debug)]
pub(in crate::operations) struct Review {
    pending: String,
    binding: String,
    access: String,
    root: fingerprint::Version,
    payload: Option<fingerprint::Version>,
    displaced: Option<undo::displaced::Review>,
}
pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection.execute_batch("CREATE TABLE IF NOT EXISTS undo_work (singleton INTEGER PRIMARY KEY CHECK(singleton=1),body TEXT NOT NULL);").map_err(db_error)
}
pub(super) fn clear(connection: &Connection) -> io::Result<()> {
    connection
        .execute_cached("DELETE FROM undo_work", [])
        .map_err(db_error)?;
    if body(connection)?.is_some() {
        return Err(invalid("The original Undo work binding was not retired."));
    }
    Ok(())
}
fn body(connection: &Connection) -> io::Result<Option<String>> {
    connection
        .query_row_cached("SELECT body FROM undo_work WHERE singleton=1", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(db_error)
}
fn pending(connection: &Connection) -> io::Result<Option<String>> {
    connection
        .query_row_cached("SELECT body FROM pending WHERE singleton=1", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(db_error)
}
fn task(pending: &Pending) -> io::Result<(u64, u64, &Change, &Stage)> {
    let (id, ordinal, change, stage) = owner(pending)?;
    let stage = stage
        .or(direct_preparation(pending).map(|record| &record.retained.stage))
        .ok_or_else(|| invalid("Undo return preparation is not yet registered."))?;
    Ok((id, ordinal, change, stage))
}
pub(in crate::operations) fn direct_preparation(pending: &Pending) -> Option<&recovery::Record> {
    let pending = match pending {
        Pending::Disposing { disposal } => disposal.undo.as_deref()?,
        value => value,
    };
    match pending {
        Pending::Undo {
            change:
                Change::Copied { retained: None, .. }
                | Change::SourceRemoved { .. }
                | Change::Rename { .. }
                | Change::Trashed { .. }
                | Change::RetainedOnly { .. },
            stage: None,
            displaced: Some(record),
            ..
        } => Some(record),
        _ => None,
    }
}

fn review_payload(pending: &Pending, stage: &Stage) -> PathBuf {
    direct_preparation(pending).map_or_else(|| stage.payload(), |record| record.retained.path())
}
fn review_displaced(pending: &Pending) -> io::Result<Option<undo::displaced::Review>> {
    if direct_preparation(pending).is_some() {
        return Ok(None);
    }
    match pending {
        Pending::Undo {
            displaced: Some(record),
            ..
        } => undo::displaced::observe(record).map(Some),
        _ => Ok(None),
    }
}
fn owner(pending: &Pending) -> io::Result<(u64, u64, &Change, Option<&Stage>)> {
    let pending = origin(pending)?;
    let Pending::Undo {
        id,
        ordinal,
        change,
        stage,
        displaced,
        ..
    } = pending
    else {
        return Err(invalid("Expected original Undo return work."));
    };
    if !(matches!(change, Change::SourceRemoved { .. }) || direct_preparation(pending).is_some()) {
        return Err(invalid(
            "Undo return work needs its original created folder proof.",
        ));
    }
    if let Some(displaced) = displaced {
        undo::displaced::validate(
            displaced,
            change
                .reverse_path()
                .unwrap_or_else(|| change.forward_path()),
        )?;
        if stage.as_ref().is_some_and(|stage| {
            stage.directory == displaced.retained.stage.directory
                || stage.identity == displaced.retained.stage.identity
                || stage.ownership.as_ref().map(|o| o.roots().0)
                    != displaced
                        .retained
                        .stage
                        .ownership
                        .as_ref()
                        .map(|o| o.roots().0)
        }) {
            return Err(invalid(
                "Undo return and displaced work need distinct created roots.",
            ));
        }
    } else if stage.is_none() {
        return Err(invalid("Undo has no original work preparation."));
    }
    if let Some(stage) = stage {
        if stage.ownership.is_none() {
            return Err(invalid("Undo work lost its original created roots."));
        }
        validate_stage(Some(stage), change.forward_path())?;
    }
    Ok((*id, *ordinal, change, stage.as_ref()))
}
fn origin(pending: &Pending) -> io::Result<&Pending> {
    if let Pending::Disposing { disposal } = pending {
        disposal.validate()?;
        return disposal
            .undo
            .as_deref()
            .ok_or_else(|| invalid("Expected original Undo disposal work."));
    }
    Ok(pending)
}
fn canonical(
    connection: &Connection,
    id: u64,
    ordinal: u64,
    change: &Change,
    pending: &Pending,
) -> io::Result<[u8; 32]> {
    let stored = stored_change(connection, id, ordinal)?;
    let mut expected = match (&stored, change) {
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
        _ => stored.clone(),
    };
    if let (
        Change::Rename {
            reviewed: selected,
            retained: selected_previous,
            ..
        },
        Change::Rename {
            reviewed, retained, ..
        },
    ) = (change, &mut expected)
    {
        *reviewed = selected.clone();
        // A live source-only review can defer an unavailable saved version.
        // The binding still hashes the full original history below.
        if selected_previous.is_none() {
            *retained = None;
        }
    }
    // ReturnItem can defer an unavailable previous destination. The task omits
    // that retained record; the digest below still binds the complete history.
    if let (Change::Rename { retained: None, .. }, Change::Rename { retained, .. }) =
        (change, &mut expected)
    {
        *retained = None;
    }
    if let (Some(selected), Some(retained)) = (change.retained(), expected.retained_mut())
        && selected.version.is_some()
        && !selected.recovery
    {
        retained.version = selected.version.clone();
        retained.recovery = false;
    }
    if let Change::SourceRemoved {
        fingerprint,
        source_fingerprint,
        reviewed: Some(reviewed),
        ..
    } = change
    {
        if fingerprint != source_fingerprint {
            return Err(invalid("Reviewed Undo return fingerprints disagree."));
        }
        let Change::SourceRemoved {
            fingerprint: expected_fingerprint,
            source_fingerprint: expected_source,
            reviewed: expected_review,
            ..
        } = &mut expected
        else {
            return Err(invalid("Unsupported reviewed Undo return."));
        };
        // A live review can select newer work without rewriting the original
        // Move history. Registration checks the actual opened reviewed source;
        // the binding below still hashes that unchanged history body.
        *expected_fingerprint = fingerprint.clone();
        *expected_source = source_fingerprint.clone();
        *expected_review = Some(reviewed.clone());
    }
    if let Pending::Undo {
        relocation: Some(movement),
        ..
    } = pending
    {
        movement.validate()?;
        let original = match &mut expected {
            Change::SourceRemoved { original, .. }
            | Change::Rename { original, .. }
            | Change::Trashed { original, .. } => original,
            Change::RetainedOnly { path, .. } => path,
            _ => return Err(invalid("Unsupported Undo work relocation.")),
        };
        if original != &movement.from {
            return Err(invalid(
                "Undo relocation differs from its original history path.",
            ));
        }
        *original = movement.to.clone();
    }
    if encode(&expected)? != encode(change)? {
        return Err(invalid(
            "Undo return work differs from its original history step.",
        ));
    }
    Ok(serialized::digest(encode(&stored)?.as_bytes()))
}
pub(super) fn require(
    connection: &Connection,
    history: Identity,
    value: &Pending,
) -> io::Result<()> {
    let expected = matches!(
        value,
        Pending::Undo {
            change: Change::SourceRemoved { .. } | Change::Copied { retained: None, .. },
            stage,
            displaced,
            ..
        } if stage.is_some() || displaced.is_some()
    ) || direct_preparation(value).is_some()
        || matches!(value, Pending::Disposing { disposal } if disposal.undo.is_some());
    let Some(body) = body(connection)? else {
        return if expected {
            Err(invalid(
                "The interrupted Undo return has no original work binding. Its files are preserved for Recovery.",
            ))
        } else {
            Ok(())
        };
    };
    let binding: Binding = decode(&body)?;
    let (id, ordinal, change, _) = owner(value)?;
    binding
        .history
        .require(&transfer_completion::history_file(connection, history)?)?;
    if binding.operation != id
        || binding.ordinal != ordinal
        || binding.canonical != canonical(connection, id, ordinal, change, origin(value)?)?
        || binding.pending != serialized::digest(encode(value)?.as_bytes())
    {
        return Err(invalid(
            "Interrupted Undo work changed from its original binding. Its files are preserved for Recovery.",
        ));
    }
    Ok(())
}
pub(super) fn preserve(
    connection: &Connection,
    history: Identity,
    next: &Pending,
) -> io::Result<bool> {
    if body(connection)?.is_none() {
        return Ok(false);
    }
    let current = pending(connection)?
        .ok_or_else(|| invalid("The original Undo work journal is missing."))?;
    let current: Pending = decode_pending(&current)?;
    require(connection, history, &current)?;
    if encode(&current)? != encode(next)? {
        return Err(invalid(
            "Undo return work needs its original private transition.",
        ));
    }
    Ok(true)
}
fn access(connection: &Connection) -> io::Result<String> {
    let mut q = connection
        .prepare_cached("SELECT id,body FROM directory_access WHERE lane=0 ORDER BY id")
        .map_err(db_error)?;
    let rows = q
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)?;
    encode(&rows)
}
impl Store {
    pub(in crate::operations) fn register_undo_work(
        &mut self,
        value: &Pending,
        created: &staging::Registration<'_>,
    ) -> io::Result<()> {
        self.ensure_location()?;
        if !matches!(self.lane, IntentLane::Primary) {
            return Err(invalid("Undo return work belongs to the primary journal."));
        }
        created.require()?;
        let (id, ordinal, change, stage) = task(value)?;
        if created.stage() != stage {
            return Err(invalid("Undo work differs from its actual creator."));
        }
        let Change::SourceRemoved {
            current, reviewed, ..
        } = change
        else {
            unreachable!();
        };
        if created.source() != current {
            return Err(invalid(
                "Undo work differs from its selected return source.",
            ));
        }
        if let Some(reviewed) = reviewed {
            created.require_reviewed(reviewed)?;
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        source_retention::check_intent(&transaction, value)?;
        let canonical = canonical(&transaction, id, ordinal, change, value)?;
        let old =
            pending(&transaction)?.ok_or_else(|| invalid("Undo work lost its original intent."))?;
        let mut expected = value.clone();
        let previous: Pending = decode_pending(&old)?;
        let Pending::Undo { stage: before, .. } = &previous else {
            return Err(invalid("Another operation owns this file journal."));
        };
        let Pending::Undo { stage: after, .. } = &mut expected else {
            unreachable!();
        };
        match before {
            None => {
                if body(&transaction)?.is_some() {
                    require(&transaction, self.identity, &previous)?;
                }
                *after = None;
            }
            Some(before) => {
                require(&transaction, self.identity, &previous)?;
                if before.payload_identity.is_some() {
                    return Err(invalid("Undo work already registered its payload."));
                }
                let after = after.as_mut().unwrap();
                after.payload_identity = before.payload_identity;
                after.ownership = before.ownership.clone();
            }
        }
        if encode(&expected)? != old {
            return Err(invalid("Undo preparation differs from its original task."));
        }
        let source = created.reproduction();
        let original = if let Some(body) = body(&transaction)? {
            let old: Binding = decode(&body)?;
            if encode(&old.source)? != encode(&source)?
                && !(before.is_none() && old.source.is_none())
            {
                return Err(invalid(
                    "Undo preparation changed its original source proof.",
                ));
            }
            old.history
        } else {
            original::Original::capture(&transfer_completion::history_file(
                &transaction,
                self.identity,
            )?)?
        };
        let encoded = encode(value)?;
        let binding = encode(&Binding {
            history: original,
            operation: id,
            ordinal,
            canonical,
            pending: serialized::digest(encoded.as_bytes()),
            source,
        })?;
        if transaction.execute_cached("INSERT OR REPLACE INTO pending VALUES(1,?1)",[&encoded]).map_err(db_error)?!=1
            ||transaction.execute_cached("INSERT INTO undo_work VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET body=excluded.body",[&binding]).map_err(db_error)?!=1
            ||pending(&transaction)?.as_deref()!=Some(encoded.as_str())||body(&transaction)?.as_deref()!=Some(binding.as_str()) {
            return Err(invalid("Undo preparation was not saved with its original binding."));
        }
        require(&transaction, self.identity, value)?;
        transaction.commit().map_err(db_error)?;
        #[cfg(test)]
        undo::abort_boundary::displaced_boundary::empty_copy(value);
        Ok(())
    }
    pub(in crate::operations) fn undo_work_review(&self, slot: u8) -> io::Result<Option<Review>> {
        if slot > 2
            || self.hide_running_journals && self.recovery_problem(IntentLane::Primary).is_none()
        {
            return Ok(None);
        }
        let Some(pending) = pending(&self.connection)? else {
            return Ok(None);
        };
        let decoded: Pending = decode_pending(&pending)?;
        if direct_preparation(&decoded).is_none()
            && !matches!(
                decoded,
                Pending::Undo {
                    change: Change::SourceRemoved { .. },
                    stage: Some(_),
                    ..
                }
            )
        {
            return Ok(None);
        }
        require(&self.connection, self.identity, &decoded)?;
        let (_, _, _, stage) = task(&decoded)?;
        Ok(Some(Review {
            pending,
            binding: body(&self.connection)?
                .ok_or_else(|| invalid("Missing Undo work binding."))?,
            access: access(&self.connection)?,
            root: staging::disposal::container_version(stage)?,
            payload: fingerprint::Version::read(&review_payload(&decoded, stage)).ok(),
            displaced: review_displaced(&decoded)?,
        }))
    }
    pub(in crate::operations) fn keep_undo_work(&mut self, review: &Review) -> io::Result<()> {
        self.ensure_location()?;
        let value: Pending = decode_pending(&review.pending)?;
        let (_, _, change, stage) = task(&value)?;
        let check = || -> io::Result<()> {
            if staging::disposal::container_version(stage)? != review.root
                || fingerprint::Version::read(&review_payload(&value, stage)).ok() != review.payload
            {
                return Err(invalid(
                    "Undo work changed; refresh Recovery before keeping it.",
                ));
            }
            let displaced = review_displaced(&value)?;
            if displaced != review.displaced {
                return Err(invalid(
                    "Displaced Undo work changed after this Recovery review.",
                ));
            }
            Ok(())
        };
        check()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if pending(&transaction)?.as_deref() != Some(review.pending.as_str())
            || body(&transaction)?.as_deref() != Some(review.binding.as_str())
            || access(&transaction)? != review.access
        {
            return Err(invalid("Undo work changed after this Recovery review."));
        }
        require(&transaction, self.identity, &value)?;
        let mut records = Vec::new();
        if direct_preparation(&value).is_none() {
            if let Some(record) = value.record(0, "Kept interrupted Undo return".into())
                && record.available().is_ok()
            {
                records.push(record);
            }
            records.push(recovery::Record {
                retained: Retained {
                    replacement_facts: None,
                    physical_identity: None,
                    stage: stage.clone(),
                    identity: stage.identity,
                    recovery: true,
                    version: None,
                },
                context: Some(recovery::Context {
                    original: change.forward_path().to_owned(),
                    label: "Kept interrupted Undo work folder".into(),
                }),
                context_moves: Vec::new(),
                payload: true,
                container: true,
            });
        }
        if let Pending::Undo {
            displaced: Some(record),
            ..
        } = &value
        {
            if record.available().is_ok() {
                records.push((**record).clone());
            }
            records.push(recovery::Record {
                context_moves: Vec::new(),
                retained: Retained {
                    identity: record.retained.stage.identity,
                    stage: record.retained.stage.clone(),
                    replacement_facts: None,
                    physical_identity: None,
                    recovery: true,
                    version: None,
                },
                context: record.context.clone(),
                payload: true,
                container: true,
            });
        }
        for mut record in records {
            record.retained.version = Some(record.available()?);
            let encoded = encode(&record)?;
            if transaction
                .execute_cached("INSERT INTO garbage(body) VALUES(?1)", [&encoded])
                .map_err(db_error)?
                != 1
            {
                return Err(invalid("Undo work was not retained."));
            }
            let entry = u64::try_from(transaction.last_insert_rowid()).map_err(io::Error::other)?;
            let saved: String = transaction
                .query_row_cached("SELECT body FROM garbage WHERE id=?1", [entry], |r| {
                    r.get(0)
                })
                .map_err(db_error)?;
            if saved != encoded {
                return Err(invalid("Retained Undo work changed while saving."));
            }
            paths::index_record(&transaction, entry, &record)?;
            paths::require_retained_record(&transaction, entry, &record)?;
        }
        check()?;
        clear_intent(&transaction, IntentLane::Primary)?;
        if pending(&transaction)?.is_some() || body(&transaction)?.is_some() {
            return Err(invalid(
                "The Undo journal was not retired. Its data remains protected.",
            ));
        }
        transaction.commit().map_err(db_error)?;
        self.problems
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[0] = None;
        self.restore_directory_access()
    }
}

/// A bound direct return already carries its selected incoming identity. Do not
/// reopen its completed source name when committing that return's owner context.
pub(in crate::operations::store) fn returned_context(
    connection: &Connection,
    lane: IntentLane,
    id: u64,
    ordinal: u64,
) -> io::Result<bool> {
    if !matches!(lane, IntentLane::Primary) {
        return Ok(false);
    }
    let Some(value) = pending(connection)? else {
        return Ok(false);
    };
    let value: Pending = decode_pending(&value)?;
    if direct_preparation(&value).is_none() {
        return Ok(false);
    }
    let (owner, step, change, _) = owner(&value)?;
    if owner != id || step != ordinal {
        return Err(invalid(
            "The recorded direct Undo return changed its history slot.",
        ));
    }
    let incoming = match change {
        Change::Rename {
            current,
            identity,
            reviewed,
            ..
        } => match reviewed {
            Some(version) => version.identity,
            None => references::resolve(connection, id, ordinal, current, *identity)?,
        },
        Change::RetainedOnly { .. } => return Ok(true),
        _ => return Ok(false),
    };
    context::returned_recorded(connection, id, ordinal, incoming)?;
    Ok(true)
}
