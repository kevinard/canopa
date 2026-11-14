//! One checkpoint's observations, shared by its checks (ADR0179).
//!
//! A transfer checkpoint only reads. Within one call it proves the selected
//! source folder up to three times (the observed selection, the recorded folder
//! original and its identity) and the destination folder twice, each time
//! opening and capturing the same objects again. Inside [`scope`], the first
//! request for a path opens its parent and entry and captures them once; later
//! requests in the same call compare against those observations. Outside a
//! scope every request observes afresh, exactly as before.
use super::*;
use original::Original;
use rustix::fs::{AtFlags, Mode, OFlags, StatxFlags};
use std::{
    cell::{OnceCell, RefCell},
    collections::HashMap,
    rc::Rc,
};

thread_local! {
    static SCOPE: RefCell<Option<HashMap<PathBuf, Rc<Observed>>>> = const { RefCell::new(None) };
}

/// Run `check` sharing observations of the same paths. A nested call joins the
/// outer scope.
pub(super) fn scope<T>(check: impl FnOnce() -> T) -> T {
    struct End;
    impl Drop for End {
        fn drop(&mut self) {
            SCOPE.with(|scope| scope.borrow_mut().take());
        }
    }
    let outer = SCOPE.with(|scope| {
        let mut scope = scope.borrow_mut();
        if scope.is_some() {
            return false;
        }
        *scope = Some(HashMap::new());
        true
    });
    let _end = outer.then(|| End);
    check()
}

/// The observations of `path`: shared inside a scope, fresh outside one.
pub(super) fn observed(path: &Path) -> Rc<Observed> {
    SCOPE.with(|scope| match scope.borrow_mut().as_mut() {
        Some(known) => known
            .entry(path.to_owned())
            .or_insert_with(|| Rc::new(Observed::new(path)))
            .clone(),
        None => Rc::new(Observed::new(path)),
    })
}

fn in_scope() -> bool {
    SCOPE.with(|scope| scope.borrow().is_some())
}

/// `lstat`'s identity and folder type of `path`, shared inside a scope.
pub(super) fn status(path: &Path) -> io::Result<(Identity, bool)> {
    if in_scope() {
        return observed(path).status();
    }
    let metadata = fs::symlink_metadata(path)?;
    Ok((Identity::from_metadata(&metadata), metadata.is_dir()))
}

/// Inside a scope, the shared observations of `path`; outside, `None` so the
/// caller observes afresh in its own way.
pub(super) fn shared(path: &Path) -> Option<Rc<Observed>> {
    in_scope().then(|| observed(path))
}

/// An error kept for every later request of the same observation.
#[derive(Clone)]
struct Kept {
    kind: io::ErrorKind,
    os: Option<i32>,
    message: String,
}
impl Kept {
    fn of(error: &io::Error) -> Self {
        Self {
            kind: error.kind(),
            os: error.raw_os_error(),
            message: error.to_string(),
        }
    }
    fn error(&self) -> io::Error {
        match self.os {
            Some(code) => io::Error::from_raw_os_error(code),
            None => io::Error::new(self.kind, self.message.clone()),
        }
    }
}
type Result<T> = std::result::Result<T, Kept>;

fn kept<T>(result: io::Result<T>) -> Result<T> {
    result.map_err(|error| Kept::of(&error))
}

pub(super) struct Observed {
    path: PathBuf,
    parent: OnceCell<Result<fs::File>>,
    entry: OnceCell<Result<fs::File>>,
    status: OnceCell<Result<(Identity, bool)>>,
    entry_original: OnceCell<Result<Original>>,
    parent_original: OnceCell<Result<Original>>,
}

impl Observed {
    fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            parent: OnceCell::new(),
            entry: OnceCell::new(),
            status: OnceCell::new(),
            entry_original: OnceCell::new(),
            parent_original: OnceCell::new(),
        }
    }

    /// The parent folder, following links as path resolution does.
    fn parent(&self) -> io::Result<&fs::File> {
        self.parent
            .get_or_init(|| {
                kept((|| {
                    let parent = self
                        .path
                        .parent()
                        .ok_or_else(|| invalid("An observed entry needs its parent."))?;
                    Ok(fs::File::from(rustix::fs::open(
                        parent,
                        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                        Mode::empty(),
                    )?))
                })())
            })
            .as_ref()
            .map_err(Kept::error)
    }

    /// The entry itself, a final link not followed.
    fn entry(&self) -> io::Result<&fs::File> {
        self.entry
            .get_or_init(|| {
                kept((|| {
                    let name = self
                        .path
                        .file_name()
                        .ok_or_else(|| invalid("An observed entry needs its name."))?;
                    Ok(fs::File::from(rustix::fs::openat(
                        self.parent()?,
                        name,
                        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )?))
                })())
            })
            .as_ref()
            .map_err(Kept::error)
    }

    /// The entry's identity and whether it is a folder, as `lstat` reports them.
    pub(super) fn status(&self) -> io::Result<(Identity, bool)> {
        self.status
            .get_or_init(|| {
                kept((|| {
                    let status = rustix::fs::statx(
                        self.entry()?,
                        c"",
                        AtFlags::EMPTY_PATH | AtFlags::SYMLINK_NOFOLLOW,
                        StatxFlags::TYPE | StatxFlags::INO,
                    )?;
                    let identity = Identity {
                        device: rustix::fs::makedev(status.stx_dev_major, status.stx_dev_minor),
                        inode: status.stx_ino,
                    };
                    let directory = u32::from(status.stx_mode) & libc_type_mask() == DIRECTORY;
                    Ok((identity, directory))
                })())
            })
            .clone()
            .map_err(|kept| kept.error())
    }

    /// The entry's persistent identity proof.
    fn entry_original(&self) -> io::Result<Original> {
        self.entry_original
            .get_or_init(|| kept(self.entry().and_then(Original::capture)))
            .clone()
            .map_err(|kept| kept.error())
    }

    /// The parent's persistent identity proof.
    fn parent_original(&self) -> io::Result<Original> {
        self.parent_original
            .get_or_init(|| kept(self.parent().and_then(Original::capture)))
            .clone()
            .map_err(|kept| kept.error())
    }

    /// [`Original::require`] of the parent and then of the entry, as a selected
    /// entry's name is proved.
    pub(super) fn require_name(&self, parent: &Original, entry: &Original) -> io::Result<()> {
        parent.require_equal(&self.parent_original()?)?;
        entry.require_equal(&self.entry_original()?)
    }

    /// A recorded folder: the entry opened as a folder must be the recorded
    /// entry, then its parent the recorded parent.
    pub(super) fn require_folder(&self, parent: &Original, entry: &Original) -> io::Result<()> {
        self.entry()?;
        if !self.status()?.1 {
            return Err(io::Error::from_raw_os_error(
                rustix::io::Errno::NOTDIR.raw_os_error(),
            ));
        }
        entry.require_equal(&self.entry_original()?)?;
        parent.require_equal(&self.parent_original()?)
    }
}

const DIRECTORY: u32 = 0o040000;
const fn libc_type_mask() -> u32 {
    0o170000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scope_observes_each_path_once_and_outside_observes_afresh() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("folder");
        fs::create_dir(&folder).unwrap();
        let first = observed(&folder);
        let (identity, directory) = first.status().unwrap();
        assert!(directory);
        assert_eq!(identity, Identity::read(&folder).unwrap());
        // Outside a scope, a replacement is seen at once.
        fs::rename(&folder, root.path().join("moved")).unwrap();
        fs::create_dir(&folder).unwrap();
        assert_ne!(observed(&folder).status().unwrap().0, identity);
        scope(|| {
            let shared = observed(&folder).status().unwrap().0;
            fs::remove_dir(&folder).unwrap();
            // The same check sees its own first observation…
            assert_eq!(observed(&folder).status().unwrap().0, shared);
            // …and a nested scope joins it.
            scope(|| assert_eq!(observed(&folder).status().unwrap().0, shared));
        });
        // The scope ended: the missing folder is reported again.
        assert_eq!(
            observed(&folder).status().unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn recorded_folders_and_names_are_proved_like_their_descriptors() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("folder");
        fs::create_dir(&folder).unwrap();
        let file = root.path().join("file");
        fs::write(&file, b"x").unwrap();
        let open = |path: &Path| {
            fs::File::from(
                rustix::fs::open(
                    path,
                    OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .unwrap(),
            )
        };
        let Ok(parent) = Original::capture(&open(root.path())) else {
            eprintln!("skipping: no persistent identities here");
            return;
        };
        let entry = Original::capture(&open(&folder)).unwrap();
        let other = Original::capture(&open(&file)).unwrap();
        observed(&folder).require_folder(&parent, &entry).unwrap();
        observed(&folder).require_name(&parent, &entry).unwrap();
        assert_eq!(
            observed(&folder)
                .require_folder(&parent, &other)
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(
            observed(&file)
                .require_folder(&parent, &other)
                .unwrap_err()
                .raw_os_error(),
            Some(rustix::io::Errno::NOTDIR.raw_os_error())
        );
        observed(&file).require_name(&parent, &other).unwrap();
    }
}
