//! shared-mime-info detection without GLib: `globs2` weights and case rules,
//! `magic` content sniffing, aliases, subclasses and icon names.
//!
//! Loading reads bounded database files once; detection is CPU-only except
//! [`MimeDatabase::detect_path`], which reads at most a caller-chosen prefix.
//! Malformed lines are skipped; an over-budget file contributes nothing.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

pub const DIRECTORY: &str = "inode/directory";
pub const OCTET_STREAM: &str = "application/octet-stream";
pub const PLAIN_TEXT: &str = "text/plain";
pub const ZERO_SIZE: &str = "application/x-zerosize";
/// Enough for nearly every magic rule; ISO-style deep offsets are not sniffed.
pub const DEFAULT_SNIFF_BYTES: usize = 4096;

const MAX_DIRECTORIES: usize = 16;
const MAX_GLOBS_BYTES: usize = 4 * 1024 * 1024;
const MAX_MAGIC_BYTES: usize = 8 * 1024 * 1024;
const MAX_LIST_BYTES: usize = 1024 * 1024;
const MAX_GLOBS: usize = 65536;
const MAX_MATCHLETS: usize = 262_144;
const MAX_INDENT: u32 = 64;
const MAX_VALUE_BYTES: usize = 4096;
const MAX_RANGE: u32 = 1 << 20;
const MAX_SUBCLASS_DEPTH: usize = 64;
const REVALIDATE_AFTER: Duration = Duration::from_secs(2);

#[derive(Debug)]
struct Glob {
    mime: Arc<str>,
    weight: u32,
    length: usize,
    case_sensitive: bool,
    pattern: Vec<char>,
}

#[derive(Debug)]
struct Matchlet {
    offset: u32,
    range: u32,
    value: Vec<u8>,
    mask: Option<Vec<u8>>,
    children: Vec<Matchlet>,
}

impl Matchlet {
    fn matches(&self, data: &[u8]) -> bool {
        let start = self.offset as usize;
        let end = start.saturating_add(self.range as usize);
        let found = (start..end).any(|position| {
            let Some(window) = data.get(position..position + self.value.len()) else {
                return false;
            };
            match &self.mask {
                None => window == self.value.as_slice(),
                Some(mask) => window
                    .iter()
                    .zip(&self.value)
                    .zip(mask)
                    .all(|((byte, value), mask)| byte & mask == value & mask),
            }
        });
        found && (self.children.is_empty() || self.children.iter().any(|child| child.matches(data)))
    }
}

#[derive(Debug)]
struct MagicRule {
    priority: u32,
    mime: Arc<str>,
    matchlets: Vec<Matchlet>,
}

/// One loaded database. Values are immutable; [`MimeDatabase::system`] replaces
/// the shared copy when a database directory changes.
#[derive(Debug, Default)]
pub struct MimeDatabase {
    globs: Vec<Glob>,
    literal: HashMap<String, Vec<usize>>,
    suffix: HashMap<String, Vec<usize>>,
    complex: Vec<usize>,
    magic: Vec<MagicRule>,
    aliases: HashMap<String, Arc<str>>,
    parents: HashMap<String, Vec<Arc<str>>>,
    icons: HashMap<String, String>,
    generic_icons: HashMap<String, String>,
    names: HashMap<String, Arc<str>>,
}

impl MimeDatabase {
    /// `$XDG_DATA_HOME/mime`, then each `$XDG_DATA_DIRS/mime`, highest precedence first.
    pub fn search_dirs() -> Vec<PathBuf> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute());
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| home.map(|home| home.join(".local/share")));
        let data_dirs = std::env::var_os("XDG_DATA_DIRS")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        let mut dirs = Vec::new();
        for path in data_home
            .into_iter()
            .chain(std::env::split_paths(&data_dirs).filter(|path| path.is_absolute()))
        {
            let path = path.join("mime");
            if !dirs.contains(&path) && dirs.len() < MAX_DIRECTORIES {
                dirs.push(path);
            }
        }
        dirs
    }

    /// Load database directories in precedence order. A type described by a
    /// higher-precedence directory ignores lower globs and magic for that type.
    pub fn load(dirs: &[PathBuf]) -> Self {
        let mut database = Self::default();
        let mut globbed = HashSet::new();
        let mut magicked = HashSet::new();
        let mut matchlets = 0;
        for dir in dirs.iter().take(MAX_DIRECTORIES) {
            let mut here = HashSet::new();
            if let Some(source) = read_text(&dir.join("globs2"), MAX_GLOBS_BYTES) {
                database.parse_globs(&source, &globbed, &mut here);
            }
            globbed.extend(here);
            let mut here = HashSet::new();
            if let Ok(bytes) = crate::read_bounded(&dir.join("magic"), MAX_MAGIC_BYTES) {
                database.parse_magic(&bytes, &magicked, &mut here, &mut matchlets);
            }
            magicked.extend(here);
            for (file, pairs) in [
                ("aliases", 0usize),
                ("subclasses", 1),
                ("icons", 2),
                ("generic-icons", 3),
            ] {
                let Some(source) = read_text(&dir.join(file), MAX_LIST_BYTES) else {
                    continue;
                };
                for line in source.lines().filter(|line| !line.starts_with('#')) {
                    let separator = if pairs >= 2 { ':' } else { ' ' };
                    let Some((key, value)) = line.split_once(separator) else {
                        continue;
                    };
                    let (key, value) = (key.trim(), value.trim());
                    if !valid_mime(key) || value.is_empty() {
                        continue;
                    }
                    match pairs {
                        0 if valid_mime(value) => {
                            let value = database.intern(value);
                            database.aliases.entry(key.into()).or_insert(value);
                        }
                        1 if valid_mime(value) => {
                            let value = database.intern(value);
                            let parents = database.parents.entry(key.into()).or_default();
                            if !parents.contains(&value) && parents.len() < 64 {
                                parents.push(value);
                            }
                        }
                        2 => {
                            database.icons.entry(key.into()).or_insert(value.into());
                        }
                        3 => {
                            database
                                .generic_icons
                                .entry(key.into())
                                .or_insert(value.into());
                        }
                        _ => {}
                    }
                }
            }
        }
        // Stable: equal priorities keep file order, as update-mime-database wrote it.
        database
            .magic
            .sort_by_key(|rule| std::cmp::Reverse(rule.priority));
        database
    }

    /// Process-wide database for [`Self::search_dirs`], reloaded at most every two
    /// seconds when a database directory's modification time changes.
    pub fn system() -> Arc<Self> {
        struct Shared {
            database: Arc<MimeDatabase>,
            stamp: Vec<Option<SystemTime>>,
            dirs: Vec<PathBuf>,
            checked: Instant,
        }
        static SHARED: OnceLock<Mutex<Option<Shared>>> = OnceLock::new();
        let mut shared = SHARED
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(current) = shared.as_mut() {
            if current.checked.elapsed() < REVALIDATE_AFTER {
                return current.database.clone();
            }
            let dirs = Self::search_dirs();
            let stamp = stamp(&dirs);
            current.checked = Instant::now();
            if dirs == current.dirs && stamp == current.stamp {
                return current.database.clone();
            }
        }
        let dirs = Self::search_dirs();
        let stamp = stamp(&dirs);
        let database = Arc::new(Self::load(&dirs));
        *shared = Some(Shared {
            database: database.clone(),
            stamp,
            dirs,
            checked: Instant::now(),
        });
        database
    }

    fn intern(&mut self, mime: &str) -> Arc<str> {
        if let Some(value) = self.names.get(mime) {
            return value.clone();
        }
        let value: Arc<str> = Arc::from(mime);
        self.names.insert(mime.into(), value.clone());
        value
    }

    fn parse_globs(&mut self, source: &str, skip: &HashSet<String>, here: &mut HashSet<String>) {
        for line in source.lines() {
            if line.starts_with('#') || self.globs.len() >= MAX_GLOBS {
                continue;
            }
            let mut fields = line.splitn(4, ':');
            let (Some(weight), Some(mime), Some(pattern)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let flags = fields.next().unwrap_or("");
            let Ok(weight) = weight.parse::<u32>() else {
                continue;
            };
            if !valid_mime(mime) || skip.contains(mime) || weight > 100 {
                continue;
            }
            here.insert(mime.to_owned());
            if pattern == "__NOGLOBS__" || pattern.is_empty() || pattern.len() > 255 {
                continue;
            }
            let case_sensitive = flags.split(',').any(|flag| flag == "cs");
            let stored = if case_sensitive {
                pattern.to_owned()
            } else {
                pattern.to_lowercase()
            };
            let mime = self.intern(mime);
            let index = self.globs.len();
            let wildcard = |text: &str| text.contains(['*', '?', '[']);
            if !wildcard(&stored) {
                self.literal.entry(stored.clone()).or_default().push(index);
            } else if let Some(rest) = stored.strip_prefix('*')
                && !rest.is_empty()
                && !wildcard(rest)
            {
                self.suffix.entry(rest.to_owned()).or_default().push(index);
            } else {
                self.complex.push(index);
            }
            self.globs.push(Glob {
                mime,
                weight,
                length: stored.chars().count(),
                case_sensitive,
                pattern: stored.chars().collect(),
            });
        }
    }

    fn parse_magic(
        &mut self,
        bytes: &[u8],
        skip: &HashSet<String>,
        here: &mut HashSet<String>,
        matchlets: &mut usize,
    ) {
        let Some(mut rest) = bytes.strip_prefix(b"MIME-Magic\0\n") else {
            return;
        };
        // (indent, matchlet) stack for the current rule.
        type Pending = (u32, Arc<str>, Vec<(u32, Matchlet)>);
        let mut current: Option<Pending> = None;
        fn finish(database: &mut MimeDatabase, rule: Option<Pending>) {
            if let Some((priority, mime, flat)) = rule
                && !flat.is_empty()
            {
                database.magic.push(MagicRule {
                    priority,
                    mime,
                    matchlets: build_tree(flat),
                });
            }
        }
        while !rest.is_empty() {
            if rest[0] == b'[' {
                let Some(end) = rest.iter().position(|&byte| byte == b'\n') else {
                    break;
                };
                let header = std::str::from_utf8(&rest[1..end]).ok();
                rest = &rest[end + 1..];
                finish(self, current.take());
                let Some((priority, mime)) = header
                    .and_then(|header| header.strip_suffix(']'))
                    .and_then(|header| header.split_once(':'))
                else {
                    continue;
                };
                let Ok(priority) = priority.parse::<u32>() else {
                    continue;
                };
                if !valid_mime(mime) || skip.contains(mime) {
                    continue;
                }
                here.insert(mime.to_owned());
                let mime = self.intern(mime);
                current = Some((priority, mime, Vec::new()));
                continue;
            }
            let Some((matchlet, indent, consumed)) = parse_matchlet(rest) else {
                // Unknown or malformed line: skip to the next newline, per the spec.
                match rest.iter().position(|&byte| byte == b'\n') {
                    Some(end) => rest = &rest[end + 1..],
                    None => break,
                }
                continue;
            };
            rest = &rest[consumed..];
            if *matchlets >= MAX_MATCHLETS {
                continue;
            }
            if let Some((_, _, flat)) = current.as_mut() {
                *matchlets += 1;
                flat.push((indent, matchlet));
            }
        }
        finish(self, current.take());
    }

    /// Resolve an alias to its canonical type; unknown names are returned unchanged.
    pub fn unalias<'a>(&'a self, mime: &'a str) -> &'a str {
        self.aliases.get(mime).map_or(mime, |canonical| canonical)
    }

    /// Explicit parents plus the implicit `text/plain` and `application/octet-stream` rules.
    pub fn parents(&self, mime: &str) -> Vec<&str> {
        let mime = self.unalias(mime);
        let mut parents: Vec<&str> = self
            .parents
            .get(mime)
            .map(|parents| parents.iter().map(|parent| &**parent).collect())
            .unwrap_or_default();
        if mime.starts_with("text/") && mime != PLAIN_TEXT && !parents.contains(&PLAIN_TEXT) {
            parents.push(PLAIN_TEXT);
        }
        if !mime.starts_with("inode/") && mime != OCTET_STREAM && !parents.contains(&OCTET_STREAM) {
            parents.push(OCTET_STREAM);
        }
        parents
    }

    /// True when `mime` equals or inherits from `ancestor`, after unaliasing both.
    pub fn is_subclass(&self, mime: &str, ancestor: &str) -> bool {
        let ancestor = self.unalias(ancestor);
        let mut queue = vec![self.unalias(mime)];
        let mut seen = HashSet::new();
        while let Some(current) = queue.pop() {
            if current == ancestor {
                return true;
            }
            if !seen.insert(current) || seen.len() > MAX_SUBCLASS_DEPTH {
                continue;
            }
            queue.extend(self.parents(current));
        }
        false
    }

    /// Best glob matches for a file name: the highest weight, then the longest
    /// pattern, without duplicates. More than one result means the name is ambiguous.
    pub fn by_name(&self, file_name: &str) -> Vec<Arc<str>> {
        if file_name.is_empty() {
            return Vec::new();
        }
        let lower = file_name.to_lowercase();
        let mut found: Vec<usize> = Vec::new();
        let mut add = |indices: Option<&Vec<usize>>, lowered: bool| {
            for &index in indices.into_iter().flatten() {
                if (!lowered || !self.globs[index].case_sensitive) && !found.contains(&index) {
                    found.push(index);
                }
            }
        };
        add(self.literal.get(file_name), false);
        add(self.literal.get(&lower), true);
        for (text, lowered) in [(file_name, false), (lower.as_str(), true)] {
            for (position, _) in text.char_indices() {
                add(self.suffix.get(&text[position..]), lowered);
            }
        }
        let name: Vec<char> = file_name.chars().collect();
        let lower_name: Vec<char> = lower.chars().collect();
        for &index in &self.complex {
            let glob = &self.globs[index];
            let subject = if glob.case_sensitive {
                &name
            } else {
                &lower_name
            };
            if !found.contains(&index) && fnmatch(&glob.pattern, subject) {
                found.push(index);
            }
        }
        let Some(best) = found
            .iter()
            .map(|&index| (self.globs[index].weight, self.globs[index].length))
            .max()
        else {
            return Vec::new();
        };
        let mut result: Vec<Arc<str>> = Vec::new();
        for index in found {
            let glob = &self.globs[index];
            // Globs may name an alias of a type another glob names directly.
            let mime = self.aliases.get(&*glob.mime).unwrap_or(&glob.mime);
            if (glob.weight, glob.length) == best && !result.contains(mime) {
                result.push(mime.clone());
            }
        }
        result
    }

    /// The highest-priority magic rule matching the data, if any.
    pub fn by_data(&self, data: &[u8]) -> Option<Arc<str>> {
        self.magic
            .iter()
            .find(|rule| rule.matchlets.iter().any(|matchlet| matchlet.matches(data)))
            .map(|rule| rule.mime.clone())
    }

    /// Whether [`Self::detect`] would read content for this name: no glob or an ambiguous one.
    pub fn needs_data(&self, file_name: &str) -> bool {
        self.by_name(file_name).len() != 1
    }

    /// The specification's recommended order: a unique glob wins; otherwise magic
    /// chooses among ambiguous globs (a glob equal to or inheriting from the magic
    /// result wins), then the first glob, then a text/binary guess on the data.
    pub fn detect(&self, file_name: Option<&str>, data: Option<&[u8]>) -> Arc<str> {
        let globs = file_name.map(|name| self.by_name(name)).unwrap_or_default();
        if globs.len() == 1 {
            return globs[0].clone();
        }
        let magic = data.and_then(|data| self.by_data(data));
        if let Some(magic) = magic {
            return globs
                .iter()
                .find(|glob| self.is_subclass(glob, &magic))
                .or(globs.first())
                .cloned()
                .unwrap_or(magic);
        }
        if let Some(first) = globs.first() {
            return first.clone();
        }
        Arc::from(match data {
            Some([]) => ZERO_SIZE,
            Some(data) if looks_like_text(data) => PLAIN_TEXT,
            _ => OCTET_STREAM,
        })
    }

    /// Detect a local path. Directories and special files get `inode/*` types; a
    /// regular file's content is read (at most `sniff_bytes`) only when its name is
    /// missing or ambiguous. Symlinks are followed; dangling ones are `inode/symlink`.
    pub fn detect_path(&self, path: &Path, sniff_bytes: usize) -> Arc<str> {
        use std::os::unix::fs::FileTypeExt;
        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(_) if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_symlink()) => {
                return Arc::from("inode/symlink");
            }
            Err(_) => return Arc::from(OCTET_STREAM),
        };
        let kind = metadata.file_type();
        let special = if kind.is_dir() {
            Some(DIRECTORY)
        } else if kind.is_char_device() {
            Some("inode/chardevice")
        } else if kind.is_block_device() {
            Some("inode/blockdevice")
        } else if kind.is_fifo() {
            Some("inode/fifo")
        } else if kind.is_socket() {
            Some("inode/socket")
        } else {
            None
        };
        if let Some(special) = special {
            return Arc::from(special);
        }
        let name = path.file_name().map(|name| name.to_string_lossy());
        if let Some(name) = &name
            && !self.needs_data(name)
        {
            return self.detect(Some(name), None);
        }
        let data = if metadata.len() == 0 {
            Some(Vec::new())
        } else {
            crate::read_prefix(path, sniff_bytes).ok()
        };
        self.detect(name.as_deref(), data.as_deref())
    }

    /// Icon names in lookup order: the database `icon`, the type with `/` replaced
    /// by `-`, then its `generic-icon` or `media-x-generic`.
    pub fn icon_names(&self, mime: &str) -> Vec<String> {
        let mime = self.unalias(mime);
        let mut names = Vec::new();
        if let Some(icon) = self.icons.get(mime) {
            names.push(icon.clone());
        }
        let flat = mime.replace('/', "-");
        if !names.contains(&flat) {
            names.push(flat);
        }
        let generic = self.generic_icons.get(mime).cloned().unwrap_or_else(|| {
            format!(
                "{}-x-generic",
                mime.split('/').next().unwrap_or("application")
            )
        });
        if !names.contains(&generic) {
            names.push(generic);
        }
        names
    }
}

fn build_tree(flat: Vec<(u32, Matchlet)>) -> Vec<Matchlet> {
    // Each matchlet's parent is the closest preceding matchlet with a lower indent.
    let mut roots = Vec::new();
    let mut stack: Vec<(u32, Matchlet)> = Vec::new();
    fn attach(stack: &mut Vec<(u32, Matchlet)>, roots: &mut Vec<Matchlet>, indent: u32) {
        while stack.last().is_some_and(|(top, _)| *top >= indent) {
            let (_, done) = stack.pop().unwrap();
            match stack.last_mut() {
                Some((_, parent)) => parent.children.push(done),
                None => roots.push(done),
            }
        }
    }
    for (indent, matchlet) in flat {
        attach(&mut stack, &mut roots, indent);
        if indent > 0 && stack.is_empty() {
            continue; // orphaned child of a dropped line
        }
        stack.push((indent, matchlet));
    }
    attach(&mut stack, &mut roots, 0);
    roots
}

fn parse_matchlet(line: &[u8]) -> Option<(Matchlet, u32, usize)> {
    let mut position = 0;
    let decimal = |position: &mut usize| -> Option<u32> {
        let start = *position;
        while line.get(*position).is_some_and(u8::is_ascii_digit) {
            *position += 1;
        }
        std::str::from_utf8(&line[start..*position])
            .ok()?
            .parse()
            .ok()
    };
    let indent = if line.first() == Some(&b'>') {
        0
    } else {
        decimal(&mut position)?
    };
    if line.get(position) != Some(&b'>') || indent > MAX_INDENT {
        return None;
    }
    position += 1;
    let offset = decimal(&mut position)?;
    if line.get(position) != Some(&b'=') {
        return None;
    }
    position += 1;
    let length = usize::from(u16::from_be_bytes([
        *line.get(position)?,
        *line.get(position + 1)?,
    ]));
    position += 2;
    if length == 0 || length > MAX_VALUE_BYTES {
        return None;
    }
    let mut value = line.get(position..position + length)?.to_vec();
    position += length;
    let mut mask = None;
    if line.get(position) == Some(&b'&') {
        mask = Some(line.get(position + 1..position + 1 + length)?.to_vec());
        position += 1 + length;
    }
    let mut word_size = 1;
    if line.get(position) == Some(&b'~') {
        position += 1;
        word_size = decimal(&mut position)?;
    }
    let mut range = 1;
    if line.get(position) == Some(&b'+') {
        position += 1;
        range = decimal(&mut position)?;
    }
    if line.get(position) != Some(&b'\n') || range == 0 || range > MAX_RANGE {
        return None;
    }
    // Values are stored big-endian; host-endian word matches swap on little-endian.
    if cfg!(target_endian = "little") && matches!(word_size, 2 | 4) {
        let size = word_size as usize;
        if length % size != 0 {
            return None;
        }
        value.chunks_exact_mut(size).for_each(<[u8]>::reverse);
        if let Some(mask) = mask.as_mut() {
            mask.chunks_exact_mut(size).for_each(<[u8]>::reverse);
        }
    }
    Some((
        Matchlet {
            offset,
            range,
            value,
            mask,
            children: Vec::new(),
        },
        indent,
        position + 1,
    ))
}

/// fnmatch(3) without flags for `*`, `?` and bracket expressions (`!`/`^` negation, ranges).
fn fnmatch(pattern: &[char], name: &[char]) -> bool {
    let (mut p, mut n) = (0, 0);
    let mut backtrack: Option<(usize, usize)> = None;
    while n < name.len() {
        let step = match pattern.get(p) {
            Some('*') => {
                backtrack = Some((p, n));
                p += 1;
                continue;
            }
            Some('?') => Some(p + 1),
            Some('[') => match bracket(pattern, p, name[n]) {
                Some(step) => step,
                // An unterminated bracket is a literal '[' in fnmatch.
                None => (name[n] == '[').then_some(p + 1),
            },
            Some(&character) => (character == name[n]).then_some(p + 1),
            None => None,
        };
        match step {
            Some(next) => {
                p = next;
                n += 1;
            }
            None => match backtrack {
                Some((star, matched)) => {
                    p = star + 1;
                    n = matched + 1;
                    backtrack = Some((star, matched + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&character| character == '*')
}

/// `None` when the bracket is unterminated; otherwise the next index if it matched.
fn bracket(pattern: &[char], start: usize, character: char) -> Option<Option<usize>> {
    let mut index = start + 1;
    let negated = matches!(pattern.get(index), Some('!' | '^'));
    if negated {
        index += 1;
    }
    let mut matched = false;
    let mut first = true;
    loop {
        let current = *pattern.get(index)?;
        if current == ']' && !first {
            break;
        }
        first = false;
        if pattern.get(index + 1) == Some(&'-')
            && let Some(&end) = pattern.get(index + 2)
            && end != ']'
        {
            matched |= (current..=end).contains(&character);
            index += 3;
        } else {
            matched |= current == character;
            index += 1;
        }
    }
    Some((matched != negated).then_some(index + 1))
}

fn looks_like_text(data: &[u8]) -> bool {
    data.iter()
        .take(128)
        .all(|&byte| byte >= 0x20 || matches!(byte, b'\t' | b'\n' | b'\r' | 0x0c | 0x1b | 0x08))
}

fn valid_mime(value: &str) -> bool {
    value.len() <= 255
        && value.split_once('/').is_some_and(|(media, subtype)| {
            !media.is_empty()
                && !subtype.is_empty()
                && !subtype.contains('/')
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_graphic() && !matches!(byte, b':' | b'[' | b']'))
        })
}

fn read_text(path: &Path, maximum: usize) -> Option<String> {
    String::from_utf8(crate::read_bounded(path, maximum).ok()?).ok()
}

fn stamp(dirs: &[PathBuf]) -> Vec<Option<SystemTime>> {
    // update-mime-database replaces its outputs by rename, which updates the directory.
    dirs.iter()
        .map(|dir| std::fs::metadata(dir).and_then(|meta| meta.modified()).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn magic(rules: &[(&str, &[u8])]) -> Vec<u8> {
        let mut bytes = b"MIME-Magic\0\n".to_vec();
        for (header, body) in rules {
            bytes.extend_from_slice(header.as_bytes());
            bytes.push(b'\n');
            bytes.extend_from_slice(body);
        }
        bytes
    }

    fn matchlet(indent: &str, offset: u32, value: &[u8], suffix: &[u8]) -> Vec<u8> {
        let mut bytes = format!("{indent}>{offset}=").into_bytes();
        bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
        bytes.extend_from_slice(value);
        bytes.extend_from_slice(suffix);
        bytes.push(b'\n');
        bytes
    }

    fn fixture() -> (tempfile::TempDir, MimeDatabase) {
        let temp = tempfile::tempdir().unwrap();
        let user = temp.path().join("user/mime");
        let system = temp.path().join("system/mime");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&system).unwrap();
        std::fs::write(
            system.join("globs2"),
            "# generated\n\
             80:text/html:*.html\n\
             50:text/x-python:*.py\n\
             50:application/msword:*.doc\n\
             50:text/x-doc-notes:*.doc\n\
             50:application/x-msword:*.doc\n\
             50:application/x-compressed-tar:*.tar.gz\n\
             50:application/gzip:*.gz\n\
             50:text/x-makefile:makefile\n\
             50:application/x-core:core:cs\n\
             50:application/x-sharedlib:*.so.[0-9]*\n\
             50:image/png:*.png\n\
             50:application/x-old:*.old\n",
        )
        .unwrap();
        // Higher-precedence user database overrides the system globs for one type.
        std::fs::write(
            user.join("globs2"),
            "50:application/x-old:__NOGLOBS__\n60:text/x-readme:readme\n",
        )
        .unwrap();
        let png = matchlet("", 0, b"\x89PNG", b"");
        let ole = matchlet("", 0, b"\xd0\xcf\x11\xe0", b"");
        let mut script = matchlet("", 0, b"#!", b"");
        script.extend(matchlet("1", 2, b"/bin/sh", b"+8"));
        std::fs::write(
            system.join("magic"),
            magic(&[
                ("[50:image/png]", &png),
                ("[50:application/x-ole-storage]", &ole),
                ("[60:application/x-shellscript]", &script),
                (
                    "[40:application/x-masked]",
                    &matchlet("", 0, b"\xa0", b"&\xf0"),
                ),
                (
                    "[30:application/x-word16]",
                    &matchlet("", 0, b"\x12\x34", b"~2"),
                ),
            ]),
        )
        .unwrap();
        std::fs::write(
            system.join("aliases"),
            "application/x-msword application/msword\n",
        )
        .unwrap();
        std::fs::write(
            system.join("subclasses"),
            "application/msword application/x-ole-storage\n\
             application/x-compressed-tar application/gzip\n",
        )
        .unwrap();
        std::fs::write(system.join("icons"), "text/x-python:custom-python\n").unwrap();
        std::fs::write(
            system.join("generic-icons"),
            "text/x-python:text-x-script\ninode/directory:folder\n",
        )
        .unwrap();
        let database = MimeDatabase::load(&[user, system]);
        (temp, database)
    }

    fn names(values: Vec<Arc<str>>) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn globs_use_weight_length_and_case_rules() {
        let (_temp, database) = fixture();
        assert_eq!(names(database.by_name("Index.HTML")), ["text/html"]);
        assert_eq!(
            names(database.by_name("backup.tar.gz")),
            ["application/x-compressed-tar"]
        );
        assert_eq!(names(database.by_name("Makefile")), ["text/x-makefile"]);
        assert_eq!(names(database.by_name("core")), ["application/x-core"]);
        assert!(database.by_name("CORE").is_empty());
        assert_eq!(
            names(database.by_name("libfoo.so.6")),
            ["application/x-sharedlib"]
        );
        assert!(database.by_name("libfoo.so.x").is_empty());
        assert_eq!(names(database.by_name("README")), ["text/x-readme"]);
        assert!(database.by_name("file.old").is_empty());
        assert_eq!(
            names(database.by_name("letter.doc")),
            ["application/msword", "text/x-doc-notes"]
        );
        assert!(database.needs_data("letter.doc"));
        assert!(database.needs_data("noext"));
        assert!(!database.needs_data("a.py"));
    }

    #[test]
    fn magic_resolves_missing_and_ambiguous_names_through_subclasses() {
        let (_temp, database) = fixture();
        assert_eq!(
            &*database.detect(Some("photo"), Some(b"\x89PNG\r\n")),
            "image/png"
        );
        assert_eq!(
            &*database.detect(Some("letter.doc"), Some(b"\xd0\xcf\x11\xe0rest")),
            "application/msword"
        );
        assert_eq!(
            &*database.detect(Some("letter.doc"), Some(b"plain words")),
            "application/msword"
        );
        // A unique glob wins without reading content.
        assert_eq!(
            &*database.detect(Some("a.png"), Some(b"#!/bin/sh")),
            "image/png"
        );
        assert_eq!(
            &*database.detect(Some("run"), Some(b"#!  /bin/sh\n")),
            "application/x-shellscript"
        );
        assert_eq!(
            &*database.detect(Some("run"), Some(b"#!/usr/bin/perl")),
            PLAIN_TEXT
        );
        assert_eq!(
            &*database.detect(None, Some(b"\xa7xx")),
            "application/x-masked"
        );
        let word = if cfg!(target_endian = "little") {
            b"\x34\x12"
        } else {
            b"\x12\x34"
        };
        assert_eq!(&*database.detect(None, Some(word)), "application/x-word16");
        assert_eq!(
            &*database.detect(None, Some(b"\x00\x01binary")),
            OCTET_STREAM
        );
        assert_eq!(&*database.detect(Some("empty"), Some(b"")), ZERO_SIZE);
        assert_eq!(&*database.detect(None, None), OCTET_STREAM);
    }

    #[test]
    fn aliases_subclasses_and_icon_names_follow_the_database() {
        let (_temp, database) = fixture();
        assert_eq!(
            database.unalias("application/x-msword"),
            "application/msword"
        );
        assert!(database.is_subclass("application/x-msword", "application/x-ole-storage"));
        assert!(database.is_subclass("text/x-python", PLAIN_TEXT));
        assert!(database.is_subclass("image/png", OCTET_STREAM));
        assert!(!database.is_subclass(DIRECTORY, OCTET_STREAM));
        assert!(!database.is_subclass("application/gzip", "application/x-compressed-tar"));
        assert_eq!(
            database.icon_names("text/x-python"),
            ["custom-python", "text-x-python", "text-x-script"]
        );
        assert_eq!(
            database.icon_names("application/x-msword"),
            ["application-msword", "application-x-generic"]
        );
        assert_eq!(
            database.icon_names(DIRECTORY),
            ["inode-directory", "folder"]
        );
    }

    #[test]
    fn paths_sniff_only_when_the_name_is_not_enough() {
        let (temp, database) = fixture();
        let dir = temp.path().join("files");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("photo"), b"\x89PNG\r\n\x1a\n").unwrap();
        std::fs::write(dir.join("script.py"), b"\x89PNG").unwrap();
        std::fs::write(dir.join("empty"), b"").unwrap();
        std::os::unix::fs::symlink(dir.join("missing"), dir.join("dangling")).unwrap();
        assert_eq!(&*database.detect_path(&dir.join("photo"), 64), "image/png");
        assert_eq!(
            &*database.detect_path(&dir.join("script.py"), 64),
            "text/x-python"
        );
        assert_eq!(&*database.detect_path(&dir.join("empty"), 64), ZERO_SIZE);
        assert_eq!(&*database.detect_path(&dir, 64), DIRECTORY);
        assert_eq!(
            &*database.detect_path(&dir.join("dangling"), 64),
            "inode/symlink"
        );
        // Truncated sniffing never reads past the limit.
        assert_ne!(&*database.detect_path(&dir.join("photo"), 2), "image/png");
    }

    #[test]
    fn malformed_database_lines_are_skipped() {
        let temp = tempfile::tempdir().unwrap();
        let mut bytes = b"MIME-Magic\0\n[50:image/x-good]\n".to_vec();
        bytes.extend_from_slice(b"garbage line\n");
        bytes.extend(matchlet("", 0, b"GOOD", b""));
        bytes.extend_from_slice(b"[bad header]\n");
        bytes.extend(matchlet("", 0, b"ORPHAN", b""));
        std::fs::write(temp.path().join("magic"), bytes).unwrap();
        std::fs::write(
            temp.path().join("globs2"),
            "x:image/x-good:*.good\n50:not-a-type:*.bad\n50:image/x-good:*.good:cs\n",
        )
        .unwrap();
        let database = MimeDatabase::load(&[temp.path().to_owned()]);
        assert_eq!(&*database.detect(None, Some(b"GOOD")), "image/x-good");
        assert_eq!(&*database.detect(None, Some(b"ORPHAN")), PLAIN_TEXT);
        assert!(database.by_name("x.bad").is_empty());
        assert!(database.by_name("x.GOOD").is_empty());
        assert_eq!(names(database.by_name("x.good")), ["image/x-good"]);
    }

    #[test]
    fn fnmatch_handles_classes_ranges_and_backtracking() {
        let matches = |pattern: &str, name: &str| {
            fnmatch(
                &pattern.chars().collect::<Vec<_>>(),
                &name.chars().collect::<Vec<_>>(),
            )
        };
        assert!(matches("*.anim[1-9j]", "a.anim7"));
        assert!(matches("*.anim[1-9j]", "a.animj"));
        assert!(!matches("*.anim[1-9j]", "a.anim0"));
        assert!(matches("[!a]*", "bcd"));
        assert!(!matches("[!a]*", "abc"));
        assert!(matches("a*b*c", "aXbYbZc"));
        assert!(matches("?ore", "core"));
        assert!(matches("x[", "x["));
    }
}
