//! Desktop-entry discovery and localized presentation metadata. Call on a worker.
//! Consumers retain launch, chooser selection, visibility and process lifetimes.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApplication {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub keywords: Vec<String>,
    pub icon: Option<String>,
    pub desktop_file: PathBuf,
    pub dbus_activatable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawDesktopEntry {
    pub id: String,
    pub body: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEnvironment {
    names: Vec<String>,
    known: bool,
    locales: Vec<String>,
}

impl DesktopEnvironment {
    pub fn unknown() -> Self {
        Self {
            names: Vec::new(),
            known: false,
            locales: Vec::new(),
        }
    }

    pub fn current() -> Self {
        let mut env = std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| Self::from_current_desktop(&value))
            .unwrap_or_else(Self::unknown);
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(|key| std::env::var(key).ok())
            .find(|value| !value.is_empty());
        if let Some(locale) =
            locale.filter(|value| value != "C" && value != "POSIX" && !value.starts_with("C."))
        {
            env.locales = std::env::var("LANGUAGE")
                .ok()
                .filter(|value| !value.is_empty())
                .map(|value| value.split(':').map(str::to_owned).collect())
                .unwrap_or_else(|| vec![locale]);
        }
        env
    }

    pub fn from_current_desktop(value: &str) -> Self {
        Self {
            names: value
                .split(':')
                .map(|name| name.trim().to_ascii_lowercase())
                .filter(|name| !name.is_empty())
                .collect(),
            known: true,
            locales: Vec::new(),
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.names
            .iter()
            .any(|candidate| candidate == &name.to_ascii_lowercase())
    }
}

pub fn discover_installed_applications() -> std::io::Result<Vec<DesktopApplication>> {
    discover_installed_applications_in_data_dirs(xdg_data_dirs(), &DesktopEnvironment::current())
}

/// Metadata for frontend-supplied application choices. Menu-only restrictions
/// must not erase a legitimate file handler supplied by the portal frontend.
pub fn discover_choice_metadata() -> std::io::Result<Vec<DesktopApplication>> {
    discover_in_data_dirs(
        xdg_data_dirs(),
        &DesktopEnvironment::current(),
        Purpose::Chooser,
    )
}

pub fn discover_installed_applications_in_data_dirs(
    data_dirs: Vec<PathBuf>,
    env: &DesktopEnvironment,
) -> std::io::Result<Vec<DesktopApplication>> {
    discover_in_data_dirs(data_dirs, env, Purpose::Launcher)
}

fn discover_in_data_dirs(
    data_dirs: Vec<PathBuf>,
    env: &DesktopEnvironment,
    purpose: Purpose,
) -> std::io::Result<Vec<DesktopApplication>> {
    let mut entries = Vec::new();
    for data_dir in data_dirs {
        let applications_dir = data_dir.join("applications");
        collect_desktop_entries(&applications_dir, &applications_dir, &mut entries)?;
    }
    Ok(applications(entries, env, purpose))
}

pub fn applications_from_entries(
    entries: Vec<RawDesktopEntry>,
    env: &DesktopEnvironment,
) -> Vec<DesktopApplication> {
    applications(entries, env, Purpose::Launcher)
}

/// Hidden user overrides still suppress lower-priority system metadata.
pub fn choice_metadata_from_entries(
    entries: Vec<RawDesktopEntry>,
    env: &DesktopEnvironment,
) -> Vec<DesktopApplication> {
    applications(entries, env, Purpose::Chooser)
}

#[derive(Clone, Copy)]
enum Purpose {
    Launcher,
    Chooser,
}

fn applications(
    entries: Vec<RawDesktopEntry>,
    env: &DesktopEnvironment,
    purpose: Purpose,
) -> Vec<DesktopApplication> {
    let mut seen = HashSet::new();
    let mut apps = entries
        .into_iter()
        .filter(|entry| seen.insert(entry.id.clone()))
        .filter_map(|entry| application_from_entry(entry, env, purpose))
        .collect::<Vec<_>>();
    apps.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
            .then_with(|| {
                left.id
                    .to_ascii_lowercase()
                    .cmp(&right.id.to_ascii_lowercase())
            })
    });
    apps
}

fn xdg_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    if let Some(data_home) = data_home {
        dirs.push(data_home);
    }
    match std::env::var_os("XDG_DATA_DIRS") {
        Some(value) => dirs.extend(std::env::split_paths(&value)),
        None => {
            dirs.push("/usr/local/share".into());
            dirs.push("/usr/share".into());
        }
    }
    dirs
}

fn collect_desktop_entries(
    root: &Path,
    dir: &Path,
    entries: &mut Vec<RawDesktopEntry>,
) -> std::io::Result<()> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for item in read_dir {
        let Ok(item) = item else { continue };
        let path = item.path();
        let Ok(file_type) = item.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_desktop_entries(root, &path, entries)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("desktop") {
            let body = match canopa_file::read_bounded(&path, 256 * 1024)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
            {
                Some(body) => body,
                None => {
                    tracing::debug!(target: "canopa_apps", "skipping unavailable desktop entry");
                    continue;
                }
            };
            let relative = path.strip_prefix(root).unwrap_or(&path);
            let id = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("-");
            entries.push(RawDesktopEntry { id, body, path });
        }
    }
    Ok(())
}

fn application_from_entry(
    entry: RawDesktopEntry,
    env: &DesktopEnvironment,
    purpose: Purpose,
) -> Option<DesktopApplication> {
    let fields = parse_desktop_entry_fields(&entry.body);
    if fields.get("Type").map(String::as_str) != Some("Application") {
        return None;
    }
    if bool_field(&fields, "Hidden") {
        return None;
    }
    if matches!(purpose, Purpose::Launcher) {
        if bool_field(&fields, "NoDisplay") || bool_field(&fields, "Terminal") {
            return None;
        }
        if let Some(only_show_in) = fields.get("OnlyShowIn")
            && (!env.known
                || !semicolon_list(only_show_in)
                    .iter()
                    .any(|name| env.contains(name)))
        {
            return None;
        }
        if let Some(not_show_in) = fields.get("NotShowIn")
            && semicolon_list(not_show_in)
                .iter()
                .any(|name| env.contains(name))
        {
            return None;
        }
    }
    let name = localized_field(&fields, "Name", &env.locales)?;
    if non_empty_field(&fields, "Exec").is_none() && !bool_field(&fields, "DBusActivatable") {
        return None;
    }
    Some(DesktopApplication {
        id: entry.id,
        name,
        generic_name: localized_field(&fields, "GenericName", &env.locales),
        comment: localized_field(&fields, "Comment", &env.locales),
        keywords: localized_field(&fields, "Keywords", &env.locales)
            .as_deref()
            .map(|value| {
                semicolon_list(value)
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        icon: optional_non_empty_field(&fields, "Icon"),
        desktop_file: entry.path,
        dbus_activatable: bool_field(&fields, "DBusActivatable"),
    })
}

fn localized_field(
    fields: &HashMap<String, String>,
    key: &str,
    locales: &[String],
) -> Option<String> {
    for locale in locales {
        let (locale, modifier) = locale
            .split_once('@')
            .map_or((locale.as_str(), None), |(base, modifier)| {
                (base, Some(modifier))
            });
        let base = locale.split('.').next().unwrap_or(locale);
        let language = base.split('_').next().unwrap_or(base);
        let mut candidates = Vec::new();
        if let Some(modifier) = modifier {
            candidates.push(format!("{base}@{modifier}"));
        }
        candidates.push(base.to_owned());
        if let Some(modifier) = modifier {
            candidates.push(format!("{language}@{modifier}"));
        }
        candidates.push(language.to_owned());
        for locale in candidates {
            if let Some(value) = optional_non_empty_field(fields, &format!("{key}[{locale}]")) {
                return Some(unescape_string(&value));
            }
        }
    }
    optional_non_empty_field(fields, key).map(|value| unescape_string(&value))
}

fn unescape_string(value: &str) -> String {
    let mut chars = value.chars();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push(ch);
            continue;
        }
        match chars.next() {
            Some('s') => result.push(' '),
            Some('n') => result.push('\n'),
            Some('t') => result.push('\t'),
            Some('r') => result.push('\r'),
            Some('\\') => result.push('\\'),
            Some(ch) => {
                result.push('\\');
                result.push(ch);
            }
            None => result.push('\\'),
        }
    }
    result
}

fn parse_desktop_entry_fields(body: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let mut in_desktop_entry = false;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            fields.insert(key.to_string(), value.to_string());
        }
    }
    fields
}

fn bool_field(fields: &HashMap<String, String>, key: &str) -> bool {
    fields
        .get(key)
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn non_empty_field(fields: &HashMap<String, String>, key: &str) -> Option<String> {
    optional_non_empty_field(fields, key)
}

fn optional_non_empty_field(fields: &HashMap<String, String>, key: &str) -> Option<String> {
    fields
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn semicolon_list(value: &str) -> Vec<&str> {
    value
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_metadata_uses_locale_fallbacks_and_retains_entry_path() {
        let mut env = DesktopEnvironment::from_current_desktop("niri");
        env.locales = vec!["fr_CA.UTF-8@custom".into(), "en".into()];
        let apps = applications_from_entries(
            vec![entry(
                "app.desktop",
                concat!(
                    "[Desktop Entry]\nType=Application\nName=Default\nExec=true\n",
                    "Name[fr]=Français\nName[fr_CA]=Québécois\n",
                    "GenericName[fr]=Éditeur\\sde texte\nComment[fr]=Description\n",
                    "Keywords[fr]=texte;édition;\n"
                ),
            )],
            &env,
        );
        assert_eq!(apps[0].name, "Québécois");
        assert_eq!(apps[0].generic_name.as_deref(), Some("Éditeur de texte"));
        assert_eq!(apps[0].comment.as_deref(), Some("Description"));
        assert_eq!(apps[0].keywords, ["texte", "édition"]);
        assert_eq!(apps[0].desktop_file, PathBuf::from("app.desktop"));
    }

    #[test]
    fn dbus_activatable_application_does_not_require_exec() {
        let apps = applications_from_entries(
            vec![entry(
                "org.example.App.desktop",
                "[Desktop Entry]\nType=Application\nName=Bus App\nDBusActivatable=true\n",
            )],
            &DesktopEnvironment::unknown(),
        );
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Bus App");
    }

    fn entry(id: &str, body: &str) -> RawDesktopEntry {
        RawDesktopEntry {
            id: id.into(),
            body: body.into(),
            path: id.into(),
        }
    }

    #[test]
    fn filters_to_launchable_gui_apps_and_sorts_by_name() {
        let entries = vec![
            entry(
                "z.desktop",
                "[Desktop Entry]\nType=Application\nName=Zed\nExec=zed\n",
            ),
            entry(
                "hidden.desktop",
                "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nHidden=true\n",
            ),
            entry(
                "terminal.desktop",
                "[Desktop Entry]\nType=Application\nName=Terminal App\nExec=vim\nTerminal=true\n",
            ),
            entry(
                "a.desktop",
                "[Desktop Entry]\nType=Application\nName=Alpha\nExec=alpha\nNoDisplay=false\n",
            ),
            entry(
                "link.desktop",
                "[Desktop Entry]\nType=Link\nName=Docs\nURL=https://example.com\n",
            ),
        ];

        let apps = applications_from_entries(entries, &DesktopEnvironment::unknown());

        assert_eq!(
            apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
            vec!["Alpha", "Zed"]
        );
    }

    #[test]
    fn excludes_only_show_in_when_current_desktop_is_unknown() {
        let entries = vec![entry(
            "gnome-only.desktop",
            "[Desktop Entry]\nType=Application\nName=GNOME Only\nExec=gnome-app\nOnlyShowIn=GNOME;\n",
        )];

        let apps = applications_from_entries(entries, &DesktopEnvironment::unknown());

        assert!(apps.is_empty());
    }

    #[test]
    fn honors_current_desktop_for_show_in_rules() {
        let env = DesktopEnvironment::from_current_desktop("GNOME:niri");
        let entries = vec![
            entry(
                "gnome.desktop",
                "[Desktop Entry]\nType=Application\nName=GNOME App\nExec=app\nOnlyShowIn=GNOME;\n",
            ),
            entry(
                "blocked.desktop",
                "[Desktop Entry]\nType=Application\nName=Blocked\nExec=blocked\nNotShowIn=niri;\n",
            ),
        ];

        let apps = applications_from_entries(entries, &env);

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "GNOME App");
    }

    #[test]
    fn extracts_search_metadata() {
        let entries = vec![entry(
            "browser.desktop",
            "[Desktop Entry]\nType=Application\nName=Browser\nGenericName=Web Browser\nComment=Browse the web\nKeywords=web;internet;\nIcon=browser\nExec=browser %U\n",
        )];

        let apps = applications_from_entries(entries, &DesktopEnvironment::unknown());

        assert_eq!(apps[0].generic_name.as_deref(), Some("Web Browser"));
        assert_eq!(apps[0].comment.as_deref(), Some("Browse the web"));
        assert_eq!(apps[0].keywords, vec!["web", "internet"]);
        assert_eq!(apps[0].icon.as_deref(), Some("browser"));
    }
    #[test]
    fn user_hidden_override_suppresses_system_entry_before_filtering() {
        let entries = vec![
            RawDesktopEntry {
                id: "app.desktop".into(),
                path: "app.desktop".into(),
                body: "[Desktop Entry]\nType=Application\nName=Hidden\nHidden=true\nExec=true\n"
                    .into(),
            },
            RawDesktopEntry {
                id: "app.desktop".into(),
                path: "app.desktop".into(),
                body: "[Desktop Entry]\nType=Application\nName=System\nExec=true\n".into(),
            },
        ];
        assert!(applications_from_entries(entries, &DesktopEnvironment::unknown()).is_empty());
    }

    #[test]
    fn unreadable_entry_does_not_hide_other_applications() {
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("applications");
        std::fs::create_dir(&apps).unwrap();
        std::fs::write(apps.join("broken.desktop"), [0xff]).unwrap();
        std::fs::write(
            apps.join("working.desktop"),
            "[Desktop Entry]\nType=Application\nName=Working\nExec=true\n",
        )
        .unwrap();
        let found = discover_installed_applications_in_data_dirs(
            vec![dir.path().into()],
            &DesktopEnvironment::unknown(),
        )
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Working");
    }

    #[test]
    fn chooser_metadata_keeps_explicit_handlers_outside_the_launcher_menu() {
        let entries = vec![
            entry(
                "handler.desktop",
                "[Desktop Entry]\nType=Application\nName=File handler\nExec=handler %f\nNoDisplay=true\n",
            ),
            entry(
                "terminal.desktop",
                "[Desktop Entry]\nType=Application\nName=Terminal editor\nExec=editor %f\nTerminal=true\n",
            ),
            entry(
                "other-desktop.desktop",
                "[Desktop Entry]\nType=Application\nName=Other desktop tool\nExec=tool %f\nOnlyShowIn=GNOME;\n",
            ),
        ];
        let environment = DesktopEnvironment::from_current_desktop("niri");
        assert!(applications_from_entries(entries.clone(), &environment).is_empty());
        let metadata = choice_metadata_from_entries(entries, &environment);
        assert_eq!(
            metadata
                .iter()
                .map(|app| app.id.as_str())
                .collect::<Vec<_>>(),
            [
                "handler.desktop",
                "other-desktop.desktop",
                "terminal.desktop"
            ]
        );
    }

    #[test]
    fn hidden_user_override_suppresses_chooser_metadata_too() {
        let entries = vec![
            entry(
                "handler.desktop",
                "[Desktop Entry]\nType=Application\nName=Hidden\nHidden=true\n",
            ),
            entry(
                "handler.desktop",
                "[Desktop Entry]\nType=Application\nName=System handler\nExec=handler %f\n",
            ),
        ];
        assert!(choice_metadata_from_entries(entries, &DesktopEnvironment::unknown()).is_empty());
    }

    #[test]
    fn oversized_entries_do_not_block_discovery_of_regular_applications() {
        let directory = tempfile::tempdir().unwrap();
        let applications = directory.path().join("applications");
        std::fs::create_dir(&applications).unwrap();
        std::fs::write(
            applications.join("large.desktop"),
            vec![b'x'; 256 * 1024 + 1],
        )
        .unwrap();
        std::fs::write(
            applications.join("normal.desktop"),
            "[Desktop Entry]\nType=Application\nName=Normal\nExec=true\n",
        )
        .unwrap();
        let found = discover_installed_applications_in_data_dirs(
            vec![directory.path().into()],
            &DesktopEnvironment::unknown(),
        )
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "normal.desktop");
    }
}
