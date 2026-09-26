use crate::DEFAULT_NOTIFICATION_SOUND_THEME;
use crate::local_file::read_bounded;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_ROOTS: usize = 32;
const MAX_THEMES: usize = 32;
const MAX_DIRECTORIES: usize = 64;
const MAX_INDEX_BYTES: usize = 64 * 1024;
const MAX_LOOKUP_CHECKS: usize = 4096;
const EXTENSIONS: [&str; 6] = ["disabled", "oga", "ogg", "wav", "flac", "mp3"];

#[derive(Clone, Debug)]
pub(super) struct SoundThemeResolver {
    data_dirs: Arc<[PathBuf]>,
    locales: Arc<[String]>,
}

#[derive(Debug)]
pub(super) enum SoundLookupStopped {
    Disabled,
    Cancelled,
    Limit,
    InvalidMetadata,
}

type Lookup = Result<Option<PathBuf>, SoundLookupStopped>;

impl SoundThemeResolver {
    pub(super) fn from_environment(data_dirs: Vec<PathBuf>) -> Self {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
            .unwrap_or_else(|| "C".into());
        Self::new(data_dirs, &locale)
    }

    pub(super) fn new(data_dirs: Vec<PathBuf>, locale: &str) -> Self {
        Self {
            data_dirs: data_dirs
                .into_iter()
                .filter(|path| path.is_absolute() && path.as_os_str().len() <= 4096)
                .take(MAX_ROOTS + 1)
                .collect(),
            locales: locale_candidates(locale).into(),
        }
    }

    pub(super) fn resolve(&self, theme: &str, name: &str, cancelled: &AtomicBool) -> Lookup {
        if self.data_dirs.len() > MAX_ROOTS {
            return Err(SoundLookupStopped::Limit);
        }
        if !valid_theme(theme) || !valid_component(name) {
            return Err(SoundLookupStopped::InvalidMetadata);
        }
        let mut budget = LookupBudget {
            remaining: MAX_LOOKUP_CHECKS,
            cancelled,
        };
        let mut seen = HashSet::new();
        if let Some(path) = self.lookup_theme(theme, name, &mut seen, &mut budget)? {
            return Ok(Some(path));
        }
        if let Some(path) = self.lookup_theme(
            DEFAULT_NOTIFICATION_SOUND_THEME,
            name,
            &mut seen,
            &mut budget,
        )? {
            return Ok(Some(path));
        }
        self.lookup_directories(
            "",
            &[SoundDirectory {
                path: ".".into(),
                profile: "".into(),
            }],
            name,
            &mut budget,
        )
    }

    fn lookup_theme(
        &self,
        theme: &str,
        name: &str,
        seen: &mut HashSet<String>,
        budget: &mut LookupBudget<'_>,
    ) -> Lookup {
        budget.check()?;
        if seen.contains(theme) {
            return Ok(None);
        }
        if seen.len() == MAX_THEMES {
            return Err(SoundLookupStopped::Limit);
        }
        seen.insert(theme.to_string());
        let metadata = self.theme_metadata(theme, budget)?;
        if let Some(path) = self.lookup_directories(theme, &metadata.directories, name, budget)? {
            return Ok(Some(path));
        }
        for parent in metadata.parents {
            // The specification's prose places the freedesktop fallback after
            // the complete selected-theme inheritance tree.
            if parent == DEFAULT_NOTIFICATION_SOUND_THEME {
                continue;
            }
            if let Some(path) = self.lookup_theme(&parent, name, seen, budget)? {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }

    fn theme_metadata(
        &self,
        theme: &str,
        budget: &mut LookupBudget<'_>,
    ) -> Result<ThemeMetadata, SoundLookupStopped> {
        for root in self.data_dirs.iter() {
            let index = root.join("sounds").join(theme).join("index.theme");
            if budget.exists(&index)? {
                // First index wins across roots, including when malformed.
                let bytes = read_bounded(&index, MAX_INDEX_BYTES)
                    .map_err(|_| SoundLookupStopped::InvalidMetadata)?;
                let text =
                    std::str::from_utf8(&bytes).map_err(|_| SoundLookupStopped::InvalidMetadata)?;
                return ThemeMetadata::parse(text);
            }
        }
        // Compatibility for unindexed sound installations, without walking
        // arbitrary subdirectories or following directory cycles.
        Ok(ThemeMetadata {
            directories: vec![
                SoundDirectory {
                    path: "stereo".into(),
                    profile: "stereo".into(),
                },
                SoundDirectory {
                    path: "mono".into(),
                    profile: "".into(),
                },
                SoundDirectory {
                    path: ".".into(),
                    profile: "".into(),
                },
            ],
            parents: Vec::new(),
        })
    }

    fn lookup_directories(
        &self,
        theme: &str,
        directories: &[SoundDirectory],
        name: &str,
        budget: &mut LookupBudget<'_>,
    ) -> Lookup {
        // Notify requests stereo event sounds. Profile-less directories are
        // the final profile fallback; unrelated surround profiles do not match.
        for profile in ["stereo", ""] {
            for directory in directories
                .iter()
                .filter(|directory| directory.profile == profile)
            {
                for root in self.data_dirs.iter() {
                    let base = root.join("sounds").join(theme).join(&directory.path);
                    let mut candidate = name;
                    loop {
                        for locale in self.locales.iter() {
                            for extension in EXTENSIONS {
                                let path =
                                    base.join(locale).join(format!("{candidate}.{extension}"));
                                if budget.exists(&path)? {
                                    if extension == "disabled" {
                                        return Err(SoundLookupStopped::Disabled);
                                    }
                                    return Ok(Some(path));
                                }
                            }
                        }
                        let Some((prefix, _)) = candidate.rsplit_once('-') else {
                            break;
                        };
                        if prefix.is_empty() {
                            break;
                        }
                        candidate = prefix;
                    }
                }
            }
        }
        Ok(None)
    }
}

struct LookupBudget<'a> {
    remaining: usize,
    cancelled: &'a AtomicBool,
}

impl LookupBudget<'_> {
    fn check(&mut self) -> Result<(), SoundLookupStopped> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(SoundLookupStopped::Cancelled);
        }
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(SoundLookupStopped::Limit)?;
        Ok(())
    }

    fn exists(&mut self, path: &Path) -> Result<bool, SoundLookupStopped> {
        self.check()?;
        match std::fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err(SoundLookupStopped::InvalidMetadata),
        }
    }
}

struct SoundDirectory {
    path: String,
    profile: String,
}

struct ThemeMetadata {
    directories: Vec<SoundDirectory>,
    parents: Vec<String>,
}

impl ThemeMetadata {
    fn parse(text: &str) -> Result<Self, SoundLookupStopped> {
        let invalid = || SoundLookupStopped::InvalidMetadata;
        let mut groups: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
        let mut current = None;
        for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(group) = line
                .strip_prefix('[')
                .and_then(|line| line.strip_suffix(']'))
            {
                if groups.is_empty() && group != "Sound Theme" {
                    return Err(invalid());
                }
                if groups.len() == 128 || groups.insert(group, HashMap::new()).is_some() {
                    return Err(invalid());
                }
                current = Some(group);
            } else {
                let (key, value) = line.split_once('=').ok_or_else(invalid)?;
                let group = groups
                    .get_mut(current.ok_or_else(invalid)?)
                    .ok_or_else(invalid)?;
                if group.len() == 128 || group.insert(key.trim(), value.trim()).is_some() {
                    return Err(invalid());
                }
            }
        }
        let theme = groups.get("Sound Theme").ok_or_else(invalid)?;
        let mut directories = Vec::new();
        for path in theme
            .get("Directories")
            .ok_or_else(invalid)?
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if directories.len() == MAX_DIRECTORIES || !valid_directory(path) {
                return Err(invalid());
            }
            let section = groups.get(path).ok_or_else(invalid)?;
            let profile = section.get("OutputProfile").copied().unwrap_or("");
            if profile.len() > 255 {
                return Err(invalid());
            }
            directories.push(SoundDirectory {
                path: path.into(),
                profile: profile.into(),
            });
        }
        let mut parents = Vec::new();
        for parent in theme
            .get("Inherits")
            .copied()
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if parents.len() == MAX_THEMES || !valid_theme(parent) {
                return Err(invalid());
            }
            parents.push(parent.into());
        }
        Ok(Self {
            directories,
            parents,
        })
    }
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !value.contains(['\0', '/'])
        && matches!(
            Path::new(value).components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        )
}

fn locale_candidates(locale: &str) -> Vec<String> {
    let mut locales = Vec::new();
    let mut add = |value: &str| {
        if !locales.iter().any(|old| old == value) {
            locales.push(value.to_string());
        }
    };
    if valid_component(locale) && locale.is_ascii() {
        let without_modifier = locale.split('@').next().unwrap();
        let language = without_modifier.split('_').next().unwrap();
        for value in [locale, without_modifier, language] {
            add(value);
            // Accept conventional codeset-free directories as well as the
            // literal LC_MESSAGES spelling, preserving any modifier.
            if let Some((base, rest)) = value.split_once('.') {
                let normalized = match rest.split_once('@') {
                    Some((_, modifier)) => format!("{base}@{modifier}"),
                    None => base.to_string(),
                };
                if valid_component(&normalized) {
                    add(&normalized);
                }
            }
        }
    }
    add("C");
    add("");
    locales
}

fn valid_theme(value: &str) -> bool {
    valid_component(value)
        && value.is_ascii()
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte == b',')
}

fn valid_directory(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && !value.contains('\0')
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}
