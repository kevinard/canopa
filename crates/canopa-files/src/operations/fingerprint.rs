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
    pub(super) fn digest(&self) -> [u8; 32] {
        self.digest
    }
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

/// Read the owned output through its already-readable descriptor, including
/// under a restrictive umask. Positioned reads leave the writer offset alone.
pub(super) fn copied_digest(file: &fs::File, cancellation: &Cancellation) -> io::Result<[u8; 32]> {
    use std::os::unix::fs::FileExt;
    let before = file.metadata()?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut offset = 0;
    loop {
        check_cancel(cancellation)?;
        let count = match file.read_at(&mut buffer, offset) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        offset += count as u64;
    }
    if !same_snapshot(&before, &file.metadata()?) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "The prepared output changed while its receipt was read.",
        ));
    }
    Ok(hash.finalize().into())
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
    use std::io::Write;

    #[test]
    fn an_owned_output_receipt_uses_its_readable_fd_even_at_mode_zero() {
        let mut output = tempfile::tempfile().unwrap();
        output.write_all(b"owned copied bytes").unwrap();
        output
            .set_permissions(fs::Permissions::from_mode(0o0))
            .unwrap();
        let expected: [u8; 32] = Sha256::digest(b"owned copied bytes").into();
        assert_eq!(
            copied_digest(&output, &Cancellation::default()).unwrap(),
            expected
        );
    }
}
