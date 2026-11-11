//! The desktop's selected icon theme name.
//!
//! Source of truth: `gtk-icon-theme-name` in the `[Settings]` group of GTK's
//! `settings.ini`, GTK 4 before GTK 3, in `$XDG_CONFIG_HOME` before each
//! `$XDG_CONFIG_DIRS` entry. Canopa's shared configuration and Settings portal
//! do not own an icon theme; GTK applications on a non-GNOME session read the
//! same file, so both agree. dconf (`org.gnome.desktop.interface icon-theme`)
//! is not read. Applications may still apply their own explicit override.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const MAX_SETTINGS_BYTES: usize = 64 * 1024;
const MAX_CONFIG_DIRS: usize = 16;
const REVALIDATE_AFTER: Duration = Duration::from_secs(2);

/// Candidate settings files, highest precedence first.
pub fn settings_files() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(".config")));
    let config_dirs = std::env::var_os("XDG_CONFIG_DIRS")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/etc/xdg".into());
    let mut files = Vec::new();
    for dir in config_home.into_iter().chain(
        std::env::split_paths(&config_dirs)
            .filter(|path| path.is_absolute())
            .take(MAX_CONFIG_DIRS),
    ) {
        for toolkit in ["gtk-4.0", "gtk-3.0"] {
            let file = dir.join(toolkit).join("settings.ini");
            if !files.contains(&file) {
                files.push(file);
            }
        }
    }
    files
}

/// The first valid `gtk-icon-theme-name` among `files`.
pub fn theme_from_files(files: &[PathBuf]) -> Option<String> {
    files.iter().find_map(|file| theme_from_file(file))
}

fn theme_from_file(file: &Path) -> Option<String> {
    let bytes = crate::read_bounded(file, MAX_SETTINGS_BYTES).ok()?;
    let source = std::str::from_utf8(&bytes).ok()?;
    let mut in_settings = false;
    for line in source.lines().map(str::trim) {
        if line.starts_with('[') {
            in_settings = line == "[Settings]";
            continue;
        }
        if !in_settings || line.starts_with(['#', ';']) {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == "gtk-icon-theme-name"
        {
            let value = value.trim().trim_matches('"');
            return valid_theme_name(value).then(|| value.to_owned());
        }
    }
    None
}

/// A theme directory name: one path component, ASCII, without spaces or commas.
pub fn valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name.is_ascii()
        && !matches!(name, "." | "..")
        && !name.bytes().any(|byte| {
            byte.is_ascii_control()
                || byte.is_ascii_whitespace()
                || matches!(byte, b'/' | b'\\' | b',')
        })
}

/// Modification times of the settings files, for cheap change detection.
pub fn stamp(files: &[PathBuf]) -> Vec<Option<SystemTime>> {
    files
        .iter()
        .map(|file| {
            std::fs::metadata(file)
                .and_then(|meta| meta.modified())
                .ok()
        })
        .collect()
}

/// The selected desktop theme, rechecked at most every two seconds by
/// modification time. `None` means no selection: callers use `hicolor`.
pub fn desktop_theme() -> Option<String> {
    struct Cached {
        files: Vec<PathBuf>,
        stamp: Vec<Option<SystemTime>>,
        theme: Option<String>,
        checked: Instant,
    }
    static CACHE: OnceLock<Mutex<Option<Cached>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(cached) = cache.as_mut()
        && cached.checked.elapsed() < REVALIDATE_AFTER
    {
        return cached.theme.clone();
    }
    let files = settings_files();
    let stamp = stamp(&files);
    if let Some(cached) = cache.as_mut()
        && cached.files == files
        && cached.stamp == stamp
    {
        cached.checked = Instant::now();
        return cached.theme.clone();
    }
    let theme = theme_from_files(&files);
    *cache = Some(Cached {
        files,
        stamp,
        theme: theme.clone(),
        checked: Instant::now(),
    });
    theme
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtk4_settings_precede_gtk3_and_invalid_values_are_ignored() {
        let temp = tempfile::tempdir().unwrap();
        let gtk4 = temp.path().join("gtk-4.0/settings.ini");
        let gtk3 = temp.path().join("gtk-3.0/settings.ini");
        std::fs::create_dir_all(gtk4.parent().unwrap()).unwrap();
        std::fs::create_dir_all(gtk3.parent().unwrap()).unwrap();
        std::fs::write(
            &gtk3,
            "[Settings]\ngtk-theme-name=Adwaita\ngtk-icon-theme-name = Papirus-Dark\n",
        )
        .unwrap();
        let files = [gtk4.clone(), gtk3.clone()];
        assert_eq!(theme_from_files(&files).as_deref(), Some("Papirus-Dark"));
        std::fs::write(&gtk4, "[Other]\ngtk-icon-theme-name=Wrong\n[Settings]\n# gtk-icon-theme-name=Commented\ngtk-icon-theme-name=\"Breeze\"\n").unwrap();
        assert_eq!(theme_from_files(&files).as_deref(), Some("Breeze"));
        std::fs::write(&gtk4, "[Settings]\ngtk-icon-theme-name=../escape\n").unwrap();
        assert_eq!(theme_from_files(&files).as_deref(), Some("Papirus-Dark"));
        std::fs::remove_file(&gtk3).unwrap();
        assert_eq!(theme_from_files(&files), None);
    }
}
