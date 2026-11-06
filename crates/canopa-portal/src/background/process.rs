//! Resolve sandbox identity through a pinned process directory, never the
//! client-declared Wayland app ID. All filesystem work runs on one worker.
use super::observer::{Job, WindowKey};
use super::{Activity, AppStates, Unavailable};
use rustix::fs::{Mode, OFlags, open, openat};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, Metadata},
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
};

const INFO_BYTES: usize = 64 * 1024;
const BATCH_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileStamp {
    dev: u64,
    ino: u64,
    size: u64,
    modified: (i64, i64),
}
impl FileStamp {
    fn new(metadata: &Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            size: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
        }
    }
}

struct Process {
    directory: File,
    root: File,
    pid: i32,
    start: u64,
    root_id: (u64, u64),
    info: Option<FileStamp>,
    app_id: Option<String>,
}

fn read(file: File, limit: usize) -> io::Result<Vec<u8>> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(io::Error::other("invalid process metadata"));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::other("process metadata over budget"));
    }
    Ok(bytes)
}

fn relative(directory: &File, name: &str) -> io::Result<File> {
    Ok(File::from(openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOFOLLOW,
        Mode::empty(),
    )?))
}

fn birth(directory: &File, pid: i32) -> io::Result<u64> {
    let bytes = read(relative(directory, "stat")?, 16 * 1024)?;
    let separator = bytes
        .windows(2)
        .position(|part| part == b" (")
        .ok_or_else(|| io::Error::other("invalid process stat"))?;
    let prefix = std::str::from_utf8(&bytes[..separator]).map_err(io::Error::other)?;
    if prefix.parse::<i32>().ok() != Some(pid) {
        return Err(io::Error::other("process changed"));
    }
    let end = bytes
        .windows(2)
        .rposition(|part| part == b") ")
        .ok_or_else(|| io::Error::other("invalid process stat"))?;
    let fields = std::str::from_utf8(&bytes[end + 2..]).map_err(io::Error::other)?;
    let fields: Vec<_> = fields.split_whitespace().take(20).collect();
    if fields.len() != 20 || matches!(fields[0], "Z" | "X" | "x") {
        return Err(io::Error::other("process exited"));
    }
    fields[19]
        .parse::<u64>()
        .ok()
        .filter(|start| *start > 0)
        .ok_or_else(|| io::Error::other("invalid process birth"))
}

pub(super) fn app_id(value: &str) -> bool {
    let Some((prefix, last)) = value.rsplit_once('.') else {
        return false;
    };
    value.len() <= 255
        && !last.is_empty()
        && prefix.split('.').all(|part| {
            !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
        && last
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn identity(bytes: &[u8]) -> io::Result<String> {
    let source = std::str::from_utf8(bytes).map_err(io::Error::other)?;
    if source.contains('\0') {
        return Err(io::Error::other("invalid sandbox metadata"));
    }
    let mut group = "";
    let mut id = None;
    let mut instance = None;
    for line in source.lines() {
        let line = line.trim_end_matches('\r').trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(header) = line
            .trim_end()
            .strip_prefix('[')
            .and_then(|v| v.strip_suffix(']'))
        {
            group = header;
        } else if let Some((key, value)) = line.split_once('=') {
            let target = match (group, key.trim()) {
                ("Application" | "Runtime", "name") => &mut id,
                ("Instance", "instance-id") => &mut instance,
                _ => continue,
            };
            if target.replace(value.trim_start()).is_some() {
                return Err(io::Error::other("ambiguous sandbox metadata"));
            }
        } else {
            return Err(io::Error::other("invalid sandbox metadata syntax"));
        }
    }
    let id = id
        .filter(|id| app_id(id))
        .ok_or_else(|| io::Error::other("invalid sandbox application"))?;
    let _ = instance
        .filter(|id| !id.is_empty() && id.len() <= 255 && !id.chars().any(char::is_control))
        .ok_or_else(|| io::Error::other("invalid sandbox instance"))?;
    Ok(id.into())
}

fn info(root: &File) -> io::Result<Option<File>> {
    match relative(root, ".flatpak-info") {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

impl Process {
    fn open(proc_root: &Path, pid: i32, budget: &mut usize) -> io::Result<Self> {
        if pid <= 0 {
            return Err(io::Error::other("invalid client process"));
        }
        let directory = File::from(open(
            proc_root.join(pid.to_string()),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        if directory.metadata()?.uid() != rustix::process::geteuid().as_raw() {
            return Err(io::Error::other("foreign client process"));
        }
        let start = birth(&directory, pid)?;
        let root = File::from(openat(
            &directory,
            "root",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )?);
        let metadata = root.metadata()?;
        let root_id = (metadata.dev(), metadata.ino());
        let (info, app_id) = match info(&root)? {
            None => {
                let host = std::fs::metadata("/")?;
                if root_id != (host.dev(), host.ino()) {
                    return Err(io::Error::other("unidentified process namespace"));
                }
                (None, None)
            }
            Some(file) => {
                let stamp = FileStamp::new(&file.metadata()?);
                let bytes = read(file, INFO_BYTES.min(*budget))?;
                *budget = budget
                    .checked_sub(bytes.len())
                    .ok_or_else(|| io::Error::other("metadata budget"))?;
                (Some(stamp), Some(identity(&bytes)?))
            }
        };
        let process = Self {
            directory,
            root,
            pid,
            start,
            root_id,
            info,
            app_id,
        };
        process.verify()?;
        Ok(process)
    }

    fn verify(&self) -> io::Result<()> {
        if birth(&self.directory, self.pid)? != self.start {
            return Err(io::Error::other("client process changed"));
        }
        let root = File::from(openat(
            &self.directory,
            "root",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )?);
        let metadata = root.metadata()?;
        if (metadata.dev(), metadata.ino()) != self.root_id {
            return Err(io::Error::other("client root changed"));
        }
        let current = info(&self.root)?
            .map(|file| file.metadata().map(|m| FileStamp::new(&m)))
            .transpose()?;
        if current != self.info {
            return Err(io::Error::other("sandbox identity changed"));
        }
        Ok(())
    }
}

pub(super) struct Processes {
    proc_root: PathBuf,
    cache: BTreeMap<WindowKey, Arc<Process>>,
}
impl Default for Processes {
    fn default() -> Self {
        Self {
            proc_root: "/proc".into(),
            cache: BTreeMap::new(),
        }
    }
}
impl Processes {
    pub(super) fn project(&mut self, job: &Job) -> Result<AppStates, Unavailable> {
        let keys: BTreeSet<_> = job.windows.iter().map(|w| w.key).collect();
        self.cache.retain(|key, _| keys.contains(key));
        let mut apps = AppStates::new();
        let mut budget = BATCH_BYTES;
        for window in &job.windows {
            let pid = window.pid.ok_or(Unavailable::Identity)?;
            let process = match self.cache.get(&window.key) {
                Some(process) => process.clone(),
                None => {
                    let process = Arc::new(
                        Process::open(&self.proc_root, pid, &mut budget)
                            .map_err(|_| Unavailable::Identity)?,
                    );
                    self.cache.insert(window.key, process.clone());
                    process
                }
            };
            process.verify().map_err(|_| Unavailable::Identity)?;
            if let Some(id) = &process.app_id {
                let state = if window.focused {
                    Activity::Active
                } else {
                    Activity::Running
                };
                apps.entry(id.clone())
                    .and_modify(|current| {
                        if state as u32 > *current as u32 {
                            *current = state;
                        }
                    })
                    .or_insert(state);
            }
        }
        // Revalidate earlier reads after preparing the entire batch.
        for process in self.cache.values() {
            process.verify().map_err(|_| Unavailable::Identity)?;
        }
        Ok(apps)
    }
}

#[cfg(test)]
mod tests;
