//! A private, durable work directory owns the staged document and any displaced
//! entry. It is never recursively removed, including on Drop or after a crash.
use super::{Identity, Pinned, Selection, Version, changed, invalid};
mod handoff;
mod recovery;
pub use handoff::{Preparation, Prepared};
pub use recovery::{RecoveryBody, RecoveryItem, RecoveryProtection, RecoveryStatus, RecoveryStore};
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, openat};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Write},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStringExt,
            fs::{FileExt, MetadataExt},
        },
    },
    path::PathBuf,
};

const DOCUMENT: &str = "document";
const PREVIOUS: &str = "previous";
const RECORD: &str = "record.jsonl";
const PAYLOAD: &str = "payload.json";
const RECORD_LIMIT: u64 = 64 * 1024;

/// Like Selection, this is evidence for inherited descriptors, not authority to
/// reopen a destination or replay a transaction after the backend has died.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationInfo {
    selection: Selection,
    directory_name: Vec<u8>,
    directory: Identity,
    document: Identity,
    record: Identity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Ready,
    Claiming,
    Claimed,
    Publishing,
    Published,
    Restoring,
    Restored,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    phase: Phase,
    claimed: Option<Version>,
    #[serde(default)]
    payload: Option<Identity>,
}

/// A publication can be known to have committed even when its worker reply was
/// lost. Uncertain publication is never retried or rolled back automatically.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disposition {
    pub published: bool,
    pub uncertain: bool,
    pub recovery: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct Publication {
    pin: Pinned,
    info: PublicationInfo,
    directory: File,
    document: File,
    record: File,
}

impl Publication {
    pub fn prepare(pin: Pinned) -> io::Result<Self> {
        let preparation = Preparation::new(&pin)?;
        Self::prepare_with(pin, preparation)
    }
    fn prepare_with(pin: Pinned, preparation: Preparation) -> io::Result<Self> {
        if pin.selection != preparation.selection {
            return Err(invalid());
        }
        pin.verify()?;
        let name = work_name(&preparation.directory_name)?;
        rustix::fs::mkdirat(&pin.parent, &name, Mode::RWXU)?;
        // A failed setup leaves only this private directory. Never remove a
        // tree by path: a replacement or foreign child must survive cleanup.
        let directory = File::from(openat(
            &pin.parent,
            &name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let document = fresh(&directory, DOCUMENT)?;
        let mut record = fresh(&directory, RECORD)?;
        let info = PublicationInfo {
            selection: pin.selection.clone(),
            directory_name: preparation.directory_name,
            directory: Identity::of(&directory.metadata()?),
            document: Identity::of(&document.metadata()?),
            record: Identity::of(&record.metadata()?),
        };
        serde_json::to_writer(&mut record, &info)?;
        record.write_all(b"\n")?;
        let result = Self {
            pin,
            info,
            directory,
            document,
            record,
        };
        result.append(State {
            phase: Phase::Ready,
            claimed: None,
            payload: None,
        })?;
        result.sync_directories()?;
        if let Err(error) = result.pin.verify() {
            if result
                .settle(false)
                .is_ok_and(|disposition| disposition.recovery.is_none())
            {
                return Err(error);
            }
            return Err(io::Error::other(format!(
                "{error} Recovery retained at {:?}",
                OsString::from_vec(result.recovery())
            )));
        }
        Ok(result)
    }

    pub fn info(&self) -> &PublicationInfo {
        &self.info
    }
    /// Ordered descriptors for the private worker; the optional original entry
    /// stays pinned in both processes until all helpers and cleanup have exited.
    pub fn files(&self) -> Vec<&File> {
        let mut files = vec![&self.pin.parent];
        files.extend(self.pin.entry.as_ref());
        files.extend([&self.directory, &self.document, &self.record]);
        files
    }
    pub fn document(&self) -> &File {
        &self.document
    }

    pub fn inherited(info: PublicationInfo, files: Vec<File>) -> io::Result<Self> {
        if files.len() != 4 + usize::from(info.selection.replaces()) {
            return Err(invalid());
        }
        let mut files = files.into_iter();
        let parent = files.next().unwrap();
        let entry = info.selection.replaces().then(|| files.next().unwrap());
        let pin = Pinned {
            selection: info.selection.clone(),
            parent,
            entry,
        };
        let result = Self {
            pin,
            directory: access(
                files.next().unwrap(),
                info.directory,
                OFlags::RDONLY | OFlags::DIRECTORY,
            )?,
            document: access(files.next().unwrap(), info.document, OFlags::RDWR)?,
            record: access(files.next().unwrap(), info.record, OFlags::RDWR)?,
            info,
        };
        result.verify_work(false)?;
        Ok(result)
    }

    fn directory_name(&self) -> io::Result<OsString> {
        work_name(&self.info.directory_name)
    }

    fn verify_work(&self, complete: bool) -> io::Result<()> {
        let name = self.directory_name()?;
        let directory = self.directory.metadata()?;
        if !directory.is_dir()
            || directory.mode() & 0o777 != 0o700
            || Identity::of(&directory) != self.info.directory
            || Identity::of(&self.pin.parent.metadata()?) != self.info.selection.parent
            || !super::path_only(&self.pin.parent)?
            || self.info.document == self.info.record
        {
            return Err(changed());
        }
        if complete || directory.nlink() != 0 {
            let named = File::from(openat(
                &self.pin.parent,
                &name,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            if Identity::of(&named.metadata()?) != self.info.directory {
                return Err(changed());
            }
        }
        if let Some(entry) = &self.pin.entry
            && (!super::path_only(entry)?
                || !entry.metadata()?.is_file()
                || Some(Identity::of(&entry.metadata()?))
                    != self.pin.selection.existing.map(|version| version.identity))
        {
            return Err(changed());
        }
        for (name, file, identity) in [
            (DOCUMENT, &self.document, self.info.document),
            (RECORD, &self.record, self.info.record),
        ] {
            let current = any(&self.directory, OsStr::new(name))?;
            if !file.metadata()?.is_file()
                || Identity::of(&file.metadata()?) != identity
                || current.as_ref().is_some_and(|file| {
                    file.metadata()
                        .map_or(true, |metadata| Identity::of(&metadata) != identity)
                })
                || (complete && current.is_none())
            {
                return Err(changed());
            }
        }
        for file in [&self.document, &self.record] {
            let flags = rustix::fs::fcntl_getfl(file)?;
            if flags.contains(OFlags::APPEND)
                || flags.intersects(OFlags::PATH)
                || flags & OFlags::ACCMODE != OFlags::RDWR
            {
                return Err(invalid());
            }
        }
        Ok(())
    }

    fn parent_current(&self) -> io::Result<()> {
        let (path, _) = self.pin.selection.location()?;
        let current = File::from(rustix::fs::open(
            path,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        if Identity::of(&current.metadata()?) != self.info.selection.parent {
            return Err(changed());
        }
        Ok(())
    }

    fn append(&self, state: State) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(&state)?;
        bytes.push(b'\n');
        let offset = self.record.metadata()?.len();
        if offset + bytes.len() as u64 > RECORD_LIMIT {
            return Err(invalid());
        }
        self.record.write_all_at(&bytes, offset)?;
        self.record.sync_all()
    }
    fn state(&self) -> io::Result<State> {
        read_state(&self.record, &self.info)
    }
    fn sync_directories(&self) -> io::Result<()> {
        self.directory.sync_all()?;
        File::from(openat(
            &self.pin.parent,
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?)
        .sync_all()
    }

    /// The staged bytes must already be complete. Copying never opens the old
    /// entry for writing, and the only publication syscall links this exact FD.
    pub fn commit(&self, mut check_cancel: impl FnMut() -> io::Result<()>) -> io::Result<()> {
        self.commit_at(&mut check_cancel, &mut |_| Ok(()))
    }
    pub(crate) fn commit_recovery(
        &self,
        mut check_cancel: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        self.commit_body(true, &mut check_cancel, &mut |_| Ok(()))
    }
    fn commit_at(
        &self,
        check_cancel: &mut impl FnMut() -> io::Result<()>,
        boundary: &mut impl FnMut(Phase) -> io::Result<()>,
    ) -> io::Result<()> {
        self.commit_body(false, check_cancel, boundary)
    }
    fn commit_body(
        &self,
        allow_empty: bool,
        check_cancel: &mut impl FnMut() -> io::Result<()>,
        boundary: &mut impl FnMut(Phase) -> io::Result<()>,
    ) -> io::Result<()> {
        self.verify_work(true)?;
        if self.state()?.phase != Phase::Ready
            || (!allow_empty && self.document.metadata()?.len() == 0)
        {
            return Err(invalid());
        }
        self.document.sync_all()?;
        let payload =
            recovery::complete(&self.directory, &self.document, allow_empty, check_cancel)?;
        self.append(State {
            phase: Phase::Ready,
            claimed: None,
            payload: Some(payload),
        })?;
        check_cancel()?;
        self.pin.verify()?;
        let (_, target) = self.pin.selection.location()?;
        let mut state = State {
            phase: Phase::Ready,
            claimed: None,
            payload: Some(payload),
        };
        if self.pin.selection.replaces() {
            state.phase = Phase::Claiming;
            self.append(state)?;
            boundary(Phase::Claiming)?;
            rustix::fs::renameat_with(
                &self.pin.parent,
                &target,
                &self.directory,
                PREVIOUS,
                RenameFlags::NOREPLACE,
            )?;
            self.sync_directories()?;
            let actual = any(&self.directory, OsStr::new(PREVIOUS))?.ok_or_else(changed)?;
            state = State {
                phase: Phase::Claimed,
                claimed: Some(Version::of(&actual.metadata()?)),
                payload: state.payload,
            };
            self.append(state)?;
            boundary(Phase::Claimed)?;
            if !self.original(&actual)? {
                return Err(changed());
            }
        }
        check_cancel()?;
        self.parent_current()?;
        self.verify_work(true)?;
        state.phase = Phase::Publishing;
        self.append(state)?;
        boundary(Phase::Publishing)?;
        // /proc/self/fd with SYMLINK_FOLLOW links the held inode without the
        // capability required by AT_EMPTY_PATH. linkat never replaces a name.
        let source = format!("/proc/self/fd/{}", self.document.as_raw_fd());
        rustix::fs::linkat(
            rustix::fs::CWD,
            source.as_str(),
            &self.pin.parent,
            &target,
            AtFlags::SYMLINK_FOLLOW,
        )?;
        // Keep Publishing even on a syscall error. A remote filesystem can
        // commit the link and lose the reply; only positive evidence can settle
        // that uncertainty. Never reset to a state which permits blind rollback.
        boundary(Phase::Published)?;
        state.phase = Phase::Published;
        self.append(state)?;
        self.sync_directories()
    }

    fn original(&self, actual: &File) -> io::Result<bool> {
        let (Some(expected), Some(held)) = (self.pin.selection.existing, &self.pin.entry) else {
            return Ok(false);
        };
        let mut current = Version::of(&actual.metadata()?);
        let mut retained = Version::of(&held.metadata()?);
        // Moving the directory entry changes ctime. All other reviewed facts
        // and the held object's identity must still match; never refresh consent.
        current.changed = expected.changed;
        retained.changed = expected.changed;
        Ok(current == expected && retained == expected)
    }

    fn recovery(&self) -> Vec<u8> {
        // Reading this link reports a moved work directory's current location;
        // it does not reopen it or grant authority to an on-disk journal.
        std::fs::read_link(format!("/proc/self/fd/{}", self.directory.as_raw_fd()))
            .unwrap_or_else(|_| {
                PathBuf::from(OsString::from_vec(self.pin.selection.path.clone()))
                    .parent()
                    .unwrap()
                    .join(OsString::from_vec(self.info.directory_name.clone()))
            })
            .into_os_string()
            .into_vec()
    }
    fn retained(&self, published: bool, uncertain: bool) -> Disposition {
        Disposition {
            published,
            uncertain,
            recovery: Some(self.recovery()),
        }
    }

    /// Run only after the publishing worker is reaped. A new independent helper
    /// can settle a cancelled operation while its original caller is gone.
    pub fn settle(&self, acknowledged: bool) -> io::Result<Disposition> {
        self.verify_work(false)?;
        let (_, target) = self.pin.selection.location()?;
        let current = any(&self.pin.parent, &target)?;
        let named_document = current.as_ref().is_some_and(|file| {
            file.metadata()
                .is_ok_and(|metadata| Identity::of(&metadata) == self.info.document)
        });
        let state = match self.state() {
            Ok(state) => state,
            Err(_) => {
                return Ok(self.retained(
                    acknowledged || named_document,
                    !acknowledged && !named_document,
                ));
            }
        };
        let published = acknowledged || state.phase == Phase::Published || named_document;
        if self.directory.metadata()?.nlink() == 0 {
            return Ok(Disposition {
                published,
                uncertain: !published && state.phase == Phase::Publishing,
                recovery: None,
            });
        }
        if !published && state.phase == Phase::Publishing {
            return Ok(self.retained(false, true));
        }
        if published && state.phase != Phase::Published {
            self.append(State {
                phase: Phase::Published,
                claimed: state.claimed,
                payload: state.payload,
            })?;
        }
        if let Some(previous) = any(&self.directory, OsStr::new(PREVIOUS))? {
            if published {
                if !self.original(&previous)?
                    || state.claimed != Some(Version::of(&previous.metadata()?))
                {
                    return Ok(self.retained(true, false));
                }
                rustix::fs::unlinkat(&self.directory, PREVIOUS, AtFlags::empty())?;
            } else {
                // Restore the actual claimed object, including an unreviewed
                // occupant caught between validation and rename. Never clobber
                // a newer name, and never discard the saved entry on conflict.
                self.append(State {
                    phase: Phase::Restoring,
                    claimed: Some(Version::of(&previous.metadata()?)),
                    payload: state.payload,
                })?;
                if rustix::fs::renameat_with(
                    &self.directory,
                    PREVIOUS,
                    &self.pin.parent,
                    &target,
                    RenameFlags::NOREPLACE,
                )
                .is_err()
                {
                    return Ok(self.retained(false, false));
                }
                self.sync_directories()?;
                self.append(State {
                    phase: Phase::Restored,
                    claimed: None,
                    payload: state.payload,
                })?;
            }
        } else if !published
            && matches!(state.phase, Phase::Claiming | Phase::Claimed)
            && !current
                .as_ref()
                .map(|file| self.original(file))
                .transpose()?
                .unwrap_or(false)
        {
            return Ok(self.retained(false, true));
        }
        self.sync_directories()?;
        // Every unlink is of a known private entry. An extra child prevents
        // rmdir; it is preserved rather than removed by a recursive cleanup.
        self.verify_work(false)?;
        let entries = std::fs::read_dir(format!("/proc/self/fd/{}", self.directory.as_raw_fd()))?;
        for entry in entries {
            if ![
                OsStr::new(DOCUMENT),
                OsStr::new(RECORD),
                OsStr::new(PAYLOAD),
            ]
            .contains(&entry?.file_name().as_os_str())
            {
                return Ok(self.retained(published, false));
            }
        }
        if let Some(marker) = any(&self.directory, OsStr::new(PAYLOAD))?
            && state.payload != Some(Identity::of(&marker.metadata()?))
        {
            return Ok(self.retained(published, false));
        }
        for name in [DOCUMENT, RECORD, PAYLOAD] {
            match rustix::fs::unlinkat(&self.directory, name, AtFlags::empty()) {
                Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                Err(error) => return Err(error.into()),
            }
        }
        if rustix::fs::unlinkat(&self.pin.parent, self.directory_name()?, AtFlags::REMOVEDIR)
            .is_err()
        {
            return Ok(self.retained(published, false));
        }
        File::from(openat(
            &self.pin.parent,
            c".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?)
        .sync_all()?;
        Ok(Disposition {
            published,
            uncertain: false,
            recovery: None,
        })
    }

    /// Used if a cleanup helper itself cannot finish. Without a positive
    /// publication receipt, the remaining journal needs explicit recovery.
    pub fn interrupted(&self, acknowledged: bool) -> Disposition {
        let state = self.state().ok();
        let published = acknowledged
            || state.is_some_and(|state| state.phase == Phase::Published)
            || self
                .pin
                .selection
                .location()
                .ok()
                .and_then(|(_, name)| any(&self.pin.parent, &name).ok().flatten())
                .is_some_and(|file| {
                    file.metadata()
                        .is_ok_and(|metadata| Identity::of(&metadata) == self.info.document)
                });
        if self
            .directory
            .metadata()
            .is_ok_and(|metadata| metadata.nlink() == 0)
        {
            return Disposition {
                published,
                uncertain: !published && state.is_none_or(|state| state.phase == Phase::Publishing),
                recovery: None,
            };
        }
        self.retained(published, !published)
    }
}

fn read_state(record: &File, info: &PublicationInfo) -> io::Result<State> {
    let bytes = record.metadata()?.len();
    if bytes == 0 || bytes > RECORD_LIMIT {
        return Err(invalid());
    }
    let mut content = vec![0; bytes as usize];
    record.read_exact_at(&mut content, 0)?;
    if content.last() != Some(&b'\n') {
        return Err(invalid());
    }
    let mut lines = content[..content.len() - 1].split(|byte| *byte == b'\n');
    let evidence: PublicationInfo = serde_json::from_slice(lines.next().ok_or_else(invalid)?)?;
    if evidence != *info {
        return Err(invalid());
    }
    let mut last = None;
    for line in lines {
        last = Some(serde_json::from_slice::<State>(line)?);
    }
    last.ok_or_else(invalid)
}

fn work_name(bytes: &[u8]) -> io::Result<OsString> {
    if !bytes.starts_with(b".canopa-print-")
        || bytes.len() != 46
        || !bytes[14..].iter().all(u8::is_ascii_hexdigit)
    {
        return Err(invalid());
    }
    Ok(OsString::from_vec(bytes.to_vec()))
}

/// A received O_PATH handle pins the exact object. Only the worker reopens
/// access through its own proc FD, after checking type and identity; it never
/// follows a caller-selected pathname to acquire writable staging authority.
fn access(file: File, identity: Identity, flags: OFlags) -> io::Result<File> {
    let metadata = file.metadata()?;
    if Identity::of(&metadata) != identity
        || if flags.contains(OFlags::DIRECTORY) {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        return Err(changed());
    }
    if rustix::fs::fcntl_getfl(&file)?.contains(OFlags::PATH) {
        Ok(File::from(openat(
            rustix::fs::CWD,
            format!("/proc/self/fd/{}", file.as_raw_fd()),
            flags | OFlags::CLOEXEC,
            Mode::empty(),
        )?))
    } else {
        Ok(file)
    }
}

fn fresh(directory: &File, name: &str) -> io::Result<File> {
    Ok(File::from(openat(
        directory,
        name,
        OFlags::CREATE | OFlags::EXCL | OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )?))
}
fn any(directory: &File, name: &OsStr) -> io::Result<Option<File>> {
    match openat(
        directory,
        name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(file) => Ok(Some(File::from(file))),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests;
