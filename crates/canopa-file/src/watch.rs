//! Event-driven local path watching, including atomic replacement and missing parents.
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd},
        unix::ffi::OsStrExt,
    },
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    File(PathBuf),
    Directory(PathBuf),
}
struct Watch {
    wd: i32,
    names: Option<BTreeSet<OsString>>,
}
pub struct Watcher {
    fd: OwnedFd,
    targets: Vec<Target>,
    watches: BTreeMap<PathBuf, Watch>,
    drained: bool,
}
impl AsFd for Watcher {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}
impl AsRawFd for Watcher {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}
impl Watcher {
    pub fn new(targets: Vec<Target>) -> io::Result<Self> {
        let mut watcher = Self {
            fd: inotify::init(CreateFlags::NONBLOCK | CreateFlags::CLOEXEC)?,
            targets: Vec::new(),
            watches: BTreeMap::new(),
            drained: true,
        };
        watcher.set_targets(targets)?;
        Ok(watcher)
    }
    pub fn set_targets(&mut self, targets: Vec<Target>) -> io::Result<()> {
        if targets.len() > 256
            || targets.iter().any(|t| {
                let (Target::File(p) | Target::Directory(p)) = t;
                !p.is_absolute() || p.as_os_str().len() > 4096
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "watch targets exceed budget or are not absolute",
            ));
        }
        self.targets = targets;
        self.reconcile();
        Ok(())
    }
    fn reconcile(&mut self) {
        let mut desired: BTreeMap<PathBuf, Option<BTreeSet<OsString>>> = BTreeMap::new();
        for target in &self.targets {
            let (path, directory) = match target {
                Target::File(p) => (p, false),
                Target::Directory(p) => (p, true),
            };
            add(&mut desired, path, directory);
            if let Ok(canonical) = path.canonicalize()
                && canonical != *path
            {
                add(&mut desired, &canonical, directory);
            }
        }
        self.watches.retain(|path, watch| {
            if desired.contains_key(path) {
                true
            } else {
                let _ = inotify::remove_watch(&self.fd, watch.wd);
                false
            }
        });
        let flags = WatchFlags::CLOSE_WRITE
            | WatchFlags::MOVED_TO
            | WatchFlags::MOVED_FROM
            | WatchFlags::CREATE
            | WatchFlags::DELETE
            | WatchFlags::ATTRIB
            | WatchFlags::DELETE_SELF
            | WatchFlags::MOVE_SELF
            | WatchFlags::ONLYDIR;
        for (path, names) in desired {
            if let Some(watch) = self.watches.get_mut(&path) {
                watch.names = names;
            } else if let Ok(wd) = inotify::add_watch(&self.fd, &path, flags) {
                self.watches.insert(path, Watch { wd, names });
            }
        }
    }
    /// Drain/coalesce an event burst. Callers rescan/admit candidates transactionally.
    pub fn changed(&mut self) -> io::Result<bool> {
        self.drained = false;
        let mut bytes = [MaybeUninit::uninit(); 65536];
        let mut reader = inotify::Reader::new(&self.fd, &mut bytes);
        let mut changed = false;
        let mut overflow = false;
        let mut retired = BTreeSet::new();
        for _ in 0..4096 {
            let event = match reader.next() {
                Ok(e) => e,
                Err(rustix::io::Errno::AGAIN) => {
                    self.drained = true;
                    break;
                }
                Err(error) => return Err(error.into()),
            };
            if event.events().contains(ReadFlags::QUEUE_OVERFLOW) {
                changed = true;
                overflow = true;
                continue;
            }
            let Some(watch) = self.watches.values().find(|w| w.wd == event.wd()) else {
                continue;
            };
            if event
                .events()
                .intersects(ReadFlags::IGNORED | ReadFlags::DELETE_SELF | ReadFlags::MOVE_SELF)
            {
                retired.insert(event.wd());
                changed = true;
            }
            if event.file_name().is_none_or(|name| {
                watch
                    .names
                    .as_ref()
                    .is_none_or(|names| names.contains(OsStr::from_bytes(name.to_bytes())))
            }) {
                changed = true;
            }
        }
        if overflow || !self.drained {
            // A lost invalidation may have retired an inode while its pathname
            // still exists. Rebuild instead of trusting cached watch descriptors.
            for (_, watch) in std::mem::take(&mut self.watches) {
                let _ = inotify::remove_watch(&self.fd, watch.wd);
            }
            changed = true;
        }
        for wd in retired {
            self.watches.retain(|_, watch| watch.wd != wd);
            let _ = inotify::remove_watch(&self.fd, wd);
        }
        if changed {
            self.reconcile();
        }
        Ok(changed)
    }
    /// Retain async readiness when a burst exceeded the per-dispatch budget.
    pub fn drained(&self) -> bool {
        self.drained
    }
}
fn add(desired: &mut BTreeMap<PathBuf, Option<BTreeSet<OsString>>>, path: &Path, directory: bool) {
    if directory && path.is_dir() {
        desired.insert(path.to_path_buf(), None);
    }
    let Some(mut parent) = path.parent() else {
        return;
    };
    let mut name = path.file_name();
    while !parent.is_dir() {
        name = parent.file_name();
        let Some(next) = parent.parent() else {
            return;
        };
        parent = next;
    }
    let Some(name) = name else {
        return;
    };
    desired
        .entry(parent.to_path_buf())
        .and_modify(|names| {
            if let Some(names) = names {
                names.insert(name.to_os_string());
            }
        })
        .or_insert_with(|| Some(BTreeSet::from([name.to_os_string()])));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_replacement_missing_parents_and_sibling_filter_are_event_driven() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("nested/image");
        let mut watcher = Watcher::new(vec![Target::File(path.clone())]).unwrap();
        std::fs::write(root.path().join("unrelated"), b"x").unwrap();
        assert!(!watcher.changed().unwrap());
        std::fs::create_dir(path.parent().unwrap()).unwrap();
        assert!(watcher.changed().unwrap());
        std::fs::write(&path, b"x").unwrap();
        assert!(watcher.changed().unwrap());
        assert!(!watcher.changed().unwrap());
        let temporary = path.with_extension("new");
        std::fs::write(&temporary, b"new").unwrap();
        std::fs::rename(temporary, &path).unwrap();
        assert!(watcher.changed().unwrap());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
        assert!(watcher.changed().unwrap());
        std::fs::create_dir(path.parent().unwrap()).unwrap();
        assert!(watcher.changed().unwrap());
        std::fs::write(&path, b"again").unwrap();
        assert!(watcher.changed().unwrap());
    }
}
