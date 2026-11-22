use crate::storage::{Snapshot, quote};
use anyhow::{Result, ensure};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub directory: PathBuf,
}
impl Paths {
    pub fn discover() -> Result<Self> {
        Ok(Self {
            directory: canopa_config::config_home()?.join("canopa"),
        })
    }
    pub fn niri(&self) -> PathBuf {
        self.directory.join("niri.kdl")
    }
    pub fn input(&self) -> PathBuf {
        self.directory.join("niri-input.kdl")
    }
    pub fn displays(&self) -> PathBuf {
        self.directory.join("niri-displays.kdl")
    }
    pub fn profiles(&self) -> PathBuf {
        self.directory.join("display-profiles.kdl")
    }
    pub fn common(&self) -> PathBuf {
        self.directory.join("config.kdl")
    }
    pub fn idle(&self) -> PathBuf {
        self.directory.join("stasis.rune")
    }
    /// The lid-close choice the Canopa session follows.
    pub fn power(&self) -> PathBuf {
        self.directory.join("power.kdl")
    }
    /// The user's changes to the shipped key bindings.
    pub fn shortcuts(&self) -> PathBuf {
        self.directory.join(crate::shortcuts::DELTA)
    }
    /// Shipped defaults with the shortcut changes applied, for Niri to include.
    pub fn generated_shortcuts(&self) -> PathBuf {
        self.directory.join(crate::shortcuts::GENERATED)
    }
    /// Titlebar style generated from the shared appearance, for Niri to include.
    pub fn titlebar_appearance(&self) -> PathBuf {
        self.directory.join(crate::titlebar::GENERATED)
    }
    /// Whether the running compositor reads this managed `niri.kdl`.
    pub fn active(&self) -> bool {
        // Unit tests often run inside a developer's Canopa session; their
        // temporary configurations must not count as the running one.
        let canopa_session = if cfg!(test) {
            None
        } else {
            std::env::var_os("CANOPA_SESSION")
        };
        self.active_with(
            std::env::var_os("NIRI_CONFIG").as_deref(),
            canopa_session.as_deref(),
        )
    }

    /// `NIRI_CONFIG` names the compositor configuration when it is set. Niri
    /// removes it from its own environment after loading, so applications it
    /// spawns (the launcher, key bindings) lack it; there `CANOPA_SESSION=1`
    /// stands in, because `canopa-session` always points Niri at this file.
    fn active_with(
        &self,
        niri_config: Option<&std::ffi::OsStr>,
        canopa_session: Option<&std::ffi::OsStr>,
    ) -> bool {
        let niri = self.niri();
        let configured = match niri_config {
            Some(path) => Path::new(path).canonicalize().ok() == niri.canonicalize().ok(),
            None => canopa_session.is_some_and(|value| value == "1"),
        };
        configured && niri.is_file()
    }
    pub fn initialize(&self, defaults: Option<&Path>) -> Result<()> {
        std::fs::create_dir_all(&self.directory)?;
        let base = self.shipped_defaults(defaults)?;
        create(&self.input(), &crate::input::Input::default().text()?)?;
        create(&self.displays(), "// Managed display preferences.\n")?;
        create(&self.titlebar_appearance(), crate::titlebar::placeholder())?;
        create(
            &self.directory.join("niri-custom.kdl"),
            "// Your additional Niri options and key bindings.\n",
        )?;
        create(
            &self.niri(),
            &format!(
                "// Canopa session; your ordinary Niri configuration is separate.\ninclude {}\ninclude \"niri-input.kdl\"\ninclude \"niri-displays.kdl\"\ninclude \"niri-appearance.kdl\"\ninclude \"niri-custom.kdl\"\n",
                quote(
                    base.to_str()
                        .ok_or_else(|| anyhow::anyhow!("session path must be UTF-8"))?
                )
            ),
        )?;
        create(
            &self.idle(),
            include_str!("../../../packaging/stasis/stasis.rune"),
        )?;
        Ok(())
    }

    /// The packaged compositor defaults: an explicit file, the system copy, or
    /// a private copy of the built-in defaults for uninstalled builds.
    pub fn shipped_defaults(&self, defaults: Option<&Path>) -> Result<PathBuf> {
        if let Some(path) = defaults {
            ensure!(
                path.is_absolute() && path.is_file(),
                "session defaults are unavailable"
            );
            return Ok(path.to_owned());
        }
        let system = PathBuf::from(SYSTEM_DEFAULTS);
        if system.is_file() {
            return Ok(system);
        }
        std::fs::create_dir_all(&self.directory)?;
        let path = self.directory.join("niri-defaults.kdl");
        create(&path, include_str!("../../../packaging/niri/defaults.kdl"))?;
        Ok(path)
    }
}

pub const SYSTEM_DEFAULTS: &str = "/usr/share/canopa/niri-defaults.kdl";

fn create(path: &Path, text: &str) -> Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Snapshot::load(path)?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Paths;
    use std::ffi::OsStr;

    fn managed() -> (tempfile::TempDir, Paths) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths {
            directory: root.path().join("canopa"),
        };
        std::fs::create_dir_all(&paths.directory).unwrap();
        std::fs::write(paths.niri(), "// managed\n").unwrap();
        (root, paths)
    }

    #[test]
    fn niri_config_names_the_compositor_configuration_when_set() {
        let (root, paths) = managed();
        let niri = paths.niri();
        assert!(paths.active_with(Some(niri.as_os_str()), None));
        let other = root.path().join("config.kdl");
        std::fs::write(&other, "").unwrap();
        // CANOPA_SESSION does not override a compositor started on another file.
        assert!(!paths.active_with(Some(other.as_os_str()), Some(OsStr::new("1"))));
    }

    #[test]
    fn the_canopa_session_counts_when_niri_removed_its_config_variable() {
        let (_root, paths) = managed();
        assert!(paths.active_with(None, Some(OsStr::new("1"))));
        assert!(!paths.active_with(None, Some(OsStr::new("0"))));
        assert!(!paths.active_with(None, None));
    }

    #[test]
    fn a_missing_managed_configuration_is_never_active() {
        let (_root, paths) = managed();
        std::fs::remove_file(paths.niri()).unwrap();
        assert!(!paths.active_with(None, Some(OsStr::new("1"))));
        assert!(!paths.active_with(Some(paths.niri().as_os_str()), None));
    }
}
