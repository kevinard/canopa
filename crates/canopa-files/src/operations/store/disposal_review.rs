//! Explicitly retain original disposal namespaces before retiring their journal.
use super::*;

#[derive(Clone, Debug)]
pub(in crate::operations) struct Review {
    undo: bool,
    lane: IntentLane,
    pending: String,
    binding: String,
    access: String,
    roots: staging::disposal::Review,
}
impl Review {
    pub(in crate::operations) fn is_undo(&self) -> bool {
        self.undo
    }
}

fn bodies(
    connection: &Connection,
    lane: IntentLane,
) -> io::Result<Option<(String, String, String)>> {
    let pending: Option<String> = connection
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
    let Some(pending) = pending else {
        return Ok(None);
    };
    let undo = matches!(decode::<Pending>(&pending)?, Pending::Disposing { disposal } if disposal.undo.is_some());
    if undo && !matches!(lane, IntentLane::Primary) {
        return Err(invalid("Undo disposal belongs to its primary journal."));
    }
    let binding: Option<String> = connection
        .query_row(
            if undo {
                "SELECT body FROM undo_work WHERE singleton=1 AND ?1=0"
            } else {
                "SELECT body FROM stage_intents WHERE lane=?1"
            },
            [lane.index()],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let Some(binding) = binding else {
        return Ok(None);
    };
    let mut statement = connection
        .prepare("SELECT id,body FROM directory_access WHERE lane=?1 ORDER BY id")
        .map_err(db_error)?;
    let access = statement
        .query_map([lane.index()], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)?;
    Ok(Some((pending, binding, encode(&access)?)))
}

fn require(
    connection: &Connection,
    history: Identity,
    lane: IntentLane,
    pending: &Pending,
    problems: Problems,
) -> io::Result<()> {
    stage_intent::require(connection, history, lane, pending, problems)?;
    context::require_forward(connection, history, lane, pending)?;
    transfer_cursor::require_forward(connection, history, lane, pending)
}

impl Store {
    /// A live transfer still owns this exact failed task. Retaining its data
    /// permits failure reporting and unrelated batch work, never replaying it.
    pub(in crate::operations) fn retain_failed_partial(
        &mut self,
        id: u64,
        destination: &Path,
    ) -> io::Result<()> {
        let Some(Pending::Disposing { disposal }) = self.pending()? else {
            return Err(invalid("The failed copy has no original disposal journal."));
        };
        if disposal.id != id || disposal.destination != destination {
            return Err(invalid("The failed copy differs from its original task."));
        }
        let slot = (self.lane.index() * 4) as u8;
        let reviewed = self
            .disposal_review(slot)?
            .ok_or_else(|| invalid("The failed copy needs Recovery review before more work."))?;
        self.keep_disposal(&reviewed)
    }
    pub(in crate::operations) fn disposal_review(&self, slot: u8) -> io::Result<Option<Review>> {
        let Some((lane, _)) = interrupted::pending_position(slot) else {
            return Ok(None);
        };
        if self.hide_running_journals && self.recovery_problem(lane).is_none() {
            return Ok(None);
        }
        let Some((pending, binding, access)) = bodies(&self.connection, lane)? else {
            return Ok(None);
        };
        let decoded = decode(&pending)?;
        let Pending::Disposing { disposal } = &decoded else {
            return Ok(None);
        };
        require(
            &self.connection,
            self.identity,
            lane,
            &decoded,
            self.problems.clone(),
        )?;
        let roots = staging::disposal::Retention::open(disposal)?.review();
        Ok(Some(Review {
            undo: disposal.undo.is_some(),
            lane,
            pending,
            binding,
            access,
            roots,
        }))
    }

    pub(in crate::operations) fn keep_disposal(&mut self, reviewed: &Review) -> io::Result<()> {
        self.ensure_location()?;
        let pending: Pending = decode(&reviewed.pending)?;
        let Pending::Disposing { disposal } = &pending else {
            return Err(invalid("Expected interrupted disposal work."));
        };
        let retained = staging::disposal::Retention::open(disposal)?;
        retained.require(&reviewed.roots)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if bodies(&transaction, reviewed.lane)?
            != Some((
                reviewed.pending.clone(),
                reviewed.binding.clone(),
                reviewed.access.clone(),
            ))
        {
            return Err(invalid(
                "Interrupted work changed; refresh Recovery before keeping it.",
            ));
        }
        require(
            &transaction,
            self.identity,
            reviewed.lane,
            &pending,
            self.problems.clone(),
        )?;
        // Archive whole roots and leaf identities in the same transaction. Old
        // selections keep resolving while arbitrary additional children remain
        // copyable from the root. No path is moved, removed or freshly adopted.
        for part in 0..8 {
            if !reviewed.roots.contains(part) {
                continue;
            }
            let mut record = pending
                .record(part, "Kept interrupted working files".into())
                .ok_or_else(|| invalid("Invalid interrupted work folder."))?;
            record.retained.version = Some(record.available()?);
            let body = encode(&record)?;
            if transaction
                .execute("INSERT INTO garbage(body) VALUES(?1)", [&body])
                .map_err(db_error)?
                != 1
            {
                return Err(invalid("The retained work was not recorded."));
            }
            let entry = u64::try_from(transaction.last_insert_rowid()).map_err(io::Error::other)?;
            let recorded: String = transaction
                .query_row("SELECT body FROM garbage WHERE id=?1", [entry], |row| {
                    row.get(0)
                })
                .map_err(db_error)?;
            if recorded != body {
                return Err(invalid("The retained work changed while saving."));
            }
            paths::index_record(&transaction, entry, &record)?;
            paths::require_retained_record(&transaction, entry, &record)?;
        }
        retained.require(&reviewed.roots)?;
        clear_intent(&transaction, reviewed.lane)?;
        let remains: bool = transaction.query_row(match reviewed.lane {
            IntentLane::Primary => "SELECT EXISTS(SELECT 1 FROM pending) OR EXISTS(SELECT 1 FROM stage_intents WHERE lane=0) OR EXISTS(SELECT 1 FROM pending_bindings WHERE lane=0) OR EXISTS(SELECT 1 FROM undo_work)",
            IntentLane::RecoveryCopy => "SELECT EXISTS(SELECT 1 FROM recovery_pending) OR EXISTS(SELECT 1 FROM stage_intents WHERE lane=1) OR EXISTS(SELECT 1 FROM pending_bindings WHERE lane=1)",
        }, [], |row| row.get(0)).map_err(db_error)?;
        if remains {
            return Err(invalid(
                "The interrupted work was not retired. Its Recovery records were not changed.",
            ));
        }
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
