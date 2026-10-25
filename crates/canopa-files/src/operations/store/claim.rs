//! Transfer an explicitly reviewed Rename claim's ownership without moving data.
use super::*;

impl Store {
    pub(in crate::operations) fn rename_claim_review(&self) -> io::Result<Option<String>> {
        if self.hide_running_journals && self.recovery_problem(IntentLane::Primary).is_none() {
            return Ok(None);
        }
        let body: Option<String> = self
            .connection
            .query_row("SELECT body FROM pending WHERE singleton=1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(db_error)?;
        let Some(body) = body else { return Ok(None) };
        let pending: Pending = decode(&body)?;
        let Pending::Forward {
            change:
                Change::Rename {
                    retained: None,
                    saved: None,
                    ..
                },
            stage: Some(_),
            ..
        } = &pending
        else {
            return Ok(None);
        };
        if matches!(&pending, Pending::Forward { stage: Some(stage), .. } if stage.entry_move.is_some())
        {
            return Ok(None);
        }
        if !self.directory_access_paths(IntentLane::Primary)?.is_empty() {
            return Ok(None);
        }
        if let Pending::Forward { change, stage, .. } = &pending {
            validate_change(change)?;
            validate_claim_stage(stage.as_ref(), change.forward_path())?;
        }
        Ok(Some(body))
    }

    pub(in crate::operations) fn keep_rename_claim(
        &mut self,
        reviewed: &str,
        record: &recovery::Record,
    ) -> io::Result<()> {
        self.ensure_location()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pending WHERE singleton=1 AND body=?1)
             AND NOT EXISTS(SELECT 1 FROM directory_access WHERE lane=0)",
                [reviewed],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if !current {
            return Err(invalid(
                "Interrupted work changed; refresh Recovery before keeping it.",
            ));
        }
        archive(&transaction, Some(record))?;
        clear_intent(&transaction, IntentLane::Primary)?;
        transaction.commit().map_err(db_error)?;
        self.problems
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[IntentLane::Primary.index()] = None;
        Ok(())
    }
}
