//! Synchronous descriptor-only content and metadata transfer (ADR0153).
//! Cancellation/checkpoint errors return on the calling thread; no writer can
//! outlive this call. Cancellation callbacks return Interrupted; changed
//! selection/placement checks return WouldBlock. Native I/O errors propagate. The caller
//! owns hashing, sync, cleanup, publication, and durable completion.
use std::{
    fs, io,
    os::unix::fs::{MetadataExt, PermissionsExt},
};

pub(super) fn copy_regular(
    input: &mut fs::File,
    output: &mut fs::File,
    mut progress: impl FnMut(u64) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(cloned) = clone_regular(input, output)? {
        // One progress step runs the same cancellation, placement and
        // namespace checks as a copied window, after the clone.
        return progress(cloned);
    }
    let copied = uucore::buf_copy::copy_with_progress(input, output, progress)?;
    if output.metadata()?.len() != copied {
        return Err(super::fingerprint::changed());
    }
    Ok(())
}

/// Clone a whole source into a new, empty output on btrfs (ADR 0152). No data
/// is read or written: the output shares the source's extents, which btrfs
/// copies on the next write to either file. Only a fresh copy from the start of
/// its source qualifies; anything else, and every refusal, returns `None` with
/// an empty output so uutils copies the bytes. The caller's snapshot checks and
/// output receipt apply to a clone exactly as to a copy.
fn clone_regular(input: &fs::File, output: &fs::File) -> io::Result<Option<u64>> {
    use rustix::fs::{SeekFrom, seek};
    let length = input.metadata()?.len();
    if length == 0
        || seek(input, SeekFrom::Current(0))? != 0
        || seek(output, SeekFrom::Current(0))? != 0
        || output.metadata()?.len() != 0
        || !crate::btrfs::clone_file(input, output)?
    {
        return Ok(None);
    }
    if output.metadata()?.len() != length {
        return Err(super::fingerprint::changed());
    }
    Ok(Some(length))
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

/// Holes of a sparse source, punched in the copy as it is written (ADR0174).
///
/// uutils copies every byte; the holes read as zeros. Each range the source
/// reports as a hole is punched once the copy has passed it, before the
/// write-behind sync, so those zeros are dropped from the page cache instead of
/// reaching the device. The copy keeps the source's logical size and contents
/// and allocates only its data. Holes are found with `SEEK_HOLE`, so a file
/// whose zeros are allocated stays allocated. A destination that cannot punch
/// holes keeps a full copy; that costs space, not data.
///
/// A source written during the copy is refused by the caller's snapshot check,
/// so a hole that gained data is never punched into a published copy.
pub(super) struct Holes {
    ranges: Vec<(u64, u64)>,
    next: usize,
    position: u64,
    punched: u64,
    output: Option<fs::File>,
}

impl Holes {
    /// Find the holes from the input's current offset, then return to it.
    pub(super) fn find(input: &fs::File, output: &fs::File) -> io::Result<Self> {
        use rustix::fs::{SeekFrom, seek};
        let metadata = input.metadata()?;
        let start = seek(input, SeekFrom::Current(0))?;
        let mut holes = Self {
            ranges: Vec::new(),
            next: 0,
            position: start,
            punched: start,
            output: None,
        };
        // A file with every block allocated has no holes to look for.
        if metadata.blocks().saturating_mul(512) >= metadata.len() {
            return Ok(holes);
        }
        let end = metadata.len();
        let mut offset = start;
        while offset < end {
            let data = match seek(input, SeekFrom::Data(offset)) {
                Ok(data) => data.min(end),
                // No data after offset: the rest is one hole.
                Err(rustix::io::Errno::NXIO) => end,
                // A filesystem without hole reporting: copy it in full.
                Err(_) => {
                    holes.ranges.clear();
                    break;
                }
            };
            if data > offset {
                holes.ranges.push((offset, data));
            }
            if data >= end {
                break;
            }
            offset = match seek(input, SeekFrom::Hole(data)) {
                Ok(hole) => hole,
                Err(_) => {
                    holes.ranges.clear();
                    break;
                }
            };
        }
        seek(input, SeekFrom::Start(start))?;
        if !holes.ranges.is_empty() {
            holes.output = Some(output.try_clone()?);
        }
        Ok(holes)
    }

    /// Punch every hole the copy has now passed.
    pub(super) fn written(&mut self, count: u64) -> io::Result<()> {
        use rustix::fs::{FallocateFlags, fallocate};
        self.position = self.position.saturating_add(count);
        let Some(output) = &self.output else {
            return Ok(());
        };
        while let Some(&(start, end)) = self.ranges.get(self.next) {
            let from = start.max(self.punched);
            let to = end.min(self.position);
            if from >= to {
                if end <= self.position {
                    self.next += 1;
                    continue;
                }
                break;
            }
            match fallocate(
                output,
                FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE,
                from,
                to - from,
            ) {
                Ok(()) => {}
                Err(rustix::io::Errno::OPNOTSUPP | rustix::io::Errno::NOSYS) => {
                    self.output = None;
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            }
            self.punched = to;
            if to < end {
                break;
            }
            self.next += 1;
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
