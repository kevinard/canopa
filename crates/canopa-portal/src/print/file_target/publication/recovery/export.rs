use super::*;
use crate::print::worker::{Cancellation, Document, document};

pub(super) fn active(entry: &Entry) -> io::Result<bool> {
    entry.worker.alive().and_then(|worker| {
        if worker || entry.retained {
            Ok(worker)
        } else {
            entry.owner.alive()
        }
    })
}

pub(super) fn location(entry: &Entry) -> io::Result<Vec<u8>> {
    let bytes = entry
        .location
        .clone()
        .or_else(|| entry.preparation.retained(false).recovery)
        .ok_or_else(invalid)?;
    let path = PathBuf::from(OsString::from_vec(bytes.clone()));
    if !path.is_absolute()
        || bytes.len() > 8192
        || bytes.contains(&0)
        || path.file_name() != Some(work_name(&entry.preparation.directory_name)?.as_os_str())
    {
        return Err(invalid());
    }
    Ok(bytes)
}

impl RecoveryStore {
    fn retired(&self, key: &str) -> io::Result<Entry> {
        let directory = self
            .open(false)?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        let _lock = Self::lock(&directory, false)?;
        let entry = self
            .load(&directory)?
            .entries
            .into_iter()
            .find(|entry| id(&entry.preparation).is_ok_and(|id| id == key))
            .ok_or_else(invalid)?;
        if active(&entry)? {
            return Err(io::Error::other(
                "This print work may still be active. Try again after its owner exits.",
            ));
        }
        Ok(entry)
    }

    /// Read only a selected retired body. A new sealed memfd owns the result;
    /// publication still needs a separately reviewed destination capability.
    pub(crate) fn snapshot(
        &self,
        key: &str,
        body: RecoveryBody,
        target: File,
        cancel: &Cancellation,
    ) -> anyhow::Result<(Document, RecoveryProtection)> {
        cancel.check()?;
        let entry = self.retired(key)?;
        let path = location(&entry)?;
        let (_, complete, displaced) = inspect(&entry, &path)?;
        anyhow::ensure!(
            match body {
                RecoveryBody::Document => complete,
                RecoveryBody::Displaced => displaced,
            },
            "This retained copy is incomplete, changed, or exceeds the recovery limit"
        );
        let info = entry.publication.as_ref().ok_or_else(invalid)?;
        let directory = File::from(rustix::fs::open(
            PathBuf::from(OsString::from_vec(path)),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        private(&directory, true)?;
        anyhow::ensure!(
            Identity::of(&directory.metadata()?) == info.directory,
            "Recovery folder changed"
        );
        let expected = if body == RecoveryBody::Document {
            let record = read_only(&directory, RECORD)?;
            private(&record, false)?;
            anyhow::ensure!(
                Identity::of(&record.metadata()?) == info.record,
                "Recovery journal changed"
            );
            let state = read_state(&record, info)?;
            let marker = read_only(&directory, PAYLOAD)?;
            private(&marker, false)?;
            anyhow::ensure!(
                state.payload == Some(Identity::of(&marker.metadata()?)),
                "Recovery completion record changed"
            );
            Some(read_json::<Payload>(&marker, 4096)?)
        } else {
            None
        };
        let source = read_only(
            &directory,
            if body == RecoveryBody::Document {
                DOCUMENT
            } else {
                PREVIOUS
            },
        )?;
        if body == RecoveryBody::Document {
            let metadata = source.metadata()?;
            anyhow::ensure!(
                Identity::of(&metadata) == info.document && metadata.mode() & 0o7777 == 0o600,
                "Retained document changed"
            );
        }
        let snapshot = target.try_clone()?;
        let result = document::snapshot_recovery(source, target, cancel)?;
        if let Some(expected) = expected {
            let actual = digest_body(&snapshot, true, &mut || {
                cancel.check().map_err(io::Error::other)
            })?;
            anyhow::ensure!(
                actual.document.bytes == expected.document.bytes
                    && actual.sha256 == expected.sha256,
                "Retained document failed its completion check. Nothing has been saved."
            );
        }
        Ok((
            result,
            RecoveryProtection {
                directory: info.directory,
            },
        ))
    }

    /// Dismissal removes only a catalog entry; all retained files remain intact.
    pub(crate) fn dismiss(&self, key: &str) -> io::Result<()> {
        self.change(false, |catalog| {
            let index = catalog
                .entries
                .iter()
                .position(|entry| id(&entry.preparation).is_ok_and(|id| id == key))
                .ok_or_else(invalid)?;
            if active(&catalog.entries[index])? {
                return Err(io::Error::other("Active print work cannot be dismissed"));
            }
            catalog.entries.remove(index);
            Ok(())
        })
    }
}
