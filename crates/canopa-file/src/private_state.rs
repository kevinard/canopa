//! Descriptor-checked private application state, with atomic durable replacement.
//! Consumers own schema, admission budgets and when a preference may be saved.
use rustix::fs::{self, Mode, OFlags};
use std::{
    fs::{DirBuilder, File},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "application state must be private and owned by this account",
    )
}
fn parent(path: &Path, create: bool) -> io::Result<File> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(denied());
    }
    let parent = path.parent().ok_or_else(denied)?;
    if create {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
    }
    let file = File::from(fs::open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )?);
    let metadata = file.metadata()?;
    if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o022 != 0 {
        return Err(denied());
    }
    Ok(file)
}
pub fn read(path: &Path, maximum: usize) -> io::Result<Vec<u8>> {
    let parent = parent(path, false)?;
    let file = File::from(fs::openat(
        &parent,
        path.file_name().ok_or_else(denied)?,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?);
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(denied());
    }
    if metadata.len() > maximum as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "application state exceeds budget",
        ));
    }
    let mut bytes = Vec::new();
    file.take(maximum.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "application state exceeds budget",
        ));
    }
    Ok(bytes)
}
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = parent(path, true)?;
    let name = path.file_name().ok_or_else(denied)?;
    let (temporary, mut file) = (0..32)
        .find_map(|_| {
            let temporary = format!(
                ".canopa-state-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            match fs::openat(
                &parent,
                temporary.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::from_bits_truncate(0o600),
            ) {
                Ok(fd) => Some(Ok((temporary, File::from(fd)))),
                Err(rustix::io::Errno::EXIST) => None,
                Err(error) => Some(Err(io::Error::from(error))),
            }
        })
        .unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "application state temporary namespace exhausted",
            ))
        })?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::renameat(&parent, temporary.as_str(), &parent, name)?;
        parent.sync_all()
    })();
    if result.is_err() {
        let _ = fs::unlinkat(&parent, temporary.as_str(), fs::AtFlags::empty());
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn state_is_private_bounded_atomic_and_never_follows_final_symlinks_or_fifos() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state/preferences.json");
        write(&path, b"first").unwrap();
        write(&path, b"second").unwrap();
        assert_eq!(read(&path, 6).unwrap(), b"second");
        assert_eq!(path.metadata().unwrap().mode() & 0o777, 0o600);
        assert!(read(&path, 5).is_err());
        let other = root.path().join("other");
        std::fs::write(&other, b"untouched").unwrap();
        std::fs::remove_file(&path).unwrap();
        symlink(&other, &path).unwrap();
        assert!(read(&path, 20).is_err());
        write(&path, b"new").unwrap();
        assert_eq!(std::fs::read(other).unwrap(), b"untouched");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read(&path, 20).is_err());
        std::fs::remove_file(&path).unwrap();
        fs::mknodat(
            fs::CWD,
            &path,
            fs::FileType::Fifo,
            Mode::RUSR | Mode::WUSR,
            0,
        )
        .unwrap();
        assert!(read(&path, 20).is_err());
    }
}
