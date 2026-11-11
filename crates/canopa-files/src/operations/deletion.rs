//! Permanent removal, by explicit confirmed request only. It is not undoable and
//! keeps no journal: an interruption leaves the remaining entries in place.
//! Traversal is descriptor-relative, never follows symbolic links and never
//! descends into another mounted filesystem. Data that Recovery or a queued
//! request still needs is refused instead of being destroyed.
use super::*;
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, openat, statat, unlinkat};
use std::{
    ffi::{CStr, CString},
    os::{fd::OwnedFd, unix::ffi::OsStrExt},
};

/// Deeper trees are refused rather than exhausting descriptors.
const DEPTH_LIMIT: usize = 384;

#[derive(Clone, Debug, Default)]
pub struct DeleteOutcome {
    /// Selected entries that no longer exist.
    pub completed: Vec<PathBuf>,
    pub failures: Vec<(PathBuf, String)>,
    /// Selected entries not started, for example after cancellation.
    pub remaining: Vec<PathBuf>,
    pub cancelled: bool,
    /// Removed filesystem entries, including folder contents.
    pub entries: u64,
    pub bytes: u64,
}

/// Running totals for presentation; never a resumption point.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeleteProgress {
    pub entries: u64,
    pub bytes: u64,
}

pub(super) struct Removal<'a> {
    cancellation: &'a Cancellation,
    progress: &'a mut dyn FnMut(&Path, DeleteProgress),
    totals: DeleteProgress,
    device: u64,
}

impl<'a> Removal<'a> {
    pub(super) fn new(
        cancellation: &'a Cancellation,
        progress: &'a mut dyn FnMut(&Path, DeleteProgress),
    ) -> Self {
        Self {
            cancellation,
            progress,
            totals: DeleteProgress::default(),
            device: 0,
        }
    }

    pub(super) fn totals(&self) -> DeleteProgress {
        self.totals
    }

    /// Remove `path` and, for a folder, everything below it. The final
    /// component is removed itself: a symbolic link is unlinked, not followed.
    pub(super) fn remove(&mut self, path: &Path) -> io::Result<()> {
        validate_target(path)?;
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(invalid("deletion paths cannot contain parent traversal"));
        }
        let parent = path.parent().ok_or_else(|| invalid("missing parent"))?;
        let name = CString::new(path.file_name().unwrap().as_bytes())
            .map_err(|_| invalid("invalid file name"))?;
        let parent = open_directory(rustix::fs::CWD, parent)?;
        let stat = statat(&parent, &name, AtFlags::SYMLINK_NOFOLLOW)?;
        self.device = stat.st_dev;
        self.entry(&parent, &name, path, 0)
    }

    fn entry(
        &mut self,
        parent: &OwnedFd,
        name: &CStr,
        path: &Path,
        depth: usize,
    ) -> io::Result<()> {
        check_cancel(self.cancellation)?;
        let stat = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            if stat.st_dev != self.device {
                return Err(io::Error::other(format!(
                    "{} is another mounted filesystem; it was not deleted",
                    path.display()
                )));
            }
            if depth >= DEPTH_LIMIT {
                return Err(io::Error::other(format!(
                    "{} is nested too deeply to delete here",
                    path.display()
                )));
            }
            let directory = open_directory(parent, name)?;
            let opened = rustix::fs::fstat(&directory)?;
            if (opened.st_dev, opened.st_ino) != (stat.st_dev, stat.st_ino) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!("{} changed while it was being deleted", path.display()),
                ));
            }
            let mut children = Vec::new();
            for child in Dir::read_from(&directory)? {
                let child = child?;
                let child = child.file_name();
                if child != c"." && child != c".." {
                    children.push(child.to_owned());
                }
            }
            for child in children {
                let child_path = path.join(std::ffi::OsStr::from_bytes(child.to_bytes()));
                self.entry(&directory, &child, &child_path, depth + 1)?;
            }
            check_cancel(self.cancellation)?;
            unlinkat(parent, name, AtFlags::REMOVEDIR)?;
        } else {
            unlinkat(parent, name, AtFlags::empty())?;
            self.totals.bytes = self.totals.bytes.saturating_add(stat.st_size.max(0) as u64);
        }
        self.totals.entries += 1;
        (self.progress)(path, self.totals);
        Ok(())
    }
}

fn open_directory(
    parent: impl rustix::fd::AsFd,
    name: impl rustix::path::Arg,
) -> io::Result<OwnedFd> {
    Ok(openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
}

impl OperationEngine {
    /// Permanently delete the selected entries. Prefer the confirmed queue
    /// request; this public seam exists for headless tests and tools.
    pub fn delete_permanently(
        &self,
        sources: &[PathBuf],
        cancellation: &Cancellation,
    ) -> io::Result<DeleteOutcome> {
        self.delete_with(sources, None, cancellation, &mut |_, _| {})
    }

    pub(crate) fn delete_observed(
        &self,
        sources: &[PathBuf],
        observed: &super::transfer::ObservedSources,
        cancellation: &Cancellation,
        progress: &mut dyn FnMut(&Path, DeleteProgress),
    ) -> io::Result<DeleteOutcome> {
        self.delete_with(sources, Some(observed), cancellation, progress)
    }

    fn delete_with(
        &self,
        sources: &[PathBuf],
        observed: Option<&super::transfer::ObservedSources>,
        cancellation: &Cancellation,
        progress: &mut dyn FnMut(&Path, DeleteProgress),
    ) -> io::Result<DeleteOutcome> {
        let mut outcome = DeleteOutcome::default();
        let mut sources: std::collections::BTreeSet<_> = sources.iter().cloned().collect();
        // A selected folder already covers its selected descendants.
        let folders: Vec<_> = sources
            .iter()
            .filter(|path| fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()))
            .cloned()
            .collect();
        sources.retain(|source| {
            !folders
                .iter()
                .any(|folder| source != folder && source.starts_with(folder))
        });
        let mut pending: std::collections::VecDeque<_> = sources.into_iter().collect();
        check_cancel(cancellation)?;
        // Serialize with every other engine mutation and settle interrupted
        // journals first, exactly as Trash does.
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("operation history failed"))?;
        recover(&mut state, cancellation)?;
        drop(state);
        let mut removal = Removal::new(cancellation, progress);
        while let Some(source) = pending.front().cloned() {
            if cancellation.is_cancelled() {
                outcome.cancelled = true;
                break;
            }
            let result = (|| -> io::Result<()> {
                if let Some(observed) = observed {
                    observed.require(&source)?;
                }
                if !self
                    .capture_retained_sources(std::slice::from_ref(&source))?
                    .stages
                    .is_empty()
                {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "Recovery or an unfinished operation still needs data inside this item. Finish or dismiss that work first.",
                    ));
                }
                let state = self
                    .state
                    .lock()
                    .map_err(|_| io::Error::other("operation history failed"))?;
                let result = removal.remove(&source);
                drop(state);
                result
            })();
            match result {
                Ok(()) => outcome.completed.push(source),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                    outcome.cancelled = true;
                    if fs::symlink_metadata(&source).is_ok() {
                        outcome.failures.push((
                            source,
                            "Stopped; part of this folder may already be deleted.".into(),
                        ));
                    } else {
                        outcome.completed.push(source);
                    }
                    pending.pop_front();
                    break;
                }
                Err(error) => outcome.failures.push((source, error.to_string())),
            }
            pending.pop_front();
        }
        outcome.remaining = pending.into_iter().collect();
        let totals = removal.totals();
        outcome.entries = totals.entries;
        outcome.bytes = totals.bytes;
        Ok(outcome)
    }
}
