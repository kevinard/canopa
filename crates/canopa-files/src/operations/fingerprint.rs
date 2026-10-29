use super::*;
use std::os::unix::fs::PermissionsExt;
use xxhash_rust::xxh3::{Xxh3, xxh3_128};

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
    // The algorithm-specific required field rejects old SHA-256 receipts rather
    // than reinterpreting them. Development histories have no conversion path.
    xxh3_128: [u8; 16],
}

/// XXH3-128 in canonical big-endian byte order, without a legacy-hash fallback.
pub(super) fn content_digest(bytes: &[u8]) -> [u8; 16] {
    xxh3_128(bytes).to_be_bytes()
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
            content_digest(target.as_bytes())
        } else if before.is_file() {
            let mut input = fs::File::open(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
            if !same_snapshot(&before, &input.metadata()?) {
                return Err(changed());
            }
            let mut hash = Xxh3::new();
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
            hash.digest128().to_be_bytes()
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
    pub(super) fn from_metadata(metadata: &fs::Metadata, digest: [u8; 16]) -> Self {
        Self {
            size: metadata.len(),
            mode: metadata.mode() & 0o777,
            modified: metadata.mtime(),
            modified_nanos: metadata.mtime_nsec(),
            link: metadata.is_symlink(),
            xxh3_128: digest,
        }
    }
    pub fn read(path: &Path, cancellation: &Cancellation) -> io::Result<Self> {
        check_cancel(cancellation)?;
        let source = source::Opened::open(path)?;
        source.require_current()?;
        let before = source.metadata();
        let digest = if before.is_symlink() {
            content_digest(source.link()?.as_os_str().as_encoded_bytes())
        } else {
            let mut file = source.regular()?;
            let mut hash = Xxh3::new();
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
            hash.digest128().to_be_bytes()
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

/// Fingerprint the actual output of a successful upstream copy for later Undo
/// and destructive-action checks. Trust the copier's source/output transfer;
/// this reads only the held output, without moving its writer offset or reopening
/// its pathname (including at mode zero).
pub(super) fn copied_output_digest(
    output: &fs::File,
    cancellation: &Cancellation,
) -> io::Result<[u8; 16]> {
    check_cancel(cancellation)?;
    let output_before = output.metadata()?;
    if !output_before.is_file() {
        return Err(invalid(
            "Only an opened regular output can be fingerprinted.",
        ));
    }
    let mut hash = Xxh3::new();
    let mut buffer = [0; 64 * 1024];
    let mut offset = 0;
    loop {
        let count = read_at_retry(output, &mut buffer, offset, cancellation)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        offset += count as u64;
        #[cfg(test)]
        receipt_tests::read_chunk();
    }
    if !same_snapshot(&output_before, &output.metadata()?) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "The prepared output changed while its receipt was read.",
        ));
    }
    Ok(hash.digest128().to_be_bytes())
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
    pub(super) fn read_chunk() {
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
    fn content_fingerprints_use_canonical_xxh3_128_vectors() {
        for (bytes, expected) in [
            (&b""[..], 0x99aa06d3014798d86001c324468d497fu128),
            (&b"abc"[..], 0x06b05ab6733a618578af5f94892f3950u128),
        ] {
            assert_eq!(content_digest(bytes), expected.to_be_bytes());
            let mut streamed = Xxh3::new();
            for byte in bytes {
                streamed.update(std::slice::from_ref(byte));
            }
            assert_eq!(streamed.digest128().to_be_bytes(), expected.to_be_bytes());
        }
    }

    #[test]
    fn old_or_ambiguous_content_receipts_are_not_reinterpreted() {
        let file = containing(b"copied bytes");
        let fingerprint =
            Fingerprint::from_metadata(&file.metadata().unwrap(), content_digest(b"copied bytes"));
        let value = serde_json::to_value(&fingerprint).unwrap();
        assert_eq!(
            serde_json::from_value::<Fingerprint>(value.clone()).unwrap(),
            fingerprint
        );
        let mut legacy = value.clone();
        legacy.as_object_mut().unwrap().remove("xxh3_128");
        legacy["digest"] = serde_json::json!(vec![0; 32]);
        assert!(serde_json::from_value::<Fingerprint>(legacy).is_err());
        let mut ambiguous = value.clone();
        ambiguous["digest"] = serde_json::json!(vec![0; 32]);
        assert!(serde_json::from_value::<Fingerprint>(ambiguous).is_err());
        for width in [0, 15, 17, 32] {
            let mut malformed = value.clone();
            malformed["xxh3_128"] = serde_json::json!(vec![0; width]);
            assert!(serde_json::from_value::<Fingerprint>(malformed).is_err());
        }
    }

    #[test]
    fn a_copy_receipt_hashes_the_complete_output_through_eof() {
        let bytes: Vec<_> = (0..2 * 64 * 1024 + 17).map(|n| (n % 251) as u8).collect();
        for length in [0, 1, 63, 64, 65, 65535, 65536, 65537, bytes.len()] {
            let selected = &bytes[..length];
            let output = containing(selected);
            let digest = copied_output_digest(&output, &Cancellation::default()).unwrap();
            let expected = content_digest(selected);
            assert_eq!(digest, expected, "length {length}");
        }
    }

    #[test]
    fn cancelled_output_hashing_cannot_make_a_copy_receipt() {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert_eq!(
            copied_output_digest(&containing(b"bytes"), &cancellation)
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted,
        );
        let cancellation = Cancellation::default();
        let signal = cancellation.clone();
        let _reset = at_chunk(move || signal.cancel());
        let bytes = vec![b'x'; 2 * 64 * 1024 + 17];
        assert_eq!(
            copied_output_digest(&containing(&bytes), &cancellation)
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted,
        );
    }

    #[test]
    fn the_output_snapshot_refuses_edits_to_bytes_already_hashed() {
        use std::os::unix::fs::FileExt;
        let bytes = vec![b'x'; 2 * 64 * 1024 + 17];
        let output = containing(&bytes);
        let edited = output.try_clone().unwrap();
        let _reset = at_chunk(move || {
            assert_eq!(edited.write_at(b"y", 0).unwrap(), 1);
            // Make the snapshot change deterministic even on coarse clocks.
            edited
                .set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
                .unwrap();
        });
        assert_eq!(
            copied_output_digest(&output, &Cancellation::default())
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock,
        );
    }

    #[test]
    fn an_owned_output_receipt_uses_its_readable_fd_even_at_mode_zero() {
        let mut output = containing(b"owned copied bytes");
        output
            .set_permissions(fs::Permissions::from_mode(0o0))
            .unwrap();
        let expected = content_digest(b"owned copied bytes");
        assert_eq!(
            copied_output_digest(&output, &Cancellation::default()).unwrap(),
            expected
        );
        assert_eq!(
            output.stream_position().unwrap(),
            b"owned copied bytes".len() as u64
        );
    }
}
