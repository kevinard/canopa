use super::*;
use sha2::{Digest, Sha256};
use std::{io::Write, os::unix::fs::PermissionsExt};

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
    fn from_metadata(metadata: &fs::Metadata, digest: [u8; 32]) -> Self {
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

pub(super) struct PreparedCopy {
    pub source: source::Opened,
    pub opened: staging::Prepared,
    pub stage: Stage,
    pub temporary: tempfile::TempDir,
    pub identity: Identity,
    pub fingerprint: Fingerprint,
    pub source_identity: Identity,
    pub source_fingerprint: Fingerprint,
}

pub(super) fn prepare_copy(
    source: &Path,
    parent: &Path,
    cancellation: &Cancellation,
    register: impl FnMut(&staging::Registration<'_>) -> io::Result<()>,
    progress: impl FnMut(u64),
    checkpoint: impl FnMut() -> io::Result<()>,
) -> io::Result<PreparedCopy> {
    prepare_using(
        || source::Opened::open(source),
        parent,
        None,
        cancellation,
        register,
        progress,
        checkpoint,
    )
}

/// Prepare through the source folder and destination parent already retained
/// by the actual transfer selection.
pub(super) fn prepare_pinned(
    source: source::Opened,
    parent: &Path,
    pinned: &fs::File,
    cancellation: &Cancellation,
    register: impl FnMut(&staging::Registration<'_>) -> io::Result<()>,
    progress: impl FnMut(u64),
    checkpoint: impl FnMut() -> io::Result<()>,
) -> io::Result<PreparedCopy> {
    prepare_using(
        || Ok(source),
        parent,
        Some(pinned),
        cancellation,
        register,
        progress,
        checkpoint,
    )
}

fn prepare_using(
    source: impl FnOnce() -> io::Result<source::Opened>,
    parent: &Path,
    pinned: Option<&fs::File>,
    cancellation: &Cancellation,
    mut register: impl FnMut(&staging::Registration<'_>) -> io::Result<()>,
    mut progress: impl FnMut(u64),
    mut checkpoint: impl FnMut() -> io::Result<()>,
) -> io::Result<PreparedCopy> {
    check_cancel(cancellation)?;
    checkpoint()?;
    let source = source()?;
    source.require_current()?;
    let before = source.metadata();
    let (opened, mut stage, mut temporary) = match pinned {
        Some(pinned) => staging::Opened::prepare_in(parent, pinned)?,
        None => staging::Opened::prepare(parent)?,
    };
    register(&opened.registration(&source, &stage)?)?;
    #[cfg(test)]
    super::transfer::stage_boundary::preparing("empty", source.path(), &stage);
    // The durable intent owns this stage from registration onward. Cleanup must
    // validate its placement/identity instead of running through TempDir::drop.
    temporary.disable_cleanup(true);
    checkpoint()?;
    source.require_current()?;
    let (payload, digest) = if before.is_symlink() {
        let target = source.link()?;
        let file = opened.link(&target)?;
        let payload = opened.payload(file, &mut stage)?;
        register(&payload.registration(&source, &stage)?)?;
        #[cfg(test)]
        super::transfer::stage_boundary::preparing("payload", source.path(), &stage);
        payload.require_current()?;
        (
            payload,
            Sha256::digest(target.as_os_str().as_encoded_bytes()).into(),
        )
    } else {
        let mut input = source.regular()?;
        let file = opened.regular()?;
        let mut payload = opened.payload(file, &mut stage)?;
        // Record the actual new leaf before the first data write. A crash
        // between creation and this save leaves an unknown leaf for review.
        register(&payload.registration(&source, &stage)?)?;
        #[cfg(test)]
        super::transfer::stage_boundary::preparing("payload", source.path(), &stage);
        payload.require_current()?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            check_cancel(cancellation)?;
            checkpoint()?;
            payload.require_current()?;
            let count = read_retry(&mut input, &mut buffer)?;
            if count == 0 {
                break;
            }
            payload.file().write_all(&buffer[..count])?;
            #[cfg(test)]
            super::transfer::stage_boundary::preparing("written", source.path(), &stage);
            #[cfg(test)]
            super::undo::abort_boundary::preparing(source.path(), &stage);
            #[cfg(test)]
            super::transfer::stage_boundary::restricted_preparing(
                source.path(),
                &stage,
                cancellation,
            );
            hash.update(&buffer[..count]);
            progress(count as u64);
        }
        if !same_snapshot(before, &input.metadata()?) {
            return Err(changed());
        }
        checkpoint()?;
        payload.require_current()?;
        let output = payload.file();
        output.set_permissions(fs::Permissions::from_mode(before.mode() & 0o777))?;
        let mut times = fs::FileTimes::new();
        if let Ok(time) = before.modified() {
            times = times.set_modified(time);
        }
        if let Ok(time) = before.accessed() {
            times = times.set_accessed(time);
        }
        output.set_times(times)?;
        output.sync_all()?;
        (payload, hash.finalize().into())
    };
    source.require_current()?;
    check_cancel(cancellation)?;
    checkpoint()?;
    source.require_current()?;
    payload.require_current()?;
    let metadata = payload.metadata()?;
    let identity = Identity::from_metadata(&metadata);
    stage.payload_identity = Some(identity);
    Ok(PreparedCopy {
        opened: payload,
        stage,
        temporary,
        identity,
        fingerprint: Fingerprint::from_metadata(&metadata, digest),
        source_identity: Identity::from_metadata(before),
        source_fingerprint: Fingerprint::from_metadata(before, digest),
        source,
    })
}

fn open_regular(path: &Path) -> io::Result<fs::File> {
    let source = source::Opened::open(path)?;
    source.require_current()?;
    source.regular()
}
fn same_snapshot(a: &fs::Metadata, b: &fs::Metadata) -> bool {
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
fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "the source changed while it was being copied",
    )
}
