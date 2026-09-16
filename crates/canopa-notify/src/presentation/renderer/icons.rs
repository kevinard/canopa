//! Indexed local icon discovery. One lookup context belongs to one prepared frame.
use super::BannerIconSettings;
use crate::local_file::read_bounded;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

const MAX_ROOTS: usize = 32;
const MAX_THEMES: usize = 32;
const MAX_DIRECTORIES: usize = 1024;
const MAX_INDEX_BYTES: usize = 256 * 1024;
// PNG is the supported standard format; JPEG is retained as a local extension.
const EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];
type LookupResult<T> = Result<T, ()>;

pub(super) fn search_roots() -> Vec<PathBuf> {
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

pub(super) struct IconLookup<'a> {
    roots: &'a [PathBuf],
    settings: &'a BannerIconSettings,
    cancelled: &'a dyn Fn() -> bool,
    checks: usize,
    index_bytes: usize,
    metadata: HashMap<String, Arc<Theme>>,
    stopped: bool,
}

impl<'a> IconLookup<'a> {
    pub(super) fn new(
        roots: &'a [PathBuf],
        settings: &'a BannerIconSettings,
        cancelled: &'a dyn Fn() -> bool,
        enabled: bool,
    ) -> Self {
        Self {
            roots,
            settings,
            cancelled,
            checks: 65536,
            index_bytes: 2 * 1024 * 1024,
            metadata: HashMap::new(),
            stopped: !enabled
                || roots.len() > MAX_ROOTS
                || BannerIconSettings::new(settings.theme.clone(), settings.scale).is_err(),
        }
    }

    pub(super) fn resolve(&mut self, names: &[String], size: u16) -> Option<PathBuf> {
        // The app-identity fallback supplies two names; sender icon references supply one.
        for name in names.iter().take(2) {
            if !valid_name(name) {
                continue;
            }
            let result = self.resolve_name(name, size);
            match result {
                Ok(Some(path)) => return Some(path),
                Ok(None) => {}
                Err(()) => {
                    self.stopped = true;
                    return None;
                }
            }
        }
        None
    }

    fn resolve_name(&mut self, name: &str, size: u16) -> LookupResult<Option<PathBuf>> {
        let mut seen = HashSet::new();
        if let Some(path) = self.in_theme(&self.settings.theme.clone(), name, size, &mut seen)? {
            return Ok(Some(path));
        }
        if let Some(path) = self.in_theme("hicolor", name, size, &mut seen)? {
            return Ok(Some(path));
        }
        for root in self.roots {
            for extension in EXTENSIONS {
                let path = root.join(format!("{name}.{extension}"));
                if self.exists(&path)? {
                    return Ok(Some(path));
                }
            }
        }
        Ok(None)
    }

    fn in_theme(
        &mut self,
        theme: &str,
        name: &str,
        size: u16,
        seen: &mut HashSet<String>,
    ) -> LookupResult<Option<PathBuf>> {
        self.check()?;
        if !seen.insert(theme.to_owned()) {
            return Ok(None);
        }
        if seen.len() > MAX_THEMES {
            return Err(());
        }
        let metadata = self.theme(theme)?;
        let mut directories: Vec<_> = metadata.directories.iter().enumerate().collect();
        // Exact nominal-size/scale matches precede every approximate match. Ties
        // retain declaration order, then ordered roots and supported extensions.
        directories
            .sort_by_key(|(index, directory)| (directory.rank(size, self.settings.scale), *index));
        for (_, directory) in directories {
            for root in self.roots {
                for extension in EXTENSIONS {
                    let path = root
                        .join(theme)
                        .join(&directory.path)
                        .join(format!("{name}.{extension}"));
                    if self.exists(&path)? {
                        return Ok(Some(path));
                    }
                }
            }
        }
        for parent in &metadata.parents {
            // Required fallback comes after the complete explicit parent tree.
            if parent == "hicolor" {
                continue;
            }
            if let Some(path) = self.in_theme(parent, name, size, seen)? {
                return Ok(Some(path));
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
}

#[derive(Default)]
struct Theme {
    directories: Vec<Directory>,
    parents: Vec<String>,
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
        if parents
            .iter()
            .any(|name| BannerIconSettings::new((*name).into(), 1).is_err())
        {
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
            directories,
            parents: parents.into_iter().map(str::to_owned).collect(),
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

use super::valid_icon_name as valid_name;
