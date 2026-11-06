//! One small transaction owns each complete policy update.
use super::{Error, Property, Source, Values};
use rusqlite::{Connection, OpenFlags, TransactionBehavior, config::DbConfig};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::Path,
    time::Duration,
};

const APPLICATION_ID: i64 = 0x434e504c; // CNPL
const MAX_BYTES: u64 = 1024 * 1024;
type Identity = (u64, u64);

pub(super) struct Store {
    source: Source,
    identity: Option<Identity>,
    values: Values,
}

impl Store {
    pub(super) fn open(source: Source) -> Result<Self, Error> {
        let mut store = Self {
            source,
            identity: None,
            values: [false; 7],
        };
        store.reload()?;
        Ok(store)
    }
    pub(super) fn values(&self) -> Values {
        self.values
    }

    pub(super) fn reload(&mut self) -> Result<Values, Error> {
        let (identity, values) = match metadata(&self.source)? {
            None if self.source.read_only => return Err(Error::Source),
            None => (None, [false; 7]),
            Some(before) => {
                let mut db = connect(&self.source.path, true)?;
                let transaction = db
                    .transaction_with_behavior(TransactionBehavior::Deferred)
                    .map_err(db_error)?;
                let values = snapshot(&transaction)?;
                transaction.commit().map_err(db_error)?;
                same_source(&self.source, id(&before))?;
                (Some(id(&before)), values)
            }
        };
        self.identity = identity;
        self.values = values;
        Ok(values)
    }

    pub(super) fn set(&mut self, property: Property, enabled: bool) -> Result<Values, Error> {
        if self.source.read_only {
            return Err(Error::ReadOnly);
        }
        let before = metadata(&self.source)?;
        if self.identity.is_some() && self.identity != before.as_ref().map(id) {
            return Err(Error::Changed);
        }
        // Absent state has authoritative defaults. A no-op creates nothing.
        if before.is_none() && !enabled {
            return Ok([false; 7]);
        }
        let mut created = None;
        if before.is_none() {
            create_parents(self.source.path.parent().ok_or(Error::Location)?, 0)?;
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(
                    rustix::fs::OFlags::NOFOLLOW.bits() as i32
                        | rustix::fs::OFlags::NONBLOCK.bits() as i32,
                )
                .open(&self.source.path)
            {
                Ok(file) => {
                    created = Some(Created {
                        source: self.source.clone(),
                        identity: id(&file.metadata().map_err(io_error)?),
                        keep: false,
                    })
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io_error(error)),
            }
        }
        let selected = metadata(&self.source)?.ok_or(Error::Changed)?;
        // Test the opened file's write permission even when its directory permits replacement.
        let writable = OpenOptions::new()
            .write(true)
            .custom_flags(
                rustix::fs::OFlags::NOFOLLOW.bits() as i32
                    | rustix::fs::OFlags::NONBLOCK.bits() as i32,
            )
            .open(&self.source.path)
            .map_err(io_error)?;
        if id(&writable.metadata().map_err(io_error)?) != id(&selected) {
            return Err(Error::Changed);
        }
        let mut db = connect(&self.source.path, false)?;
        let transaction = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let application: i64 = transaction
            .pragma_query_value(None, "application_id", |row| row.get(0))
            .map_err(db_error)?;
        let empty = selected.len() == 0 && self.identity.is_none();
        if application == 0 && empty {
            transaction
                .execute_batch(
                    "PRAGMA application_id=1129205836; PRAGMA user_version=1;
                CREATE TABLE policy (
                    id INTEGER PRIMARY KEY CHECK(id=1),
                    revision INTEGER NOT NULL CHECK(revision>=0),
                    printing INTEGER NOT NULL CHECK(printing IN (0,1)),
                    save_to_disk INTEGER NOT NULL CHECK(save_to_disk IN (0,1)),
                    application_handlers INTEGER NOT NULL CHECK(application_handlers IN (0,1)),
                    location INTEGER NOT NULL CHECK(location IN (0,1)),
                    camera INTEGER NOT NULL CHECK(camera IN (0,1)),
                    microphone INTEGER NOT NULL CHECK(microphone IN (0,1)),
                    sound_output INTEGER NOT NULL CHECK(sound_output IN (0,1))
                ) STRICT;
                INSERT INTO policy VALUES(1,0,0,0,0,0,0,0,0);",
                )
                .map_err(db_error)?;
        }
        let mut after = snapshot(&transaction)?;
        if after[property as usize] != enabled {
            let column = match property {
                Property::Printing => "printing",
                Property::SaveToDisk => "save_to_disk",
                Property::ApplicationHandlers => "application_handlers",
                Property::Location => "location",
                Property::Camera => "camera",
                Property::Microphone => "microphone",
                Property::SoundOutput => "sound_output",
            };
            transaction
                .execute(
                    &format!("UPDATE policy SET {column}=?1,revision=revision+1 WHERE id=1"),
                    [enabled],
                )
                .map_err(db_error)?;
            after = snapshot(&transaction)?;
        }
        same_source(&self.source, id(&selected))?;
        transaction.commit().map_err(db_error)?;
        same_source(&self.source, id(&selected))?;
        if let Some(created) = &mut created {
            created.keep = true;
        }
        self.identity = Some(id(&selected));
        self.values = after;
        Ok(after)
    }
}

fn id(metadata: &Metadata) -> Identity {
    (metadata.dev(), metadata.ino())
}

fn metadata(source: &Source) -> Result<Option<Metadata>, Error> {
    let metadata = match fs::symlink_metadata(&source.path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(error)),
    };
    let owner = metadata.uid();
    if !metadata.is_file()
        || metadata.len() > MAX_BYTES
        || metadata.nlink() != 1
        || metadata.mode() & 0o022 != 0
        || (owner != rustix::process::geteuid().as_raw() && !(source.read_only && owner == 0))
    {
        return Err(Error::Source);
    }
    Ok(Some(metadata))
}

fn same_source(source: &Source, expected: Identity) -> Result<(), Error> {
    if metadata(source)?.as_ref().map(id) != Some(expected) {
        return Err(Error::Changed);
    }
    Ok(())
}

fn connect(path: &Path, read_only: bool) -> Result<Connection, Error> {
    let flags = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let db = Connection::open_with_flags(
        path,
        flags | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(db_error)?;
    db.busy_timeout(Duration::from_millis(250))
        .map_err(db_error)?;
    db.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)
        .map_err(db_error)?;
    db.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)
        .map_err(db_error)?;
    db.execute_batch("PRAGMA trusted_schema=OFF;")
        .map_err(db_error)?;
    if !read_only {
        db.execute_batch("PRAGMA synchronous=FULL;")
            .map_err(db_error)?;
    }
    Ok(db)
}

fn snapshot(db: &Connection) -> Result<Values, Error> {
    let application: i64 = db
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(db_error)?;
    let version: i64 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(db_error)?;
    let journal: String = db
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(db_error)?;
    if application != APPLICATION_ID || version != 1 || journal != "delete" {
        return Err(Error::Source);
    }
    let objects: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    let tables: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name='policy'",
            [],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if objects != 1 || tables != 1 {
        return Err(Error::Source);
    }
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM policy", [], |row| row.get(0))
        .map_err(db_error)?;
    if count != 1 {
        return Err(Error::Source);
    }
    let raw: [i64; 8] = db.query_row("SELECT revision,printing,save_to_disk,application_handlers,location,camera,microphone,sound_output FROM policy WHERE id=1", [], |row| {
        Ok([row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?])
    }).map_err(db_error)?;
    if raw[0] < 0 || raw[1..].iter().any(|value| !matches!(value, 0 | 1)) {
        return Err(Error::Source);
    }
    Ok(std::array::from_fn(|index| raw[index + 1] == 1))
}

fn create_parents(path: &Path, depth: usize) -> Result<(), Error> {
    if depth > 64 {
        return Err(Error::Location);
    }
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => return Ok(()),
        Ok(_) => return Err(Error::Source),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(error)),
    }
    let parent = path.parent().ok_or(Error::Location)?;
    create_parents(parent, depth + 1)?;
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(io_error)?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => {}
        Err(error) => return Err(io_error(error)),
    }
    Ok(())
}

struct Created {
    source: Source,
    identity: Identity,
    keep: bool,
}
impl Drop for Created {
    fn drop(&mut self) {
        if !self.keep
            && let Ok(Some(metadata)) = metadata(&self.source)
            && id(&metadata) == self.identity
            && metadata.len() == 0
        {
            let _ = fs::remove_file(&self.source.path);
        }
    }
}

fn io_error(error: std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::PermissionDenied
        || error.raw_os_error() == Some(rustix::io::Errno::ROFS.raw_os_error())
    {
        Error::ReadOnly
    } else {
        Error::Source
    }
}
fn db_error(error: rusqlite::Error) -> Error {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
            Error::Busy
        }
        Some(rusqlite::ErrorCode::ReadOnly | rusqlite::ErrorCode::PermissionDenied) => {
            Error::ReadOnly
        }
        _ => Error::Source,
    }
}

#[cfg(test)]
mod tests;
