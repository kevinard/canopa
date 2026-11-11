//! Bounded freedesktop icon-theme lookup, UI-free.
//!
//! [`IconLookup`] is one synchronous batch whose metadata lives only as long as the
//! value. [`IconResolver`] adds the process-wide desktop selection, a result cache
//! keyed by names, size, scale and theme, and modification-time invalidation.
use canopa_file::read_bounded;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const MAX_ROOTS: usize = 32;
const MAX_THEMES: usize = 32;
const MAX_DIRECTORIES: usize = 1024;
const MAX_INDEX_BYTES: usize = 256 * 1024;
const MAX_NAMES: usize = 8;
const MAX_CANDIDATES: usize = 32;
const MAX_CACHED: usize = 4096;
const REVALIDATE_AFTER: Duration = Duration::from_secs(2);
/// Extensions are explicit: raster-only consumers must not receive SVG paths.
#[derive(Clone, Copy)]
pub enum IconFormats {
    PngJpeg,
    PngSvgXpm,
}

impl IconFormats {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::PngJpeg => &["png", "jpg", "jpeg"],
            Self::PngSvgXpm => &["png", "svg", "xpm"],
        }
    }
}

/// Selection policy; source scale does not change widget geometry.
pub struct LookupOptions<'a> {
    pub theme: &'a str,
    pub scale: u16,
    pub formats: IconFormats,
    /// Compatibility for hicolor installations without any index.theme. A bad
    /// existing index always stops lookup; it never activates this fallback.
    pub allow_unindexed_hicolor: bool,
}
type LookupResult<T> = Result<T, ()>;

/// Ordered absolute XDG icon roots, legacy ~/.icons, then unthemed pixmap roots.
/// Invalid relative paths are ignored; an overflow sentinel disables lookup.
pub fn search_roots() -> Vec<PathBuf> {
    let user_home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| user_home.as_ref().map(|path| path.join(".local/share")));
    let value = std::env::var_os("XDG_DATA_DIRS")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let mut data_dirs = Vec::new();
    for path in std::env::split_paths(&value) {
        push_root(&mut data_dirs, path);
        if data_dirs.len() > MAX_ROOTS {
            break;
        }
    }
    let mut roots = Vec::new();
    if let Some(path) = &data_home {
        push_root(&mut roots, path.join("icons"));
    }
    if let Some(path) = &user_home {
        push_root(&mut roots, path.join(".icons"));
    }
    for path in &data_dirs {
        push_root(&mut roots, path.join("icons"));
    }
    if let Some(path) = data_home {
        push_root(&mut roots, path.join("pixmaps"));
    }
    for path in data_dirs {
        push_root(&mut roots, path.join("pixmaps"));
    }
    roots
}

fn push_root(roots: &mut Vec<PathBuf>, path: PathBuf) {
    // Keep one overflow sentinel so excess roots disable lookup instead of silently
    // changing precedence. Invalid XDG paths are ignored, never made relative to cwd.
    if roots.len() <= MAX_ROOTS
        && path.is_absolute()
        && path.as_os_str().len() <= 4096
        && !roots.contains(&path)
    {
        roots.push(path);
    }
}

/// One synchronous lookup batch. At most 32 roots/themes, 1,024 directories per
/// index, 256 KiB per index, 2 MiB total index bytes and 65,536 checks are admitted.
/// Metadata is cached only in this value; callers choose its lifetime and executor.
/// Cancellation and malformed/over-budget metadata stop lookup without falling
/// through to a lower-priority override. Returned paths are not decoded or cached.
///
/// Order follows the Icon Theme Specification for a list of names: the selected
/// theme, then its inherited themes depth-first, then hicolor, then unthemed
/// pixmaps. Within each theme every candidate name is tried before moving on.
pub struct IconLookup<'a> {
    roots: &'a [PathBuf],
    options: LookupOptions<'a>,
    cancelled: &'a dyn Fn() -> bool,
    generic_fallback: bool,
    checks: usize,
    index_bytes: usize,
    metadata: HashMap<String, Arc<Theme>>,
    stopped: bool,
}

impl<'a> IconLookup<'a> {
    pub fn new(
        roots: &'a [PathBuf],
        options: LookupOptions<'a>,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        let stopped = roots.len() > MAX_ROOTS
            || !valid_theme(options.theme)
            || !(1..=8).contains(&options.scale);
        Self {
            roots,
            options,
            cancelled,
            generic_fallback: false,
            checks: 65536,
            index_bytes: 2 * 1024 * 1024,
            metadata: HashMap::new(),
            stopped,
        }
    }

    /// After the given names, also try their generic forms: for `a-b-symbolic`,
    /// `a-symbolic`, `a-b` and `a`; for `a-b-c`, `a-b` and `a`.
    pub fn with_generic_fallback(mut self, enabled: bool) -> Self {
        self.generic_fallback = enabled;
        self
    }

    fn with_metadata(mut self, metadata: HashMap<String, Arc<Theme>>) -> Self {
        self.metadata = metadata;
        self
    }

    /// True once cancellation, a budget or malformed metadata stopped lookup.
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    /// Resolve the first available of up to eight ordered names.
    pub fn resolve(&mut self, names: &[String], size: u16) -> Option<PathBuf> {
        let names = candidate_names(names, self.generic_fallback);
        if names.is_empty() {
            return None;
        }
        match self.resolve_names(&names, size) {
            Ok(path) => path,
            Err(()) => {
                self.stopped = true;
                None
            }
        }
    }

    fn resolve_names(&mut self, names: &[String], size: u16) -> LookupResult<Option<PathBuf>> {
        let mut chain = Vec::new();
        self.chain(self.options.theme, &mut chain)?;
        if !chain.iter().any(|theme| theme == "hicolor") {
            chain.push("hicolor".to_owned());
        }
        for theme in &chain {
            if let Some(path) = self.in_theme(theme, names, size)? {
                return Ok(Some(path));
            }
        }
        for name in names {
            for root in self.roots {
                for extension in self.options.formats.extensions() {
                    let path = root.join(format!("{name}.{extension}"));
                    if self.exists(&path)? {
                        return Ok(Some(path));
                    }
                }
            }
        }
        Ok(None)
    }

    /// The selected theme and its inherited themes, depth-first, without hicolor:
    /// the required fallback comes after the complete explicit parent tree.
    fn chain(&mut self, theme: &str, chain: &mut Vec<String>) -> LookupResult<()> {
        self.check()?;
        if chain.iter().any(|seen| seen == theme) {
            return Ok(());
        }
        if chain.len() >= MAX_THEMES {
            return Err(());
        }
        chain.push(theme.to_owned());
        if theme == "hicolor" {
            return Ok(());
        }
        let metadata = self.theme(theme)?;
        for parent in &metadata.parents {
            if parent != "hicolor" {
                self.chain(parent, chain)?;
            }
        }
        Ok(())
    }

    fn in_theme(
        &mut self,
        theme: &str,
        names: &[String],
        size: u16,
    ) -> LookupResult<Option<PathBuf>> {
        let metadata = self.theme(theme)?;
        if theme == "hicolor" && !metadata.indexed && self.options.allow_unindexed_hicolor {
            for name in names {
                if let Some(path) = self.unindexed_hicolor(name)? {
                    return Ok(Some(path));
                }
            }
            return Ok(None);
        }
        let mut directories: Vec<_> = metadata.present.iter().collect();
        // Exact nominal-size/scale matches precede every approximate match. Ties
        // retain declaration order, then ordered roots and supported extensions.
        directories.sort_by_key(|(index, root, _)| {
            (
                metadata.directories[*index].rank(size, self.options.scale),
                *index,
                *root,
            )
        });
        for name in names {
            for (_, _, base) in &directories {
                for extension in self.options.formats.extensions() {
                    let path = base.join(format!("{name}.{extension}"));
                    if self.exists(&path)? {
                        return Ok(Some(path));
                    }
                }
            }
        }
        Ok(None)
    }

    fn unindexed_hicolor(&mut self, name: &str) -> LookupResult<Option<PathBuf>> {
        // Preserve the shell's historical fallback for indexless installations.
        // This is a fixed candidate list, never a recursive directory scan.
        for root in self.roots {
            if matches!(self.options.formats, IconFormats::PngSvgXpm) {
                for category in ["apps", "status", "devices"] {
                    let path = root.join(format!("hicolor/scalable/{category}/{name}.svg"));
                    if self.exists(&path)? {
                        return Ok(Some(path));
                    }
                }
            }
            for size in [22, 24, 16, 32, 48, 64, 128, 256] {
                for category in ["apps", "status", "devices"] {
                    for extension in self.options.formats.extensions() {
                        let path = root.join(format!(
                            "hicolor/{size}x{size}/{category}/{name}.{extension}"
                        ));
                        if self.exists(&path)? {
                            return Ok(Some(path));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn theme(&mut self, theme: &str) -> LookupResult<Arc<Theme>> {
        if let Some(metadata) = self.metadata.get(theme) {
            return Ok(metadata.clone());
        }
        if self.metadata.len() == MAX_THEMES {
            return Err(());
        }
        let mut metadata = Theme::default();
        for root in self.roots {
            let path = root.join(theme).join("index.theme");
            if !self.exists(&path)? {
                continue;
            }
            if self.index_bytes == 0 {
                return Err(());
            }
            let read = read_bounded(&path, self.index_bytes.min(MAX_INDEX_BYTES));
            let consumed = read
                .as_ref()
                .map(Vec::len)
                .unwrap_or_else(|error| error.bytes_read);
            self.index_bytes = self.index_bytes.saturating_sub(consumed);
            let bytes = read.map_err(|_| ())?;
            self.check()?;
            metadata = Theme::parse(std::str::from_utf8(&bytes).map_err(|_| ())?)?;
            break;
        }
        // Record which declared directories exist under which roots once, so a
        // miss costs one check per present directory rather than per root.
        let mut present = Vec::new();
        for (root_index, root) in self.roots.iter().enumerate() {
            let base = root.join(theme);
            if metadata.directories.is_empty() || !self.is_directory(&base)? {
                continue;
            }
            for (index, directory) in metadata.directories.iter().enumerate() {
                let path = base.join(&directory.path);
                if self.is_directory(&path)? {
                    present.push((index, root_index, path));
                }
            }
        }
        metadata.present = present;
        let metadata = Arc::new(metadata);
        self.metadata.insert(theme.to_owned(), metadata.clone());
        Ok(metadata)
    }

    fn check(&mut self) -> LookupResult<()> {
        if self.stopped || (self.cancelled)() {
            return Err(());
        }
        self.checks = self.checks.checked_sub(1).ok_or(())?;
        Ok(())
    }

    fn exists(&mut self, path: &Path) -> LookupResult<bool> {
        self.check()?;
        if !path.is_absolute() {
            return Ok(false);
        }
        match std::fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err(()),
        }
    }

    fn is_directory(&mut self, path: &Path) -> LookupResult<bool> {
        self.check()?;
        match std::fs::metadata(path) {
            Ok(metadata) => Ok(metadata.is_dir()),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                Ok(false)
            }
            Err(_) => Err(()),
        }
    }
}

/// Valid explicit names (at most eight), then their generic fallbacks when
/// enabled. Duplicates are removed; at most 32 candidates are returned.
pub fn candidate_names(names: &[String], generic_fallback: bool) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    let push = |candidates: &mut Vec<String>, name: &str| {
        if candidates.len() < MAX_CANDIDATES && !candidates.iter().any(|seen| seen == name) {
            candidates.push(name.to_owned());
        }
    };
    let explicit: Vec<&String> = names
        .iter()
        .filter(|name| valid_name(name))
        .take(MAX_NAMES)
        .collect();
    for name in &explicit {
        push(&mut candidates, name);
    }
    if generic_fallback {
        for name in explicit {
            let (base, symbolic) = match name.strip_suffix("-symbolic") {
                Some(base) if !base.is_empty() => (base, true),
                _ => (name.as_str(), false),
            };
            let truncations = || {
                base.match_indices('-')
                    .rev()
                    .map(|(index, _)| &base[..index])
                    .filter(|prefix| !prefix.is_empty())
            };
            if symbolic {
                for prefix in truncations() {
                    push(&mut candidates, &format!("{prefix}-symbolic"));
                }
                push(&mut candidates, base);
            }
            for prefix in truncations() {
                push(&mut candidates, prefix);
            }
        }
    }
    candidates
}

/// A resolved `Icon=`, `app_icon` or similar value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconSource {
    /// An absolute local file, from a path or a local `file://` URL.
    Path(PathBuf),
    /// A themed icon name.
    Name(String),
}

impl IconSource {
    /// Absolute paths and local `file://` URLs are files; anything else must be a
    /// valid icon name. As GIO does for desktop entries, a trailing `.png`, `.svg`
    /// or `.xpm` on a name is removed. Relative paths and remote URLs are rejected.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if let Some(rest) = value
            .get(..7)
            .filter(|scheme| scheme.eq_ignore_ascii_case("file://"))
            .map(|_| &value[7..])
        {
            let path = rest.strip_prefix("localhost").unwrap_or(rest);
            if !path.starts_with('/') {
                return None;
            }
            let path = percent_decode(path)?;
            return local_path(path).map(Self::Path);
        }
        if value.starts_with('/') {
            return local_path(PathBuf::from(value)).map(Self::Path);
        }
        if value.contains(['/', '\\']) || value.contains("://") {
            return None;
        }
        let name = [".png", ".svg", ".xpm"]
            .iter()
            .find_map(|extension| {
                value
                    .len()
                    .checked_sub(extension.len())
                    .filter(|&start| value[start..].eq_ignore_ascii_case(extension))
                    .map(|start| &value[..start])
            })
            .filter(|name| !name.is_empty())
            .unwrap_or(value);
        valid_name(name).then(|| Self::Name(name.to_owned()))
    }
}

fn local_path(path: PathBuf) -> Option<PathBuf> {
    (path.is_absolute()
        && path.as_os_str().len() <= 4096
        && path
            .components()
            .all(|part| !matches!(part, Component::ParentDir)))
    .then_some(path)
}

fn percent_decode(value: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
                decoded.push(u8::from_str_radix(hex, 16).ok()?);
                index += 3;
            }
            b'?' | b'#' => break,
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    if decoded.contains(&0) {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(decoded)))
}

/// A cached lookup request. `theme: None` follows the desktop selection.
#[derive(Clone, Copy)]
pub struct IconQuery<'a> {
    pub names: &'a [String],
    pub size: u16,
    pub scale: u16,
    pub theme: Option<&'a str>,
    pub formats: IconFormats,
    pub generic_fallback: bool,
    pub allow_unindexed_hicolor: bool,
}

impl<'a> IconQuery<'a> {
    /// PNG/SVG/XPM at scale 1, following the desktop theme, without generic fallback.
    pub fn new(names: &'a [String], size: u16) -> Self {
        Self {
            names,
            size,
            scale: 1,
            theme: None,
            formats: IconFormats::PngSvgXpm,
            generic_fallback: false,
            allow_unindexed_hicolor: false,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    theme: String,
    names: Vec<String>,
    size: u16,
    scale: u16,
    formats: u8,
    generic_fallback: bool,
    allow_unindexed_hicolor: bool,
}

struct ResolverState {
    desktop_theme: String,
    stamp: HashMap<PathBuf, Option<SystemTime>>,
    checked: Option<Instant>,
    metadata: HashMap<String, Arc<Theme>>,
    results: HashMap<CacheKey, Option<PathBuf>>,
}

/// Process-wide cached icon resolution. Results are keyed by theme, names, size,
/// scale and options, and are dropped when the icon-theme selection, an icon root
/// or a used theme directory changes modification time (checked at most every two
/// seconds). Cancelled or stopped lookups are not cached. Lookup runs on the
/// caller's thread: call from a blocking worker.
pub struct IconResolver {
    roots: Vec<PathBuf>,
    settings: Vec<PathBuf>,
    revalidate_after: Duration,
    state: Mutex<ResolverState>,
}

impl IconResolver {
    /// Resolver over explicit roots and GTK settings files, for tests and tools.
    pub fn new(roots: Vec<PathBuf>, settings: Vec<PathBuf>) -> Self {
        Self {
            roots,
            settings,
            revalidate_after: REVALIDATE_AFTER,
            state: Mutex::new(ResolverState {
                desktop_theme: "hicolor".into(),
                stamp: HashMap::new(),
                checked: None,
                metadata: HashMap::new(),
                results: HashMap::new(),
            }),
        }
    }

    /// How long a validated cache is trusted before modification times are rechecked.
    pub fn with_revalidate_interval(mut self, interval: Duration) -> Self {
        self.revalidate_after = interval;
        self
    }

    /// The process resolver over [`search_roots`] and the GTK settings files.
    pub fn system() -> &'static Self {
        static SYSTEM: OnceLock<IconResolver> = OnceLock::new();
        SYSTEM.get_or_init(|| Self::new(search_roots(), canopa_file::icon_theme::settings_files()))
    }

    /// The current desktop theme name, or `hicolor` when none is selected.
    pub fn desktop_theme(&self) -> String {
        let mut state = self.lock();
        self.revalidate(&mut state);
        state.desktop_theme.clone()
    }

    pub fn resolve(&self, query: &IconQuery<'_>, cancelled: &dyn Fn() -> bool) -> Option<PathBuf> {
        let (key, metadata) = {
            let mut state = self.lock();
            self.revalidate(&mut state);
            let theme = query.theme.unwrap_or(&state.desktop_theme).to_owned();
            let key = CacheKey {
                theme,
                names: candidate_names(query.names, false),
                size: query.size,
                scale: query.scale,
                formats: query.formats as u8,
                generic_fallback: query.generic_fallback,
                allow_unindexed_hicolor: query.allow_unindexed_hicolor,
            };
            if let Some(result) = state.results.get(&key) {
                return result.clone();
            }
            (key, state.metadata.clone())
        };
        let mut lookup = IconLookup::new(
            &self.roots,
            LookupOptions {
                theme: &key.theme,
                scale: key.scale,
                formats: query.formats,
                allow_unindexed_hicolor: key.allow_unindexed_hicolor,
            },
            cancelled,
        )
        .with_generic_fallback(key.generic_fallback)
        .with_metadata(metadata);
        let result = lookup.resolve(&key.names, key.size);
        if lookup.stopped() {
            return None;
        }
        let mut state = self.lock();
        for (theme, metadata) in lookup.metadata {
            if state.metadata.len() < MAX_THEMES {
                state.metadata.entry(theme).or_insert(metadata);
            }
        }
        if state.results.len() >= MAX_CACHED {
            state.results.clear();
        }
        state.results.insert(key, result.clone());
        result
    }

    /// Resolve an `Icon=`-style value: files are returned when they exist, names
    /// through [`Self::resolve`] with the remaining query options.
    pub fn resolve_source(
        &self,
        source: &IconSource,
        query: &IconQuery<'_>,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<PathBuf> {
        match source {
            IconSource::Path(path) => std::fs::metadata(path)
                .is_ok_and(|metadata| metadata.is_file())
                .then(|| path.clone()),
            IconSource::Name(name) => self.resolve(
                &IconQuery {
                    names: std::slice::from_ref(name),
                    ..*query
                },
                cancelled,
            ),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ResolverState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn revalidate(&self, state: &mut ResolverState) {
        if state
            .checked
            .is_some_and(|checked| checked.elapsed() < self.revalidate_after)
        {
            return;
        }
        let first = state.checked.is_none();
        state.checked = Some(Instant::now());
        let mut paths: Vec<PathBuf> = self.settings.clone();
        paths.extend(self.roots.iter().cloned());
        for theme in state.metadata.keys() {
            paths.extend(self.roots.iter().map(|root| root.join(theme)));
        }
        let current: Vec<_> = paths
            .into_iter()
            .map(|path| {
                let modified = std::fs::metadata(&path)
                    .and_then(|meta| meta.modified())
                    .ok();
                (path, modified)
            })
            .collect();
        // Paths first seen now (a theme loaded since the last check) only extend
        // the stamp; a changed recorded path invalidates everything.
        let changed = first
            || current
                .iter()
                .any(|(path, modified)| state.stamp.get(path).is_some_and(|old| old != modified));
        if changed {
            state.desktop_theme = canopa_file::icon_theme::theme_from_files(&self.settings)
                .unwrap_or_else(|| "hicolor".into());
            state.metadata.clear();
            state.results.clear();
            state.stamp.clear();
        }
        for (path, modified) in current {
            state.stamp.entry(path).or_insert(modified);
        }
    }
}

#[derive(Default)]
struct Theme {
    indexed: bool,
    directories: Vec<Directory>,
    parents: Vec<String>,
    /// (directory index, root index, existing directory path), filled on load.
    present: Vec<(usize, usize, PathBuf)>,
}

impl Theme {
    fn parse(source: &str) -> LookupResult<Self> {
        let mut sections: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
        let mut current = None;
        for line in source
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            if let Some(section) = line
                .strip_prefix('[')
                .and_then(|line| line.strip_suffix(']'))
            {
                if sections.is_empty() && section != "Icon Theme" {
                    return Err(());
                }
                if sections.insert(section, HashMap::new()).is_some() {
                    return Err(());
                }
                current = Some(section);
            } else {
                let (key, value) = line.split_once('=').ok_or(())?;
                let entries = sections.get_mut(current.ok_or(())?).ok_or(())?;
                if entries.insert(key.trim(), value.trim()).is_some() {
                    return Err(());
                }
            }
        }
        let header = sections.get("Icon Theme").ok_or(())?;
        let directories = list(header.get("Directories").ok_or(())?, MAX_DIRECTORIES)?;
        let scaled = list(
            header.get("ScaledDirectories").copied().unwrap_or(""),
            MAX_DIRECTORIES,
        )?;
        let parents = list(header.get("Inherits").copied().unwrap_or(""), MAX_THEMES)?;
        if directories.len() + scaled.len() > MAX_DIRECTORIES {
            return Err(());
        }
        if parents.iter().any(|name| !valid_theme(name)) {
            return Err(());
        }
        let directories = directories
            .into_iter()
            .chain(scaled)
            .map(|path| {
                if path.len() > 1024
                    || path.contains('\\')
                    || !Path::new(path)
                        .components()
                        .all(|part| matches!(part, Component::Normal(_)))
                {
                    return Err(());
                }
                let section = sections.get(path).ok_or(())?;
                let size = number(section, "Size", None)?;
                let scale = number(section, "Scale", Some(1))?;
                let (min, max) = match section.get("Type").copied().unwrap_or("Threshold") {
                    "Fixed" => (size, size),
                    "Scalable" => (
                        number(section, "MinSize", Some(size))?,
                        number(section, "MaxSize", Some(size))?,
                    ),
                    "Threshold" => {
                        let threshold = match section.get("Threshold") {
                            Some(value) => value.parse::<u32>().map_err(|_| ())?,
                            None => 2,
                        };
                        if threshold > 4096 {
                            return Err(());
                        }
                        (size.saturating_sub(threshold), size + threshold)
                    }
                    _ => return Err(()),
                };
                if min > max {
                    return Err(());
                }
                Ok(Directory {
                    path: path.into(),
                    scale,
                    min,
                    max,
                })
            })
            .collect::<LookupResult<Vec<_>>>()?;
        Ok(Self {
            indexed: true,
            directories,
            parents: parents.into_iter().map(str::to_owned).collect(),
            present: Vec::new(),
        })
    }
}

fn number(section: &HashMap<&str, &str>, key: &str, default: Option<u32>) -> LookupResult<u32> {
    let value = match section.get(key) {
        Some(value) => value.parse().map_err(|_| ())?,
        None => default.ok_or(())?,
    };
    if !(1..=4096).contains(&value) {
        return Err(());
    }
    Ok(value)
}

fn list(value: &str, maximum: usize) -> LookupResult<Vec<&str>> {
    let items: Vec<_> = value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .take(maximum + 1)
        .collect();
    if items.len() > maximum {
        return Err(());
    }
    Ok(items)
}

struct Directory {
    path: String,
    scale: u32,
    min: u32,
    max: u32,
}

impl Directory {
    fn rank(&self, size: u16, scale: u16) -> (bool, u32) {
        let (size, scale) = (u32::from(size), u32::from(scale));
        let exact = scale == self.scale && (self.min..=self.max).contains(&size);
        let requested = size * scale;
        let minimum = self.min * self.scale;
        let maximum = self.max * self.scale;
        let distance = if requested < minimum {
            minimum - requested
        } else {
            requested.saturating_sub(maximum)
        };
        (!exact, distance)
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !matches!(name, "." | "..")
        && !name
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
}

fn valid_theme(theme: &str) -> bool {
    valid_name(theme)
        && theme.is_ascii()
        && !theme
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte == b',')
}
