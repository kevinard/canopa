use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationIcon {
    pub(crate) name: String,
    pub(crate) path: Option<PathBuf>,
}

pub(crate) fn resolve_application_icon(app_id: &str) -> Option<ApplicationIcon> {
    if app_id.is_empty() {
        return None;
    }

    let cache = icon_cache();
    if let Some(icon) = cache
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get(app_id)
        .cloned()
    {
        return icon;
    }

    let resolved = resolve_application_icon_uncached(app_id);
    cache
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert(app_id.to_string(), resolved.clone());
    resolved
}

fn icon_cache() -> &'static Mutex<HashMap<String, Option<ApplicationIcon>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<ApplicationIcon>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

type Catalog = Arc<Vec<crate::desktop_apps::DesktopApplication>>;

fn catalog() -> &'static RwLock<Option<Catalog>> {
    static APPLICATIONS: RwLock<Option<Catalog>> = RwLock::new(None);
    &APPLICATIONS
}

/// Installed applications: discovered on first use, then replaced by each
/// catalog the launcher reads ([`publish_installed_applications`]).
pub(crate) fn installed_applications() -> Catalog {
    if let Some(catalog) = catalog()
        .read()
        .unwrap_or_else(|poison| poison.into_inner())
        .clone()
    {
        return catalog;
    }
    let discovered =
        Arc::new(crate::desktop_apps::discover_installed_applications().unwrap_or_default());
    catalog()
        .write()
        .unwrap_or_else(|poison| poison.into_inner())
        .get_or_insert(discovered)
        .clone()
}

/// Adopt a freshly discovered catalog, so applications installed or removed
/// since the last one get (or lose) their window icons without a restart.
pub(crate) fn publish_installed_applications(
    applications: Vec<crate::desktop_apps::DesktopApplication>,
) {
    *catalog()
        .write()
        .unwrap_or_else(|poison| poison.into_inner()) = Some(Arc::new(applications));
    icon_cache()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .clear();
}

fn resolve_application_icon_uncached(app_id: &str) -> Option<ApplicationIcon> {
    resolve_application_icon_from_applications(app_id, &installed_applications())
}

fn resolve_application_icon_from_applications(
    app_id: &str,
    applications: &[crate::desktop_apps::DesktopApplication],
) -> Option<ApplicationIcon> {
    if let Some(application) = match_desktop_application_for_app_id(app_id, applications)
        && let Some(icon) = application.icon.as_deref()
    {
        return application_icon_from_icon_value(icon);
    }

    crate::icon_theme::resolve_icon_path(app_id).map(|path| ApplicationIcon {
        name: app_id.into(),
        path: Some(path),
    })
}

fn application_icon_from_icon_value(icon: &str) -> Option<ApplicationIcon> {
    // Absolute paths and file:// URLs must exist; themed names may stay unresolved.
    if icon.contains('/') {
        let Some(canopa_assets::icons::IconSource::Path(path)) =
            canopa_assets::icons::IconSource::parse(icon)
        else {
            return None;
        };
        return path.exists().then(|| ApplicationIcon {
            name: icon.into(),
            path: Some(path),
        });
    }

    Some(ApplicationIcon {
        name: icon.into(),
        path: crate::icon_theme::resolve_icon_path(icon),
    })
}

pub(crate) use canopa_apps::match_desktop_application_for_app_id;

#[cfg(test)]
mod tests {
    use crate::desktop_apps::DesktopApplication;

    #[test]
    fn xwayland_class_resolves_desktop_icon_despite_casing() {
        let directory = tempfile::tempdir().unwrap();
        let icon_path = directory.path().join("chatgpt.png");
        std::fs::write(&icon_path, []).unwrap();
        let applications = crate::desktop_apps::applications_from_entries(
            vec![crate::desktop_apps::RawDesktopEntry {
                id: "chatgpt.desktop".into(),
                body: format!(
                    "[Desktop Entry]\nType=Application\nName=ChatGPT\nExec=chatgpt %U\nIcon={}\n",
                    icon_path.display()
                ),
                path: directory.path().join("chatgpt.desktop"),
            }],
            &crate::desktop_apps::DesktopEnvironment::unknown(),
        );

        assert_eq!(
            super::resolve_application_icon_from_applications("Chatgpt", &applications)
                .and_then(|icon| icon.path),
            Some(icon_path)
        );
    }

    fn app(id: &str, name: &str) -> DesktopApplication {
        DesktopApplication {
            id: id.into(),
            name: name.into(),
            generic_name: None,
            comment: None,
            keywords: Vec::new(),
            icon: Some(format!("{id}-icon")),
            desktop_file: "fixture.desktop".into(),
            dbus_activatable: false,
            ..Default::default()
        }
    }

    #[test]
    fn an_application_installed_later_gets_its_icon_from_the_next_catalog() {
        let id = "org.example.InstalledLater";
        super::publish_installed_applications(Vec::new());
        assert_eq!(super::resolve_application_icon(id), None);
        // The launcher read a new catalog: the cached miss is forgotten.
        super::publish_installed_applications(vec![app(&format!("{id}.desktop"), "Later")]);
        assert_eq!(
            super::resolve_application_icon(id).map(|icon| icon.name),
            Some(format!("{id}.desktop-icon"))
        );
    }

    #[test]
    fn desktop_application_matching_is_conservative() {
        let applications = vec![
            app("firefox.desktop", "Firefox"),
            app("code.desktop", "Code"),
        ];

        assert_eq!(
            super::match_desktop_application_for_app_id("firefox.desktop", &applications)
                .map(|app| app.id.as_str()),
            Some("firefox.desktop")
        );
        assert_eq!(
            super::match_desktop_application_for_app_id("firefox", &applications)
                .map(|app| app.id.as_str()),
            Some("firefox.desktop")
        );
        assert_eq!(
            super::match_desktop_application_for_app_id("org.mozilla.firefox", &applications)
                .map(|app| app.id.as_str()),
            Some("firefox.desktop")
        );
        assert_eq!(
            super::match_desktop_application_for_app_id("Firefox", &applications)
                .map(|app| app.id.as_str()),
            Some("firefox.desktop")
        );
        assert_eq!(
            super::match_desktop_application_for_app_id("Mozilla Firefox", &applications)
                .map(|app| app.id.as_str()),
            None
        );
    }

    #[test]
    fn case_insensitive_matching_covers_desktop_ids_stems_and_dns_suffixes() {
        let applications = vec![app("chatgpt.desktop", "ChatGPT")];

        for app_id in ["CHATGPT.DESKTOP", "Chatgpt", "com.openai.Chatgpt"] {
            assert_eq!(
                super::match_desktop_application_for_app_id(app_id, &applications)
                    .map(|app| app.id.as_str()),
                Some("chatgpt.desktop"),
                "{app_id}"
            );
        }
    }

    #[test]
    fn exact_matches_take_precedence_over_case_insensitive_matches() {
        let applications = vec![
            app("chatgpt.desktop", "Lowercase"),
            app("Chatgpt.desktop", "Exact"),
        ];

        for app_id in ["Chatgpt.desktop", "Chatgpt", "com.openai.Chatgpt"] {
            assert_eq!(
                super::match_desktop_application_for_app_id(app_id, &applications)
                    .map(|app| app.id.as_str()),
                Some("Chatgpt.desktop"),
                "{app_id}"
            );
        }
    }

    #[test]
    fn case_insensitive_matching_rejects_ambiguous_desktop_entries() {
        let applications = vec![
            app("chatgpt.desktop", "Lowercase"),
            app("Chatgpt.desktop", "Mixed case"),
        ];

        assert!(super::match_desktop_application_for_app_id("CHATGPT", &applications).is_none());
    }
}
