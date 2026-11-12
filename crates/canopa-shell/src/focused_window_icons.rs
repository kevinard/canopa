use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationIcon {
    pub(crate) name: String,
    pub(crate) path: Option<PathBuf>,
}

pub(crate) fn resolve_application_icon(app_id: &str) -> Option<ApplicationIcon> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<ApplicationIcon>>>> = OnceLock::new();

    if app_id.is_empty() {
        return None;
    }

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(icon) = cache
        .lock()
        .expect("focused-window icon cache poisoned")
        .get(app_id)
        .cloned()
    {
        return icon;
    }

    let resolved = resolve_application_icon_uncached(app_id);
    cache
        .lock()
        .expect("focused-window icon cache poisoned")
        .insert(app_id.to_string(), resolved.clone());
    resolved
}

/// Installed applications, discovered once per process.
pub(crate) fn installed_applications() -> &'static [crate::desktop_apps::DesktopApplication] {
    static APPLICATIONS: OnceLock<Vec<crate::desktop_apps::DesktopApplication>> = OnceLock::new();

    APPLICATIONS
        .get_or_init(|| crate::desktop_apps::discover_installed_applications().unwrap_or_default())
}

fn resolve_application_icon_uncached(app_id: &str) -> Option<ApplicationIcon> {
    resolve_application_icon_from_applications(app_id, installed_applications())
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

pub(crate) fn match_desktop_application_for_app_id<'a>(
    app_id: &str,
    applications: &'a [crate::desktop_apps::DesktopApplication],
) -> Option<&'a crate::desktop_apps::DesktopApplication> {
    if app_id.is_empty() {
        return None;
    }

    applications
        .iter()
        .find(|application| application.id == app_id)
        .or_else(|| {
            applications
                .iter()
                .find(|application| desktop_id_stem(&application.id) == Some(app_id))
        })
        .or_else(|| {
            let suffix = app_id.rsplit('.').next()?;
            applications
                .iter()
                .find(|application| desktop_id_stem(&application.id) == Some(suffix))
        })
        .or_else(|| {
            // XWayland classes can differ in case from their desktop IDs (for
            // example, Chatgpt and chatgpt.desktop). Preserve exact matches and
            // only accept an unambiguous case-insensitive fallback.
            let suffix = app_id.rsplit('.').next()?;
            let mut matches = applications.iter().filter(|application| {
                application.id.eq_ignore_ascii_case(app_id)
                    || desktop_id_stem(&application.id).is_some_and(|stem| {
                        stem.eq_ignore_ascii_case(app_id) || stem.eq_ignore_ascii_case(suffix)
                    })
            });
            let application = matches.next()?;
            matches.next().is_none().then_some(application)
        })
}

fn desktop_id_stem(id: &str) -> Option<&str> {
    id.strip_suffix(".desktop")
}

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
