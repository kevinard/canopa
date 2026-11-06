//! Per-application navigation hints. Saved folders never grant filesystem access.
use anyhow::{Result, ensure};
use rusqlite::{Connection, OpenFlags, OptionalExtension, config::DbConfig, params};
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;

const APPLICATION_ID: i64 = 0x434e5046; // CNPF
const MAX_FILE: u64 = 2 * 1024 * 1024;
const MAX_APPS: usize = 128;
const DEADLINE: Duration = Duration::from_millis(200);

enum Source {
    Disabled,
    Memory(Mutex<VecDeque<(String, PathBuf)>>),
    File(PathBuf),
}
struct Inner {
    source: Source,
    operations: Arc<Semaphore>,
}

#[derive(Clone)]
pub struct History(Arc<Inner>);

fn app_id(app: &str) -> bool {
    !app.is_empty()
        && app.len() <= 255
        && app
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn folder(path: &Path) -> bool {
    path.is_absolute()
        && path.as_os_str().len() <= 4096
        && !path.as_os_str().as_bytes().contains(&0)
}

impl History {
    fn source(source: Source) -> Self {
        Self(Arc::new(Inner {
            source,
            operations: Arc::new(Semaphore::new(1)),
        }))
    }
    pub fn disabled() -> Self {
        Self::source(Source::Disabled)
    }
    pub fn memory() -> Self {
        Self::source(Source::Memory(Mutex::default()))
    }
    /// Lazy: reading an absent preference file creates nothing.
    pub fn file(path: PathBuf) -> Result<Self> {
        ensure!(path.is_absolute(), "Chooser history path must be absolute");
        Ok(Self::source(Source::File(path)))
    }
    pub fn discover() -> Result<Self> {
        let base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|p| p.join(".local/state"))
            })
            .ok_or_else(|| anyhow::anyhow!("Chooser history location unavailable"))?;
        Self::file(base.join("canopa/portal-file-chooser.db"))
    }
    /// Preference failures, contention and stale folders use the ordinary default.
    pub async fn load(&self, app: String) -> Option<PathBuf> {
        if !app_id(&app) || matches!(self.0.source, Source::Disabled) {
            return None;
        }
        let permit = self.0.operations.clone().try_acquire_owned().ok()?;
        let this = self.clone();
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            this.read(&app).ok().flatten().filter(|path| path.is_dir())
        });
        tokio::time::timeout(DEADLINE, task).await.ok()?.ok()?
    }
    /// Only call after a valid, accepted request. Optional bookkeeping cannot
    /// turn an accepted choice into a failure or queue behind another operation.
    pub async fn save(&self, app: String, path: PathBuf) {
        if !app_id(&app) || !folder(&path) || matches!(self.0.source, Source::Disabled) {
            return;
        }
        let Ok(permit) = self.0.operations.clone().try_acquire_owned() else {
            return;
        };
        let this = self.clone();
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            if path.is_dir() {
                let _ = this.write(&app, path);
            }
        });
        let _ = tokio::time::timeout(DEADLINE, task).await;
    }
    fn read(&self, app: &str) -> Result<Option<PathBuf>> {
        match &self.0.source {
            Source::Disabled => Ok(None),
            Source::Memory(values) => Ok(values
                .lock()
                .expect("chooser history")
                .iter()
                .find(|(id, _)| id == app)
                .map(|(_, p)| p.clone())),
            Source::File(path) => {
                let Some((identity, length)) = metadata(path)? else {
                    return Ok(None);
                };
                if length == 0 {
                    return Ok(None);
                }
                let db = connect(path, true)?;
                schema(&db)?;
                let bytes: Option<Vec<u8>> = db
                    .query_row("SELECT path FROM folders WHERE app=?1", [app], |row| {
                        row.get(0)
                    })
                    .optional()?;
                ensure!(
                    metadata(path)?.map(|v| v.0) == Some(identity),
                    "Chooser history changed"
                );
                let selected = bytes.map(|b| PathBuf::from(std::ffi::OsString::from_vec(b)));
                ensure!(
                    selected.as_deref().is_none_or(folder),
                    "Invalid saved chooser folder"
                );
                Ok(selected)
            }
        }
    }
    fn write(&self, app: &str, selected: PathBuf) -> Result<()> {
        match &self.0.source {
            Source::Disabled => Ok(()),
            Source::Memory(values) => {
                let mut values = values.lock().expect("chooser history");
                values.retain(|(id, _)| id != app);
                values.push_back((app.into(), selected));
                while values.len() > MAX_APPS {
                    values.pop_front();
                }
                Ok(())
            }
            Source::File(path) => {
                let parent = path
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("Invalid history directory"))?;
                fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(parent)?;
                let directory = fs::symlink_metadata(parent)?;
                ensure!(
                    directory.is_dir()
                        && directory.uid() == rustix::process::geteuid().as_raw()
                        && directory.mode() & 0o022 == 0,
                    "Unsafe chooser history directory"
                );
                if metadata(path)?.is_none() {
                    match OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
                        .open(path)
                    {
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                let (identity, length) = metadata(path)?
                    .ok_or_else(|| anyhow::anyhow!("Chooser history unavailable"))?;
                if length != 0 {
                    schema(&connect(path, true)?)?;
                }
                let mut db = connect(path, false)?;
                let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                if length == 0 {
                    tx.execute_batch("PRAGMA application_id=1129205830; PRAGMA user_version=1;
                        CREATE TABLE folders(app TEXT PRIMARY KEY CHECK(length(app) BETWEEN 1 AND 255), path BLOB NOT NULL CHECK(length(path) BETWEEN 1 AND 4096), used INTEGER NOT NULL CHECK(used>0)) STRICT;")?;
                }
                schema(&tx)?;
                let used: i64 =
                    tx.query_row("SELECT coalesce(max(used),0) FROM folders", [], |r| {
                        r.get(0)
                    })?;
                let used = used
                    .checked_add(1)
                    .ok_or_else(|| anyhow::anyhow!("Chooser history counter exhausted"))?;
                tx.execute("INSERT INTO folders VALUES(?1,?2,?3) ON CONFLICT(app) DO UPDATE SET path=excluded.path,used=excluded.used",
                    params![app, selected.as_os_str().as_bytes(), used])?;
                tx.execute("DELETE FROM folders WHERE app NOT IN (SELECT app FROM folders ORDER BY used DESC LIMIT 128)", [])?;
                ensure!(
                    metadata(path)?.map(|v| v.0) == Some(identity),
                    "Chooser history changed"
                );
                tx.commit()?;
                ensure!(
                    metadata(path)?.map(|v| v.0) == Some(identity),
                    "Chooser history changed"
                );
                Ok(())
            }
        }
    }
}

fn metadata(path: &Path) -> Result<Option<((u64, u64), u64)>> {
    let value = match fs::symlink_metadata(path) {
        Ok(v) => v,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        value.is_file()
            && value.len() <= MAX_FILE
            && value.nlink() == 1
            && value.uid() == rustix::process::geteuid().as_raw()
            && value.mode() & 0o077 == 0,
        "Unsafe chooser history file"
    );
    Ok(Some(((value.dev(), value.ino()), value.len())))
}
fn connect(path: &Path, readonly: bool) -> Result<Connection> {
    let flags = if readonly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let db = Connection::open_with_flags(
        path,
        flags | OpenFlags::SQLITE_OPEN_NOFOLLOW | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(Duration::from_millis(50))?;
    db.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
    db.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)?;
    db.execute_batch("PRAGMA trusted_schema=OFF;")?;
    if !readonly {
        let page: u64 = db.pragma_query_value(None, "page_size", |row| row.get(0))?;
        ensure!(page > 0 && page <= MAX_FILE, "Invalid history page size");
        db.pragma_update(None, "max_page_count", MAX_FILE / page)?;
        db.execute_batch("PRAGMA synchronous=FULL;")?;
    }
    Ok(db)
}
fn schema(db: &Connection) -> Result<()> {
    let app: i64 = db.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: i64 = db.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let journal: String = db.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    ensure!(
        app == APPLICATION_ID && version == 1 && journal == "delete",
        "Unknown chooser history schema"
    );
    let objects: i64 = db.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    let tables: i64 = db.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name='folders' AND type='table'",
        [],
        |row| row.get(0),
    )?;
    let apps: i64 = db.query_row("SELECT count(*) FROM folders", [], |row| row.get(0))?;
    ensure!(
        objects == 1 && tables == 1 && apps <= MAX_APPS as i64,
        "Invalid chooser history tables"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
