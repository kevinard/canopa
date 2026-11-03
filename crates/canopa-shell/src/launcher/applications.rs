//! MIME recommendations and file-capable desktop applications, prepared off the UI thread.
use gio::prelude::*;
pub(crate) fn for_file(
    path: &std::path::Path,
    apps: &[crate::desktop_apps::DesktopApplication],
) -> Vec<crate::desktop_apps::DesktopApplication> {
    let content_type = gio::File::for_path(path)
        .query_info(
            "standard::content-type",
            gio::FileQueryInfoFlags::NONE,
            None::<&gio::Cancellable>,
        )
        .ok()
        .and_then(|info| info.content_type());
    let recommended: std::collections::HashSet<String> = content_type
        .as_ref()
        .map(|content_type| {
            gio::AppInfo::recommended_for_type(content_type)
                .into_iter()
                .filter_map(|app| app.id().map(|id| id.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let capable: std::collections::HashSet<String> = gio::AppInfo::all()
        .into_iter()
        .filter(|app| app.supports_files() || app.supports_uris())
        .filter_map(|app| app.id().map(|id| id.to_string()))
        .collect();
    let mut choices: Vec<_> = apps
        .iter()
        .filter(|app| recommended.contains(&app.id) || capable.contains(&app.id))
        .cloned()
        .collect();
    choices.sort_by_key(|app| (!recommended.contains(&app.id), app.name.to_lowercase()));
    choices
}
