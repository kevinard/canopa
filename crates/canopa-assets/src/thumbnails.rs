//! The freedesktop Thumbnail Managing Standard: the shared cache under
//! `$XDG_CACHE_HOME/thumbnails`, its PNG metadata, failure records, and the
//! installed external thumbnailers (`thumbnailers/*.thumbnailer`).
//!
//! Everything here is synchronous and bounded; callers run it on workers and
//! own scheduling, sandboxing and presentation. No paths or content are logged.
use std::collections::HashMap;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// A thumbnail stored or decoded from the cache is at most this large.
pub const MAX_THUMBNAIL_BYTES: usize = 16 * 1024 * 1024;
const MAX_THUMBNAILER_BYTES: usize = 64 * 1024;
const MAX_THUMBNAILERS: usize = 256;
const MAX_TEXT_CHUNK: usize = 64 * 1024;

/// The standard's size directories. A thumbnail fits a square of [`Flavor::side`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Flavor {
    Normal,
    Large,
    XLarge,
    XXLarge,
}

impl Flavor {
    pub const ALL: [Self; 4] = [Self::Normal, Self::Large, Self::XLarge, Self::XXLarge];

    pub fn side(self) -> u32 {
        match self {
            Self::Normal => 128,
            Self::Large => 256,
            Self::XLarge => 512,
            Self::XXLarge => 1024,
        }
    }

    pub fn directory(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Large => "large",
            Self::XLarge => "x-large",
            Self::XXLarge => "xx-large",
        }
    }

    /// The smallest flavor whose side covers `pixels`.
    pub fn for_pixels(pixels: u32) -> Self {
        Self::ALL
            .into_iter()
            .find(|flavor| flavor.side() >= pixels)
            .unwrap_or(Self::XXLarge)
    }

    /// This flavor, then the larger ones: a larger thumbnail may be scaled down.
    pub fn and_larger(self) -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().filter(move |flavor| *flavor >= self)
    }
}

/// The `file://` URI GLib's `g_filename_to_uri` builds. Thumbnail names are the
/// MD5 of this exact string, so the escaping must match other desktops':
/// unreserved characters plus `!$&'()*+,-./:=@_~` stay, every other byte is
/// `%XX` with uppercase hex. `None` for a relative path.
pub fn file_uri(path: &Path) -> Option<String> {
    if !path.is_absolute() {
        return None;
    }
    let mut uri = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"!$&'()*+,-./:=@_~".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    Some(uri)
}

/// Lowercase hexadecimal MD5, as thumbnail file names use it.
pub fn md5_hex(data: &[u8]) -> String {
    md5(data).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn md5(data: &[u8]) -> [u8; 16] {
    const SHIFTS: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let constants: [u32; 64] =
        std::array::from_fn(|index| ((index as f64 + 1.0).sin().abs() * 4_294_967_296.0) as u32);
    let mut state: [u32; 4] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u64).wrapping_mul(8)).to_le_bytes());
    for block in message.chunks_exact(64) {
        let words: [u32; 16] = std::array::from_fn(|index| {
            u32::from_le_bytes(block[index * 4..index * 4 + 4].try_into().unwrap())
        });
        let [mut a, mut b, mut c, mut d] = state;
        for round in 0..64 {
            let (mix, word) = match round / 16 {
                0 => ((b & c) | (!b & d), round),
                1 => ((d & b) | (!d & c), (5 * round + 1) % 16),
                2 => (b ^ c ^ d, (3 * round + 5) % 16),
                _ => (c ^ (b | !d), (7 * round) % 16),
            };
            let rotated = a
                .wrapping_add(mix)
                .wrapping_add(constants[round])
                .wrapping_add(words[word])
                .rotate_left(SHIFTS[round]);
            (a, b, c, d) = (d, b.wrapping_add(rotated), b, c);
        }
        for (value, add) in state.iter_mut().zip([a, b, c, d]) {
            *value = value.wrapping_add(add);
        }
    }
    let mut digest = [0; 16];
    for (chunk, value) in digest.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    digest
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// What a thumbnail says about its source file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub uri: Option<String>,
    pub mtime: Option<i64>,
    pub size: Option<u64>,
    pub mime: Option<String>,
    pub software: Option<String>,
}

/// The source file as the cache identifies it: URI, whole-second modification
/// time and size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub uri: String,
    pub mtime: i64,
    pub size: u64,
}

impl Source {
    /// Describe a local file. `None` when its path is relative or it is unreadable.
    pub fn of(path: &Path) -> Option<Self> {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::metadata(path).ok()?;
        Some(Self {
            uri: file_uri(path)?,
            mtime: metadata.mtime(),
            size: metadata.len(),
        })
    }

    /// The cache file name: `md5(uri).png`.
    pub fn file_name(&self) -> String {
        format!("{}.png", md5_hex(self.uri.as_bytes()))
    }
}

impl Metadata {
    /// Read `tEXt` and uncompressed `iTXt` `Thumb::` keys and `Software` from a
    /// PNG. `None` when the bytes are not a well-formed PNG chunk stream.
    pub fn read(png: &[u8]) -> Option<Self> {
        let mut metadata = Self::default();
        let mut rest = png.strip_prefix(b"\x89PNG\r\n\x1a\n")?;
        loop {
            if rest.len() < 12 {
                return None;
            }
            let length = u32::from_be_bytes(rest[..4].try_into().ok()?) as usize;
            let kind = &rest[4..8];
            let data = rest.get(8..8usize.checked_add(length)?)?;
            rest = rest.get(12 + length..)?;
            match kind {
                b"IEND" => return Some(metadata),
                b"tEXt" if length <= MAX_TEXT_CHUNK => {
                    if let Some((key, value)) = split_once(data, 0) {
                        metadata.set(key, &String::from_utf8_lossy(value));
                    }
                }
                b"iTXt" if length <= MAX_TEXT_CHUNK => {
                    // keyword \0 compression-flag method language \0 translated \0 text
                    if let Some((key, rest)) = split_once(data, 0)
                        && let [0, _, rest @ ..] = rest
                        && let Some((_, rest)) = split_once(rest, 0)
                        && let Some((_, text)) = split_once(rest, 0)
                        && let Ok(text) = std::str::from_utf8(text)
                    {
                        metadata.set(key, text);
                    }
                }
                _ => {}
            }
        }
    }

    fn set(&mut self, key: &[u8], value: &str) {
        match key {
            b"Thumb::URI" => self.uri = Some(value.to_owned()),
            b"Thumb::MTime" => self.mtime = value.trim().parse().ok(),
            b"Thumb::Size" => self.size = value.trim().parse().ok(),
            b"Thumb::Mimetype" => self.mime = Some(value.to_owned()),
            b"Software" => self.software = Some(value.to_owned()),
            _ => {}
        }
    }

    /// The standard's validity rule: the URI and modification time match. A
    /// recorded size must match too.
    pub fn describes(&self, source: &Source) -> bool {
        self.uri.as_deref() == Some(source.uri.as_str())
            && self.mtime == Some(source.mtime)
            && self.size.is_none_or(|size| size == source.size)
    }
}

fn split_once(bytes: &[u8], separator: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|&byte| byte == separator)?;
    Some((&bytes[..index], &bytes[index + 1..]))
}

/// Insert `tEXt` chunks right after `IHDR` of an encoded PNG.
pub fn with_text(png: &[u8], entries: &[(&str, &str)]) -> Option<Vec<u8>> {
    let signature = 8;
    let header_end = signature + 8 + 13 + 4;
    if png.len() < header_end || &png[signature + 4..signature + 8] != b"IHDR" {
        return None;
    }
    let mut output = png[..header_end].to_vec();
    for (key, value) in entries {
        if key.is_empty() || key.len() > 79 || key.contains('\0') || value.contains('\0') {
            return None;
        }
        let mut chunk = b"tEXt".to_vec();
        chunk.extend_from_slice(key.as_bytes());
        chunk.push(0);
        // tEXt is Latin-1; URIs and numbers are ASCII. Other text is dropped.
        chunk.extend(
            value
                .chars()
                .filter_map(|ch| u8::try_from(u32::from(ch)).ok()),
        );
        output.extend_from_slice(&((chunk.len() - 4) as u32).to_be_bytes());
        output.extend_from_slice(&chunk);
        output.extend_from_slice(&crc32(&chunk).to_be_bytes());
    }
    output.extend_from_slice(&png[header_end..]);
    Some(output)
}

/// Encode RGBA pixels as a cache thumbnail of `source`.
pub fn encode(
    width: u32,
    height: u32,
    rgba: &[u8],
    source: &Source,
    mime: Option<&str>,
    software: &str,
) -> Option<Vec<u8>> {
    use image::ImageEncoder;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .ok()?;
    let mtime = source.mtime.to_string();
    let size = source.size.to_string();
    let mut entries = vec![
        ("Thumb::URI", source.uri.as_str()),
        ("Thumb::MTime", mtime.as_str()),
        ("Thumb::Size", size.as_str()),
        ("Software", software),
    ];
    if let Some(mime) = mime {
        entries.push(("Thumb::Mimetype", mime));
    }
    with_text(&png, &entries)
}

/// One user's shared cache, normally `$XDG_CACHE_HOME/thumbnails`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cache {
    root: PathBuf,
}

/// What the cache knows about one source.
#[derive(Debug, PartialEq, Eq)]
pub enum Lookup {
    /// A valid thumbnail, with its flavor and encoded bytes.
    Found(Flavor, Vec<u8>),
    /// This application recorded a failure for this version of the file.
    Failed,
    Missing,
}

impl Cache {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// `$XDG_CACHE_HOME/thumbnails`, or `~/.cache/thumbnails`.
    pub fn user() -> Option<Self> {
        let cache = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .map(|home| home.join(".cache"))
            })?;
        Some(Self::new(cache.join("thumbnails")))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, flavor: Flavor, source: &Source) -> PathBuf {
        self.root.join(flavor.directory()).join(source.file_name())
    }

    /// `fail/<application>/md5(uri).png`. `application` is the standard's
    /// `<appname>-<version>`, so a newer release retries old failures.
    pub fn fail_path(&self, application: &str, source: &Source) -> PathBuf {
        self.root
            .join("fail")
            .join(application)
            .join(source.file_name())
    }

    /// True for a path inside this cache: thumbnails are never thumbnailed.
    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }

    /// The first valid thumbnail of `flavor` or larger, else this application's
    /// failure record for the same version. Invalid entries are ignored, not
    /// deleted: another program may be about to replace them.
    pub fn lookup(&self, flavor: Flavor, source: &Source, application: &str) -> Lookup {
        for flavor in flavor.and_larger() {
            if let Some(bytes) = read_valid(&self.path(flavor, source), source) {
                return Lookup::Found(flavor, bytes);
            }
        }
        if read_valid(&self.fail_path(application, source), source).is_some() {
            return Lookup::Failed;
        }
        Lookup::Missing
    }

    /// Store an encoded thumbnail: directories `0700`, the file `0600`,
    /// written to a temporary name in the same directory and renamed.
    pub fn store(&self, flavor: Flavor, source: &Source, png: &[u8]) -> io::Result<PathBuf> {
        let path = self.path(flavor, source);
        write_private(&path, png)?;
        Ok(path)
    }

    /// Record that `application` cannot thumbnail this version of the file:
    /// a 1×1 PNG carrying the source's metadata.
    pub fn record_failure(&self, application: &str, source: &Source) -> io::Result<()> {
        let png = encode(1, 1, &[0, 0, 0, 0], source, None, application)
            .ok_or_else(|| io::Error::other("failure record could not be encoded"))?;
        write_private(&self.fail_path(application, source), &png)
    }
}

fn read_valid(path: &Path, source: &Source) -> Option<Vec<u8>> {
    let bytes = canopa_file::read_canonical_bounded(path, MAX_THUMBNAIL_BYTES).ok()?;
    Metadata::read(&bytes)?.describes(source).then_some(bytes)
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no cache folder"))?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)?;
    let mut name = std::ffi::OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        TEMPORARY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let temporary = parent.join(name);
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(bytes)?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

static TEMPORARY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// One installed `[Thumbnailer Entry]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thumbnailer {
    /// The file name, which identifies it across data directories.
    pub id: String,
    pub try_exec: Option<String>,
    /// The `Exec` line split into arguments, field codes unexpanded.
    pub exec: Vec<String>,
    pub mime_types: Vec<String>,
}

impl Thumbnailer {
    pub fn parse(id: &str, source: &str) -> Option<Self> {
        let mut in_entry = false;
        let mut fields = HashMap::new();
        for line in source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') {
                in_entry = line == "[Thumbnailer Entry]";
                continue;
            }
            if in_entry && let Some((key, value)) = line.split_once('=') {
                fields
                    .entry(key.trim().to_owned())
                    .or_insert_with(|| value.trim().to_owned());
            }
        }
        let exec = split_exec(fields.get("Exec")?)?;
        let mime_types: Vec<String> = fields
            .get("MimeType")?
            .split(';')
            .map(str::trim)
            .filter(|mime| !mime.is_empty() && mime.contains('/'))
            .map(str::to_owned)
            .collect();
        if exec.is_empty() || mime_types.is_empty() {
            return None;
        }
        Some(Self {
            id: id.to_owned(),
            try_exec: fields
                .get("TryExec")
                .filter(|value| !value.is_empty())
                .cloned(),
            exec,
            mime_types,
        })
    }

    /// The arguments for one run: `%i` input path, `%u` input URI, `%o`
    /// output path, `%s` size in pixels, `%%` a percent sign. Unknown codes are
    /// dropped, like GNOME's thumbnail factory does.
    pub fn arguments(&self, input: &Path, uri: &str, output: &Path, size: u32) -> Vec<String> {
        let mut arguments = Vec::with_capacity(self.exec.len());
        for argument in &self.exec {
            let mut expanded = String::new();
            let mut chars = argument.chars();
            while let Some(ch) = chars.next() {
                if ch != '%' {
                    expanded.push(ch);
                    continue;
                }
                match chars.next() {
                    Some('i') => expanded.push_str(&input.to_string_lossy()),
                    Some('u') => expanded.push_str(uri),
                    Some('o') => expanded.push_str(&output.to_string_lossy()),
                    Some('s') => expanded.push_str(&size.to_string()),
                    Some('%') => expanded.push('%'),
                    _ => {}
                }
            }
            arguments.push(expanded);
        }
        arguments
    }
}

/// Split an `Exec` line like a POSIX shell word list without expansion:
/// whitespace separates, single and double quotes group, backslash escapes.
/// `None` for an unterminated quote.
pub fn split_exec(line: &str) -> Option<Vec<String>> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' => {
                started = true;
                loop {
                    match chars.next()? {
                        '\'' => break,
                        ch => current.push(ch),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => match chars.next()? {
                            ch @ ('"' | '\\' | '$' | '`') => current.push(ch),
                            ch => {
                                current.push('\\');
                                current.push(ch);
                            }
                        },
                        ch => current.push(ch),
                    }
                }
            }
            '\\' => {
                started = true;
                current.push(chars.next()?);
            }
            ch if ch.is_whitespace() => {
                if started {
                    arguments.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            ch => {
                started = true;
                current.push(ch);
            }
        }
    }
    if started {
        arguments.push(current);
    }
    Some(arguments)
}

/// Installed thumbnailers by MIME type.
#[derive(Clone, Debug, Default)]
pub struct Thumbnailers {
    by_type: HashMap<String, usize>,
    entries: Vec<Thumbnailer>,
}

impl Thumbnailers {
    /// `thumbnailers` under `$XDG_DATA_HOME` and each `$XDG_DATA_DIRS`.
    pub fn search_dirs() -> Vec<PathBuf> {
        let home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .map(|home| home.join(".local/share"))
            });
        let dirs = std::env::var_os("XDG_DATA_DIRS")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        home.into_iter()
            .chain(std::env::split_paths(&dirs).filter(|path| path.is_absolute()))
            .map(|dir| dir.join("thumbnailers"))
            .collect()
    }

    /// Load directories in precedence order. A file name in a higher directory
    /// hides the same name below; for one type, the first loaded entry wins.
    /// Entries whose `TryExec` program is missing are skipped.
    pub fn load(dirs: &[PathBuf], search_path: &[PathBuf]) -> Self {
        let mut loaded = Self::default();
        let mut names = std::collections::HashSet::new();
        for dir in dirs {
            let Ok(read) = std::fs::read_dir(dir) else {
                continue;
            };
            let mut files: Vec<PathBuf> = read
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "thumbnailer"))
                .collect();
            files.sort();
            for path in files {
                if loaded.entries.len() >= MAX_THUMBNAILERS {
                    return loaded;
                }
                let Some(id) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if !names.insert(id.to_owned()) {
                    continue;
                }
                let Some(entry) = canopa_file::read_bounded(&path, MAX_THUMBNAILER_BYTES)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .and_then(|source| Thumbnailer::parse(id, &source))
                else {
                    continue;
                };
                let installed = |program: &str| find_program(program, search_path).is_some();
                if entry
                    .try_exec
                    .as_deref()
                    .is_some_and(|program| !installed(program))
                    || !installed(&entry.exec[0])
                {
                    continue;
                }
                let index = loaded.entries.len();
                for mime in &entry.mime_types {
                    loaded.by_type.entry(mime.clone()).or_insert(index);
                }
                loaded.entries.push(entry);
            }
        }
        loaded
    }

    /// The thumbnailer for the first type in `types` (a type, then its aliases).
    pub fn for_types<'a>(&self, types: impl IntoIterator<Item = &'a str>) -> Option<&Thumbnailer> {
        types
            .into_iter()
            .find_map(|mime| self.by_type.get(mime))
            .map(|&index| &self.entries[index])
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn find_program(program: &str, search_path: &[PathBuf]) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let executable = |path: &Path| {
        path.metadata()
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    };
    if program.contains('/') {
        let path = PathBuf::from(program);
        return (path.is_absolute() && executable(&path)).then_some(path);
    }
    search_path
        .iter()
        .map(|dir| dir.join(program))
        .find(|path| executable(path))
}

/// The absolute program a thumbnailer runs, resolved on `search_path`.
pub fn resolve_program(thumbnailer: &Thumbnailer, search_path: &[PathBuf]) -> Option<PathBuf> {
    find_program(&thumbnailer.exec[0], search_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_the_rfc_1321_vectors() {
        for (input, digest) in [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ] {
            assert_eq!(md5_hex(input.as_bytes()), digest, "{input:?}");
        }
    }

    #[test]
    fn uris_escape_like_glib_and_name_the_cache_file() {
        assert_eq!(
            file_uri(Path::new("/home/jens/photos/me.png")).unwrap(),
            "file:///home/jens/photos/me.png"
        );
        assert_eq!(
            file_uri(Path::new("/tmp/a b#c%;?é[1]!$&'()*+,=:@~_-.")).unwrap(),
            "file:///tmp/a%20b%23c%25%3B%3F%C3%A9%5B1%5D!$&'()*+,=:@~_-."
        );
        assert!(file_uri(Path::new("relative.png")).is_none());
        // The standard's own example.
        let source = Source {
            uri: "file:///home/jens/photos/me.png".into(),
            mtime: 0,
            size: 0,
        };
        assert_eq!(source.file_name(), "c6ee772d9e49320e97ec29a7eb5b1697.png");
    }

    fn source() -> Source {
        Source {
            uri: "file:///tmp/photo.jpg".into(),
            mtime: 1_700_000_000,
            size: 42,
        }
    }

    #[test]
    fn encoded_thumbnails_carry_metadata_and_decode() {
        let png = encode(
            2,
            1,
            &[255, 0, 0, 255, 0, 255, 0, 128],
            &source(),
            Some("image/jpeg"),
            "test",
        )
        .unwrap();
        let metadata = Metadata::read(&png).unwrap();
        assert_eq!(metadata.uri.as_deref(), Some("file:///tmp/photo.jpg"));
        assert_eq!(metadata.mtime, Some(1_700_000_000));
        assert_eq!(metadata.size, Some(42));
        assert_eq!(metadata.mime.as_deref(), Some("image/jpeg"));
        assert_eq!(metadata.software.as_deref(), Some("test"));
        assert!(metadata.describes(&source()));
        let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.get_pixel(1, 0).0, [0, 255, 0, 128]);
        assert!(Metadata::read(b"not a png").is_none());
        assert!(Metadata::read(&png[..png.len() - 4]).is_none());
    }

    #[test]
    fn validity_requires_the_same_uri_mtime_and_any_recorded_size() {
        let metadata = Metadata {
            uri: Some(source().uri),
            mtime: Some(source().mtime),
            size: None,
            ..Metadata::default()
        };
        assert!(metadata.describes(&source()));
        let changed = |edit: fn(&mut Source)| {
            let mut other = source();
            edit(&mut other);
            other
        };
        assert!(!metadata.describes(&changed(|s| s.mtime += 1)));
        assert!(!metadata.describes(&changed(|s| s.uri.push('x'))));
        assert!(metadata.describes(&changed(|s| s.size = 7)));
        let sized = Metadata {
            size: Some(42),
            ..metadata.clone()
        };
        assert!(!sized.describes(&changed(|s| s.size = 7)));
        assert!(!Metadata::default().describes(&source()));
    }

    #[test]
    fn the_cache_finds_valid_larger_flavors_and_failure_records() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let cache = Cache::new(root.path().join("thumbnails"));
        let source = source();
        assert_eq!(
            cache.lookup(Flavor::Normal, &source, "app-1"),
            Lookup::Missing
        );
        let png = encode(1, 1, &[1, 2, 3, 255], &source, None, "test").unwrap();
        let stored = cache.store(Flavor::Large, &source, &png).unwrap();
        assert_eq!(
            stored,
            root.path()
                .join("thumbnails/large")
                .join(source.file_name())
        );
        assert_eq!(
            stored.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(stored.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            cache.lookup(Flavor::Normal, &source, "app-1"),
            Lookup::Found(Flavor::Large, png.clone())
        );
        assert_eq!(
            cache.lookup(Flavor::XLarge, &source, "app-1"),
            Lookup::Missing
        );
        // A stale normal thumbnail is skipped, not trusted.
        let stale = Source {
            mtime: 1,
            ..source.clone()
        };
        let old = encode(1, 1, &[0; 4], &stale, None, "test").unwrap();
        cache.store(Flavor::Normal, &source, &old).unwrap();
        assert_eq!(
            cache.lookup(Flavor::Normal, &source, "app-1"),
            Lookup::Found(Flavor::Large, png)
        );
        let video = Source {
            uri: "file:///tmp/video.mp4".into(),
            ..source.clone()
        };
        cache.record_failure("app-1", &video).unwrap();
        assert!(
            root.path()
                .join("thumbnails/fail/app-1")
                .join(video.file_name())
                .is_file()
        );
        assert_eq!(
            cache.lookup(Flavor::Normal, &video, "app-1"),
            Lookup::Failed
        );
        assert_eq!(
            cache.lookup(Flavor::Normal, &video, "app-2"),
            Lookup::Missing
        );
        let edited = Source {
            mtime: video.mtime + 5,
            ..video
        };
        assert_eq!(
            cache.lookup(Flavor::Normal, &edited, "app-1"),
            Lookup::Missing
        );
        assert!(cache.contains(&root.path().join("thumbnails/normal/x.png")));
        assert!(!cache.contains(&root.path().join("photo.png")));
        assert_eq!(Flavor::for_pixels(144), Flavor::Large);
        assert_eq!(Flavor::for_pixels(96), Flavor::Normal);
        assert_eq!(Flavor::for_pixels(5000), Flavor::XXLarge);
    }

    #[test]
    fn thumbnailer_entries_parse_and_expand_field_codes() {
        let entry = Thumbnailer::parse(
            "video.thumbnailer",
            "[Thumbnailer Entry]\nTryExec=ffmpegthumbnailer\nExec=ffmpegthumbnailer -i %i -o \"%o\" -s %s -t '10%%' -u %u%x\nMimeType=video/mp4;video/webm;",
        )
        .unwrap();
        assert_eq!(entry.mime_types, ["video/mp4", "video/webm"]);
        assert_eq!(
            entry.arguments(
                Path::new("/input/a b.mp4"),
                "file:///input/a%20b.mp4",
                Path::new("/output/t.png"),
                256
            ),
            [
                "ffmpegthumbnailer",
                "-i",
                "/input/a b.mp4",
                "-o",
                "/output/t.png",
                "-s",
                "256",
                "-t",
                "10%",
                "-u",
                "file:///input/a%20b.mp4"
            ]
        );
        assert!(Thumbnailer::parse("x", "[Thumbnailer Entry]\nExec=x %i\n").is_none());
        assert!(Thumbnailer::parse("x", "[Other]\nExec=x\nMimeType=a/b;\n").is_none());
        assert!(split_exec("a 'unterminated").is_none());
        assert_eq!(
            split_exec(r#"a\ b "c\"d" ''"#).unwrap(),
            ["a b", "c\"d", ""]
        );
    }

    #[test]
    fn thumbnailers_load_by_precedence_and_skip_missing_programs() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let program = bin.join("thumb");
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let user = root.path().join("user/thumbnailers");
        let system = root.path().join("system/thumbnailers");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&system).unwrap();
        let entry = |exec: &str, types: &str| {
            format!("[Thumbnailer Entry]\nExec={exec} %i %o\nMimeType={types}\n")
        };
        std::fs::write(
            user.join("a.thumbnailer"),
            entry("thumb --user", "image/png;"),
        )
        .unwrap();
        std::fs::write(
            system.join("a.thumbnailer"),
            entry("thumb --hidden", "video/mp4;"),
        )
        .unwrap();
        std::fs::write(
            system.join("b.thumbnailer"),
            entry("thumb --system", "image/png;image/gif;"),
        )
        .unwrap();
        std::fs::write(
            system.join("c.thumbnailer"),
            entry("missing", "image/tiff;"),
        )
        .unwrap();
        let loaded = Thumbnailers::load(&[user, system], std::slice::from_ref(&bin));
        assert_eq!(loaded.for_types(["image/png"]).unwrap().exec[1], "--user");
        assert_eq!(loaded.for_types(["image/gif"]).unwrap().exec[1], "--system");
        assert!(loaded.for_types(["video/mp4"]).is_none());
        assert!(loaded.for_types(["image/tiff"]).is_none());
        assert_eq!(
            loaded.for_types(["image/x-alias", "image/gif"]).unwrap().id,
            "b.thumbnailer"
        );
        assert_eq!(
            resolve_program(loaded.for_types(["image/png"]).unwrap(), &[bin]),
            Some(program)
        );
    }
}
