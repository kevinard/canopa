//! Bind an actual Undo conflict preparation before any return copy or displacement.
use super::*;
impl Store {
    pub(in crate::operations) fn register_undo_displaced(
        &mut self,
        value: &Pending,
        prepared: &undo::displaced::Prepared,
    ) -> io::Result<()> {
        self.ensure_location()?;
        if !matches!(self.lane, IntentLane::Primary) {
            return Err(invalid("Undo displacement belongs to the primary journal."));
        }
        let Pending::Undo {
            displaced: Some(record),
            stage: None,
            ..
        } = value
        else {
            return Err(invalid("Expected an actual Undo conflict preparation."));
        };
        prepared.require(record)?;
        let (id, ordinal, change, _) = owner(value)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if pending(&transaction)?.is_some() || body(&transaction)?.is_some() {
            return Err(invalid("Another operation owns the Undo journal."));
        }
        source_retention::check_intent(&transaction, value)?;
        let encoded = encode(value)?;
        let binding = encode(&Binding {
            history: original::Original::capture(&transfer_completion::history_file(
                &transaction,
                self.identity,
            )?)?,
            operation: id,
            ordinal,
            canonical: canonical(&transaction, id, ordinal, change, value)?,
            pending: Sha256::digest(encoded.as_bytes()).into(),
            source: None,
        })?;
        if transaction
            .execute("INSERT INTO pending VALUES(1,?1)", [&encoded])
            .map_err(db_error)?
            != 1
            || transaction
                .execute("INSERT INTO undo_work VALUES(1,?1)", [&binding])
                .map_err(db_error)?
                != 1
            || pending(&transaction)?.as_deref() != Some(encoded.as_str())
            || body(&transaction)?.as_deref() != Some(binding.as_str())
        {
            return Err(invalid(
                "The original Undo conflict preparation was not saved.",
            ));
        }
        require(&transaction, self.identity, value)?;
        transaction.commit().map_err(db_error)?;
        #[cfg(test)]
        {
            undo::abort_boundary::displaced_boundary::registered(value);
            undo::abort_boundary::direct_displacement_boundary::cut("registered");
        }
        Ok(())
    }
}
