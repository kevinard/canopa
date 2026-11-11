//! Synchronous descriptor-only content and metadata transfer (ADR0153).
//! Cancellation/checkpoint errors return on the calling thread; no writer can
//! outlive this call. Cancellation callbacks return Interrupted; changed
//! selection/placement checks return WouldBlock. Native I/O errors propagate. The caller
//! owns hashing, sync, cleanup, publication, and durable completion.
use std::{fs, io, os::unix::fs::PermissionsExt};

pub(super) fn copy_regular(
    input: &mut fs::File,
    output: &mut fs::File,
    progress: impl FnMut(u64) -> io::Result<()>,
) -> io::Result<()> {
    let copied = uucore::buf_copy::copy_with_progress(input, output, progress)?;
    if output.metadata()?.len() != copied {
        return Err(super::fingerprint::changed());
    }
    Ok(())
}

/// Bounded write-behind for large outputs (ADR0165). Without it a slow device
/// accepts the whole file into the page cache at memory speed, progress reaches
/// the end at once and the final sync then stalls for the real transfer. After
/// each window the file's data is synced, so at most one window is dirty and
/// progress follows the device. Smaller files are untouched.
pub(super) struct WriteBehind {
    file: fs::File,
    pending: u64,
}

impl WriteBehind {
    pub(super) fn new(output: &fs::File) -> io::Result<Self> {
        Ok(Self {
            file: output.try_clone()?,
            pending: 0,
        })
    }

    pub(super) fn written(&mut self, count: u64) -> io::Result<()> {
        self.pending += count;
        if self.pending >= super::durability::MAX_BYTES {
            self.pending = 0;
            rustix::fs::fdatasync(&self.file)?;
        }
        Ok(())
    }
}

/// Apply policy after receipt reads, so they cannot change the preserved atime.
pub(super) fn restore_metadata(output: &fs::File, metadata: &fs::Metadata) -> io::Result<()> {
    output.set_permissions(fs::Permissions::from_mode(
        metadata.permissions().mode() & 0o777,
    ))?;
    let mut times = fs::FileTimes::new();
    if let Ok(time) = metadata.modified() {
        times = times.set_modified(time);
    }
    if let Ok(time) = metadata.accessed() {
        times = times.set_accessed(time);
    }
    output.set_times(times)
}
