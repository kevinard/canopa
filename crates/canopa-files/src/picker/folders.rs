//! Explicit folder creation, anchored to the directory reviewed in the prompt.
use std::{
    fs::File,
    io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Parent {
    directory: File,
    path: PathBuf,
}

pub fn valid_name(name: &str) -> bool {
    !name.contains('\0') && name.len() <= 255 && canopa_file::picker::file_name(Path::new(name))
}

impl Parent {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        use rustix::fs::{Mode, OFlags, open};
        if !path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                fl!("files-picker-absolute-folder"),
            ));
        }
        let directory = File::from(open(
            &path,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Ok(Self { directory, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn verify(&self) -> io::Result<()> {
        let pinned = self.directory.metadata()?;
        let current = std::fs::metadata(&self.path)?;
        if (pinned.dev(), pinned.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other(fl!("files-picker-folder-changed")));
        }
        Ok(())
    }

    pub fn create(&self, name: &str) -> io::Result<PathBuf> {
        if !valid_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                fl!("files-picker-folder-name-rule"),
            ));
        }
        self.verify()?;
        // mkdirat never replaces an existing entry, including a dangling symlink.
        rustix::fs::mkdirat(
            &self.directory,
            name,
            rustix::fs::Mode::RWXU | rustix::fs::Mode::RWXG | rustix::fs::Mode::RWXO,
        )?;
        self.verify()?;
        Ok(self.path.join(name))
    }
}
