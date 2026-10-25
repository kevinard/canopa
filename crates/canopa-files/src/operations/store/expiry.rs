//! Expired replacement cleanup is a recorded mutation, including the interval
//! after unlinking its payload and before retiring its empty private container.
use super::*;

impl Store {
    pub(in crate::operations) fn begin_expiry(
        &mut self,
        entry: u64,
        record: &recovery::Record,
    ) -> io::Result<()> {
        self.with_directory_access(|state| {
            let parent_path = record.retained.stage.directory.parent().unwrap();
            let parent = Identity::from_metadata(&fs::metadata(parent_path)?);
            if record
                .retained
                .stage
                .parent_identity
                .is_some_and(|expected| expected != parent)
            {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the expired replacement's original parent is unavailable",
                ));
            }
            if record.retained.stage.created_parent() == Some(parent) {
                state.permit_directory(parent_path, parent)?;
            }
            // Refuse before unlinking a saved file when the unowned parent does
            // not permit removing its container. The journal covers later races.
            rustix::fs::accessat(
                rustix::fs::CWD,
                parent_path,
                rustix::fs::Access::WRITE_OK | rustix::fs::Access::EXEC_OK,
                rustix::fs::AtFlags::EACCESS,
            )?;
            state.intent(&Pending::Expiry {
                entry,
                record: Box::new(record.clone()),
                parent,
            })?;
            state.recover_expiry(entry, record, parent)
        })
    }

    pub(in crate::operations) fn recover_expiry(
        &mut self,
        entry: u64,
        record: &recovery::Record,
        parent: Identity,
    ) -> io::Result<()> {
        self.ensure_location()?;
        let current = self
            .recovery_record(&recovery::Key::Archived(entry))?
            .ok_or_else(|| invalid("the recorded expired replacement is unavailable"))?;
        if encode(&current)? != encode(record)? || record.retained.recovery {
            return Err(invalid("the recorded expired replacement changed"));
        }
        let stage = &record.retained.stage;
        validate_stage(Some(stage), &stage.directory)?;
        let parent_path = stage.directory.parent().unwrap();
        if Identity::from_metadata(&fs::metadata(parent_path)?) != parent {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "return the expired replacement's original parent before cleanup",
            ));
        }
        match fs::symlink_metadata(&stage.directory) {
            Ok(metadata) => {
                if !metadata.is_dir() || Identity::from_metadata(&metadata) != stage.identity {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "the expired replacement container was replaced",
                    ));
                }
                saved_stage::require_only(stage, &["previous"])?;
                let present = match fs::symlink_metadata(record.retained.path()) {
                    Ok(_) => require_retained_expiry(&record.retained)?,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                    Err(error) => return Err(error),
                };
                if self.expiry_is_shared(entry, record, present)? {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "the expired replacement gained another saved-data owner",
                    ));
                }
                if record.retained.stage.created_parent() == Some(parent) {
                    self.permit_directory(parent_path, parent)?;
                }
                rustix::fs::accessat(
                    rustix::fs::CWD,
                    parent_path,
                    rustix::fs::Access::WRITE_OK | rustix::fs::Access::EXEC_OK,
                    rustix::fs::AtFlags::EACCESS,
                )?;
                if present {
                    fs::remove_file(record.retained.path())?;
                    fs::File::open(&stage.directory)?.sync_all()?;
                }
                fs::remove_dir(&stage.directory)?;
                sync_parent(&stage.directory)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if self.expiry_is_shared(entry, record, false)? {
                    return Err(invalid("the expired replacement still has another owner"));
                }
            }
            Err(error) => return Err(error),
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        transaction
            .execute("DELETE FROM garbage WHERE id=?1", [entry])
            .map_err(db_error)?;
        clear_intent(&transaction, self.lane)?;
        transaction.commit().map_err(db_error)
    }
}
