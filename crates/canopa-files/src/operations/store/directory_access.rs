//! Temporary owner access to folders created by a transfer or restored by Undo.
//! Persist before chmod; restore through the opened object while alive, and a
//! guarded recorded location after restart. Separate lanes never borrow access
//! from an unrelated interrupted operation.
use super::*;
use std::os::unix::fs::PermissionsExt;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Access {
    #[serde(with = "crate::path_serde")]
    pub(super) path: PathBuf,
    pub(super) identity: Identity,
    pub(super) parent: Identity,
    pub(super) mode: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) original: Option<original::Original>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) parent_original: Option<original::Original>,
}

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS directory_access (
        id INTEGER PRIMARY KEY, lane INTEGER NOT NULL CHECK(lane IN (0,1)),
        path BLOB NOT NULL, body TEXT NOT NULL, UNIQUE(lane,path));",
        )
        .map_err(db_error)
}

pub(super) fn open(path: &Path, identity: Identity) -> io::Result<fs::File> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::NOFOLLOW).bits() as i32)
        .open(path)?;
    if Identity::from_metadata(&file.metadata()?) != identity {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "the folder needing temporary access was replaced",
        ));
    }
    Ok(file)
}

impl Store {
    fn directory_access_records(&self, lane: IntentLane) -> io::Result<Vec<(i64, Access)>> {
        let mut statement = self
            .connection
            .prepare("SELECT id,body FROM directory_access WHERE lane=?1 ORDER BY id DESC")
            .map_err(db_error)?;
        statement
            .query_map([lane.index()], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(db_error)?
            .map(|row| {
                let (id, body) = row.map_err(db_error)?;
                Ok((id, decode(&body)?))
            })
            .collect()
    }

    pub(in crate::operations) fn directory_access_paths(
        &self,
        lane: IntentLane,
    ) -> io::Result<Vec<PathBuf>> {
        Ok(self
            .directory_access_records(lane)?
            .into_iter()
            .map(|(_, access)| access.path)
            .collect())
    }

    pub(in crate::operations) fn directory_access_context(
        &self,
        lane: IntentLane,
    ) -> io::Result<String> {
        encode(&self.directory_access_records(lane)?)
    }

    pub(in crate::operations) fn permit_directory(
        &mut self,
        path: &Path,
        identity: Identity,
    ) -> io::Result<()> {
        self.permit_directory_inner(path, identity, None)
    }
    pub(in crate::operations) fn permit_transfer_directory(
        &mut self,
        path: &Path,
        identity: Identity,
        originals: (original::Original, original::Original),
    ) -> io::Result<()> {
        self.permit_directory_inner(path, identity, Some(originals))
    }
    fn permit_directory_inner(
        &mut self,
        path: &Path,
        identity: Identity,
        originals: Option<(original::Original, original::Original)>,
    ) -> io::Result<()> {
        self.ensure_location()?;
        validate_target(path)?;
        let shared = self.directory_reads.clone();
        let mut reads = shared
            .lock()
            .map_err(|_| io::Error::other("folder access failed"))?;
        let file = open(path, identity)?;
        let parent_file = fs::File::open(path.parent().unwrap())?;
        if let Some((parent, entry)) = originals {
            parent.require(&parent_file)?;
            entry.require(&file)?;
        }
        let current_mode = file.metadata()?.mode() & 0o7777;
        let mode = if let Some(access) = reads.reader_access(path) {
            access
                .original
                .ok_or_else(|| invalid("Missing original read-access folder."))?
                .require(&file)?;
            if current_mode == access.mode | 0o500 {
                access.mode
            } else {
                current_mode
            }
        } else {
            current_mode
        };
        if mode & 0o700 == 0o700 {
            return Ok(());
        }
        let access = Access {
            path: path.to_owned(),
            identity,
            parent: Identity::from_metadata(&parent_file.metadata()?),
            mode,
            original: Some(original::Original::capture(&file)?),
            parent_original: Some(original::Original::capture(&parent_file)?),
        };
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        transaction
            .execute(
                "INSERT INTO directory_access(lane,path,body) VALUES(?1,?2,?3)",
                params![self.lane.index(), paths::key(path), encode(&access)?],
            )
            .map_err(db_error)?;
        let id = transaction.last_insert_rowid();
        directory_reads::remember(&transaction, &access)?;
        transaction.commit().map_err(db_error)?;
        self.opened_directories.insert(id, file);
        let file = &self.opened_directories[&id];
        reads.writers.insert(path.to_owned());
        file.set_permissions(fs::Permissions::from_mode(mode | 0o700))?;
        file.sync_all()
    }

    pub(in crate::operations) fn restore_directory_access(&mut self) -> io::Result<()> {
        let shared = self.directory_reads.clone();
        let mut reads = shared
            .lock()
            .map_err(|_| io::Error::other("folder access failed"))?;
        // Descendants are normally borrowed later. Restore them before their
        // ancestors so an originally non-searchable ancestor cannot hide them.
        for (id, access) in self.directory_access_records(self.lane)? {
            validate_target(&access.path)?;
            if access.mode & !0o7777 != 0 {
                return Err(invalid("invalid recorded folder permissions"));
            }
            let file = if let Some(file) = self.opened_directories.get(&id) {
                file.try_clone()?
            } else {
                if !access.parent.matches(access.path.parent().unwrap()) {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "return the original folder before restoring its permissions",
                    ));
                }
                if let Some(original) = &access.parent_original {
                    original.require(&fs::File::open(access.path.parent().unwrap())?)?;
                }
                open(&access.path, access.identity)?
            };
            if let Some(original) = &access.original {
                original.require(&file)?;
            }
            let metadata = file.metadata()?;
            let target = if reads.has_reader(&access) {
                access.mode | 0o500
            } else {
                access.mode
            };
            let mode = metadata.mode() & 0o7777;
            if metadata.nlink() != 0
                && (mode == access.mode | 0o700
                    || (mode == access.mode | 0o500
                        && directory_reads::recorded(&self.connection, &access)?))
            {
                file.set_permissions(fs::Permissions::from_mode(target))?;
                file.sync_all()?;
            }
            reads.writers.remove(&access.path);
            // A different observed mode is an external edit; keep that edit.
            if self.pending()?.is_none() {
                self.connection
                    .execute("DELETE FROM directory_access WHERE id=?1", [id])
                    .map_err(db_error)?;
                self.opened_directories.remove(&id);
            }
        }
        Ok(())
    }

    pub(in crate::operations) fn prepare_directory_access(&mut self) -> io::Result<()> {
        if self.pending()?.is_none() {
            return Ok(());
        }
        let shared = self.directory_reads.clone();
        let mut reads = shared
            .lock()
            .map_err(|_| io::Error::other("folder access failed"))?;
        let mut records = self.directory_access_records(self.lane)?;
        records.reverse();
        for (id, access) in records {
            validate_target(&access.path)?;
            if access.mode & !0o7777 != 0 {
                return Err(invalid("invalid recorded folder permissions"));
            }
            if !access.parent.matches(access.path.parent().unwrap()) {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "return the original folder before reconciling its temporary access",
                ));
            }
            if let Some(original) = &access.parent_original {
                original.require(&fs::File::open(access.path.parent().unwrap())?)?;
            }
            let file = open(&access.path, access.identity)?;
            if let Some(original) = &access.original {
                original.require(&file)?;
            }
            let mode = file.metadata()?.mode() & 0o7777;
            if mode == access.mode
                || (mode == access.mode | 0o500
                    && directory_reads::recorded(&self.connection, &access)?)
            {
                file.set_permissions(fs::Permissions::from_mode(access.mode | 0o700))?;
                file.sync_all()?;
            } else if mode & 0o700 != 0o700 {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "folder permissions changed before interrupted work could be reconciled",
                ));
            }
            reads.writers.insert(access.path.clone());
            self.opened_directories.insert(id, file);
        }
        Ok(())
    }

    pub(in crate::operations) fn with_directory_access<T>(
        &mut self,
        run: impl FnOnce(&mut Self) -> io::Result<T>,
    ) -> io::Result<T> {
        let result = run(self);
        self.restore_directory_access()?;
        result
    }

    pub(in crate::operations) fn permit_undo_parent(
        &mut self,
        id: u64,
        ordinal: u64,
        change: &Change,
    ) -> io::Result<()> {
        self.permit_owned_return_parents(id, ordinal)?;
        if let Some(moved) = undo::directory_return::moved(change)
            && let Some(owner) = moved.created
        {
            let cleanup = stored_change(&self.connection, id, owner)?;
            if !matches!(&cleanup, Change::CreatedFolder { path, .. } if paths::key(path) == paths::key(&moved.current))
            {
                return Err(invalid("the returned folder has no matching cleanup owner"));
            }
            self.permit_undo_parent(id, owner, &cleanup)?;
        }
        let (path, expected, owned) = match change {
            Change::CreatedFolder {
                path,
                access_parent,
                ..
            }
            | Change::Copied {
                path,
                access_parent,
                ..
            } => (path, *access_parent, true),
            Change::SourceRemoved {
                original,
                quarantine,
                ..
            } => (original, quarantine.parent_identity, false),
            Change::RemovedDirectory {
                path,
                parent_identity,
                ..
            } => (path, Some(*parent_identity), false),
            Change::DirectoryMerge { change, .. } => {
                return self.permit_undo_parent(id, ordinal, change);
            }
            _ => return Ok(()),
        };
        if let Some(expected) = expected {
            let parent = path.parent().unwrap();
            let actual = self.resolve_for(id, ordinal, parent, expected)?;
            if owned || expected != actual {
                self.permit_directory(parent, actual)?;
            }
        }
        Ok(())
    }
}
