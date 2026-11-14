//! Compressed archives: creation and extraction, independent of the renderer and
//! of job scheduling.
//!
//! A run writes its result under a hidden partial name inside the destination
//! folder and publishes it with a no-replace rename under the next free name.
//! A cancelled, failed or interrupted run therefore never leaves a half-written
//! result under a visible name, and never replaces an existing item.
//!
//! Extraction refuses entries that would leave the extracted folder: absolute
//! paths, `..` components, symbolic links whose target leaves it, and hard links
//! to anything but a regular file extracted earlier. Symbolic links are created
//! only after every other entry, so nothing is ever written through one. Written
//! bytes are bounded by the destination's free space and entries by a count, so a
//! decompression bomb stops with an error instead of filling the disk. Set-id and
//! sticky bits are never restored.
use crate::Cancellation;
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Component, Path, PathBuf},
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// An archive format. Every format can be extracted; [`Format::WRITABLE`] lists
/// those offered when compressing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    Zip,
    Tar,
    TarGz,
    TarXz,
    TarZst,
    TarBz2,
}

/// Recognised name suffixes, compound ones before their `.tar` tail.
const SUFFIXES: &[(&str, Format)] = &[
    (".tar.gz", Format::TarGz),
    (".tgz", Format::TarGz),
    (".tar.xz", Format::TarXz),
    (".txz", Format::TarXz),
    (".tar.zst", Format::TarZst),
    (".tzst", Format::TarZst),
    (".tar.bz2", Format::TarBz2),
    (".tbz2", Format::TarBz2),
    (".tbz", Format::TarBz2),
    (".tar", Format::Tar),
    (".zip", Format::Zip),
];

impl Format {
    /// Formats offered for new archives, the default first.
    pub const WRITABLE: [Self; 4] = [Self::Zip, Self::TarGz, Self::TarXz, Self::TarZst];

    /// The name suffix written for this format, including its leading dot.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Zip => ".zip",
            Self::Tar => ".tar",
            Self::TarGz => ".tar.gz",
            Self::TarXz => ".tar.xz",
            Self::TarZst => ".tar.zst",
            Self::TarBz2 => ".tar.bz2",
        }
    }

    /// The format a file name announces, ignoring ASCII case.
    pub fn detect(name: &OsStr) -> Option<Self> {
        split(name).map(|(_, format)| format)
    }

    fn sniff(file: &mut fs::File) -> io::Result<Option<Self>> {
        let mut head = [0; 512];
        let mut read = 0;
        while read < head.len() {
            match file.read(&mut head[read..]) {
                Ok(0) => break,
                Ok(count) => read += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        file.seek(SeekFrom::Start(0))?;
        let head = &head[..read];
        Ok(
            if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
                Some(Self::Zip)
            } else if head.starts_with(&[0x1f, 0x8b]) {
                Some(Self::TarGz)
            } else if head.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0]) {
                Some(Self::TarXz)
            } else if head.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
                Some(Self::TarZst)
            } else if head.starts_with(b"BZh") {
                Some(Self::TarBz2)
            } else if head.get(257..262) == Some(b"ustar") {
                Some(Self::Tar)
            } else {
                None
            },
        )
    }
}

/// The name without its archive suffix, and the format, when the name has a
/// recognised suffix and something before it.
fn split(name: &OsStr) -> Option<(&OsStr, Format)> {
    let bytes = name.as_bytes();
    SUFFIXES.iter().find_map(|(suffix, format)| {
        let stem = bytes
            .len()
            .checked_sub(suffix.len())
            .filter(|&end| end > 0)?;
        bytes[stem..]
            .eq_ignore_ascii_case(suffix.as_bytes())
            .then(|| (OsStr::from_bytes(&bytes[..stem]), *format))
    })
}

/// A name for the result of extracting `archive`: the archive's name without its
/// suffix.
pub fn stem(archive: &OsStr) -> &OsStr {
    split(archive).map_or(archive, |(stem, _)| stem)
}

/// The proposed archive name, without a suffix: a single item's name (a file's
/// without its extension), otherwise "Archive".
pub fn suggested_name(items: &[(&OsStr, bool)]) -> OsString {
    match items {
        [(name, true)] => name.to_os_string(),
        [(name, false)] => Path::new(name).file_stem().unwrap_or(name).to_os_string(),
        _ => fl!("files-archive-default-name").into(),
    }
}

/// Whether `name` can name an archive in a folder.
pub fn valid_name(name: &OsStr) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes != b"."
        && bytes != b".."
        && !bytes.contains(&b'/')
        && !bytes.contains(&0)
        && bytes.len() <= 200
}

/// A durable archive request. The token names this run's partial result so an
/// interrupted run can be cleaned up without touching anything else.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum ArchiveRequest {
    Compress {
        #[serde(with = "crate::path_serde::sequence")]
        sources: Vec<PathBuf>,
        /// The folder that receives the archive.
        #[serde(with = "crate::path_serde")]
        destination: PathBuf,
        /// The archive's name without its suffix.
        #[serde(with = "crate::path_serde")]
        name: PathBuf,
        format: Format,
        token: u64,
    },
    Extract {
        #[serde(with = "crate::path_serde")]
        archive: PathBuf,
        /// The folder that receives the extracted item.
        #[serde(with = "crate::path_serde")]
        destination: PathBuf,
        token: u64,
    },
}

fn token() -> u64 {
    let mut bytes = [0; 8];
    if rustix::rand::getrandom(&mut bytes, rustix::rand::GetRandomFlags::empty()).is_err() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        return now.as_nanos() as u64 ^ u64::from(std::process::id()).rotate_left(32);
    }
    u64::from_ne_bytes(bytes)
}

impl ArchiveRequest {
    pub fn compress(
        sources: Vec<PathBuf>,
        destination: PathBuf,
        name: OsString,
        format: Format,
    ) -> Self {
        Self::Compress {
            sources,
            destination,
            name: name.into(),
            format,
            token: token(),
        }
    }

    pub fn extract(archive: PathBuf, destination: PathBuf) -> Self {
        Self::Extract {
            archive,
            destination,
            token: token(),
        }
    }

    pub fn valid(&self) -> bool {
        match self {
            Self::Compress {
                sources,
                destination,
                name,
                ..
            } => {
                !sources.is_empty()
                    && sources
                        .iter()
                        .all(|source| source.is_absolute() && source.file_name().is_some())
                    && destination.is_absolute()
                    && valid_name(name.as_os_str())
            }
            Self::Extract {
                archive,
                destination,
                ..
            } => {
                archive.is_absolute() && archive.file_name().is_some() && destination.is_absolute()
            }
        }
    }

    /// The selected items, then the destination folder.
    pub fn paths(&self) -> Vec<PathBuf> {
        match self {
            Self::Compress {
                sources,
                destination,
                ..
            } => sources
                .iter()
                .cloned()
                .chain(std::iter::once(destination.clone()))
                .collect(),
            Self::Extract {
                archive,
                destination,
                ..
            } => vec![archive.clone(), destination.clone()],
        }
    }

    pub fn destination(&self) -> &Path {
        match self {
            Self::Compress { destination, .. } | Self::Extract { destination, .. } => destination,
        }
    }

    /// Where this run keeps its unfinished result.
    pub fn partial_path(&self) -> PathBuf {
        match self {
            Self::Compress {
                destination, token, ..
            } => destination.join(format!(".canopa-archive-{token:016x}.partial")),
            Self::Extract {
                destination, token, ..
            } => destination.join(format!(".canopa-extract-{token:016x}.partial")),
        }
    }

    /// Remove an interrupted run's unfinished result. Only this run's own hidden
    /// partial name is removed, and never through a symbolic link.
    pub fn discard_partial(&self) -> io::Result<bool> {
        let partial = self.partial_path();
        let metadata = match fs::symlink_metadata(&partial) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        match self {
            Self::Compress { .. } if metadata.is_file() => fs::remove_file(&partial)?,
            Self::Extract { .. } if metadata.is_dir() => remove_tree(&partial)?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// Live progress of a run.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    /// The entry being processed, at its place in the destination.
    pub path: PathBuf,
    pub entries: u64,
    /// Source bytes read when compressing; archive bytes read when extracting.
    pub bytes: u64,
    /// Unknown while a tar stream is extracted.
    pub total_entries: Option<u64>,
    pub total_bytes: Option<u64>,
}

/// Bounds on extraction.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_entries: u64,
    /// Free space left untouched on the destination's filesystem.
    pub reserve: u64,
    /// An explicit byte budget instead of the free space.
    pub max_bytes: Option<u64>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 1_000_000,
            reserve: 64 << 20,
            max_bytes: None,
        }
    }
}

/// A published result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub output: PathBuf,
    pub entries: u64,
    pub bytes: u64,
    /// Extraction entries refused because they lead outside the result.
    pub skipped_unsafe: u64,
    /// Devices, pipes and sockets, which are neither stored nor extracted.
    pub skipped_special: u64,
}

impl Outcome {
    /// A summary for the operation's report.
    pub fn detail(&self, request: &ArchiveRequest) -> String {
        let name = self
            .output
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut detail = match request {
            ArchiveRequest::Compress { .. } => {
                fl!(
                    "files-archive-compressed",
                    name = name,
                    count = self.entries
                )
            }
            ArchiveRequest::Extract { .. } => {
                fl!("files-archive-extracted", name = name, count = self.entries)
            }
        };
        if self.skipped_unsafe > 0 {
            detail.push(' ');
            detail.push_str(&fl!(
                "files-archive-skipped-unsafe",
                count = self.skipped_unsafe
            ));
        }
        if self.skipped_special > 0 {
            detail.push(' ');
            detail.push_str(&fl!(
                "files-archive-skipped-special",
                count = self.skipped_special
            ));
        }
        detail
    }
}

/// Whether `error` reports this module's cancellation.
/// Readers can wrap it with context, so its whole source chain is searched.
pub fn cancelled(error: &io::Error) -> bool {
    let mut current: Option<&(dyn std::error::Error + 'static)> = error
        .get_ref()
        .map(|inner| inner as &(dyn std::error::Error + 'static));
    while let Some(inner) = current {
        if inner.is::<Cancelled>() {
            return true;
        }
        current = match inner.downcast_ref::<io::Error>() {
            Some(error) => error
                .get_ref()
                .map(|inner| inner as &(dyn std::error::Error + 'static)),
            None => inner.source(),
        };
    }
    false
}

#[derive(Debug)]
struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&fl!("files-archive-cancelled"))
    }
}

impl std::error::Error for Cancelled {}

fn check(cancellation: &Cancellation) -> io::Result<()> {
    if cancellation.is_cancelled() {
        // Not `Interrupted`, which `io::copy` and friends silently retry.
        Err(io::Error::other(Cancelled))
    } else {
        Ok(())
    }
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Run `request`. Nothing remains under the partial name when this returns,
/// whether it succeeds or not.
pub fn run(
    request: &ArchiveRequest,
    cancellation: &Cancellation,
    limits: Limits,
    progress: &mut dyn FnMut(&Progress),
) -> io::Result<Outcome> {
    if !request.valid() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "An archive request needs absolute paths and a valid name",
        ));
    }
    let partial = request.partial_path();
    let result = match request {
        ArchiveRequest::Compress {
            sources,
            destination,
            name,
            format,
            ..
        } => compress(
            sources,
            destination,
            name.as_os_str(),
            *format,
            &partial,
            cancellation,
            progress,
        ),
        ArchiveRequest::Extract {
            archive,
            destination,
            ..
        } => extract(
            archive,
            destination,
            &partial,
            cancellation,
            limits,
            progress,
        ),
    };
    if result.is_err() {
        let _ = request.discard_partial();
    }
    result
}

/// Rename `from` to the first free name made of `base`, an optional " (n)" and
/// `suffix`, without ever replacing an existing item.
fn publish(from: &Path, folder: &Path, base: &OsStr, suffix: &OsStr) -> io::Result<PathBuf> {
    for number in 1..=10_000u32 {
        let mut name = base.to_os_string();
        if number > 1 {
            name.push(format!(" ({number})"));
        }
        name.push(suffix);
        let candidate = folder.join(&name);
        match rustix::fs::renameat_with(
            rustix::fs::CWD,
            from,
            rustix::fs::CWD,
            &candidate,
            rustix::fs::RenameFlags::NOREPLACE,
        ) {
            Ok(()) => {
                if let Ok(folder) = fs::File::open(folder) {
                    let _ = folder.sync_all();
                }
                return Ok(candidate);
            }
            Err(rustix::io::Errno::EXIST) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        fl!("files-archive-no-name"),
    ))
}

/// Split an item's name for numbering: folders keep their whole name, files
/// keep their extension after the number.
fn numbered_parts(name: &OsStr, directory: bool) -> (OsString, OsString) {
    if !directory && let Some((stem, _)) = split(name) {
        let suffix = &name.as_bytes()[stem.len()..];
        return (
            stem.to_os_string(),
            OsStr::from_bytes(suffix).to_os_string(),
        );
    }
    let path = Path::new(name);
    match (directory, path.file_stem(), path.extension()) {
        (false, Some(stem), Some(extension)) => {
            let mut suffix = OsString::from(".");
            suffix.push(extension);
            (stem.to_os_string(), suffix)
        }
        _ => (name.to_os_string(), OsString::new()),
    }
}

fn remove_tree(path: &Path) -> io::Result<()> {
    // Extraction can leave folders without write permission for their owner.
    fn writable(path: &Path) {
        if let Ok(metadata) = fs::symlink_metadata(path)
            && metadata.is_dir()
        {
            let _ = fs::set_permissions(
                path,
                fs::Permissions::from_mode(metadata.mode() & 0o777 | 0o700),
            );
            if let Ok(children) = fs::read_dir(path) {
                for child in children.flatten() {
                    writable(&child.path());
                }
            }
        }
    }
    writable(path);
    fs::remove_dir_all(path)
}

// Compression.

enum Kind {
    Directory,
    File,
    Symlink(PathBuf),
}

struct Item {
    path: PathBuf,
    /// The entry's relative name in the archive.
    name: PathBuf,
    kind: Kind,
    metadata: fs::Metadata,
}

fn scan(
    sources: &[PathBuf],
    partial: &Path,
    cancellation: &Cancellation,
    skipped: &mut u64,
) -> io::Result<Vec<Item>> {
    let mut items = Vec::new();
    let mut pending: Vec<(PathBuf, PathBuf)> = sources
        .iter()
        .rev()
        .map(|source| {
            (
                source.clone(),
                PathBuf::from(source.file_name().unwrap_or_default()),
            )
        })
        .collect();
    while let Some((path, name)) = pending.pop() {
        check(cancellation)?;
        if path == partial {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        let file_type = metadata.file_type();
        let kind = if file_type.is_dir() {
            let mut children = fs::read_dir(&path)?
                .map(|child| child.map(|child| child.file_name()))
                .collect::<io::Result<Vec<_>>>()?;
            children.sort();
            pending.extend(
                children
                    .into_iter()
                    .rev()
                    .map(|child| (path.join(&child), name.join(&child))),
            );
            Kind::Directory
        } else if file_type.is_file() {
            Kind::File
        } else if file_type.is_symlink() {
            Kind::Symlink(fs::read_link(&path)?)
        } else {
            *skipped += 1;
            continue;
        };
        items.push(Item {
            path,
            name,
            kind,
            metadata,
        });
    }
    Ok(items)
}

/// Report progress while reading exactly the scanned size of a file.
struct Meter<'a> {
    cancellation: &'a Cancellation,
    progress: &'a mut dyn FnMut(&Progress),
    state: Progress,
}

impl Meter<'_> {
    fn report(&mut self) {
        (self.progress)(&self.state);
    }
}

struct Exact<'a, 'b> {
    file: fs::File,
    remaining: u64,
    name: &'a Path,
    meter: &'a mut Meter<'b>,
}

impl Read for Exact<'_, '_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        check(self.meter.cancellation)?;
        if self.remaining == 0 {
            return Ok(0);
        }
        let limit = buffer
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let count = self.file.read(&mut buffer[..limit])?;
        if count == 0 {
            return Err(invalid(fl!(
                "files-archive-changed",
                name = self.name.to_string_lossy().into_owned()
            )));
        }
        self.remaining -= count as u64;
        self.meter.state.bytes += count as u64;
        self.meter.report();
        Ok(count)
    }
}

enum Encoder {
    Plain(fs::File),
    Gz(flate2::write::GzEncoder<fs::File>),
    Xz(lzma_rust2::XzWriter<fs::File>),
    Zst(zstd::Encoder<'static, fs::File>),
}

impl Encoder {
    fn new(file: fs::File, format: Format) -> io::Result<Self> {
        Ok(match format {
            Format::Tar => Self::Plain(file),
            Format::TarGz => Self::Gz(flate2::write::GzEncoder::new(
                file,
                flate2::Compression::default(),
            )),
            Format::TarXz => Self::Xz(lzma_rust2::XzWriter::new(
                file,
                lzma_rust2::XzOptions::with_preset(6),
            )?),
            Format::TarZst => Self::Zst(zstd::Encoder::new(file, 0)?),
            Format::Zip | Format::TarBz2 => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "This format is not written as a compressed tar stream",
                ));
            }
        })
    }

    fn finish(self) -> io::Result<fs::File> {
        match self {
            Self::Plain(file) => Ok(file),
            Self::Gz(encoder) => encoder.finish(),
            Self::Xz(encoder) => Ok(encoder.finish()?),
            Self::Zst(encoder) => encoder.finish(),
        }
    }
}

impl Write for Encoder {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(file) => file.write(buffer),
            Self::Gz(encoder) => encoder.write(buffer),
            Self::Xz(encoder) => encoder.write(buffer),
            Self::Zst(encoder) => encoder.write(buffer),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(file) => file.flush(),
            Self::Gz(encoder) => encoder.flush(),
            Self::Xz(encoder) => encoder.flush(),
            Self::Zst(encoder) => encoder.flush(),
        }
    }
}

fn compress(
    sources: &[PathBuf],
    destination: &Path,
    name: &OsStr,
    format: Format,
    partial: &Path,
    cancellation: &Cancellation,
    progress: &mut dyn FnMut(&Progress),
) -> io::Result<Outcome> {
    let mut skipped_special = 0;
    let items = scan(sources, partial, cancellation, &mut skipped_special)?;
    let mut meter = Meter {
        cancellation,
        progress,
        state: Progress {
            total_entries: Some(items.len() as u64),
            total_bytes: Some(
                items
                    .iter()
                    .filter(|item| matches!(item.kind, Kind::File))
                    .map(|item| item.metadata.len())
                    .sum(),
            ),
            ..Progress::default()
        },
    };
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(partial)?;
    let file = if format == Format::Zip {
        write_zip(file, &items, &mut meter)?
    } else {
        write_tar(Encoder::new(file, format)?, &items, &mut meter)?
    };
    file.sync_all()?;
    drop(file);
    let output = publish(partial, destination, name, OsStr::new(format.suffix()))?;
    Ok(Outcome {
        output,
        entries: items.len() as u64,
        bytes: meter.state.bytes,
        skipped_unsafe: 0,
        skipped_special,
    })
}

fn write_tar(encoder: Encoder, items: &[Item], meter: &mut Meter<'_>) -> io::Result<fs::File> {
    let mut builder = tar::Builder::new(encoder);
    builder.follow_symlinks(false);
    for item in items {
        check(meter.cancellation)?;
        meter.state.path.clone_from(&item.path);
        let mut header = tar::Header::new_gnu();
        header.set_metadata_in_mode(&item.metadata, tar::HeaderMode::Complete);
        match &item.kind {
            Kind::Directory => {
                header.set_size(0);
                builder.append_data(&mut header, &item.name, io::empty())?;
            }
            Kind::Symlink(target) => {
                header.set_size(0);
                builder.append_link(&mut header, &item.name, target)?;
            }
            Kind::File => {
                let reader = Exact {
                    file: fs::File::open(&item.path)?,
                    remaining: item.metadata.len(),
                    name: &item.name,
                    meter,
                };
                builder.append_data(&mut header, &item.name, reader)?;
            }
        }
        meter.state.entries += 1;
        meter.report();
    }
    builder.into_inner()?.finish()
}

fn zip_time(time: SystemTime) -> zip::DateTime {
    use chrono::{Datelike, Timelike};
    let local = chrono::DateTime::<chrono::Local>::from(time);
    u16::try_from(local.year())
        .ok()
        .and_then(|year| {
            zip::DateTime::from_date_and_time(
                year,
                local.month() as u8,
                local.day() as u8,
                local.hour() as u8,
                local.minute() as u8,
                local.second() as u8,
            )
            .ok()
        })
        .unwrap_or_default()
}

fn write_zip(file: fs::File, items: &[Item], meter: &mut Meter<'_>) -> io::Result<fs::File> {
    use zip::write::SimpleFileOptions;
    let mut writer = zip::ZipWriter::new(file);
    let mut buffer = vec![0; 256 << 10];
    for item in items {
        check(meter.cancellation)?;
        meter.state.path.clone_from(&item.path);
        let Some(name) = item.name.to_str() else {
            return Err(invalid(fl!(
                "files-archive-zip-name",
                name = item.name.to_string_lossy().into_owned()
            )));
        };
        let mut options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(item.metadata.mode() & 0o7777);
        if let Ok(modified) = item.metadata.modified() {
            options = options.last_modified_time(zip_time(modified));
        }
        match &item.kind {
            Kind::Directory => writer
                .add_directory(format!("{name}/"), options)
                .map_err(zip_error)?,
            Kind::Symlink(target) => {
                let Some(target) = target.to_str() else {
                    return Err(invalid(fl!(
                        "files-archive-zip-name",
                        name = item.name.to_string_lossy().into_owned()
                    )));
                };
                writer
                    .add_symlink(name, target, options)
                    .map_err(zip_error)?;
            }
            Kind::File => {
                let size = item.metadata.len();
                writer
                    .start_file(name, options.large_file(size >= u64::from(u32::MAX)))
                    .map_err(zip_error)?;
                let mut reader = Exact {
                    file: fs::File::open(&item.path)?,
                    remaining: size,
                    name: &item.name,
                    meter,
                };
                loop {
                    let count = reader.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    writer.write_all(&buffer[..count])?;
                }
            }
        }
        meter.state.entries += 1;
        meter.report();
    }
    writer.finish().map_err(zip_error)
}

fn zip_error(error: zip::result::ZipError) -> io::Error {
    use zip::result::ZipError;
    match error {
        ZipError::Io(error) => error,
        ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED) | ZipError::InvalidPassword => {
            invalid(fl!("files-archive-encrypted"))
        }
        ZipError::CompressionMethodNotSupported(_) | ZipError::UnsupportedArchive(_) => {
            io::Error::new(io::ErrorKind::Unsupported, fl!("files-archive-unsupported"))
        }
        _ => invalid(fl!("files-archive-damaged")),
    }
}

// Extraction.

/// Tracks how much of the archive file was consumed, for progress.
struct Counted<R> {
    inner: R,
    position: Rc<Cell<u64>>,
}

impl<R: Read> Read for Counted<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.position.set(self.position.get() + count as u64);
        Ok(count)
    }
}

impl<R: Seek> Seek for Counted<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = self.inner.seek(from)?;
        self.position.set(position);
        Ok(position)
    }
}

struct Extraction<'a> {
    root: PathBuf,
    destination: &'a Path,
    budget: u64,
    written: u64,
    limits: Limits,
    meter: Meter<'a>,
    consumed: Rc<Cell<u64>>,
    /// Folders whose final permissions and time are applied last, deepest first.
    folders: Vec<(PathBuf, u32, Option<SystemTime>)>,
    /// Symbolic links, created after every other entry.
    links: Vec<(PathBuf, PathBuf)>,
    skipped_unsafe: u64,
    skipped_special: u64,
}

/// The entry's path below the extraction root, or `None` when it would leave it.
/// An empty path such as `./` names the root itself.
fn enclosed(path: &Path) -> Option<PathBuf> {
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(relative)
}

/// Whether a symbolic link at `link` (relative to the root) pointing to `target`
/// resolves inside the root. `..` is accepted only before every named component,
/// so no step climbs out of a folder reached through another link.
fn link_inside(link: &Path, target: &Path) -> bool {
    if target.as_os_str().is_empty() || target.as_os_str().as_bytes().contains(&0) {
        return false;
    }
    let mut depth = link.components().count().saturating_sub(1);
    let mut named = false;
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if !named && depth > 0 => depth -= 1,
            Component::Normal(_) => named = true,
            _ => return false,
        }
    }
    true
}

impl Extraction<'_> {
    fn begin(&mut self, raw: &Path) -> io::Result<Option<PathBuf>> {
        check(self.meter.cancellation)?;
        self.meter.state.entries += 1;
        if self.meter.state.entries > self.limits.max_entries {
            return Err(invalid(fl!(
                "files-archive-too-many-entries",
                limit = self.limits.max_entries
            )));
        }
        self.meter.state.bytes = self.consumed.get();
        let Some(relative) = enclosed(raw) else {
            self.skipped_unsafe += 1;
            return Ok(None);
        };
        if relative.as_os_str().is_empty() {
            return Ok(None);
        }
        self.meter.state.path = self.destination.join(&relative);
        self.meter.report();
        Ok(Some(relative))
    }

    fn conflict(relative: &Path) -> io::Error {
        invalid(fl!(
            "files-archive-conflict",
            name = relative.to_string_lossy().into_owned()
        ))
    }

    /// Create the folders above `relative`, refusing anything but real folders
    /// on the way.
    fn parent(&mut self, relative: &Path) -> io::Result<PathBuf> {
        let mut current = self.root.clone();
        let mut walked = PathBuf::new();
        let components: Vec<_> = relative.components().collect();
        for component in &components[..components.len().saturating_sub(1)] {
            current.push(component);
            walked.push(component);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => return Err(Self::conflict(&walked)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::DirBuilder::new().mode(0o755).create(&current)?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(self.root.join(relative))
    }

    /// Make room for a non-folder entry, which replaces an earlier one with the
    /// same name, as a later tar member does.
    fn vacate(relative: &Path, path: &Path) -> io::Result<()> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => Err(Self::conflict(relative)),
            Ok(_) => fs::remove_file(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn folder(
        &mut self,
        relative: PathBuf,
        mode: u32,
        modified: Option<SystemTime>,
    ) -> io::Result<()> {
        let path = self.parent(&relative)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(Self::conflict(&relative)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::DirBuilder::new().mode(0o755).create(&path)?;
            }
            Err(error) => return Err(error),
        }
        self.folders.push((relative, mode, modified));
        Ok(())
    }

    fn file(
        &mut self,
        relative: PathBuf,
        reader: &mut dyn Read,
        mode: u32,
        modified: Option<SystemTime>,
    ) -> io::Result<()> {
        let path = self.parent(&relative)?;
        Self::vacate(&relative, &path)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let mut buffer = vec![0; 256 << 10];
        loop {
            check(self.meter.cancellation)?;
            let count = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            };
            self.written += count as u64;
            if self.written > self.budget {
                return Err(io::Error::new(
                    io::ErrorKind::StorageFull,
                    fl!("files-archive-no-space"),
                ));
            }
            file.write_all(&buffer[..count])?;
            self.meter.state.bytes = self.consumed.get();
            self.meter.report();
        }
        file.set_permissions(fs::Permissions::from_mode(mode & 0o777))?;
        if let Some(modified) = modified {
            let _ = file.set_modified(modified);
        }
        Ok(())
    }

    fn hard_link(&mut self, relative: PathBuf, target: &Path) -> io::Result<()> {
        let Some(target) = enclosed(target).filter(|target| !target.as_os_str().is_empty()) else {
            self.skipped_unsafe += 1;
            return Ok(());
        };
        let source = self.root.join(&target);
        // The target must be a regular file reached through real folders only.
        let mut walked = self.root.clone();
        for component in target.parent().into_iter().flat_map(Path::components) {
            walked.push(component);
            if !fs::symlink_metadata(&walked).is_ok_and(|metadata| metadata.is_dir()) {
                self.skipped_unsafe += 1;
                return Ok(());
            }
        }
        if !fs::symlink_metadata(&source).is_ok_and(|metadata| metadata.is_file()) {
            self.skipped_unsafe += 1;
            return Ok(());
        }
        let path = self.parent(&relative)?;
        Self::vacate(&relative, &path)?;
        fs::hard_link(&source, &path)
    }

    fn symlink(&mut self, relative: PathBuf, target: PathBuf) {
        if link_inside(&relative, &target) {
            self.links.push((relative, target));
        } else {
            self.skipped_unsafe += 1;
        }
    }

    fn special(&mut self) {
        self.skipped_special += 1;
    }

    fn finish(&mut self) -> io::Result<()> {
        for (relative, target) in std::mem::take(&mut self.links) {
            check(self.meter.cancellation)?;
            let path = self.parent(&relative)?;
            Self::vacate(&relative, &path)?;
            std::os::unix::fs::symlink(&target, &path)?;
        }
        let mut folders = std::mem::take(&mut self.folders);
        folders.sort_by_key(|(relative, ..)| std::cmp::Reverse(relative.components().count()));
        for (relative, mode, modified) in folders {
            let path = self.root.join(&relative);
            // The owner keeps access, so the result can be moved and removed.
            fs::set_permissions(&path, fs::Permissions::from_mode(mode & 0o777 | 0o700))?;
            if let Some(modified) = modified
                && let Ok(folder) = fs::File::open(&path)
            {
                let _ = folder.set_modified(modified);
            }
        }
        Ok(())
    }
}

fn available(destination: &Path) -> io::Result<u64> {
    let stat = rustix::fs::statvfs(destination)?;
    Ok(stat.f_bavail.saturating_mul(stat.f_frsize))
}

fn unix_time(seconds: u64) -> Option<SystemTime> {
    UNIX_EPOCH.checked_add(Duration::from_secs(seconds))
}

fn extract(
    archive: &Path,
    destination: &Path,
    partial: &Path,
    cancellation: &Cancellation,
    limits: Limits,
    progress: &mut dyn FnMut(&Progress),
) -> io::Result<Outcome> {
    let mut file = fs::File::open(archive)?;
    let size = file.metadata()?.len();
    let name = archive.file_name().unwrap_or_default();
    let format = Format::sniff(&mut file)?
        .or_else(|| Format::detect(name))
        .ok_or_else(|| invalid(fl!("files-archive-damaged")))?;
    let budget = match limits.max_bytes {
        Some(budget) => budget,
        None => available(destination)?.saturating_sub(limits.reserve),
    };
    fs::DirBuilder::new().mode(0o700).create(partial)?;
    let consumed = Rc::new(Cell::new(0));
    let mut extraction = Extraction {
        root: partial.to_owned(),
        destination,
        budget,
        written: 0,
        limits,
        meter: Meter {
            cancellation,
            progress,
            state: Progress {
                total_bytes: Some(size),
                ..Progress::default()
            },
        },
        consumed: consumed.clone(),
        folders: Vec::new(),
        links: Vec::new(),
        skipped_unsafe: 0,
        skipped_special: 0,
    };
    let reader = Counted {
        inner: io::BufReader::with_capacity(256 << 10, file),
        position: consumed,
    };
    if format == Format::Zip {
        read_zip(reader, &mut extraction)?;
    } else {
        let decoder: Box<dyn Read> = match format {
            Format::Tar => Box::new(reader),
            Format::TarGz => Box::new(flate2::read::MultiGzDecoder::new(reader)),
            Format::TarXz => Box::new(lzma_rust2::XzReader::new(reader, true)),
            Format::TarZst => Box::new(zstd::Decoder::new(reader)?),
            Format::TarBz2 => Box::new(bzip2::read::MultiBzDecoder::new(reader)),
            Format::Zip => unreachable!(),
        };
        read_tar(decoder, &mut extraction)?;
    }
    extraction.finish()?;
    let entries = extraction.meter.state.entries;
    let (skipped_unsafe, skipped_special) = (extraction.skipped_unsafe, extraction.skipped_special);
    let written = extraction.written;
    let mut top = fs::read_dir(partial)?;
    let first = top.next().transpose()?;
    let output = match (first, top.next().transpose()?) {
        (None, _) => return Err(invalid(fl!("files-archive-empty"))),
        (Some(only), None) => {
            let name = only.file_name();
            let directory = only.file_type()?.is_dir();
            let (base, suffix) = numbered_parts(&name, directory);
            let output = publish(&only.path(), destination, &base, &suffix)?;
            let _ = fs::remove_dir(partial);
            output
        }
        (Some(_), Some(_)) => {
            fs::set_permissions(partial, fs::Permissions::from_mode(0o755))?;
            publish(partial, destination, stem(name), OsStr::new(""))?
        }
    };
    Ok(Outcome {
        output,
        entries,
        bytes: written,
        skipped_unsafe,
        skipped_special,
    })
}

fn read_tar(decoder: Box<dyn Read>, extraction: &mut Extraction<'_>) -> io::Result<()> {
    let damaged = |error: io::Error| {
        if cancelled(&error) || error.kind() == io::ErrorKind::StorageFull {
            error
        } else {
            invalid(format!("{} ({error})", fl!("files-archive-damaged")))
        }
    };
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries().map_err(damaged)? {
        let mut entry = entry.map_err(damaged)?;
        let path = entry.path().map_err(damaged)?.into_owned();
        let header = entry.header();
        let kind = header.entry_type();
        let mode = header.mode().unwrap_or(0o644);
        let modified = header.mtime().ok().and_then(unix_time);
        let link = entry
            .link_name()
            .map_err(damaged)?
            .map(|link| link.into_owned());
        use tar::EntryType;
        if matches!(
            kind,
            EntryType::XGlobalHeader
                | EntryType::XHeader
                | EntryType::GNULongName
                | EntryType::GNULongLink
        ) {
            continue;
        }
        let Some(relative) = extraction.begin(&path)? else {
            continue;
        };
        match kind {
            EntryType::Directory => extraction.folder(relative, mode, modified)?,
            EntryType::Regular | EntryType::Continuous | EntryType::GNUSparse => extraction
                .file(relative, &mut entry, mode, modified)
                .map_err(damaged)?,
            EntryType::Symlink => match link {
                Some(target) => extraction.symlink(relative, target),
                None => extraction.special(),
            },
            EntryType::Link => match link {
                Some(target) => extraction.hard_link(relative, &target)?,
                None => extraction.special(),
            },
            _ => extraction.special(),
        }
    }
    Ok(())
}

fn zip_modified(time: zip::DateTime) -> Option<SystemTime> {
    use chrono::TimeZone;
    chrono::Local
        .with_ymd_and_hms(
            i32::from(time.year()),
            u32::from(time.month()),
            u32::from(time.day()),
            u32::from(time.hour()),
            u32::from(time.minute()),
            u32::from(time.second()),
        )
        .earliest()
        .map(SystemTime::from)
}

fn read_zip<R: Read + Seek>(reader: R, extraction: &mut Extraction<'_>) -> io::Result<()> {
    let mut archive = zip::ZipArchive::new(reader).map_err(zip_error)?;
    // Overlapping entries only serve to multiply output: a known bomb shape.
    if archive.has_overlapping_files().map_err(zip_error)? {
        return Err(invalid(fl!("files-archive-damaged")));
    }
    extraction.meter.state.total_entries = Some(archive.len() as u64);
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(zip_error)?;
        let name = entry.name().map_err(zip_error)?.into_owned();
        let directory = entry.is_dir();
        let symlink = entry.is_symlink();
        let mode = entry
            .unix_mode()
            .unwrap_or(if directory { 0o755 } else { 0o644 });
        let modified = entry.last_modified().and_then(zip_modified);
        let Some(relative) = extraction.begin(Path::new(&name))? else {
            continue;
        };
        if directory {
            extraction.folder(relative, mode, modified)?;
        } else if symlink {
            let mut target = Vec::new();
            (&mut entry).take(4096).read_to_end(&mut target)?;
            extraction.symlink(relative, PathBuf::from(OsString::from_vec(target)));
        } else {
            extraction.file(relative, &mut entry, mode, modified)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffixes_are_recognised_without_case() {
        assert_eq!(Format::detect(OsStr::new("a.TAR.GZ")), Some(Format::TarGz));
        assert_eq!(Format::detect(OsStr::new("a.tgz")), Some(Format::TarGz));
        assert_eq!(Format::detect(OsStr::new("a.tar")), Some(Format::Tar));
        assert_eq!(Format::detect(OsStr::new("a.Zip")), Some(Format::Zip));
        assert_eq!(Format::detect(OsStr::new(".zip")), None);
        assert_eq!(Format::detect(OsStr::new("a.gz")), None);
        assert_eq!(stem(OsStr::new("photos.tar.zst")), OsStr::new("photos"));
        assert_eq!(stem(OsStr::new("notes.txt")), OsStr::new("notes.txt"));
    }

    #[test]
    fn suggested_names_follow_the_selection() {
        assert_eq!(
            suggested_name(&[(OsStr::new("report.pdf"), false)]),
            OsString::from("report")
        );
        assert_eq!(
            suggested_name(&[(OsStr::new("v1.2"), true)]),
            OsString::from("v1.2")
        );
        assert_eq!(
            suggested_name(&[(OsStr::new("a"), false), (OsStr::new("b"), true)]),
            OsString::from("Archive")
        );
    }

    #[test]
    fn numbering_keeps_extensions() {
        assert_eq!(
            numbered_parts(OsStr::new("a.tar.gz"), false),
            ("a".into(), ".tar.gz".into())
        );
        assert_eq!(
            numbered_parts(OsStr::new("notes.txt"), false),
            ("notes".into(), ".txt".into())
        );
        assert_eq!(
            numbered_parts(OsStr::new("v1.2"), true),
            ("v1.2".into(), "".into())
        );
    }

    #[test]
    fn entries_and_links_stay_inside() {
        assert_eq!(enclosed(Path::new("./a/b")), Some(PathBuf::from("a/b")));
        assert_eq!(enclosed(Path::new("a/../../b")), None);
        assert_eq!(enclosed(Path::new("/etc/passwd")), None);
        assert!(link_inside(Path::new("a/b/link"), Path::new("../c")));
        assert!(link_inside(Path::new("a/link"), Path::new("../c/d")));
        assert!(!link_inside(Path::new("link"), Path::new("../c")));
        assert!(!link_inside(Path::new("a/link"), Path::new("b/../../..")));
        assert!(!link_inside(Path::new("a/link"), Path::new("b/..")));
        assert!(!link_inside(Path::new("link"), Path::new("/etc")));
    }
}
