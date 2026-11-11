//! Applications for MIME types: the XDG MIME Applications Associations
//! specification (`mimeapps.list`) over installed desktop entries.
//!
//! A [`MimeApps`] snapshot reads every `mimeapps.list` and desktop entry once;
//! call [`MimeApps::load`] on a worker. Queries are CPU-only. Consumers keep
//! launching, presentation and process lifetimes.
//!
//! Precedence, highest first: `$XDG_CONFIG_HOME`, each `$XDG_CONFIG_DIRS`,
//! `$XDG_DATA_HOME/applications`, then each `$XDG_DATA_DIRS/applications`. In
//! each directory `<desktop>-mimeapps.list` (for each `XDG_CURRENT_DESKTOP`
//! name) comes before `mimeapps.list`. Application directories also contribute
//! the entries whose `MimeType=` lists a type, after their own added
//! associations. A removed association hides that application for the type in
//! the same and every lower-precedence directory, except where a higher list
//! adds it again.
use crate::{DesktopApplication, DesktopEnvironment, Purpose, RawDesktopEntry};
use canopa_file::mime::{MimeDatabase, OCTET_STREAM};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

const MAX_LIST_BYTES: usize = 1024 * 1024;
const MAX_TYPE_CHAIN: usize = 32;
pub const DEFAULT_GROUP: &str = "Default Applications";
pub const ADDED_GROUP: &str = "Added Associations";
pub const REMOVED_GROUP: &str = "Removed Associations";

/// Where associations are read, highest precedence first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MimeAppsPaths {
    pub config_home: Option<PathBuf>,
    pub config_dirs: Vec<PathBuf>,
    pub data_home: Option<PathBuf>,
    pub data_dirs: Vec<PathBuf>,
    /// Lowercased `XDG_CURRENT_DESKTOP` names, in order.
    pub desktops: Vec<String>,
}

impl MimeAppsPaths {
    /// The XDG Base Directory locations of this process.
    pub fn current() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute());
        let absolute = |key: &str| {
            std::env::var_os(key)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
        };
        let list = |key: &str, default: &str| -> Vec<PathBuf> {
            let value = std::env::var_os(key)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| default.into());
            std::env::split_paths(&value)
                .filter(|path| path.is_absolute())
                .collect()
        };
        Self {
            config_home: absolute("XDG_CONFIG_HOME")
                .or_else(|| home.as_ref().map(|home| home.join(".config"))),
            config_dirs: list("XDG_CONFIG_DIRS", "/etc/xdg"),
            data_home: absolute("XDG_DATA_HOME")
                .or_else(|| home.as_ref().map(|home| home.join(".local/share"))),
            data_dirs: list("XDG_DATA_DIRS", "/usr/local/share:/usr/share"),
            desktops: DesktopEnvironment::current().names,
        }
    }

    /// The `applications` directories, highest precedence first. A desktop
    /// entry inside one of these is installed, hence trusted to launch.
    pub fn application_dirs(&self) -> Vec<PathBuf> {
        self.data_home
            .iter()
            .chain(&self.data_dirs)
            .map(|dir| dir.join("applications"))
            .collect()
    }

    /// The list a user's choice is written to: `$XDG_CONFIG_HOME/mimeapps.list`.
    pub fn user_list(&self) -> Option<PathBuf> {
        self.config_home
            .as_ref()
            .map(|dir| dir.join("mimeapps.list"))
    }

    fn levels(&self) -> Vec<(PathBuf, bool)> {
        let mut levels = Vec::new();
        for dir in self.config_home.iter().chain(&self.config_dirs) {
            levels.push((dir.clone(), false));
        }
        for dir in self.application_dirs() {
            levels.push((dir, true));
        }
        levels
    }

    fn list_files(&self, dir: &Path) -> Vec<PathBuf> {
        self.desktops
            .iter()
            .filter(|name| valid_desktop_name(name))
            .map(|name| dir.join(format!("{name}-mimeapps.list")))
            .chain(std::iter::once(dir.join("mimeapps.list")))
            .collect()
    }
}

fn valid_desktop_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && !name.starts_with('.')
}

/// The three association groups of one `mimeapps.list`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Associations {
    pub defaults: HashMap<String, Vec<String>>,
    pub added: HashMap<String, Vec<String>>,
    pub removed: HashMap<String, Vec<String>>,
}

impl Associations {
    /// Parse a list. Unknown groups, comments and malformed lines are ignored;
    /// a repeated key keeps its first value, as GLib's key file reader does.
    pub fn parse(source: &str) -> Self {
        let mut associations = Self::default();
        let mut group: Option<&mut HashMap<String, Vec<String>>> = None;
        for line in source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                group = match name {
                    DEFAULT_GROUP => Some(&mut associations.defaults),
                    ADDED_GROUP => Some(&mut associations.added),
                    REMOVED_GROUP => Some(&mut associations.removed),
                    _ => None,
                };
                continue;
            }
            let (Some(map), Some((key, value))) = (group.as_deref_mut(), line.split_once('='))
            else {
                continue;
            };
            let key = key.trim();
            if !valid_mime(key) {
                continue;
            }
            let ids = value
                .split(';')
                .map(str::trim)
                .filter(|id| valid_id(id))
                .map(str::to_owned)
                .collect();
            map.entry(key.to_owned()).or_insert(ids);
        }
        associations
    }
}

/// True for a plausible `type/subtype` key; the group keys of `mimeapps.list`.
pub fn valid_mime(mime: &str) -> bool {
    let Some((kind, subtype)) = mime.split_once('/') else {
        return false;
    };
    let token = |part: &str| {
        !part.is_empty()
            && part.len() <= 127
            && part.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        byte,
                        b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                    )
            })
    };
    token(kind) && token(subtype)
}

/// True for a desktop file ID that can appear in a list value.
pub fn valid_id(id: &str) -> bool {
    id.len() > ".desktop".len()
        && id.len() <= 255
        && id.ends_with(".desktop")
        && !id.starts_with('.')
        && !id.contains(['/', '\\', ';', '=', '[', ']', '\0', '\n', '\r'])
}

/// The type, then its ancestors breadth-first, without `application/octet-stream`
/// unless it is the type itself: every file inherits from it, and its handlers
/// (hex editors) would be offered for everything. `text/*` reaches `text/plain`.
pub fn type_chain(database: &MimeDatabase, mime: &str) -> Vec<String> {
    let mut chain = vec![database.unalias(mime).to_owned()];
    let mut index = 0;
    while index < chain.len() && chain.len() < MAX_TYPE_CHAIN {
        let parents: Vec<String> = database
            .parents(&chain[index])
            .into_iter()
            .filter(|parent| *parent != OCTET_STREAM)
            .map(str::to_owned)
            .collect();
        for parent in parents {
            if !chain.contains(&parent) && chain.len() < MAX_TYPE_CHAIN {
                chain.push(parent);
            }
        }
        index += 1;
    }
    chain
}

struct Level {
    lists: Vec<Associations>,
    /// Installed IDs defined by this applications directory, in discovery order.
    implicit: Vec<String>,
}

/// One read of every association list and installed desktop entry.
pub struct MimeApps {
    levels: Vec<Level>,
    applications: Vec<DesktopApplication>,
    index: HashMap<String, usize>,
    types: HashMap<String, Vec<String>>,
    no_display: HashSet<String>,
}

impl MimeApps {
    /// Read this process's XDG locations. Call on a worker.
    pub fn load() -> Self {
        Self::load_from(&MimeAppsPaths::current(), &DesktopEnvironment::current())
    }

    /// Read explicit locations. Unreadable or oversized lists contribute nothing.
    pub fn load_from(paths: &MimeAppsPaths, env: &DesktopEnvironment) -> Self {
        let mut levels = Vec::new();
        let mut entries: Vec<(usize, RawDesktopEntry)> = Vec::new();
        let mut seen = HashSet::new();
        for (dir, applications) in paths.levels() {
            let lists = paths
                .list_files(&dir)
                .iter()
                .filter_map(|file| canopa_file::read_bounded(file, MAX_LIST_BYTES).ok())
                .filter_map(|bytes| String::from_utf8(bytes).ok())
                .map(|source| Associations::parse(&source))
                .collect();
            let mut here = Vec::new();
            if applications {
                let _ = crate::collect_desktop_entries(&dir, &dir, &mut here);
                here.sort_by(|left, right| left.id.cmp(&right.id));
            }
            let level = levels.len();
            for entry in here {
                // The highest-precedence directory defines an ID, even when that
                // definition is hidden or invalid.
                if seen.insert(entry.id.clone()) {
                    entries.push((level, entry));
                }
            }
            levels.push(Level {
                lists,
                implicit: Vec::new(),
            });
        }
        let mut applications = Vec::new();
        let mut index = HashMap::new();
        let mut types = HashMap::new();
        let mut no_display = HashSet::new();
        for (level, entry) in entries {
            let fields = crate::parse_desktop_entry_fields(&entry.body);
            if let Some(try_exec) = crate::optional_non_empty_field(&fields, "TryExec")
                && crate::find_program(&crate::unescape_string(&try_exec), &env.search_path)
                    .is_none()
            {
                continue;
            }
            let mimes: Vec<String> = fields
                .get("MimeType")
                .map(|value| {
                    crate::semicolon_list(value)
                        .into_iter()
                        .filter(|mime| valid_mime(mime))
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let hidden_from_menus = crate::bool_field(&fields, "NoDisplay");
            let Some(app) = crate::application_from_entry(entry, env, Purpose::Chooser) else {
                continue;
            };
            if hidden_from_menus {
                no_display.insert(app.id.clone());
            }
            levels[level].implicit.push(app.id.clone());
            index.insert(app.id.clone(), applications.len());
            types.insert(app.id.clone(), mimes);
            applications.push(app);
        }
        Self {
            levels,
            applications,
            index,
            types,
            no_display,
        }
    }

    /// Every installed application, in discovery order.
    pub fn applications(&self) -> &[DesktopApplication] {
        &self.applications
    }

    /// Installed applications that menus show (not `NoDisplay=true`), for an
    /// "Other application" list.
    pub fn menu_applications(&self) -> Vec<&DesktopApplication> {
        self.applications
            .iter()
            .filter(|app| !self.no_display.contains(&app.id))
            .collect()
    }

    pub fn application(&self, id: &str) -> Option<&DesktopApplication> {
        self.index.get(id).map(|&index| &self.applications[index])
    }

    /// The `MimeType=` list of an installed application.
    pub fn declared_types(&self, id: &str) -> &[String] {
        self.types.get(id).map_or(&[], Vec::as_slice)
    }

    /// Applications associated with exactly `mime`, most preferred first.
    pub fn associations(&self, mime: &str) -> Vec<&DesktopApplication> {
        let mut blocked: HashSet<&str> = HashSet::new();
        let mut ids: Vec<&str> = Vec::new();
        fn push<'a>(apps: &MimeApps, id: &'a str, ids: &mut Vec<&'a str>) {
            if !ids.contains(&id) && apps.index.contains_key(id) {
                ids.push(id);
            }
        }
        for level in &self.levels {
            for list in &level.lists {
                for id in list.added.get(mime).into_iter().flatten() {
                    if !blocked.contains(id.as_str()) {
                        push(self, id, &mut ids);
                    }
                }
            }
            for list in &level.lists {
                blocked.extend(
                    list.removed
                        .get(mime)
                        .into_iter()
                        .flatten()
                        .map(String::as_str),
                );
            }
            for id in &level.implicit {
                if !blocked.contains(id.as_str())
                    && !self.no_display.contains(id)
                    && self
                        .declared_types(id)
                        .iter()
                        .any(|declared| declared == mime)
                {
                    push(self, id, &mut ids);
                }
            }
        }
        ids.into_iter()
            .filter_map(|id| self.application(id))
            .collect()
    }

    /// The first installed `[Default Applications]` choice for exactly `mime`.
    pub fn explicit_default(&self, mime: &str) -> Option<&DesktopApplication> {
        self.levels
            .iter()
            .flat_map(|level| &level.lists)
            .flat_map(|list| list.defaults.get(mime).into_iter().flatten())
            .find_map(|id| self.application(id))
    }

    /// The default for a type chain (see [`type_chain`]): an explicit default for
    /// the type, then for each ancestor, then the most preferred association of
    /// the type, then of each ancestor. This is GLib's order.
    pub fn default_for(&self, chain: &[String]) -> Option<&DesktopApplication> {
        chain
            .iter()
            .find_map(|mime| self.explicit_default(mime))
            .or_else(|| {
                chain
                    .iter()
                    .find_map(|mime| self.associations(mime).into_iter().next())
            })
    }

    /// The default first, then the associations of the type and of each
    /// ancestor in chain order, without duplicates.
    pub fn applications_for(&self, chain: &[String]) -> Vec<&DesktopApplication> {
        let mut result: Vec<&DesktopApplication> = self.default_for(chain).into_iter().collect();
        for mime in chain {
            for app in self.associations(mime) {
                if !result.iter().any(|known| known.id == app.id) {
                    result.push(app);
                }
            }
        }
        result
    }

    /// Applications for every type of a multi-file selection: the chain of the
    /// first type ordered as [`Self::applications_for`], kept only when every
    /// other chain also offers it.
    pub fn applications_for_all(&self, chains: &[Vec<String>]) -> Vec<&DesktopApplication> {
        let Some((first, rest)) = chains.split_first() else {
            return Vec::new();
        };
        let others: Vec<HashSet<&str>> = rest
            .iter()
            .map(|chain| {
                self.applications_for(chain)
                    .into_iter()
                    .map(|app| app.id.as_str())
                    .collect()
            })
            .collect();
        self.applications_for(first)
            .into_iter()
            .filter(|app| others.iter().all(|ids| ids.contains(app.id.as_str())))
            .collect()
    }
}

/// Make `id` the user's default for `mime`, the way GLib's
/// `g_app_info_set_as_default_for_type` records it: `[Default Applications]`
/// gets `mime=id;`, `[Added Associations]` gets `id` first, and `id` leaves the
/// type's `[Removed Associations]`. Every other line is kept byte for byte. A
/// desktop-specific user list that already names a default for the type is
/// updated too, because it takes precedence. Files are replaced atomically; a
/// symlinked list is written through to its target.
pub fn set_default(paths: &MimeAppsPaths, mime: &str, id: &str) -> io::Result<()> {
    if !valid_mime(mime) || !valid_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid MIME type or desktop file ID",
        ));
    }
    let Some(config_home) = &paths.config_home else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no user configuration directory",
        ));
    };
    for name in paths
        .desktops
        .iter()
        .filter(|name| valid_desktop_name(name))
    {
        let path = config_home.join(format!("{name}-mimeapps.list"));
        let Some(source) = read_list(&path)? else {
            continue;
        };
        if Associations::parse(&source).defaults.contains_key(mime) {
            let mut edited = ListEditor::new(&source);
            edited.set(DEFAULT_GROUP, mime, vec![id.to_owned()]);
            edited.remove_from(REMOVED_GROUP, mime, id);
            write_list(&path, &edited.finish())?;
        }
    }
    let path = config_home.join("mimeapps.list");
    let source = read_list(&path)?.unwrap_or_default();
    let mut edited = ListEditor::new(&source);
    edited.set(DEFAULT_GROUP, mime, vec![id.to_owned()]);
    let current = Associations::parse(&source);
    let mut added = vec![id.to_owned()];
    added.extend(
        current
            .added
            .get(mime)
            .into_iter()
            .flatten()
            .filter(|known| *known != id)
            .cloned(),
    );
    edited.set(ADDED_GROUP, mime, added);
    edited.remove_from(REMOVED_GROUP, mime, id);
    std::fs::create_dir_all(config_home)?;
    write_list(&path, &edited.finish())
}

fn read_list(path: &Path) -> io::Result<Option<String>> {
    match canopa_file::read_bounded(path, MAX_LIST_BYTES) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "mimeapps.list is not UTF-8")),
        Err(error) if error.bytes_read == 0 && !path.exists() => Ok(None),
        Err(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mimeapps.list is unreadable or too large",
        )),
    }
}

fn write_list(path: &Path, contents: &str) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    // Dotfile managers symlink the list; replace the target, keep the link.
    let path = match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::canonicalize(path)?,
        _ => path.to_owned(),
    };
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "list has no folder"))?;
    let mode = std::fs::metadata(&path)
        .map(|meta| std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o777)
        .unwrap_or(0o644);
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "list has no name"))?;
    let mut temporary_name = std::ffi::OsString::from(".");
    temporary_name.push(name);
    temporary_name.push(format!(".{}.tmp", std::process::id()));
    let temporary = parent.join(temporary_name);
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(mode))?;
        file.sync_all()?;
        std::fs::rename(&temporary, &path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written?;
    if let Ok(folder) = std::fs::File::open(parent) {
        let _ = folder.sync_all();
    }
    Ok(())
}

/// Line-preserving edits of a key file: only touched keys are rewritten.
struct ListEditor {
    lines: Vec<String>,
}

impl ListEditor {
    fn new(source: &str) -> Self {
        Self {
            lines: source.lines().map(str::to_owned).collect(),
        }
    }

    /// `(header index, end index)` of the first group named `group`.
    fn group(&self, group: &str) -> Option<(usize, usize)> {
        let header = format!("[{group}]");
        let start = self.lines.iter().position(|line| line.trim() == header)?;
        let end = self.lines[start + 1..]
            .iter()
            .position(|line| line.trim_start().starts_with('['))
            .map_or(self.lines.len(), |offset| start + 1 + offset);
        Some((start, end))
    }

    fn key(&self, group: (usize, usize), key: &str) -> Option<usize> {
        (group.0 + 1..group.1).find(|&index| {
            let line = self.lines[index].trim_start();
            !line.starts_with('#')
                && line
                    .split_once('=')
                    .is_some_and(|(name, _)| name.trim() == key)
        })
    }

    fn value(&self, index: usize) -> Vec<String> {
        self.lines[index]
            .split_once('=')
            .map(|(_, value)| {
                value
                    .split(';')
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn set(&mut self, group: &str, key: &str, ids: Vec<String>) {
        let line = format!("{key}={};", ids.join(";"));
        let span = match self.group(group) {
            Some(span) => span,
            None => {
                if self
                    .lines
                    .last()
                    .is_some_and(|last| !last.trim().is_empty())
                {
                    self.lines.push(String::new());
                }
                self.lines.push(format!("[{group}]"));
                (self.lines.len() - 1, self.lines.len())
            }
        };
        if let Some(index) = self.key(span, key) {
            self.lines[index] = line;
            return;
        }
        // After the group's last non-blank line, keeping a blank separator.
        let mut at = span.1;
        while at > span.0 + 1 && self.lines[at - 1].trim().is_empty() {
            at -= 1;
        }
        self.lines.insert(at, line);
    }

    fn remove_from(&mut self, group: &str, key: &str, id: &str) {
        let Some(span) = self.group(group) else {
            return;
        };
        let Some(index) = self.key(span, key) else {
            return;
        };
        let remaining: Vec<String> = self
            .value(index)
            .into_iter()
            .filter(|known| known != id)
            .collect();
        if remaining.is_empty() {
            self.lines.remove(index);
        } else {
            self.lines[index] = format!("{key}={};", remaining.join(";"));
        }
    }

    fn finish(self) -> String {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn app(dir: &Path, id: &str, extra: &str) {
        write(
            &dir.join("applications").join(id),
            &format!("[Desktop Entry]\nType=Application\nName={id}\nExec=true %f\n{extra}"),
        );
    }

    struct Fixture {
        _root: tempfile::TempDir,
        paths: MimeAppsPaths,
    }

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let paths = MimeAppsPaths {
            config_home: Some(root.path().join("config")),
            config_dirs: vec![root.path().join("xdg")],
            data_home: Some(root.path().join("data")),
            data_dirs: vec![root.path().join("system")],
            desktops: vec!["canopa".into()],
        };
        Fixture { _root: root, paths }
    }

    fn ids(apps: Vec<&DesktopApplication>) -> Vec<&str> {
        apps.into_iter().map(|app| app.id.as_str()).collect()
    }

    fn env() -> DesktopEnvironment {
        DesktopEnvironment::from_current_desktop("canopa")
    }

    #[test]
    fn lists_parse_the_three_groups_and_ignore_everything_else() {
        let parsed = Associations::parse(concat!(
            "# comment\n[Default Applications]\nimage/png = viewer.desktop;editor.desktop\n",
            "image/png=ignored.desktop;\nnot a type=x.desktop;\n[Other]\ntext/plain=no.desktop;\n",
            "[Added Associations]\ntext/plain=edit.desktop;;bad;../x.desktop;\n",
            "[Removed Associations]\ntext/plain=gone.desktop;\n",
        ));
        assert_eq!(
            parsed.defaults["image/png"],
            ["viewer.desktop", "editor.desktop"]
        );
        assert_eq!(parsed.defaults.len(), 1);
        assert_eq!(parsed.added["text/plain"], ["edit.desktop"]);
        assert_eq!(parsed.removed["text/plain"], ["gone.desktop"]);
    }

    #[test]
    fn associations_follow_list_precedence_removals_and_mime_type_declarations() {
        let fixture = fixture();
        let paths = &fixture.paths;
        let system = &paths.data_dirs[0];
        let data = paths.data_home.as_ref().unwrap();
        app(system, "viewer.desktop", "MimeType=image/png;\n");
        app(system, "gimp.desktop", "MimeType=image/png;image/jpeg;\n");
        app(
            system,
            "hidden.desktop",
            "MimeType=image/png;\nNoDisplay=true\n",
        );
        app(system, "removed.desktop", "MimeType=image/png;\n");
        app(
            system,
            "absent.desktop",
            "MimeType=image/png;\nTryExec=/nonexistent/x\n",
        );
        app(data, "user.desktop", "MimeType=image/png;\n");
        // A user entry shadows a system one with the same ID.
        app(data, "shadow.desktop", "MimeType=text/plain;\n");
        app(system, "shadow.desktop", "MimeType=image/png;\n");
        write(
            &system.join("applications/mimeapps.list"),
            "[Removed Associations]\nimage/png=removed.desktop;\n[Added Associations]\nimage/png=hidden.desktop;\n",
        );
        write(
            &paths.config_home.as_ref().unwrap().join("mimeapps.list"),
            "[Added Associations]\nimage/png=gimp.desktop;missing.desktop;\n",
        );
        let apps = MimeApps::load_from(paths, &env());
        assert!(!ids(apps.menu_applications()).contains(&"hidden.desktop"));
        assert!(apps.application("hidden.desktop").is_some());
        assert_eq!(
            ids(apps.associations("image/png")),
            [
                "gimp.desktop",
                "user.desktop",
                "hidden.desktop",
                "viewer.desktop"
            ]
        );
        // A higher list adds back what a lower one removes.
        write(
            &paths.config_dirs[0].join("canopa-mimeapps.list"),
            "[Added Associations]\nimage/png=removed.desktop;\n",
        );
        let apps = MimeApps::load_from(paths, &env());
        assert_eq!(ids(apps.associations("image/png"))[1], "removed.desktop");
        // The user directory removes a system declaration below it.
        write(
            &data.join("applications/mimeapps.list"),
            "[Removed Associations]\nimage/png=viewer.desktop;\n",
        );
        let apps = MimeApps::load_from(paths, &env());
        assert!(!ids(apps.associations("image/png")).contains(&"viewer.desktop"));
        assert_eq!(ids(apps.associations("text/plain")), ["shadow.desktop"]);
    }

    #[test]
    fn defaults_prefer_desktop_specific_lists_then_parents_then_associations() {
        let fixture = fixture();
        let paths = &fixture.paths;
        let system = &paths.data_dirs[0];
        app(system, "editor.desktop", "MimeType=text/plain;\n");
        app(system, "ide.desktop", "MimeType=text/x-python;\n");
        app(system, "notes.desktop", "MimeType=text/plain;\n");
        app(system, "viewer.desktop", "MimeType=image/png;\n");
        let config = paths.config_home.as_ref().unwrap();
        write(
            &config.join("mimeapps.list"),
            "[Default Applications]\ntext/plain=uninstalled.desktop;notes.desktop;\nimage/png=editor.desktop;\n",
        );
        write(
            &config.join("canopa-mimeapps.list"),
            "[Default Applications]\nimage/png=viewer.desktop;\n",
        );
        let apps = MimeApps::load_from(paths, &env());
        let chain = |mime: &str| -> Vec<String> {
            let mut chain = vec![mime.to_owned()];
            if mime != "text/plain" && mime.starts_with("text/") {
                chain.push("text/plain".into());
            }
            chain
        };
        assert_eq!(
            apps.default_for(&chain("image/png")).unwrap().id,
            "viewer.desktop"
        );
        // No explicit default for the type: the parent's explicit default wins
        // over the type's own association, as GLib orders it.
        assert_eq!(
            apps.default_for(&chain("text/x-python")).unwrap().id,
            "notes.desktop"
        );
        assert_eq!(
            ids(apps.applications_for(&chain("text/x-python"))),
            ["notes.desktop", "ide.desktop", "editor.desktop"]
        );
        assert_eq!(
            ids(apps.applications_for_all(&[chain("text/x-python"), chain("text/plain")])),
            ["notes.desktop", "editor.desktop"]
        );
        assert!(apps.default_for(&chain("video/mp4")).is_none());
    }

    #[test]
    fn type_chains_skip_octet_stream_and_reach_text_plain() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("mime");
        write(
            &dir.join("subclasses"),
            "application/x-shellscript application/x-executable\napplication/x-shellscript text/plain\n",
        );
        write(
            &dir.join("aliases"),
            "application/x-sh application/x-shellscript\n",
        );
        let database = MimeDatabase::load(&[dir]);
        assert_eq!(
            type_chain(&database, "application/x-sh"),
            [
                "application/x-shellscript",
                "application/x-executable",
                "text/plain"
            ]
        );
        assert_eq!(
            type_chain(&database, "text/markdown"),
            ["text/markdown", "text/plain"]
        );
        assert_eq!(type_chain(&database, OCTET_STREAM), [OCTET_STREAM]);
    }

    #[test]
    fn setting_a_default_rewrites_only_the_touched_keys() {
        let fixture = fixture();
        let paths = &fixture.paths;
        let config = paths.config_home.as_ref().unwrap();
        let original = concat!(
            "# keep me\n[Default Applications]\nimage/png=old.desktop;\ntext/plain=editor.desktop;\n\n",
            "[Added Associations]\nimage/png=other.desktop;gimp.desktop;\n\n",
            "[Removed Associations]\nimage/png=gimp.desktop;bad.desktop;\nvideo/mp4=gimp.desktop;\n\n",
            "[X-Custom]\nKey=Value\n",
        );
        write(&config.join("mimeapps.list"), original);
        set_default(paths, "image/png", "gimp.desktop").unwrap();
        let written = std::fs::read_to_string(config.join("mimeapps.list")).unwrap();
        assert_eq!(
            written,
            concat!(
                "# keep me\n[Default Applications]\nimage/png=gimp.desktop;\ntext/plain=editor.desktop;\n\n",
                "[Added Associations]\nimage/png=gimp.desktop;other.desktop;\n\n",
                "[Removed Associations]\nimage/png=bad.desktop;\nvideo/mp4=gimp.desktop;\n\n",
                "[X-Custom]\nKey=Value\n",
            )
        );
        // New groups and keys are created; the result reads back as the default.
        set_default(paths, "text/markdown", "editor.desktop").unwrap();
        let parsed =
            Associations::parse(&std::fs::read_to_string(config.join("mimeapps.list")).unwrap());
        assert_eq!(parsed.defaults["text/markdown"], ["editor.desktop"]);
        assert_eq!(parsed.added["text/markdown"], ["editor.desktop"]);
        assert_eq!(parsed.defaults["image/png"], ["gimp.desktop"]);
        assert!(
            !std::fs::read_to_string(config.join("mimeapps.list"))
                .unwrap()
                .contains("\n\n\n")
        );
    }

    #[test]
    fn setting_a_default_creates_the_list_follows_symlinks_and_updates_desktop_lists() {
        let fixture = fixture();
        let paths = &fixture.paths;
        let config = paths.config_home.as_ref().unwrap();
        set_default(paths, "image/png", "viewer.desktop").unwrap();
        assert_eq!(
            std::fs::read_to_string(config.join("mimeapps.list")).unwrap(),
            "[Default Applications]\nimage/png=viewer.desktop;\n\n[Added Associations]\nimage/png=viewer.desktop;\n"
        );
        let target = config.join("dotfiles/mimeapps.list");
        write(
            &target,
            "[Default Applications]\nimage/png=viewer.desktop;\n",
        );
        std::fs::remove_file(config.join("mimeapps.list")).unwrap();
        std::os::unix::fs::symlink(&target, config.join("mimeapps.list")).unwrap();
        write(
            &config.join("canopa-mimeapps.list"),
            "[Default Applications]\nimage/png=viewer.desktop;\ntext/plain=editor.desktop;\n",
        );
        set_default(paths, "image/png", "gimp.desktop").unwrap();
        assert!(
            std::fs::symlink_metadata(config.join("mimeapps.list"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            std::fs::read_to_string(&target)
                .unwrap()
                .contains("image/png=gimp.desktop;")
        );
        assert_eq!(
            std::fs::read_to_string(config.join("canopa-mimeapps.list")).unwrap(),
            "[Default Applications]\nimage/png=gimp.desktop;\ntext/plain=editor.desktop;\n"
        );
        assert!(set_default(paths, "image/png", "../x.desktop").is_err());
        assert!(set_default(paths, "not-a-type", "gimp.desktop").is_err());
    }

    #[test]
    fn the_written_default_is_the_loaded_default() {
        let fixture = fixture();
        let paths = &fixture.paths;
        let system = &paths.data_dirs[0];
        app(system, "viewer.desktop", "MimeType=image/png;\n");
        app(system, "gimp.desktop", "MimeType=image/png;\n");
        let chain = vec!["image/png".to_owned()];
        assert_eq!(
            MimeApps::load_from(paths, &env())
                .default_for(&chain)
                .unwrap()
                .id,
            "gimp.desktop"
        );
        set_default(paths, "image/png", "viewer.desktop").unwrap();
        let apps = MimeApps::load_from(paths, &env());
        assert_eq!(apps.default_for(&chain).unwrap().id, "viewer.desktop");
        assert_eq!(
            ids(apps.applications_for(&chain)),
            ["viewer.desktop", "gimp.desktop"]
        );
    }
}
