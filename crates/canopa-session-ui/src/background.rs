use anyhow::{Context, Result, ensure};
use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use canopa_theme::Rgb;
use image::{
    RgbImage, RgbaImage,
    imageops::{self, FilterType},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MAX_PIXELS: u64 = 33_554_432;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Fit {
    #[default]
    Fill,
    Fit,
    Stretch,
    Center,
}

pub fn dimensions(width: u32, height: u32) -> Result<usize> {
    let pixels = u64::from(width) * u64::from(height);
    ensure!(
        width > 0 && height > 0 && pixels <= MAX_PIXELS,
        "surface dimensions exceed the 32-megapixel budget"
    );
    Ok(pixels as usize * 4)
}

pub fn load(path: &Path) -> Result<RgbaImage> {
    let mut batch = RasterBatch::new(
        RasterPolicy {
            max_encoded_bytes: 64 * 1024 * 1024,
            max_dimension: 16384,
            max_pixels: MAX_PIXELS,
            decoder_allocation: DecoderAllocation::IncludingOutput(256 * 1024 * 1024),
            formats: Formats::EnabledRasterCodecs,
            file_access: FileAccess::Follow,
            apply_orientation: true,
            thumbnail: None,
        },
        RasterBudget {
            file_attempts: 1,
            encoded_bytes: 64 * 1024 * 1024,
            pixel_bytes: MAX_PIXELS as usize * 4,
        },
        &|| false,
    );
    let raster = batch
        .load(path)
        .context("image is unavailable, unsupported, or over budget")?;
    RgbaImage::from_raw(raster.width, raster.height, raster.pixels)
        .context("invalid image dimensions")
}

/// Whether an output with no assigned image shows Canopa's embedded wallpaper
/// rather than a solid backdrop.
pub fn shows_built_in(solid_color: bool, appearance: &canopa_theme::Appearance) -> bool {
    !solid_color && !appearance.high_contrast()
}

/// Decode Canopa's embedded canopy wallpaper for the appearance's color scheme.
pub fn built_in(appearance: &canopa_theme::Appearance) -> Result<RgbaImage> {
    let bytes: &[u8] = match appearance.color_scheme {
        canopa_theme::ColorScheme::Dark => include_bytes!("../assets/canopy-dark.jpg"),
        canopa_theme::ColorScheme::Light => include_bytes!("../assets/canopy-light.jpg"),
    };
    Ok(image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)?.into_rgba8())
}

pub fn render(
    path: Option<&Path>,
    fit: Fit,
    color: Rgb,
    width: u32,
    height: u32,
) -> Result<RgbImage> {
    dimensions(width, height)?;
    let source = path.map(load).transpose()?;
    render_source(source.as_ref(), fit, color, width, height)
}
/// Render an already admitted image, so callers can reuse one decode across outputs.
pub fn render_source(
    source: Option<&RgbaImage>,
    fit: Fit,
    color: Rgb,
    width: u32,
    height: u32,
) -> Result<RgbImage> {
    dimensions(width, height)?;
    let mut target = RgbImage::from_pixel(width, height, image::Rgb([color.0, color.1, color.2]));
    if let Some(source) = source {
        let (sw, sh) = source.dimensions();
        let resized = match fit {
            Fit::Fill => {
                // Crop before resizing, so a very wide source never allocates a huge intermediate.
                let ratio = f64::from(width) / f64::from(height);
                let cw = sw.min((f64::from(sh) * ratio).round().max(1.) as u32);
                let ch = sh.min((f64::from(sw) / ratio).round().max(1.) as u32);
                imageops::resize(
                    &*imageops::crop_imm(source, (sw - cw) / 2, (sh - ch) / 2, cw, ch),
                    width,
                    height,
                    FilterType::Triangle,
                )
            }
            Fit::Fit => {
                let ratio =
                    (f64::from(width) / f64::from(sw)).min(f64::from(height) / f64::from(sh));
                imageops::resize(
                    source,
                    (f64::from(sw) * ratio).round().clamp(1., f64::from(width)) as u32,
                    (f64::from(sh) * ratio).round().clamp(1., f64::from(height)) as u32,
                    FilterType::Triangle,
                )
            }
            Fit::Stretch => imageops::resize(source, width, height, FilterType::Triangle),
            Fit::Center => imageops::crop_imm(
                source,
                sw.saturating_sub(width) / 2,
                sh.saturating_sub(height) / 2,
                sw.min(width),
                sh.min(height),
            )
            .to_image(),
        };
        let (rw, rh) = resized.dimensions();
        for (x, y, pixel) in resized.enumerate_pixels() {
            let alpha = u32::from(pixel[3]);
            let dst = target.get_pixel_mut(x + (width - rw) / 2, y + (height - rh) / 2);
            for i in 0..3 {
                dst[i] = ((u32::from(pixel[i]) * alpha + u32::from(dst[i]) * (255 - alpha) + 127)
                    / 255) as u8;
            }
        }
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fit_letterboxes_fill_crops_and_alpha_is_opaque() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("wide.png");
        RgbaImage::from_pixel(8, 2, image::Rgba([255, 0, 0, 128]))
            .save(&file)
            .unwrap();
        let fit = render(Some(&file), Fit::Fit, Rgb(0, 0, 255), 4, 4).unwrap();
        assert_eq!(fit.get_pixel(0, 0).0, [0, 0, 255]);
        assert_eq!(fit.get_pixel(0, 1).0, [128, 0, 127]);
        let fill = render(Some(&file), Fit::Fill, Rgb(0, 0, 255), 4, 4).unwrap();
        assert!(fill.pixels().all(|p| p.0 == [128, 0, 127]));
        assert!(dimensions(u32::MAX, u32::MAX).is_err());
        assert!(dimensions(0, 1).is_err());
    }
}

/// Soften an image with three box passes (a close Gaussian approximation).
/// `radius` is in pixels; large radii blur a reduced copy, then enlarge it.
pub fn soften(image: &mut RgbImage, radius: f32) {
    if radius < 1. {
        return;
    }
    let (width, height) = image.dimensions();
    let factor = (radius / 6.).floor().clamp(1., 8.) as u32;
    let mut work = if factor > 1 {
        imageops::resize(
            image,
            (width / factor).max(1),
            (height / factor).max(1),
            FilterType::Triangle,
        )
    } else {
        image.clone()
    };
    let radius = ((radius / factor as f32).round() as usize).max(1);
    for _ in 0..3 {
        box_pass(&mut work, radius, true);
        box_pass(&mut work, radius, false);
    }
    *image = if factor > 1 {
        imageops::resize(&work, width, height, FilterType::Triangle)
    } else {
        work
    };
}

fn box_pass(image: &mut RgbImage, radius: usize, horizontal: bool) {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let (lines, length) = if horizontal {
        (height, width)
    } else {
        (width, height)
    };
    let stride = if horizontal { 3 } else { width * 3 };
    let step = if horizontal { width * 3 } else { 3 };
    let data: &mut [u8] = image;
    let mut line = vec![[0u32; 3]; length];
    let window = (2 * radius + 1) as u32;
    for index in 0..lines {
        let base = index * step;
        for (i, value) in line.iter_mut().enumerate() {
            let at = base + i * stride;
            *value = [
                u32::from(data[at]),
                u32::from(data[at + 1]),
                u32::from(data[at + 2]),
            ];
        }
        let sample = |i: isize| line[i.clamp(0, length as isize - 1) as usize];
        let mut sum = [0u32; 3];
        for i in -(radius as isize)..=(radius as isize) {
            let value = sample(i);
            for c in 0..3 {
                sum[c] += value[c];
            }
        }
        for i in 0..length {
            let at = base + i * stride;
            for c in 0..3 {
                data[at + c] = ((sum[c] + window / 2) / window) as u8;
            }
            let (add, remove) = (
                sample(i as isize + radius as isize + 1),
                sample(i as isize - radius as isize),
            );
            for c in 0..3 {
                sum[c] = sum[c] + add[c] - remove[c];
            }
        }
    }
}

#[cfg(test)]
mod soften_tests {
    use super::*;

    #[test]
    fn soften_spreads_an_edge_and_keeps_flat_areas_and_size() {
        let mut image = RgbImage::from_fn(200, 100, |x, _| {
            if x < 100 {
                image::Rgb([0; 3])
            } else {
                image::Rgb([255; 3])
            }
        });
        soften(&mut image, 12.);
        assert_eq!(image.dimensions(), (200, 100));
        assert!(image.get_pixel(2, 50)[0] < 8);
        assert!(image.get_pixel(197, 50)[0] > 247);
        let middle = image.get_pixel(100, 50)[0];
        assert!((60..200).contains(&middle), "edge stays sharp: {middle}");
    }
}
