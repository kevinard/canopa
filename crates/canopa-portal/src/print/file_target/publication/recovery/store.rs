use super::*;
use std::os::unix::fs::DirBuilderExt;

impl RecoveryStore {
    pub(super) fn lock(directory: &File, exclusive: bool) -> io::Result<File> {
        let lock = if exclusive {
            File::from(openat(
                directory,
                "catalog.lock",
                OFlags::CREATE
                    | OFlags::RDWR
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )?)
        } else {
            read_only(directory, "catalog.lock")?
        };
        private(&lock, false)?;
        let operation = if exclusive {
            rustix::fs::FlockOperation::NonBlockingLockExclusive
        } else {
            rustix::fs::FlockOperation::NonBlockingLockShared
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
        loop {
            match rustix::fs::flock(&lock, operation) {
                Ok(()) => break,
                Err(rustix::io::Errno::WOULDBLOCK) if std::time::Instant::now() < deadline => {
                    // Only the isolated worker waits, and never indefinitely.
                    // This also covers a fork's brief pre-exec CLOEXEC window.
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(error) => return Err(error.into()),
            }
        }
        let named = read_only(directory, "catalog.lock")?;
        if Identity::of(&lock.metadata()?) != Identity::of(&named.metadata()?) {
            return Err(changed());
        }
        Ok(lock)
    }
    pub(super) fn open(&self, create: bool) -> io::Result<Option<File>> {
        let path = PathBuf::from(OsString::from_vec(self.path.clone()));
        Self::new(&path)?;
        if create {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&path)?;
        }
        let directory = match rustix::fs::open(
            &path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(file) => File::from(file),
            Err(rustix::io::Errno::NOENT) if !create => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        private(&directory, true)?;
        Ok(Some(directory))
    }

    pub(super) fn load(&self, directory: &File) -> io::Result<Catalog> {
        let catalog = match read_only(directory, "catalog.json") {
            Ok(file) => {
                private(&file, false)?;
                read_json::<Catalog>(&file, CATALOG_LIMIT)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Catalog {
                version: 1,
                entries: vec![],
            },
            Err(error) => return Err(error),
        };
        if catalog.version != 1 || catalog.entries.len() > MAX_ENTRIES {
            return Err(invalid());
        }
        let mut keys = std::collections::HashSet::new();
        for entry in &catalog.entries {
            if !keys.insert(id(&entry.preparation)?)
                || entry.publication.as_ref().is_some_and(|info| {
                    info.selection != entry.preparation.selection
                        || info.directory_name != entry.preparation.directory_name
                })
            {
                return Err(invalid());
            }
        }
        Ok(catalog)
    }

    pub(super) fn change(
        &self,
        create: bool,
        update: impl FnOnce(&mut Catalog) -> io::Result<()>,
    ) -> io::Result<()> {
        let directory = self
            .open(create)?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let _lock = Self::lock(&directory, true)?;
        let mut catalog = self.load(&directory)?;
        update(&mut catalog)?;
        let bytes = serde_json::to_vec(&catalog)?;
        if bytes.len() as u64 > CATALOG_LIMIT {
            return Err(invalid());
        }
        // A fixed private scratch name bounds crash debris. An unrelated type,
        // symlink, hardlink or permission change is preserved and blocks writes.
        if let Some(stale) = any(&directory, OsStr::new("catalog.next"))? {
            private(&stale, false)?;
            rustix::fs::unlinkat(&directory, "catalog.next", AtFlags::empty())?;
        }
        let mut next = fresh(&directory, "catalog.next")?;
        next.write_all(&bytes)?;
        next.sync_all()?;
        rustix::fs::renameat(&directory, "catalog.next", &directory, "catalog.json")?;
        directory.sync_all()?;
        // Persist newly created ancestors too. A failed fsync prevents this
        // worker from proceeding to destination changes.
        let path = PathBuf::from(OsString::from_vec(self.path.clone()));
        let mut parent = path.parent();
        while let Some(path) = parent {
            File::open(path)?.sync_all()?;
            parent = path.parent();
        }
        Ok(())
    }
}
