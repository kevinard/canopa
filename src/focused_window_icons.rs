use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplicationIcon {
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

fn resolve_application_icon_uncached(app_id: &str) -> Option<ApplicationIcon> {
    static APPLICATIONS: OnceLock<Vec<crate::desktop_apps::DesktopApplication>> = OnceLock::new();

    let applications = APPLICATIONS
        .get_or_init(|| crate::desktop_apps::discover_installed_applications().unwrap_or_default());

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
    if icon.contains('/') {
        let path = PathBuf::from(icon);
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
}

fn desktop_id_stem(id: &str) -> Option<&str> {
    id.strip_suffix(".desktop")
}

#[cfg(test)]
mod tests {
    use crate::desktop_apps::DesktopApplication;

    fn app(id: &str, name: &str) -> DesktopApplication {
        DesktopApplication {
            id: id.into(),
            name: name.into(),
            generic_name: None,
            comment: None,
            keywords: Vec::new(),
            icon: Some(format!("{id}-icon")),
            exec: name.into(),
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
            None
        );
    }
}
