//! Persisted wallpaper choices. Configuration remains the base; choices are overlays.
use crate::{
    background::Fit,
    settings::{Background, Settings},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slideshow {
    pub directory: PathBuf,
    pub interval: u64,
    pub output: Option<String>,
    pub fit: Fit,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    version: u32,
    pub background: Option<Background>,
    pub outputs: BTreeMap<String, Background>,
    pub slideshow: Option<Slideshow>,
}
impl Default for Saved {
    fn default() -> Self {
        Self {
            version: 1,
            background: None,
            outputs: BTreeMap::new(),
            slideshow: None,
        }
    }
}
fn image_valid(bg: &Background) -> bool {
    bg.image.as_deref().is_none_or(path_valid)
}
fn path_valid(path: &Path) -> bool {
    path.is_absolute()
        && path.as_os_str().len() <= 4096
        && path
            .to_str()
            .is_some_and(|s| !s.chars().any(char::is_control))
}
fn output_valid(name: &str) -> bool {
    !name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control)
}
impl Saved {
    pub fn path() -> Option<PathBuf> {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|p| p.join(".local/state"))
            })
            .map(|p| p.join("canopa/wallpaper.json"))
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1
                && self.outputs.len() <= 64
                && self.background.as_ref().is_none_or(image_valid)
                && self
                    .outputs
                    .iter()
                    .all(|(name, bg)| output_valid(name) && image_valid(bg)),
            "invalid wallpaper assignments"
        );
        if let Some(slideshow) = &self.slideshow {
            ensure!(
                path_valid(&slideshow.directory)
                    && (10..=86400).contains(&slideshow.interval)
                    && slideshow.output.as_deref().is_none_or(output_valid),
                "invalid slideshow settings"
            );
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = canopa_file::private_state::read(path, 512 * 1024)?;
        let saved: Self = serde_json::from_slice(&bytes)?;
        saved.validate()?;
        Ok(saved)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        ensure!(bytes.len() <= 512 * 1024, "wallpaper state exceeds budget");
        canopa_file::private_state::write(path, &bytes)?;
        Ok(())
    }
    pub fn apply(&self, base: &Settings) -> Settings {
        let mut settings = base.clone();
        if let Some(background) = &self.background {
            settings.background = background.clone();
            settings.outputs.clear();
        }
        settings.outputs.extend(self.outputs.clone());
        settings
    }
    pub fn set(&mut self, output: Option<&str>, background: Background) -> Result<()> {
        ensure!(image_valid(&background), "invalid wallpaper path");
        if let Some(name) = output {
            ensure!(
                output_valid(name) && (self.outputs.contains_key(name) || self.outputs.len() < 64),
                "invalid or too many output assignments"
            );
            self.outputs.insert(name.into(), background);
        } else {
            self.background = Some(background);
            self.outputs.clear();
        }
        Ok(())
    }
}
impl Slideshow {
    pub fn images(&self) -> Result<Vec<PathBuf>> {
        ensure!(
            path_valid(&self.directory),
            "slideshow directory must be absolute"
        );
        let mut paths = std::fs::read_dir(&self.directory)?
            .take(2048)
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|p| {
                p.is_file()
                    && p.extension().and_then(|e| e.to_str()).is_some_and(|s| {
                        matches!(s.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg")
                    })
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths.truncate(256);
        ensure!(
            !paths.is_empty(),
            "slideshow directory contains no PNG/JPEG images"
        );
        Ok(paths)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use canopa_config::{Component, Request};
    #[test]
    fn output_overlays_and_all_output_choices_survive_atomic_restart() {
        let base = Settings::load(&Request::Defaults, Component::Wallpaper)
            .unwrap()
            .0;
        let mut saved = Saved::default();
        saved
            .set(
                Some("DP-1"),
                Background {
                    image: Some("/one.png".into()),
                    fit: Fit::Fit,
                },
            )
            .unwrap();
        let applied = saved.apply(&base);
        assert_eq!(
            applied.for_output("DP-1").image.as_deref(),
            Some(Path::new("/one.png"))
        );
        assert!(applied.for_output("DP-2").image.is_none());
        saved
            .set(
                None,
                Background {
                    image: Some("/all.jpg".into()),
                    fit: Fit::Fill,
                },
            )
            .unwrap();
        assert!(saved.outputs.is_empty());
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state/wallpaper.json");
        saved.save(&path).unwrap();
        let restored = Saved::load(&path).unwrap().apply(&base);
        assert_eq!(
            restored.for_output("new-monitor").image.as_deref(),
            Some(Path::new("/all.jpg"))
        );
        assert!(saved.set(Some("bad\nname"), Background::default()).is_err());
    }
}
