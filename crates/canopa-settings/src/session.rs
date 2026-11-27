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
    /// Compositor appearance generated from the shared appearance, for Niri to include.
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
        refresh_built_in_defaults(&path)?;
        Ok(path)
    }
}

pub const SYSTEM_DEFAULTS: &str = "/usr/share/canopa/niri-defaults.kdl";

/// Exact texts of `packaging/niri/defaults.kdl` that earlier builds copied
/// without a header, from the first private copy (887ec8ae) to 33c74cab.
/// Add the previous text's hash here whenever that file changes.
const PREVIOUS_DEFAULTS: &[&str] = &[
    "a561b27d965526079f261a44f5eea296d39ce629d182fcc88d407a97aa97b23b",
    "159ff29c3b7ca486dbec326f580e1513216428938208c4a7200e1983355b1c78",
    "da6ba2d5593522e0fa6af72846e91ada8d1f36f69b587188182440ed4e00cc15",
    "561acd99404024484caad3d1f972536ab004724f9624897854e375c6f6b655ae",
    "32cfd9ad6f7f1f445994e0278c97042ccc406858347fc76b8efe4fb7e64620fb",
    "d4ac13bb2cbf2958a7421453775fb050b17a1bf908782d4579fa1a3a5bd493d8",
    "e91160fa0f7cb4e8374ae46eb0fe4a5dd6ffd0db9c3cca93ce9d1d6ded2efb90",
    "a6b853dfbb034f31a330ed458980327ba742f48ca7d45afb18e5f9b0a3fda5b9",
    "f5832eb0fdfc2a96a268b29ea9dc7086d46040ccdca56a26f3d58fc4fc3be1f4",
    "65276bd1649b31260cb34c5b5cdbfde7075a0a3563be9e24c6bfcc1af422b5d6",
    "97b083420f601d4faca0433e7d2224f0a2f03f7337315888d313a7423e1e1ae4",
    "df8f03a329c64794c4e44044b4d1611897ea51ebcfa108123efb130ca46859ad",
];

fn refresh_built_in_defaults(path: &Path) -> Result<()> {
    use canopa_config::fingerprint;
    use std::os::unix::fs::MetadataExt;
    const DEFAULTS: &str = include_str!("../../../packaging/niri/defaults.kdl");
    const HEADER: &str = "// Canopa built-in defaults SHA-256: ";

    // A fallback path managed by a dotfile tool or an administrator is already
    // a source of defaults. Only consider private regular files for migration.
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.is_file() || metadata.uid() != rustix::process::geteuid().as_raw()
    }) {
        return Ok(());
    }
    let current = format!("{HEADER}{}\n{DEFAULTS}", fingerprint(DEFAULTS));
    let snapshot = Snapshot::load(path)?;
    if !snapshot.exists() {
        return create(path, &current);
    }
    if snapshot.text == current {
        return Ok(());
    }
    let unchanged = snapshot
        .text
        .split_once('\n')
        .is_some_and(|(header, body)| {
            header
                .strip_prefix(HEADER)
                .is_some_and(|sha| fingerprint(body) == sha)
        })
        || PREVIOUS_DEFAULTS.contains(&fingerprint(&snapshot.text).as_str());
    // The existing copy still starts a session, so a busy lock or a full disk
    // must not stop the login; the next one retries.
    if unchanged && let Err(error) = snapshot.save(&current, |_| Ok(())) {
        eprintln!(
            "canopa-settings: the built-in compositor defaults were not refreshed: {error:#}"
        );
    }
    Ok(())
}

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

    #[test]
    fn only_refresh_untouched_private_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("niri-defaults.kdl");
        let previous = "// Previous generated defaults\n";
        let generated = format!(
            "// Canopa built-in defaults SHA-256: {}\n{previous}",
            canopa_config::fingerprint(previous),
        );
        std::fs::write(&path, &generated).unwrap();
        super::refresh_built_in_defaults(&path).unwrap();
        let refreshed = std::fs::read_to_string(&path).unwrap();
        assert!(refreshed.ends_with(include_str!("../../../packaging/niri/defaults.kdl")));

        let edited = format!("{generated}// My defaults\n");
        std::fs::write(&path, &edited).unwrap();
        super::refresh_built_in_defaults(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), edited);
    }

    #[test]
    fn leave_managed_default_links_intact() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("managed.kdl");
        let path = directory.path().join("niri-defaults.kdl");
        let contents = "// Defaults owned by a dotfile tool\n";
        std::fs::write(&target, contents).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        super::refresh_built_in_defaults(&path).unwrap();

        assert_eq!(std::fs::read_link(&path).unwrap(), target);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), contents);
    }

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
