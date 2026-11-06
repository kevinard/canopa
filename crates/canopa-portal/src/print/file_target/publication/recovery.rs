//! Startup discovery is evidence, never a reconstructed publication capability.
//! All calls run inside bounded workers, including local catalog and /proc I/O.
use super::*;
mod export;
mod jobs;
mod process;
mod store;
use export::{active, location};
use process::Process;
use sha2::{Digest, Sha256};
use std::path::Path;

const CATALOG_LIMIT: u64 = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 16;
const NORMAL_ENTRIES: usize = MAX_ENTRIES - 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryBody {
    Document,
    Displaced,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryStore {
    path: Vec<u8>,
}

/// Evidence used only to forbid a destination; it grants no write authority.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryProtection {
    directory: Identity,
}
impl RecoveryProtection {
    pub fn allows(&self, selection: &Selection) -> bool {
        selection.parent != self.directory
    }
    pub(crate) fn allows_preparation(&self, preparation: &Preparation) -> bool {
        self.allows(&preparation.selection)
    }
    pub(crate) fn allows_publication(&self, publication: &PublicationInfo) -> bool {
        self.allows(&publication.selection)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStatus {
    Busy,
    Missing,
    Incomplete,
    Interrupted,
    Uncertain,
    Published,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryItem {
    pub id: String,
    pub destination: Vec<u8>,
    pub directory: Vec<u8>,
    pub status: RecoveryStatus,
    /// Offering an export still requires hashing a fresh immutable snapshot.
    pub document: bool,
    pub displaced: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    preparation: Preparation,
    publication: Option<PublicationInfo>,
    owner: Process,
    worker: Process,
    retained: bool,
    #[serde(default)]
    location: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u32,
    entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    document: Version,
    sha256: [u8; 32],
    #[serde(default)]
    allow_empty: bool,
}

fn id(preparation: &Preparation) -> io::Result<String> {
    work_name(&preparation.directory_name)?;
    preparation.selection.location()?;
    String::from_utf8(preparation.directory_name[14..].to_vec()).map_err(|_| invalid())
}

impl RecoveryStore {
    pub fn discover() -> io::Result<Self> {
        let base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .map(|path| path.join(".local/state"))
            })
            .ok_or_else(|| io::Error::other("Print recovery state location is unavailable"))?;
        Self::new(&base.join("canopa/portal/print-recovery"))
    }
    /// Explicit configuration avoids writing user state from stateless catalog
    /// clients and private fixtures. Construction performs no filesystem I/O.
    pub fn new(path: &Path) -> io::Result<Self> {
        use std::os::unix::ffi::OsStrExt;
        if !path.is_absolute() || path.as_os_str().len() > 4096 {
            return Err(invalid());
        }
        Ok(Self {
            path: path.as_os_str().as_bytes().to_vec(),
        })
    }

    pub(crate) fn begin(&self, preparation: &Preparation, owner: u32) -> io::Result<()> {
        self.begin_body(preparation, owner, false)
    }
    pub(crate) fn begin_recovery(&self, preparation: &Preparation, owner: u32) -> io::Result<()> {
        self.begin_body(preparation, owner, true)
    }
    fn begin_body(
        &self,
        preparation: &Preparation,
        owner: u32,
        recovering: bool,
    ) -> io::Result<()> {
        let key = id(preparation)?;
        let entry = Entry {
            preparation: preparation.clone(),
            publication: None,
            owner: Process::read(owner)?,
            worker: Process::read(std::process::id())?,
            retained: false,
            location: None,
        };
        self.change(true, |catalog| {
            if !entry.owner.alive()? {
                return Err(io::Error::other(
                    "Print owner exited before recovery registration",
                ));
            }
            if catalog.entries.iter().any(|existing| {
                existing
                    .publication
                    .as_ref()
                    .is_some_and(|info| info.directory == preparation.selection.parent)
            }) {
                return Err(io::Error::other(
                    "Choose a destination outside retained print folders",
                ));
            }
            if catalog.entries.len()
                >= if recovering {
                    MAX_ENTRIES
                } else {
                    NORMAL_ENTRIES
                }
                || catalog
                    .entries
                    .iter()
                    .any(|entry| id(&entry.preparation).ok().as_ref() == Some(&key))
            {
                return Err(io::Error::other(
                    "Print recovery list is full or allocation already exists",
                ));
            }
            catalog.entries.push(entry);
            Ok(())
        })
    }

    /// Persist the executing worker before it can change staged/target bytes.
    pub(crate) fn active(&self, info: &PublicationInfo, owner: u32) -> io::Result<()> {
        let owner = Process::read(owner)?;
        let worker = Process::read(std::process::id())?;
        self.change(false, |catalog| {
            let entry = catalog
                .entries
                .iter_mut()
                .find(|entry| {
                    entry.preparation.selection == info.selection
                        && entry.preparation.directory_name == info.directory_name
                })
                .ok_or_else(invalid)?;
            if entry.owner != owner
                || entry.retained
                || !owner.alive()?
                || entry.publication.as_ref().is_some_and(|old| old != info)
            {
                return Err(invalid());
            }
            entry.worker = worker;
            entry.publication = Some(info.clone());
            Ok(())
        })
    }

    pub(crate) fn settled(
        &self,
        info: &PublicationInfo,
        owner: u32,
        disposition: &Disposition,
    ) -> io::Result<()> {
        let owner = Process::read(owner)?;
        self.change(false, |catalog| {
            let index = catalog
                .entries
                .iter()
                .position(|entry| {
                    entry.preparation.selection == info.selection
                        && entry.preparation.directory_name == info.directory_name
                        && entry.owner == owner
                })
                .ok_or_else(invalid)?;
            if disposition.recovery.is_none() {
                catalog.entries.remove(index);
            } else {
                catalog.entries[index].retained = true;
                catalog.entries[index].location = disposition.recovery.clone();
            }
            Ok(())
        })
    }

    /// Reads at most one bounded catalog and sixteen exact recorded locations.
    /// Missing/moved work stays visible. No destination is opened for writing.
    pub fn list(&self) -> io::Result<Vec<RecoveryItem>> {
        let Some(directory) = self.open(false)? else {
            return Ok(vec![]);
        };
        let lock = match Self::lock(&directory, false) {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if any(&directory, OsStr::new("catalog.json"))?.is_some() {
                    return Err(error);
                }
                return Ok(vec![]);
            }
            Err(error) => return Err(error),
        };
        let entries = self.load(&directory)?.entries;
        // Pair process checks with registration's exclusive lock. A future
        // writer must recheck owner liveness under that same lock before acting.
        let activity = entries.iter().map(active).collect::<Vec<_>>();
        drop(lock);
        entries
            .iter()
            .zip(activity)
            .map(|(entry, active)| {
                let mut item = RecoveryItem {
                    id: id(&entry.preparation)?,
                    destination: entry.preparation.selection.path.clone(),
                    directory: location(entry)?,
                    status: RecoveryStatus::Unavailable,
                    document: false,
                    displaced: false,
                };
                // An error reading process identity is not proof of retirement.
                match active {
                    Ok(true) => item.status = RecoveryStatus::Busy,
                    Ok(false) => match inspect(entry, &item.directory) {
                        Ok((status, document, displaced)) => {
                            item.status = status;
                            item.document = document;
                            item.displaced = displaced;
                        }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            item.status = RecoveryStatus::Missing
                        }
                        Err(_) => {}
                    },
                    Err(_) => {}
                }
                Ok(item)
            })
            .collect()
    }
}

fn private(file: &File, directory: bool) -> io::Result<()> {
    let metadata = file.metadata()?;
    // SAFETY: geteuid has no preconditions or side effects.
    if metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file() || metadata.nlink() != 1
        }
    {
        return Err(invalid());
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(file: &File, limit: u64) -> io::Result<T> {
    let length = file.metadata()?.len();
    if length == 0 || length > limit {
        return Err(invalid());
    }
    let mut bytes = vec![0; length as usize];
    file.read_exact_at(&mut bytes, 0)?;
    serde_json::from_slice(&bytes).map_err(Into::into)
}

fn read_only(directory: &File, name: &str) -> io::Result<File> {
    // NONBLOCK prevents a substituted FIFO from hanging open; type checks then
    // reject it. The worker deadline also bounds remote metadata/read stalls.
    Ok(File::from(openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?))
}

fn digest(document: &File, check: &mut impl FnMut() -> io::Result<()>) -> io::Result<Payload> {
    digest_body(document, false, check)
}
fn digest_body(
    document: &File,
    allow_empty: bool,
    check: &mut impl FnMut() -> io::Result<()>,
) -> io::Result<Payload> {
    let before = Version::of(&document.metadata()?);
    if (!allow_empty && before.bytes == 0)
        || before.bytes > crate::print::worker::MAX_DOCUMENT_BYTES
    {
        return Err(invalid());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut offset = 0;
    while offset < before.bytes {
        check()?;
        let length = buffer.len().min((before.bytes - offset) as usize);
        document.read_exact_at(&mut buffer[..length], offset)?;
        hash.update(&buffer[..length]);
        offset += length as u64;
    }
    if Version::of(&document.metadata()?) != before {
        return Err(changed());
    }
    Ok(Payload {
        document: before,
        sha256: hash.finalize().into(),
        allow_empty,
    })
}

pub(super) fn complete(
    directory: &File,
    document: &File,
    allow_empty: bool,
    check: &mut impl FnMut() -> io::Result<()>,
) -> io::Result<Identity> {
    let payload = if allow_empty {
        digest_body(document, true, check)?
    } else {
        digest(document, check)?
    };
    let mut marker = fresh(directory, PAYLOAD)?;
    serde_json::to_writer(&mut marker, &payload)?;
    marker.sync_all()?;
    directory.sync_all()?;
    Ok(Identity::of(&marker.metadata()?))
}

fn inspect(entry: &Entry, path: &[u8]) -> io::Result<(RecoveryStatus, bool, bool)> {
    let directory = File::from(rustix::fs::open(
        PathBuf::from(OsString::from_vec(path.to_vec())),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    private(&directory, true)?;
    let Some(info) = &entry.publication else {
        return Ok((RecoveryStatus::Incomplete, false, false));
    };
    if Identity::of(&directory.metadata()?) != info.directory {
        return Err(changed());
    }
    let record = read_only(&directory, RECORD)?;
    private(&record, false)?;
    if Identity::of(&record.metadata()?) != info.record {
        return Err(changed());
    }
    let state = read_state(&record, info)?;
    let (complete, invalid_document) = match document_ready(&directory, info, state) {
        Ok(complete) => (complete, false),
        Err(_) => (false, true),
    };
    let displaced = match read_only(&directory, PREVIOUS) {
        Ok(previous) => {
            let metadata = previous.metadata()?;
            metadata.is_file() && metadata.len() <= crate::print::worker::MAX_DOCUMENT_BYTES
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    if invalid_document && !displaced {
        return Ok((RecoveryStatus::Unavailable, false, false));
    }
    let status = match state.phase {
        Phase::Published => RecoveryStatus::Published,
        Phase::Publishing => RecoveryStatus::Uncertain,
        _ if complete || displaced => RecoveryStatus::Interrupted,
        _ => RecoveryStatus::Incomplete,
    };
    Ok((status, complete, displaced))
}

fn document_ready(directory: &File, info: &PublicationInfo, state: State) -> io::Result<bool> {
    let document = read_only(directory, DOCUMENT)?;
    let metadata = document.metadata()?;
    // Published payloads have two links until settlement. Never require nlink=1
    // for that file, nor ctime equality across the publication link syscall.
    if !metadata.is_file()
        || metadata.mode() & 0o7777 != 0o600
        || Identity::of(&metadata) != info.document
    {
        return Err(changed());
    }
    let payload = match read_only(directory, PAYLOAD) {
        Ok(marker) => {
            private(&marker, false)?;
            if state.payload != Some(Identity::of(&marker.metadata()?)) {
                return Err(changed());
            }
            Some(read_json::<Payload>(&marker, 4096)?)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let complete = payload.is_some_and(|payload| {
        payload.document.identity == info.document
            && payload.document.bytes == metadata.len()
            && (metadata.len() > 0 || payload.allow_empty)
            && metadata.len() <= crate::print::worker::MAX_DOCUMENT_BYTES
            && payload.document.modified == (metadata.mtime(), metadata.mtime_nsec())
    });
    Ok(complete)
}

#[cfg(test)]
mod tests;
