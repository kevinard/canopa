//! Descriptor-safe, content-free bounded reads for local assets and configuration.
use std::io::Read;
use std::path::Path;

#[derive(Debug)]
pub struct BoundedReadError {
    /// Bytes consumed before failure, including an overflow-detection probe.
    pub bytes_read: usize,
}

impl std::fmt::Display for BoundedReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("local asset is unavailable, invalid or over budget")
    }
}

impl std::error::Error for BoundedReadError {}

pub fn read_bounded(path: &Path, maximum_bytes: usize) -> Result<Vec<u8>, BoundedReadError> {
    use rustix::fs::{Mode, OFlags, open};
    let unavailable = || BoundedReadError { bytes_read: 0 };
    let probe_limit = maximum_bytes.checked_add(1).ok_or_else(unavailable)?;
    // Validate the opened descriptor, not a prior path lookup. NONBLOCK avoids
    // waiting for FIFO writers; symlinks to regular files remain supported.
    let fd = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| unavailable())?;
    let file = std::fs::File::from(fd);
    let metadata = file.metadata().map_err(|_| unavailable())?;
    if !metadata.is_file() || metadata.len() > maximum_bytes as u64 {
        return Err(unavailable());
    }
    let mut bytes = Vec::new();
    let read = file.take(probe_limit as u64).read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > maximum_bytes {
        return Err(BoundedReadError {
            bytes_read: bytes.len(),
        });
    }
    Ok(bytes)
}
