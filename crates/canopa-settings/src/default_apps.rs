//! The Default applications page's model: the XDG MIME applications
//! associations (`mimeapps.list`, through `canopa_apps::mime_apps`, ADR 0141)
//! and the terminal `xdg-terminal-exec` opens (`xdg-terminals.list`).
//!
//! Each choice is written at once, as GLib records a user's choice, so other
//! desktops and toolkits see it. The page follows outside edits by checking
//! the lists and application folders every few seconds while it is shown.
use crate::live::{LiveAction, LiveService, ServiceError};
use canopa_apps::mime_apps::{self, MimeApps, MimeAppsPaths};
use canopa_file::mime::MimeDatabase;
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

/// A kind of application and the types it opens. The first type is the one
/// whose default the page shows.
#[derive(Debug)]
pub struct Category {
    pub key: &'static str,
    pub types: &'static [&'static str],
}

impl Category {
    /// The kind's name on the page.
    pub fn title(&self) -> String {
        match self.key {
            "browser" => fl!("settings-default-apps-browser"),
            "mail" => fl!("settings-default-apps-mail"),
            "calendar" => fl!("settings-default-apps-calendar"),
            "music" => fl!("settings-default-apps-music"),
            "video" => fl!("settings-default-apps-video"),
            "photos" => fl!("settings-default-apps-photos"),
            "text" => fl!("settings-default-apps-text"),
            _ => fl!("settings-default-apps-files"),
        }
    }
}

pub static CATEGORIES: &[Category] = &[
    Category {
        key: "browser",
        types: &[
            "x-scheme-handler/http",
            "x-scheme-handler/https",
            "text/html",
            "application/xhtml+xml",
            "x-scheme-handler/about",
            "x-scheme-handler/unknown",
        ],
    },
    Category {
        key: "mail",
        types: &["x-scheme-handler/mailto", "message/rfc822"],
    },
    Category {
        key: "calendar",
        types: &["text/calendar", "x-scheme-handler/webcal"],
    },
    Category {
        key: "music",
        types: &[
            "audio/mpeg",
            "audio/flac",
            "audio/x-flac",
            "audio/ogg",
            "audio/x-vorbis+ogg",
            "audio/x-opus+ogg",
            "audio/mp4",
            "audio/aac",
            "audio/x-wav",
            "audio/x-mpegurl",
        ],
    },
    Category {
        key: "video",
        types: &[
            "video/mp4",
            "video/x-matroska",
            "video/webm",
            "video/mpeg",
            "video/quicktime",
            "video/x-msvideo",
            "video/ogg",
        ],
    },
    Category {
        key: "photos",
        types: &[
            "image/jpeg",
            "image/png",
            "image/gif",
            "image/webp",
            "image/tiff",
            "image/bmp",
            "image/avif",
            "image/heif",
        ],
    },
    Category {
        key: "text",
        types: &["text/plain"],
    },
    Category {
        key: "files",
        types: &["inode/directory"],
    },
];

/// One installed application a choice can name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppOption {
    /// The desktop file ID, with `.desktop`.
    pub id: String,
    pub name: String,
}

/// A choice: what is used now and what could be.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choice {
    pub current: Option<String>,
    pub options: Vec<AppOption>,
}

impl Choice {
    pub fn name(&self, id: &str) -> Option<&str> {
        self.options
            .iter()
            .find(|option| option.id == id)
            .map(|option| option.name.as_str())
    }
}

/// The associations read once, kept for searching other file types.
#[derive(Clone)]
pub struct Catalog {
    apps: Arc<MimeApps>,
    database: Arc<MimeDatabase>,
}

impl std::fmt::Debug for Catalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Catalog")
    }
}

impl Catalog {
    fn choice(&self, mime: &str) -> Choice {
        let chain = mime_apps::type_chain(&self.database, mime);
        Choice {
            current: self.apps.default_for(&chain).map(|app| app.id.clone()),
            options: self
                .apps
                .applications_for(&chain)
                .into_iter()
                .map(|app| AppOption {
                    id: app.id.clone(),
                    name: app.name.clone(),
                })
                .collect(),
        }
    }

    /// File types matching `query`: a type name, part of one, or an
    /// extension such as `pdf`. At most `limit`, with their choices.
    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, Choice)> {
        let query = query
            .trim()
            .trim_start_matches("*.")
            .trim_start_matches('.');
        let query = query.to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let mut types: Vec<String> = Vec::new();
        let mut push = |mime: &str| {
            let mime = self.database.unalias(mime).to_owned();
            if mime_apps::valid_mime(&mime) && !types.contains(&mime) {
                types.push(mime);
            }
        };
        if query.contains('/') {
            push(&query);
        } else if query.len() <= 16 && query.bytes().all(|b| b.is_ascii_alphanumeric()) {
            for mime in self.database.by_name(&format!("file.{query}")) {
                push(&mime);
            }
        }
        let mut declared: Vec<&String> = self
            .apps
            .applications()
            .iter()
            .flat_map(|app| self.apps.declared_types(&app.id))
            .filter(|mime| mime.contains(&query))
            .collect();
        declared.sort();
        declared.dedup();
        for mime in declared {
            push(mime);
        }
        types
            .into_iter()
            .map(|mime| {
                let choice = self.choice(&mime);
                (mime, choice)
            })
            .filter(|(_, choice)| !choice.options.is_empty())
            .take(limit)
            .collect()
    }
}

/// What the page shows.
#[derive(Clone, Debug)]
pub struct DefaultApps {
    /// One choice per [`CATEGORIES`] entry, in order.
    pub categories: Vec<Choice>,
    /// Terminal emulators; `current` is what `xdg-terminal-exec` opens, when a
    /// list names one.
    pub terminal: Choice,
    pub catalog: Catalog,
}

impl PartialEq for DefaultApps {
    fn eq(&self, other: &Self) -> bool {
        self.categories == other.categories && self.terminal == other.terminal
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefaultAction {
    /// A [`Category::key`] and the application to use.
    Category(&'static str, String),
    /// One file type and the application to use.
    Type(String, String),
    Terminal(String),
}

impl LiveAction for DefaultAction {
    fn control(&self) -> String {
        match self {
            Self::Category(key, _) => format!("category-{key}"),
            Self::Type(mime, _) => format!("type-{mime}"),
            Self::Terminal(_) => "terminal".into(),
        }
    }
}

const TERMINALS_LIST: &str = "xdg-terminals.list";
const MAX_LIST_BYTES: usize = 64 * 1024;
/// How often the page checks for outside changes.
const CHECK: Duration = Duration::from_secs(2);

fn desktop_entry_categories(path: &Path) -> Vec<String> {
    let Ok(bytes) = canopa_file::read_bounded(path, 256 * 1024) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry && let Some(value) = line.strip_prefix("Categories=") {
            return value
                .split(';')
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect();
        }
    }
    Vec::new()
}

/// The desktop IDs a terminals list names, in order, without actions.
fn terminal_ids(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let id = line.trim_start_matches(['+', '-']);
            let id = id.split(':').next().unwrap_or(id).trim();
            (!line.starts_with('-') && mime_apps::valid_id(id)).then(|| id.to_owned())
        })
        .collect()
}

/// `text` with `id` first and its earlier lines removed; comments and the
/// other entries keep their order.
pub fn with_terminal_first(text: &str, id: &str) -> String {
    let mut lines = vec![id.to_owned()];
    for line in text.lines() {
        let entry = line.trim().trim_start_matches(['+', '-']);
        let entry = entry.split(':').next().unwrap_or(entry).trim();
        if entry != id {
            lines.push(line.to_owned());
        }
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

fn read_list(path: &Path) -> Option<String> {
    canopa_file::read_bounded(path, MAX_LIST_BYTES)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

/// Replace a small list atomically, through a symlink to its target.
fn write_list(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let path = match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::canonicalize(path)?,
        _ => path.to_owned(),
    };
    let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(text.as_bytes())?;
    file.as_file().sync_all()?;
    if let Ok(meta) = std::fs::metadata(&path) {
        file.as_file().set_permissions(meta.permissions())?;
    }
    file.persist(&path).map_err(|error| error.error)?;
    Ok(())
}

/// The Default applications page's source: the XDG lists of this session.
#[derive(Clone, Debug)]
pub struct DefaultAppsService {
    paths: MimeAppsPaths,
    /// The desktop environment for reading entries, and the MIME database
    /// from the data folders; `None` uses this session's (tests set their own).
    desktop: Option<String>,
}

impl Default for DefaultAppsService {
    fn default() -> Self {
        Self {
            paths: MimeAppsPaths::current(),
            desktop: None,
        }
    }
}

impl DefaultAppsService {
    /// A service over explicit locations (tests).
    pub fn with_paths(paths: MimeAppsPaths) -> Self {
        Self {
            desktop: Some(paths.desktops.join(":")),
            paths,
        }
    }

    fn environment(&self) -> canopa_apps::DesktopEnvironment {
        match &self.desktop {
            Some(names) => canopa_apps::DesktopEnvironment::from_current_desktop(names),
            None => canopa_apps::DesktopEnvironment::current(),
        }
    }

    /// The terminals lists `xdg-terminal-exec` reads, highest precedence first.
    fn terminal_lists(&self) -> Vec<PathBuf> {
        let mut lists = Vec::new();
        for dir in self.paths.config_home.iter().chain(&self.paths.config_dirs) {
            for desktop in &self.paths.desktops {
                lists.push(dir.join(format!("{desktop}-{TERMINALS_LIST}")));
            }
            lists.push(dir.join(TERMINALS_LIST));
        }
        lists
    }

    fn load(&self) -> Result<DefaultApps, ServiceError> {
        if self.paths.config_home.is_none() {
            return Err(ServiceError::Unavailable(fl!(
                "settings-default-apps-no-config"
            )));
        }
        let catalog = Catalog {
            apps: Arc::new(MimeApps::load_from(&self.paths, &self.environment())),
            database: match &self.desktop {
                Some(_) => Arc::new(MimeDatabase::load(&mime_dirs(&self.paths))),
                None => MimeDatabase::system(),
            },
        };
        let categories = CATEGORIES
            .iter()
            .map(|category| catalog.choice(category.types[0]))
            .collect();
        let mut terminals: Vec<AppOption> = catalog
            .apps
            .menu_applications()
            .into_iter()
            .filter(|app| {
                desktop_entry_categories(&app.desktop_file)
                    .iter()
                    .any(|category| category == "TerminalEmulator")
            })
            .map(|app| AppOption {
                id: app.id.clone(),
                name: app.name.clone(),
            })
            .collect();
        terminals.sort_by_key(|option| option.name.to_lowercase());
        let current = self
            .terminal_lists()
            .iter()
            .filter_map(|path| read_list(path))
            .flat_map(|text| terminal_ids(&text))
            .find(|id| terminals.iter().any(|option| &option.id == id));
        Ok(DefaultApps {
            categories,
            terminal: Choice {
                current,
                options: terminals,
            },
            catalog,
        })
    }

    fn run(&self, action: &DefaultAction) -> Result<(), ServiceError> {
        let failed = |error: std::io::Error| {
            ServiceError::Failed(fl!(
                "settings-default-apps-not-saved",
                error = error.to_string()
            ))
        };
        match action {
            DefaultAction::Category(key, id) => {
                let category = CATEGORIES
                    .iter()
                    .find(|category| category.key == *key)
                    .ok_or_else(|| {
                        ServiceError::Failed(fl!("settings-default-apps-unknown-kind"))
                    })?;
                let catalog = self.load()?.catalog;
                // Every type of the kind that the application opens, and always the first.
                for (index, mime) in category.types.iter().enumerate() {
                    let opens = catalog.choice(mime).options.iter().any(|o| &o.id == id);
                    if index == 0 || opens {
                        mime_apps::set_default(&self.paths, mime, id).map_err(failed)?;
                    }
                }
                Ok(())
            }
            DefaultAction::Type(mime, id) => {
                mime_apps::set_default(&self.paths, mime, id).map_err(failed)
            }
            DefaultAction::Terminal(id) => {
                if !mime_apps::valid_id(id) {
                    return Err(ServiceError::Failed(fl!(
                        "settings-default-apps-unknown-terminal"
                    )));
                }
                let config =
                    self.paths.config_home.as_ref().ok_or_else(|| {
                        ServiceError::Failed(fl!("settings-region-error-no-config"))
                    })?;
                // A desktop-specific list wins, so it is updated when it exists.
                for desktop in &self.paths.desktops {
                    let path = config.join(format!("{desktop}-{TERMINALS_LIST}"));
                    if let Some(text) = read_list(&path) {
                        write_list(&path, &with_terminal_first(&text, id)).map_err(failed)?;
                    }
                }
                let path = config.join(TERMINALS_LIST);
                let text = read_list(&path).unwrap_or_default();
                write_list(&path, &with_terminal_first(&text, id)).map_err(failed)
            }
        }
    }

    /// Modification times of everything a reading depends on.
    fn signature(&self) -> Vec<Option<SystemTime>> {
        let mut files: Vec<PathBuf> = Vec::new();
        for dir in self.paths.config_home.iter().chain(&self.paths.config_dirs) {
            files.push(dir.join("mimeapps.list"));
            for desktop in &self.paths.desktops {
                files.push(dir.join(format!("{desktop}-mimeapps.list")));
            }
        }
        files.extend(self.terminal_lists());
        files.extend(self.paths.application_dirs());
        files
            .iter()
            .map(|path| {
                std::fs::metadata(path)
                    .and_then(|meta| meta.modified())
                    .ok()
            })
            .collect()
    }
}

fn mime_dirs(paths: &MimeAppsPaths) -> Vec<PathBuf> {
    paths
        .data_home
        .iter()
        .chain(&paths.data_dirs)
        .map(|dir| dir.join("mime"))
        .collect()
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ServiceError> + Send + 'static,
) -> Result<T, ServiceError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| ServiceError::Failed(fl!("settings-default-apps-read-failed")))?
}

impl LiveService for DefaultAppsService {
    type State = DefaultApps;
    type Action = DefaultAction;

    fn name(&self) -> String {
        fl!("settings-service-applications")
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<DefaultApps, ServiceError>> {
        let service = self.clone();
        blocking(move || service.load()).boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<DefaultApps, ServiceError>> {
        let service = self.clone();
        futures_util::stream::unfold(None, move |last| {
            let service = service.clone();
            async move {
                let mut last = last;
                loop {
                    let reader = service.clone();
                    let signature = blocking(move || Ok(reader.signature())).await.ok();
                    if last.is_some() && signature != last {
                        let reader = service.clone();
                        let state = blocking(move || reader.load()).await;
                        return Some((state, signature));
                    }
                    last = signature;
                    tokio::time::sleep(CHECK).await;
                }
            }
        })
        .boxed()
    }

    fn apply(
        &self,
        action: DefaultAction,
    ) -> BoxFuture<'static, Result<Option<DefaultApps>, ServiceError>> {
        let service = self.clone();
        blocking(move || {
            service.run(&action)?;
            service.load().map(Some)
        })
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(dir: &Path, id: &str, name: &str, extra: &str) {
        std::fs::write(
            dir.join(id),
            format!("[Desktop Entry]\nType=Application\nName={name}\nExec=/bin/true %U\n{extra}\n"),
        )
        .unwrap();
    }

    fn fixture() -> (tempfile::TempDir, MimeAppsPaths) {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config");
        let data = root.path().join("data");
        let apps = data.join("applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        entry(
            &apps,
            "firefox.desktop",
            "Firefox",
            "MimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;",
        );
        entry(
            &apps,
            "web.desktop",
            "Web",
            "MimeType=x-scheme-handler/http;text/html;",
        );
        entry(
            &apps,
            "reader.desktop",
            "Reader",
            "MimeType=application/pdf;",
        );
        entry(
            &apps,
            "foot.desktop",
            "Foot",
            "Categories=System;TerminalEmulator;",
        );
        entry(
            &apps,
            "kitty.desktop",
            "kitty",
            "Categories=TerminalEmulator;",
        );
        let paths = MimeAppsPaths {
            config_home: Some(config),
            config_dirs: vec![],
            data_home: Some(data),
            data_dirs: vec![],
            desktops: vec!["canopa".into()],
        };
        (root, paths)
    }

    #[tokio::test]
    async fn choices_are_written_like_glib_and_followed() {
        let (_root, paths) = fixture();
        let service = DefaultAppsService::with_paths(paths.clone());
        let state = service.snapshot().await.unwrap();
        let browser = &state.categories[0];
        assert_eq!(browser.options.len(), 2);
        assert_eq!(state.terminal.options.len(), 2);
        assert_eq!(state.terminal.current, None);

        let state = service
            .apply(DefaultAction::Category("browser", "web.desktop".into()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.categories[0].current.as_deref(), Some("web.desktop"));
        let list = std::fs::read_to_string(paths.user_list().unwrap()).unwrap();
        assert!(
            list.contains("x-scheme-handler/http=web.desktop;"),
            "{list}"
        );
        assert!(list.contains("text/html=web.desktop;"));
        // Web does not open https; that default is left alone.
        assert!(!list.contains("x-scheme-handler/https=web.desktop"));

        let state = service
            .apply(DefaultAction::Terminal("kitty.desktop".into()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.terminal.current.as_deref(), Some("kitty.desktop"));
        let config = paths.config_home.clone().unwrap();
        std::fs::write(
            config.join("canopa-xdg-terminals.list"),
            "# mine\nfoot.desktop\n",
        )
        .unwrap();
        let mut updates = service.updates();
        let state = service.snapshot().await.unwrap();
        // The desktop-specific list wins.
        assert_eq!(state.terminal.current.as_deref(), Some("foot.desktop"));
        service
            .apply(DefaultAction::Terminal("kitty.desktop".into()))
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(config.join("canopa-xdg-terminals.list")).unwrap(),
            "kitty.desktop\n# mine\nfoot.desktop\n"
        );

        // Outside edits are followed.
        let _ = tokio::time::timeout(Duration::from_millis(100), updates.next()).await;
        std::fs::write(
            paths.user_list().unwrap(),
            "[Default Applications]\nx-scheme-handler/http=firefox.desktop;\n",
        )
        .unwrap();
        let state = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let state = updates.next().await.unwrap().unwrap();
                if state.categories[0].current.as_deref() == Some("firefox.desktop") {
                    return state;
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(state.categories[0].name("firefox.desktop"), Some("Firefox"));
    }

    #[tokio::test]
    async fn other_types_are_found_by_name_or_extension() {
        let (_root, paths) = fixture();
        let service = DefaultAppsService::with_paths(paths.clone());
        let state = service.snapshot().await.unwrap();
        let found = state.catalog.search("pdf", 10);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "application/pdf");
        assert_eq!(found[0].1.current.as_deref(), Some("reader.desktop"));
        assert!(state.catalog.search("", 10).is_empty());
        assert!(state.catalog.search("nothing/here", 10).is_empty());
        service
            .apply(DefaultAction::Type(
                "application/pdf".into(),
                "reader.desktop".into(),
            ))
            .await
            .unwrap();
        assert!(
            std::fs::read_to_string(paths.user_list().unwrap())
                .unwrap()
                .contains("application/pdf=reader.desktop;")
        );
    }

    #[test]
    fn terminal_lists_keep_comments_and_order() {
        assert_eq!(
            terminal_ids("# c\nfoot.desktop:new-window\n-xterm.desktop\nbad\nkitty.desktop\n"),
            ["foot.desktop", "kitty.desktop"]
        );
        assert_eq!(
            with_terminal_first("# c\nfoot.desktop\nkitty.desktop:x\n", "kitty.desktop"),
            "kitty.desktop\n# c\nfoot.desktop\n"
        );
        assert_eq!(with_terminal_first("", "foot.desktop"), "foot.desktop\n");
    }
}
