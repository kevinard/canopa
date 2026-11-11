//! Shell icon references through the shared `canopa-assets` resolver: the desktop
//! icon theme, its parents and hicolor, plus absolute paths and `file://` URLs.
use canopa_assets::icons::{IconQuery, IconResolver, IconSource};
use std::path::PathBuf;

/// A themed icon name, as accepted from configuration and external providers.
pub(crate) fn is_valid_icon_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\'])
}

/// A desktop-entry `Icon=` value: a themed name, an absolute path or a local file URL.
pub(crate) fn is_valid_icon_reference(value: &str) -> bool {
    IconSource::parse(value).is_some()
}

#[cfg(test)]
thread_local! {
    static LOOKUPS_ON_THREAD: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn take_lookup_count() -> usize {
    LOOKUPS_ON_THREAD.with(|count| count.replace(0))
}

/// Resolve a bar-sized icon. Results are cached by the shared resolver and
/// dropped when the icon theme or icon directories change.
pub(crate) fn resolve_icon_path(name: &str) -> Option<PathBuf> {
    resolve_icon_path_at_size(name, 22)
}

pub(crate) fn resolve_icon_path_at_size(reference: &str, size: u16) -> Option<PathBuf> {
    #[cfg(test)]
    LOOKUPS_ON_THREAD.with(|count| count.set(count.get() + 1));
    resolve_with(IconResolver::system(), reference, size, 1)
}

/// Launcher and menu rows are rasterized at 72 px from a 48 px nominal size, so
/// prefer double-density sources.
pub(crate) fn resolve_menu_icon_path(reference: &str) -> Option<PathBuf> {
    #[cfg(test)]
    LOOKUPS_ON_THREAD.with(|count| count.set(count.get() + 1));
    resolve_with(IconResolver::system(), reference, 48, 2)
}

fn resolve_with(
    resolver: &IconResolver,
    reference: &str,
    size: u16,
    scale: u16,
) -> Option<PathBuf> {
    let source = IconSource::parse(reference)?;
    let query = IconQuery {
        scale,
        // Preserve the historical fallback for hicolor installations without an index.
        allow_unindexed_hicolor: true,
        ..IconQuery::new(&[], size)
    };
    resolver.resolve_source(&source, &query, &|| false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_icons_keep_svg_support_with_and_without_theme_indexes() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("hicolor/scalable/apps");
        std::fs::create_dir_all(&directory).unwrap();
        let icon = directory.join("shell-fixture.svg");
        std::fs::write(&icon, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
        let resolver = || IconResolver::new(vec![temp.path().to_owned()], Vec::new());
        assert_eq!(
            resolve_with(&resolver(), "shell-fixture", 22, 1),
            Some(icon.clone())
        );
        std::fs::write(temp.path().join("hicolor/index.theme"), "[Icon Theme]\nDirectories=scalable/apps\n[scalable/apps]\nSize=22\nType=Scalable\nMinSize=16\nMaxSize=128\n").unwrap();
        assert_eq!(
            resolve_with(&resolver(), "shell-fixture", 22, 1),
            Some(icon.clone())
        );
        let url = format!("file://{}", icon.display());
        assert_eq!(resolve_with(&resolver(), &url, 22, 1), Some(icon.clone()));
        assert_eq!(
            resolve_with(&resolver(), icon.to_str().unwrap(), 48, 2),
            Some(icon)
        );
        assert!(is_valid_icon_reference("/opt/app/icon.png"));
        assert!(!is_valid_icon_name("/opt/app/icon.png"));
        assert!(!is_valid_icon_reference("relative/icon.png"));
    }

    #[test]
    fn shell_icons_follow_the_selected_desktop_theme() {
        let temp = tempfile::tempdir().unwrap();
        let icons = temp.path().join("icons");
        for theme in ["Papirus", "hicolor"] {
            let directory = icons.join(theme).join("48x48/apps");
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                icons.join(theme).join("index.theme"),
                "[Icon Theme]\nDirectories=48x48/apps\n[48x48/apps]\nSize=48\nType=Fixed\n",
            )
            .unwrap();
            std::fs::write(directory.join("app.png"), "png").unwrap();
        }
        let settings = temp.path().join("settings.ini");
        std::fs::write(&settings, "[Settings]\ngtk-icon-theme-name=Papirus\n").unwrap();
        let resolver = IconResolver::new(vec![icons.clone()], vec![settings]);
        assert_eq!(
            resolve_with(&resolver, "app", 48, 1),
            Some(icons.join("Papirus/48x48/apps/app.png"))
        );
    }
}
