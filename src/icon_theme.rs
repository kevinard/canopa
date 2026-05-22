use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub(crate) fn is_valid_icon_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\'])
}

/// Resolve a file path for a named icon by searching XDG icon theme directories.
///
/// Checks hicolor at multiple sizes, then pixmaps. Returns `None` for invalid
/// icon names or names that cannot be found.
pub(crate) fn resolve_icon_path(name: &str) -> Option<PathBuf> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();

    if !is_valid_icon_name(name) {
        return None;
    }

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(path) = cache
        .lock()
        .expect("icon cache poisoned")
        .get(name)
        .cloned()
    {
        return path;
    }

    let resolved = resolve_icon_path_uncached(name);
    cache
        .lock()
        .expect("icon cache poisoned")
        .insert(name.to_string(), resolved.clone());
    resolved
}

fn resolve_icon_path_uncached(name: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let xdg_data_home =
        std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share"));
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());

    let mut data_dirs: Vec<String> = vec![xdg_data_home];
    data_dirs.extend(xdg_data_dirs.split(':').map(|s| s.to_string()));

    let sizes = [
        "22x22", "24x24", "16x16", "32x32", "48x48", "64x64", "128x128", "256x256",
    ];
    let exts = ["png", "svg", "xpm"];
    let categories = ["apps", "status", "devices"];

    for base in &data_dirs {
        for cat in &categories {
            let svg = PathBuf::from(format!("{base}/icons/hicolor/scalable/{cat}/{name}.svg"));
            if svg.exists() {
                return Some(svg);
            }
        }
        for size in &sizes {
            for cat in &categories {
                for ext in &exts {
                    let p =
                        PathBuf::from(format!("{base}/icons/hicolor/{size}/{cat}/{name}.{ext}"));
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
    }

    for ext in &["png", "svg", "xpm"] {
        let p = PathBuf::from(format!("/usr/share/pixmaps/{name}.{ext}"));
        if p.exists() {
            return Some(p);
        }
    }

    None
}
