//! Read the opened leaf itself. Namespace validation never supplies a new
//! readable source, even for a dangling final link or a replaced parent alias.
use super::*;
use rustix::fs::{Mode, OFlags, open, openat, readlinkat};
use std::os::{fd::AsRawFd, unix::ffi::OsStringExt};

pub(in crate::operations) struct Opened {
    entry: fs::File,
    parent: fs::File,
    path: PathBuf,
    parent_identity: Identity,
    metadata: fs::Metadata,
}

/// Captured from the actual copy reader before preparation, never reconstructed
/// from a path after restart. Missing generations remain missing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::operations) struct Reproduction {
    parent: Option<original::Original>,
    entry: Option<original::Original>,
    version: Version,
}

pub(in crate::operations) struct Reproduced {
    proof: Reproduction,
    source: Opened,
    payload: fs::File,
    version: Version,
}
fn reproduction_attention() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The interrupted copy cannot be reproduced from its original source. Its partial data is preserved for Recovery.",
    )
}

#[derive(Debug)]
struct ReproductionProblem(io::Error);
impl std::fmt::Display for ReproductionProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
impl std::error::Error for ReproductionProblem {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
pub(in crate::operations) fn is_reproduction_problem(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|cause| cause.is::<ReproductionProblem>())
}
pub(in crate::operations) fn reproduction_problem(error: io::Error) -> io::Error {
    if error.kind() == io::ErrorKind::Interrupted || is_reproduction_problem(&error) {
        error
    } else {
        io::Error::new(error.kind(), ReproductionProblem(error))
    }
}
impl Reproduction {
    fn require(&self, opened: &Opened) -> io::Result<()> {
        self.parent
            .ok_or_else(reproduction_attention)?
            .require(opened.parent())?;
        self.entry
            .ok_or_else(reproduction_attention)?
            .require(opened.entry())?;
        // Returning this same original object can change ctime. Byte comparison
        // still has to succeed, and the fresh reader retains strict live checks.
        if !self.version.matches_opened(opened.entry(), true)? {
            return Err(reproduction_attention());
        }
        opened.require_current()
    }
    fn open(&self, path: &Path) -> io::Result<Opened> {
        let opened = Opened::open(path).map_err(|_| reproduction_attention())?;
        self.require(&opened)?;
        Ok(opened)
    }
    pub fn reproduce(
        &self,
        path: &Path,
        payload: &fs::File,
        cancellation: &Cancellation,
    ) -> io::Result<Reproduced> {
        use std::io::Read;
        check_cancel(cancellation)?;
        let source = self.open(path)?;
        let metadata = payload.metadata()?;
        let version = Version::from_metadata(&metadata);
        if metadata.is_symlink() && source.metadata.is_symlink() {
            if readlinkat(payload, c"", Vec::new())?.as_bytes()
                != source.link()?.as_os_str().as_encoded_bytes()
            {
                return Err(reproduction_attention());
            }
        } else if metadata.is_file()
            && source.metadata.is_file()
            && metadata.len() <= source.metadata.len()
        {
            let mut original = source.regular()?;
            let mut partial = fs::File::from(open(
                format!("/proc/self/fd/{}", payload.as_raw_fd()),
                OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            if !version.matches_opened(&partial, false)? {
                return Err(reproduction_attention());
            }
            let mut left = [0; 64 * 1024];
            let mut right = [0; 64 * 1024];
            let mut remaining = metadata.len();
            while remaining != 0 {
                check_cancel(cancellation)?;
                let count = remaining.min(left.len() as u64) as usize;
                original.read_exact(&mut left[..count])?;
                partial.read_exact(&mut right[..count])?;
                #[cfg(test)]
                reproduction_boundary("chunk");
                if left[..count] != right[..count] {
                    return Err(reproduction_attention());
                }
                remaining -= count as u64;
            }
        } else {
            return Err(reproduction_attention());
        }
        let verified = Reproduced {
            proof: self.clone(),
            source,
            payload: payload.try_clone()?,
            version,
        };
        verified.require(payload, cancellation)?;
        Ok(verified)
    }
}
impl Reproduced {
    /// Recheck the pinned objects and their current names after the consumed
    /// phase save. A changed source or partial never retains unlink authority.
    pub fn require(&self, named: &fs::File, cancellation: &Cancellation) -> io::Result<()> {
        check_cancel(cancellation)?;
        self.proof.require(&self.source)?;
        let current = self.proof.open(self.source.path())?;
        if !same_snapshot(&self.source.metadata, &current.entry.metadata()?) {
            return Err(reproduction_attention());
        }
        if !self.version.matches_opened(&self.payload, false)?
            || !self.version.matches_opened(named, false)?
        {
            return Err(reproduction_attention());
        }
        Ok(())
    }
}

#[cfg(test)]
type ReproductionHook = (&'static str, Box<dyn FnOnce()>);
#[cfg(test)]
thread_local! {
    static REPRODUCTION_HOOK: std::cell::RefCell<Option<ReproductionHook>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
pub(in crate::operations) struct ReproductionReset;
#[cfg(test)]
impl Drop for ReproductionReset {
    fn drop(&mut self) {
        REPRODUCTION_HOOK.with(|hook| {
            hook.borrow_mut().take();
        });
    }
}
#[cfg(test)]
pub(in crate::operations) fn at_reproduction(
    point: &'static str,
    action: impl FnOnce() + 'static,
) -> ReproductionReset {
    REPRODUCTION_HOOK.with(|hook| {
        assert!(
            hook.borrow_mut()
                .replace((point, Box::new(action)))
                .is_none()
        );
    });
    ReproductionReset
}
#[cfg(test)]
pub(in crate::operations) fn reproduction_boundary(point: &str) {
    let action = REPRODUCTION_HOOK.with(|hook| {
        let mut hook = hook.borrow_mut();
        if hook.as_ref().is_some_and(|(wanted, _)| *wanted == point) {
            hook.take()
        } else {
            None
        }
    });
    if let Some((_, action)) = action {
        action();
    }
}

impl Opened {
    pub(in crate::operations) fn reproduction(&self) -> Reproduction {
        Reproduction {
            parent: original::Original::capture(&self.parent).ok(),
            entry: original::Original::capture(&self.entry).ok(),
            version: Version::from_metadata(&self.metadata),
        }
    }
    pub(in crate::operations) fn entry(&self) -> &fs::File {
        &self.entry
    }
    pub(in crate::operations) fn parent(&self) -> &fs::File {
        &self.parent
    }
    pub(in crate::operations) fn path(&self) -> &Path {
        &self.path
    }
    pub(in crate::operations) fn open(path: &Path) -> io::Result<Self> {
        let parent_path = path
            .parent()
            .ok_or_else(|| invalid("A copied item needs its parent folder."))?;
        let parent = fs::File::from(open(
            parent_path,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Self::open_in(path, &parent)
    }

    /// Read a child of the actual selected folder, never a replacement opened
    /// later through that folder's display path.
    pub(in crate::operations) fn open_in(path: &Path, parent: &fs::File) -> io::Result<Self> {
        let name = path
            .file_name()
            .ok_or_else(|| invalid("A copied item needs its name."))?;
        let parent = parent.try_clone()?;
        let entry = fs::File::from(openat(
            &parent,
            name,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let metadata = entry.metadata()?;
        if !metadata.is_file() && !metadata.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "only regular files and symbolic links can be copied",
            ));
        }
        Ok(Self {
            parent_identity: Identity::from_metadata(&parent.metadata()?),
            entry,
            parent,
            path: path.to_owned(),
            metadata,
        })
    }

    pub(super) fn metadata(&self) -> &fs::Metadata {
        &self.metadata
    }

    pub(in crate::operations) fn require_current(&self) -> io::Result<()> {
        if !fs::metadata(self.path.parent().unwrap()).is_ok_and(|metadata| {
            metadata.is_dir() && Identity::from_metadata(&metadata) == self.parent_identity
        }) {
            return Err(changed());
        }
        let named = fs::File::from(openat(
            &self.parent,
            self.path.file_name().unwrap(),
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        if !same_snapshot(&self.metadata, &named.metadata()?)
            || !same_snapshot(&self.metadata, &self.entry.metadata()?)
        {
            return Err(changed());
        }
        Ok(())
    }

    pub(super) fn regular(&self) -> io::Result<fs::File> {
        if !self.metadata.is_file() {
            return Err(invalid("The opened copy source is not a regular file."));
        }
        // Only an already-typed regular leaf reaches this magic link. Following
        // it reopens our pinned object, including after unlink, without opening
        // an occupant substituted at the source name or touching a device.
        let file = fs::File::from(open(
            format!("/proc/self/fd/{}", self.entry.as_raw_fd()),
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        if !same_snapshot(&self.metadata, &file.metadata()?) {
            return Err(changed());
        }
        Ok(file)
    }

    pub(super) fn link(&self) -> io::Result<PathBuf> {
        if !self.metadata.is_symlink() {
            return Err(invalid("The opened copy source is not a symbolic link."));
        }
        // Linux readlinkat with an empty path reads the O_PATH final link itself.
        let target = readlinkat(&self.entry, c"", Vec::new())?;
        Ok(std::ffi::OsString::from_vec(target.into_bytes()).into())
    }
}
