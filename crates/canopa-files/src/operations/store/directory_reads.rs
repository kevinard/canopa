//! Read/search access belongs to a live Recovery selection, never its serialized
//! description. Durable grants restore abandoned permissions after process death.
use super::directory_access::{Access, open};
use super::*;
use std::{collections::BTreeMap, os::unix::fs::PermissionsExt, sync::Weak};

pub(super) type Shared = Arc<Mutex<Readers>>;
#[derive(Default)]
pub(super) struct Readers {
    active: BTreeMap<PathBuf, (Access, Weak<Lease>)>,
    pub(super) writers: BTreeSet<PathBuf>,
    release: Option<std::sync::mpsc::Sender<Release>>,
}
impl Readers {
    pub(super) fn has_reader(&self, access: &Access) -> bool {
        self.active
            .get(&access.path)
            .is_some_and(|(reader, lease)| {
                lease.strong_count() != 0
                    && reader.mode == access.mode
                    && reader.original == access.original
                    && reader.parent_original == access.parent_original
            })
    }
    pub(super) fn reader_access(&self, path: &Path) -> Option<&Access> {
        self.active.get(path).map(|(access, _)| access)
    }
    fn releaser(&mut self) -> io::Result<std::sync::mpsc::Sender<Release>> {
        if let Some(sender) = &self.release {
            return Ok(sender.clone());
        }
        let (sender, receiver) = std::sync::mpsc::channel::<Release>();
        std::thread::Builder::new()
            .name("files-recovery-access".into())
            .spawn(move || {
                while let Ok(release) = receiver.recv() {
                    release.run();
                }
            })?;
        self.release = Some(sender.clone());
        Ok(sender)
    }
}

pub(in crate::operations) struct Lease {
    access: Access,
    file: Option<fs::File>,
    reads: ReadSource,
    _lock: Arc<history_lock::HistoryLock>,
    release: std::sync::mpsc::Sender<Release>,
}
impl std::fmt::Debug for Lease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RecoveryFolderAccess")
            .field(&self.access.path)
            .finish()
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        // Recovery selections can be released by the renderer. Only enqueue;
        // permission restoration and durable release run off the UI thread.
        let _ = self.release.send(Release {
            access: self.access.clone(),
            file: self.file.take().unwrap(),
            reads: self.reads.clone(),
            _lock: self._lock.clone(),
            token: self as *const Self as usize,
        });
    }
}
struct Release {
    access: Access,
    file: fs::File,
    reads: ReadSource,
    _lock: Arc<history_lock::HistoryLock>,
    token: usize,
}
impl Release {
    fn run(self) {
        let mut readers = self
            .reads
            .directory_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if readers
            .active
            .get(&self.access.path)
            .is_some_and(|(_, lease)| lease.as_ptr() as usize == self.token)
        {
            readers.active.remove(&self.access.path);
        } else {
            return;
        }
        if readers.writers.contains(&self.access.path) {
            return;
        }
        // The opened object, rather than a possibly replaced path, owns chmod.
        // A failed save/restore keeps the durable grant for restart retry.
        if restore(&self.file, &self.access).is_ok() {
            let _ = self.reads.request_writer(|transaction| {
                transaction
                    .execute(
                        "DELETE FROM directory_read_grants WHERE path=?1 AND body=?2",
                        params![paths::key(&self.access.path), encode(&self.access)?],
                    )
                    .map_err(db_error)?;
                Ok(())
            });
            #[cfg(test)]
            transfer::stage_boundary::nested_read_released(&self.access.path);
        }
    }
}

pub(super) fn create(connection: &Connection) -> io::Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS directory_read_context (
        path BLOB PRIMARY KEY, body TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS directory_read_grants (
        path BLOB PRIMARY KEY, body TEXT NOT NULL);",
        )
        .map_err(db_error)
}
pub(super) fn remember(connection: &Connection, access: &Access) -> io::Result<()> {
    save(connection, "directory_read_context", access)
}
pub(super) fn recorded(connection: &Connection, access: &Access) -> io::Result<bool> {
    let body: Option<String> = connection
        .query_row(
            "SELECT body FROM directory_read_grants WHERE path=?1",
            [paths::key(&access.path)],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    Ok(body.as_deref() == Some(encode(access)?.as_str()))
}
fn save(connection: &Connection, table: &str, access: &Access) -> io::Result<()> {
    let body = encode(access)?;
    if connection
        .execute(
            &format!(
                "INSERT INTO {table}(path,body) VALUES(?1,?2)
        ON CONFLICT(path) DO UPDATE SET body=excluded.body"
            ),
            params![paths::key(&access.path), &body],
        )
        .map_err(db_error)?
        != 1
    {
        return Err(invalid("Temporary folder access was not recorded."));
    }
    let recorded: String = connection
        .query_row(
            &format!("SELECT body FROM {table} WHERE path=?1"),
            [paths::key(&access.path)],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if recorded != body {
        return Err(invalid("Temporary folder access changed while saving."));
    }
    Ok(())
}
fn require(access: &Access) -> io::Result<fs::File> {
    validate_target(&access.path)?;
    if access.mode & !0o7777 != 0 {
        return Err(invalid("invalid recorded folder permissions"));
    }
    let original = access
        .original
        .ok_or_else(|| invalid("Recovery access needs its original folder proof."))?;
    let parent = access
        .parent_original
        .ok_or_else(|| invalid("Recovery access needs its original parent proof."))?;
    let parent_file = open(access.path.parent().unwrap(), access.parent)?;
    parent.require(&parent_file)?;
    let file = open(&access.path, access.identity)?;
    original.require(&file)?;
    Ok(file)
}
fn restore(file: &fs::File, access: &Access) -> io::Result<()> {
    access
        .original
        .ok_or_else(|| invalid("Missing original folder proof."))?
        .require(file)?;
    let metadata = file.metadata()?;
    if metadata.nlink() != 0 && metadata.mode() & 0o7777 == access.mode | 0o500 {
        file.set_permissions(fs::Permissions::from_mode(access.mode))?;
        file.sync_all()?;
    }
    Ok(())
}
fn records(connection: &Connection, table: &str) -> io::Result<Vec<Access>> {
    let mut statement = connection
        .prepare(&format!("SELECT path,body FROM {table}"))
        .map_err(db_error)?;
    statement
        .query_map([], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(db_error)?
        .map(|row| {
            let (path, body) = row.map_err(db_error)?;
            let access: Access = decode(&body)?;
            if paths::key(&access.path) != path {
                return Err(invalid(
                    "Temporary folder access changed its recorded location.",
                ));
            }
            Ok(access)
        })
        .collect()
}
fn contexts(connection: &Connection, path: &Path) -> io::Result<Vec<Access>> {
    let mut contexts = Vec::new();
    for path in path.ancestors().skip(1) {
        let body: Option<String> = connection
            .query_row(
                "SELECT body FROM directory_read_context WHERE path=?1",
                [paths::key(path)],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if let Some(body) = body {
            let access: Access = decode(&body)?;
            if access.path != path {
                return Err(invalid("Recovery folder context changed its location."));
            }
            if access.mode & 0o500 != 0o500 {
                contexts.push(access);
            }
        }
    }
    contexts.reverse();
    Ok(contexts)
}
fn retire(connection: &Connection, access: &Access) -> io::Result<()> {
    connection
        .execute(
            "DELETE FROM directory_read_grants WHERE path=?1 AND body=?2",
            params![paths::key(&access.path), encode(access)?],
        )
        .map_err(db_error)?;
    Ok(())
}
impl Store {
    fn restore_read_through_ancestors(&self, access: &Access) -> io::Result<()> {
        let mut opened = Vec::<(Access, fs::File)>::new();
        let outcome = (|| {
            for parent in contexts(&self.connection, &access.path)? {
                let file = require(&parent)?;
                let mode = file.metadata()?.mode() & 0o7777;
                if mode & 0o500 == 0o500 {
                    if recorded(&self.connection, &parent)? {
                        opened.push((parent, file));
                    }
                    continue;
                }
                if mode != parent.mode {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "Original ancestor permissions changed before temporary access could be restored.",
                    ));
                }
                // Every borrowed ancestor has its own durable restoration record
                // before chmod. Another death can safely repeat this traversal.
                save(&self.connection, "directory_read_grants", &parent)?;
                opened.push((parent, file));
                let (parent, file) = opened.last().unwrap();
                file.set_permissions(fs::Permissions::from_mode(parent.mode | 0o500))?;
                file.sync_all()?;
                #[cfg(test)]
                transfer::stage_boundary::nested_read_opened(&parent.path);
            }
            let file = require(access)?;
            restore(&file, access)?;
            retire(&self.connection, access)
        })();
        let mut cleanup = Ok(());
        for (parent, file) in opened.into_iter().rev() {
            let restored = restore(&file, &parent).and_then(|()| retire(&self.connection, &parent));
            if cleanup.is_ok() {
                cleanup = restored;
            }
        }
        outcome.and(cleanup)
    }
    pub(super) fn restore_abandoned_directory_reads(&mut self) -> io::Result<()> {
        let mut grants = records(&self.connection, "directory_read_grants")?;
        grants.sort_by_key(|access| std::cmp::Reverse(access.path.components().count()));
        for access in grants {
            // Unavailable/replaced originals and failed saves retain their grant
            // for later review; no fresh path observation replaces their proof.
            let _ = self.restore_read_through_ancestors(&access);
        }
        Ok(())
    }
    pub(in crate::operations) fn recovery_directory_reads(
        &self,
        record: &recovery::Record,
        lock: &Arc<history_lock::HistoryLock>,
    ) -> io::Result<Vec<Arc<Lease>>> {
        // Context is captured only when the writer borrows its owned created
        // folder. A catalog row must actually lie below that recorded object.
        let contexts = contexts(&self.connection, &record.retained.stage.directory)?;
        let reads = self.read_source();
        let mut acquired = Vec::new();
        for access in contexts {
            let mut shared = self
                .directory_reads
                .lock()
                .map_err(|_| io::Error::other("folder access failed"))?;
            let release = shared.releaser()?;
            if let Some(lease) = shared
                .active
                .get(&access.path)
                .and_then(|(_, lease)| Weak::upgrade(lease))
            {
                // Do not let a retained path borrow access from a replacement.
                let file = require(&lease.access)?;
                if file.metadata()?.mode() & 0o500 == 0o500 {
                    acquired.push(lease);
                    continue;
                }
                // A newer writer can record a different external original
                // mode. Refresh must borrow that context instead of extending
                // an older selection whose search access was revoked.
            }
            let file = require(&access)?;
            let mode = file.metadata()?.mode() & 0o7777;
            let write: Option<String> = self
                .connection
                .query_row(
                    "SELECT body FROM directory_access WHERE path=?1 AND body=?2 LIMIT 1",
                    params![paths::key(&access.path), encode(&access)?],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db_error)?;
            let owned = mode == access.mode
                || (mode == access.mode | 0o500 && recorded(&self.connection, &access)?)
                || (mode == access.mode | 0o700 && write.is_some());
            if !owned && mode & 0o500 == 0o500 {
                // An external mode already permits inspection. Do not claim
                // its permissions and later mistake them for our own grant.
                continue;
            }
            if !owned {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Recovery folder permissions changed; its current mode is preserved.",
                ));
            }
            reads.request_writer(|transaction| {
                save(transaction, "directory_read_grants", &access)
            })?;
            if mode & 0o500 != 0o500 {
                file.set_permissions(fs::Permissions::from_mode(mode | 0o500))?;
                file.sync_all()?;
            }
            let lease = Arc::new(Lease {
                access,
                file: Some(file),
                reads: reads.clone(),
                _lock: lock.clone(),
                release,
            });
            shared.active.insert(
                lease.access.path.clone(),
                (lease.access.clone(), Arc::downgrade(&lease)),
            );
            acquired.push(lease);
        }
        Ok(acquired)
    }
}
