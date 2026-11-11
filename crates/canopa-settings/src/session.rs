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
    /// The user's changes to the shipped key bindings.
    pub fn shortcuts(&self) -> PathBuf {
        self.directory.join(crate::shortcuts::DELTA)
    }
    /// Shipped defaults with the shortcut changes applied, for Niri to include.
    pub fn generated_shortcuts(&self) -> PathBuf {
        self.directory.join(crate::shortcuts::GENERATED)
    }
    pub fn active(&self) -> bool {
        std::env::var_os("NIRI_CONFIG")
            .map(PathBuf::from)
            .and_then(|path| path.canonicalize().ok())
            == self.niri().canonicalize().ok()
            && self.niri().is_file()
    }
    pub fn initialize(&self, defaults: Option<&Path>) -> Result<()> {
        std::fs::create_dir_all(&self.directory)?;
        let base = self.shipped_defaults(defaults)?;
        create(&self.input(), &crate::input::Input::default().text()?)?;
        create(&self.displays(), "// Managed display preferences.\n")?;
        create(
            &self.directory.join("niri-custom.kdl"),
            "// Your additional Niri options and key bindings.\n",
        )?;
        create(
            &self.niri(),
            &format!(
                "// Canopa session; your ordinary Niri configuration is separate.\ninclude {}\ninclude \"niri-input.kdl\"\ninclude \"niri-displays.kdl\"\ninclude \"niri-custom.kdl\"\n",
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
