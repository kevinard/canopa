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
    /// Canonical spellings observed at the last reconciliation, per target.
    resolved: Vec<Option<PathBuf>>,
    watches: BTreeMap<PathBuf, Watch>,
    drained: bool,
    limited: bool,
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
            resolved: Vec::new(),
            watches: BTreeMap::new(),
            drained: true,
            limited: false,
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
        self.resolved.clear();
        for target in &self.targets {
            let (path, directory) = match target {
                Target::File(p) => (p, false),
                Target::Directory(p) => (p, true),
            };
            add(&mut desired, path, directory);
            let canonical = path
                .canonicalize()
                .ok()
                .filter(|canonical| canonical != path);
            if let Some(canonical) = &canonical {
                add(&mut desired, canonical, directory);
            }
            self.resolved.push(canonical);
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
        self.limited = false;
        for (path, names) in desired {
            if let Some(watch) = self.watches.get_mut(&path) {
                watch.names = names;
            } else {
                match inotify::add_watch(&self.fd, &path, flags) {
                    Ok(wd) => {
                        self.watches.insert(path, Watch { wd, names });
                    }
                    // The per-user watch budget is exhausted. Other failures,
                    // such as a path vanishing meanwhile, are ordinary races.
                    Err(rustix::io::Errno::NOSPC | rustix::io::Errno::NOMEM) => {
                        self.limited = true;
                    }
                    Err(_) => {}
                }
            }
        }
    }
    /// The last reconciliation could not install every watch because the
    /// kernel's inotify watch budget is exhausted. Unwatched targets report
    /// no changes; callers keep a manual refresh available.
    pub fn limited(&self) -> bool {
        self.limited
    }
    /// Drain/coalesce an event burst. Callers rescan/admit candidates transactionally.
    pub fn changed(&mut self) -> io::Result<bool> {
        Ok(self.drain()?.0)
    }
    /// Like `changed`, but names the targets an event burst may concern. A
    /// queue overflow or an undrained burst conservatively reports every target.
    pub fn changed_targets(&mut self) -> io::Result<Vec<Target>> {
        let (changed, mut indices) = self.drain()?;
        if changed && indices.is_empty() {
            indices = (0..self.targets.len()).collect();
        }
        Ok(indices
            .into_iter()
            .filter_map(|index| self.targets.get(index).cloned())
            .collect())
    }
    fn affected(&self, watched: &Path, name: Option<&OsStr>, retired: bool) -> BTreeSet<usize> {
        let mut indices = BTreeSet::new();
        for (index, target) in self.targets.iter().enumerate() {
            let (Target::File(path) | Target::Directory(path)) = target;
            let directory = matches!(target, Target::Directory(_));
            let concerns = |path: &Path| {
                (directory && path == watched)
                    || (retired && path.starts_with(watched))
                    || match name {
                        Some(name) => path.starts_with(watched.join(name)),
                        None => path.starts_with(watched),
                    }
            };
            if concerns(path) || self.resolved[index].as_deref().is_some_and(concerns) {
                indices.insert(index);
            }
        }
        indices
    }
    fn drain(&mut self) -> io::Result<(bool, BTreeSet<usize>)> {
        self.drained = false;
        let mut affected = BTreeSet::new();
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
            let Some((watched, watch)) = self.watches.iter().find(|(_, w)| w.wd == event.wd())
            else {
                continue;
            };
            let name = event
                .file_name()
                .map(|name| OsStr::from_bytes(name.to_bytes()));
            let retiring = event
                .events()
                .intersects(ReadFlags::IGNORED | ReadFlags::DELETE_SELF | ReadFlags::MOVE_SELF);
            if retiring {
                retired.insert(event.wd());
                changed = true;
            }
            let admitted = name.is_none_or(|name| {
                watch
                    .names
                    .as_ref()
                    .is_none_or(|names| names.contains(name))
            });
            if admitted {
                changed = true;
            }
            if admitted || retiring {
                affected.extend(self.affected(watched, name.filter(|_| !retiring), retiring));
            }
        }
        if overflow || !self.drained {
            // A lost invalidation may have retired an inode while its pathname
            // still exists. Rebuild instead of trusting cached watch descriptors.
            for (_, watch) in std::mem::take(&mut self.watches) {
                let _ = inotify::remove_watch(&self.fd, watch.wd);
            }
            changed = true;
            affected.extend(0..self.targets.len());
        }
        for wd in retired {
            self.watches.retain(|_, watch| watch.wd != wd);
            let _ = inotify::remove_watch(&self.fd, wd);
        }
        if changed {
            self.reconcile();
        }
        Ok((changed, affected))
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

    #[test]
    fn changed_targets_name_only_the_directories_an_event_concerns() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let mut watcher = Watcher::new(vec![
            Target::Directory(first.clone()),
            Target::Directory(second.clone()),
        ])
        .unwrap();
        assert!(!watcher.limited());
        assert!(watcher.changed_targets().unwrap().is_empty());
        std::fs::write(first.join("download"), b"x").unwrap();
        assert_eq!(
            watcher.changed_targets().unwrap(),
            vec![Target::Directory(first.clone())]
        );
        std::fs::write(root.path().join("sibling"), b"x").unwrap();
        assert!(watcher.changed_targets().unwrap().is_empty());
        std::fs::remove_dir_all(&second).unwrap();
        assert_eq!(
            watcher.changed_targets().unwrap(),
            vec![Target::Directory(second.clone())]
        );
        std::fs::create_dir(&second).unwrap();
        assert_eq!(
            watcher.changed_targets().unwrap(),
            vec![Target::Directory(second.clone())]
        );
        std::fs::write(second.join("saved"), b"x").unwrap();
        assert_eq!(
            watcher.changed_targets().unwrap(),
            vec![Target::Directory(second)]
        );
    }
}
