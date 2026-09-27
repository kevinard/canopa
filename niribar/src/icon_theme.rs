use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub(crate) fn is_valid_icon_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\'])
}

/// Resolve a file path for a named icon by searching XDG icon theme directories.
///
/// Uses bounded indexed hicolor lookup with SVG support, falling back to the
/// historical size grid only when hicolor has no index. Results live until restart.
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
    resolve_in_roots(name, &canopa_assets::icons::search_roots())
}

fn resolve_in_roots(name: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    use canopa_assets::icons::{IconFormats, IconLookup, LookupOptions};

    IconLookup::new(
        roots,
        LookupOptions {
            theme: "hicolor",
            scale: 1,
            formats: IconFormats::PngSvgXpm,
            allow_unindexed_hicolor: true,
        },
        &|| false,
    )
    .resolve(&[name.to_owned()], 22)
}

#[cfg(test)]
mod tests {
    #[test]
    fn shell_icons_keep_svg_support_with_and_without_theme_indexes() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("hicolor/scalable/apps");
        std::fs::create_dir_all(&directory).unwrap();
        let icon = directory.join("shell-fixture.svg");
        std::fs::write(&icon, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
        let roots = [temp.path().to_owned()];
        assert_eq!(
            super::resolve_in_roots("shell-fixture", &roots),
            Some(icon.clone())
        );
        std::fs::write(temp.path().join("hicolor/index.theme"), "[Icon Theme]\nDirectories=scalable/apps\n[scalable/apps]\nSize=22\nType=Scalable\nMinSize=16\nMaxSize=128\n").unwrap();
        assert_eq!(super::resolve_in_roots("shell-fixture", &roots), Some(icon));
    }
}
