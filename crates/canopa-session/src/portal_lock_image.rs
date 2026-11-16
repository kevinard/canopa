//! The lock-screen image an application chose through the Wallpaper portal.
//!
//! Canopa Shell owns the portal's image store
//! (`$XDG_STATE_HOME/canopa/shell/wallpaper/`): `selection.json` names the
//! SHA-256 of the PNG chosen for the desktop and for the lock screen. The locker
//! only reads it; an absent, malformed or vanished choice means "none".
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
struct Selection {
    version: u32,
    lockscreen: Option<String>,
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// The Shell's portal wallpaper store, or `None` without a state directory.
pub fn store() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".local/state"))
        })
        .map(|state| state.join("canopa/shell/wallpaper"))
}

/// The PNG chosen for the lock screen in `store`, if any.
pub fn lock_image_in(store: &Path) -> Option<PathBuf> {
    let regular = |path: &Path| std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file());
    if !std::fs::symlink_metadata(store).is_ok_and(|m| m.is_dir()) {
        return None;
    }
    let selection = store.join("selection.json");
    if !regular(&selection) {
        return None;
    }
    let bytes = std::fs::read(&selection)
        .ok()
        .filter(|bytes| bytes.len() <= 4096)?;
    let selection: Selection = serde_json::from_slice(&bytes).ok()?;
    let hash = selection
        .lockscreen
        .filter(|hash| selection.version == 1 && is_hash(hash))?;
    let image = store.join(format!("{hash}.png"));
    regular(&image).then_some(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn reads_only_a_valid_lock_screen_choice() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join("wallpaper");
        assert_eq!(lock_image_in(&store), None);
        std::fs::create_dir(&store).unwrap();
        let write = |json: &str| std::fs::write(store.join("selection.json"), json).unwrap();
        write(&format!(
            r#"{{"version":1,"background":null,"lockscreen":"{HASH}"}}"#
        ));
        // The chosen image must exist.
        assert_eq!(lock_image_in(&store), None);
        std::fs::write(store.join(format!("{HASH}.png")), b"png").unwrap();
        assert_eq!(
            lock_image_in(&store),
            Some(store.join(format!("{HASH}.png")))
        );
        write(r#"{"version":1,"background":null,"lockscreen":null}"#);
        assert_eq!(lock_image_in(&store), None);
        write(r#"{"version":1,"lockscreen":"../../etc/passwd"}"#);
        assert_eq!(lock_image_in(&store), None);
        write(&format!(r#"{{"version":2,"lockscreen":"{HASH}"}}"#));
        assert_eq!(lock_image_in(&store), None);
        write("not json");
        assert_eq!(lock_image_in(&store), None);
    }
}
