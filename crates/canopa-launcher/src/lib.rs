//! Launcher-owned preferences and read-only discovery shared with Settings.
#[macro_use]
pub mod i18n;
pub mod clipboard;

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs::OpenOptions,
    io,
    os::{
        fd::AsRawFd,
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Path, PathBuf},
};
pub const MAX_BYTES: usize = 1024 * 1024;
fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Application { id: String },
    File { bytes: Vec<u8> },
    Url { url: String },
    Control { name: String },
    Command { argv: Vec<String> },
}
impl Target {
    pub fn file(path: &Path) -> Self {
        Self::File {
            bytes: path.as_os_str().as_bytes().to_vec(),
        }
    }
    pub fn path(&self) -> Option<PathBuf> {
        if let Self::File { bytes } = self {
            Some(PathBuf::from(std::ffi::OsString::from_vec(bytes.clone())))
        } else {
            None
        }
    }
    pub fn key(&self) -> String {
        serde_json::to_string(self).expect("target serializes")
    }
    pub fn validate(&self) -> io::Result<()> {
        let valid = match self {
            Self::Application { id } => !id.is_empty() && id.len() <= 4096 && !id.contains('\0'),
            Self::File { bytes } => {
                bytes.first() == Some(&b'/') && bytes.len() <= 4096 && !bytes.contains(&0)
            }
            Self::Url { url } => valid_url(url),
            Self::Control { name } => matches!(name.as_str(), "wifi" | "audio" | "notifications"),
            Self::Command { argv } => {
                !argv.is_empty()
                    && argv.len() <= 64
                    && !argv[0].trim().is_empty()
                    && argv.iter().all(|s| s.len() <= 4096 && !s.contains('\0'))
            }
        };
        if valid {
            Ok(())
        } else {
            Err(invalid(fl!("launcher-invalid-shortcut")))
        }
    }
}
pub fn valid_url(value: &str) -> bool {
    value.len() <= 8192
        && !value.chars().any(char::is_control)
        && url::Url::parse(value).is_ok_and(|u| {
            matches!(u.scheme(), "https" | "http")
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
}
pub fn valid_keyword(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('=')
        && !value.chars().any(|c| c.is_whitespace() || c.is_control())
        && !matches!(value, "apps" | "files" | "=")
}
pub fn valid_template(value: &str) -> bool {
    value.len() <= 2048
        && value.matches("{query}").count() == 1
        && valid_url(&value.replace("{query}", "example"))
        && !value.replace("{query}", "").contains(['{', '}'])
}
pub fn search_url(template: &str, query: &str) -> io::Result<String> {
    if !valid_template(template) || query.len() > 1024 {
        return Err(invalid(fl!("launcher-invalid-template")));
    }
    let mut escaped = String::new();
    use std::fmt::Write;
    for byte in query.as_bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(byte) {
            escaped.push(*byte as char);
        } else {
            write!(&mut escaped, "%{byte:02X}").unwrap();
        }
    }
    let url = template.replace("{query}", &escaped);
    if valid_url(&url) {
        Ok(url)
    } else {
        Err(invalid(fl!("launcher-invalid-search-url")))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shortcut {
    pub target: Target,
    pub label: String,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub icon: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionPreference {
    pub enabled: bool,
    pub keyword: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    pub keyword: String,
    pub template: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub version: u8,
    #[serde(default)]
    pub extensions: BTreeMap<String, ExtensionPreference>,
    pub web_template: String,
    #[serde(default)]
    pub searches: Vec<Search>,
    #[serde(default)]
    pub shortcuts: Vec<Shortcut>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            extensions: BTreeMap::new(),
            web_template: "https://duckduckgo.com/?q={query}".into(),
            searches: vec![Search {
                keyword: "wiki".into(),
                template: "https://en.wikipedia.org/w/index.php?search={query}".into(),
            }],
            shortcuts: vec![],
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> io::Result<()> {
        if self.version != 1
            || self.extensions.len() > 64
            || self.searches.len() > 16
            || self.shortcuts.len() > 256
            || !valid_template(&self.web_template)
        {
            return Err(invalid(fl!("launcher-invalid-preferences")));
        }
        let mut keywords = HashSet::new();
        for (id, pref) in &self.extensions {
            if id.is_empty()
                || id.len() > 256
                || !valid_keyword(&pref.keyword)
                || pref.enabled && !keywords.insert(pref.keyword.to_lowercase())
            {
                return Err(invalid(fl!("launcher-extension-keywords")));
            }
        }
        for (id, keyword) in [("builtin:web", "web"), ("builtin:date-time", "date")] {
            if !self.extensions.contains_key(id) && !keywords.insert(keyword.into()) {
                return Err(invalid(fl!("launcher-extension-keyword-conflict")));
            }
        }
        let web_enabled = self.extensions.get("builtin:web").is_none_or(|p| p.enabled);
        let mut search_keywords = HashSet::new();
        for search in &self.searches {
            if !valid_keyword(&search.keyword)
                || !search_keywords.insert(search.keyword.to_lowercase())
                || web_enabled && !keywords.insert(search.keyword.to_lowercase())
                || !valid_template(&search.template)
            {
                return Err(invalid(fl!("launcher-search-shortcuts")));
            }
        }
        let mut targets = HashSet::new();
        let mut aliases = HashSet::new();
        for shortcut in &self.shortcuts {
            shortcut.target.validate()?;
            if shortcut.label.trim().is_empty()
                || shortcut.label.len() > 512
                || shortcut.icon.as_ref().is_some_and(|s| s.len() > 256)
                || !targets.insert(shortcut.target.key())
                || !shortcut.alias.is_empty()
                    && (!valid_keyword(&shortcut.alias)
                        || !aliases.insert(shortcut.alias.to_lowercase())
                        || keywords.contains(&shortcut.alias.to_lowercase()))
            {
                return Err(invalid(fl!("launcher-shortcut-names")));
            }
        }
        Ok(())
    }
    pub fn read(path: &Path) -> io::Result<Self> {
        match canopa_file::private_state::read(path, MAX_BYTES) {
            Ok(bytes) => {
                let value: Self = serde_json::from_slice(&bytes)
                    .map_err(|_| invalid(fl!("launcher-invalid-json")))?;
                value.validate()?;
                Ok(value)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }
    /// Cooperating writers lock and read again before applying a field-level change.
    pub fn update(path: &Path, edit: impl FnOnce(&mut Self) -> io::Result<()>) -> io::Result<Self> {
        let _lock = lock_preferences(path)?;
        let mut value = Self::read(path)?;
        edit(&mut value)?;
        value.validate()?;
        let bytes = serde_json::to_vec_pretty(&value).map_err(io::Error::other)?;
        if bytes.len() > MAX_BYTES {
            return Err(invalid(fl!("launcher-preferences-too-large")));
        }
        canopa_file::private_state::write(path, &bytes)?;
        Ok(value)
    }
    pub fn edit_shortcut(&mut self, mut shortcut: Shortcut, pin: bool) -> io::Result<()> {
        if let Some(old) = self
            .shortcuts
            .iter_mut()
            .find(|s| s.target == shortcut.target)
        {
            if pin {
                old.pinned = !old.pinned;
            } else {
                old.alias = std::mem::take(&mut shortcut.alias);
            }
        } else {
            if pin {
                shortcut.pinned = true;
            }
            self.shortcuts.push(shortcut);
        }
        self.shortcuts.retain(|s| s.pinned || !s.alias.is_empty());
        self.validate()
    }
}
fn lock_preferences(path: &Path) -> io::Result<std::fs::File> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid(fl!("launcher-no-preferences-directory")))?;
    std::fs::create_dir_all(parent)?;
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(parent.join(".launcher.lock"))?;
    let meta = lock.metadata()?;
    if !meta.is_file() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(invalid(fl!("launcher-invalid-lock")));
    }
    // A child forked by another thread keeps a copy of a lock descriptor this
    // process just closed until it executes. Retry briefly so that window is
    // not reported as a concurrent writer.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        if std::time::Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                fl!("launcher-preferences-busy"),
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(lock)
}

fn xdg(variable: &str, fallback: &str) -> io::Result<PathBuf> {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(fallback))
        })
        .ok_or_else(|| invalid(fl!("launcher-no-storage-directory")))
}
pub fn preferences_path() -> io::Result<PathBuf> {
    Ok(xdg("XDG_CONFIG_HOME", ".config")?.join("canopa/launcher.json"))
}
pub fn catalog_path() -> io::Result<PathBuf> {
    Ok(xdg("XDG_STATE_HOME", ".local/state")?.join("canopa/shell/launcher-extensions.json"))
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub extensions: Vec<ExtensionInfo>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub keyword: String,
    pub enabled: bool,
    pub description: String,
    pub actions: Vec<String>,
    pub status: String,
}
impl Catalog {
    pub fn read(path: &Path) -> io::Result<Self> {
        let bytes = canopa_file::private_state::read(path, MAX_BYTES)?;
        let value: Self = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if value.extensions.len() > 64 {
            return Err(invalid(fl!("launcher-catalog-too-large")));
        }
        Ok(value)
    }
    pub fn write(&self, path: &Path) -> io::Result<()> {
        canopa_file::private_state::write(
            path,
            &serde_json::to_vec(self).map_err(io::Error::other)?,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn templates_escape_literal_queries_and_reject_non_web_schemes() {
        assert_eq!(
            search_url("https://example.test/?q={query}", "a&b #é").unwrap(),
            "https://example.test/?q=a%26b%20%23%C3%A9"
        );
        for s in [
            "javascript:{query}",
            "https://example.test/",
            "https://example.test/{query}/{query}",
        ] {
            assert!(!valid_template(s));
        }
    }
    #[test]
    fn editors_preserve_other_fields_and_original_path_bytes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("launcher.json");
        let target = Target::File {
            bytes: b"/tmp/odd-\xff".to_vec(),
        };
        let shortcut = Shortcut {
            target: target.clone(),
            label: "Odd".into(),
            alias: "work".into(),
            pinned: false,
            icon: None,
        };
        Preferences::update(&path, |p| p.edit_shortcut(shortcut, true)).unwrap();
        Preferences::update(&path, |p| {
            p.web_template = "https://example.test/?q={query}".into();
            Ok(())
        })
        .unwrap();
        let p = Preferences::read(&path).unwrap();
        assert_eq!(p.shortcuts[0].target, target);
        assert!(p.shortcuts[0].pinned);
        assert_eq!(p.shortcuts[0].alias, "work");
        assert_eq!(
            std::fs::File::open(path)
                .unwrap()
                .metadata()
                .unwrap()
                .mode()
                & 0o777,
            0o600
        );
    }
    #[test]
    fn invalid_and_duplicate_aliases_cannot_be_published() {
        let mut p = Preferences::default();
        let s = Shortcut {
            target: Target::Application { id: "a".into() },
            label: "A".into(),
            alias: "apps".into(),
            pinned: true,
            icon: None,
        };
        assert!(p.edit_shortcut(s, false).is_err());
    }
}
