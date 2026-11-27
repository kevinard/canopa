pub use canopa_session_ui::background::*;

use crate::settings::Settings;
use anyhow::Result;
use image::RgbaImage;
use std::path::Path;

/// Resolve the desktop wallpaper after configured and saved assignments are applied.
pub(crate) fn wallpaper_source(
    path: Option<&Path>,
    settings: &Settings,
) -> Result<Option<RgbaImage>> {
    if let Some(path) = path {
        return load(path).map(Some);
    }
    if !shows_built_in(settings.solid_color, &settings.appearance) {
        return Ok(None);
    }
    built_in(&settings.appearance).map(Some)
}
