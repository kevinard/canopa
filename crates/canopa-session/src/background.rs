pub use canopa_session_ui::background::*;

use crate::settings::Settings;
use anyhow::Result;
use canopa_theme::ColorScheme;
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
    if settings.solid_color || settings.appearance.high_contrast() {
        return Ok(None);
    }
    let bytes: &[u8] = match settings.appearance.color_scheme {
        ColorScheme::Dark => include_bytes!("../assets/canopy-dark.jpg"),
        ColorScheme::Light => include_bytes!("../assets/canopy-light.jpg"),
    };
    let source = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)?;
    Ok(Some(source.into_rgba8()))
}
