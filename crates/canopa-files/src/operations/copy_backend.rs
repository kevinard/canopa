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
