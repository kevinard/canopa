//! History belongs to the engine and its release worker, not inherited file
//! descriptors in a concurrent fork/exec child.
use super::*;
use rustix::fs::{FlockOperation, Mode, OFlags, flock, open};
use std::sync::Arc;

pub(super) struct HistoryLock(fs::File);

impl HistoryLock {
    pub(super) fn acquire(path: &Path) -> io::Result<Arc<Self>> {
        let file = fs::File::from(open(
            path,
            OFlags::CREATE | OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR,
        )?);
        if !file.metadata()?.is_file() {
            return Err(invalid("history lock is not a regular file"));
        }
        flock(&file, FlockOperation::NonBlockingLockExclusive)?;
        Ok(Arc::new(Self(file)))
    }
}

impl Drop for HistoryLock {
    fn drop(&mut self) {
        // Closing our last descriptor alone leaves an inherited fork copy's
        // open-file-description lock alive until exec/exit. Only this last
        // logical owner may unlock; active source leases retain the same guard.
        let _ = flock(&self.0, FlockOperation::Unlock);
    }
}
