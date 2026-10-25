use super::*;
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;

pub(super) mod source;
#[cfg(test)]
mod source_boundary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Version {
    pub identity: Identity,
    size: u64,
    mode: u32,
    modified: i64,
    modified_nanos: i64,
    changed: i64,
    changed_nanos: i64,
}
impl Version {
    pub fn read(path: &Path) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        Ok(Self::from_metadata(&metadata))
    }
    pub(super) fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            identity: Identity::from_metadata(metadata),
            size: metadata.len(),
            mode: metadata.mode(),
            modified: metadata.mtime(),
            modified_nanos: metadata.mtime_nsec(),
            changed: metadata.ctime(),
            changed_nanos: metadata.ctime_nsec(),
        }
    }
    pub(super) fn matches_opened(&self, file: &fs::File, moved: bool) -> io::Result<bool> {
        let mut actual = Self::from_metadata(&file.metadata()?);
        if moved {
            actual.changed = self.changed;
            actual.changed_nanos = self.changed_nanos;
        }
        Ok(*self == actual)
    }
    pub fn matches(&self, path: &Path) -> bool {
        Self::read(path).is_ok_and(|other| self == &other)
    }
    pub fn matches_moved(&self, path: &Path) -> bool {
        Self::read(path).is_ok_and(|other| {
            self.identity == other.identity
                && self.size == other.size
                && self.mode == other.mode
                && self.modified == other.modified
                && self.modified_nanos == other.modified_nanos
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Fingerprint {
    size: u64,
    mode: u32,
    modified: i64,
    modified_nanos: i64,
    link: bool,
    digest: [u8; 32],
}

impl Fingerprint {
    pub(super) fn copied_bytes(&self) -> u64 {
        if self.link { 0 } else { self.size }
    }
    pub(super) fn read_opened(file: &fs::File, cancellation: &Cancellation) -> io::Result<Self> {
        use std::os::fd::AsRawFd;
        check_cancel(cancellation)?;
        let before = file.metadata()?;
        let digest = if before.is_symlink() {
            let target = rustix::fs::readlinkat(file, c"", Vec::new())?;
            Sha256::digest(target.as_bytes()).into()
        } else if before.is_file() {
            let mut input = fs::File::open(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
            if !same_snapshot(&before, &input.metadata()?) {
                return Err(changed());
            }
            let mut hash = Sha256::new();
            let mut buffer = [0; 64 * 1024];
            loop {
                check_cancel(cancellation)?;
                let count = read_retry(&mut input, &mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            if !same_snapshot(&before, &input.metadata()?) {
                return Err(changed());
            }
            hash.finalize().into()
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Only an opened regular file or link can be verified.",
            ));
        };
        if !same_snapshot(&before, &file.metadata()?) {
            return Err(changed());
        }
        Ok(Self::from_metadata(&before, digest))
    }
    pub fn restore_metadata(&self, path: &Path) -> io::Result<()> {
        use rustix::fs::{AtFlags, Timespec, Timestamps, UTIME_OMIT, utimensat};
        if !self.link {
            open_regular(path)?.set_permissions(fs::Permissions::from_mode(self.mode))?;
        }
        utimensat(
            rustix::fs::CWD,
            path,
            &Timestamps {
                last_access: Timespec {
                    tv_sec: 0,
                    tv_nsec: UTIME_OMIT,
                },
                last_modification: Timespec {
                    tv_sec: self.modified,
                    tv_nsec: self.modified_nanos,
                },
            },
            AtFlags::SYMLINK_NOFOLLOW,
        )?;
        Ok(())
    }
    pub(super) fn restore_opened(&self, file: &fs::File) -> io::Result<()> {
        use rustix::fs::{AtFlags, Timespec, Timestamps, UTIME_OMIT, utimensat};
        let metadata = file.metadata()?;
        if metadata.is_symlink() != self.link || (!self.link && !metadata.is_file()) {
            return Err(invalid(
                "The prepared saved version changed its entry type.",
            ));
        }
        if !self.link {
            file.set_permissions(fs::Permissions::from_mode(self.mode))?;
        }
        utimensat(
            file,
            c"",
            &Timestamps {
                last_access: Timespec {
                    tv_sec: 0,
                    tv_nsec: UTIME_OMIT,
                },
                last_modification: Timespec {
                    tv_sec: self.modified,
                    tv_nsec: self.modified_nanos,
                },
            },
            AtFlags::EMPTY_PATH | AtFlags::SYMLINK_NOFOLLOW,
        )?;
        Ok(())
    }
    pub(super) fn from_metadata(metadata: &fs::Metadata, digest: [u8; 32]) -> Self {
        Self {
            size: metadata.len(),
            mode: metadata.mode() & 0o777,
            modified: metadata.mtime(),
            modified_nanos: metadata.mtime_nsec(),
            link: metadata.is_symlink(),
            digest,
        }
    }
    pub fn read(path: &Path, cancellation: &Cancellation) -> io::Result<Self> {
        check_cancel(cancellation)?;
        let source = source::Opened::open(path)?;
        source.require_current()?;
        let before = source.metadata();
        let digest = if before.is_symlink() {
            Sha256::digest(source.link()?.as_os_str().as_encoded_bytes()).into()
        } else {
            let mut file = source.regular()?;
            let mut hash = Sha256::new();
            let mut buffer = [0; 64 * 1024];
            loop {
                check_cancel(cancellation)?;
                let count = read_retry(&mut file, &mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            if !same_snapshot(before, &file.metadata()?) {
                return Err(changed());
            }
            hash.finalize().into()
        };
        source.require_current()?;
        Ok(Self::from_metadata(before, digest))
    }
    pub fn matches(&self, path: &Path, cancellation: &Cancellation) -> io::Result<bool> {
        match Self::read(path, cancellation) {
            Ok(other) => Ok(self == &other),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::InvalidInput
                ) =>
            {
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }
}

fn open_regular(path: &Path) -> io::Result<fs::File> {
    let source = source::Opened::open(path)?;
    source.require_current()?;
    source.regular()
}

/// Compare the complete selected and staged byte streams, hashing matching bytes
/// once. Equal bytes have equal SHA-256 digests; no second compression is needed.
/// Keep both snapshots for the entire comparison. The output stays on its held
/// readable descriptor, even at mode zero, and its writer offset is untouched.
pub(super) fn verified_copy_digest(
    source: &fs::File,
    output: &fs::File,
    cancellation: &Cancellation,
) -> io::Result<[u8; 32]> {
    use std::os::fd::AsRawFd;
    check_cancel(cancellation)?;
    let source_before = source.metadata()?;
    let output_before = output.metadata()?;
    if !source_before.is_file() || !output_before.is_file() {
        return Err(invalid(
            "Only opened regular files can verify copied bytes.",
        ));
    }
    // As in read_opened, obtain an independent reader of the pinned inode;
    // neither the user's pathname nor the copier's current offset supplies data.
    let mut input = fs::File::open(format!("/proc/self/fd/{}", source.as_raw_fd()))?;
    if !same_snapshot(&source_before, &input.metadata()?) {
        return Err(changed());
    }
    let mut hash = Sha256::new();
    let mut selected = [0; 64 * 1024];
    let mut staged = [0; 64 * 1024];
    let mut offset = 0;
    loop {
        check_cancel(cancellation)?;
        let count = read_retry(&mut input, &mut selected)?;
        if count == 0 {
            if read_at_retry(output, &mut staged[..1], offset, cancellation)? != 0 {
                return Err(different_copy());
            }
            break;
        }
        let mut read = 0;
        while read < count {
            let next = read_at_retry(
                output,
                &mut staged[read..count],
                offset + read as u64,
                cancellation,
            )?;
            if next == 0 {
                return Err(different_copy());
            }
            read += next;
        }
        if selected[..count] != staged[..count] {
            return Err(different_copy());
        }
        hash.update(&selected[..count]);
        offset += count as u64;
        #[cfg(test)]
        receipt_tests::compared_chunk();
    }
    if !same_snapshot(&source_before, &input.metadata()?)
        || !same_snapshot(&source_before, &source.metadata()?)
    {
        return Err(changed());
    }
    if !same_snapshot(&output_before, &output.metadata()?) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "The prepared output changed while its receipt was read.",
        ));
    }
    Ok(hash.finalize().into())
}

fn different_copy() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The copied bytes differ from the source. Staged data is preserved for Recovery.",
    )
}

fn read_at_retry(
    file: &fs::File,
    buffer: &mut [u8],
    offset: u64,
    cancellation: &Cancellation,
) -> io::Result<usize> {
    use std::os::unix::fs::FileExt;
    loop {
        check_cancel(cancellation)?;
        match file.read_at(buffer, offset) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}
pub(super) fn same_snapshot(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    Identity::from_metadata(a) == Identity::from_metadata(b)
        && a.len() == b.len()
        && a.mode() == b.mode()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
fn read_retry(file: &mut fs::File, buffer: &mut [u8]) -> io::Result<usize> {
    loop {
        match file.read(buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}
pub(super) fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "the source changed while it was being copied",
    )
}

#[cfg(test)]
mod receipt_tests {
    use super::*;
    use std::io::{Seek, Write};

    type Action = Box<dyn FnOnce()>;
    thread_local! {
        static AT_CHUNK: std::cell::RefCell<Option<Action>> = const { std::cell::RefCell::new(None) };
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            AT_CHUNK.with(|action| action.borrow_mut().take());
        }
    }
    fn at_chunk(action: impl FnOnce() + 'static) -> Reset {
        AT_CHUNK.with(|slot| assert!(slot.borrow_mut().replace(Box::new(action)).is_none()));
        Reset
    }
    pub(super) fn compared_chunk() {
        let action = AT_CHUNK.with(|slot| slot.borrow_mut().take());
        if let Some(action) = action {
            action();
        }
    }

    fn containing(bytes: &[u8]) -> fs::File {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(bytes).unwrap();
        file
    }

    #[test]
    fn a_verified_copy_receipt_covers_every_byte_and_both_eofs() {
        let bytes: Vec<_> = (0..2 * 64 * 1024 + 17).map(|n| (n % 251) as u8).collect();
        for length in [0, 1, 63, 64, 65, 65535, 65536, 65537, bytes.len()] {
            let selected = &bytes[..length];
            let source = containing(selected);
            let output = containing(selected);
            let digest = verified_copy_digest(&source, &output, &Cancellation::default()).unwrap();
            let expected: [u8; 32] = Sha256::digest(selected).into();
            assert_eq!(digest, expected, "length {length}");
        }
        for offset in [0, 63, 64, 65535, 65536, bytes.len() - 1] {
            let mut different = bytes.clone();
            different[offset] ^= 0xff;
            assert_eq!(
                verified_copy_digest(
                    &containing(&bytes),
                    &containing(&different),
                    &Cancellation::default(),
                )
                .unwrap_err()
                .kind(),
                io::ErrorKind::WouldBlock,
                "offset {offset}",
            );
        }
        for (left, right) in [
            (&bytes[..], &bytes[..bytes.len() - 1]),
            (&bytes[..bytes.len() - 1], &bytes[..]),
        ] {
            assert_eq!(
                verified_copy_digest(
                    &containing(left),
                    &containing(right),
                    &Cancellation::default()
                )
                .unwrap_err()
                .kind(),
                io::ErrorKind::WouldBlock,
            );
        }
    }

    #[test]
    fn a_cancelled_verification_cannot_make_a_copy_receipt() {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert_eq!(
            verified_copy_digest(&containing(b"bytes"), &containing(b"bytes"), &cancellation)
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted,
        );
        let cancellation = Cancellation::default();
        let signal = cancellation.clone();
        let _reset = at_chunk(move || signal.cancel());
        let bytes = vec![b'x'; 2 * 64 * 1024 + 17];
        assert_eq!(
            verified_copy_digest(&containing(&bytes), &containing(&bytes), &cancellation)
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted,
        );
    }

    #[test]
    fn snapshots_refuse_edits_to_bytes_already_compared() {
        use std::os::unix::fs::FileExt;
        for edit_source in [true, false] {
            let bytes = vec![b'x'; 2 * 64 * 1024 + 17];
            let source = containing(&bytes);
            let output = containing(&bytes);
            let edited = if edit_source { &source } else { &output }
                .try_clone()
                .unwrap();
            let _reset = at_chunk(move || {
                assert_eq!(edited.write_at(b"y", 0).unwrap(), 1);
                // Make the snapshot change deterministic even on coarse clocks.
                edited
                    .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
                    .unwrap();
            });
            assert_eq!(
                verified_copy_digest(&source, &output, &Cancellation::default())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock,
            );
        }
    }

    #[test]
    fn an_owned_output_receipt_uses_its_readable_fd_even_at_mode_zero() {
        let source = containing(b"owned copied bytes");
        let mut output = containing(b"owned copied bytes");
        output
            .set_permissions(fs::Permissions::from_mode(0o0))
            .unwrap();
        let expected: [u8; 32] = Sha256::digest(b"owned copied bytes").into();
        assert_eq!(
            verified_copy_digest(&source, &output, &Cancellation::default()).unwrap(),
            expected
        );
        assert_eq!(
            output.stream_position().unwrap(),
            b"owned copied bytes".len() as u64
        );
    }
}
