//! Folder traversal and publication use held directories. Persistent facts come
//! from those same objects, including the payload before its final publication.
use super::*;
use crate::operations::original::Original;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use std::{
    ffi::OsString,
    os::{fd::AsRawFd, unix::ffi::OsStringExt},
};
mod publication;
pub(in crate::operations) use publication::recover as recover_created;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FolderOriginal {
    entry: Original,
    parent: Original,
}
impl FolderOriginal {
    fn capture(file: &fs::File, parent: &fs::File) -> Option<Self> {
        Some(Self {
            entry: Original::capture(file).ok()?,
            parent: Original::capture(parent).ok()?,
        })
    }
    fn require(&self, folder: &Opened) -> io::Result<()> {
        self.entry.require(&folder.file)?;
        self.parent.require(&folder.parent)
    }
    /// [`Self::require`] for the folder at `path`, opened as `Opened::open`
    /// opens it.
    fn require_path(&self, path: &Path) -> io::Result<()> {
        if let Some(observed) = crate::operations::proofs::shared(path) {
            return observed.require_folder(&self.parent, &self.entry);
        }
        let (file, parent) = Opened::pins(path)?;
        self.entry.require(&file)?;
        self.parent.require(&parent)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::operations) struct DirectoryOriginal {
    source: FolderOriginal,
    destination: FolderOriginal,
}
impl DirectoryOriginal {
    pub(super) fn capture(source: &Opened, destination: Option<FolderOriginal>) -> Option<Self> {
        Some(Self {
            source: source.original()?,
            destination: destination?,
        })
    }
    pub(in crate::operations) fn require(
        &self,
        source: &Path,
        destination: &Path,
        absent: bool,
    ) -> io::Result<()> {
        // Only the handles are compared here, so the folders are opened
        // without the identity reads `Opened::open` also makes.
        if !absent {
            self.source.require_path(source)?;
        }
        self.destination.require_path(destination)
    }
    pub(in crate::operations) fn require_opened(
        &self,
        source: &Opened,
        destination: &Opened,
    ) -> io::Result<()> {
        self.source.require(source)?;
        self.destination.require(destination)
    }
    pub(in crate::operations) fn source_origins(&self) -> (Original, Original) {
        (self.source.parent, self.source.entry)
    }
    pub(in crate::operations) fn destination_origins(&self) -> (Original, Original) {
        (self.destination.parent, self.destination.entry)
    }
    pub(in crate::operations) fn require_source(&self, path: &Path) -> io::Result<()> {
        let opened = Opened::open(path)?;
        self.source.require(&opened)?;
        opened.require_current()
    }
}

pub(in crate::operations) struct Opened {
    file: fs::File,
    parent: fs::File,
    path: PathBuf,
    pub(super) identity: Identity,
    pub(super) parent_identity: Identity,
}
impl Opened {
    pub(in crate::operations) fn path(&self) -> &Path {
        &self.path
    }
    pub(in crate::operations) fn open(path: &Path) -> io::Result<Self> {
        let (file, parent) = Self::pins(path)?;
        Ok(Self {
            identity: Identity::from_metadata(&file.metadata()?),
            parent_identity: Identity::from_metadata(&parent.metadata()?),
            file,
            parent,
            path: path.to_owned(),
        })
    }
    /// The folder at `path` and its parent, as `O_PATH` descriptors.
    fn pins(path: &Path) -> io::Result<(fs::File, fs::File)> {
        let parent = directory(
            path.parent()
                .ok_or_else(|| invalid("A transfer folder needs a parent."))?,
        )?;
        let file = fs::File::from(rustix::fs::openat(
            &parent,
            path.file_name()
                .ok_or_else(|| invalid("A transfer folder needs a name."))?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Ok((file, parent))
    }
    pub(in crate::operations) fn metadata(&self) -> io::Result<fs::Metadata> {
        self.file.metadata()
    }
    pub(super) fn original(&self) -> Option<FolderOriginal> {
        FolderOriginal::capture(&self.file, &self.parent)
    }
    pub(in crate::operations) fn require_current(&self) -> io::Result<()> {
        if !self.identity.matches(&self.path)
            || !fs::metadata(self.path.parent().unwrap())
                .is_ok_and(|m| Identity::from_metadata(&m) == self.parent_identity)
        {
            return Err(changed());
        }
        Ok(())
    }
    pub(in crate::operations) fn entry(&self) -> &fs::File {
        &self.file
    }
    pub(in crate::operations) fn parent(&self) -> &fs::File {
        &self.parent
    }
    pub(super) fn children(&self) -> io::Result<Vec<OsString>> {
        let mut names = Vec::new();
        let readable = fs::File::from(rustix::fs::openat(
            &self.file,
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        for entry in rustix::fs::Dir::new(readable)? {
            let entry = entry?;
            let name = entry.file_name().to_bytes();
            if name != b"." && name != b".." {
                names.push(OsString::from_vec(name.to_vec()));
            }
        }
        names.sort();
        Ok(names)
    }
}

/// Keeps the real created payload and both publication directories alive.
/// The ordinary Forward journal continues to own interrupted cleanup.
pub(super) struct Created {
    parent: fs::File,
    stage: fs::File,
    payload: fs::File,
    destination: PathBuf,
    stage_name: OsString,
    parent_identity: Identity,
    stage_identity: Identity,
    pub(super) identity: Identity,
}
impl Created {
    pub(super) fn registration<'a>(
        &'a self,
        source: &'a Opened,
        record: &'a Stage,
    ) -> io::Result<staging::Registration<'a>> {
        self.require_current()?;
        Ok(staging::Registration::directory(
            source,
            record,
            &self.parent,
            &self.stage,
            &self.payload,
        ))
    }
    pub(super) fn prepare(
        destination: &Path,
        mode: u32,
    ) -> io::Result<(Self, Stage, tempfile::TempDir)> {
        let parent_path = destination
            .parent()
            .ok_or_else(|| invalid("A transfer folder needs a parent."))?;
        let parent = directory(parent_path)?;
        let parent_identity = Identity::from_metadata(&parent.metadata()?);
        // tempfile allocates the name under the actual held parent. Its path is
        // process-local; only the stable namespace label enters the journal.
        let mut temporary = tempfile::Builder::new()
            .prefix(".canopa-files-stage-")
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))?;
        temporary.disable_cleanup(true);
        let stage_name = temporary.path().file_name().unwrap().to_owned();
        let stage = fs::File::from(rustix::fs::openat(
            &parent,
            &stage_name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let stage_identity = Identity::from_metadata(&stage.metadata()?);
        rustix::fs::mkdirat(&stage, c"folder", Mode::RUSR | Mode::WUSR | Mode::XUSR)?;
        let payload = fs::File::from(rustix::fs::openat(
            &stage,
            c"folder",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        payload.set_permissions(fs::Permissions::from_mode((mode & 0o777) | 0o700))?;
        let identity = Identity::from_metadata(&payload.metadata()?);
        let record = Stage {
            replacement: None,
            entry_move: None,
            source_claim: None,
            directory_claim: None,
            ownership: staging::Ownership::capture(&parent, &stage)
                .map(|proof| Box::new(proof.with_payload(&payload))),
            parent_access: Some(StageParentAccess::Existing),
            directory: parent_path.join(&stage_name),
            identity: stage_identity,
            payload_identity: Some(identity),
            parent_identity: Some(parent_identity),
            sibling_payload: false,
        };
        Ok((
            Self {
                parent,
                stage,
                payload,
                destination: destination.to_owned(),
                stage_name,
                parent_identity,
                stage_identity,
                identity,
            },
            record,
            temporary,
        ))
    }
    pub(super) fn original(&self) -> Option<FolderOriginal> {
        FolderOriginal::capture(&self.payload, &self.parent)
    }
    pub(super) fn file(&self) -> &fs::File {
        &self.payload
    }
    pub(super) fn publish(&self) -> io::Result<()> {
        self.publish_checked(|| {})
    }
    fn publish_checked(&self, after_publication: impl FnOnce()) -> io::Result<()> {
        // An unverified private stage/payload is not ordinary disposable work,
        // including when refusal happens before publication.
        self.publish_attempt(after_publication).map_err(|error| {
            io::Error::new(
                error.kind(),
                PlacementFailure {
                    access: TransferAccess::DestinationFolder,
                    error,
                },
            )
        })
    }
    fn publish_attempt(&self, after_publication: impl FnOnce()) -> io::Result<()> {
        self.require_current()?;
        if at(&self.stage, c"folder")? != self.identity {
            return Err(changed());
        }
        rustix::fs::renameat_with(
            &self.stage,
            c"folder",
            &self.parent,
            self.destination.file_name().unwrap(),
            RenameFlags::NOREPLACE,
        )?;
        after_publication();
        if at(&self.parent, self.destination.file_name().unwrap())? != self.identity {
            return Err(changed());
        }
        self.require_current()
    }
    fn require_current(&self) -> io::Result<()> {
        if !fs::metadata(self.destination.parent().unwrap())
            .is_ok_and(|m| Identity::from_metadata(&m) == self.parent_identity)
            || at(&self.parent, &self.stage_name)? != self.stage_identity
        {
            return Err(changed());
        }
        Ok(())
    }
}
fn directory(path: &Path) -> io::Result<fs::File> {
    Ok(fs::File::from(rustix::fs::open(
        path,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?))
}
fn at(parent: &fs::File, name: impl rustix::path::Arg) -> io::Result<Identity> {
    let stat = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
    Ok(Identity {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}
fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The original transfer folder or its parent changed; existing contents are preserved.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumeration_keeps_the_opened_folder_and_original_unix_names_after_replacement() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source");
        fs::create_dir(&path).unwrap();
        let name = OsString::from_vec(b"original-\xff".to_vec());
        fs::write(path.join(&name), b"Original").unwrap();
        let opened = Opened::open(&path).unwrap();
        fs::rename(&path, root.path().join("original")).unwrap();
        fs::create_dir(&path).unwrap();
        fs::write(path.join("replacement"), b"Replacement").unwrap();
        assert_eq!(opened.children().unwrap(), vec![name]);
        assert_eq!(
            opened.require_current().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(fs::read(path.join("replacement")).unwrap(), b"Replacement");
    }

    #[test]
    fn created_folder_facts_precede_publication_and_follow_the_actual_payload() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("created");
        let (created, stage, temporary) = Created::prepare(&destination, 0o500).unwrap();
        let original = created
            .original()
            .expect("private test filesystem supports original facts");
        assert!(!destination.exists());
        original.entry.require(&created.payload).unwrap();
        original.parent.require(&created.parent).unwrap();
        created.publish().unwrap();
        let published = Opened::open(&destination).unwrap();
        original.require(&published).unwrap();
        assert_eq!(published.identity, created.identity);
        assert!(published.children().unwrap().is_empty());
        clean_stage(&stage).unwrap();
        drop(temporary);
    }

    #[test]
    fn created_folder_publication_refuses_a_replaced_parent_and_preserves_both_namespaces() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("destination");
        let parked = root.path().join("original");
        fs::create_dir(&parent).unwrap();
        let target = parent.join("created");
        let (created, stage, temporary) = Created::prepare(&target, 0o700).unwrap();
        fs::rename(&parent, &parked).unwrap();
        fs::create_dir(&parent).unwrap();
        fs::write(parent.join("sentinel"), b"Replacement parent").unwrap();
        assert_eq!(
            created.publish().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(!target.exists() && !parked.join("created").exists());
        assert!(
            parked
                .join(stage.directory.file_name().unwrap())
                .join("folder")
                .is_dir()
        );
        assert_eq!(
            fs::read(parent.join("sentinel")).unwrap(),
            b"Replacement parent"
        );
        drop(temporary);
    }

    #[test]
    fn prepublication_refusal_keeps_foreign_payloads_and_the_actual_created_descriptor() {
        for symlink in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let target = root.path().join("created");
            let parked = root.path().join("original payload");
            let protected = root.path().join("protected foreign file");
            fs::write(&protected, b"Protected referent").unwrap();
            fs::set_permissions(&protected, fs::Permissions::from_mode(0o600)).unwrap();
            let engine = OperationEngine::open(root.path().join("history")).unwrap();
            let (created, stage, temporary) = Created::prepare(&target, 0o500).unwrap();
            fs::rename(stage.payload(), &parked).unwrap();
            if symlink {
                std::os::unix::fs::symlink(&protected, stage.payload()).unwrap();
            } else {
                fs::write(stage.payload(), b"Foreign staged file").unwrap();
            }
            let foreign = stage.payload();
            let mut state = engine.state.lock().unwrap();
            let id = state.begin("private prepublication boundary").unwrap();
            let change = Change::CreatedFolder {
                access_parent: None,
                mode: Some(0o500),
                path: target.clone(),
                identity: created.identity,
            };
            let error = commit_forward_with_created(
                &mut state,
                id,
                change,
                Some((stage, temporary)),
                Some(created.file()),
                |_| created.publish(),
            )
            .unwrap_err();
            assert_eq!(
                placement_failure(&error),
                Some(TransferAccess::DestinationFolder)
            );
            assert!(matches!(
                state.pending().unwrap(),
                Some(Pending::Forward { .. })
            ));
            assert!(parked.is_dir() && !target.exists());
            if symlink {
                assert!(fs::symlink_metadata(&foreign).unwrap().is_symlink());
            } else {
                assert_eq!(fs::read(foreign).unwrap(), b"Foreign staged file");
            }
            assert_eq!(fs::read(&protected).unwrap(), b"Protected referent");
            assert_eq!(fs::metadata(protected).unwrap().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn a_postpublication_refusal_keeps_the_journal_and_all_foreign_namespaces() {
        for replaced in ["target", "parent", "stage"] {
            let root = tempfile::tempdir().unwrap();
            let parent = root.path().join("destination");
            fs::create_dir(&parent).unwrap();
            let target = parent.join("created");
            let parked = root.path().join("parked-original");
            let engine = OperationEngine::open(root.path().join("history")).unwrap();
            let (created, stage, temporary) = Created::prepare(&target, 0o700).unwrap();
            let stage_path = stage.directory.clone();
            let sentinel = match replaced {
                "target" => target.join("sentinel"),
                "parent" => parent.join("sentinel"),
                _ => stage_path.join("sentinel"),
            };
            let mut state = engine.state.lock().unwrap();
            let id = state.begin("private folder-publication boundary").unwrap();
            let change = Change::CreatedFolder {
                access_parent: None,
                mode: Some(0o700),
                path: target.clone(),
                identity: created.identity,
            };
            let error = commit_forward_with_created(
                &mut state,
                id,
                change,
                Some((stage, temporary)),
                Some(created.file()),
                |_| {
                    created.publish_checked(|| {
                        let path = match replaced {
                            "target" => &target,
                            "parent" => &parent,
                            _ => &stage_path,
                        };
                        fs::rename(path, &parked).unwrap();
                        fs::create_dir(path).unwrap();
                        fs::write(&sentinel, b"Foreign work after publication").unwrap();
                    })
                },
            )
            .unwrap_err();
            assert_eq!(
                placement_failure(&error),
                Some(TransferAccess::DestinationFolder),
                "{replaced}"
            );
            assert!(
                matches!(state.pending().unwrap(), Some(Pending::Forward { .. })),
                "{replaced}: unverified publication lost its real journal"
            );
            assert_eq!(
                fs::read(sentinel).unwrap(),
                b"Foreign work after publication"
            );
            assert!(parked.is_dir());
        }
    }

    #[test]
    fn created_folder_publication_refuses_a_replaced_staged_payload_without_deleting_it() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("created");
        let (created, stage, temporary) = Created::prepare(&target, 0o700).unwrap();
        let original = root.path().join("original payload");
        fs::rename(stage.payload(), &original).unwrap();
        fs::create_dir(stage.payload()).unwrap();
        fs::write(stage.payload().join("sentinel"), b"Replacement payload").unwrap();
        assert_eq!(
            created.publish().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(!target.exists() && original.is_dir());
        assert_eq!(
            fs::read(stage.payload().join("sentinel")).unwrap(),
            b"Replacement payload"
        );
        drop(temporary);
    }
}
